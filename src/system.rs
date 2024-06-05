use std::{
    any::TypeId,
    borrow::Cow,
    collections::HashMap,
    fmt::Debug,
    future::Future,
    mem::take,
    ops::Range,
    pin::Pin,
};

use pi_share::Share;

use crate::{
    archetype::{Archetype, ComponentInfo, ShareArchetype},
    column::Column,
    world::{ComponentIndex, World},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeInfo {
    pub type_id: TypeId,
    pub type_name: Cow<'static, str>,
}
impl TypeInfo {
    pub fn of<T: 'static>() -> Self {
        TypeInfo {
            type_id: TypeId::of::<T>(),
            type_name: Cow::Borrowed(std::any::type_name::<T>()),
        }
    }
}

/// The metadata of a [`System`].
#[derive(Clone, Debug, Default)]
pub struct ReadWrite {
    pub(crate) reads: HashMap<TypeId, Cow<'static, str>>, // 该系统所有读的组件
    pub(crate) writes: HashMap<TypeId, Cow<'static, str>>, // 该系统所有写的组件。用来和读进行判断，不允许一个组件又读又写
    pub(crate) withs: HashMap<TypeId, Cow<'static, str>>,
    pub(crate) withouts: HashMap<TypeId, Cow<'static, str>>,
}
impl ReadWrite {
    pub fn merge(&mut self, rw: ReadWrite) {
        self.reads.extend(rw.reads);
        self.writes.extend(rw.writes);
        self.withs.extend(rw.withs);
        self.withouts.extend(rw.withouts);
    }
    pub fn contains(&self, sub: &ReadWrite) -> Result<(), Cow<'static, str>> {
        Self::check(&self.reads, &sub.reads)?;
        Self::check(&self.writes, &sub.writes)
    }
    pub fn check(
        map: &HashMap<TypeId, Cow<'static, str>>,
        sub: &HashMap<TypeId, Cow<'static, str>>,
    ) -> Result<(), Cow<'static, str>> {
        for (id, name) in sub.iter() {
            if !map.contains_key(id) {
                return Err(name.clone());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum Relation<T: Eq> {
    With(T),
    Without(T),
    Read(T),
    Write(T),
    OptRead(T),
    OptWrite(T),
    Count(usize),
    Or,
    And,
    End,
}
impl<T: Eq> Relation<T> {
    pub fn replace(self, arg: T) -> Self {
        match self {
            Relation::With(_) => Relation::With(arg),
            Relation::Without(_) => Relation::Without(arg),
            Relation::Read(_) => Relation::Read(arg),
            Relation::Write(_) => Relation::Write(arg),
            Relation::OptRead(_) => Relation::OptRead(arg),
            Relation::OptWrite(_) => Relation::OptWrite(arg),
            _ => self,
        }
    }
    pub fn without(&self, arg: &mut T) -> bool {
        match self {
            Relation::With(id) if id == arg => true,
            Relation::Read(id) if id == arg => true,
            Relation::Write(id) if id == arg => true,
            _ => false,
        }
    }
    pub fn read(&self, arg: &mut T) -> bool {
        match self {
            Relation::Write(id) if id == arg => false,
            Relation::OptWrite(id) if id == arg => false,
            _ => true,
        }
    }
    pub fn write(&self, arg: &mut T) -> bool {
        match self {
            Relation::Read(id) if id == arg => true,
            Relation::OptRead(id) if id == arg => false,
            Relation::Write(id) if id == arg => false,
            Relation::OptWrite(id) if id == arg => false,
            _ => true,
        }
    }
    pub fn node(&self) -> Option<bool> {
        match self {
            Relation::And => Some(true),
            Relation::Or => Some(false),
            _ => None,
        }
    }
    pub fn is_end(&self) -> bool {
        match self {
            Relation::End => true,
            _ => false,
        }
    }
}
#[derive(Debug, Default, Clone, Copy)]
pub struct RelateNode {
    is_and: bool,
    result: bool,
}
impl RelateNode {
    pub fn new(is_and: bool) -> Self {
        Self {
            is_and,
            result: is_and,
        }
    }
    pub fn ok(&mut self, result: bool) {
        if self.is_and {
            self.result &= result;
        } else {
            self.result |= result;
        }
    }
}
/// 用指定的函数和参数，遍历关系列表
pub fn traversal<T: Eq, F, A>(
    vec: &Vec<Relation<T>>,
    f: &F,
    arg: &mut A,
    start: &mut usize,
    mut node: RelateNode,
) -> bool
where
    F: Fn(&Relation<T>, &mut A) -> bool,
{
    while *start < vec.len() {
        let r = unsafe { vec.get_unchecked(*start) };
        *start += 1;
        if r.is_end() {
            // 返回node的结果
            return node.result;
        }
        let b = if let Some(is_and) = r.node() {
            // 创建新的node，递归遍历
            traversal(vec, f, arg, start, RelateNode::new(is_and))
        } else {
            f(r, arg)
        };
        node.ok(b);
    }
    node.result
}

struct ArchetypeFilter<'a>(&'a Archetype);

// 判断原型是否和该关系相关
fn archetype_relate<'a>(r: &Relation<ComponentIndex>, arg: &mut ArchetypeFilter<'a>) -> bool {
    match r {
        Relation::Without(i) => !arg.0.contains(*i),
        Relation::With(i) => arg.0.contains(*i),
        Relation::Read(i) => arg.0.contains(*i),
        Relation::Write(i) => arg.0.contains(*i),
        Relation::Count(i) => arg.0.get_columns().len() == *i,
        _ => true,
    }
}


