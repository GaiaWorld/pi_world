

use core::fmt::*;
use std::any::TypeId;
use std::marker::PhantomData;
use std::mem::{needs_drop, size_of, transmute};
use std::ops::Range;
use std::ptr::{copy, null_mut, write};

use pi_arr::*;
use pi_key_alloter::*;
use pi_null::Null;
use pi_phf_map::PhfMap;

new_key_type! {
    pub struct ArchetypeKey;
}

/// Thread-safe archetype
// #[derive(Default)]
pub struct Archetype {
    id: u128,
    arr: RawArr,
    alloter: KeyAlloter,
    vec: Vec<TypeInfo>,
    map: PhfMap<TypeId, TypeInfo>,
    item_size: usize,
}

impl Archetype {
    /// Creates an [`Archetype`] with the given TypeId and type size and a custom key
    /// type.
    ///
    /// # Examples
    ///
    /// ```
    /// # use pi_world::archetype::*;
    /// new_key_type! {
    ///     struct MessageKey;
    /// }
    /// let vec = vec![
    ///     (TypeId::of::<u8>(), size_of::<u8>()),
    ///     (TypeId::of::<Arc<i32>>(), size_of::<Arc<i32>>()),
    /// ];
    /// let ar = Archetype::new(vec);
    /// let offset = size_of::<u8>();
    /// let (k, ptr) = ar.insert();
    ///  unsafe { copy(&2u8, ptr, 1) };
    ///  unsafe { copy(&Arc::new(1u32), ptr.add(offset) as *mut Arc<i32>, 1) };
    /// let ptr = ar.get(k);
    /// let t10: &u8 = unsafe { std::mem::transmute(ptr) };
    /// let t11: &Arc<i32> = unsafe { std::mem::transmute(p1.add(offset)) };
    ///
    /// assert_eq!(t10, &2);
    /// assert_eq!(t11, &Arc::new(1));
    /// assert_eq!(Arc::<i32>::strong_count(&t11), 0);
    /// ```
    pub fn new<T: Iterator<Item = TypeInfo>>(components: T) -> Self {
        let mut id = 0;
        let mut item_size = 0;
        let mut vec = Vec::with_capacity(components.size_hint().0);
        let mut vec1 = Vec::with_capacity(vec.capacity());
        for mut i in components {
            id ^= unsafe { transmute::<TypeId, u128>(i.type_id) };
            i.offset = item_size as u32;
            item_size += i.type_size as usize;
            vec.push(i);
            vec1.push((i.type_id, i));
        }
        Self {
            id,
            arr: RawArr::with_capacity(0, Self::initialize, item_size),
            alloter: KeyAlloter::new(0),
            vec,
            map: PhfMap::new(vec1),
            item_size,
        }
    }
    /// Returns the id of the archetype.
    ///
    /// # Examples
    ///
    /// ```
    /// # use pi_world::archetype::*;
    /// let vec = vec![
    ///     (TypeId::of::<u8>(), size_of::<u8>()),
    ///     (TypeId::of::<Arc<i32>>(), size_of::<Arc<i32>>()),
    /// ];
    /// let ar = Archetype::new(vec);
    /// assert_eq!(ar.id(), 136982586060323025009695824984285423444);
    /// ```
    pub fn get_id(&self) -> &u128 {
        &self.id
    }

    pub fn get_type_infos(&self) -> &Vec<TypeInfo> {
        &self.vec
    }
    pub fn get_type_info(&self, type_id: &TypeId) -> &TypeInfo {
        self.map.get(&type_id)
    }

    /// Returns the number of elements in the archetype.
    pub fn len(&self) -> usize {
        self.alloter.len()
    }
    /// Returns the max index in the archetype.
    pub fn max(&self) -> u32 {
        self.alloter.max()
    }
    /// Returns if the archetype is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// 分配一个Key, 后面要求一定要用alloc_value设置Value，否则remove时回收Key会失败，另外，再没有插入数据期间，如果进行迭代，也是没有该key的
    pub unsafe fn alloc_key(&self) -> ArchetypeKey {
        self.alloter.alloc(2).into()
    }
    pub unsafe fn alloc_value(&self, k: ArchetypeKey) -> *mut u8 {
        let kd = k.data();
        let ptr = self
            .arr
            .load_alloc(kd.index() as usize, Self::initialize, self.item_size);
        Self::update(kd, Slot(ptr))
    }
    /// Returns [`true`] if the archetype contains `key`.
    pub fn contains_key(&self, k: ArchetypeKey) -> bool {
        let kd = k.data();
        if let Some(r) = self.arr.get(kd.index() as usize, self.item_size) {
            let s = Slot(unsafe { transmute(r) });
            return *s.version() == kd.version();
        }
        false
    }

