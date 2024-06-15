//! 异步执行 静态有向无环图 的执行节点
//! 执行节点有3种，system执行节点，原型执行节点， 单例执行节点
//! 内部维护了图的节点向量和边向量
//! 执行节点采用pi_append_vec存放, 边也采用pi_append_vec
//! 执行图本身支持动态添加原型执行节点及创建相应的边，
//! 可线程安全的放入新节点和边，并线程安全的连接from和to的边
//! 如果有A对X写和Y读，则创建Y-->A和A-->X的边
//! 如果有A和B都会对X写，写不能并行，而A在B前面先写，则创建A-->B的边， 这样B就会等待A执行后再执行
//!
//! 在检查边和添加边是有时间间隔的，为了保证这个过程不会有改变，添加原型节点时需要锁住，保证不会同时添加2个原型节点。
//! 图执行时，是无锁的。执行时要遍历to边，添加时要修改to边，同时为了保证from_count被正确减少，要求执行或添加必须串行，因此通过节点状态来互相等待。
//! 图执行时，会根据节点状态等待添加节点完成，添加节点时也会根据节点状态等待节点执行完成，为了防止死锁，要求system.align方法必须不会调用添加原型节点，并尽快完成。

use std::any::TypeId;
use std::borrow::Cow;
use std::collections::HashSet;
use std::ops::Range;
use std::fmt::{Debug, Display, Formatter, Result};
use std::hint::spin_loop;
use std::marker::PhantomData;
use std::mem::transmute;
use std::sync::atomic::Ordering;

use async_channel::{bounded, Receiver, RecvError, Sender};
use dashmap::mapref::entry::Entry;
use dashmap::DashMap;

use pi_append_vec::{AppendVec, SafeVec};
use pi_arr::Iter;
use pi_async_rt::prelude::AsyncRuntime;
use pi_map::vecmap::VecMap;
use pi_null::Null;
use pi_share::{fence, Share, ShareMutex, ShareU32, ShareU64};

use crate::archetype::{Archetype, ArchetypeDependResult, Flags};
#[cfg(debug_assertions)]
use crate::column::ARCHETYPE_INDEX;
#[cfg(debug_assertions)]
use crate::column::COMPONENT_INDEX;
use crate::dot::{Config, Dot};
use crate::listener::Listener;
use crate::system::BoxedSystem;
use crate::world::{ArchetypeInit, ComponentIndex, World};

const NODE_STATUS_STEP: u32 = 0x1000_0000;
const NODE_STATUS_ALIGN_MASK: u32 = !0x1000_0001;

const NODE_STATUS_WAIT: u32 = 0;
const NODE_STATUS_RUN_START: u32 = NODE_STATUS_STEP; // 节点执行前（包括原型节点）前，状态被设为RUN_START
const NODE_STATUS_RUNNING: u32 = NODE_STATUS_RUN_START + NODE_STATUS_STEP; // system系统如果通过长度检查新原型后，状态被设为Running
const NODE_STATUS_RUN_END: u32 = NODE_STATUS_RUNNING + NODE_STATUS_STEP; // 节点执行后（包括原型节点）前，状态被设为RUN_END
const NODE_STATUS_OVER: u32 = NODE_STATUS_RUN_END + NODE_STATUS_STEP; // 节点的所有to邻居都被调用后，状态才为Over


#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Ord, Eq, Hash)]
pub struct NodeIndex(u32);
impl NodeIndex {
    #[inline]
    pub fn new(index: usize) -> Self {
        NodeIndex(index as u32)
    }
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}
impl Null for NodeIndex {
    fn null() -> Self {
        Self(u32::null())
    }

    fn is_null(&self) -> bool {
        self.0.is_null()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Ord, Eq, Hash)]
pub struct EdgeIndex(u32);
impl EdgeIndex {
    #[inline]
    pub fn new(index: usize) -> Self {
        EdgeIndex(index as u32)
    }
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}
impl Null for EdgeIndex {
    fn null() -> Self {
        Self(u32::null())
    }

    fn is_null(&self) -> bool {
        self.0.is_null()
    }
}
// Index into the NodeIndex and EdgeIndex arrays
/// Edge direction.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Ord, Eq, Hash)]
#[repr(usize)]
pub enum Direction {
    /// An `From` edge is an inbound edge *to* the current node.
    From = 0,
    /// An `To` edge is an outward edge *from* the current node.
    To = 1,
}

impl Direction {
    /// Return the opposite `Direction`.
    #[inline]
    pub const fn opposite(self) -> Direction {
        unsafe { transmute((self as usize) ^ 1) }
    }

    /// Return `0` for `To` and `1` for `From`.
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }
}

fn vec_set(vec: &mut Vec<NodeIndex>, index: usize, value: NodeIndex) {
    if vec.len() <= index {
        vec.resize(index + 1, NodeIndex::null());
    }
    *unsafe { vec.get_unchecked_mut(index) } = value;
}
#[derive(Clone)]
pub struct ExecGraph(Share<GraphInner>, pub String, pub Vec<usize>/*toop排序*/);

impl ExecGraph {
    pub fn new(name: String) -> Self {
        Self(Default::default(), name, Vec::new())
    }

    pub fn check(&self) -> Vec<usize> {
        let graph = self.0.as_ref();
        let mut ngraph = NGraph::default();
        for n in graph.nodes.iter().enumerate() {
            ngraph.add_node(n.0);
        }

        for edge in graph.edges.iter() {
            let from = edge.load(Direction::From).0;
            let to = edge.load(Direction::To).0;
            ngraph.add_edge(from.index() as usize, to.index() as usize, graph);
        }

        let cycle_keys = ngraph.build();
        match cycle_keys {
            Ok(r) => r,
            Err(cycle_keys) => if cycle_keys.len() > 0 {
                let cycle: Vec<(usize, &Node)> = cycle_keys.iter().map(|k| {(k.clone(), graph.nodes.get(*k).unwrap())}).collect();
                panic!("cycle=========={:?}", cycle);
            } else {
                Vec::default()
            },
        }
    }

