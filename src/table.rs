/// 在插入时， table上上分配了row，row的e为初始化状态，先写组件及record，然后再写world上entitys的ar_row，最后改table上的e为正确值的Entity。
/// 删除时， 是先改table上的e为删除状态，然后删除world上的entitys的e，最后销毁组件。
/// Alter移动时， 新table上分配了新row，先写移动相同的组件和新增组件及record，再改world上的entitys的ar_row，然后改旧table上row的e为删除状态，接着销毁旧table上的组件。最后改新table上row的e为正确值的Entity。
///
/// Alter所操作的源table， 在执行图中，会被严格保证不会同时有其他system进行操作。
use core::fmt::*;
use std::any::TypeId;
use std::cell::SyncUnsafeCell;
use std::mem::{replace, transmute};
use std::ops::Range;

use fixedbitset::FixedBitSet;
use pi_append_vec::AppendVec;
use pi_async_rt::lock::spin_lock::SpinLock;
use pi_null::Null;
use smallvec::SmallVec;

use crate::archetype::{ComponentInfo, COMPONENT_TICK};
use crate::archetype::{ColumnIndex, Row};
use crate::column::Column;
use crate::dirty::{Dirty, DirtyIndex, DirtyIter, DirtyType, EntityRow};
use crate::safe_vec::SafeVec;
use crate::world::{ComponentIndex, Entity, World, Tick};


