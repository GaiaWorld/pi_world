//! 脏
//! 为每个Query::Filter的Added和Changed都维护一个单独ShareUsize的AddedIndex和ChangedIndex
//! 并且将AddedIndex和ChangedIndex注册到关联的原型列上
//! AddedIndex和ChangedIndex仅记录本system query读取到的最后位置
//! 每Column用AppendVec脏所有Added和Changed的Row，也不做去重
//! 查询时，每Query::Filter内维护自己的fixedbitset，用来做去重（可去重多次修改相同组件和或修改相同组件）
//! 整理时，如果该Column所关联的所有AddedIndex都和该AppendVec的长度一样，则所有AddedIndex和AppendVec就可以清空，否则继续保留，等下一帧再检查是否清空。 因为必须保证Row不被错误重用，只有清空的情况下，才可以做原型的Removed整理。
//!
use core::fmt::*;
use std::sync::atomic::Ordering;
use std::any::TypeId;

use pi_append_vec::AppendVec;
use pi_arr::Iter;
use pi_null::Null;
use pi_share::ShareUsize;
use smallvec::SmallVec;

use crate::archetype::{Archetype, ColumnIndex, Row};
use crate::filter::ListenType;
use crate::world::Entity;

#[derive(Debug, Default, Clone, Copy)]
pub struct DirtyIndex {
    column_index: ColumnIndex, // 对应列的位置
    vec_index: u32,    // 在Dirty的Vec中的位置
    ltype: ListenType,
}
impl DirtyIndex {
    #[inline]
    pub fn new(ltype: ListenType, column_index: ColumnIndex, vec_index: usize) -> Self {
        DirtyIndex {
            column_index,
            vec_index: vec_index as u32,
            ltype,
        }
    }
    #[inline]
    pub(crate) fn get_iter<'a>(self, archetype: &'a Archetype) -> Iter<'a, EntityDirty> {
        let r = match self.ltype {
            ListenType::Changed => &archetype.table.get_column_unchecked(self.column_index).dirty,
            ListenType::Removed => &archetype.table.get_column_unchecked(self.column_index).dirty,
            ListenType::Destroyed => &archetype.table.get_column_unchecked(self.column_index).dirty,
        };
        let end = r.vec.len();
        // 从上次读取到的位置开始读取
        let len = unsafe { &r.listener_list().get_unchecked(self.vec_index as usize).1 };
        let start = len.load(Ordering::Relaxed);
        len.store(end, Ordering::Relaxed);
        r.vec.slice(start..end)
    }
}
#[derive(Debug, Default)]
pub(crate) struct EntityDirty {
    pub(crate) e: Entity,
    pub(crate) row: Row,
}
impl Null for EntityDirty {
    fn is_null(&self) -> bool {
        self.e.is_null()
    }

    fn null() -> Self {
        EntityDirty {
            e: Entity::null(),
            row: Row::null(),
        }
    }
}

#[derive(Debug, Default)]
pub struct Dirty {
    listeners: Vec<(TypeId, ShareUsize)>,           // 每个监听器的TypeId和当前读取的长度
    vec: AppendVec<EntityDirty>,                    // 记录的脏Row，可以重复
}
unsafe impl Sync for Dirty {}
unsafe impl Send for Dirty {}
impl Dirty {
    /// 插入一个监听者的类型id
    pub(crate) fn insert_listener(&mut self, owner: TypeId) {
        self.listeners.push((owner, ShareUsize::new(0)));
    }
    /// 插入一个监听者的类型id
    pub(crate) fn listener_list(&self) -> &Vec<(TypeId, ShareUsize)> {
        &self.listeners
    }
    pub fn find(
        &self,
        index: ColumnIndex,
        owner: TypeId,
        ltype: ListenType,
        result: &mut SmallVec<[DirtyIndex; 1]>,
    ) {
        for (j, d) in self.listener_list().iter().enumerate() {
            if d.0 == owner {
                result.push(DirtyIndex::new(ltype, index, j))
            }
        }
    }
    #[inline(always)]
    pub(crate) fn listener_len(&self) -> usize {
        self.listener_list().len()
    }
    #[inline(always)]
    pub(crate) fn record_unchecked(&self, e: Entity, row: Row) {
        self.vec.insert(EntityDirty { e, row });
    }
    #[inline(always)]
    pub(crate) fn record(&self, e: Entity, row: Row) {
        if !self.listener_list().is_empty() {
            self.vec.insert(EntityDirty { e, row });
        }
    }
    #[inline(always)]
    pub fn reserve(&mut self, additional: usize) {
        if self.listener_len() > 0 {
            self.vec.reserve(additional);
        }
    }

    // 整理方法， 返回是否已经将脏列表清空，只有所有的监听器都读取了全部的脏列表，才可以清空脏列表
    pub(crate) fn collect(&mut self) -> bool {
        if self.listeners.is_empty() {
            return true;
        }
        let len = self.vec.len();
        if len == 0 {
            return true;
        }
        for (_, read_len) in self.listeners.iter_mut() {
            if *read_len.get_mut() < len {
                return false;
            }
        }
        self.vec.clear();
        // 以前用到了arr，所以扩容
        if self.vec.vec_capacity() < len {
            unsafe { self.vec.vec_reserve(len - self.vec.vec_capacity()) };
        }
        for (_, read_len) in self.listeners.iter_mut() {
            *read_len.get_mut() = 0;
        }
        true
    }
}
