
use core::fmt::*;
use std::{mem::{size_of, transmute, ManuallyDrop, MaybeUninit}, sync::atomic::Ordering, ptr};

use pi_arr::{RawArr, RawIter};
use pi_share::ShareUsize;

use crate::{world::*, archetype::MemOffset};

pub type ArchetypeData = *mut u8;

pub trait ArchetypePtr {
    fn set_null(&self) -> bool;
    fn get_tick(&self) -> Tick;
    fn set_tick(&self, tick: Tick);
    fn entity(&self) -> &mut Entity;
    fn get<'a, T>(&self, offset: u32) -> &'a T;
    fn get_mut<'a, T>(&mut self, offset: u32) -> &'a mut T;
    /// 直接使用类型来初始化对象
    fn init_component<'a, T>(&mut self, offset: u32) -> &'a mut MaybeUninit<T>;
    /// 直接使用类型来释放对象
    fn drop_component<T>(&mut self, offset: u32);
}
impl ArchetypePtr for *mut u8 {
    fn set_null(&self) -> bool {
        let t: &ShareUsize = unsafe { transmute(*self) };
        if t.load(Ordering::Relaxed) == 0 {
            return false
        }
        t.store(0, Ordering::Release);
        true
    }

    fn get_tick(&self) -> Tick {
        let t: &ShareUsize = unsafe { transmute(*self) };
        t.load(Ordering::Acquire)
    }
    fn set_tick(&self, tick: Tick) {
        let t: &ShareUsize = unsafe { transmute(*self) };
        t.store(tick, Ordering::Release)
    }

    fn entity(&self) -> &mut Entity {
        unsafe { transmute(self.add(size_of::<Tick>())) }
    }
    
    fn get<'a, T>(&self, offset: MemOffset) -> &'a T {
        unsafe { transmute(self.add(offset as usize)) }
    }

    fn get_mut<'a, T>(&mut self, offset: MemOffset) -> &'a mut T {
        unsafe { transmute(self.add(offset as usize)) }
    }

    fn init_component<'a, T>(&mut self, offset: MemOffset) -> &'a mut MaybeUninit<T> {
        unsafe { transmute(self.add(offset as usize)) }
    }

    fn drop_component<T>(&mut self, offset: MemOffset) {
        unsafe {
            let item: &mut ManuallyDrop<T> = transmute(self.add(offset as usize));
            ManuallyDrop::<T>::drop(item);
        };
    }
}

// #[derive(Debug, Clone)]
// pub struct Ptr(NonNull<u8>);
// impl Ptr {
//     pub fn tick(&self) -> &mut Tick {
//         unsafe { transmute(self.0) }
//     }
//     pub fn entity(&self) -> &Entity {
//         unsafe { transmute(self.raw(size_of::<u32>())) }
//     }
//     pub fn get<T>(&self, offset: usize) -> &T {
//         unsafe { transmute(self.raw(offset)) }
//     }
//     pub fn get_mut<T>(&mut self, offset: usize) -> &mut T {
//         unsafe { transmute(self.raw_mut(offset)) }
//     }
//     /// 直接使用类型来初始化对象
//     pub fn init_component<T>(&mut self, offset: usize) -> &mut MaybeUninit<T> {
//         unsafe { transmute(self.raw_mut(offset)) }
//     }
//     /// 直接使用类型来释放对象
//     pub fn drop_component<T>(&mut self, offset: usize) {
//         unsafe {
//             let item: &mut ManuallyDrop<T> = transmute(self.raw_mut(offset));
//             ManuallyDrop::<T>::drop(item);
//         };
//     }
//     pub unsafe fn raw(&self, offset: usize) -> *const u8 {
//         (self.0.as_ref() as *const u8).add(offset)
//     }
//     pub unsafe fn raw_mut(&mut self, offset: usize) -> *mut u8 {
//         (self.0.as_mut() as *mut u8).add(offset)
//     }
// }

#[derive(Default)]
pub struct RawVec {
    arr: RawArr,
    len: ShareUsize,
    components_size: usize, // 每个条目的内存大小
}

impl RawVec {
    pub fn new(components_size: usize) -> Self {
        Self {
            arr: RawArr::default(),
            len: ShareUsize::new(0),
            components_size: components_size,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len.load(Ordering::Relaxed) == 0
    }
    pub fn len(&self) -> usize {
        self.len.load(Ordering::Relaxed)
    }
    
    pub fn get(&self, index: usize) -> ArchetypeData {
        let len = self.len.load(Ordering::Relaxed);
        if index >= len {
            return ptr::null_mut();
        }
        self.arr
            .get(index, self.components_size)
            .map_or(ptr::null_mut(), |r| unsafe {transmute(r) })
    }
    pub unsafe fn get_unchecked(&self, index: usize) -> ArchetypeData {
        self.arr.get_unchecked(index, self.components_size)
    }
    pub fn alloc(&self) -> (usize, ArchetypeData) {
        let len = self.len.fetch_add(1, Ordering::AcqRel);
        unsafe {
            (len, self.arr.load_alloc(len, initialize, self.components_size))
        }
    }
    pub fn iter(&self) -> RawIter {
        self.arr.slice(0..self.len(), self.components_size)
    }
    // pub fn remove(&mut self, index: usize) -> Option<(usize, ArchetypeData)> {
    //     let len = self.len.load(Ordering::Relaxed);
    //     let offset = index * self.components_size;
    //     if offset >= len {
    //         return None;
    //     }
    //     let prev_len = self.arr.len() - self.components_size;
    //     if offset >= prev_len {
    //         unsafe { self.arr.set_len(prev_len) };
    //         return None;
    //     }
    //     let ptr = self.arr.as_mut_ptr();
    //     unsafe {
    //         let dest = ptr.add(offset);
    //         ptr.add(prev_len).copy_to(dest, self.components_size);
    //         self.arr.set_len(prev_len);
    //         Some((index, dest))
    //     }
    // }
}

impl Debug for RawVec {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("RawVec")
            .field("len", &self.len())
            .field("components_size", &self.components_size)
            .finish()
    }
}

fn initialize(ptr: *mut u8, _size: usize, len: usize) {
    unsafe { std::ptr::write_bytes(ptr, 0, len) };
}