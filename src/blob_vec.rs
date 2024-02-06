
use core::fmt::*;
use std::{mem::{size_of, transmute, ManuallyDrop, MaybeUninit, replace}, sync::atomic::Ordering, ptr, marker::PhantomData};


pub type Blob = *mut u8;

#[derive(Default)]
pub struct BlobVec {
    vec: Vec<u8>,
    blob_size: u32, // 每个条目的内存大小
    drop_fn: Option<fn(*mut u8)>,
}

impl BlobVec {
    #[inline(always)]
    pub fn new(blob_size: u32, drop_fn: Option<fn(*mut u8)>) -> Self {
        Self {
            vec: Default::default(),
            blob_size,
            drop_fn
        }
    }
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.vec.is_empty()
    }
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.vec.len() / self.blob_size as usize
    }
    #[inline(always)]
    pub fn get(&self, index: usize) -> Blob {
        self.vec
            .get(index * self.blob_size as usize)
            .map_or(ptr::null_mut(), |r| unsafe {transmute(r) })
    }
    #[inline(always)]
    pub unsafe fn get_unchecked(&self, index: usize) -> Blob {
        transmute(self.vec.get_unchecked(index * self.blob_size as usize))
    }
    #[inline(always)]
    pub fn alloc(&mut self) -> Blob {
        let len = self.len();
        unsafe {
            // let vec: &mut Vec<u8> = transmute(&self.vec as *const Vec<u8>);
            self.vec.reserve(self.blob_size as usize);
            self.vec.set_len(len + self.blob_size as usize);
            transmute(self.vec.get_unchecked(len))
        }
    }
}

impl Debug for BlobVec {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("BlobVec")
            .field("len", &self.vec.len())
            .field("blob_size", &self.blob_size)
            .finish()
    }
}

fn initialize(ptr: *mut u8, _size: usize, len: usize) {
    unsafe { std::ptr::write_bytes(ptr, 0, len) };
}