    pub fn alloc(&self) -> (ArchetypeKey, *mut u8) {
        let k = unsafe { self.alloc_key() };
        let kd = k.data();
        let e = unsafe {
            self.arr
                .load_alloc(kd.index() as usize, Self::initialize, self.item_size)
        };
        (k, Self::update(kd, Slot(e)))
    }
    /// Removes a key from the archetype, returning the value at the key if the
    /// key was not previously removed.
    pub fn remove(&self, k: ArchetypeKey) -> bool {
        let kd = k.data();
        if let Some(r) = self.arr.get(kd.index() as usize, self.item_size) {
            let s = Slot(unsafe { transmute(r) });
            if *s.version() == kd.version() {
                *s.version() += 1;
                self.alloter.recycle(k.data());
                self.drop_item(s.value());
                return true;
            }
        }
        false
    }
    /// Returns a reference to the value corresponding to the key.
    pub fn get(&self, k: ArchetypeKey) -> *mut u8 {
        let kd = k.data();
        if let Some(r) = self.arr.get(kd.index() as usize, self.item_size) {
            let s = Slot(unsafe { transmute(r) });
            if *s.version() == kd.version() {
                return s.value();
            }
        }
        null_mut()
    }

    /// Returns a reference to the value corresponding to the key without
    /// version or bounds checking.
    pub unsafe fn get_unchecked(&self, k: ArchetypeKey) -> *mut u8 {
        self.arr
            .get_unchecked(k.data().index() as usize, self.item_size)
            .add(size_of::<u32>())
    }

    /// Returns a mutable reference to the value corresponding to the key.
    pub fn get_mut(&mut self, k: ArchetypeKey) -> *mut u8 {
        let kd = k.data();
        if let Some(r) = self.arr.get(kd.index() as usize, self.item_size) {
            let s = Slot(unsafe { transmute(r) });
            if *s.version() == kd.version() {
                return s.value();
            }
        }
        null_mut()
    }
    /// Inserts a value into the archetype. Returns a unique key that can be used
    /// to access this value.
    pub fn set(&mut self) -> (ArchetypeKey, *mut u8) {
        let k = unsafe { self.alloc_key() };
        let kd = k.data();
        let e = unsafe {
            self.arr
                .get_alloc(kd.index() as usize, Self::initialize, self.item_size)
        };
        (k, Self::update(kd, Slot(e)))
    }
    fn update(kd: KeyData, slot: Slot) -> *mut u8 {
        if is_older_version(kd.version(), *slot.version()) {
            return null_mut();
        }
        *slot.version() = kd.version();
        slot.value()
    }
    /// An iterator visiting all key-value pairs in arbitrary order. The
    /// iterator element type is `(K, *mut u8)`.
    ///
    /// This function must iterate over all slots, empty or not. In the face of
    /// many deleted elements it can be inefficient.
    ///
    pub fn iter(&self) -> Iter<'_, ArchetypeKey> {
        self.slice(0..self.alloter.max() as usize)
    }
    /// Returns an iterator over the array at the given range.
    ///
    /// Values are yielded in the form `(K, *mut u8)`.
    pub fn slice(&self, range: Range<usize>) -> Iter<'_, ArchetypeKey> {
        Iter {
            iter: self.arr.slice(range, self.item_size),
            len: self.len(),
            _k: PhantomData,
        }
    }
    /// 整理方法
    pub fn collect_key(&self) -> Drain {
        self.alloter.collect(2)
    }
    /// 整理方法
    pub unsafe fn collect_value(&self, tail: u32, free: KeyData) {
        let e = Slot(self.arr.get_unchecked(tail as usize, self.item_size));
        *e.version() = 1;
        let hole = Slot(
            self.arr
                .get_unchecked(free.index() as usize, self.item_size),
        );
        *hole.version() = free.version();
        copy(hole.value(), e.value(), self.item_size);
    }
    #[inline]
    fn initialize(ptr: *mut u8, type_size: usize, len: usize) {
        let mut index = 0;
        while index < len {
            unsafe {
                let p = ptr.add(index) as *mut u32;
                write(p, 1);
            }
            index += type_size;
        }
    }
    fn drop_item(&self, ptr: *mut u8) {
        for t in self.vec.iter() {
            if let Some(d) = t.drop_fn {
                d(unsafe { ptr.add(t.offset as usize) });
            }
        }
    }
}

