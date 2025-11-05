use core::fmt::*;
use std::{
    cell::SyncUnsafeCell,
    mem::transmute,
    ops::{Deref, DerefMut},
    sync::atomic::AtomicUsize,
};

use pi_append_vec::SafeVec;
use pi_arr::Arr;
use pi_null::Null;
use pi_share::Share;

use crate::{
    archetype::{Archetype, ArchetypeIndex, ComponentInfo, Row, ShareArchetype},
    event::ComponentEventVec,
    world::{Entity, Tick},
};

use crate::blob::Blob;
#[cfg(debug_assertions)]
pub static COMPONENT_INDEX: AtomicUsize = AtomicUsize::new(usize::MAX);
#[cfg(debug_assertions)]
pub static ARCHETYPE_INDEX: AtomicUsize = AtomicUsize::new(usize::MAX);

pub struct Column {
    pub(crate) info: ColumnInfo,
    pub(crate) arr: Arr<BlobTicks>,
    pub(crate) last_len: SyncUnsafeCell<usize>,
}
impl Column {
    pub fn memsize(&self) -> usize {
        let mut result = 0;
        let end = self.arr.capacity(0);
        for idx in 0..end {
            if let Some(item) = self.arr.get(idx) {
                result += item.memsize();
            }
        }
        // self.arr.slice(Range{ start: 0, end: self.arr.capacity(0) }).for_each(|item| {
        //     result += item.memsize();
        // });
        result += self.info.memsize();
        result
    }
    // #[inline(always)]
    pub fn new(info: ComponentInfo) -> Self {
        // log::warn!("New Column");
        Self {
            info: ColumnInfo {
                changed: None,
                added: None,
                removed: None,
                info,
            },
            arr: Arr::default(),
            last_len: SyncUnsafeCell::new(0usize.into()),
        }
    }
    #[inline(always)]
    pub fn info(&self) -> &ComponentInfo {
        &self.info
    }
    #[inline(always)]
    pub fn info_mut(&mut self) -> &mut ComponentInfo {
        &mut self.info.info
    }
    // 初始化原型对应列的blob
    pub fn init_blob(&self, index: ArchetypeIndex) {
        *unsafe { &mut *self.last_len.get() } = index.index() + 1;
        if self.info.info.size() == 0 {
            // 如果是0， 设置容量为最大数减1， std::usize::MAX有特殊意义， 表示blob未初始化
            unsafe { self.arr.load_alloc(index.index()).blob.set_vec_capacity(std::usize::MAX - 1) };
        } else {
            unsafe { self.arr.load_alloc(index.index()).blob.set_vec_capacity(0) };
        }
    }
    // 列是否包含指定原型
    pub fn contains(&self, index: ArchetypeIndex) -> bool {
        match self.arr.load(index.index()) {
            Some(b) => !b.blob.vec_capacity().is_null(),
            None => false,
        }
    }
    // #[inline(always)]
    pub fn blob_ref_unchecked(&self, index: ArchetypeIndex) -> BlobRef<'_> {
        #[cfg(debug_assertions)]
        let debug_a_index = ARCHETYPE_INDEX.load(std::sync::atomic::Ordering::Relaxed);
        #[cfg(debug_assertions)]
        let debug_c_index = COMPONENT_INDEX.load(std::sync::atomic::Ordering::Relaxed);
        #[cfg(debug_assertions)]
        if (!debug_a_index.is_null() || !debug_c_index.is_null())
            && (debug_a_index.is_null() || index.index() == debug_a_index)
            && (debug_c_index.is_null() || self.info.index.index() == debug_c_index)
        {
            println!(
                "blob_ref_unchecked, {} {:p}",
                self.arr.vec_capacity(),
                unsafe { self.arr.load_unchecked(index.index()) }
            );
        }
        BlobRef::new(
            unsafe { self.arr.load_unchecked(index.index()) },
            &self.info,
            #[cfg(debug_assertions)]
            index,
        )
    }
    // #[inline(always)]
    pub fn blob_ref(&self, index: ArchetypeIndex) -> Option<BlobRef<'_>> {
        #[cfg(debug_assertions)]
        let debug_a_index = ARCHETYPE_INDEX.load(std::sync::atomic::Ordering::Relaxed);
        #[cfg(debug_assertions)]
        let debug_c_index = COMPONENT_INDEX.load(std::sync::atomic::Ordering::Relaxed);
        #[cfg(debug_assertions)]
        if (!debug_a_index.is_null() || !debug_c_index.is_null())
            && (debug_a_index.is_null() || index.index() == debug_a_index)
            && (debug_c_index.is_null() || self.info.index.index() == debug_c_index)
        {
            println!("blob_ref, {} {:p}", self.arr.vec_capacity(), unsafe {
                self.arr.load_unchecked(index.index())
            });
        }
        let blob = match self.arr.load(index.index()) {
            Some(b) => b,
            _ => return None,
        };
        if blob.blob.vec_capacity().is_null() {
            return None;
        }
        Some(BlobRef::new(
            blob,
            &self.info,
            #[cfg(debug_assertions)]
            index,
        ))
    }
    /// 扫描当前的所有原型，设置已有的实体，主要是解决不同的Plugin，依次添加时，Changed监听和tick被后设置的问题
    pub(crate) fn update<F>(&self, archetypes: &SafeVec<ShareArchetype>, set_fn: F)
    where
        F: Fn(&BlobRef, Row, &Archetype),
    {
        // let mut size = 0;
        for ar in archetypes.iter() {
            if let Some(blob) = self.arr.load(ar.index.index()) {
                // 判断该原型是否包含该列
                if blob.blob.vec_capacity().is_null() {
                    continue;
                }
                let r = BlobRef::new(
                    blob,
                    &self.info,
                    #[cfg(debug_assertions)]
                    ar.index,
                );
                // 设置该列该原型下的所有实体
                for row in 0..ar.len().index() {
                    set_fn(&r, row.into(), &ar)
                }
                // size += blob.memsize();
            }
        }
        // log::warn!("Column {:?}", size);
    }
    /// 整理内存
    pub(crate) fn settle(&mut self) {
        let len = *self.last_len.get_mut();
        if len > self.arr.vec_capacity() {
            self.arr.settle(len, 0);
        }
    }
    /// 整理合并指定原型的空位
    pub(crate) fn settle_by_index(
        &mut self,
        index: ArchetypeIndex,
        len: usize,
        additional: usize,
        action: &Vec<(Row, Row)>,
    ) {
        if self.info.size() == 0 {
            return;
        }
        // 判断ticks，进行ticks的整理
        let blob = unsafe { self.arr.get_unchecked_mut(index.index()) };
        #[cfg(debug_assertions)]
        let debug_index = ARCHETYPE_INDEX.load(std::sync::atomic::Ordering::Relaxed);
        #[cfg(debug_assertions)]
        if (debug_index.is_null() || index.index() == debug_index)
            && self.info.index.index() == COMPONENT_INDEX.load(std::sync::atomic::Ordering::Relaxed)
        {
            println!(
                "Column settle_by_index:===={:?} {:p}",
                (index, blob.blob.vec_capacity(), action),
                blob
            );
        }
        let r = BlobRef::new(
            blob,
            &self.info,
            #[cfg(debug_assertions)]
            index,
        );
        if self.info.is_tick() {
            for (src, dst) in action.iter() {
                unsafe {
                    // 移动指定的键到空位上
                    let src_data: *mut u8 = r.get_blob(*src);
                    let dst_data: *mut u8 = r.load_blob(*dst);
                    src_data.copy_to_nonoverlapping(dst_data, self.info.size());
                    // 及其tick
                    let tick = r.get_tick_unchecked(*src);
                    r.set_tick_unchecked(*dst, tick);
                }
            }
            // 整理合并blob内存
            blob.blob.settle(len, additional, self.info.size());
            // 整理合并ticks内存
            blob.ticks.settle(len, additional);
            return;
        }
        for (src, dst) in action.iter() {
            unsafe {
                // 整理合并指定的键
                let src_data: *mut u8 = r.get_blob(*src);
                let dst_data: *mut u8 = r.load_blob(*dst);
                src_data.copy_to_nonoverlapping(dst_data, self.info.size());
            }
        }
        // 整理合并blob内存
        blob.blob.settle(len, additional, self.info.size());
    }
}
impl Debug for Column {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("Column")
            .field("info", &self.info.info)
            .field("last_index", &self.last_len.get())
            .finish()
    }
}

