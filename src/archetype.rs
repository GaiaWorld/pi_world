/// 原型，存储了一组具有相同组件的entity。
/// 根据system对不同原型上不同组件的读写依赖，可并行执行system
///
/// 在读写依赖分析后，没有原型变动时，一个原型同时只会被一个system 修改 标记删除 Insert添加。
/// 但由于存在动态增删组件，并且组件的组合数量爆炸问题，导致会出现没有提前计算依赖关系的原型，会因为动态增删组件而被操作。虽然这种操作只会是Alter添加。
/// Alter总是采用分配新条目的方式，用Cow方式保证老数据的引用安全。
/// 可能在执行图的某个特定时刻，多个system对新原型有插入的需求，这个时候Alter利用AppendVec的原子保护来保证插入安全。
/// Alter在本地没有找到原型时，有2种情况，一种是找到已存在的原型，一种是没有原型要新创建原型。
/// 新创建原型，通过ready来保护，通知执行图调整图。
/// 已存在的原型，是有可能正在被其他System读写。为了杜绝这种情况，要求在对应原型创建时，ArchetypeDepend计算依赖时，将返回Alter后的原型idArchetypeDepend::Alter(u128)，执行图会查找或新建alter原型id的图节点，并保证一定会在图依赖上有安全的读写。
///
/// 只有主调度完毕后，每个原型进行整理，只有整理才会调整Row。在整理前，Row都是递增的。
///
use core::fmt::*;
use std::any::TypeId;
use std::borrow::Cow;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::mem::{needs_drop, size_of, transmute};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::Ordering;

use bitflags::bitflags;
use pi_null::Null;
use pi_share::{Share, ShareBool};

use crate::column::Column;
use crate::system::TypeInfo;
use crate::table::Table;
use crate::world::{ComponentIndex, SetFromWorld, World};

pub type ShareArchetype = Share<Archetype>;

#[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Row(pub(crate) u32);
impl Row {
    #[inline(always)]
    pub fn index(&self) -> usize {
        self.0 as usize
    }
}
impl From<u32> for Row {
    #[inline(always)]
    fn from(index: u32) -> Self {
        Self(index)
    }
}
impl From<usize> for Row {
    #[inline(always)]
    fn from(index: usize) -> Self {
        Self(index as u32)
    }
}
impl pi_null::Null for Row {
    #[inline(always)]
    fn null() -> Self {
        Self(u32::null())
    }
    #[inline(always)]
    fn is_null(&self) -> bool {
        self.0 == u32::null()
    }
}
#[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ArchetypeIndex(pub(crate) i32);
impl ArchetypeIndex {
    #[inline(always)]
    pub fn index(&self) -> usize {
        self.0 as usize
    }
}
impl From<u32> for ArchetypeIndex {
    #[inline(always)]
    fn from(index: u32) -> Self {
        Self(index as i32)
    }
}
impl From<usize> for ArchetypeIndex {
    #[inline(always)]
    fn from(index: usize) -> Self {
        Self(index as i32)
    }
}
impl pi_null::Null for ArchetypeIndex {
    #[inline(always)]
    fn null() -> Self {
        Self(i32::null())
    }
    #[inline(always)]
    fn is_null(&self) -> bool {
        self.0.is_null()
    }
}


bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Flags: u32 {
        const WITHOUT =         0b000000001; // 排除指定原型或组件
        const OPTION =          0b000000010; // 可选，和读写配合使用
        const READ =            0b000000100; // 读取指定组件或资源
        const WRITE =           0b000001000; // 修改指定组件或资源
        const MOVE =            0b000010000; // 在alter中，不相关的组件，被移动到新的原型中，定义为MOVE。MOVE是弱连接，可以在读后
        const ENTITY_EDIT =     0b000100000; // 对所有实体及其组件进行操作，包括创建及销毁实体，增删组件，读写组件。
        const SHARE_WRITE =     0b001000000; // 多例资源的共享写
        const WORLD_READ =      0b010000000; // World读
        const WORLD_WRITE =     0b100000000; // World写
    }
}

