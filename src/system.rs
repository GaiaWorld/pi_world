use std::{any::TypeId, borrow::Cow, collections::HashMap};


use crate::{archetype::{Archetype, ArchetypeDependResult}, world::World};

/// The metadata of a [`System`].
#[derive(Clone, Debug, Default)]
pub struct ReadWrite {
    pub(crate) reads: HashMap<TypeId, Cow<'static, str>>,  // 该系统所有读的组件
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
    pub fn add_read(&mut self, id: TypeId, name: Cow<'static, str>) {
        self.reads.insert(id, name);
    }
    pub fn contains(&self, sub: &ReadWrite) -> Result<(), Cow<'static, str>> {
        Self::check(&self.reads, &sub.reads)?;
        Self::check(&self.writes, &sub.writes)
    }
    pub fn check(map: &HashMap<TypeId, Cow<'static, str>>, sub: &HashMap<TypeId, Cow<'static, str>>) -> Result<(), Cow<'static, str>> {
        for (id, name) in sub.iter() {
            if !map.contains_key(id) {
                return Err(name.clone());
            }
        }
        Ok(())
    }
}
/// The metadata of a [`System`].
#[derive(Debug)]
pub struct SystemMeta {
    pub(crate) name: Cow<'static, str>,
    pub(crate) vec: Vec<ReadWrite>, // 该系统所有组件级读写依赖
}

impl SystemMeta {
    pub(crate) fn new<T>() -> Self {
        Self {
            name: std::any::type_name::<T>().into(),
            vec: Default::default(),
        }
    }

    /// Returns the system's name
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn add_rw(&mut self, rw: ReadWrite) -> usize {
        let len = self.vec.len();
        // 检查前面查询的rw是否有组件读写冲突
        for (i, old) in self.vec.iter().enumerate() {
            // 先检查withouts
            if Self::check_without(&old.withouts, &rw) || Self::check_without(&rw.withouts, old) {
                continue;
            }
            if Self::check_w(&old.reads, &rw)
                || Self::check_w(&old.writes, &rw)
                || Self::check_w(&rw.reads, old)
            {
                panic!("rw_conflict, i:{}, j:{}", i, len);
            }
        }
        self.vec.push(rw);
        len
    }
    // 检查withouts，without在reads或writes中，表示查询完全不重合
    pub fn check_without(withouts: &HashMap<TypeId, Cow<'static, str>>, rw: &ReadWrite) -> bool {
        for w in withouts.keys() {
            if rw.reads.contains_key(w) || rw.writes.contains_key(w)  || rw.withs.contains_key(w) {
                return true;
            }
        }
        false
    }
    // 检查数据集是否和写冲突
    pub fn check_w(map: &HashMap<TypeId, Cow<'static, str>>, rw: &ReadWrite) -> bool {
        for t in map.keys() {
            if rw.writes.contains_key(t) {
                return true;
            }
        }
        false
    }
}


pub trait System: Send + Sync {
    /// Returns the system's name.
    fn name(&self) -> &Cow<'static, str>;
    /// Returns the [`TypeId`] of the underlying system type.
    fn type_id(&self) -> TypeId;
    /// Initialize the system.
    fn initialize(&mut self, world: &World);
    /// system depend the archetype.
    fn depend(&self, world: &World, archetype: &Archetype, depend: &mut ArchetypeDependResult);

    /// system align archetype
    fn align(&mut self, world: &World);
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
/// A convenience type alias for a boxed [`System`] trait object.
pub type BoxedSystem = Box<dyn System>;

pub trait IntoSystem<Marker>: Sized {
    /// The type of [`System`] that this instance converts into.
    type System: System;

    /// Turns this value into its corresponding [`System`].
    fn into_system(this: Self) -> Self::System;
}

// All systems implicitly implement IntoSystem.
// impl<Marker, T: System> IntoSystem<Marker> for T {
//     type System = T;
//     fn into_system(this: Self) -> Self {
//         this
//     }
// }