    pub fn add_system(&self, sys_index: usize, sys_name: Cow<'static, str>) -> NodeIndex {
        let inner = self.0.as_ref();
        inner.to_len.fetch_add(1, Ordering::Relaxed);
        
        let index = inner.nodes.insert(Node::new(NodeType::System(sys_index, sys_name.clone())));
        // println!("find_node====={:?}", (index, inner.to_len.load(Ordering::Relaxed), &self.1, sys_name));
        //inner.map.insert((sys_index as u128, 0), NodeIndex(index as u32));
        NodeIndex(index as u32)
    }
    pub fn add_set(&self, set_name: Cow<'static, str>) -> NodeIndex {
        let inner = self.0.as_ref();
        inner.to_len.fetch_add(1, Ordering::Relaxed);
        
        let index = inner.nodes.insert(Node::new(NodeType::Set(set_name)));
        NodeIndex(index as u32)
    }
    pub fn add_edge(&self, from: NodeIndex, to: NodeIndex) {
        let inner = self.0.as_ref();
        inner.add_edge(from, to);
    }
    pub fn node_references<'a>(&'a self) -> Iter<'a, Node> {
        self.0.as_ref().nodes.iter()
    }
    pub fn edge_references<'a>(&'a self) -> Iter<'a, Edge> {
        self.0.as_ref().edges.iter()
    }
    // 获得指定节点的边迭代器
    pub fn neighbors(&self, node_index: NodeIndex, d: Direction) -> NeighborIter<'_> {
        self.0.as_ref().neighbors(node_index, d)
    }
    pub fn froms(&self) -> &Vec<NodeIndex> {
        let inner = self.0.as_ref();
        &inner.froms
    }
    pub fn to_len(&self) -> u32 {
        let inner = self.0.as_ref();
        inner.to_len.load(Ordering::Relaxed)
    }
    /// 初始化方法，每个图可被执行多次， 已经初始化的system和world上的资源和原型不会再次生成图节点
    /// 将system, res, archetype, 添加成图节点，并维护边
    pub fn initialize(&mut self, systems: Share<SafeVec<BoxedSystem>>, world: &mut World, init_notify: bool) {
        let inner = self.0.as_ref();
        let old_sys_len = inner.sys_len.load(Ordering::Relaxed);
        let new_sys_len = systems.len();
        inner.sys_len.store(new_sys_len as u32, Ordering::Relaxed);
        let range = old_sys_len as usize..new_sys_len;
        // 首先初始化新增的system，有Insert的会产生对应的原型，如果有监听器，则会立即调用add_archetype_node
        for sys in systems.slice(range.clone()) {
            sys.initialize(world);
        }
        // 遍历world上的单例资源，测试和system的读写关系
        // for r in world.single_res_map.iter() {
        //     self.add_res_node(&systems, range.clone(), r.key(), &r.value().2, true, world);
        // }
        // 遍历world上的多例资源，测试和system的读写关系
        // for r in world.multi_res_map.iter() {
        //     self.add_res_node(&systems, range.clone(), r.key(), r.value().name(), false, world);
        // }
        // 遍历已有的原型，添加原型节点，添加原型和system的依赖关系产生的边
        // for r in world.archetype_arr.iter() {
        //     self.add_archetype_node(&systems, range.clone(), r, world);
        // }
        log::trace!("res & archtypes initialized, {:?}", Dot::with_config(&self, Config::empty()));
        let _ = std::fs::write("system_graph".to_string() + self.1.as_str() + ".dot", Dot::with_config(&self, Config::empty()).to_string());

        let sort = self.check().into_iter().filter(|i| {
            match &inner.nodes[*i].label {
                NodeType::System(_, _) => true,
                _ => false,
            } 
        }).collect::<Vec<usize>>();
        // toop 排序
        self.2 = sort;
        // nodes和edges整理AppendVec
        let inner = Share::<GraphInner>::get_mut(&mut self.0).unwrap();
        inner.nodes.settle(0);
        inner.edges.settle(0);
        let mut to_len = 0;
        // 计算froms节点和to_len
        for (index, node) in inner.nodes.iter().enumerate() {
            if node.edge(Direction::From).0 == 0 {
                inner.froms.push(NodeIndex::new(index));
            }
            if node.edge(Direction::To).0 == 0 {
                to_len += 1;
            }
        }
        assert_eq!(to_len, self.to_len());
        // 如果需要初始化监听，并且图还没有添加过监听器，则添加监听器
        if init_notify && old_sys_len == 0 {
            // 监听原型创建， 添加原型节点和边
            let notify = Notify(self.clone(), systems, true, PhantomData);
            world.listener_mgr.register_event(Share::new(notify));
            // 整理world的监听器，合并内存
            world.listener_mgr.settle();
        }
        // println!(
        //     "graph initialized, froms: {:?},  to_len:{}",
        //     self.froms(),
        //     self.to_len()
        // );
        
        

    }
    // 添加单例和多例节点，添加单例多例和system的依赖关系产生的边。
    // 只会在初始化时调用一次。
    fn add_res_node(
        &self,
        systems: &Share<SafeVec<BoxedSystem>>,
        mut sys_range: Range<usize>,
        tid: &TypeId,
        name: &Cow<'static, str>,
        single: bool,
        world: &World,
    ) {
        let inner = self.0.as_ref();
        let _unused = inner.lock.lock();
        let id = unsafe { transmute(*tid) };
        // 如果图已经存在该节点，则返回，否则插入
        let (node_index, is_new) = inner.find_node((id, 0u32.into()), NodeType::Res(name.clone()), &self.1);
        if is_new {// 如果该资源为新的，则遍历全部system节点，否则只遍历新增的system节点
            sys_range.start = 0;
        }
        // 检查每个system和该Res的依赖关系，建立图连接
        // 因为没有维护system_index到图节点id的对应关系，所以需要遍历全部的图节点
        for (system_index, node) in inner.nodes.iter().enumerate() {
            let system_index = NodeIndex::new(system_index);
            match &node.label {
                NodeType::System(sys_index, _) if sys_range.start <= *sys_index && *sys_index < sys_range.end => {
                    let sys = unsafe { systems.load_unchecked(*sys_index) };
                    let mut result = Flags::empty();
                    // todo sys.res_depend(world, tid, name, single, &mut result);
                    if result == Flags::READ {
                        // 如果只有读，则该system为该Res的to
                        // inner.add_edge(node_index, system_index);
                        continue;
                    } else if result == Flags::WRITE {
                        // 有写，则该system为该Res的from，并根据system的次序调整写的次序
                        // inner.adjust_edge(system_index, node_index);
                    } else if result == Flags::SHARE_WRITE {
                        // 共享写，则该system为该Res的from
                        // inner.add_edge(system_index, node_index);
                    } else {
                        // 如果没有关联，则跳过
                        continue;
                    }
                }
                _ => (),
            }
        }
    }
    // 添加原型节点，添加原型和system的依赖关系产生的边。
    // 内部加锁操作，一次只能添加1个原型。
    // world的find_archetype保证了不会重复加相同的原型。
    fn add_archetype_node(
        &self,
        systems: &Share<SafeVec<BoxedSystem>>,
        mut sys_range: Range<usize>,
        archetype: &Archetype,
        world: &World,
    ) {
        let inner = self.0.as_ref();
        let _unused = inner.lock.lock();

        let aid = archetype.id() as u128;
        let mut nodes = Vec::new();
        let mut ar_component_index_node_index_map = Vec::new();
        // 遍历该原型的全部组件
        for c in archetype.get_columns().iter() {
            let info = c.info();
            // 查找图节点， 如果不存在将该原型组件id放入图的节点中，保存原型id到原型节点索引的对应关系
            let (node_index, _is_new) = inner.find_node((aid, info.index), NodeType::ArchetypeComponent(aid, info.type_name().clone()), &self.1);
            vec_set(&mut ar_component_index_node_index_map, info.index.index(), node_index);
            nodes.push(node_index);
        }
        // if is_new {// 如果该资源为新的，则遍历全部system节点，否则只遍历新增的system节点
        sys_range.start = 0;
        // }
        let mut depend = ArchetypeDependResult::new();
        // 检查每个system和该原型的依赖关系，建立图连接
        for (system_index, node) in inner.nodes.iter().enumerate() {
            let system_index = NodeIndex::new(system_index);
            match &node.label {
                NodeType::System(sys_index, _) => {
                    let sys = unsafe { systems.load_unchecked(*sys_index) };
                    depend.clear();
                    // todo sys.archetype_depend(world, archetype, &mut depend);
                    if depend.flag.contains(Flags::WITHOUT) {
                        // 如果被排除，则跳过
                        continue;
                    }
                    if !depend.alters.is_empty() {
                        // 表结构改变，则该system为该原型全部组件的from
                        inner.adjust_edges(&nodes, system_index, NodeIndex::null());
                        for infos in depend.alters.iter() {
                            let aid = infos.0;
                            // 获得该原型id到原型节点索引
                            for info in infos.2.iter() {
                                let (alter_node_index, _) = inner.find_node((aid, info.index), NodeType::ArchetypeComponent(aid, info.type_name().clone()), &self.1);
                                // 该system为该原型全部组件的from
                                inner.adjust_edge(system_index, alter_node_index);
                                nodes.push(alter_node_index);
                            }
                        }
                    } else if depend.flag == Flags::READ {
                        // 如果只有读，则该system为该原型组件的to
                        for index in depend.reads.iter() {
                            let node_index = ar_component_index_node_index_map[index.index()];
                            inner.add_edge(node_index, system_index);
                        }
                        continue;
                    } else if depend.flag.bits() != 0 {
                        // 有写或者删除，则该system为该原型的from
                        for index in depend.writes.iter() {
                            let node_index = ar_component_index_node_index_map[index.index()];
                            inner.adjust_edge(system_index, node_index);
                        }
                    } else {
                        // 如果没有关联，则跳过
                        continue;
                    }
                }
                _ => (),
            }
        }
        for node_index in nodes {
            let node = unsafe { inner.nodes.load_unchecked(node_index.index()) };
            let old_from_count = node.from_count.fetch_sub(1, Ordering::Relaxed);
            // println!("old_from_count======{:?}, {:?}", old_from_count, node_index.index());
            if old_from_count == 1 {
                // 只有当其他图的某系统S1创建该原型， 而当前图中不存在S1系统是，出现此情况， 此时需要给当前图添加froms
                // 当前图此时一定处于未运行状态，可以直接安全的修改froms
                let inner = unsafe{&mut *(Share::as_ptr(&self.0) as usize as *mut GraphInner)};
                inner.froms.push(node_index);
            }
        }
    }

    pub async fn run<A: AsyncRuntime>(
        &self,
        systems: &'static Share<SafeVec<BoxedSystem>>,
        rt: &A,
        world: &'static World,
    ) -> std::result::Result<(), RecvError> {
        let inner = self.0.as_ref();
        // let to_len = inner.to_len.load(Ordering::Relaxed);
        // if to_len == 0 {
        //     return Ok(());
        // }
        // inner.to_count.store(to_len, Ordering::Relaxed);
       

        // 确保看见每节点上的from_len, from_len被某个system的Alter设置时，system结束时也会调用fence(Ordering::Release)
        // fence(Ordering::Acquire);
        // 将所有节点的状态设置为Wait
        // 将graph.nodes的from_count设置为from_len
        // for node in inner.nodes.iter() {
        //     node.status.store(NODE_STATUS_WAIT, Ordering::Relaxed);
        //     node.from_count
        //         .store(node.edge(Direction::From).0, Ordering::Relaxed);
            
        // }
    
        // println!("graph run:---------------, to_len:{}, systems_len:{}, node_len: {:?}, \ndiff: {:?}", to_len, systems.len(), inner.nodes.len(), 
        //     inner.nodes.iter().enumerate().filter(|r| {
        //         for i in inner.froms.iter() {
        //             if i.index() == r.0 {
        //                 return false;
        //             }
        //         }
        //         return true;
        //     }).map(|r| {
        //         r.0
        //     }).collect::<Vec<usize>>()
        // );

        // 从graph的froms开始执行
        // println!("run !!!!===={:?}", (&self.1, inner.froms.len(), inner.froms.iter().map(|r| {r.index()}).collect::<Vec<usize>>()));
        #[cfg(debug_assertions)]
        if COMPONENT_INDEX.load(std::sync::atomic::Ordering::Relaxed)  < std::usize::MAX ||  ARCHETYPE_INDEX.load(std::sync::atomic::Ordering::Relaxed)  < std::usize::MAX
        {
            println!("run====={:?}, {:?}", &self.1,  self.2.len());
        }
        // let t = pi_time::Instant::now();
        for i in self.2.iter() {
            let node = unsafe { inner.nodes.load_unchecked(*i) };
            
            match node.label {
                NodeType::System(sys_index, _) => {
                    // println!("RUN_START=========={:?}", (node_index.index(), node.label()));
                    // let rt1 = rt.clone();
                    // let g = self.clone();
                    // let inner = g.0.as_ref();
                    let sys = unsafe { systems.load_unchecked(sys_index) };
                    // let old_status = node.status.fetch_add(NODE_STATUS_STEP, Ordering::Relaxed);
                    // println!("exec, sys_index: {:?} sys:{:?}", sys_index, sys.name());
                    // 如果node为要执行的system，并且未被锁定原型，则执行对齐原型
                    // if old_status & NODE_STATUS_ALIGN_MASK == 0 {
                        sys.align(world);
                    // }
                    // NODE_STATUS_RUNNING
                    // node.status.fetch_add(NODE_STATUS_STEP, Ordering::Relaxed);
                    #[cfg(debug_assertions)]
                    if COMPONENT_INDEX.load(std::sync::atomic::Ordering::Relaxed) < std::usize::MAX  ||  ARCHETYPE_INDEX.load(std::sync::atomic::Ordering::Relaxed)  < std::usize::MAX
                {
                    println!("run start===={:?}", sys.name());
                }
                    #[cfg(feature = "trace")]
                    {
                        use tracing::Instrument;
                        let system_span = tracing::info_span!("system", name = &**sys.name());
                        sys.run(world).instrument(system_span).await;
                    }
                    #[cfg(not(feature = "trace"))]
                    sys.run(world).await;
                }
                _ => {
                    // RUN_START + RUNNING
                    // node.status
                    //     .fetch_add(NODE_STATUS_STEP + NODE_STATUS_STEP, Ordering::Relaxed);
                    // self.exec_end(systems, rt, world, node, node_index)
                }
            }

        }
        // println!("run====={:?}, {:?}, {:?}", &self.1,  self.2.len(), pi_time::Instant::now() - t);
        // println!("run1 !!!!===={}", inner.froms.len());
        // let r = inner.receiver.recv().await;
        // println!("run2 !!!!===={}", inner.froms.len());
        // r
        Ok(())
        
    }


    fn exec<A: AsyncRuntime>(
        &self,
        systems: &'static SafeVec<BoxedSystem>,
        rt: &A,
        world: &'static World,
        node_index: NodeIndex,
        node: &Node,
    ) {
        // println!("exec, node_index: {:?}", node_index);
        match node.label {
            NodeType::System(sys_index, _) => {
                // println!("RUN_START=========={:?}", (node_index.index(), node.label()));
                let rt1 = rt.clone();
                let g = self.clone();
                let _ = rt.spawn(async move {
                    let inner = g.0.as_ref();
                    let node = unsafe { inner.nodes.load_unchecked(node_index.index()) };
                    let sys = unsafe { systems.load_unchecked(sys_index) };
                    let old_status = node.status.fetch_add(NODE_STATUS_STEP, Ordering::Relaxed);
                    // println!("exec, sys_index: {:?} sys:{:?}", sys_index, sys.name());
                    // 如果node为要执行的system，并且未被锁定原型，则执行对齐原型
                    if old_status & NODE_STATUS_ALIGN_MASK == 0 {
                        sys.align(world);
                    }
                    
                    // NODE_STATUS_RUNNING
                    node.status.fetch_add(NODE_STATUS_STEP, Ordering::Relaxed);
                    // println!("run start===={:?}", sys.name());
                    #[cfg(feature = "trace")]
                    {
                        use tracing::Instrument;
                        let system_span = tracing::info_span!("system", name = &**sys.name());
                        sys.run(world).instrument(system_span).await;
                    }
                    #[cfg(not(feature = "trace"))]
                    sys.run(world).await;
                    // println!("run end===={:?}", sys.name());
                    g.exec_end(systems, &rt1, world, node, node_index)
                });
            }
            _ => {
                // RUN_START + RUNNING
                node.status
                    .fetch_add(NODE_STATUS_STEP + NODE_STATUS_STEP, Ordering::Relaxed);
                self.exec_end(systems, rt, world, node, node_index)
            }
        }
    }
    fn exec_end<A: AsyncRuntime>(
        &self,
        systems: &'static SafeVec<BoxedSystem>,
        rt: &A,
        world: &'static World,
        node: &Node,
        index: NodeIndex,
    ) {
        // println!("exec_end===={:?}", node.label());
        // RUN_END
        let mut status =
            node.status.fetch_add(NODE_STATUS_STEP, Ordering::Relaxed) + NODE_STATUS_STEP;
        // 添加to邻居时，会锁定状态。如果被锁定，则等待锁定结束才去获取邻居
        // 如果全局同时有2个原型被添加，NODE_STATUS_RUN_END之后status又被加1，则会陷入死循环
        while status & 1 == 1 {
            spin_loop();
            status = node.status.load(Ordering::Relaxed);
        }
        let inner = self.0.as_ref();
        // 执行后，检查to边的数量
        // 创建to邻居节点的迭代器
        let it = NeighborIter::new(inner, Direction::To, node.edge(Direction::To));
        // let mut it1 = it.clone();
        // print!("exec next, node_index: {:?}, [", node_index);
        // while let Some(n) = it1.next() {
        //     print!("n:{:?}, ", n);
        // }
        // println!("]");
        // println!("exec_end====={:?}", (node.label(), it.edge.0, it.clone().map(|r| {r.index()}).collect::<Vec<usize>>()));
        // println!("to====={:?}", (index, it.edge.0, it.clone().map(|r| {
        //     let node = unsafe { inner.nodes.load_unchecked(r.index()) };
        //     let from_count = node.from_count.load(Ordering::Relaxed);

        //     let from = NeighborIter::new(inner, Direction::From, node.edge(Direction::From));

        //     (r.index(), from_count as usize, from.map(|r| {r.index()}).collect::<Vec<usize>>())
        // }).collect::<Vec<(usize, usize, Vec<usize>)>>(), node.label(), inner.to_count.load(Ordering::Relaxed)));
        if it.edge.0 == 0 {
            // 设置成结束状态
            node.status.fetch_add(NODE_STATUS_STEP, Ordering::Relaxed);
            // println!("run_over====={:?}", (index, node.label(), inner.to_count.load(Ordering::Relaxed)));
            //  to边的数量为0，表示为结束节点，减少to_count
            return inner.run_over(rt);
        }
        // 迭代to的邻居节点，减节点的from_count，如果减到0，则递归执行
        for n in it {
            let node = unsafe { inner.nodes.load_unchecked(n.index()) };
            let r = node.from_count.fetch_sub(1, Ordering::Relaxed);
            if r == 1 {
                // 减到0，表示要执行该节点
                self.exec(systems, rt, world, n, node);
            }
        }
        // 设置成结束状态
        node.status.fetch_add(NODE_STATUS_STEP, Ordering::Relaxed);
}
    // 图的整理方法， 将图和边的内存连续，去除原子操作
    pub fn settle(&mut self) {
        let inner = unsafe { Share::get_mut_unchecked(&mut self.0) };
        inner.nodes.settle(0);
        inner.edges.settle(0);
    }
}