#[derive(Debug, PartialEq)]
pub enum ArchetypeDepend {
    Flag(Flags),
    Alter((u128, Cow<'static, str>, Vec<Share<ComponentInfo>>)),
}
#[derive(Debug, PartialEq)]
pub struct ArchetypeDependResult {
    pub flag: Flags,
    pub reads: Vec<ComponentIndex>,
    pub writes: Vec<ComponentIndex>,
    pub alters: Vec<(u128, Cow<'static, str>, Vec<Share<ComponentInfo>>)>,
}
impl ArchetypeDependResult {
    pub fn new() -> Self {
        Self {
            flag: Flags::empty(),
            reads: Vec::with_capacity(256),
            writes: Vec::with_capacity(256),
            alters: Vec::with_capacity(256),
        }
    }
    pub fn merge(&mut self, depend: ArchetypeDepend) {
        match depend {
            ArchetypeDepend::Flag(f) => self.flag |= f,
            ArchetypeDepend::Alter(t) if !self.flag.contains(Flags::WITHOUT) => self.alters.push(t),
            _ => (),
        }
    }
    pub fn insert(&mut self, _ar: &Archetype, world: &World, components: Vec<ComponentInfo>) {
        // let id = ComponentInfo::calc_id(&components);
        // if &id != ar.id() {
        //     return;
        // }
        self.merge(ArchetypeDepend::Flag(Flags::WRITE));
        for c in components {
            let index = world.get_component_index(&c.type_id());
            self.writes.push(index);
        }
    }
    pub fn depend(
        &mut self,
        _ar: &Archetype,
        _world: &World,
        _tid: &TypeId,
        _false_result: Flags,
        _true_result: Flags,
    ) {
        // let world_index = world.get_component_index(tid);
        // let index = ar.get_column_index(world_index);
        // let r = if index.is_null() {
        //     false_result
        // } else {
        //     let set = if true_result == Flags::WRITE {
        //         &mut self.writes
        //     } else {
        //         &mut self.reads
        //     };
        //     set.push(world_index);
        //     true_result
        // };
        // self.merge(ArchetypeDepend::Flag(r))
        todo!()
    }
    pub fn clear(&mut self) {
        self.flag = Flags::empty();
        self.reads.clear();
        self.writes.clear();
        self.alters.clear();
    }
}

/// Thread-safe archetype
pub struct Archetype {
    id: u64,
    name: Cow<'static, str>,
    table: Table,
    pub(crate) ready: ShareBool, //表示是否已就绪，执行图已经修改正确
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
    pub(crate) fn new(info: ArchetypeInfo) -> Self {
        let name = info.name();
        Self {
            id: info.id,
            name,
            table: Table::new(info.sorted_components),
            ready: ShareBool::new(false),
        }
    }
    // 获得所在的World原型index
    #[inline(always)]
    pub(crate) fn set_index(&mut self, index: ArchetypeIndex) {
        self.table.index = index;
    }
    
    // 获得所在的World原型index
    #[inline(always)]
    pub fn index(&self) -> ArchetypeIndex {
        self.table.index
    }
    #[inline(always)]
    pub fn ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }
    // 从本原型上计算改变后了原型信息， 在该原型下添加一些组件，删除一些组件，得到新原型信息，及移动的组件
    pub(crate) fn alter(
        &self,
        world: &World,
        sorted_add_removes: &[(ComponentIndex, bool)], // 升序
        adding: &mut Vec<Share<Column>>,
        moving: &mut Vec<Share<Column>>,
        removing: &mut Vec<Share<Column>>,
        existed_adding_is_move: bool,
    ) -> ArchetypeInfo {
        let mut result = Vec::with_capacity(256);
        let mut column_index = 0;
        let len = self.column_len();
        let mut pre_index = ComponentIndex::null();
        for (index, add) in sorted_add_removes.iter() {
            // 去重
            if pre_index == *index {
                continue;
            } else {
                pre_index = *index;
            }
            loop {
                if column_index >= len {
                    if *add {
                        let c = world.get_column(*index).unwrap();
                        adding.push(c.clone());
                        result.push(c.clone());
                    }
                    break; // 继续迭代sort_add_removes
                }
                let c = self.get_column_unchecked(column_index);
                let info = c.info();
                if info.index > *index {
                    // info.world_index大
                    if *add {
                        let c = world.get_column(*index).unwrap();
                        adding.push(c.clone());
                        result.push(c.clone());
                    }
                    break; // 继续迭代sort_add_removes
                }
                if info.index < *index {
                    // 在原型中的列要移动
                    moving.push(c.clone());
                    result.push(c.clone());
                    column_index += 1;
                    continue; // 继续递增column_index
                }
                // info.world_index == *index
                if *add {
                    if existed_adding_is_move {
                        moving.push(c.clone());
                    }else{
                        adding.push(c.clone());
                    }
                    result.push(c.clone());

                } else {
                    removing.push(c.clone());
                }
                column_index += 1;
                break; // 继续迭代sort_add_removes
            }
        }
        // 原型中剩余的列都要移动
        while column_index < len {
            let c = self.get_column_unchecked(column_index.into());
            moving.push(c.clone());
            result.push(c.clone());
            column_index += 1;
        }
        ArchetypeInfo::new(result)
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
    pub fn id(&self) -> u64 {
        self.id
    }
    #[inline]
    pub fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
    #[inline(always)]
    pub fn column_len(&self) -> usize {
        self.get_columns().len()
    }
    /// Returns if the archetype is empty.
    #[inline(always)]
    pub fn is_empty_columns(&self) -> bool {
        self.get_columns().len() == 0
    }
}

impl Deref for Archetype {
    type Target = Table;
    fn deref(&self) -> &Self::Target {
        &self.table
    }
}
impl DerefMut for Archetype {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.table
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
#[derive(Debug, Default)]
pub struct ArchetypeInfo {
    pub(crate) id: u64,
    pub(crate) sorted_components: Vec<Share<Column>>,
    // pub(crate) hash: u64,
}
impl ArchetypeInfo {
    pub(crate) fn sort(mut components: Vec<Share<Column>>) -> Self {
        components.sort_unstable_by(|a, b| a.info.index.cmp(&b.info.index));
        Self::new(components)
    }
    pub(crate) fn new(sorted_components: Vec<Share<Column>>) -> Self {
        let mut hasher = DefaultHasher::new();
        // let mut id = 0;
        for c in sorted_components.iter() {
            c.info().index.hash(&mut hasher);
            // id ^= c.info().id();
        }
        let id = hasher.finish();
        Self {
            id,
            sorted_components,
        }
    }
    pub(crate) fn name(&self) -> Cow<'static, str> {
        let mut s = String::new();
        for c in self.sorted_components.iter() {
            s.push_str(&c.info().type_name());
            s.push('+');
        }
        if s.len() > 0 {
            s.pop();
        }
        s.into()
    }
}

pub const COMPONENT_TICK: u8 = 1;
// pub const COMPONENT_CHANGED: u8 = 2;
// pub const COMPONENT_ADDED: u8 = 4;
// pub const COMPONENT_REMOVED: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentInfo {
    pub type_info: Share<TypeInfo>,
    pub drop_fn: Option<fn(*mut u8)>,
    pub set_fn: Option<fn(&mut World, *mut u8)>,
    pub index: ComponentIndex, // 在world上的索引
    pub mem_size: u32,             // 内存大小
    pub tick_info: u8,            // tick信息 tick = 1 changed = 2 added = 4 removed = 8
}
impl ComponentInfo {
    pub fn of<T: 'static>(tick_info: u8) -> ComponentInfo {
        ComponentInfo::create(
            TypeId::of::<T>(),
            std::any::type_name::<T>().into(),
            get_drop::<T>(),
            <T as SetFromWorld>::set_fn(),
            size_of::<T>() as u32,
            tick_info,
        )
    }
    pub fn create(
        type_id: TypeId,
        type_name: Cow<'static, str>,
        drop_fn: Option<fn(*mut u8)>,
        set_fn: Option<fn(&mut World, *mut u8)>,
        mem_size: u32,
        tick_info: u8,
    ) -> Self {
        let type_info = Share::new(TypeInfo{type_id, type_name});
        ComponentInfo {
            type_info,
            drop_fn,
            set_fn,
            mem_size,
            index: ComponentIndex::null(),
            tick_info,
        }
    }
    pub fn type_id(&self) -> &TypeId {
        &self.type_info.type_id
    }
    pub fn type_name(&self) -> &Cow<'static, str> {
        &self.type_info.type_name
    }
    #[inline(always)]
    pub fn size(&self) -> usize {
        self.mem_size as usize
    }
    pub fn id(&self) -> u128 {
        unsafe { transmute::<TypeId, u128>(*self.type_id()) }.into()
    }
    pub fn is_tick(&self) -> bool {
        self.tick_info > 0
    }
    pub fn calc_id(vec: &Vec<ComponentInfo>) -> u128 {
        let mut id = 0;
        for c in vec.iter() {
            id ^= c.id();
        }
        id
    }
}

impl PartialOrd for ComponentInfo {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.type_id().partial_cmp(&other.type_id())
    }
}
impl Ord for ComponentInfo {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.type_id().cmp(&other.type_id())
    }
}

/// 获得指定类型的释放函数
pub fn get_drop<T>() -> Option<fn(*mut u8)> {
    needs_drop::<T>().then_some(|ptr: *mut u8| {
        unsafe { (ptr as *mut T).drop_in_place() }
    })
}
