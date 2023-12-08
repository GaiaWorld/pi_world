use std::{any::TypeId, borrow::Cow, collections::HashSet};

use pi_null::Null;
use smallvec::SmallVec;

use crate::{archetype::Archetype, world::World};

/// The metadata of a [`System`].
#[derive(Clone, Debug, Default)]
pub struct ReadWrite {
    pub(crate) reads: HashSet<TypeId>,  // 该系统所有读的组件
    pub(crate) writes: HashSet<TypeId>, // 该系统所有写的组件。用来和读进行判断，不允许一个组件又读又写
    pub(crate) withouts: HashSet<TypeId>,
    pub(crate) listeners: SmallVec<[(TypeId, bool); 1]>, // 记录有多少监听(true changed, false added)
}

/// The metadata of a [`System`].
#[derive(Clone, Debug)]
pub struct SystemMeta {
    pub(crate) name: Cow<'static, str>,
    pub(crate) vec: Vec<ReadWrite>, // 该系统所有组件级读写依赖
    pub(crate) read_archetype_map: HashSet<u128>, // 该系统所有读的原型
    pub(crate) write_archetype_map: HashSet<u128>, // 该系统所有写的原型
    pub(crate) reads_len: u32, // 读原型的长度， 如果比read_archetype_map.len()小，表示有新的读原型
    pub(crate) writes_len: u32, // 读原型的长度， 如果比write_archetype_map.len()小，表示有新的写原型
}

impl SystemMeta {
    pub(crate) fn new<T>() -> Self {
        Self {
            name: std::any::type_name::<T>().into(),
            vec: Default::default(),
            read_archetype_map: Default::default(),
            write_archetype_map: Default::default(),
            reads_len: 0,
            writes_len: 0,
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
            if Self::check_with(&old.withouts, &rw) || Self::check_with(&rw.withouts, old) {
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
    pub fn check_with(withouts: &HashSet<TypeId>, rw: &ReadWrite) -> bool {
        for w in withouts.iter() {
            if rw.reads.contains(w) || rw.writes.contains(w) {
                return true;
            }
        }
        false
    }
    // 检查数据集是否和写冲突
    pub fn check_w(set: &HashSet<TypeId>, rw: &ReadWrite) -> bool {
        for t in set.iter() {
            if rw.writes.contains(t) {
                return true;
            }
        }
        false
    }
    // 获得指定的指定的读写
    pub fn get_rw(&self, rw_index: usize) -> &ReadWrite {
        &self.vec[rw_index]
    }
    // 根据指定的读写，判断该原型是读还是写，并记录
    pub fn rw_archetype(&mut self, rw_index: usize, ar: &Archetype) {
        let rw = &self.vec[rw_index];
        for id in rw.writes.iter() {
            // Option<&mut C>，如果原型上没有该组件，则不会写
            if !ar.get_mem_offset_ti_index(id).0.is_null() {
                self.write_archetype_map.insert(*ar.get_id());
                return;
            }
        }
        self.read_archetype_map.insert(*ar.get_id());
    }
    pub(crate) fn collect(&mut self) {
        // todo!() 需要调整计算图
        if self.reads_len == self.read_archetype_map.len() as u32 {
            self.reads_len = self.read_archetype_map.len() as u32;
        }
        if self.writes_len == self.write_archetype_map.len() as u32 {
            self.writes_len = self.write_archetype_map.len() as u32;
        }
    }
}

pub trait System: Send + Sync {
    /// Returns the system's name.
    fn name(&self) -> Cow<'static, str>;
    /// Returns the [`TypeId`] of the underlying system type.
    fn type_id(&self) -> TypeId;

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

    /// Initialize the system.
    fn initialize(&mut self, world: &World);
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
