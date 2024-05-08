/// 在插入时， table上上分配了row，row的e为初始化状态，先写组件及record，然后再写world上entitys的ar_row，最后改table上的e为正确值的Entity。
/// 删除时， 是先改table上的e为删除状态，然后删除world上的entitys的e，最后销毁组件。
/// Alter移动时， 新table上分配了新row，先写移动相同的组件和新增组件及record，再改world上的entitys的ar_row，然后改旧table上row的e为删除状态，接着销毁旧table上的组件。最后改新table上row的e为正确值的Entity。
///
/// Alter所操作的源table， 在执行图中，会被严格保证不会同时有其他system进行操作。
use core::fmt::*;
use std::any::TypeId;
use std::cell::SyncUnsafeCell;
use std::mem::{replace, transmute};

use fixedbitset::FixedBitSet;
use pi_append_vec::AppendVec;
use pi_arr::Iter;
use pi_null::Null;
use pi_phf_map::PhfMap;
use smallvec::SmallVec;

use crate::archetype::ComponentInfo;
use crate::archetype::{ColumnIndex, Row};
use crate::column::Column;
use crate::dirty::{Dirty, DirtyIndex, DirtyType, EntityRow};
use crate::world::{Entity, World};

pub struct Table {
    entities: AppendVec<Entity>, // 记录entity
    columns: Vec<Column>,        // 每个组件
    column_map: PhfMap<TypeId, ColumnIndex>,
    remove_columns: SyncUnsafeCell<Vec<(TypeId, Dirty)>>, // 监听器监听的Removed组件，其他原型通过移除Component转到该原型
    pub(crate) destroys: SyncUnsafeCell<Dirty>, // 该原型的实体被标记销毁的脏列表，查询读取后被放入removes
    removes: AppendVec<Row>,                    // 整理前被移除的实例
}
impl Table {
    pub fn new(infos: Vec<ComponentInfo>, vec: Vec<(TypeId, u32)>) -> Self {
        Self {
            entities: AppendVec::default(),
            columns: infos.into_iter().map(|info| Column::new(info)).collect(),
            column_map: PhfMap::new(vec),
            remove_columns: Default::default(),
            destroys: SyncUnsafeCell::new(Dirty::default()),
            removes: AppendVec::default(),
        }
    }
    /// Returns the number of elements in the archetype.
    #[inline(always)]
    pub fn len(&self) -> Row {
        self.entities.len() as Row
    }
    #[inline(always)]
    pub fn get(&self, row: Row) -> Entity {
        *self.entities.load_alloc(row as usize)
    }
    #[inline(always)]
    pub fn set(&self, row: Row, e: Entity) {
        let a = self.entities.load_alloc(row as usize);
        *a = e;
    }
    #[inline(always)]
    pub fn get_columns(&self) -> &Vec<Column> {
        &self.columns
    }
    #[inline(always)]
    pub fn get_column_index(&self, type_id: &TypeId) -> ColumnIndex {
        if let Some(t) = self.column_map.get(&type_id) {
            if t.is_null() {
                return u32::null();
            }
            let ti = self.get_column_unchecked(*t);
            if &ti.info().type_id == type_id {
                return *t;
            }
        }
        u32::null()
    }
    #[inline(always)]
    pub fn get_column(&self, type_id: &TypeId) -> Option<(&Column, ColumnIndex)> {
        if let Some(t) = self.column_map.get(&type_id) {
            if t.is_null() {
                return None;
            }
            let c = self.get_column_unchecked(*t);
            if &c.info().type_id == type_id {
                return Some((c, *t));
            }
        }
        None
    }
    #[inline(always)]
    pub(crate) unsafe fn get_column_mut(
        &self,
        type_id: &TypeId,
    ) -> Option<(&mut Column, ColumnIndex)> {
        if let Some(t) = self.column_map.get(&type_id) {
            if t.is_null() {
                return None;
            }
            let c = self.get_column_unchecked(*t);
            if &c.info().type_id == type_id {
                return unsafe { transmute(Some((c, *t))) };
            }
        }
        None
    }
    #[inline(always)]
    pub(crate) fn get_column_unchecked(&self, index: ColumnIndex) -> &Column {
        assert!(!index.is_null());
        unsafe { self.columns.get_unchecked(index as usize)}
    }
    /// 添加changed监听器，原型刚创建时调用
    pub fn add_changed_listener(&self, tid: &TypeId, owner: TypeId) {
        if let Some((c, _)) = unsafe { self.get_column_mut(tid) } {
            c.is_record_tick = true;
            c.dirty.insert_listener(owner)
        }
    }
    /// 添加removed监听器，原型刚创建时调用
    pub fn add_removed_listener(&self, tid: &TypeId, owner: TypeId) {
        if !self.get_column_index(tid).is_null() {
            return;
        }
        let vec = unsafe { &mut *self.remove_columns.get() };
        let r = vec.iter_mut().find(|(t, _)| t == tid);
        if let Some((_, d)) = r {
            d.insert_listener(owner);
        }else{
            vec.push((*tid, Dirty::new(owner)));
        }
    }
    /// 添加destroyed监听器，原型刚创建时调用
    pub fn add_destroyed_listener(&self, owner: TypeId) {
        unsafe { &mut *self.destroys.get() }.insert_listener(owner)
    }
    /// 查询在同步到原型时，寻找自己添加的changed监听器，并记录组件位置和监听器位置
    pub(crate) fn find_changed_listener(
        &self,
        tid: &TypeId,
        owner: TypeId,
        vec: &mut SmallVec<[DirtyIndex; 1]>,
    ) {
        if let Some((c, column_index)) = unsafe { self.get_column_mut(tid) } {
            let listener_index = c.dirty.find_listener_index(owner);
            if !listener_index.is_null() {
                vec.push(DirtyIndex {
                    listener_index,
                    dtype: DirtyType::Changed(column_index),
                });
            }
        }
    }
    /// 查询在同步到原型时，寻找自己添加的removed监听器，并记录组件位置和监听器位置
    pub(crate) fn find_removed_listener(
        &self,
        tid: &TypeId,
        owner: TypeId,
        vec: &mut SmallVec<[DirtyIndex; 1]>,
    ) {
        let vec1 = unsafe { &*self.remove_columns.get() };
        let r = vec1.iter().enumerate().find(|r| &r.1 .0 == tid);
        if let Some((column_index, d)) = r {
            let listener_index = d.1.find_listener_index(owner);
            if !listener_index.is_null() {
                vec.push(DirtyIndex {
                    listener_index,
                    dtype: DirtyType::Removed(column_index as u32),
                });
            }
        }
    }
    /// 查询在同步到原型时，寻找自己添加的destroyed监听器，并记录监听器位置
    pub(crate) fn find_destroyed_listener(
        &self,
        owner: TypeId,
        vec: &mut SmallVec<[DirtyIndex; 1]>,
    ) {
        let listener_index = unsafe { &*self.destroys.get() }.find_listener_index(owner);
        if !listener_index.is_null() {
            vec.push(DirtyIndex {
                listener_index,
                dtype: DirtyType::Destroyed,
            });
        }
    }
    /// 寻找指定组件列的脏位置
    pub(crate) fn find_remove_column_dirty(&self, tid: &TypeId) -> u32 {
        let mut it = unsafe { &*self.remove_columns.get() }.iter().enumerate();
        it.find(|r| &r.1 .0 == tid)
            .map_or(u32::null(), |(column_index, _)| column_index as u32)
    }
    /// 寻找指定位置的组件列脏
    pub(crate) fn get_remove_column_dirty(&self, column_index: u32) -> &Dirty {
        unsafe {
            &(&*self.remove_columns.get())
                .get_unchecked(column_index as usize)
                .1
        }
    }
    /// 获得对应的脏列表, 及是否不检查entity是否存在
    pub(crate) fn get_iter<'a>(&'a self, dirty_index: &DirtyIndex) -> (Iter<'a, EntityRow>, bool) {
        let (r, b) = match dirty_index.dtype {
            DirtyType::Destroyed => (unsafe { &*self.destroys.get() }, true),
            DirtyType::Changed(column_index) => (&self.get_column_unchecked(column_index).dirty, false),
            DirtyType::Removed(column_index) => (self.get_remove_column_dirty(column_index), false),
        };
        (r.get_iter(dirty_index.listener_index), b)
    }

    /// 扩容
    pub fn reserve(&mut self, additional: usize) {
        let len = self.entities.len();
        self.entities.reserve(additional);
        for c in self.columns.iter_mut() {
            c.reserve(len, additional);
        }
    }
    #[inline(always)]
    pub fn alloc(&self) -> Row {
        self.entities.alloc_index(1) as Row
    }
    /// 标记销毁，用于destroy
    /// mark removes a key from the archetype, returning the value at the key if the
    /// key was not previously removed.
    pub(crate) fn mark_destroy(&self, row: Row) -> Entity {
        let e = self.entities.load_alloc(row as usize);
        if e.is_null() {
            return *e;
        }
        { unsafe { &*self.destroys.get() } }.record_unchecked(*e, row);
        replace(e, Entity::null())
    }
    /// 标记移出，用于delete 和 alter
    /// mark removes a key from the archetype, returning the value at the key if the
    /// key was not previously removed.
    pub(crate) fn mark_remove(&self, row: Row) -> Entity {
        let e = self.entities.load_alloc(row as usize);
        if e.is_null() {
            return *e;
        }
        self.removes.insert(row);
        replace(e, Entity::null())
    }
    // 处理标记移除的条目，返回true，表示所有监听器都已经处理完毕，然后可以清理destroys
    fn clear_destroy(&mut self) -> bool {
        let dirty = unsafe { &mut *self.destroys.get() };
        let len = match dirty.can_clear() {
            Some(len) => {
                if len == 0 {
                    return true;
                }
                len
            }
            _ => return false,
        };
        for e in dirty.vec.iter() {
            self.removes.insert(e.row);
        }
        self.drop_vec(&dirty.vec);
        dirty.clear(len);
        true
    }
    /// 删除全部组件
    pub(crate) fn drop_vec(&self, vec: &AppendVec<EntityRow>) {
        for t in self.columns.iter() {
            for e in vec.iter() {
                t.drop_row(e.row);
            }
        }
    }
    /// 获得移除数组产生的动作， 返回新entitys的长度
    pub(crate) fn removes_action(
        removes: &AppendVec<Row>,
        remove_len: usize,
        entity_len: usize,
        action: &mut Vec<(Row, Row)>,
        set: &mut FixedBitSet,
    ) -> usize {
        action.clear();
        // 根据4种情况， 获得新长度new_entity_len，并且在action中放置了移动对
        if remove_len >= entity_len {
            // 全部移除
            return 0;
        }
        if remove_len == 1 {
            // 移除一个，用交换尾部的方式
            let remove_row = unsafe { removes.get_unchecked(0) };
            if (*remove_row) as usize + 1 < entity_len {
                action.push((entity_len as u32 - 1, *remove_row));
            }
            return entity_len - 1;
        }
        let r = remove_len as f64;
        if r * r.log2() < (entity_len - remove_len) as f64 {
            // 少量移除， 走removes排序，计算好移动对
            // 需要扫描removes一次，排序一次，再扫描action一次, 消耗为n*log2n+n
            // 先将removes的数据放入action，然后排序
            for row in removes.iter() {
                action.push((*row, *row));
            }
            action.sort();
            // 按从后移动到前的方式，计算移动对
            let mut start = 0;
            let mut end = action.len();
            let mut index = entity_len;
            while start < end {
                index -= 1;
                let remove_row = unsafe { action.get_unchecked(end - 1) };
                if remove_row.0 as usize == index {
                    // 最大的要移动的行就是entitys的最后一个，则跳过
                    end -= 1;
                    continue;
                }
                // 移动到前面
                let r = unsafe { action.get_unchecked_mut(start) };
                r.0 = index as u32;
                start += 1;
            }
            action.truncate(end);
            return index;
        }
        // 大量移除，走fixbitset的位标记方式，再次扫描，计算移动对
        // 需要扫描removes一次，entitys一次, 消耗为entity_len
        set.clear();
        set.grow(entity_len);
        for row in removes.iter() {
            set.set(*row as usize, true);
        }
        let ones = set.ones();
        let mut end = entity_len;
        for row in ones {
            // 找到最后一个未被移除的
            loop {
                end -= 1;
                if row >= end {
                    return end + 1;
                }
                if !set.contains(end) {
                    // 放入移动对
                    action.push((end as u32, row as u32));
                    break;
                }
            }
        }
        end
    }
    /// 只有主调度完毕后，才能调用的整理方法
    /// 尝试清空所有列的脏列表，所有的脏都被成功的处理和清理后，才能进行row调整
    /// 调整Row，将空位的entity换到尾部，将entitys变紧凑，没有空位。
    /// 在整理前，Row都是递增的。
    pub(crate) fn collect(
        &mut self,
        world: &World,
        action: &mut Vec<(Row, Row)>,
        set: &mut FixedBitSet,
    ) -> bool {
        if !self.clear_destroy() {
            // 如果清理destroys不成功，不调整row，返回
            return false;
        }
        let mut r = true;
        // 先整理每个列，如果所有列的脏列表成功清空
        for c in self.columns.iter_mut() {
            r &= c.dirty.collect();
        }
        if !r {
            // 有失败的脏，不调整row，返回
            return false;
        }
        // 整理全部的remove_columns，如果所有移除列的脏列表成功清空
        for (_, d) in self.remove_columns.get_mut().iter_mut() {
            r &= d.collect();
        }
        if !r {
            // 有失败的脏，不调整row，返回
            return false;
        }
        let remove_len = self.removes.len();
        if remove_len == 0 {
            return true;
        }
        let new_entity_len =
            Self::removes_action(&self.removes, remove_len, self.entities.len(), action, set);
        // 清理removes
        self.removes.clear();
        // 以前用到了arr，所以扩容
        if self.removes.vec_capacity() < remove_len {
            unsafe {
                self.removes
                    .vec_reserve(remove_len - self.removes.vec_capacity())
            };
        }
        // 整理全部的列
        for c in self.columns.iter_mut() {
            // 整理合并空位
            c.collect(new_entity_len, &action);
        }
        // 再移动entitys的空位
        for (src, dst) in action.iter() {
            let e = unsafe {
                replace(
                    self.entities.get_unchecked_mut(*src as usize),
                    Entity::null(),
                )
            };
            *unsafe { self.entities.get_unchecked_mut(*dst as usize) } = e;
            // 修改world上entity的地址
            world.replace_row(e, *dst);
        }
        // 设置成正确的长度
        unsafe {
            self.entities.set_len(new_entity_len);
        };
        // 整理合并内存
        self.entities.collect();
        true
    }
}
impl Drop for Table {
    fn drop(&mut self) {
        let len = self.len() as usize;
        if len == 0 {
            return;
        }
        for c in self.columns.iter_mut() {
            if !c.needs_drop() {
                continue;
            }
            // 释放每个列中还存在的row
            for (row, e) in self.entities.iter().enumerate() {
                if !e.is_null() {
                    c.drop_row_unchecked(row as Row);
                }
            }
        }
    }
}

impl Debug for Table {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("Table")
            .field("entitys", &self.entities)
            .field("columns", &self.columns)
            .field("removes", &self.removes)
            .finish()
    }
}
