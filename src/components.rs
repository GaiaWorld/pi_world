
use core::fmt::*;
use std::{mem::{size_of, transmute, ManuallyDrop, MaybeUninit, replace}, sync::atomic::Ordering, ptr, marker::PhantomData};

use pi_null::Null;
use pi_share::ShareUsize;

use crate::{world::*, archetype::{MemOffset, Row}};

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
    #[inline(always)]
    fn set_null(&self) -> bool {
        let t: &ShareUsize = unsafe { transmute(*self) };
        if t.load(Ordering::Relaxed).is_null() {
            return false
        }
        t.store(usize::null(), Ordering::Relaxed);
        true
    }
    #[inline(always)]
    fn get_tick(&self) -> Tick {
        let t: &ShareUsize = unsafe { transmute(*self) };
        t.load(Ordering::Acquire)
    }
    #[inline(always)]
    fn set_tick(&self, tick: Tick) {
        let t: &ShareUsize = unsafe { transmute(*self) };
        t.store(tick, Ordering::Release)
    }
    #[inline(always)]
    fn entity(&self) -> &mut Entity {
        unsafe { transmute(self.add(size_of::<Tick>())) }
    }
    #[inline(always)]
    fn get<'a, T>(&self, offset: MemOffset) -> &'a T {
        unsafe { transmute(self.add(offset as usize)) }
    }
    #[inline(always)]
    fn get_mut<'a, T>(&mut self, offset: MemOffset) -> &'a mut T {
        unsafe { transmute(self.add(offset as usize)) }
    }
    #[inline(always)]
    fn init_component<'a, T>(&mut self, offset: MemOffset) -> &'a mut MaybeUninit<T> {
        unsafe { transmute(self.add(offset as usize)) }
    }
    #[inline(always)]
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
pub struct ComponentsVec {
    vec: Vec<u8>,
    components_size: u32, // 每个条目的内存大小
}

impl ComponentsVec {
    #[inline(always)]
    pub fn new(components_size: u32) -> Self {
        Self {
            vec: Default::default(),
            components_size: components_size,
        }
    }
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.vec.is_empty()
    }
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.vec.len()
    }
    #[inline(always)]
    pub fn get(&self, index: Row) -> ArchetypeData {
        self.vec
            .get(index as usize * self.components_size as usize)
            .map_or(ptr::null_mut(), |r| unsafe {transmute(r) })
    }
    #[inline(always)]
    pub unsafe fn get_unchecked(&self, index: Row) -> ArchetypeData {
        transmute(self.vec.get_unchecked(index as usize * self.components_size as usize))
    }
    #[inline(always)]
    pub fn alloc(&self) -> (Row, ArchetypeData) {
        let len = self.len();
        unsafe {
            let vec: &mut Vec<u8> = transmute(&self.vec as *const Vec<u8>);
            vec.reserve(self.components_size as usize);
            vec.set_len(len + self.components_size as usize);
            (len as u32 /self.components_size, transmute(self.vec.get_unchecked(len)))
        }
    }
    #[inline(always)]
    pub fn iter(&self) -> ComponentsIter {
        let vec: &mut Vec<u8> = unsafe { transmute(&self.vec as *const Vec<u8>) };
        ComponentsIter {
            arr: vec.as_mut(),
            components_size: self.components_size as usize,
            index: 0,
            len: self.vec.len() / self.components_size as usize,
            offset: 0,
            // ptr: unsafe { transmute(self.vec.get_unchecked(0)) },
            _k: PhantomData,
        }
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

impl Debug for ComponentsVec {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("ComponentsVec")
            .field("len", &self.vec.len())
            .field("components_size", &self.components_size)
            .finish()
    }
}

pub struct ComponentsIter<'a> {
    arr: &'a mut [u8], // u8数组
    components_size: usize, // 每个条目的内存大小
    index: usize,
    len: usize,
    offset: usize,
    // ptr: *mut u8,
    _k: PhantomData<fn(a: &'a u8)>,
}
impl<'a> ComponentsIter<'a> {
    #[inline(always)]
    pub fn empty() -> Self {
        ComponentsIter {
            arr: [].as_mut(),
            components_size: 0,
            index: 0,
            len: 0,
            offset: 0,
            // ptr: ptr::null_mut(),
            _k: PhantomData,
        }
    }
    #[inline(always)]
    pub fn components_size(&self) -> usize {
        self.components_size
    }
    #[inline(always)]
    pub fn index(&self) -> usize {
        self.index
    }
    // #[inline(always)]
    // pub(crate) fn step(&mut self) -> &'a mut u8 {
    //     unsafe {
    //         let ptr = self.ptr.add(self.components_size);
    //         transmute(replace(&mut self.ptr, ptr))
    //     }
    // }
}
impl<'a> Iterator for ComponentsIter<'a> {
    type Item = &'a mut u8;
    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.len {
            self.index += 1;
            let p = unsafe { self.arr.get_unchecked_mut(self.offset) };
            self.offset += self.components_size;
            return Some(unsafe { transmute(p) });
        }
        None
    }
    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.len.saturating_sub(self.index);
        return (n, Some(n));
    }
}

fn initialize(ptr: *mut u8, _size: usize, len: usize) {
    unsafe { std::ptr::write_bytes(ptr, 0, len) };
}