pub struct GraphInner {
    nodes: AppendVec<Node>,
    edges: AppendVec<Edge>,
    map: DashMap<(u128, ComponentIndex), NodeIndex>,
    to_len: ShareU32,
    froms: Vec<NodeIndex>,
    lock: ShareMutex<()>,
    to_count: ShareU32,
    sender: Sender<()>,
    receiver: Receiver<()>,
    sys_len: ShareU32,
}

impl GraphInner {

    // 查找图节点， 如果不存在将该label放入图的节点中，保存id到图节点索引的对应关系， 图的to_len也加1
    fn find_node(&self, id: (u128, ComponentIndex), label: NodeType, name: &str) -> (NodeIndex, bool) {
        match self.map.entry(id) {
            Entry::Occupied(entry) => (entry.get().clone(), false),
            Entry::Vacant(entry) => {
                self.to_len.fetch_add(1, Ordering::Relaxed);
                let n = Node::new(label.clone());
                n.from_count.fetch_add(1, Ordering::Relaxed);
                let node_index = NodeIndex::new(self.nodes.insert(n));
                entry.insert(node_index);
                // println!("find_node====={:?}", (node_index.index(), self.to_len.load(Ordering::Relaxed), name, label));
                (node_index, true)
            }
        }
    }

    fn adjust_edges(&self, vec: &Vec<NodeIndex>, from: NodeIndex, to: NodeIndex) {
        if from.is_null() {
            for node_index in vec {
                self.adjust_edge(*node_index, to);
            }
        }else{
            for node_index in vec {
                self.adjust_edge(from, *node_index);
            }
        }
    }
    // 尝试link2个节点，检查并调整to节点的from，如果已经连接，则忽略
    // from是系统节点，to原型节点，to的所有的from节点，都是system节点，全遍历，根据先后，将当前的from添加前后边
    fn adjust_edge(&self, from: NodeIndex, to: NodeIndex) {
        // println!("adjust_edge, from:{:?}, to:{:?}", from, to);
        let mut big_node_index = u32::MAX;
        let mut small_node_index = -1;
        for old_from in self.neighbors(to, Direction::From) {
            if old_from < from {
                // 如果from_node_index比当前system小
                if old_from.index() as i32 > small_node_index {
                    // 则取from_node_index和small_node_index大的那个
                    small_node_index = old_from.index() as i32;
                }
            } else if old_from > from {
                if old_from.0 < big_node_index {
                    // 则取from_node_index和small_node_index小的那个
                    big_node_index = old_from.0;
                }
            } else {
                // 如果已经连接了，则返回
                return;
            }
        }
        // println!("adjust_edge1, from:{:?}, to:{:?}", big_node_index, small_node_index);
        // if big_node_index != u32::MAX && !self.has_edge(from, NodeIndex(big_node_index)) {
        //     self.add_edge(from, NodeIndex(big_node_index));
        // }
        // if small_node_index >= 0 && !self.has_edge(NodeIndex(small_node_index as u32), from) {
        //     self.add_edge(NodeIndex(small_node_index as u32), from);
        // }
        // 将当前的from和to节点连起来
        self.add_edge(from, to);
    }
    // 判断是否from和to已经有边
    fn has_edge(&self, from: NodeIndex, to: NodeIndex) -> Option<(NodeIndex, NodeIndex)> {
        let it = self.neighbors(to, Direction::From);
        // if from.0 == 30 {
        //     let f = unsafe { self.nodes.load_unchecked(from.index()) };
        //     let fe = f.edge(Direction::To);
        //     let p = unsafe { self.nodes.load_unchecked(to.index()) };
        //     let v = p.edge(Direction::From);
        //     let pp = self.nodes.load(to.index());
        //     println!("has_edge from:{}, from_p:{:p}, {:?}. to:{}, to_p:{:p}, {:?}, it: {:?}, b:{}, capacity:{}", from.0, f, fe, to.0, p, v, it.edge, pp.is_some(), self.nodes.vec_capacity());
        // }
        for old_from in it {
            if old_from == from {
                // 如果已经连接了，则返回true
                return Some((from, to));
            }
        }
        None
    }
    /// 添加边，3种情况， from为sys+to为ar， from为ar+to为sys， from为sys+to为sys
    /// from节点在被link时， 有可能正在执行，如果先执行后链接，则from_count不应该被加1。 如果先链接后执行，则from_count应该被加1。但代码上没有很好的方法区分两者。
    /// 因此，采用锁阻塞的方法，先将from节点的status锁加上，然后判断status为Wait，则可以from_len加1并链接，如果status为Over则不加from_len并链接。如果为Running，则等待status为Over后再进行链接。
    /// 因为采用status加1来锁定， 所以全局只能同时有1个原型被添加。
    fn add_edge(&self, from: NodeIndex, to: NodeIndex) {
        if self.has_edge(from, to).is_some() {
            return;
        }
        // 获得to节点
        let to_node = unsafe { self.nodes.load_unchecked(to.index()) };
        // 获得from节点
        let from_node = unsafe { self.nodes.load_unchecked(from.index()) };
        // 锁定status, exec时，就会等待解锁后，才访问to边
        let status = from_node.status.fetch_add(1, Ordering::Relaxed);
        // println!("add_edge, from:{:?}, to:{:?}, from_status:{:?}", from, to, status);
        let old_from = if status < NODE_STATUS_RUN_END {
            // 节点还为执行到遍历to边，先把to_node.from_count加1
            // 这一步，如果该to节点还未执行，则不会执行， 因为要等待from_count为0
            to_node.from_count.fetch_add(1, Ordering::Relaxed)
        } else if status >= NODE_STATUS_OVER {
            1
        } else {
            // 正在遍历to边，等待status为Over，如果为Over，表示exec对to边已经遍历过，可以修改to边了
            while from_node.status.load(Ordering::Relaxed) < NODE_STATUS_OVER {
                spin_loop();
            }
            1
        };

        // 获得to节点的from边数据
        let (from_edge_len, from_next_edge) = to_node.edge(Direction::From);
        // let from_cur = encode(from_edge_len, from_next_edge.0);
        // if from.0 == 30 {
        //     println!("add_edge: to:{}, to_p:{:p} {:?}", to.0, to_node, (from_edge_len, from_next_edge));
        // }
        // 获得from节点的to边数据
        let (to_edge_len, to_next_edge) = from_node.edge(Direction::To);
        // let to_cur = encode(to_edge_len, to_next_edge.0);

        // 设置边
        let e = Edge::new(from, from_next_edge, to, to_next_edge);
        // 设置from节点的to_edge, 线程安全的单链表操作
        let edge_index = EdgeIndex::new(self.edges.insert(e));
        // let e = unsafe { self.edges.load_unchecked(edge_index.index()) };

        // 先将to节点的from和边连起来
        to_node.edges[Direction::From.index()].store(encode(from_edge_len + 1, edge_index.0), Ordering::Relaxed);
        // let _ = self.link_edge(
        //     from,
        //     &to_node.edges[Direction::From.index()],
        //     from_cur,
        //     from_edge_len,
        //     edge_index,
        //     &e,
        //     Direction::From,
        // );

        // 将from节点的to和边连起来
        from_node.edges[Direction::To.index()].store(encode(to_edge_len + 1, edge_index.0), Ordering::Relaxed);
        // if from.0 == 30 {
        //     let (flen, fed) = decode(from_node.edges[Direction::To.index()].load(Ordering::Relaxed));
        //     let (len, ed) = decode(to_node.edges[Direction::From.index()].load(Ordering::Relaxed));
        //     println!("add=====, from: {} {}, to:{} {} {}", flen, fed, to.0, len, ed);
        // }
        // let old_to_len = self.link_edge(
        //     to,
        //     &from_node.edges[Direction::To.index()],
        //     to_cur,
        //     to_edge_len,
        //     edge_index,
        //     &e,
        //     Direction::To,
        // );
        // status解锁
        from_node.status.fetch_sub(1, Ordering::Relaxed);

       
        // if from.index() == 144 || to.index() == 144 {
        //     println!("add_edge====={:?}", (from, to, old_to_len, self.to_len.load(Ordering::Relaxed), from_node.label(), to_node.label(), ));
        // }

        // 如果from的旧的to_len值为0，表示为结束节点，现在被连起来了，要将全局的to_len减1, to_count也减1
        if to_edge_len == 0 {
            self.to_len.fetch_sub(1, Ordering::Relaxed);
            self.to_count.fetch_sub(1, Ordering::Relaxed);
        }
        // 该to节点的from_count已经为0，表示正在执行（本身依赖为0的节点， 一定已经将任务派发出去了， 可以认为是正在执行的状态）
        if old_from == 0 {
            // 则添加状态2锁定原型， 使得还未进行原型对齐的to执行时不进行原型对齐；
            // 不进行原型对齐的原因： 
            //     假定系统S1生成原生A1， 系统S2对A1存在只读引用，他们在并行执行（注意， 在A1原型生成之前，S1和S2可能不存在先后顺序，他们可以并行）
            //     如果S2系统进行原型对齐， 将能看到A1，那么， S1写入A1与S2读取A1并行执行，可能存在数据不一致的情况
            let mut state = to_node.status.fetch_add(2, Ordering::Relaxed);

            // 如果该to正处于对齐阶段，则等待，直到to进入到RUNNING状态，以保证to看不到原型
            if state & NODE_STATUS_RUN_START == NODE_STATUS_RUN_START {
                while state < NODE_STATUS_RUNNING {
                    spin_loop();
                    state = to_node.status.load(Ordering::Relaxed);
                }
            }
        }
    }