#[derive(Debug, Default)]
pub struct Related {
    pub(crate) vec: Vec<Relation<ComponentIndex>>,
    // pub(crate) map: HashMap<TypeId, Cow<'static, str>>,
}
impl Related {
    // 检查新旧读写在reads或writes是否完全不重合
    pub fn check_conflict(&self, other: &Related) {
        // 先检查withouts
        if self.check_without(other) || other.check_without(self) {
            return;
        }
        assert_eq!(self.check_rw(other), None);
        assert_eq!(other.check_rw(self), None);
    }
    // 检查other的每一个without，和self的with read或writes判断，返回true表示查询完全不重合
    pub fn check_without(&self, other: &Related) -> bool {
        for w in other.vec.iter() {
            match w {
                Relation::Without(t) => {
                    let mut t = *t;
                    let mut start = 0;
                    if traversal(
                        &self.vec,
                        &Relation::without,
                        &mut t,
                        &mut start,
                        RelateNode::new(false),
                    ) {
                        return true;
                    }
                }
                _ => (),
            }
        }
        false
    }
    // 检查自身数据集是否读写冲突, 返回Some(ComponentIndex)表示冲突
    pub fn check_self(&self) -> Option<ComponentIndex> {
        for i in 0..self.vec.len() {
            let r = unsafe { self.vec.get_unchecked(i) };
            match r {
                Relation::Read(t) => {
                    if !self.check_read(*t, i + 1) {
                        return Some(*t);
                    }
                }
                Relation::OptRead(t) => {
                    if !self.check_read(*t, i + 1) {
                        return Some(*t);
                    }
                }
                Relation::Write(t) => {
                    if !self.check_write(*t, i + 1) {
                        return Some(*t);
                    }
                }
                Relation::OptWrite(t) => {
                    if !self.check_write(*t, i + 1) {
                        return Some(*t);
                    }
                }
                _ => (),
            }
        }
        None
    }
    // 检查数据集是否读写冲突, 返回Some(ComponentIndex)表示冲突
    pub fn check_rw(&self, other: &Related) -> Option<ComponentIndex> {
        for w in other.vec.iter() {
            match w {
                Relation::Read(t) => {
                    if !self.check_read(*t, 0) {
                        return Some(*t);
                    }
                }
                Relation::OptRead(t) => {
                    if !self.check_read(*t, 0) {
                        return Some(*t);
                    }
                }
                Relation::Write(t) => {
                    if !self.check_write(*t, 0) {
                        return Some(*t);
                    }
                }
                Relation::OptWrite(t) => {
                    if !self.check_write(*t, 0) {
                        return Some(*t);
                    }
                }
                _ => (),
            }
        }
        None
    }
    // 检查数据集是否读冲突
    pub fn check_read(&self, mut t: ComponentIndex, mut start: usize) -> bool {
        // println!("check_read {:?}", (t, start));
        traversal(
            &self.vec,
            &Relation::read,
            &mut t,
            &mut start,
            RelateNode::new(true),
        )
    }
    // 检查数据集是否读冲突
    pub fn check_write(&self, mut t: ComponentIndex, mut start: usize) -> bool {
        traversal(
            &self.vec,
            &Relation::write,
            &mut t,
            &mut start,
            RelateNode::new(true),
        )
    }
    // 判断该原型是否相关
    pub fn relate(&self, archetype: &Archetype, mut start: usize) -> bool {
        let mut filter = ArchetypeFilter(archetype);
        traversal(
            &self.vec,
            &archetype_relate,
            &mut filter,
            &mut start,
            RelateNode::new(true),
        )
    }
    // 获取该原型的每组件的读写依赖
    pub fn depend(&self, archetype: &Archetype) {
        todo!()
    }
}
/// The metadata of a [`System`].
pub struct SystemMeta {
    pub(crate) type_info: TypeInfo,
    pub(crate) vec: Vec<Share<Related>>, // SystemParam参数的组件关系列表
    pub(crate) cur_related: Related,     // 当前SystemParam参数的关系
    pub(crate) param_set_locations: Vec<Range<usize>>, // 参数集在vec的位置
    pub(crate) res_related: Vec<Relation<TypeId>>, // Res资源的关系

    
    pub(crate) res_map: HashMap<TypeId, (u8, Cow<'static, str>)>, // 参数集的读写依赖
    pub(crate) map: HashMap<TypeId, Cow<'static, str>>, // 当前参数的读写依赖
    pub(crate) components: ReadWrite,    // 该系统所有组件级读写依赖
    pub(crate) cur_param: ReadWrite,     // 当前参数的读写依赖
    pub(crate) param_set: ReadWrite,     // 参数集的读写依赖
    pub(crate) res_reads: HashMap<TypeId, Cow<'static, str>>, // 读Res
    pub(crate) res_writes: HashMap<TypeId, Cow<'static, str>>, // 写ResMut
}

impl SystemMeta {
    pub fn new(type_info: TypeInfo) -> Self {
        Self {
            type_info,
            vec: Default::default(),
            cur_related: Default::default(),
            param_set_locations: Default::default(),
            res_related: Default::default(),
            res_map: Default::default(),
            map: Default::default(),
            components: Default::default(),
            cur_param: Default::default(),
            param_set: Default::default(),
            res_reads: Default::default(),
            res_writes: Default::default(),
        }
    }
    /// Returns the system's type_id
    #[inline]
    pub fn type_id(&self) -> &TypeId {
        &self.type_info.type_id
    }

