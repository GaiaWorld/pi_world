
use core::fmt::*;
use std::{mem::{size_of, transmute, ManuallyDrop, MaybeUninit}, sync::atomic::Ordering, ptr};

use pi_arr::{RawArr, RawIter};
use pi_null::Null;
use pi_share::ShareUsize;

use crate::{world::*, archetype::{MemOffset, ComponentInfo}, record::ComponentRecord};

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


pub struct Component {
    info: ComponentInfo,
    cur: BlobVec, // 当前的组件容器
    cur_ticks: Vec<usize>, // 当前的组件修改tick的容器
    add: SyncBlobVec, // 本帧内被Mutate添加的线程安全的组件容器
    cur_ticks: Vec<usize>, // add的组件修改tick的容器，也需要线程安全
    record: ComponentRecord, // 组件的记录，
}

impl Component {
    #[inline(always)]
    pub fn new(components_size: usize) -> Self {
        Self {
            arr: RawArr::default(),
            len: ShareUsize::new(0),
            components_size: components_size,
        }
    }
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len.load(Ordering::Relaxed) == 0
    }
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len.load(Ordering::Relaxed)
    }
    #[inline(always)]
    pub fn get(&self, index: usize) -> ArchetypeData {
        let len = self.len.load(Ordering::Relaxed);
        if index >= len {
            return ptr::null_mut();
        }
        self.arr
            .get(index, self.components_size)
            .map_or(ptr::null_mut(), |r| unsafe {transmute(r) })
    }
    #[inline(always)]
    pub unsafe fn get_unchecked(&self, index: usize) -> ArchetypeData {
        self.arr.get_unchecked(index, self.components_size)
    }
    #[inline(always)]
    pub fn alloc(&self) -> (usize, ArchetypeData) {
        let len = self.len.fetch_add(1, Ordering::AcqRel);
        unsafe {
            (len, self.arr.load_alloc(len, initialize, self.components_size))
        }
    }
    #[inline(always)]
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

impl Debug for Component {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("ComponentsVec")
            .field("len", &self.len())
            .finish()
    }
}
