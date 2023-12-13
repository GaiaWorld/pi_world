/// 原型，存储了一组具有相同组件的entity。
/// 根据system对不同原型的读写依赖，可并行执行system
///
/// 在读写依赖分析后，没有原型变动时，一个原型同时只会被一个system 修改 标记删除 Insert添加。
/// 但由于存在动态增删组件，并且组件的组合数量爆炸问题，导致会出现没有提前计算依赖关系的原型，会因为动态增删组件而被操作。虽然这种操作只会是Mutate添加。
/// Mutate总是采用分配新条目的方式，用Cow方式保证老数据的引用安全。
/// system对原型有查找和遍历的需求，这个时候Mutate利用每条目上的tick的原子保护来保证安全。
/// Mutate在本地没有找到原型时，有2种情况，一种是找到已存在的原型，一种是没有原型要新创建原型。
/// 新创建原型，通过ready来保护，通知所有system添加脏列表，这样新增的条目，都会被记录脏。
/// 已存在的原型，是有可能正在被其他System读写和监听Change和Add的。
/// system监听Change和Add组件，是通过在原型上添加自己关心组件的脏列表来监听。
/// 有外部调度的读写分离，所以Change一定在单个system内操作的。 但考虑以后单system也可能开多future并行修改，而Archetype作为基础结构已经内置了ComponentRecord，所以ComponentRecord的flags和vec还是用线程安全的Arr及AppendVec。
/// Add由于有可能被其他system的mutate添加，所以脏的add_len来保证没有监听到的Add下次会监听。
///
/// 只有主调度完毕后，所有的脏都被处理和清理后，才进行整理，只有整理才会调整ArchetypeKey。在整理前，ArchetypeKey都是递增的。
///
use core::fmt::*;
use std::any::TypeId;
use std::borrow::Cow;
use std::mem::{needs_drop, size_of, transmute};
use std::sync::atomic::Ordering;

use pi_append_vec::AppendVec;
use pi_arr::RawIter;
use pi_null::Null;
use pi_phf_map::PhfMap;
use pi_share::{Share, ShareU8};
use smallvec::SmallVec;

use crate::record::{RecordIndex, ComponentRecord};
use crate::raw::*;
use crate::world::*;

pub type ShareArchetype = Share<Archetype>;

pub type ArchetypeKey = usize;
pub type WorldArchetypeIndex = u32;
pub type ComponentIndex = u32;
pub type MemOffset = u32;