pub(crate) struct ColumnInfo {
    pub(crate) changed: Option<Share<ComponentEventVec>>,
    pub(crate) added: Option<Share<ComponentEventVec>>,
    pub(crate) removed: Option<Share<ComponentEventVec>>,
    pub(crate) info: ComponentInfo,
}
impl Deref for ColumnInfo {
    type Target = ComponentInfo;
    fn deref(&self) -> &Self::Target {
        &self.info
    }
}
impl ColumnInfo {
    pub fn memsize(&self) -> usize {
        let mut result = 0;
        if let Some(item) = &self.changed {
            result += item.capacity();
        }
        if let Some(item) = &self.added {
            result += item.capacity();
        }
        if let Some(item) = &self.removed {
            result += item.capacity();
        }
        result += self.info.size();
        result
    }
}

#[derive(Default)]
pub(crate) struct BlobTicks {
    blob: Blob,
    pub(crate) ticks: Arr<Tick>,
}
impl BlobTicks {
    pub fn memsize(&self) -> usize {
        
        self.blob.memsize() + self.ticks.capacity(0) * 4
    }
}

#[derive(Clone)]
pub struct BlobRef<'a> {
    pub(crate) blob: &'a BlobTicks,
    pub(crate) info: &'a ColumnInfo,
    #[cfg(debug_assertions)]
    index: ArchetypeIndex,
}

