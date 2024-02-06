//! 记录
//! 为每个Query::Filter的Added和Changed都维护一个单独的AddedIndex和ChangedIndex
//! 并且将AddedIndex和ChangedIndex注册到关联的原型列上
//! AddedIndex和ChangedIndex仅记录本system query读取到的最后位置
//! 每Column用AppendVec记录所有Added和Changed的Row，也不做去重
//! 查询时，每Query::Filter内维护自己的fixedbitset，用来做去重（可去重多次修改相同组件和或修改相同组件）
//! 查询时，如果读到某个Row的Entity为0，表示有正在写的记录，则中断本次查询并记录到AddedIndex和ChangedIndex上
//! 整理时，如果该Column所关联的所有AddedIndex都和该AppendVec的长度，则所有AddedIndex和AppendVec就可以清空，否则继续保留，等下一帧再检查是否清空。 因为必须保证Row不被错误重用，只有清空的情况下，才可以做原型的Removed整理。
//!
use core::fmt::*;
use std::any::TypeId;

use pi_append_vec::AppendVec;
use pi_share::ShareUsize;
use smallvec::SmallVec;

use crate::archetype::{Row, ColumnIndex};

#[derive(Debug, Default, Clone, Copy)]
pub struct RecordIndex {
    component_index: i32, // 对应组件的位置
    vec_index: u32,
}
impl RecordIndex {
    #[inline]
    pub fn new(is_changed: bool, component_index: ColumnIndex, vec_index: usize) -> Self {
        let component_index = if is_changed {
            component_index as i32
        } else {
            -(component_index as i32 + 1)
        };
        RecordIndex {
            component_index,
            vec_index: vec_index as u32,
        }
    }
    // #[inline]
    // pub(crate) fn get_iter<'a>(self, records: &'a Vec<ComponentRecord>) -> Iter<'a, Row> {
    //     if self.component_index >= 0 {
    //         let r = unsafe {
    //             records
    //                 .get_unchecked(self.component_index as usize)
    //                 .changeds
    //                 .get_unchecked(self.vec_index as usize)
    //         };
    //         r.vec.iter()
    //     } else {
    //         let r = unsafe {
    //             records
    //                 .get_unchecked(-self.component_index as usize - 1)
    //                 .addeds
    //                 .get_unchecked(self.vec_index as usize)
    //         };
    //         // 为了安全的在其他线程正在新增条目时读取Add，记录本次读取到的位置
    //         let end = r.vec.len();
    //         let start = r.len;
    //         // todo!() r.len = end;
    //         r.vec.slice(start..end)
    //     }
    // }
}
#[derive(Debug, Default)]
pub struct ComponentRecord {
    pub(crate) addeds: Vec<AddedRecord>,
    pub(crate) changeds: Vec<ChangedRecord>,
}
impl ComponentRecord {
    pub(crate) fn insert(&mut self, owner: TypeId, changed: bool) {
        if changed {
            self.changeds.push(ChangedRecord::new(owner));
        }else{
            self.addeds.push(AddedRecord::new(owner));
        }
    }
    pub fn find(
        &self,
        index: ColumnIndex,
        owner: TypeId,
        changed: bool,
        vec: &mut SmallVec<[RecordIndex; 1]>,
    ) {
        if changed {
            for (j, d) in self.changeds.iter().enumerate() {
                if d.owner == owner {
                    vec.push(RecordIndex::new(changed, index, j))
                }
            }
        } else {
            for (j, d) in self.addeds.iter().enumerate() {
                if d.owner == owner {
                    vec.push(RecordIndex::new(changed, index, j))
                }
            }
        }
    }
    #[inline(always)]
    pub(crate) fn added(&self, row: Row) {
        for r in self.addeds.iter() {
            r.added(row)
        }
    }
    #[inline(always)]
    pub(crate) fn added_iter<I: IntoIterator<Item=Row> + Clone>(&self, it: I) {
        for r in self.addeds.iter() {
            for k in it.clone().into_iter() {
                r.added(k)
            }
        }
    }
    pub(crate) fn changed(&self, row: Row) {
        // for r in self.changeds.iter() {
        //     r.changed(row)
        // }
    }
    pub(crate) fn collect(&self) {
        for r in self.addeds.iter() {
            r.collect()
        }
        for r in self.changeds.iter() {
            r.collect()
        }
    }
}
#[derive(Debug)]
pub struct AddedRecord {
    pub(crate) owner: TypeId,     // 那个所有者
    vec: AppendVec<Row>, // 为该组件类型被新增的row，可能被其他system添加，所以使用AppendVec
    len: usize, // 上次读取到的长度， 在清理时，如果和vec的len()相等，则vec可以clear(), 否则就需要移动add_len后面的元素到前面去
}

impl AddedRecord {
    #[inline(always)]
    pub fn new(owner: TypeId) -> Self {
        Self {
            owner,
            vec: Default::default(),
            len: 0,
        }
    }
    // 添加新增的条目
    #[inline(always)]
    pub fn added(&self, row: Row) {
        self.vec.insert(row);
    }
    // 整理方法， 清理修改条目， 检查添加条目是否全部读取，如果全部读取，则也清理
    pub fn collect(&self) {
        if self.len == self.vec.len() {
            // unsafe { self.vec.clear() };
            // todo!() self.len = 0;
        }
    }
}

pub struct ChangedRecord {
    pub(crate) owner: TypeId,     // 那个所有者
    // flags: Arr<ShareU32>,         // 该组件类型被修改的脏标记，可能被多线程写，所以用Arr
    // tick: Tick, // 上次读取时的tick
    // vec: AppendVec<Row>, // 为该组件类型被修改的脏，可能被多线程写
    len: ShareUsize, // 上次读取到的长度， 在清理时，如果和vec的len()相等，则vec可以clear(), 否则就需要移动add_len后面的元素到前面去
}

impl ChangedRecord {
    #[inline(always)]
    pub fn new(owner: TypeId) -> Self {
        Self {
            owner,
            //flags: Default::default(),
            //tick: 0,
            // vec: Default::default(),
            len: ShareUsize::new(0),
        }
    }
    // 设置修改的条目，会进行标记检查
    // #[inline]
    // pub fn changed(&self, row: Row) {
    //     const DIV: u32 = 5;
    //     const MASK: u32 = 0b11111;
    //     let index = row >> DIV;
    //     let offset = row & MASK;
    //     let flag: &ShareU32 = self.flags.load_alloc(index as usize);
    //     loop {
    //         let v = flag.load(Ordering::Relaxed);
    //         // 计算掩码，用于检查 offset 位的二进制值是否为 0
    //         let mask = 1 << offset;
    //         // 如果 v 的 offset 位为 0，说明已经标记，返回
    //         if v & mask == 0 {
    //             return;
    //         }
    //         // 将 v 的 offset 位设置为 0，并保存到 vv
    //         let vv = v & !mask;
    //         if flag
    //             .compare_exchange(v, vv, Ordering::Relaxed, Ordering::Relaxed)
    //             .is_ok()
    //         {
    //             break;
    //         }
    //     }
    //     self.vec.insert(row);
    // }
    // 整理方法， 清理修改条目， 检查添加条目是否全部读取，如果全部读取，则也清理
    pub fn collect(&self) {
        todo!()
        // unsafe { self.vec.reset();
        // self.flags.clear(); }
    }
}


impl Debug for ChangedRecord {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("ChangedRecord")
            .field("owner", &self.owner)
            .field("vec", &self.len)
            .finish()
    }
}