/// Thread-safe archetype
pub struct Archetype {
    id: u128,
    arr: RawVec,
    components: Vec<ComponentInfo>,
    map: PhfMap<TypeId, ComponentIndex>,
    records: Vec<ComponentRecord>,      // 每组件对应记录的列表
    removes: AppendVec<ArchetypeKey>,      // 整理前被移除的实例
    index: WorldArchetypeIndex, // 在全局原型列表中的位置
    pub(crate) ready: ShareU8,           // 是否已就绪，脏列表是否已经被全部的system添加好了
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
    pub fn new(mut components: Vec<ComponentInfo>) -> Self {
        let mut id = 0;
        let mut components_size = size_of::<Tick>() + size_of::<Entity>();
        let mut vec1 = Vec::with_capacity(components.capacity());
        let mut records = Vec::new();
        for (i, info) in components.iter_mut().enumerate() {
            id ^= info.id();
            info.mem_offset = components_size as u32;
            components_size += info.mem_size as usize;
            records.push(Default::default());
            vec1.push((info.type_id, i as u32));
        }
        Self {
            id,
            arr: RawVec::new(components_size),
            components,
            map: PhfMap::new(vec1),
            records,
            removes: AppendVec::default(),
            index: u32::null(),
            ready: ShareU8::new(0),
        }
    }
    // 获得ready状态
    #[inline(always)]
    pub fn get_ready(&self) -> u8 {
        self.ready.load(Ordering::Relaxed)
    }
    // 原型突变， 在该原型下添加一些组件，删除一些组件，得到新原型需要包含哪些组件，及移动的组件
    pub fn mutate(
        &self,
        mut add: Vec<ComponentInfo>,
        mut del: Vec<TypeId>,
    ) -> (Vec<ComponentInfo>, Vec<TypeId>) {
        let len = add.len();
        add.sort();
        del.sort();
        for i in self.components.iter() {
            // 如果组件是要删除或要添加的组件，则不添加
            if del.binary_search(&i.type_id).is_err() && add[0..len].binary_search(i).is_err() {
                add.push(i.clone());
            }
        }
        del.clear();
        // 记录移动的组件
        for i in (len..add.len()).into_iter() {
            del.push(add[i].type_id);
        }
        (add, del)
    }
    // 根据监听列表，添加监听，该方法要么在初始化时调用，要么就是在原型刚创建时调用
    pub(crate) unsafe fn add_records(&self, owner: TypeId, listens: &SmallVec<[(TypeId, bool); 1]>) {
        for (tid, changed) in listens.iter() {
            let index = self.get_type_info_index(tid);
            if index.is_null() {
                continue;
            }
            let ptr: *mut ComponentRecord = transmute(self.get_component_record(index));
            let r: &mut ComponentRecord = transmute(ptr);
            r.insert(owner, *changed);
        }
    }
    // 根据监听列表，重新找到add_records前面放置的记录位置
    pub fn find_records(
        &self,
        owner: TypeId,
        listens: &SmallVec<[(TypeId, bool); 1]>,
        vec: &mut SmallVec<[RecordIndex; 1]>,
    ) {
        for (tid, changed) in listens.iter() {
            let index = self.get_type_info_index(tid);
            if index.is_null() {
                continue;
            }
            self.get_component_record(index).find(index, owner, *changed, vec);
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
    #[inline(always)]
    pub fn get_id(&self) -> &u128 {
        &self.id
    }
    #[inline(always)]
    pub fn get_index(&self) -> WorldArchetypeIndex {
        self.index
    }
    #[inline(always)]
    pub(crate) fn set_index(&self, index: WorldArchetypeIndex) {
        unsafe { *(&self.index as *const WorldArchetypeIndex as *mut WorldArchetypeIndex) = index }
    }
    #[inline(always)]
    pub fn get_type_infos(&self) -> &Vec<ComponentInfo> {
        &self.components
    }
    #[inline(always)]
    pub fn get_type_info_index(&self, type_id: &TypeId) -> ComponentIndex {
        if let Some(t) = self.map.get(&type_id) {
            if t.is_null() {
                return u32::null();
            }
            let ti = unsafe { self.components.get_unchecked(*t as usize) };
            if &ti.type_id == type_id {
                return *t;
            }
        }
        u32::null()
    }
    #[inline(always)]
    pub fn get_type_info(&self, type_id: &TypeId) -> Option<&ComponentInfo> {
        if let Some(t) = self.map.get(&type_id) {
            if t.is_null() {
                return None;
            }
            let t = unsafe { self.components.get_unchecked(*t as usize) };
            if &t.type_id == type_id {
                return Some(t);
            }
        }
        None
    }
    #[inline(always)]
    pub fn get_mem_offset_ti_index(&self, type_id: &TypeId) -> (MemOffset, ComponentIndex) {
        if let Some(t) = self.map.get(&type_id) {
            if t.is_null() {
                return (u32::null(), 0);
            }
            let ti = unsafe { self.components.get_unchecked(*t as usize) };
            if &ti.type_id == type_id {
                return (ti.mem_offset, *t);
            }
        }
        (u32::null(), 0)
    }
    #[inline(always)]
    pub unsafe fn get_type_info_unchecked(&self, type_id: &TypeId) -> &ComponentInfo {
        self.components
            .get_unchecked(*self.map.get_unchecked(&type_id) as usize)
    }
    #[inline(always)]
    pub(crate) fn get_records(&self) -> &Vec<ComponentRecord> {
        &self.records
    }
    #[inline(always)]
    pub(crate) fn get_component_record(&self, index: ComponentIndex) -> &ComponentRecord {
        unsafe {self.records.get_unchecked(index as usize)}
    }
    /// Returns the number of elements in the archetype.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.arr.len()
    }
    /// Returns the max index in the archetype.
    // pub fn max(&self) -> u32 {
    //     self.alloter.max()
    // }
    /// Returns if the archetype is empty.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.arr.is_empty()
    }
    /// 分配一个Key, 后面要求一定要用alloc_value设置Value，否则remove时回收Key会失败，另外，再没有插入数据期间，如果进行迭代，也是没有该key的
    // pub unsafe fn alloc_key(&self) -> ArchetypeKey {
    //     self.alloter.alloc(2, 2).into()
    // }
    // pub unsafe fn alloc_value(&self, k: ArchetypeKey) -> *mut u8 {
    //     let kd = k.data();
    //     let ptr = self
    //         .arr
    //         .load_alloc(kd.index() as usize, Self::initialize, self.components_size);
    //     Self::update(kd, Slot(ptr))
    // }
    /// Returns [`true`] if the archetype contains `key`.
    // pub fn contains_key(&self, k: ArchetypeKey) -> bool {
    //     if let Some(r) = self.arr.get(k as usize) {
    //         let s = Slot(unsafe { transmute(r) });
    //         return *s.version() == kd.version();
    //     }
    //     false
    // }
    #[inline(always)]
    pub fn alloc(&self) -> (ArchetypeKey, ArchetypeData) {
        self.arr.alloc()
    }
    /// mark removes a key from the archetype, returning the value at the key if the
    /// key was not previously removed.
    pub(crate) fn remove(&self, k: ArchetypeKey) -> bool {
        let ptr = self.arr.get(k);
        if !ptr.is_null() {
            if !ptr.set_null() {
                return false;
            }
            self.removes.insert(k);
            return true;
        }
        false
    }
    #[inline(always)]
    pub(crate) fn iter(&self) -> RawIter {
        self.arr.iter()
    }
    /// 只有主调度完毕后，所有的脏都被处理和清理后，才进行整理，只有整理才会调整ArchetypeKey。在整理前，ArchetypeKey都是递增的。
    // pub(crate) fn collect(&mut self, entitys: &SlotMap<Entity, EntityValue>) {
    //     // for k in self.removes.drain(..) {
    //     // if let Some((k, ptr)) = self.arr.remove(k) {
    //     //     let r = unsafe { entitys.load_unchecked(*ptr.entity()) };
    //     //     r.1 = k;
    //     //     r.2 = ptr;
    //     // }
    //     // }
    //     todo!() // 清理脏列表上的add
    // }

