/// 原型，存储了一组具有相同组件的entity。
/// 根据system对不同原型的读写依赖，可并行执行system
///
/// 在读写依赖分析后，没有原型变动时，一个原型同时只会被一个system 修改 标记删除 Insert添加。
/// 但由于存在动态增删组件，并且组件的组合数量爆炸问题，导致会出现没有提前计算依赖关系的原型，会因为动态增删组件而被操作。虽然这种操作只会是Alter添加。
/// Alter总是采用分配新条目的方式，用Cow方式保证老数据的引用安全。
/// 可能在执行图的某个特定时刻，多个system对新原型有插入的需求，这个时候Alter利用AppendVec的原子保护来保证插入安全。
/// Alter在本地没有找到原型时，有2种情况，一种是找到已存在的原型，一种是没有原型要新创建原型。
/// 新创建原型，通过index来保护，通知所有system添加脏列表，这样新增的条目，都会被记录脏。
/// 已存在的原型，是有可能正在被其他System读写和监听Change和Add的。为了杜绝这种情况，要求在对应原型创建时，ArchetypeDepend计算依赖时，将返回Alter后的原型idArchetypeDepend::Alter(u128)，执行图会查找或新建alter原型id的图节点，并保证一定会在图依赖上有安全的读写。
/// system监听Change和Add组件，是通过在原型上添加自己关心组件的脏列表来监听。
/// 有外部调度的安全的读写（写串行，读并行，先写后读），所以Change一定在单个system内操作的。 但考虑以后单system也可能开多future并行修改，而Archetype作为基础结构已经内置了ComponentDirty，所以ComponentDirty的vec还是用线程安全的AppendVec。
/// Add由于有可能被其他system的Alter添加，所以脏的add_len来保证没有监听到的Add下次会监听。
///
/// 只有主调度完毕后，所有的脏都被处理和清理后，才进行整理，只有整理才会调整Row。在整理前，Row都是递增的。
///
use core::fmt::*;
use std::any::TypeId;
use std::borrow::Cow;
use std::mem::{needs_drop, size_of, transmute};
use std::sync::atomic::Ordering;

use bitflags::bitflags;
use fixedbitset::FixedBitSet;
use pi_null::Null;
use pi_phf_map::PhfMap;
use pi_share::{Share, ShareU32};
use smallvec::SmallVec;

use crate::column::Column;
use crate::dirty::DirtyIndex;
use crate::table::Table;
use crate::world::World;

pub type ShareArchetype = Share<Archetype>;

pub type Row = u32;
pub type ArchetypeWorldIndex = u32;
pub type ColumnIndex = u32;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Flags: u32 {
        const WITHOUT =     0b0001;
        const READ =        0b0010;
        const WRITE =       0b0100;
        const DELETE =      0b1000;
    }
}