impl Debug for Archetype {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("Archetype")
            .field("id", &self.id)
            .field("alloter", &self.alloter)
            .field("map", &self.map)
            .field("item_size", &self.item_size)
            .finish()
    }
}

impl Drop for Archetype {
    fn drop(&mut self) {
        for (_, ptr) in self.iter() {
            self.drop_item(ptr);
        }
        // free memory
        for (entries, mut len) in self.arr.replace().into_iter() {
            len *= self.item_size;
            unsafe { drop(Vec::from_raw_parts(entries, len, len)) }
        }
    }
}
struct Slot(*mut u8);
impl Slot {
    fn version(&self) -> &mut u32 {
        unsafe { &mut *(self.0 as *mut u32) }
    }
    fn value(&self) -> *mut u8 {
        unsafe { self.0.add(size_of::<u32>()) }
    }
    fn is_null(&self) -> bool {
        *self.version() & 1 == 1
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TypeInfo {
    offset: u32,
    pub type_size: u32,
    pub type_id: TypeId,
    pub drop_fn: Option<fn(*mut u8)>,
}
impl TypeInfo {
    pub fn new(type_size: usize, type_id: TypeId, drop_fn: Option<fn(*mut u8)>) -> Self {
        TypeInfo {
            offset: Null::null(),
            type_size: type_size as u32,
            type_id,
            drop_fn,
        }
    }
}
impl Null for TypeInfo {
    fn null() -> Self {
        TypeInfo {
            offset: Null::null(),
            type_size: Null::null(),
            type_id: Null::null(),
            drop_fn: None,
        }
    }

    fn is_null(&self) -> bool {
        self.offset.is_null()
    }
}
pub struct Iter<'a, K: Key> {
    iter: pi_arr::RawIter<'a>,
    len: usize,
    _k: PhantomData<fn(K) -> K>,
}
impl<'a, K: Key> Iterator for Iter<'a, K> {
    type Item = (K, *mut u8);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(r) = self.iter.next() {
            let s = Slot(r.1);
            if !s.is_null() {
                let ffi = (u64::from(*s.version()) << 32) | u64::from(r.0 as u32);
                return Some((KeyData::from_ffi(ffi).into(), s.value()));
            }
        }
        None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}
/// 获得指定类型的释放函数
pub fn get_drop<T>() -> Option<fn(*mut u8)> {
    needs_drop::<T>().then_some(|ptr: *mut u8| unsafe { (ptr as *mut T).drop_in_place() })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::archetype::*;

    #[test]
    fn test() {
        
        let vec = vec![
            TypeInfo::new(size_of::<u8>(), TypeId::of::<u8>(), None),
            TypeInfo::new(size_of::<Arc<i32>>(), TypeId::of::<Arc<i32>>(), get_drop::<Arc<i32>>()),
        ];
        let offset = size_of::<u8>();
        let ar = Archetype::new(vec.into_iter());
        let (k1, ptr) = ar.alloc();
        let arc = Arc::new(1);
        {
            let t1 = (2u8, arc.clone());

            unsafe { copy(&t1.0, ptr, 1) };
            unsafe { copy(&t1.1, ptr.add(offset) as *mut Arc<i32>, 1) };

            println!("k:{:?} sizeof:{}", k1, size_of::<(u8, Arc<i32>)>());
            std::mem::forget(t1);
        }
        println!("strong_count1: {:?}", Arc::<i32>::strong_count(&arc));
        {
            let p1 = ar.get(k1);
            let t10: &u8 = unsafe { std::mem::transmute(p1) };
            let t11: &Arc<i32> = unsafe { std::mem::transmute(p1.add(offset)) };
            println!("strong_count2: {:?}", Arc::<i32>::strong_count(&t11));
            assert_eq!(t10, &2);
            assert_eq!(t11, &Arc::new(1));
        }
        ar.remove(k1);
        println!("strong_count3: {:?}", Arc::<i32>::strong_count(&arc));
        println!("{:?}", ar);
    }
}