    /// Returns a reference to the value corresponding to the key without
    /// version or bounds checking.
    // pub unsafe fn get_unchecked(&self, k: ArchetypeKey) -> Ptr {
    //     self.arr.get_unchecked(k)
    // }

    /// Returns a mutable reference to the value corresponding to the key.
    #[inline(always)]
    pub fn get(&self, k: ArchetypeKey) -> ArchetypeData {
        self.arr.get(k)
    }
    /// Inserts a value into the archetype. Returns a unique key that can be used
    /// to access this value.
    // pub fn set(&mut self) -> (ArchetypeKey, *mut u8) {
    //     let k = unsafe { self.alloc_key() };
    //     let kd = k.data();
    //     let e = unsafe {
    //         self.arr
    //             .get_alloc(kd.index() as usize, Self::initialize, self.components_size)
    //     };
    //     (k, Self::update(kd, Slot(e)))
    // }
    // fn update(kd: KeyData, slot: Slot) -> *mut u8 {
    //     if is_older_version(kd.version(), *slot.version()) {
    //         return null_mut();
    //     }
    //     *slot.version() = kd.version();
    //     slot.value()
    // }
    /// An iterator visiting all key-value pairs in arbitrary order. The
    /// iterator element type is `(K, *mut u8)`.
    ///
    /// This function must iterate over all slots, empty or not. In the face of
    /// many deleted elements it can be inefficient.
    ///
    // pub fn iter(&self) -> Iter<'_, ArchetypeKey> {
    //     self.slice(0..self.alloter.max() as usize)
    // }
    /// Returns an iterator over the array at the given range.
    ///
    /// Values are yielded in the form `(K, *mut u8)`.
    // pub fn slice(&self, range: Range<usize>) -> Iter<'_, ArchetypeKey> {
    //     Iter {
    //         iter: self.arr.slice(range, self.components_size),
    //         len: self.len(),
    //         _k: PhantomData,
    //     }
    // }
    /// 整理方法
    // pub fn collect_key(&self) -> Drain {
    //     self.alloter.collect(2)
    // }
    /// 整理方法
    // pub unsafe fn collect_value(&self, tail: u32, free: KeyData) {
    //     let e = Slot(self.arr.get_unchecked(tail as usize, self.components_size));
    //     *e.version() = 1;
    //     let hole = Slot(
    //         self.arr
    //             .get_unchecked(free.index() as usize, self.components_size),
    //     );
    //     *hole.version() = free.version();
    //     copy(hole.value(), e.value(), self.components_size);
    // }
    // #[inline]
    // fn initialize(ptr: *mut u8, type_size: usize, len: usize) {
    //     let mut index = 0;
    //     while index < len {
    //         unsafe {
    //             let p = ptr.add(index) as *mut u32;
    //             write(p, 1);
    //         }
    //         index += type_size;
    //     }
    // }
    pub(crate) fn drop_key(&self, key: ArchetypeKey) {
        self.drop_item(unsafe { self.arr.get_unchecked(key) })
    }
    pub(crate) fn drop_item(&self, ptr: ArchetypeData) {
        for t in self.components.iter() {
            if let Some(d) = t.drop_fn {
                println!("drop_item:1, ptr:{:?},mem_offset:{}", ptr, t.mem_offset);
                d(unsafe { ptr.add(t.mem_offset as usize) });
            }
        }
    }
    pub(crate) fn drop_component(&self, ptr: ArchetypeData, index: MemOffset) {
        let t = &self.components[index as usize];
        if let Some(d) = t.drop_fn {
            println!("drop_item:1, ptr:{:?},mem_offset:{}", ptr, t.mem_offset);
            d(unsafe { ptr.add(t.mem_offset as usize) });
        }
    }
    /// 整理方法
    pub(crate) fn collect(&self, _world: &World) {
        for r in self.records.iter() {
            r.collect()
        }
        // todo!()
    }

}

