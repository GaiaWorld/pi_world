
use core::fmt::*;
use std::mem::transmute;

use pi_append_vec::{AppendVec, DEFALLT_CAPACITY};

use crate::{record::ComponentRecord, archetype::{ComponentInfo, Row}};

pub type Data = *mut u8;

pub struct Column {
    vec: AppendVec<u8>,
    pub(crate) info: ComponentInfo,
    // ticks: Vec<Tick>, // 当前的组件修改tick的容器
    // addeds: AppendVec<Row>, // Mutate产生的添加记录，
    // changeds: AppendVec<Row>, // Query产生的修改记录，
    pub(crate) record: ComponentRecord, // 组件的添加修改记录，
    // addeds: SyncBlobVec, // 本帧内被Mutate添加的线程安全的组件容器
    // add_ticks: SyncAppendVec<Tick>, // add的组件修改tick的容器，也需要线程安全
}

impl Column {
    #[inline(always)]
    pub fn new(info: ComponentInfo) -> Self {
        let vec = AppendVec::with_capacity_multiple(DEFALLT_CAPACITY, info.mem_size as usize);
        Self {
            info,
            vec,
            record: Default::default(),
        }
    }
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.vec.len() == 0
    }
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.vec.len() / self.info.mem_size as usize
    }
    #[inline(always)]
    pub fn get<T>(&self, row: Row) -> &T {
        unsafe {transmute(self.get_row(row))}
    }
    #[inline(always)]
    pub fn get_mut<T>(&self, row: Row) -> &mut T {
        unsafe{
            let ptr: *mut T = transmute(self.get_row(row));
            transmute(ptr)
        }
    }
    #[inline(always)]
    pub unsafe fn get_row(&self, row: Row) -> &u8 {
        self.vec.get_unchecked((row * self.info.mem_size) as usize)
    }
    #[inline(always)]
    pub fn write<T>(&self, row: Row, val: T) {
        unsafe { let ptr: *mut T = transmute(self.vec.load_alloc((row * self.info.mem_size) as usize, self.info.mem_size as usize));
        ptr.write(val) };
    }
    #[inline(always)]
    pub fn record(&self, row: Row) {
        if self.record.addeds.len() > 0 {
            self.record.added(row);
        }
    }
    #[inline(always)]
    pub fn remove(&self, row: Row) {
        if let Some(f) = self.info.drop_fn {
            f(unsafe { transmute(self.get_row(row)) })
        }
    }

    #[inline(always)]
    pub fn alloc(&mut self) -> Data {
        let len = self.len();
        unsafe { // todo
            // let vec: &mut Vec<u8> = transmute(&self.vec as *const Vec<u8>);
            // self.vec.reserve(self.info.mem_size as usize);
            // self.vec.set_len(len + self.info.mem_size as usize);
            transmute(self.vec.get_unchecked(len))
        }
    }
    /// 整理方法
    pub(crate) fn collect(&self) {
        for r in self.record.addeds.iter() {
            r.collect()
        }
        for r in self.record.changeds.iter() {
            r.collect()
        }
    }
}
impl Drop for Column {
    fn drop(&mut self) {
        // for (entries, mut len) in self.vec.into_iter() {
        //     len *= self.components_size;
        //     unsafe { drop(Vec::from_raw_parts(entries, len, len)) }
        // }
    }
}
impl Debug for Column {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("Column")
            .field("len", &self.len())
            .finish()
    }
}
