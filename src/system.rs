use std::{any::TypeId, borrow::Cow, collections::HashMap, future::Future, mem::take, ops::Range, pin::Pin};

use crate::{
    archetype::{Archetype, ArchetypeDependResult, Flags},
    world::World,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeInfo {
    pub type_id: TypeId,
    pub name: Cow<'static, str>,
}
impl TypeInfo {
    pub fn of<T: 'static>() -> Self {
        TypeInfo {
            type_id: TypeId::of::<T>(),
            name: Cow::Borrowed(std::any::type_name::<T>()),
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

#[derive(Debug)]
pub enum Relation {
    El(TypeId),
    Read(TypeId),
    Write(TypeId),
    And(usize),
    Or(usize),
    Opt,
    Not,
}
#[derive(Debug)]
pub struct Related {
    pub(crate) vec: Vec<Relation>,
    pub(crate) map: HashMap<TypeId, Cow<'static, str>>,
}
impl Related {
    pub fn read(&mut self, type_info: TypeInfo) -> Result<(), Cow<'static, str>> {
        if !self.contains(&type_info) {
        }
        self.vec.push(Relation::Read(type_info.type_id));
        return Ok(())
    }
    pub fn write(&mut self, type_info: TypeInfo) -> Result<(), Cow<'static, str>> {
        if !self.contains(&type_info) {
        }
        self.vec.push(Relation::Write(type_info.type_id));
        return Ok(())
    }
    pub fn read_opt(&mut self, type_info: TypeInfo) -> Result<(), Cow<'static, str>> {
        if !self.contains(&type_info) {
        }
        self.vec.push(Relation::Or(1));
        self.vec.push(Relation::Read(type_info.type_id));
        return Ok(())
    }
    pub fn write_opt(&mut self, type_info: TypeInfo) -> Result<(), Cow<'static, str>> {
        if !self.contains(&type_info) {
        }
        self.vec.push(Relation::Or(1));
        self.vec.push(Relation::Write(type_info.type_id));
        return Ok(())
    }
    pub fn not(&mut self, type_info: TypeInfo) -> Result<(), Cow<'static, str>> {
        if !self.contains(&type_info) {
        }
        self.vec.push(Relation::Not);
        self.vec.push(Relation::Read(type_info.type_id));
        return Ok(())
    }
    pub fn contains(&mut self, type_info: &TypeInfo) -> bool {
        match self.map.entry(type_info.type_id) {
            std::collections::hash_map::Entry::Occupied(_) => true,
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(type_info.name.clone());
                false
            },
        }
    }

}
/// The metadata of a [`System`].
#[derive(Debug)]
pub struct SystemMeta {
    pub(crate) type_info: TypeInfo,
    pub(crate) components: ReadWrite, // 该系统所有组件级读写依赖
    pub(crate) cur_param: ReadWrite,  // 当前参数的读写依赖
    pub(crate) param_set: ReadWrite,  // 参数集的读写依赖
    pub(crate) res_reads: HashMap<TypeId, Cow<'static, str>>, // 读Res
    pub(crate) res_writes: HashMap<TypeId, Cow<'static, str>>, // 写ResMut
}

impl SystemMeta {
    pub fn new(type_info: TypeInfo) -> Self {
        Self {
            type_info,
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
    pub fn name(&self) -> &str {
        &self.type_info.name
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
    // 检查withouts，without在reads或writes中，表示查询完全不重合
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
            panic!("res_read conflict, name:{}", type_info.name);
        }
        self.res_reads.insert(type_info.type_id, type_info.name.clone());
    }
    pub fn res_write(&mut self, type_info: &TypeInfo) {
        if self.res_reads.contains_key(&type_info.type_id) {
            panic!("res_write read conflict, name:{}", type_info.name);
        }
        if self.res_writes.contains_key(&type_info.type_id) {
            panic!("res_write write conflict, name:{}", type_info.name);
        }
        self.res_writes.insert(type_info.type_id, type_info.name.clone());
    }
}

pub trait System: Send + Sync + 'static {
    /// Returns the system's name.
    fn name(&self) -> &Cow<'static, str>;
    /// Returns the [`TypeId`] of the underlying system type.
    fn type_id(&self) -> TypeId;
    /// Initialize the system.
    fn initialize(&mut self, world: &mut World);
    /// system depend the archetype.
    fn archetype_depend(
        &self,
        world: &World,
        archetype: &Archetype,
        result: &mut ArchetypeDependResult,
    );
    /// system depend the res.
    fn res_depend(
        &self,
        world: &World,
        res_tid: &TypeId,
        res_name: &Cow<'static, str>,
        single: bool,
        result: &mut Flags,
    );

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

    pub fn archetype_depend(
        &self,
        world: &World,
        archetype: &Archetype,
        result: &mut ArchetypeDependResult,
    ) {
        match self {
            BoxedSystem::Sync(s) => s.archetype_depend(world, archetype, result),
            BoxedSystem::Async(s) => s.archetype_depend(world, archetype, result),
        }
    }

    pub fn res_depend(
        &self,
        world: &World,
        res_tid: &TypeId,
        res_name: &Cow<'static, str>,
        single: bool,
        result: &mut Flags,
    ) {
        match self {
            BoxedSystem::Sync(s) => s.res_depend(world, res_tid, res_name, single, result),
            BoxedSystem::Async(s) => s.res_depend(world, res_tid, res_name, single, result),
        }
    }

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
