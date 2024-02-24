//! todo 处理0长度的组件

use core::fmt::*;
use std::mem::transmute;

use pi_append_vec::AppendVec;

use crate::{
    archetype::{ComponentInfo, Row},
    dirty::ComponentDirty,
};

pub struct Column {
    vec: AppendVec<u8>,
    pub(crate) info: ComponentInfo,
    pub(crate) added: ComponentDirty, // // Alter和Insert产生的添加脏，
    pub(crate) changed: ComponentDirty, // Query产生的修改脏，
}

impl Column {
    #[inline(always)]
    pub fn new(info: ComponentInfo) -> Self {
        let vec = AppendVec::with_capacity_multiple(0, info.mem_size as usize);
        Self {
            info,
            vec,
            added: Default::default(),
            changed: Default::default(),
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
        unsafe { transmute(self.get_row(row)) }
    }
    #[inline(always)]
    pub fn get_mut<T>(&self, row: Row) -> &mut T {
        unsafe {
            let ptr: *mut T = transmute(self.get_row(row));
            transmute(ptr)
        }
    }
    #[inline(always)]
    pub unsafe fn get_row(&self, row: Row) -> &u8 {
        self.vec.get_unchecked((row * self.info.mem_size) as usize)
    }
    #[inline(always)]
    pub fn write_row(&self, row: Row, data: *mut u8) {
        let dst = self.vec.load_alloc(
            (row * self.info.mem_size) as usize,
            self.info.mem_size as usize,
        );
        unsafe {
            data.copy_to_nonoverlapping(transmute(dst), self.info.mem_size as usize)
        }
    }

    #[inline(always)]
    pub(crate) fn write<T>(&self, row: Row, val: T) {
        unsafe {
            let ptr: *mut T = transmute(self.vec.load_alloc(
                (row * self.info.mem_size) as usize,
                self.info.mem_size as usize,
            ));
            ptr.write(val)
        };
    }
    #[inline(always)]
    pub(crate) fn drop_row(&self, row: Row) {
        if let Some(f) = self.info.drop_fn {
            f(unsafe { transmute(self.get_row(row)) })
        }
    }
    #[inline(always)]
    pub fn needs_drop(&self) -> bool {
        self.info.drop_fn.is_some()
    }
    #[inline(always)]
    pub fn drop_row_unchecked(&self, row: Row) {
        self.info.drop_fn.unwrap()(unsafe { transmute(self.get_row(row)) })
    }
    /// 整理合并空位
    pub(crate) fn collect(
        &mut self,
        entity_len: usize,
        action: &Vec<(Row, Row)>,
    ) {
        for (src, dst ) in action.iter() {
            unsafe {
                let src_data: *mut u8 = transmute(self.get_row(*src));
                let dst_data: *mut u8 = transmute(self.get_row(*dst));
                src_data.copy_to_nonoverlapping(dst_data, self.info.mem_size as usize);
            }
        }
        // 设置成正确的长度
        unsafe {
            self.vec.set_len(entity_len * self.info.mem_size as usize)
        };
        // 整理合并内存
        self.vec.collect(self.info.mem_size as usize);
    }
    /// 整理方法，返回该列的脏列表是否清空
    pub(crate) fn collect_dirty(&mut self) -> bool {
        self.added.collect() && self.changed.collect()
    }
}

impl Debug for Column {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("Column")
            .field("len", &self.len())
            .field("info", &self.info)
            .field("added", &self.added)
            .field("changed", &self.changed)
            .finish()
    }
}