impl<'a> BlobRef<'a> {
    // #[inline(always)]
    pub(crate) fn new(
        blob: &'a mut BlobTicks,
        info: &'a ColumnInfo,
        #[cfg(debug_assertions)] index: ArchetypeIndex,
    ) -> Self {
        Self {
            blob,
            info,
            #[cfg(debug_assertions)]
            index,
        }
    }
    // #[inline(always)]
    pub fn get_tick_unchecked(&self, row: Row) -> Tick {
        self.blob
            .ticks
            .get(row.index())
            .map_or(Tick::default(), |t| *t)
    }
    // #[inline]
    pub fn added_tick(&self, e: Entity, row: Row, tick: Tick) {
        // println!("added_tick===={:?}", (e, row, tick, self.info.type_name()));
        if !self.info.is_tick() {
            return;
        }
        *self.blob.ticks.load_alloc(row.index()) = tick;
        if let Some(vec) = &self.info.added {
            vec.record(e);
        }
    }
    // #[inline(always)]
    pub fn changed_tick(&self, e: Entity, row: Row, tick: Tick) {
        // println!("changed_tick: {:?}", (e, row, tick, self.info.is_tick(), ));
        if !self.info.is_tick() {
            return;
        }
        let old = unsafe { self.blob.ticks.load_alloc(row.index()) };
        if *old >= tick {
            return;
        }
        *old = tick;
        if let Some(vec) = &self.info.changed {
            vec.record(e);
        }
    }
    // #[inline]
    pub fn set_tick_unchecked(&self, row: Row, tick: Tick) {
        *self.blob.ticks.load_alloc(row.index()) = tick;
    }
    fn trace(&self, row: Row, e: Entity, path: &str, src_data: *mut u8) {
        #[cfg(debug_assertions)]
        let debug_a_index = ARCHETYPE_INDEX.load(std::sync::atomic::Ordering::Relaxed);
        #[cfg(debug_assertions)]
        let debug_c_index = COMPONENT_INDEX.load(std::sync::atomic::Ordering::Relaxed);
        #[cfg(debug_assertions)]
        if (!debug_a_index.is_null() || !debug_c_index.is_null())
            && (debug_a_index.is_null() || self.index.index() == debug_a_index)
            && (debug_c_index.is_null() || self.info.index.index() == debug_c_index)
        {
            let ptr: *mut u8 = self
                .blob
                .blob
                .load_alloc_multiple(row.index(), self.info.size());
            let slice: &[u8] =
                unsafe { transmute(std::ptr::slice_from_raw_parts(src_data, self.info.size())) };
            println!(
                "Column {:?}:===={:?} blob_ptr:{:p}, dst:{:p}, src:{:p}, src_data:{:?}",
                path,
                (row, e, self.index, self.blob.blob.vec_capacity()),
                self.blob,
                ptr,
                src_data,
                slice,
            );
        }
    }
    #[inline(always)]
    pub fn get<T>(&self, row: Row, _e: Entity) -> &'a T {
        // self.trace(row, e, "get", std::ptr::null_mut());
        unsafe { transmute(self.get_blob(row)) }
    }
    #[inline(always)]
    pub fn get_unchecked<T>(&self, row: Row) -> &'a T {
        // self.trace(row, e, "get", std::ptr::null_mut());
        unsafe { transmute(self.get_blob_unchecked(row)) }
    }
    #[inline(always)]
    pub fn get_mut<T>(&self, row: Row, _e: Entity) -> &'a mut T {
        // self.trace(row, e, "get_mut", std::ptr::null_mut());
        unsafe { transmute(self.load_blob(row)) }
    }

    #[inline(always)]
    pub fn get_unchecked_mut<T>(&self, row: Row) -> &'a mut T {
        // self.trace(row, e, "get", std::ptr::null_mut());
        unsafe { transmute(self.get_blob_unchecked(row)) }
    }
    #[inline(always)]
    pub(crate) fn write<T>(&self, row: Row, e: Entity, val: T) {
        self.trace(row, e, "write", unsafe { transmute(&val) });
        unsafe {
            let ptr: *mut T = transmute(self.load_blob(row));
            ptr.write(val)
        };
    }
    // 如果没有分配内存，则返回的指针为is_null()
    // #[inline(always)]
    pub fn load(&self, row: Row, e: Entity) -> *mut u8 {
        self.trace(row, e, "load", std::ptr::null_mut());
        self.load_blob(row)
    }
    // 如果没有分配内存，则返回的指针为is_null()
    // #[inline(always)]
    pub fn get_row(&self, row: Row, e: Entity) -> *mut u8 {
        assert!(!row.is_null());
        self.trace(row, e, "get_row", std::ptr::null_mut());
        self.get_blob(row)
    }
    // #[inline(always)]
    pub fn write_row(&self, row: Row, e: Entity, data: *mut u8) {
        self.trace(row, e, "write_row", data);
        unsafe {
            let dst = self.load_blob(row);
            data.copy_to_nonoverlapping(dst, self.info.size());
        }
    }
    // #[inline(always)]
    pub(crate) fn drop_row(&self, row: Row, e: Entity) {
        assert!(!row.is_null());
        self.trace(row, e, "drop_row", std::ptr::null_mut());
        if let Some(f) = self.info.drop_fn {
            f(self.get_blob(row))
        }
    }
    // #[inline(always)]
    pub fn drop_row_unchecked(&self, row: Row, e: Entity) {
        assert!(!row.is_null());
        self.trace(row, e, "drop_row_unchecked", std::ptr::null_mut());
        self.info.drop_fn.unwrap()(self.get_blob(row))
    }

    // 如果没有分配内存，则返回的指针为is_null()
    // #[inline(always)]
    pub fn get_blob(&self, row: Row) -> *mut u8 {
        let blob: *const Blob = &self.blob.blob;
        let blob = unsafe { &mut *(blob as *mut Blob) };
        unsafe { transmute(blob.get_multiple(row.index(), self.info.size())) }
        // unsafe { transmute(blob.get_mut(row.index(), /* self.info.size() */)) }
    }

    // #[inline(always)]
    pub fn get_blob_unchecked(&self, row: Row) -> *mut u8 {
        assert!(!row.is_null());
        let blob: *const Blob = &self.blob.blob;
        let blob = unsafe { &mut *(blob as *mut Blob) };
        unsafe { transmute(blob.get_multiple_unchecked(row.index(), self.info.size())) }
        // unsafe { transmute(blob.get_unchecked_mut(row.index(), /* self.info.size() */)) }
    }
    // 一定会返回分配后的内存
    // #[inline(always)]
    pub fn load_blob(&self, row: Row) -> *mut u8 {
        assert!(!row.is_null());
        unsafe {
            transmute(
                self.blob
                    .blob
                    .load_alloc_multiple(row.index(), self.info.size()),
            )
        }
    }

    // #[inline(always)]
    pub fn needs_drop(&self) -> bool {
        self.info.drop_fn.is_some()
    }
}

impl<'a> Debug for BlobRef<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("Column")
            .field("info", &self.info.info)
            .finish()
    }
}