    // 将节点和指定方向和边连起来, 线程安全的无锁单链表操作
    fn link_edge(
        &self,
        node_index: NodeIndex,
        node_edge: &ShareU64,
        mut cur: u64,
        mut edge_len: u32,
        edge_index: EdgeIndex,
        edge: &Edge,
        d: Direction,
    ) -> u32 {
        // 先尝试替换
        cur = match node_edge.compare_exchange(
            cur,
            encode(edge_len + 1, edge_index.0),
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return edge_len,
            Err(old) => old,
        };
        let next_edge = loop {
            spin_loop();
            // 更新next_edge
            let r = decode(cur);
            edge_len = r.0;
            // 再次尝试将当前边设到节点上，cur_edge_len+1
            cur = match node_edge.compare_exchange(
                cur,
                encode(edge_len + 1, edge_index.0),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break EdgeIndex(r.1),
                Err(old) => old,
            };
        };
        // 节点的边设置成功后，将边的节点重新设置
        edge.store(d, node_index, next_edge);
        return edge_len;
    }

    // 获得指定节点的边迭代器
    fn neighbors(&self, node_index: NodeIndex, d: Direction) -> NeighborIter<'_> {
        let edge = if let Some(node) = self.nodes.load(node_index.index()) {
            node.edge(d)
        } else {
            (0, EdgeIndex(u32::null()))
        };
        NeighborIter::new(self, d, edge)
    }

    // 尝试run是否over
    fn run_over<A: AsyncRuntime>(&self, rt: &A) {
        let r = self.to_count.fetch_sub(1, Ordering::Relaxed);
        // println!("run_over!!! to_count: {}", r);
        if r == 1 {
            let s = self.sender.clone();
            let _ = rt.spawn(async move {
                s.send(()).await.unwrap();
            });
        } else if r == 5 {
            // for n in self.nodes.iter() {
            //     // println!("next exec====={:?}", (n.label(), n.from_count.load(Ordering::Relaxed)));
            //     if n.from_count.load(Ordering::Relaxed) != 0 {
            //         println!("next exec====={:?}", (n.label(), n.from_count.load(Ordering::Relaxed)));
            //         // v.push(format!("{:?}", ));
            //     }
            //     // // let it = NeighborIter::new(inner, Direction::To, node.edge(Direction::To));
            //     // println!("next exec====={:?}", v.join("\n"));
            // }
        }
    }
}

