use core::fmt::*;
use std::{
    mem::transmute,
    ops::{Deref, DerefMut},
};

use pi_arr::Arr;
use pi_null::Null;

use crate::{
    archetype::{ArchetypeIndex, ComponentInfo, Row},
    world::{Entity, Tick},
};

pub struct Column {
    pub(crate) info: ComponentInfo,
    pub(crate) arr: Arr<BlobTicks>,
}
impl Column {
    #[inline(always)]
    pub fn new(info: ComponentInfo) -> Self {
        Self {
            info,
            arr: Arr::default(),
        }
    }
    #[inline(always)]
    pub fn info(&self) -> &ComponentInfo {
        &self.info
    }
    #[inline(always)]
    pub fn info_mut(&mut self) -> &mut ComponentInfo {
        &mut self.info
    }
    // 初始化原型对应列的blob
    pub fn init_blob(&self, index: ArchetypeIndex) {
        unsafe { self.arr.load_alloc(index.index()).blob.set_vec_capacity(0) };
    }
    // 列是否包含指定原型
    pub fn contains(&self, index: ArchetypeIndex) -> bool {
        match self.arr.load(index.index()) {
            Some(b) => !b.blob.vec_capacity().is_null(),
            None => false,
        }
    }
    #[inline(always)]
    pub fn blob_ref_unchecked(&self, index: ArchetypeIndex) -> BlobRef<'_> {
        BlobRef::new(unsafe { self.arr.load_unchecked(index.index()) }, &self.info)
    }
    #[inline(always)]
    pub fn blob_ref(&self, index: ArchetypeIndex) -> Option<BlobRef<'_>> {
        let b = match self.arr.load(index.index()){
            Some(b) => b,
            _ => return None,
        };
        if b.blob.vec_capacity().is_null() {
            return None
        }
        Some(BlobRef::new(b, &self.info))
    }
    /// 整理合并空位
    pub(crate) fn settle(
        &mut self,
        index: ArchetypeIndex,
        len: usize,
        additional: usize,
        action: &Vec<(Row, Row)>,
    ) {
        // 判断ticks，进行ticks的整理
        let b = unsafe { self.arr.get_unchecked_mut(index.index()) };
        let r = BlobRef::new(b, &self.info);
        if self.info.is_tick() {
            for (src, dst) in action.iter() {
                unsafe {
                    // 移动指定的键到空位上
                    let src_data: *mut u8 = r.load(*src);
                    let dst_data: *mut u8 = r.load(*dst);
                    src_data.copy_to_nonoverlapping(dst_data, self.info.size());
                    // 及其tick
                    let tick = r.get_tick_unchecked(*src);
                    r.set_tick_unchecked(*dst, tick);
                }
            }
            // 整理合并blob内存
            b.blob.settle(len, additional, self.info.size());
            // 整理合并ticks内存
            b.ticks.settle(len, additional, 1);
            return;
        }
        for (src, dst) in action.iter() {
            unsafe {
                // 整理合并指定的键
                let src_data: *mut u8 = r.load(*src);
                let dst_data: *mut u8 = r.load(*dst);
                src_data.copy_to_nonoverlapping(dst_data, self.info.size());
            }
        }
        // 整理合并blob内存
        b.blob.settle(len, additional, self.info.size());
    }
}
impl Debug for Column {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("Column").field("info", &self.info).finish()
    }
}

struct Blob(Arr<u8>);
impl Default for Blob {
    fn default() -> Self {
        let mut arr = Arr::default();
        unsafe { arr.set_vec_capacity(usize::null()) };
        Self(arr)
    }
}
impl Drop for Blob {
    fn drop(&mut self) {
        if self.vec_capacity().is_null() {
            unsafe { self.set_vec_capacity(0) };
        }
    }
}

impl Deref for Blob {
    type Target = Arr<u8>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Blob {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}


#[derive(Default)]
pub(crate) struct BlobTicks {
    blob: Blob,
    pub(crate) ticks: Arr<Tick>,
}

#[derive(Clone)]
pub struct BlobRef<'a> {
    pub(crate) blob: &'a BlobTicks,
    pub(crate) info: &'a ComponentInfo,
}

impl<'a> BlobRef<'a> {
    #[inline(always)]
    pub(crate) fn new(blob: &'a mut BlobTicks, info: &'a ComponentInfo) -> Self {
        Self { blob, info }
    }
    #[inline(always)]
    pub fn get_tick_unchecked(&self, row: Row) -> Tick {
        // todo !()
        self.blob
            .ticks
            .get(row.index())
            .map_or(Tick::default(), |t| *t)
    }
    #[inline]
    pub fn added_tick(&self, e: Entity, row: Row, tick: Tick) {
        if !self.info.is_tick() {
            return;
        }
        // println!("add_record1===={:?}", (e, row, tick, self.info.type_name()));
        *self.blob.ticks.load_alloc(row.0 as usize) = tick;
        // self.column.dirty.record(e, row, tick);
    }
    #[inline]
    pub fn changed_tick(&self, e: Entity, row: Row, tick: Tick) {
        // println!("changed_tick: {:?}", (e, row, tick, self.info));
        if !self.info.is_tick() {
            return;
        }
        let old = self.blob.ticks.load_alloc(row.0 as usize);
        if *old >= tick {
            return;
        }
        *old = tick;
        // self.column.dirty.record(e, row, tick);
    }
    #[inline]
    pub fn set_tick_unchecked(&self, row: Row, tick: Tick) {
        *self.blob.ticks.load_alloc(row.index()) = tick;
        // self.column.dirty.record(e, row, tick);
    }
    #[inline(always)]
    pub fn get<T>(&self, row: Row) -> &T {
        unsafe { transmute(self.load(row)) }
    }
    #[inline(always)]
    pub fn get_mut<T>(&self, row: Row) -> &mut T {
        unsafe { transmute(self.load(row)) }
    }
    #[inline(always)]
    pub(crate) fn write<T>(&self, row: Row, val: T) {
        unsafe {
            let ptr: *mut T = transmute(self.load(row));
            ptr.write(val)
        };
    }
    // 如果没有分配内存，则返回的指针为is_null()
    #[inline(always)]
    pub fn get_row(&self, row: Row) -> *mut u8 {
        unsafe { transmute(self.blob.blob.get_multiple(row.index(), self.info.size())) }
    }
    // 一定会返回分配后的内存
    #[inline(always)]
    pub unsafe fn load(&self, row: Row) -> *mut u8 {
        assert!(!row.is_null());
        transmute(
            self.blob
                .blob
                .load_alloc_multiple(row.index(), self.info.size()),
        )
    }
    #[inline(always)]
    pub fn write_row(&self, row: Row, data: *mut u8) {
        unsafe {
            let dst = self.load(row);
            data.copy_to_nonoverlapping(dst, self.info.size());
        }
    }
    #[inline(always)]
    pub(crate) fn drop_row(&self, row: Row) {
        if let Some(f) = self.info.drop_fn {
            f(unsafe { transmute(self.load(row)) })
        }
    }
    #[inline(always)]
    pub fn needs_drop(&self) -> bool {
        self.info.drop_fn.is_some()
    }
    #[inline(always)]
    pub fn drop_row_unchecked(&self, row: Row) {
        self.info.drop_fn.unwrap()(unsafe { transmute(self.blob.blob.get(row.index())) })
    }
}

impl<'a> Debug for BlobRef<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("Column").field("info", &self.info).finish()
    }
}
