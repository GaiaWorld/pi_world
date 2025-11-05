#[cfg(not(feature = "rc"))]
use pi_buckets::Location;

use pi_buckets::{Buckets, BUCKETS};
use std::cell::UnsafeCell;
use std::mem::replace;
use std::ptr::NonNull;
use std::ops::Range;
use pi_vec_remain::VecRemain;
use pi_null::Null;
#[cfg(feature = "rc")]
pub type Blob = VecBlob;

#[cfg(not(feature = "rc"))]
pub type Blob = BucketsBlob;

// #[cfg(not(feature = "rc"))]
// pub type Iter<'a, T> = BucketIter<'a, T>;

unsafe impl Send for Blob {}
unsafe impl Sync for Blob {}

impl Blob {
    pub fn memsize(&self) -> usize {
        let val = self.capacity(0);
        if val >= std::usize::MAX - 1 {
            24
        } else {
            val + 24
        }
    }
}

pub struct BucketsBlob {
    ptr: *mut u8,
    capacity: usize,
    buckets: *mut Buckets<u8>,
}


#[cfg(not(feature = "rc"))]
impl Default for BucketsBlob {
    fn default() -> Self {
        // println!("=========== BucketsBlob");
        let buckets = Box::into_raw(Box::new(Buckets::default()));
        let ptr = NonNull::<u8>::dangling().as_ptr();
        Self {
            ptr,
            capacity: usize::null(),
            buckets,
        }
    }
}

#[cfg(not(feature = "rc"))]
impl BucketsBlob {
    #[inline(always)]
    pub fn vec_capacity(&self) -> usize {
        self.capacity
    }

    #[inline(always)]
    pub fn capacity(&self, len: usize) -> usize {
        if len > self.capacity {
            Location::bucket_capacity(Location::bucket(len - self.capacity)) + self.capacity
        } else {
            self.capacity
        }
    }

    #[inline(always)]
    fn buckets(&self) -> &Buckets<u8> {
        unsafe { &*self.buckets }
    }

    #[inline(always)]
    pub unsafe fn set_vec_capacity(&mut self, capacity: usize) {
        self.capacity = capacity;
    }

    #[inline]
    pub fn get_multiple(&self, index: usize, multiple: usize) -> Option<&mut u8> {
        if index < self.vec_capacity() {
            return Some(unsafe { &mut *self.ptr.add(index * multiple) });
        }
        let mut loc = Location::of(index - self.capacity);
        loc.entry *= multiple;
        self.buckets().load(&loc)
    }

    #[inline]
    pub fn get_multiple_unchecked(&self, index: usize, multiple: usize) -> &mut u8 {
        if index < self.vec_capacity() {
            return unsafe { &mut *self.ptr.add(index * multiple) };
        }
        let mut loc = Location::of(index - self.capacity);
        loc.entry *= multiple;
        unsafe { self.buckets().load_unchecked(&loc) }
    }

    #[inline]
    pub fn load_alloc_multiple(&self, index: usize, multiple: usize) -> &mut u8 {
        if index < self.vec_capacity() {
            return unsafe { &mut *self.ptr.add(index * multiple) };
        }
        let mut loc = Location::of(index - self.capacity);
        loc.entry *= multiple;
        loc.len *= multiple;
        self.buckets().load_alloc(&loc)
    }

    /// 整理内存，将bucket_arr的数据移到vec上，并将当前vec_capacity容量扩容len+additional
    pub fn settle(&mut self, len: usize, additional: usize, multiple: usize) {
        self.remain_settle(0..len, len, additional, multiple);
    }

    fn reset_vec(buckets: [Vec<u8>; BUCKETS], multiple: usize) -> [Vec<u8>; BUCKETS] {
        buckets.map(|vec| {
            let len = vec.len() * multiple;
            let ptr = vec.into_raw_parts().0;
            to_vec(ptr, len)
        })
    }

    fn reserve(&mut self, mut vec: Vec<u8>, len: usize, mut additional: usize, multiple: usize) {
        additional = (len + additional).saturating_sub(self.capacity);
        if additional > 0 {
            vec.reserve(additional * multiple);
            vec.resize_with(vec.capacity(), || u8::default());
            self.capacity = vec.capacity() / multiple;
        }
        self.ptr = vec.into_raw_parts().0;
    }

    fn take_buckets(&mut self, multiple: usize) -> [Vec<u8>; BUCKETS] {
        // 取出所有的bucket
        let mut arr = self.buckets().take();
        if multiple > 1 {
            arr = Self::reset_vec(arr, multiple);
        }
        arr
    }