impl Default for GraphInner {
    fn default() -> Self {
        let (sender, receiver) = bounded(1);
        Self {
            nodes: Default::default(),
            edges: Default::default(),
            map: Default::default(),
            to_len: ShareU32::new(0),
            froms: Default::default(),
            lock: ShareMutex::new(()),
            to_count: ShareU32::new(0),
            sender,
            receiver,
            sys_len: ShareU32::new(0),
        }
    }
}
#[derive(Clone)]
pub struct NeighborIter<'a> {
    inner: &'a GraphInner,
    d: Direction,
    edge: (u32, EdgeIndex),
}
impl<'a> NeighborIter<'a> {
    fn new(inner: &'a GraphInner, d: Direction, edge: (u32, EdgeIndex)) -> Self {
        Self { inner, d, edge }
    }
}
impl<'a> Iterator for NeighborIter<'a> {
    type Item = NodeIndex;
    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.edge.0 == 0 {
            return None;
        }
        self.edge.0 -= 1;
        let edge = unsafe { self.inner.edges.load_unchecked(self.edge.1.index()) };
        let (node_index, next_edge) = edge.load(self.d);
        self.edge.1 = next_edge;
        Some(node_index)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.edge.0 as usize, Some(self.edge.0 as usize))
    }
}
impl Debug for NeighborIter<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