    /// Returns the system's name
    #[inline]
    pub fn type_name(&self) -> &str {
        &self.type_info.type_name
    }
    /// 组件关系，返回组件索引和组件对应的列
    pub fn component_relate(
        &mut self,
        world: &mut World,
        info: ComponentInfo,
        r: Relation<ComponentIndex>,
    ) -> (ComponentIndex, Share<Column>) {
        let rc = world.add_component_info(info);
        self.cur_related.vec.push(r.replace(rc.0));
        rc
    }
    /// 将插入实体对应的组件
    pub fn insert(&mut self, world: &mut World, components: Vec<ComponentInfo>) -> ShareArchetype {
        // 所有对应的组件都是写
        for info in &components {
            self.cur_related.vec.push(Relation::Write(info.index));
        }
        // 在关联分析上为了精确关联原型，加一个Count(usize)
        self.cur_related.vec.push(Relation::Count(components.len()));
        self.related_ok();
        world.find_ar(components)
    }

    /// 用当前的关系表记录关系
    pub fn relate(&mut self, r: Relation<ComponentIndex>) {
        self.cur_related.vec.push(r);
    }
    /// 关系表结束
    pub fn related_ok(&mut self) -> Share<Related> {
        let ar = Share::new(take(&mut self.cur_related));
        self.vec.push(ar.clone());
        ar
    }
    pub fn param_set_start(&mut self) {
        self.param_set_locations
            .push(self.vec.len()..self.vec.len());
    }
    pub fn param_set_end(&mut self) {
        self.param_set_locations.last_mut().unwrap().end = self.vec.len();
    }








    /// 当前参数检查通过
    pub fn cur_param_ok(&mut self) {
        // 检查前面查询的rw是否有组件读写冲突
        Self::check_rw(&self.components, &self.cur_param);
        self.components.merge(take(&mut self.cur_param));
    }
    /// 参数集检查读写
    pub fn param_set_check(&mut self) {
        // 检查前面查询的rw是否有组件读写冲突
        Self::check_rw(&self.components, &self.cur_param);
        self.param_set.merge(take(&mut self.cur_param));
    }
    /// 参数集检查通过
    pub fn param_set_ok(&mut self) {
        self.components.merge(take(&mut self.param_set));
    }