#[derive(Debug, PartialEq)]
pub enum ArchetypeDepend {
    Flag(Flags),
    Alter((u128, Cow<'static, str>)),
}
#[derive(Debug, PartialEq)]
pub struct ArchetypeDependResult {
    pub flag: Flags,
    pub alters: SmallVec<[(u128, Cow<'static, str>); 1]>,
}
impl ArchetypeDependResult {
    pub fn new() -> Self {
        Self {
            flag: Flags::empty(),
            alters: SmallVec::new(),
        }
    }
    pub fn merge(&mut self, depend: ArchetypeDepend) {
        match depend {
            ArchetypeDepend::Flag(f) => self.flag |= f,
            ArchetypeDepend::Alter(t) if !self.flag.contains(Flags::WITHOUT) => {
                self.alters.push(t)
            }
            _ => (),
        }
    }
    pub fn clear(&mut self) {
        self.flag = Flags::empty();
        self.alters.clear();
    }
}

/// Thread-safe archetype
pub struct Archetype {
    id: u128,
    name: Cow<'static, str>,
    pub(crate) table: Table,
    map: PhfMap<TypeId, ColumnIndex>,
    pub(crate) index: ShareU32, // ，在全局原型列表中的位置，也表示是否已就绪，脏列表是否已经被全部的system添加好了
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
        let mut s = String::new();
        let mut vec1 = Vec::with_capacity(components.capacity());
        for (i, info) in components.iter_mut().enumerate() {
            id ^= info.id();
            s.push_str(&info.type_name);
            s.push('+');
            vec1.push((info.type_id, i as u32));
        }
        if s.len() > 0 {
            s.pop();
        }
        Self {
            id,
            name: s.into(),
            table: Table::new(components),
            map: PhfMap::new(vec1),
            index: ShareU32::new(u32::null()),
        }
    }
    // 获得ready状态
    #[inline(always)]
    pub fn index(&self) -> ArchetypeWorldIndex {
        self.index.load(Ordering::Relaxed)
    }
    // 原型表结构改变， 在该原型下添加一些组件，删除一些组件，得到新原型需要包含哪些组件，及移动的组件
    pub fn alter(
        &self,
        sort_add: &Vec<ComponentInfo>,
        sort_del: &Vec<TypeId>,
    ) -> (Vec<ComponentInfo>, Vec<TypeId>) {
        let mut add = sort_add.clone();
        // 记录移动的组件
        let mut moving = Vec::new();
        for c in self.table.columns.iter() {
            // 如果组件是要删除或要添加的组件，则不添加，只有移动的才添加
            if sort_del.binary_search(&c.info.type_id).is_err()
                && sort_add.binary_search(&c.info).is_err()
            {
                add.push(c.info.clone());
                moving.push(c.info.type_id);
            }
        }
        (add, moving)
    }
    // 根据脏监听列表，添加监听，该方法要么在初始化时调用，要么就是在原型刚创建时调用
    pub(crate) unsafe fn add_dirty_listeners(
        &self,
        owner: TypeId,
        listeners: &SmallVec<[(TypeId, bool); 1]>,
    ) {
        //println!("add_dirty_listeners");
        for (tid, changed) in listeners.iter() {
            let index = self.get_column_index(tid);
            if index.is_null() {
                continue;
            }
            let c = self.table.get_column_unchecked(index);
            if *changed {
                c.changed.insert_listener(owner);
            } else {
                c.added.insert_listener(owner);
            }
        }
    }
    // 根据监听列表，重新找到add_dirty_listeners前面放置脏监听列表的位置
    pub fn find_dirty_listeners(
        &self,
        owner: TypeId,
        listens: &SmallVec<[(TypeId, bool); 1]>,
        vec: &mut SmallVec<[DirtyIndex; 1]>,
    ) {
        for (tid, changed) in listens.iter() {
            let index = self.get_column_index(tid);
            if index.is_null() {
                continue;
            }
            let c = self.table.get_column_unchecked(index);
            let d = if *changed { &c.changed } else { &c.added };
            d.find(index, owner, *changed, vec);
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
    pub fn id(&self) -> &u128 {
        &self.id
    }
    #[inline]
    pub fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
    #[inline(always)]
    pub fn get_columns(&self) -> &Vec<Column> {
        &self.table.columns
    }
    #[inline(always)]
    pub fn get_column_index(&self, type_id: &TypeId) -> ColumnIndex {
        if let Some(t) = self.map.get(&type_id) {
            if t.is_null() {
                return u32::null();
            }
            let ti = unsafe { self.table.columns.get_unchecked(*t as usize) };
            if &ti.info.type_id == type_id {
                return *t;
            }
        }
        u32::null()
    }
    #[inline(always)]
    pub fn get_column(&self, type_id: &TypeId) -> Option<&Column> {
        if let Some(t) = self.map.get(&type_id) {
            if t.is_null() {
                return None;
            }
            let t = unsafe { self.table.columns.get_unchecked(*t as usize) };
            if &t.info.type_id == type_id {
                return Some(t);
            }
        }
        None
    }
    #[inline(always)]
    pub unsafe fn get_column_unchecked(&self, type_id: &TypeId) -> &Column {
        self.table
            .columns
            .get_unchecked(*self.map.get_unchecked(&type_id) as usize)
    }
    /// Returns the number of elements in the archetype.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.table.len() as usize
    }
    /// Returns if the archetype is empty.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.table.columns.len() == 0
    }
    /// 整理方法
    pub(crate) fn collect(
        &mut self,
        world: &World,
        action: &mut Vec<(Row, Row)>,
        set: &mut FixedBitSet,
    ) {
        let _r = self.table.collect(world, action, set);
        // println!("{:?} collect {}", self.name, r);
    }
}

impl Debug for Archetype {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("Archetype")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("table", &self.table)
            .field("index", &self.index)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComponentInfo {
    pub type_id: TypeId,
    pub type_name: Cow<'static, str>,
    pub drop_fn: Option<fn(*mut u8)>,
    pub mem_size: u32, // 内存大小
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
        }
    }
    pub fn id(&self) -> u128 {
        unsafe { transmute::<TypeId, u128>(self.type_id) }.into()
    }
    pub fn calc_id(vec: &Vec<ComponentInfo>) -> u128 {
        let mut id = 0;
        for c in vec.iter() {
            id ^= c.id();
        }
        id
    }
    pub fn calc_id_name(vec: &Vec<ComponentInfo>) -> (u128, Cow<'static, str>) {
        // todo 用前缀树的方式，类似use pi_share::{Share, ShareU32} 方式记录为name
        let mut id = 0;
        let mut s = String::new();
        for c in vec.iter() {
            id ^= c.id();
            s.push_str(&c.type_name);
            s.push('+');
        }
        if s.len() > 0 {
            s.pop();
        }
        (id, s.into())
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