#[derive(Clone)]
pub enum NodeType {
    None,
    System(usize, Cow<'static, str>),
    ArchetypeComponent(u128, Cow<'static, str>),
    Res(Cow<'static, str>),
    Set(Cow<'static, str>),
}
impl NodeType {
    // 类型的名字
    pub fn type_name(&self) -> &Cow<'static, str> {
        match &self {
            NodeType::None => &Cow::Borrowed("None"),
            NodeType::System(_, sys_name) => &sys_name,
            NodeType::ArchetypeComponent(_, s) => &s, // 要改一下，但是先这样吧
            NodeType::Res(s) => &s,
            NodeType::Set(s) => &s,
        }
    }
}
impl Debug for NodeType {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match &self {
            NodeType::None => write!(f, "None"),
            NodeType::System(_, sys_name) => write!(f, "System({:?})", sys_name),
            NodeType::ArchetypeComponent(id, s) => write!(f, "ArchetypeComponent({},{:?})", id, s),
            NodeType::Res(s) => write!(f, "Res({:?})", s),
            NodeType::Set(s) => write!(f, "Set({:?})", s),
        }
    }
}
impl Display for NodeType {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match &self {
            NodeType::None => write!(f, "None"),
            NodeType::System(_, sys_name) => write!(f, "System({:?})", sys_name),
            NodeType::ArchetypeComponent(id, s) => write!(f, "ArchetypeComponent({},{:?})", id, s),
            NodeType::Res(s) => write!(f, "Res({:?})", s),
            NodeType::Set(s) => write!(f, "Set({:?})", s),
        }
    }
}

// 空边， 长度为0， next_edge为null
const NULL_EDGE: u64 = encode(0, u32::MAX);

pub struct Node {
    // edges的索引，from在0位， to在1位。ShareU64里，低32位是edge总数量。高32位是第一个edge的索引。
    edges: [ShareU64; 2],
    status: ShareU32,
    label: NodeType,
    // from edge的计数，每from执行一次，会减一
    from_count: ShareU32,
}
impl Node {
    #[inline(always)]
    fn new(label: NodeType) -> Self {
        Self {
            edges: [ShareU64::new(NULL_EDGE), ShareU64::new(NULL_EDGE)],
            status: ShareU32::new(NODE_STATUS_WAIT),
            label,
            from_count: ShareU32::new(0),
        }
    }