    /// 保留范围内的数组元素并整理，将保留的部分整理到扩展槽中，并将当前vec_capacity容量扩容len+additional
    pub fn remain_settle(
        &mut self,
        range: Range<usize>,
        len: usize,
        additional: usize,
        multiple: usize
    ) {
        debug_assert!(len >= range.end);
        debug_assert!(range.end >= range.start);
        let mut vec = to_vec(self.ptr, self.capacity * multiple);
        if range.end <= self.capacity {
            // 数据都在vec上
            vec.remain(range.start * multiple..range.end * multiple);
            if len > self.capacity {
                self.take_buckets(multiple);
            }
            return self.reserve(vec, range.len(), additional, multiple);
        }
        // 取出所有的bucket
        let arr = self.take_buckets(multiple);
        // 获得扩容后的总容量
        let cap = Location::bucket_capacity(Location::bucket(range.len() + additional)) * multiple;
        let mut start = range.start * multiple;
        let end = range.end * multiple;
        let mut index = vec.capacity();
        if vec.capacity() >= cap {
            // 先将扩展槽的数据根据范围保留
            start += vec.remain(start..end);
        } else if vec.capacity() > 0 {
            let mut new = Vec::with_capacity(cap);
            // 将扩展槽的数据根据范围保留到新vec中
            start += vec.remain_to(start..end, &mut new);
            vec = new;
        }
        // 将arr的数据移到vec上
        for (i, mut v) in arr.into_iter().enumerate() {
            if start >= end {
                break;
            }
            let mut vlen = v.len();
            if vlen > 0 {
                if start >= index + vlen {
                    index += vlen;
                    continue;
                }
                if vec.capacity() == 0 {
                    // 如果原vec为empty
                    if v.capacity() >= cap {
                        // 并且当前容量大于等于cap，则直接将v换上
                        _ = replace(&mut vec, v);
                        vec.remain(start - index..end - index);
                    } else {
                        vec.reserve(cap);
                        v.remain_to(start - index..end - index, &mut vec);
                    }
                } else {
                    v.remain_to(start - index..end - index, &mut vec);
                }
            } else {
                vlen = Location::bucket_len(i) * multiple;
                if start >= index + vlen {
                    index += vlen;
                    continue;
                }
                if vec.capacity() == 0 {
                    vec.reserve(cap);
                }
                vec.resize_with(vec.len() + index + vlen - start, || u8::default());
            }
            index += vlen;
            start = index;
        }
        // 如果容量比len大，则初始化为null元素
        vec.resize_with(vec.capacity(), || u8::default());
        self.capacity = vec.capacity() / multiple;
        self.ptr = vec.into_raw_parts().0;
    }
}

#[cfg(not(feature = "rc"))]
impl Drop for BucketsBlob {
    fn drop(&mut self) {
        if self.vec_capacity().is_null() {
            unsafe { self.set_vec_capacity(0) };
        }

        to_vec(self.ptr, self.capacity);
        unsafe { drop(Box::from_raw(self.buckets)) };
    }
}

pub struct VecBlob {
    ptr: UnsafeCell<*mut u8>,
    capacity: UnsafeCell<usize>,
}
impl Default for VecBlob {
    fn default() -> Self {
        // println!("=========== VecBlob");
        Self {
            ptr: NonNull::<u8>::dangling().as_ptr().into(),
            capacity: usize::null().into(),
        }
    }
}

impl VecBlob {
    /// 获得容量大小
    #[inline(always)]
    pub fn capacity(&self, _len: usize) -> usize {
        *unsafe { self.capacity.as_ref_unchecked() }
    }
    #[inline(always)]
    pub unsafe fn set_vec_capacity(&mut self, capacity: usize) {
        self.capacity = capacity.into();
    }
    #[inline(always)]
    pub fn vec_capacity(&self) -> usize {
        if size_of::<u8>() == 0 {
            usize::MAX
        } else {
            *unsafe { self.capacity.as_ref_unchecked() }
        }
    }
    #[inline]
    pub fn get_multiple(&self, index: usize, multiple: usize) -> Option<&mut u8> {
        if index < self.vec_capacity() {
            return Some(unsafe { &mut *(*self.ptr.get()).add(index * multiple) });
        }
        None
    }
    #[inline]
    pub fn get_multiple_unchecked(&self, index: usize, multiple: usize) -> &mut u8 {
        debug_assert!(index < self.vec_capacity());
        unsafe { &mut *(*self.ptr.get()).add(index * multiple) }
    }
    #[inline]
    pub fn load_alloc_multiple(&self, index: usize, multiple: usize) -> &mut u8 {
        if index >= self.vec_capacity() {
            let vec = to_vec(unsafe { *self.ptr.get() }, self.vec_capacity() * multiple);
            self.reserve(
                vec,
                self.vec_capacity(),
                index - self.vec_capacity() + 1,
                multiple,
            );
        }
        return unsafe { &mut *(*self.ptr.get()).add(index * multiple) };
    }
    /// 整理内存，将bucket_arr的数据移到vec上，并将当前vec_capacity容量扩容len+additional
    pub fn settle(&mut self, _len: usize, _additional: usize, _multiple: usize) {}

    fn reserve(&self, mut vec: Vec<u8>, len: usize, mut additional: usize, multiple: usize) {
        additional = (len + additional).saturating_sub(self.vec_capacity());
        if additional > 0 {
            vec.reserve(additional * multiple);
            vec.resize_with(vec.capacity(), || u8::default());
            unsafe { self.capacity.replace(vec.capacity() / multiple) };
        }
        unsafe { self.ptr.replace(vec.into_raw_parts().0) };
    }
}
impl Drop for VecBlob {
    fn drop(&mut self) {
        if self.vec_capacity().is_null() {
            unsafe { self.set_vec_capacity(0) };
        }
        let len = unsafe {
            *self.capacity.as_ref_unchecked()
        };
        if len == usize::MAX {
            return;
        }
        to_vec(unsafe { *self.ptr.get() }, len);
    }
}

pub fn to_vec<T>(ptr: *mut T, len: usize) -> Vec<T> {
    unsafe { Vec::from_raw_parts(ptr, len, len) }
}