    // 检查冲突
    pub fn check_conflict(&self) {
        for r in self.vec.iter() {
            // 先检查自身
            assert_eq!(r.check_self(), None);
        }
        let mut range_it = self.param_set_locations.iter();
        let mut set_loc = range_it.next();
        for mut i in 0..self.vec.len() {
            let r = &self.vec[i];
            if let Some(range) = set_loc {
                if range.end == i {
                    // 本次param_set_location结束，跳到下一个param_set_location
                    set_loc = range_it.next();
                } else if range.start <= i {
                    // 跳过参数集内的检查
                    i = range.end - 1;
                }
            }
            i += 1;
            // 依次和后面的Related比较
            for j in i..self.vec.len() {
                let r2 = &self.vec[j];
                r.check_conflict(r2);
            }
        }
    }
    // 检查withouts，without在reads或writes中，表示查询完全不重合
    // 检查新旧读写在reads或writes是否完全不重合
    pub fn check_rw(old: &ReadWrite, rw: &ReadWrite) {
        // 先检查withouts
        if Self::check_without(&old.withouts, &rw) || Self::check_without(&rw.withouts, old) {
            return;
        }
        assert_eq!(Self::check_w(&old.reads, &rw.writes), None);
        assert_eq!(Self::check_w(&old.writes, &rw.writes), None);
        assert_eq!(Self::check_w(&rw.reads, &old.writes), None);
    }
    pub fn check_without(withouts: &HashMap<TypeId, Cow<'static, str>>, rw: &ReadWrite) -> bool {
        for w in withouts.keys() {
            if rw.reads.contains_key(w) || rw.writes.contains_key(w) || rw.withs.contains_key(w) {
                return true;
            }
        }
        false
    }
    // 检查数据集是否和写冲突
    pub fn check_w(
        map: &HashMap<TypeId, Cow<'static, str>>,
        writes: &HashMap<TypeId, Cow<'static, str>>,
    ) -> Option<Cow<'static, str>> {
        for t in map.iter() {
            if writes.contains_key(t.0) {
                return Some(t.1.clone());
            }
        }
        None
    }
    pub fn res_read(&mut self, type_info: &TypeInfo) {
        if self.res_writes.contains_key(&type_info.type_id) {
            panic!("res_read conflict, name:{}", type_info.type_name);
        }
        self.res_reads
            .insert(type_info.type_id, type_info.type_name.clone());
    }
    pub fn res_write(&mut self, type_info: &TypeInfo) {
        if self.res_reads.contains_key(&type_info.type_id) {
            panic!("res_write read conflict, name:{}", type_info.type_name);
        }
        if self.res_writes.contains_key(&type_info.type_id) {
            panic!("res_write write conflict, name:{}", type_info.type_name);
        }
        self.res_writes
            .insert(type_info.type_id, type_info.type_name.clone());
    }
}

impl Debug for SystemMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemMeta")
            .field("type_info", &self.type_info)
            .finish()
    }
}
pub trait System: Send + Sync + 'static {
    /// Returns the system's name.
    fn name(&self) -> &Cow<'static, str>;
    /// Returns the [`TypeId`] of the underlying system type.
    fn type_id(&self) -> TypeId;
    /// Initialize the system.
    fn initialize(&mut self, world: &mut World);
    // /// system depend the archetype.
    // fn archetype_depend(
    //     &self,
    //     world: &World,
    //     archetype: &Archetype,
    //     result: &mut ArchetypeDependResult,
    // );
    // /// system depend the res.
    // fn res_depend(
    //     &self,
    //     world: &World,
    //     res_tid: &TypeId,
    //     res_name: &Cow<'static, str>,
    //     single: bool,
    //     result: &mut Flags,
    // );

    /// system align the world archetypes
    fn align(&mut self, world: &World);

    // /// Runs the system with the given input in the world. Unlike [`System::run`], this function
    // /// can be called in parallel with other systems and may break Rust's aliasing rules
    // /// if used incorrectly, making it unsafe to call.
    // ///
    // /// # Safety
    // ///
    // /// - The caller must ensure that `world` has permission to access any world data
    // ///   registered in [`Self::archetype_component_access`]. There must be no conflicting
    // ///   simultaneous accesses while the system is running.
    // /// - The method [`Self::update_archetype_component_access`] must be called at some
    // ///   point before this one, with the same exact [`World`]. If `update_archetype_component_access`
    // ///   panics (or otherwise does not return for any reason), this method must not be called.
    // fn run(&mut self, world: &World);
    // fn async_run(&mut self, _world: &World) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> {
    //     Box::pin(async move {})
    // }
}

