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
use std::fmt::{Debug, Display, Formatter, Result};
use std::hint::spin_loop;
use std::marker::PhantomData;
use std::mem::transmute;
use std::sync::atomic::Ordering;

use async_channel::{bounded, Receiver, RecvError, Sender};
use dashmap::mapref::entry::Entry;
use dashmap::DashMap;

use pi_append_vec::AppendVec;
use pi_arr::Iter;
use pi_async_rt::prelude::AsyncRuntime;
use pi_null::Null;
use pi_share::{fence, Share, ShareMutex, ShareU32, ShareU64};

use crate::archetype::{Archetype, ArchetypeDependResult, Flags};
use crate::dot::{Config, Dot};
use crate::listener::Listener;
use crate::safe_vec::SafeVec;
use crate::system::BoxedSystem;
use crate::world::{ArchetypeInit, World};

const NODE_STATUS_STEP: u32 = 0x1000_0000;
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

#[derive(Clone, Default)]
pub struct ExecGraph(Share<GraphInner>);

impl ExecGraph {
    pub fn add_system(&self, sys_index: usize, sys_name: Cow<'static, str>) -> usize {
        let inner = self.0.as_ref();
        inner.nodes.insert(Node::new_system(sys_index, sys_name))
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
    /// 初始化方法，每个图只能被执行一次， 执行前必须将system添加完毕
    /// 将system, res, archetype, 添加成图节点，并维护边
    pub fn initialize(&mut self, systems: Share<SafeVec<BoxedSystem>>, world: &mut World) {
        let inner = self.0.as_ref();
        inner
            .to_len
            .store(inner.nodes.len() as u32, Ordering::Relaxed);
        // 遍历world上的单例资源，测试和system的读写关系
        for r in world.single_res_map.iter() {
            self.add_res_node(&systems, r.key(), r.value().name(), true, world);
        }
        // 遍历world上的多例资源，测试和system的读写关系
        for r in world.multi_res_map.iter() {
            self.add_res_node(&systems, r.key(), r.value().name(), false, world);
        }
        // 遍历已有的原型，添加原型节点，添加原型和system的依赖关系产生的边
        for r in world.archetype_arr.iter() {
            self.add_archetype_node(&systems, r, world);
        }
        dbg!(
            "res & archtypes initialized",
            Dot::with_config(&self, Config::empty())
        );
        // nodes和edges整理AppendVec
        let inner = Share::<GraphInner>::get_mut(&mut self.0).unwrap();
        inner.nodes.collect();
        inner.edges.collect();
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
        // 监听原型创建， 添加原型节点和边
        let notify = Notify(self.clone(), systems, true, PhantomData);
        world.listener_mgr.register_event(Share::new(notify));
        // 整理world的监听器，合并内存
        world.listener_mgr.collect();
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
        tid: &TypeId,
        name: &Cow<'static, str>,
        single: bool,
        world: &World,
    ) {
        let inner = self.0.as_ref();
        let _unused = inner.lock.lock();
        inner.to_len.fetch_add(1, Ordering::Relaxed);
        let node_index = NodeIndex::new(inner.nodes.insert(Node::new_res(name.clone())));

        // 检查每个system和该Res的依赖关系，建立图连接
        for (system_index, node) in inner.nodes.iter().enumerate() {
            let system_index = NodeIndex::new(system_index);
            match &node.label {
                NodeType::System(sys_index, _) => {
                    let sys = unsafe { systems.load_unchecked_mut(*sys_index) };
                    let mut result = Flags::empty();
                    sys.res_depend(world, tid, name, single, &mut result);
                    if result == Flags::READ {
                        // 如果只有读，则该system为该Res的to
                        inner.add_edge(node_index, system_index);
                        continue;
                    } else if result == Flags::WRITE {
                        // 有写，则该system为该Res的from，并根据system的次序调整写的次序
                        inner.adjust_edge(system_index, node_index);
                    } else if result == Flags::SHARE_WRITE {
                        // 有写，则该system为该Res的from
                        inner.add_edge(system_index, node_index);
                    } else {
                        // 如果没有关联，则跳过
                        continue;
                    }
                }
                _ => break,
            }
        }
    }
    // 添加原型节点，添加原型和system的依赖关系产生的边。
    // 内部加锁操作，一次只能添加1个原型。
    // world的find_archetype保证了不会重复加相同的原型。
    fn add_archetype_node(
        &self,
        systems: &Share<SafeVec<BoxedSystem>>,
        archetype: &Archetype,
        world: &World,
    ) {
        let inner = self.0.as_ref();
        let _unused = inner.lock.lock();
        let id_name = (*archetype.id(), archetype.name().clone());
        // println!("add_archetype_node, id_name: {:?}", &id_name);
        // 查找图节点， 如果不存在将该原型id放入图的节点中，保存原型id到原型节点索引的对应关系
        let node_index = inner.find_node(id_name);
        let mut depend = ArchetypeDependResult::new();
        // 检查每个system和该原型的依赖关系，建立图连接
        for (system_index, node) in inner.nodes.iter().enumerate() {
            let system_index = NodeIndex::new(system_index);
            match &node.label {
                NodeType::System(sys_index, _) => {
                    let sys = unsafe { systems.load_unchecked_mut(*sys_index) };
                    depend.clear();
                    sys.archetype_depend(world, archetype, &mut depend);
                    if depend.flag.contains(Flags::WITHOUT) {
                        // 如果被排除，则跳过
                        continue;
                    }
                    if !depend.alters.is_empty() {
                        // 表结构改变，则该system为该原型的from
                        inner.adjust_edge(system_index, node_index);
                        for id_name in depend.alters.iter() {
                            // 获得该原型id到原型节点索引
                            let alter_node_index = inner.find_node(id_name.clone());
                            if alter_node_index != node_index {
                                // 过滤掉alter的原型和原原型一样
                                inner.adjust_edge(system_index, alter_node_index);
                            }
                        }
                    } else if depend.flag == Flags::READ {
                        // 如果只有读，则该system为该原型的to
                        inner.add_edge(node_index, system_index);
                        continue;
                    } else if depend.flag.bits() != 0 {
                        // 有写或者删除，则该system为该原型的from
                        inner.adjust_edge(system_index, node_index);
                    } else {
                        // 如果没有关联，则跳过
                        continue;
                    }
                }
                _ => break,
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
        assert!(inner.to_len.load(Ordering::Relaxed) > 0);
        let to_len = inner.to_len.load(Ordering::Relaxed);
        inner.to_count.store(to_len, Ordering::Relaxed);
        // println!("graph run:---------------, to_len:{}, systems_len:{}", to_len, systems.len());

        // 确保看见每节点上的from_len, from_len被某个system的Alter设置时，system结束时也会调用fence(Ordering::Release)
        fence(Ordering::Acquire);
        // 将所有节点的状态设置为Wait
        // 将graph.nodes的from_count设置为from_len
        for node in inner.nodes.iter() {
            node.status.store(NODE_STATUS_WAIT, Ordering::Relaxed);
            node.from_count
                .store(node.edge(Direction::From).0, Ordering::Relaxed);
        }
        // 从graph的froms开始执行
        for i in inner.froms.iter() {
            let node = unsafe { inner.nodes.load_unchecked(i.index()) };
            self.exec(systems, rt, world, *i, node, vec![], u32::null());
        }
        inner.receiver.recv().await
    }
    fn exec<A: AsyncRuntime>(
        &self,
        systems: &'static SafeVec<BoxedSystem>,
        rt: &A,
        world: &'static World,
        node_index: NodeIndex,
        node: &Node,
        mut vec: Vec<u32>,
        parent: u32,
    ) {
        // println!("exec, node_index: {:?}", node_index);
        match node.label {
            NodeType::System(sys_index, _) => {
                // RUN_START
                let r = node.status.fetch_add(NODE_STATUS_STEP, Ordering::Relaxed);
                if r != NODE_STATUS_WAIT {
                    panic!("status err:{}, node_index:{} node:{:?}, parent:{} vec:{:?}", r, node_index.index(), node, parent, vec)
                }else if parent == 0 {
                    let p = unsafe { self.0.as_ref().nodes.load_unchecked(parent as usize) };
                    if p.status.load(Ordering::Relaxed) != NODE_STATUS_RUN_END {
                        panic!("parent status err, node_index:{} node:{:?}, vec:{:?}", r, node, vec)
                    }
                }
                vec.push(r);
                let rt1 = rt.clone();
                let g = self.clone();
                let _ = rt.spawn(async move {
                    let sys = unsafe { systems.load_unchecked_mut(sys_index) };
                    // println!("exec, sys_index: {:?} sys:{:?}", sys_index, sys.name());
                    // 如果node为要执行的system，则执行对齐原型
                    sys.align(world);
                    let inner = g.0.as_ref();
                    let node = unsafe { inner.nodes.load_unchecked(node_index.index()) };
                    // NODE_STATUS_RUNNING
                    let r = node.status.fetch_add(NODE_STATUS_STEP, Ordering::Relaxed);
                    if r != NODE_STATUS_RUN_START {
                        panic!("run status err, node_index:{} node:{:?} vec:{:?}", r, node, vec)
                    }
                    vec.push(r);
                    sys.run(world).await;
                    g.exec_end(systems, &rt1, world, node, vec, node_index)
                });
            }
            _ => {
                // RUN_START + RUNNING
                node.status
                    .fetch_add(NODE_STATUS_STEP + NODE_STATUS_STEP, Ordering::Relaxed);
                self.exec_end(systems, rt, world, node, vec, node_index)
            }
        }
    }
    fn exec_end<A: AsyncRuntime>(
        &self,
        systems: &'static SafeVec<BoxedSystem>,
        rt: &A,
        world: &'static World,
        node: &Node,
        mut vec: Vec<u32>,
        node_index: NodeIndex,
    ) {
        // RUN_END
        let mut status =
            node.status.fetch_add(NODE_STATUS_STEP, Ordering::Relaxed) + NODE_STATUS_STEP;
        // 添加to邻居时，会锁定状态。如果被锁定，则等待锁定结束才去获取邻居
        // 如果全局同时有2个原型被添加，NODE_STATUS_RUN_END之后status又被加1，则会陷入死循环
        while status != NODE_STATUS_RUN_END {
            let s = status;
            spin_loop();
            status = node.status.load(Ordering::Relaxed);
            panic!("status err node_index:{} status:{}={} node:{:?} vec:{:?}, ", node_index.index(), s, status, node, vec);
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
        vec.clear();
        let mut it1 = it.clone();
        while let Some(n) = it1.next() {
            vec.push(n.index() as u32);
            let nn = unsafe { inner.nodes.load_unchecked(n.index()) };
            let r = nn.status.load(Ordering::Relaxed);
            if r != NODE_STATUS_WAIT {
                panic!("child status err node_index:{} child_status:{} child_node:{:?} child_index:{:?}, ", node_index.index(), r, node, nn);
            }
            vec.push(r);
        }
        vec.push(u32::null());
        if it.edge.0 == 0 {
            // 设置成结束状态
            node.status.fetch_add(NODE_STATUS_STEP, Ordering::Relaxed);
            //  to边的数量为0，表示为结束节点，减少to_count
            return inner.run_over(rt);
        }
        // 迭代to的邻居节点，减节点的from_count，如果减到0，则递归执行
        for n in it {
            let node = unsafe { inner.nodes.load_unchecked(n.index()) };
            let r = node.from_count.fetch_sub(1, Ordering::Relaxed);
            if r == 1 {
                // 减到0，表示要执行该节点
                self.exec(systems, rt, world, n, node, vec.clone(), node_index.index() as u32);
            }
        }
        // 设置成结束状态
        let r = node.status.fetch_add(NODE_STATUS_STEP, Ordering::Relaxed);
        if r != NODE_STATUS_RUN_END {
            panic!("end status err, node_index:{} node:{:?} vec:{:?}", r, node, vec)
        }
}
    // 图的整理方法， 将图和边的内存连续，去除原子操作
    pub fn collect(&mut self) {
        let inner = unsafe { Share::get_mut_unchecked(&mut self.0) };
        inner.nodes.collect();
        inner.edges.collect();
    }
}

pub struct GraphInner {
    nodes: AppendVec<Node>,
    edges: AppendVec<Edge>,
    map: DashMap<u128, NodeIndex>,
    to_len: ShareU32,
    froms: Vec<NodeIndex>,
    lock: ShareMutex<()>,
    to_count: ShareU32,
    sender: Sender<()>,
    receiver: Receiver<()>,
}

impl GraphInner {
    // 查找图节点， 如果不存在将该原型id放入图的节点中，保存原型id到原型节点索引的对应关系， 图的to_len也加1
    fn find_node(&self, id_name: (u128, Cow<'static, str>)) -> NodeIndex {
        match self.map.entry(id_name.0) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                self.to_len.fetch_add(1, Ordering::Relaxed);
                let node_index = NodeIndex::new(self.nodes.insert(Node::new_archetype(id_name)));
                entry.insert(node_index);
                node_index
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
        if big_node_index != u32::MAX && !self.has_edge(from, NodeIndex(big_node_index)) {
            self.add_edge(from, NodeIndex(big_node_index));
        }
        if small_node_index >= 0 && !self.has_edge(NodeIndex(small_node_index as u32), from) {
            self.add_edge(NodeIndex(small_node_index as u32), from);
        }
        // 将当前的from和to节点连起来
        self.add_edge(from, to);
    }
    // 判断是否from和to已经有边
    fn has_edge(&self, from: NodeIndex, to: NodeIndex) -> bool {
        for old_from in self.neighbors(to, Direction::From) {
            if old_from == from {
                // 如果已经连接了，则返回true
                return true;
            }
        }
        false
    }
    /// 添加边，3种情况， from为sys+to为ar， from为ar+to为sys， from为sys+to为sys
    /// from节点在被link时， 有可能正在执行，如果先执行后链接，则from_count不应该被加1。 如果先链接后执行，则from_count应该被加1。但代码上没有很好的方法区分两者。
    /// 因此，采用锁阻塞的方法，先将from节点的status锁加上，然后判断status为Wait，则可以from_len加1并链接，如果status为Over则不加from_len并链接。如果为Running，则等待status为Over后再进行链接。
    /// 因为采用status加1来锁定， 所以全局只能同时有1个原型被添加。
    fn add_edge(&self, from: NodeIndex, to: NodeIndex) {
        // 获得to节点
        let to_node = unsafe { self.nodes.load_unchecked(to.index()) };
        // 获得from节点
        let from_node = unsafe { self.nodes.load_unchecked(from.index()) };
        // 锁定status, exec时，就会等待解锁后，才访问to边
        let status = from_node.status.fetch_add(1, Ordering::Relaxed);
        // println!("add_edge, from:{:?}, to:{:?}, from_status:{:?}", from, to, status);
        let r = if status < NODE_STATUS_RUN_END {
            // 节点还为执行到遍历to边，先把to_node.from_count加1
            // 这一步，如果该to节点还未执行，则不会执行， 因为要等待from_count为0
            to_node.from_count.fetch_add(1, Ordering::Relaxed)
        } else if status >= NODE_STATUS_OVER {
            1
        } else {
            // 等待status为Over，如果为Over，表示exec对to边已经遍历过，可以修改to边了
            while from_node.status.load(Ordering::Relaxed) < NODE_STATUS_OVER {
                spin_loop();
            }
            1
        };

        // 获得to节点的from边数据
        let (from_edge_len, from_next_edge) = to_node.edge(Direction::From);
        let from_cur = encode(from_edge_len, from_next_edge.0);

        // 获得from节点的to边数据
        let (to_edge_len, to_next_edge) = from_node.edge(Direction::To);
        let to_cur = encode(to_edge_len, to_next_edge.0);

        // 设置边
        let e = Edge::new(from, from_next_edge, to, to_next_edge);
        // 设置from节点的to_edge, 线程安全的单链表操作
        let edge_index = EdgeIndex::new(self.edges.insert(e));
        let e = unsafe { self.edges.load_unchecked(edge_index.index()) };

        // 先将to节点的from和边连起来
        let _ = self.link_edge(
            from,
            &to_node.edges[Direction::From.index()],
            from_cur,
            from_edge_len,
            edge_index,
            &e,
            Direction::From,
        );

        // 将from节点的to和边连起来
        let old_to_len = self.link_edge(
            to,
            &from_node.edges[Direction::To.index()],
            to_cur,
            to_edge_len,
            edge_index,
            &e,
            Direction::To,
        );
        // status解锁
        from_node.status.fetch_sub(1, Ordering::Relaxed);

        // 如果from的旧的to_len值为0，表示为结束节点，现在被连起来了，要将全局的to_len减1, to_count也减1
        if old_to_len == 0 {
            self.to_len.fetch_sub(1, Ordering::Relaxed);
            self.to_count.fetch_sub(1, Ordering::Relaxed);
        }
        // 该to节点的from_count已经为0，表示正在执行或已经执行，则等待该to节点执行到RUNNING，这样返回后，确保该to节点为system，则不会看到该原型
        if r == 0 {
            while to_node.status.load(Ordering::Relaxed) < NODE_STATUS_RUNNING {
                spin_loop();
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
        // println!("run_over, {}", r);
        if r == 1 {
            let s = self.sender.clone();
            let _ = rt.spawn(async move {
                s.send(()).await.unwrap();
            });
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

pub enum NodeType {
    None,
    System(usize, Cow<'static, str>),
    Archetype((u128, Cow<'static, str>)),
    Res(Cow<'static, str>),
}
impl NodeType {
    // 类型的名字
    pub fn type_name(&self) -> &Cow<'static, str> {
        match &self {
            NodeType::None => &Cow::Borrowed("None"),
            NodeType::System(_, sys_name) => &sys_name,
            NodeType::Archetype(s) => &s.1,
            NodeType::Res(s) => &s,
        }
    }
}
impl Debug for NodeType {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match &self {
            NodeType::None => write!(f, "None"),
            NodeType::System(_, sys_name) => write!(f, "System({:?})", sys_name),
            NodeType::Archetype(s) => write!(f, "Archetype({:?})", s.1),
            NodeType::Res(s) => write!(f, "Res({:?})", s),
        }
    }
}
impl Display for NodeType {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{:#}", self.type_name())
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
    fn new_system(sys_index: usize, sys_name: Cow<'static, str>) -> Self {
        Self {
            edges: [ShareU64::new(NULL_EDGE), ShareU64::new(NULL_EDGE)],
            status: ShareU32::new(NODE_STATUS_OVER),
            label: NodeType::System(sys_index, sys_name),
            from_count: ShareU32::new(0),
        }
    }
    #[inline(always)]
    fn new_archetype(id_name: (u128, Cow<'static, str>)) -> Self {
        Self {
            edges: [ShareU64::new(NULL_EDGE), ShareU64::new(NULL_EDGE)],
            status: ShareU32::new(NODE_STATUS_OVER),
            label: NodeType::Archetype(id_name),
            from_count: ShareU32::new(0),
        }
    }
    #[inline(always)]
    fn new_res(name: Cow<'static, str>) -> Self {
        Self {
            edges: [ShareU64::new(NULL_EDGE), ShareU64::new(NULL_EDGE)],
            status: ShareU32::new(NODE_STATUS_OVER),
            label: NodeType::Res(name),
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
        unsafe {
            transmute(decode(
                self.edges.get_unchecked(d.index()).load(Ordering::Relaxed),
            ))
        }
    }
}
impl Null for Node {
    fn null() -> Self {
        Self {
            edges: [ShareU64::new(NULL_EDGE), ShareU64::new(NULL_EDGE)],
            status: ShareU32::new(NODE_STATUS_OVER),
            label: NodeType::None,
            from_count: ShareU32::new(0),
        }
    }

    fn is_null(&self) -> bool {
        match self.label {
            NodeType::None => true,
            _ => false,
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
impl Null for Edge {
    #[inline(always)]
    fn null() -> Self {
        Self([ShareU64::null(), ShareU64::null()])
    }
    #[inline(always)]
    fn is_null(&self) -> bool {
        self.0[0].is_null()
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
        self.0.add_archetype_node(&self.1, &ar.0, &ar.1);
        if self.2 {
            dbg!(Dot::with_config(&self.0, Config::empty()));
        }
    }
}
