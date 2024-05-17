//! todo 处理0长度的组件

use core::fmt::*;
use std::{
    mem::{replace, take, transmute},
    ptr::null_mut,
};

use pi_append_vec::AppendVec;
use pi_arr::{Arr, Location, BUCKETS};
use pi_null::Null;

use crate::{
    archetype::{ComponentInfo, Row, COMPONENT_TICK},
    dirty::Dirty,
    prelude::Entity,
    world::Tick,
};

pub struct Column {
    blob: Blob,
    pub(crate) ticks: AppendVec<Tick>,
    pub(crate) dirty: Dirty, // Alter和Insert产生的添加脏和Query产生的修改脏，
}

impl Column {
    #[inline(always)]
    pub fn new(info: ComponentInfo) -> Self {
        Self {
            blob: Blob::new(info),
            ticks: Default::default(),
            dirty: Default::default(),
        }
    }
    #[inline(always)]
    pub fn info(&self) -> &ComponentInfo {
        &self.blob.info
    }
    #[inline(always)]
    pub fn info_mut(&mut self) -> &mut ComponentInfo {
        &mut self.blob.info
    }
    #[inline(always)]
    pub fn get_tick_unchecked(&self, row: Row) -> Tick {
        // todo!()
        self.ticks.get_i(row.0 as usize).map_or(Tick::default(), |t| *t)
    }
    #[inline(always)]
    pub fn get_tick(&self, row: Row) -> Option<Tick> {
        self.ticks.get(row.0 as usize).map(|t| *t)
    }
    #[inline]
    pub fn add_record(&self, e: Entity, row: Row, tick: Tick) {
        if self.info().tick_removed & COMPONENT_TICK == 0 {
            return;
        }
        // println!("add_record1===={:?}", (e, self.is_record_tick, self.ticks.load_alloc(row.0 as usize), row, tick, &self.blob.info.type_name));
        *self.ticks.load_alloc(row.0 as usize) = tick;
        self.dirty.record(e, row);
    }
    #[inline]
    pub fn change_record(&self, e: Entity, row: Row, tick: Tick) {
        if self.info().tick_removed & COMPONENT_TICK == 0 {
            return;
        }
        let old = self.ticks.load_alloc(row.0 as usize);
        if *old >= tick {
            return;
        }
        *old = tick;
        self.dirty.record(e, row);
    }
    #[inline]
    pub fn add_record_unchecked(&self, e: Entity, row: Row, tick: Tick) {
        *self.ticks.load_alloc(row.index()) = tick;
        self.dirty.record(e, row);
    }
    #[inline(always)]
    pub fn get<T>(&self, row: Row) -> &T {
        unsafe { transmute(self.blob.get(row)) }
    }
    #[inline(always)]
    pub fn get_mut<T>(&self, row: Row) -> &mut T {
        unsafe {
            let ptr: *mut T = transmute(self.blob.get(row));
            transmute(ptr)
        }
    }
    #[inline(always)]
    pub fn load(&self, row: Row) -> *mut u8 {
        unsafe { self.blob.load(row) }
    }
    #[inline(always)]
    pub(crate) fn write<T>(&self, row: Row, val: T) {
        unsafe {
            let ptr: *mut T = transmute(self.blob.load(row));
            ptr.write(val)
        };
    }
    #[inline(always)]
    pub fn get_row(&self, row: Row) -> *mut u8 {
        unsafe { self.blob.get(row) }
    }
    #[inline(always)]
    pub fn write_row(&self, row: Row, data: *mut u8) {
        unsafe {
            let dst = self.blob.load(row);
            data.copy_to_nonoverlapping(dst, self.info().mem_size);
        }
    }
    #[inline(always)]
    pub(crate) fn drop_row(&self, row: Row) {
        if let Some(f) = self.info().drop_fn {
            f(unsafe { self.blob.get(row) })
        }
    }
    #[inline(always)]
    pub fn needs_drop(&self) -> bool {
        self.info().drop_fn.is_some()
    }
    #[inline(always)]
    pub fn drop_row_unchecked(&self, row: Row) {
        self.info().drop_fn.unwrap()(unsafe { transmute(self.blob.get(row)) })
    }
    /// 扩容
    pub fn reserve(&mut self, len: usize, additional: usize) {
        self.blob.reserve(len, additional);
        if self.info().tick_removed & COMPONENT_TICK != 0 {
            self.ticks.reserve(additional);
        }
        self.dirty.reserve(additional);
    }
    /// 整理合并空位
    pub(crate) fn collect(&mut self, entity_len: usize, action: &Vec<(Row, Row)>) {
        if self.info().tick_removed & COMPONENT_TICK != 0 {
            for (src, dst) in action.iter() {
                self.collect_key(src, dst);
                unsafe {
                    let tick = self.ticks.get_unchecked((*src).0 as usize);
                    *self.ticks.get_unchecked_mut((*dst).0 as usize) = *tick;
                }
            }
            self.ticks.collect();
        } else {
            for (src, dst) in action.iter() {
                self.collect_key(src, dst);
            }
        }
        // 整理合并内存
        self.blob.reserve(entity_len, 0);
    }
    /// 整理合并指定的键
    fn collect_key(&mut self, src: &Row, dst: &Row) {
        unsafe {
            let src_data: *mut u8 = transmute(self.blob.get(*src));
            let dst_data: *mut u8 = transmute(self.blob.get(*dst));
            src_data.copy_to_nonoverlapping(dst_data, self.info().mem_size);
        }
    }
}

