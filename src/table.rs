/// 在插入时， table上上分配了row，row的e为初始化状态，先写组件及record，然后再写world上entitys的ar_row，最后改table上的e为正确值的Entity。
/// 删除时， 是先改table上的e为删除状态，然后删除world上的entitys的e，最后销毁组件。
/// Alter移动时， 新table上分配了新row，先写移动相同的组件和新增组件及record，再改world上的entitys的ar_row，然后改旧table上row的e为删除状态，接着销毁旧table上的组件。最后改新table上row的e为正确值的Entity。
///
/// Alter所操作的源table， 在执行图中，会被严格保证不会同时有其他system进行操作。
use core::fmt::*;
use std::mem::replace;

use fixedbitset::FixedBitSet;
use pi_append_vec::AppendVec;
use pi_null::Null;

use crate::archetype::ComponentInfo;
use crate::archetype::{ColumnIndex, Row};
use crate::column::Column;
use crate::world::{Entity, World};

pub struct Table {
    pub(crate) entitys: AppendVec<Entity>, // 记录entity
    pub(crate) columns: Vec<Column>,       // 每个组件
    removes: AppendVec<Row>,               // 整理前被移除的实例
}
impl Table {
    pub fn new(infos: Vec<ComponentInfo>) -> Self {
        Self {
            entitys: AppendVec::default(),
            columns: infos.into_iter().map(|info| Column::new(info)).collect(),
            removes: AppendVec::default(),
        }
    }
    /// 长度
    #[inline(always)]
    pub fn len(&self) -> Row {
        self.entitys.len() as Row
    }
    #[inline(always)]
    pub fn get(&self, row: Row) -> Entity {
        *self.entitys.load_alloc(row as usize, 1)
    }
    #[inline(always)]
    pub fn set(&self, row: Row, e: Entity) {
        let a = self.entitys.load_alloc(row as usize, 1);
        *a = e;
    }

    #[inline(always)]
    pub(crate) fn get_column_unchecked(&self, index: ColumnIndex) -> &Column {
        unsafe { self.columns.get_unchecked(index as usize) }
    }
    #[inline(always)]
    pub fn alloc(&self) -> Row {
        self.entitys.alloc_index(1) as Row
    }
    /// 标记移出，用于delete 和 alter
    /// mark removes a key from the archetype, returning the value at the key if the
    /// key was not previously removed.
    #[inline(always)]
    pub(crate) fn mark_remove(&self, row: Row) -> Entity {
        let e = self.entitys.load_alloc(row as usize, 1);
        if e.is_null() {
            return *e;
        }
        self.removes.insert(row);
        replace(e, Entity::null())
    }
    /// 删除全部组件，用于delete
    #[inline(always)]
    pub(crate) fn drop_row(&self, row: Row) {
        for t in self.columns.iter() {
            t.drop_row(row);
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
        let mut r = true;
        // 先整理每个列，如果所有列的脏列表成功清空
        for c in self.columns.iter_mut() {
            r &= c.collect_dirty();
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
            Self::removes_action(&self.removes, remove_len, self.entitys.len(), action, set);
        // 清理removes
        self.removes.clear(1);
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
                    self.entitys.get_unchecked_mut(*src as usize),
                    Entity::null(),
                )
            };
            *unsafe { self.entitys.get_unchecked_mut(*dst as usize) } = e;
            // 修改world上entity的地址
            world.replace_row(e, *dst);
        }
        // 设置成正确的长度
        unsafe {
            self.entitys.set_len(new_entity_len);
        };
        // 整理合并内存
        self.entitys.collect(1);
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
            for (row, e) in self.entitys.iter().enumerate() {
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
            .field("entitys", &self.entitys)
            .field("columns", &self.columns)
            .field("removes", &self.removes)
            .finish()
    }
}
