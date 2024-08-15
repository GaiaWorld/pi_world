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

#[derive(Debug, Clone)]
pub enum Relation<T: Eq> {
    With(T),
    Without(T),
    Read(T),
    Write(T),
    ShareWrite(T),
    OptRead(T),
    OptWrite(T),
    Count(usize),
    ReadAll,
    WriteAll,
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
            Relation::ShareWrite(_) => Relation::ShareWrite(arg),
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
            Relation::ShareWrite(id) if id == arg => true,
            _ => false,
        }
    }
    pub fn read(&self, arg: &mut T) -> bool {
        match self {
            Relation::Write(id) if id == arg => false,
            Relation::ShareWrite(id) if id == arg => false,
            Relation::OptWrite(id) if id == arg => false,
            Relation::WriteAll => false,
            _ => true,
        }
    }
    pub fn write(&self, arg: &mut T) -> bool {
        match self {
            Relation::Read(id) if id == arg => false,
            Relation::OptRead(id) if id == arg => false,
            Relation::Write(id) if id == arg => false,
            Relation::ShareWrite(id) if id == arg => false,
            Relation::OptWrite(id) if id == arg => false,
            Relation::ReadAll => false,
            Relation::WriteAll => false,
            _ => true,
        }
    }
    pub fn share_write(&self, arg: &mut T) -> bool {
        match self {
            Relation::Read(id) if id == arg => false,
            Relation::OptRead(id) if id == arg => false,
            Relation::Write(id) if id == arg => false,
            Relation::OptWrite(id) if id == arg => false,
            Relation::ReadAll => false,
            Relation::WriteAll => false,
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
pub struct Related<T: Eq> {
    pub(crate) vec: Vec<Relation<T>>,
}
impl<T: Eq + Copy + Debug> Related<T> {
    pub fn new() -> Self {
        Self { vec: Vec::new() }
    }
    // 检查新旧读写在reads或writes是否完全不重合
    pub fn check_conflict(&self, other: &Related<T>) {
        // 先检查withouts
        if self.check_without(other) || other.check_without(self) {
            return;
        }
        assert_eq!(
            self.check_rw(other),
            None,
            "conflict1!, {:?} {:?}",
            self,
            other
        );
        assert_eq!(
            other.check_rw(self),
            None,
            "conflict2!, {:?} {:?}",
            self,
            other
        );
    }
    // 检查other的每一个without，和self的with read或writes判断，返回true表示查询完全不重合
    pub fn check_without(&self, other: &Related<T>) -> bool {
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
    pub fn check_self(&self) -> Option<T> {
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
    pub fn check_rw(&self, other: &Related<T>) -> Option<T> {
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
    pub fn check_read(&self, mut t: T, mut start: usize) -> bool {
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
    pub fn check_write(&self, mut t: T, mut start: usize) -> bool {
        traversal(
            &self.vec,
            &Relation::write,
            &mut t,
            &mut start,
            RelateNode::new(true),
        )
    }
}
// 判断该原型是否相关
pub fn relate(r: &Related<ComponentIndex>, archetype: &Archetype, mut start: usize) -> bool {
    let mut filter = ArchetypeFilter(archetype);
    traversal(
        &r.vec,
        &archetype_relate,
        &mut filter,
        &mut start,
        RelateNode::new(true),
    )
}
// 获取该原型的每组件的读写依赖
pub fn depend(r: &Related<ComponentIndex>, archetype: &Archetype) {
    todo!()
}
/// The metadata of a [`System`].
pub struct SystemMeta {
    pub(crate) type_info: TypeInfo,
    pub(crate) vec: Vec<Share<Related<ComponentIndex>>>, // SystemParam参数的组件关系列表
    pub(crate) cur_related: Related<ComponentIndex>,     // 当前SystemParam参数的关系
    pub(crate) param_set_locations: Vec<Range<usize>>,   // 参数集在vec的位置
    pub(crate) res_related: Related<TypeId>,             // Res资源的关系

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
            res_related: Related::new(),

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
    pub fn related_ok(&mut self) -> Share<Related<ComponentIndex>> {
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
    /// 加入一个资源
    pub fn add_single_res<'w>(
        &mut self,
        world: &'w mut World,
        info: TypeInfo,
        r: Relation<TypeId>,
    ) -> usize {
        self.res_related.vec.push(r);
        world.or_register_single_res(info)
    }
    /// 加入一个资源
    pub fn add_res<'w>(&mut self, r: Relation<TypeId>) {
        self.res_related.vec.push(r);
    }

    // 检查冲突
    pub fn check_conflict(&self) {
        // 先检查资源是否冲突
        assert_eq!(
            self.res_related.check_self(),
            None,
            "self res conflict, related:{:?}",
            &self.res_related
        );
        for r in self.vec.iter() {
            // 先检查自身
            assert_eq!(r.check_self(), None, "self conflict, related:{:?}", r);
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
    type Out;
    /// Returns the system's name.
    fn name(&self) -> &Cow<'static, str>;
    /// Returns the [`TypeId`] of the underlying system type.
    fn id(&self) -> TypeId;
    /// Initialize the system.
    fn initialize(&mut self, world: &mut World);

    /// system align the world archetypes
    fn align(&mut self, world: &World);
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
    fn run(&mut self, world: &World) -> Self::Out;
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
        -> Pin<Box<dyn Future<Output = Self::Out> + Send + 'static>>;
}

/// A convenience type alias for a boxed [`System`] trait object.
pub enum BoxedSystem<Out> {
    Sync(Box<dyn RunSystem<Out = Out>>),
    Async(Box<dyn AsyncRunSystem<Out = Out>>),
}

impl<Out: 'static> BoxedSystem<Out> {
    pub fn name(&self) -> &Cow<'static, str> {
        match self {
            BoxedSystem::Sync(s) => s.name(),
            BoxedSystem::Async(s) => s.name(),
        }
    }

    pub fn id(&self) -> TypeId {
        match self {
            BoxedSystem::Sync(s) => s.id(),
            BoxedSystem::Async(s) => s.id(),
        }
    }

    pub fn initialize(&mut self, world: &mut World) {
        match self {
            BoxedSystem::Sync(s) => s.initialize(world),
            BoxedSystem::Async(s) => s.initialize(world),
        }
    }

    pub fn align(&mut self, world: &World) {
        match self {
            BoxedSystem::Sync(s) => s.align(world),
            BoxedSystem::Async(s) => s.align(world),
        }
    }

    pub async fn run(&mut self, world: &'static World) -> Out {
        match self {
            BoxedSystem::Sync(s) => s.run(world),
            BoxedSystem::Async(s) => s.run(world).await,
        }
    }
}
pub trait IntoSystem<Marker, Out>: Sized {
    /// The type of [`System`] that this instance converts into.
    type System: RunSystem<Out = Out>;

    /// Turns this value into its corresponding [`System`].
    fn into_system(self) -> Self::System;
}
pub trait IntoAsyncSystem<Marker, Out>: Sized {
    /// The type of [`System`] that this instance converts into.
    type System: AsyncRunSystem<Out = Out>;

    /// Turns this value into its corresponding [`System`].
    fn into_async_system(self) -> Self::System;
}