pub struct Table {
    entities: AppendVec<Entity>, // 记录entity
    sorted_columns: Vec<Column>,        // 每个组件
    column_map: Vec<ColumnIndex>, // 用全局的组件索引作为该数组的索引，方便快速查询
    remove_columns: SafeVec<RemovedColumn>, // 监听器监听的Removed组件，其他原型通过移除Component转到该原型
    lock: SpinLock<()>, // 用于保护多线程下两个alter同时添加相同组件的情况
    pub(crate) destroys: SyncUnsafeCell<Dirty>, // 该原型的实体被标记销毁的脏列表，查询读取后被放入removes
    pub(crate) removes: AppendVec<Row>,                    // 整理前被移除的实例
}
impl Table {
    pub fn new(sorted_components: Vec<ComponentInfo>) -> Self {
        let len = sorted_components.len();
        let max = if len > 0 {
            unsafe { sorted_components.get_unchecked(len - 1).world_index.index() + 1 }
        }else{
            0
        };
        let mut column_map = Vec::with_capacity(max);
        column_map.resize(max, ColumnIndex::null());
        let mut sorted_columns = Vec::with_capacity(len);
        for (i, c) in sorted_components.into_iter().enumerate() {
            *unsafe { column_map.get_unchecked_mut(c.world_index.index()) } = i.into();
            sorted_columns.push(Column::new(c));
        }
        Self {
            entities: AppendVec::default(),
            sorted_columns,
            column_map: column_map,
            remove_columns: Default::default(),
            lock: SpinLock::new(()),
            destroys: SyncUnsafeCell::new(Dirty::default()),
            removes: AppendVec::default(),
        }
    }
    /// Returns the number of elements in the archetype.
    #[inline(always)]
    pub fn len(&self) -> Row {
        Row(self.entities.len() as u32)
    }
    #[inline(always)]
    pub fn get(&self, row: Row) -> Entity {
        *self.entities.load_alloc(row.index())
    }
    #[inline(always)]
    pub fn set(&self, row: Row, e: Entity) {
        let a = self.entities.load_alloc(row.index());
        // println!("set1：{:p} {:p} {:?}", &self.entities, a, (&a, row, e, self.entities.vec_capacity(), self.entities.len()));
        *a = e;
    }
    #[inline(always)]
    pub fn get_columns(&self) -> &Vec<Column> {
        &self.sorted_columns
    }
    pub fn get_column_index_by_tid(&self, world: &World, tid: &TypeId) -> ColumnIndex {
        self.get_column_index(world.get_component_index(tid))
    }
    pub fn get_column_by_tid(&self, world: &World, tid: &TypeId) -> Option<(&Column, ColumnIndex)> {
        self.get_column(world.get_component_index(tid))
    }
    pub fn get_column_index(&self, index: ComponentIndex) -> ColumnIndex {
        self.column_map.get(index.index()).map_or(ColumnIndex::null(), |r| *r)
    }
    pub fn get_column(&self, index: ComponentIndex) -> Option<(&Column, ColumnIndex)> {
        // println!("get_column：{:?}", index);
        if let Some(t) = self.column_map.get(index.index()) {
            // println!("get_column1：{:?}", t);
            if t.is_null() {
                return None;
            }
            let c = self.get_column_unchecked(*t);
            return Some((c, *t));
        }
        None
    }
    pub(crate) unsafe fn get_column_mut(
        &self,
        index: ComponentIndex,
    ) -> Option<(&mut Column, ColumnIndex)> {
        if let Some(t) = self.column_map.get(index.index()) {
            if t.is_null() {
                return None;
            }
            let c = self.get_column_unchecked(*t);
            return unsafe { transmute(Some((c, *t))) };
        }
        None
    }
    pub(crate) fn get_column_unchecked(&self, index: ColumnIndex) -> &Column {
        unsafe { self.sorted_columns.get_unchecked(index.index())}
    }
    /// 添加changed监听器，原型刚创建时调用
    pub fn add_changed_listener(&self, index: ComponentIndex, owner: Tick) {
        // println!("add_changed_listener!! self: {:p}, index: {:?}", self, index);
        if let Some((c, _)) = unsafe { self.get_column_mut(index) } {
            c.dirty.insert_listener(owner)
        }
    }
    /// 添加removed监听器，原型刚创建时调用
    pub fn add_removed_listener(&self, index: ComponentIndex, owner: Tick) {
        // 获取索引
        let column_index = self.add_remove_column_index(index);
        let r = unsafe { self.remove_columns.load_unchecked(column_index.index()) };
        // 添加新的监听
        r.dirty.insert_listener(owner);
    }
    /// 添加destroyed监听器，原型刚创建时调用
    pub fn add_destroyed_listener(&self, owner: Tick) {
        unsafe { &mut *self.destroys.get() }.insert_listener(owner)
    }
    /// 查询在同步到原型时，寻找自己添加的changed监听器，并记录组件位置和监听器位置
    pub(crate) fn find_changed_listener(
        &self,
        index: ComponentIndex,
        owner: Tick,
        vec: &mut SmallVec<[DirtyIndex; 1]>,
    ) {
        if let Some((c, column_index)) = unsafe { self.get_column_mut(index) } {
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
        index: ComponentIndex,
        owner: Tick,
        vec: &mut SmallVec<[DirtyIndex; 1]>,
    ) {
        let column_index = self.add_remove_column_index(index);
        let r = self.get_remove_column(column_index);
        let listener_index = r.dirty.find_listener_index(owner);
        if !listener_index.is_null() {
            vec.push(DirtyIndex {
                listener_index,
                dtype: DirtyType::Removed(column_index.into()),
            });
        }
    }
    /// 查询在同步到原型时，寻找自己添加的destroyed监听器，并记录监听器位置
    pub(crate) fn find_destroyed_listener(
        &self,
        owner: Tick,
        vec: &mut SmallVec<[DirtyIndex; 1]>,
    ) {
        let list = unsafe { &*self.destroys.get() };
        let listener_index = list.find_listener_index(owner);
        if !listener_index.is_null() {
            vec.push(DirtyIndex {
                listener_index,
                dtype: DirtyType::Destroyed,
            });
        }
    }
    /// 寻找所有被移除的组件列
    pub(crate) fn get_remove_columns(&self) -> &SafeVec<RemovedColumn> {
        &self.remove_columns
    }
    /// 寻找指定组件列的脏位置
    pub(crate) fn find_remove_column_index(&self, range: Range<usize>, index: ComponentIndex) -> ColumnIndex {
        let start = range.start;
        for (i, t) in self.remove_columns.slice(range).enumerate() {
            if t.index == index {
                return (i + start).into()
            }
        }
        ColumnIndex::null()
    }
    /// 添加被移除组件，返回其位置
    pub(crate) fn add_remove_column_index(&self, index: ComponentIndex) -> ColumnIndex {
        let len = self.remove_columns.len();
        let mut column_index = self.find_remove_column_index(0..len, index);
        if column_index.is_null() {
            // 没有找到，则先加锁，再次寻找，如果还没找到，则创建新的
            let _ = self.lock.lock();
            let new_len = self.remove_columns.len();
            if len < new_len {
                column_index = self.find_remove_column_index(0..new_len, index);
                if column_index.is_null() {
                    column_index = self.remove_columns.insert(RemovedColumn::new(index)).into();
                }
            } else {
                column_index = self.remove_columns.insert(RemovedColumn::new(index)).into();
            }
        }
        column_index
    }
    /// 寻找指定位置的组件列脏
    pub(crate) fn get_remove_column(&self, column_index: ColumnIndex) -> &RemovedColumn {
        unsafe { self.remove_columns.get_unchecked(column_index.index()) }
    }
    /// 获得对应的脏列表, 及是否不检查entity是否存在
    pub(crate) fn get_dirty_iter<'a>(&'a self, dirty_index: &DirtyIndex, tick: Tick) -> DirtyIter<'a> {
         match dirty_index.dtype {
            DirtyType::Destroyed => {
                let r = unsafe { &*self.destroys.get() };
                DirtyIter::new(r.get_iter(dirty_index.listener_index, tick), None)},
            DirtyType::Changed(column_index) => {
                let r = self.get_column_unchecked(column_index);
                DirtyIter::new(r.dirty.get_iter(dirty_index.listener_index, tick), Some(&r.ticks))
            },
            DirtyType::Removed(column_index) => {
                let r = self.get_remove_column(column_index);
                DirtyIter::new(r.dirty.get_iter(dirty_index.listener_index, tick), Some(&r.ticks))
            },
        }
    }

    /// 扩容
    pub fn reserve(&mut self, additional: usize) {
        let len = self.entities.len();
        self.entities.reserve(additional);
        for c in self.sorted_columns.iter_mut() {
            c.reserve(len, additional);
        }
    }
    #[inline(always)]
    pub fn alloc(&self) -> Row {
        Row(self.entities.alloc_index(1) as u32)
    }
    /// 标记销毁，用于destroy
    /// mark removes a key from the archetype, returning the value at the key if the
    /// key was not previously removed.
    pub(crate) fn mark_destroy(&self, row: Row) -> Entity {
        let e = self.entities.load_alloc(row.index());
        if e.is_null() {
            return *e;
        }
        { unsafe { &*self.destroys.get() } }.record(*e, row, Tick::max());
        replace(e, Entity::null())
    }
    /// 标记移出，用于delete 和 alter
    /// mark removes a key from the archetype, returning the value at the key if the
    /// key was not previously removed.
    pub(crate) fn mark_remove(&self, row: Row) -> Entity {
        let e = self.entities.load_alloc(row.index());
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
        for t in self.sorted_columns.iter() {
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
            if remove_row.index() + 1 < entity_len {
                action.push((Row(entity_len as u32 - 1), *remove_row));
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
            action.sort_unstable();
            // 按从后移动到前的方式，计算移动对
            let mut start = 0;
            let mut end = action.len();
            let mut index = entity_len;
            while start < end {
                index -= 1;
                let remove_row = unsafe { action.get_unchecked(end - 1) };
                if remove_row.0.index() == index {
                    // 最大的要移动的行就是entitys的最后一个，则跳过
                    end -= 1;
                    continue;
                }
                // 移动到前面
                let r = unsafe { action.get_unchecked_mut(start) };
                r.0 = Row(index as u32);
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
            set.set(row.index(), true);
        }
        let ones = set.ones();
        let mut end = entity_len;
        for row in ones {
            // 找到最后一个未被移除的
            loop {
                if row >= end {
                    return end;
                }
                end -= 1;
                if !set.contains(end) {
                    // 放入移动对
                    action.push((Row(end as u32), Row(row as u32)));
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
        for c in self.sorted_columns.iter_mut() {
            r &= c.dirty.collect();
        }
        if !r {
            // 有失败的脏，不调整row，返回
            return false;
        }
        // 整理全部的remove_columns，如果所有移除列的脏列表成功清空
        for d in self.remove_columns.iter() {
            r &= d.dirty.collect();
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
        for c in self.sorted_columns.iter_mut() {
            // 整理合并空位
            c.collect(new_entity_len, &action);
        }
        // 整理全部的列ticks
        for c in self.sorted_columns.iter_mut() {
            if c.info().is_tick() {
                collect_ticks(&mut c.ticks, new_entity_len, &action);
            }
        }
        // 整理全部的RemovedColumn列ticks
        for c in self.remove_columns.iter() {
            collect_ticks(&mut c.ticks, new_entity_len, &action);
        }
        // 再移动entitys的空位
        for (src, dst) in action.iter() {
            let e = unsafe {
                replace(
                    self.entities.get_unchecked_mut(src.index()),
                    Entity::null(),
                )
            };
            *unsafe { self.entities.get_unchecked_mut(dst.index()) } = e;
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
        let len = self.len().index();
        if len == 0 {
            return;
        }
        for c in self.sorted_columns.iter_mut() {
            if !c.needs_drop() {
                continue;
            }
            // 释放每个列中还存在的row
            for (row, e) in self.entities.iter().enumerate() {
                if !e.is_null() {
                    c.drop_row_unchecked(Row(row as u32));
                }
            }
        }
    }
}

impl Debug for Table {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("Table")
            .field("entitys", &self.entities)
            .field("sorted_columns", &self.sorted_columns)
            .field("remove_columns", &self.remove_columns)
            .field("destroys", unsafe { &*self.destroys.get() })
            .field("removes", &self.removes)
            .finish()
    }
}

#[derive(Debug)]
pub struct RemovedColumn {
    pub(crate) ticks: AppendVec<Tick>,
    pub(crate) dirty: Dirty,
    pub(crate) index: ComponentIndex,
}

impl RemovedColumn {
    pub fn new(index: ComponentIndex) -> Self {
        Self {
            ticks: AppendVec::default(),
            dirty: Dirty::default(),
            index,
        }
    }
    pub fn clear(&mut self) {
        self.ticks.clear();
    }
}

/// 整理合并空位
pub(crate) fn collect_ticks(ticks: &mut AppendVec<Tick>, entity_len: usize, action: &Vec<(Row, Row)>) {
    for (src, dst) in action.iter() {
        if let Some(tick) = ticks.get_i(src.index()) {
            *ticks.load_alloc(dst.index()) = *tick;
        }
    }
    if entity_len <= ticks.vec_capacity() {
        return;
    }
    ticks.collect_raw(entity_len, 0);
}
