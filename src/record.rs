//! 记录
//! 为每个Query::Filter的Added和Changed都维护一个单独的AddedRecord和ChangedRecord
//! 并且将AddedRecord和ChangedRecord注册到关联的原型上
//!
use core::fmt::*;
use std::any::TypeId;
use std::sync::atomic::Ordering;

use pi_append_vec::AppendVec;
use pi_arr::*;
use pi_share::{ShareU32, ShareUsize};
use smallvec::SmallVec;

use crate::archetype::{ArchetypeKey, ComponentIndex};

#[derive(Debug, Default, Clone, Copy)]
pub struct RecordIndex {
    component_index: i32, // 对应组件的位置
    vec_index: u32,
}
impl RecordIndex {
    #[inline]
    pub fn new(is_changed: bool, component_index: ComponentIndex, vec_index: usize) -> Self {
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
    #[inline]
    pub(crate) fn get_iter<'a>(self, records: &'a Vec<ComponentRecord>) -> Iter<'a, ArchetypeKey> {
        if self.component_index >= 0 {
            let r = unsafe {
                records
                    .get_unchecked(self.component_index as usize)
                    .changeds
                    .get_unchecked(self.vec_index as usize)
            };
            r.vec.iter()
        } else {
            let r = unsafe {
                records
                    .get_unchecked(-self.component_index as usize - 1)
                    .addeds
                    .get_unchecked(self.vec_index as usize)
            };
            // 为了安全的在其他线程正在新增条目时读取Add，记录本次读取到的位置
            let end = r.vec.len();
            let start = r.add_len.load(Ordering::Relaxed);
            r.add_len.store(end, Ordering::Relaxed);
            r.vec.slice(start..end)
        }
    }
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
        index: ComponentIndex,
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
    pub(crate) fn added(&self, key: ArchetypeKey) {
        for r in self.addeds.iter() {
            r.added(key)
        }
    }
    #[inline(always)]
    pub(crate) fn added_iter<I: IntoIterator<Item=ArchetypeKey> + Clone>(&self, it: I) {
        for r in self.addeds.iter() {
            for k in it.clone().into_iter() {
                r.added(k)
            }
        }
    }
    pub(crate) fn changed(&self, key: ArchetypeKey) {
        for r in self.changeds.iter() {
            r.changed(key)
        }
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
    vec: AppendVec<ArchetypeKey>, // 为该组件类型被新增的key，可能被其他system添加，所以使用AppendVec
    add_len: ShareUsize, // 上次读取到的长度， 在清理时，如果和add的len()相等，则add可以clear(), 否则就需要移动add_len后面的元素到前面去
}

impl AddedRecord {
    #[inline(always)]
    pub fn new(owner: TypeId) -> Self {
        Self {
            owner,
            vec: Default::default(),
            add_len: ShareUsize::new(0),
        }
    }
    // 添加新增的条目
    #[inline(always)]
    pub fn added(&self, key: ArchetypeKey) {
        self.vec.insert(key);
    }
    // 整理方法， 清理修改条目， 检查添加条目是否全部读取，如果全部读取，则也清理
    pub fn collect(&self) {
        if self.add_len.load(Ordering::Relaxed) == self.vec.len() {
            unsafe { self.vec.reset() };
            self.add_len.store(0, Ordering::Relaxed);
        }
    }
}

pub struct ChangedRecord {
    pub(crate) owner: TypeId,     // 那个所有者
    flags: Arr<ShareU32>,         // 该组件类型被修改的脏标记，可能被多线程写，所以用Arr
    vec: AppendVec<ArchetypeKey>, // 为该组件类型被修改的脏，可能被多线程写
}

impl ChangedRecord {
    #[inline(always)]
    pub fn new(owner: TypeId) -> Self {
        Self {
            owner,
            flags: Default::default(),
            vec: Default::default(),
        }
    }
    // 设置修改的条目，会进行标记检查
    #[inline]
    pub fn changed(&self, key: ArchetypeKey) {
        const DIV: u32 = 5;
        const MASK: usize = 0b11111;
        let index = key >> DIV;
        let offset = key & MASK;
        let flag: &ShareU32 = self.flags.load_alloc(index as usize);
        loop {
            let v = flag.load(Ordering::Relaxed);
            // 计算掩码，用于检查 offset 位的二进制值是否为 0
            let mask = 1 << offset;
            // 如果 v 的 offset 位为 0，说明已经标记，返回
            if v & mask == 0 {
                return;
            }
            // 将 v 的 offset 位设置为 0，并保存到 vv
            let vv = v & !mask;
            if flag
                .compare_exchange(v, vv, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
        self.vec.insert(key);
    }
    // 整理方法， 清理修改条目， 检查添加条目是否全部读取，如果全部读取，则也清理
    pub fn collect(&self) {
        unsafe { self.vec.reset();
        self.flags.clear(); }
    }
}


impl Debug for ChangedRecord {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("ChangedRecord")
            .field("owner", &self.owner)
            .field("vec", &self.vec)
            .finish()
    }
}