impl Debug for Archetype {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("Archetype")
            .field("id", &self.id)
            .field("components", &self.components)
            .field("arr", &self.arr)
            .field("index", &self.index)
            .field("records", self.get_records())
            .field("removes", &self.removes)
            .finish()
    }
}

impl Drop for Archetype {
    fn drop(&mut self) {
        // for (_, ptr) in self.iter() {
        //     self.drop_item(ptr);
        // }
        // free memory
        // for (entries, mut len) in self.arr.replace().into_iter() {
        //     len *= self.components_size;
        //     unsafe { drop(Vec::from_raw_parts(entries, len, len)) }
        // }
    }
}
// struct Slot(*mut u8);
// impl Slot {
//     fn version(&self) -> &mut u32 {
//         unsafe { &mut *(self.0 as *mut u32) }
//     }
//     fn value(&self) -> *mut u8 {
//         unsafe { self.0.add(size_of::<u32>()) }
//     }
//     fn is_null(&self) -> bool {
//         *self.version() & 1 == 1
//     }
// }

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComponentInfo {
    pub type_id: TypeId,
    pub type_name: Cow<'static, str>,
    pub drop_fn: Option<fn(*mut u8)>,
    pub mem_size: u32,         // 内存大小
    pub mem_offset: MemOffset, // 内存偏移量
}
impl ComponentInfo {
    pub fn of<T: 'static>() -> ComponentInfo {
        ComponentInfo::create(
            TypeId::of::<T>(),
            std::any::type_name::<T>().into(),
            get_drop::<T>(),
            size_of::<T>(),
        )
    }
    pub fn create(
        type_id: TypeId,
        type_name: Cow<'static, str>,
        drop_fn: Option<fn(*mut u8)>,
        mem_size: usize,
    ) -> Self {
        ComponentInfo {
            type_id,
            type_name,
            drop_fn,
            mem_size: mem_size as u32,
            mem_offset: Null::null(),
        }
    }
    pub fn id(&self) -> u128 {
        unsafe { transmute::<TypeId, u64>(self.type_id) }.into()
    }
    pub fn calc_id(vec: &Vec<ComponentInfo>) -> u128 {
        let mut id = 0;
        for c in vec.iter() {
            id ^= c.id();
        }
        id
    }
}
impl Null for ComponentInfo {
    fn null() -> Self {
        ComponentInfo {
            type_id: Null::null(),
            type_name: Cow::Borrowed(""),
            drop_fn: None,
            mem_size: Null::null(),
            mem_offset: Null::null(),
        }
    }

    fn is_null(&self) -> bool {
        self.mem_offset.is_null()
    }
}
impl PartialOrd for ComponentInfo {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.type_id.partial_cmp(&other.type_id)
    }
}
impl Ord for ComponentInfo {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.type_id.cmp(&other.type_id)
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
        let vec = vec![ComponentInfo::of::<u8>(), ComponentInfo::of::<Arc<i32>>()];
        let ar = Archetype::new(vec);
        let info1 = ar.get_type_info(&TypeId::of::<u8>()).unwrap().clone();
        let info2 = ar.get_type_info(&TypeId::of::<Arc<i32>>()).unwrap().clone();
        println!("ar:{:?}", &ar);
        let (k1, mut ptr) = ar.alloc();
        ptr.set_tick(1);

        let arc = Arc::new(1);
        {
            println!(
                "k:{:?} sizeof:{}, ptr:{:?}",
                k1,
                size_of::<(u8, Arc<i32>)>(),
                ptr
            );
            let t1 = (2u8, arc.clone());
            println!("step:{}", 0);
            ptr.init_component::<u8>(info1.mem_offset).write(t1.0);
            println!("step:{}", 1);
            ptr.init_component::<Arc<i32>>(info2.mem_offset)
                .write(t1.1.clone());
            println!("step:{}", 1);
            std::mem::forget(t1);
            println!("step:{}", 1);
        }
        println!("strong_count1: {:?}", Arc::<i32>::strong_count(&arc));
        {
            let p1 = ar.get(k1);
            let t10: &u8 = p1.get(info1.mem_offset);
            let t11: &Arc<i32> = p1.get(info2.mem_offset);
            println!("strong_count2: {:?}", Arc::<i32>::strong_count(&t11));
            assert_eq!(t10, &2);
            assert_eq!(t11, &Arc::new(1));
        }
        //let map = SlotMap::default();
        ar.remove(k1);
        println!("strong_count3: {:?}", Arc::<i32>::strong_count(&arc));
        println!("{:?}", ar);
    }
}