pub trait RunSystem: System {
    /// Runs the system with the given input in the world. Unlike [`System::run`], this function
    /// can be called in parallel with other systems and may break Rust's aliasing rules
    /// if used incorrectly, making it unsafe to call.
    ///
    /// # Safety
    ///
    /// - The caller must ensure that `world` has permission to access any world data
    ///   registered in [`Self::archetype_component_access`]. There must be no conflicting
    ///   simultaneous accesses while the system is running.
    /// - The method [`Self::update_archetype_component_access`] must be called at some
    ///   point before this one, with the same exact [`World`]. If `update_archetype_component_access`
    ///   panics (or otherwise does not return for any reason), this method must not be called.
    fn run(&mut self, world: &World);
}

pub trait AsyncRunSystem: System {
    /// Runs the system with the given input in the world. Unlike [`System::run`], this function
    /// can be called in parallel with other systems and may break Rust's aliasing rules
    /// if used incorrectly, making it unsafe to call.
    ///
    /// # Safety
    ///
    /// - The caller must ensure that `world` has permission to access any world data
    ///   registered in [`Self::archetype_component_access`]. There must be no conflicting
    ///   simultaneous accesses while the system is running.
    /// - The method [`Self::update_archetype_component_access`] must be called at some
    ///   point before this one, with the same exact [`World`]. If `update_archetype_component_access`
    ///   panics (or otherwise does not return for any reason), this method must not be called.
    fn run(&mut self, _world: &'static World)
        -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
}

/// A convenience type alias for a boxed [`System`] trait object.
pub enum BoxedSystem {
    Sync(Box<dyn RunSystem>),
    Async(Box<dyn AsyncRunSystem>),
}

impl BoxedSystem {
    pub fn name(&self) -> &Cow<'static, str> {
        match self {
            BoxedSystem::Sync(s) => s.name(),
            BoxedSystem::Async(s) => s.name(),
        }
    }

    pub fn type_id(&self) -> TypeId {
        match self {
            BoxedSystem::Sync(s) => s.type_id(),
            BoxedSystem::Async(s) => s.type_id(),
        }
    }

    pub fn initialize(&mut self, world: &mut World) {
        match self {
            BoxedSystem::Sync(s) => s.initialize(world),
            BoxedSystem::Async(s) => s.initialize(world),
        }
    }

    // pub fn archetype_depend(
    //     &self,
    //     world: &World,
    //     archetype: &Archetype,
    //     result: &mut ArchetypeDependResult,
    // ) {
    //     match self {
    //         BoxedSystem::Sync(s) => s.archetype_depend(world, archetype, result),
    //         BoxedSystem::Async(s) => s.archetype_depend(world, archetype, result),
    //     }
    // }

    // pub fn res_depend(
    //     &self,
    //     world: &World,
    //     res_tid: &TypeId,
    //     res_name: &Cow<'static, str>,
    //     single: bool,
    //     result: &mut Flags,
    // ) {
    //     match self {
    //         BoxedSystem::Sync(s) => s.res_depend(world, res_tid, res_name, single, result),
    //         BoxedSystem::Async(s) => s.res_depend(world, res_tid, res_name, single, result),
    //     }
    // }

    pub fn align(&mut self, world: &World) {
        match self {
            BoxedSystem::Sync(s) => s.align(world),
            BoxedSystem::Async(s) => s.align(world),
        }
    }

    pub async fn run(&mut self, world: &'static World) {
        match self {
            BoxedSystem::Sync(s) => s.run(world),
            BoxedSystem::Async(s) => s.run(world).await,
        }
    }
}
pub trait IntoSystem<Marker>: Sized {
    /// The type of [`System`] that this instance converts into.
    type System: RunSystem;

    /// Turns this value into its corresponding [`System`].
    fn into_system(self) -> Self::System;
}
pub trait IntoAsyncSystem<Marker>: Sized {
    /// The type of [`System`] that this instance converts into.
    type System: AsyncRunSystem;

    /// Turns this value into its corresponding [`System`].
    fn into_async_system(self) -> Self::System;
}

// All systems implicitly implement IntoSystem.
// impl<Marker, T: System> IntoSystem<Marker> for T {
//     type System = T;
//     fn into_system(this: Self) -> Self {
//         this
//     }
// }