    #[inline(always)]
    pub fn label(&self) -> &NodeType {
        &self.label
    }
    #[inline(always)]
    pub fn is_system(&self) -> bool {
        match self.label {
            NodeType::System(_, _) => true,
            _ => false,
        }
    }
    #[inline(always)]
    pub fn edge(&self, d: Direction) -> (u32, EdgeIndex) {
        let (len, edge) = decode(unsafe { self.edges.get_unchecked(d.index()).load(Ordering::Relaxed) });
        (len, EdgeIndex(edge))
    }
}
impl Default for Node {
    fn default() -> Self {
        Self {
            edges: [ShareU64::new(NULL_EDGE), ShareU64::new(NULL_EDGE)],
            status: ShareU32::new(NODE_STATUS_OVER),
            label: NodeType::None,
            from_count: ShareU32::new(0),
        }
    }
}
impl Debug for Node {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let (from_len, from) = self.edge(Direction::From);
        let (to_len, to) = self.edge(Direction::To);
        f.debug_struct("Node")
            .field("label", &self.label)
            .field("from_len", &from_len)
            .field("from_first_edge", &from)
            .field("to_len", &to_len)
            .field("to_first_edge", &to)
            .finish()
    }
}

/// 边。from在0位， to在1位。ShareU64里，低32位是节点的索引。高32位是下一个edge的索引。
pub struct Edge([ShareU64; 2]);

impl Edge {
    #[inline(always)]
    fn new(
        from_node: NodeIndex,
        from_next_edge: EdgeIndex,
        to_node: NodeIndex,
        to_next_edge: EdgeIndex,
    ) -> Self {
        Self([
            ShareU64::new(encode(from_node.0, from_next_edge.0)),
            ShareU64::new(encode(to_node.0, to_next_edge.0)),
        ])
    }
    #[inline(always)]
    pub fn load(&self, d: Direction) -> (NodeIndex, EdgeIndex) {
        unsafe {
            transmute(decode(
                self.0.get_unchecked(d.index()).load(Ordering::Relaxed),
            ))
        }
    }
    #[inline(always)]
    fn store(&self, d: Direction, node: NodeIndex, next_edge: EdgeIndex) {
        unsafe {
            self.0
                .get_unchecked(d.index())
                .store(encode(node.0, next_edge.0), Ordering::Relaxed)
        };
    }
}
impl Default for Edge {
    #[inline(always)]
    fn default() -> Self {
        Self([ShareU64::null(), ShareU64::null()])
    }
}
impl Debug for Edge {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let (from_node, from_next_edge) = self.load(Direction::From);
        let (to_node, to_next_edge) = self.load(Direction::To);
        f.debug_struct("Edge")
            .field("from_node", &from_node)
            .field("from_next_edge", &from_next_edge)
            .field("to_node", &to_node)
            .field("to_next_edge", &to_next_edge)
            .finish()
    }
}