impl Debug for Column {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("Column")
            .field("info", &self.info())
            .field("changed", &self.dirty)
            .finish()
    }
}

struct Blob {
    vec: Vec<u8>,
    arr: Arr<u8>,
    info: ComponentInfo,
    vec_capacity: usize,
}
impl Blob {
    #[inline(always)]
    pub fn new(info: ComponentInfo) -> Self {
        let (vec_capacity, vec) = if info.mem_size == 0 {
            let mut vec = Vec::with_capacity(1);
            unsafe { vec.set_len(1) };
            (std::usize::MAX, vec)
        } else {
            (0, Vec::new())
        };
        Self {
            vec: vec,
            arr: Arr::new(),
            info,
            vec_capacity,
        }
    }
    #[inline(always)]
    pub unsafe fn get(&self, row: Row) -> *mut u8 {
        assert!(!row.is_null());
        let row = row.0 as usize;
        if row < self.vec_capacity {
            // todo get_unchecked()
            return transmute(self.vec.get(row * self.info.mem_size).unwrap());
        }
        let mut loc = Location::of(row - self.vec_capacity);
        loc.entry *= self.info.mem_size;
        // println!("======{:p}, {:?}", self, (row, &loc, self.info.mem_size));
        // todo get_unchecked()
        transmute(self.arr.get(&loc).unwrap())
    }
    #[inline(always)]
    pub unsafe fn load(&self, row: Row) -> *mut u8 {
        let row = row.0 as usize;
        if row < self.vec_capacity {
            // todo get_unchecked()
            return transmute(self.vec.get(row * self.info.mem_size).unwrap());
        }
        let mut loc = Location::of(row - self.vec_capacity);
        loc.entry *= self.info.mem_size;
        loc.len *= self.info.mem_size;
        // println!("load======{:p}, {:?}", self, (row, &loc, self.info.mem_size));
        transmute(self.arr.load_alloc(&loc))
    }
    #[inline(always)]
    fn vec_reserve(&mut self, additional: usize) {
        self.vec.reserve(additional * self.info.mem_size);
        unsafe { self.vec.set_len(self.vec.capacity()) };
        self.vec_capacity = self.vec.capacity() / self.info.mem_size;
    }
    /// 扩容additional，并将arr的内容移动到vec上，让内存连续，并且没有原子操作
    #[inline(always)]
    pub fn reserve(&mut self, len: usize, additional: usize) {
        if self.info.mem_size == 0 {
            return;
        }

        if len <= self.vec_capacity {
            return self.vec_reserve(additional);
        }
        let loc = Location::of(len - self.vec_capacity);
        let mut raw_len = Location::index(loc.bucket as u32 + 1, 0) * self.info.mem_size;
        let mut arr = self.replace(self.arr.replace());
        if self.vec.capacity() == 0 {
            // 如果原vec为empty，则直接将arr的0位vec换上
            raw_len = raw_len.saturating_sub(arr[0].len());
            let _ = replace(&mut self.vec, take(&mut arr[0]));
        }
        // 将vec扩容
        self.vec.reserve(raw_len + additional * self.info.mem_size);
        for mut v in arr.into_iter() {
            raw_len = raw_len.saturating_sub(v.len());
            self.vec.append(&mut v);
            if raw_len == 0 {
                break;
            }
        }
        unsafe { self.vec.set_len(self.vec.capacity()) };
        self.vec_capacity = self.vec.capacity() / self.info.mem_size;
    }
    fn replace(&self, arr: [*mut u8; BUCKETS]) -> [Vec<u8>; BUCKETS] {
        let mut buckets = [0; BUCKETS].map(|_| Vec::new());
        for (i, ptr) in arr.iter().enumerate() {
            if *ptr != null_mut() {
                let len = Location::bucket_len(i) * self.info.mem_size;
                buckets[i] = unsafe { Vec::from_raw_parts(*ptr, len, len) };
            }
        }
        buckets
    }
}
