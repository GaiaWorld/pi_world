use core::fmt::*;
use std::mem::{needs_drop, take, transmute, MaybeUninit};
use std::ops::{Index, IndexMut, Range};
use std::sync::atomic::Ordering;

use pi_append_vec::AppendVec;
use pi_arr::Iter;
use pi_share::ShareUsize;

#[derive(Debug)]
pub struct SafeVec<T> {
    vec: AppendVec<MaybeUninit<T>>,
    len: ShareUsize,
}
impl<T> SafeVec<T> {
    #[inline(always)]
    pub fn with_capacity(capacity: usize) -> Self {
        let vec = AppendVec::with_capacity(capacity);
        Self {
            vec,
            len: ShareUsize::new(0),
        }
    }
    /// 长度
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len.load(Ordering::Acquire)
    }
    #[inline(always)]
    pub fn get(&self, index: usize) -> Option<&T> {
        let len = self.len();
        if index >= len {
            return None;
        }
        self.vec.get(index).map(|r| unsafe { &*r.as_ptr() })
    }
    #[inline(always)]
    pub unsafe fn get_unchecked(&self, index: usize) -> &T {
        &*self.vec.get_unchecked(index).as_ptr()
    }
    #[inline(always)]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        let len = self.len();
        if index >= len {
            return None;
        }
        self.vec
            .get_mut(index)
            .map(|r| unsafe { &mut *r.as_mut_ptr() })
    }
    #[inline(always)]
    pub unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut T {
        &mut *self.vec.get_unchecked_mut(index).as_mut_ptr()
    }
    #[inline(always)]
    pub fn load(&self, index: usize) -> Option<&mut T> {
        self.vec.load(index).map(|r| unsafe { &mut *r.as_mut_ptr() })
    }
    #[inline(always)]
    pub unsafe fn load_unchecked(&self, index: usize) -> &mut T {
        &mut *self.vec.load_unchecked(index).as_mut_ptr()
    }

    #[inline(always)]
    pub fn insert(&self, value: T) -> usize {
        let index = self.vec.alloc_index(1);
        *self.vec.load_alloc(index) = MaybeUninit::new(value);
        while self
            .len
            .compare_exchange(index, index + 1, Ordering::Release, Ordering::Relaxed)
            .is_err()
        {
            std::hint::spin_loop();
        }
        index
    }
    #[inline(always)]
    pub fn alloc_entry<'a>(&'a self) -> Entry<'a, T> {
        let index = self.vec.alloc_index(1);
        Entry {
            index,
            len: &self.len,
            value: self.vec.load_alloc(index),
        }
    }
    #[inline(always)]
    pub fn iter(&self) -> SafeVecIter<'_, T> {
        SafeVecIter(self.vec.slice(0..self.len()))
    }
    #[inline(always)]
    pub fn slice(&self, range: Range<usize>) -> SafeVecIter<'_, T> {
        SafeVecIter(self.vec.slice(range))
    }
    #[inline(always)]
    pub fn collect(&mut self) {
        self.vec.collect();
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        let len = take(self.len.get_mut());
        if len == 0 {
            return;
        }
        if needs_drop::<T>() {
            for i in self.vec.iter() {
                unsafe { i.assume_init_drop() }
            }
        }
        self.vec.clear();
    }
}
impl<T> Index<usize> for SafeVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect("no element found at index {index}")
    }
}
impl<T> IndexMut<usize> for SafeVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_mut(index)
            .expect("no element found at index_mut {index}")
    }
}
impl<T> Drop for SafeVec<T> {
    fn drop(&mut self) {
        if needs_drop::<T>() {
            for i in self.vec.iter() {
                unsafe { i.assume_init_drop() }
            }
        }
    }
}
impl<T> Default for SafeVec<T> {
    fn default() -> Self {
        SafeVec {
            vec: Default::default(),
            len: ShareUsize::new(0),
        }
    }
}

pub struct SafeVecIter<'a, T>(Iter<'a, MaybeUninit<T>>);
impl<'a, T> Iterator for SafeVecIter<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|r| unsafe { transmute(r.as_ptr()) })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub struct Entry<'a, T> {
    index: usize,
    len: &'a ShareUsize,
    value: &'a mut MaybeUninit<T>,
}
impl<'a, T> Entry<'_, T> {
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn insert(self, value: T) {
        *self.value = MaybeUninit::new(value);
    }
}
impl<'a, T> Drop for Entry<'_, T> {
    fn drop(&mut self) {
        while self
            .len
            .compare_exchange(
                self.index,
                self.index + 1,
                Ordering::Release,
                Ordering::Relaxed,
            )
            .is_err()
        {
            std::hint::spin_loop();
        }
    }
}