#[inline(always)]
pub(crate) const fn encode(low: u32, high: u32) -> u64 {
    (low as u64) | ((high as u64) << 32)
}
#[inline(always)]
pub(crate) const fn decode(value: u64) -> (u32, u32) {
    let low = value & 0xffff_ffff;
    let high = value >> 32;
    (low as u32, high as u32)
}

struct Notify<'a>(
    ExecGraph,
    Share<SafeVec<BoxedSystem>>,
    bool,
    PhantomData<&'a ()>,
);
impl<'a> Listener for Notify<'a> {
    type Event = ArchetypeInit<'a>;

    #[inline(always)]
    fn listen(&self, ar: Self::Event) {
        // self.0.add_archetype_node(&self.1, 0..self.1.len(), &ar.0, &ar.1);
        // log::trace!("{:?}", Dot::with_config(&self.0, Config::empty()));
        // let _ = std::fs::write("system_graph".to_string() + self.0.1.as_str() + ".dot", Dot::with_config(&self.0, Config::empty()).to_string());
        // self.0.check();
    }
}


/// 图节点
#[derive(Debug, Clone)]
pub struct NGraphNode {
    // 该节点的入度节点
    from: Vec<usize>,
    // 该节点的出度节点
    to: Vec<usize>,
	
}

#[derive(Default)]
pub struct NGraph {
    nodes: pi_map::vecmap::VecMap<NGraphNode>,
    from: Vec<usize>,
    to: Vec<usize>,
    edges: HashSet<(usize, usize)>,
}

impl NGraph {
	/// 如果parent_graph_id是Null， 表示插入到根上
    pub fn add_node(&mut self, k: usize) {
        if self.nodes.contains(k) {
            panic!("节点已经存在{:?}", k);
        }
        self.nodes.insert(k, NGraphNode {
            from: Vec::new(),
            to: Vec::new(),
        });
    }

    pub fn add_edge(&mut self, before: usize, after: usize, graph: &GraphInner) {
        if self.edges.contains(&(before, after)) {
            // return;
            panic!("边已经存在！！{:?}", (before, after));
        }
        self.edges.insert((before, after));
		let before_node = self.nodes.get_mut(before).unwrap();
        before_node.to.push(after);

        let after_node = self.nodes.get_mut(after).unwrap();
        after_node.from.push(before);
    }

    pub fn build(&mut self) -> std::result::Result<Vec<usize>, Vec<usize>>  {
        let mut queue = std::collections::VecDeque::new();
        let mut counts: VecMap<usize> = VecMap::with_capacity(self.nodes.len());
        for k in self.nodes.iter().enumerate() {
            if let Some(r) = k.1 {
                if r.from.is_empty() {
                    queue.push_back(k.0);
                }
                counts.insert(k.0, r.from.len());
            }
        }

        let nodes = &self.nodes;
        let mut topological = Vec::new();
        let mut topological_len = 0;
        while let Some(k) = queue.pop_front() { // 遍历依赖就绪的节点
			let node = nodes.get(k).unwrap();
			topological.push(k);
			topological_len += 1;
            
			
            // 处理 from 的 下一层
           
			// println!("from = {:?}, to: {:?}", k, &node.to);
            // 遍历节点的后续节点
            for to in &node.to  {
				// println!("graph's each = {:?}, count = {:?}", to, counts[*to]);
				counts[*to] -= 1;
                // handle_set.insert(*to, ());
				if counts[*to] == 0 {
					queue.push_back(*to);
				}
            }
        }

		// 如果拓扑排序列表的节点数等于图中的总节点数，则返回拓扑排序列表，否则返回空列表（说明图中存在环路）
		if topological_len == nodes.len() {
			// topological = topos;
			return Ok(topological);
		}

		let not_contains = nodes.iter().enumerate().map(|k|{k.clone()}).filter(|r| {
            match r.1 {
                Some(r) => r,
                None => return false,
            };
			let is_not_contains = !topological.contains(&r.0);

			return  is_not_contains;
		}).map(|r| {r.0}).collect::<Vec<usize>>();

        // println!("cycle1======{:?}", not_contains);
		let mut iter = not_contains.into_iter();
		while let Some(n) = iter.next() {
			let mut cycle_keys = Vec::new();
			Self::find_cycle(nodes, n, &mut cycle_keys, Vec::new(), HashSet::default());

			if cycle_keys.len() > 0 {
				return Err(cycle_keys);
			}
		}

        Err(Default::default())
    }

    // 寻找循环依赖
    fn find_cycle(map: &VecMap<NGraphNode>, node: usize, nodes: &mut Vec<usize>, mut indexs: Vec<usize>, mut nodes_set: HashSet<usize>) {
		nodes.push(node.clone());
        nodes_set.insert(node);
        indexs.push(0);
        // println!("find_cycle======{:?}, {:?}", nodes, map.len());
        // let mut i = 0;
        while nodes.len() > 0 {
            let index = nodes.len() - 1;
            let k = &nodes[index];
            let n = map.get(*k).unwrap();
            let to = &n.to;
            let child_index = indexs[index];
            // if i < 500 {
            //     //   println!("pop====={:?}", (index, k, child_index, to.len(), &n.from, to));
            //       i += 1;
            // }
          
            if child_index >= to.len() {
                if let Some(r) = nodes.pop() {
                    nodes_set.remove(&r);
                }
                indexs.pop();
               
                continue
            }
            let child = to[child_index].clone();
            // if child == node {
            //     break;
            // }

            if nodes_set.contains(&child) {
                let i = nodes.iter().position(|r| {*r == child}).unwrap();
                let r = nodes[i..].to_vec();
                *nodes = r;
                break;
            }
            indexs[index] += 1;
            nodes.push(child);
            nodes_set.insert(child);
            indexs.push(0);
        }
    }

}
