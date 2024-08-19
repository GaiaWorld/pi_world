/// system上只能看到Query等SystemParm参数，SystemParm参数一般包含：事件、单例和多例资源、实体、组件
/// world上包含了全部的资源和实体，及实体原型。 加一个监听管理器，
/// world上的数据（资源、实体和原型）的线程安全的保护仅在于保护容器，
/// 由调度器生成的执行图，来保证正确的读写。
/// 执行图会注册监听器来监听新增的原型
/// 比如一个原型不可被多线程同时读写，是由执行图时分析依赖，先执行写的sys，再执行读的sys。
/// 由于sys会进行组件的增删，导致实体对于的原型会变化，执行图可能会产生变化，执行图本身保证对原型的访问是安全的读写。
/// 整理操作时，一般是在整个执行图执行完毕后，进行进行相应的调整。举例：
///
/// 如果sys上通过Alter来增删组件，则可以在entity插入时，分析出sys的依赖。除了首次原型创建时，时序不确定，其余的增删，sys会保证先写后读。
/// 如果sys通过是MultiRes实现的CmdQueue来延迟动态增删组件，则sys就不会因此产生依赖，动态增删的结果就只能在可能在下一帧才会看到。
///
///
use crate::alter::{AlterState, QueryAlterState};
use crate::archetype::{
    Archetype, ArchetypeIndex, ArchetypeInfo, ComponentInfo, Row, ShareArchetype,
};
use crate::column::Column;
#[cfg(debug_assertions)]
use crate::column::{ARCHETYPE_INDEX, COMPONENT_INDEX};
use crate::editor::{EditorState, EntityEditor};
use crate::fetch::{ColumnTick, FetchComponents};
use crate::filter::FilterComponents;
use crate::insert::{Bundle, InsertState};
use crate::listener::{EventListKey, ListenerMgr};
use crate::multi_res::ResVec;
use crate::prelude::Mut;
use crate::query::{QueryError, QueryState};
use crate::single_res::TickRes;
use crate::system::{SystemMeta, TypeInfo};
use core::fmt::*;
use core::result::Result;
use std::marker::PhantomData;
use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use fixedbitset::FixedBitSet;
use pi_append_vec::{SafeVec, SafeVecIter};
use pi_key_alloter::new_key_type;
use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::collections::{hash_map::Entry as StdEntry, HashMap};
use std::mem::{self, size_of, transmute, ManuallyDrop};
use std::ops::Deref;
use std::ptr;
use std::sync::atomic::Ordering;
// use pi_map::hashmap::HashMap;
// use pi_map::Map;
use pi_null::Null;
use pi_share::{Share, ShareUsize};
use pi_slot::{Iter, SlotMap};

new_key_type! {
    pub struct Entity;
}

#[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentIndex(u32);
impl ComponentIndex {
    pub fn index(&self) -> usize {
        self.0 as usize
    }
}
impl From<u32> for ComponentIndex {
    fn from(index: u32) -> Self {
        Self(index)
    }
}
impl From<usize> for ComponentIndex {
    fn from(index: usize) -> Self {
        Self(index as u32)
    }
}
impl pi_null::Null for ComponentIndex {
    fn null() -> Self {
        Self(u32::null())
    }
    fn is_null(&self) -> bool {
        self.0 == u32::null()
    }
}
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(u32);
impl Tick {
    pub fn index(&self) -> usize {
        self.0 as usize
    }
    pub fn max() -> Self {
        Self(u32::MAX)
    }
}
impl Deref for Tick {
    type Target = u32;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Null for Tick {
    fn null() -> Self {
        Self(0)
    }
    fn is_null(&self) -> bool {
        self.0 == 0
    }
}
impl From<u32> for Tick {
    fn from(v: u32) -> Self {
        Self(v)
    }
}
impl From<usize> for Tick {
    fn from(v: usize) -> Self {
        Self(v as u32)
    }
}

#[derive(Clone, Debug)]
pub struct ArchetypeInit<'a>(pub &'a ShareArchetype, pub &'a World);
#[derive(Clone, Debug)]
pub struct ArchetypeOk<'a>(pub &'a ShareArchetype, pub ArchetypeIndex, pub &'a World);

pub struct World {
    pub(crate) single_res_map: HashMap<TypeId, usize>,
    pub(crate) single_res_arr: Vec<Option<Share<dyn TickMut>>>,
    pub(crate) multi_res_map: HashMap<TypeId, (Share<dyn Any + Send + Sync>, Share<ShareUsize>)>,
    pub(crate) event_map: HashMap<TypeId, Share<dyn Settle>>, // 事件表
    pub(crate) component_map: HashMap<TypeId, ComponentIndex>,
    pub(crate) component_arr: Vec<Share<Column>>,
    pub(crate) entities: SlotMap<Entity, EntityAddr>,
    pub(crate) archetype_map: DashMap<u64, ShareArchetype>,
    pub(crate) archetype_arr: SafeVec<ShareArchetype>,
    pub(crate) archetype_arr_len: usize,
    pub(crate) empty_archetype: ShareArchetype,
    pub(crate) entity_editor_state: EditorState,
    pub(crate) listener_mgr: ListenerMgr,
    archetype_init_key: EventListKey,
    archetype_ok_key: EventListKey,
    // 世界当前的tick
    tick: ShareUsize,
}
impl Debug for World {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("World")
            .field("entitys", &self.entities)
            .field("component_arr", &self.component_arr)
            .field("archetype_arr", &self.archetype_arr)
            .finish()
    }
}
impl World {
    pub fn new() -> Self {
        #[cfg(debug_assertions)]
        match std::env::var("ECS_DEBUG") {
            Ok(r) => {
                let r = r.split(",").map(|r| {
                    if r == "*" {
                        std::usize::MAX
                    } else {
                        r.parse::<usize>().unwrap()
                    }

                }).collect::<Vec<usize>>();
                if r.len() == 2 {
                    ARCHETYPE_INDEX.store(r[0], Ordering::Relaxed);
                    COMPONENT_INDEX.store(r[1], Ordering::Relaxed); 
                }
            },
            _ => (),
        };

        let listener_mgr = ListenerMgr::default();
        let archetype_init_key = listener_mgr.init_register_event::<ArchetypeInit>();
        let archetype_ok_key = listener_mgr.init_register_event::<ArchetypeOk>();
        let mut empty = Archetype::new(Default::default());
        empty.set_index(0usize.into());
        empty.ready.store(true, Ordering::Relaxed);
        let empty_archetype = ShareArchetype::new(empty);
        let archetype_map = DashMap::new();
        archetype_map.insert(0, empty_archetype.clone());
        let archetype_arr = SafeVec::with_capacity(1);
        archetype_arr.insert(empty_archetype.clone());
        Self {
            single_res_map: Default::default(),
            single_res_arr: Default::default(),
            multi_res_map: Default::default(),
            event_map: Default::default(),
            entities: Default::default(),
            component_map: Default::default(),
            component_arr: Default::default(),
            archetype_map,
            archetype_arr,
            archetype_arr_len: 1,
            empty_archetype,
            listener_mgr,
            archetype_init_key,
            archetype_ok_key,
            tick: ShareUsize::new(1),
            entity_editor_state: Default::default(),
        }
    }
    // 获得世界当前的tick
    pub fn tick(&self) -> Tick {
        self.tick.load(Ordering::Relaxed).into()
    }
    // 递增世界当前的tick，一般是每执行图执行时递增
    pub fn increment_tick(&self) -> Tick {
        self.tick.fetch_add(1, Ordering::Relaxed).into()
    }
    // /// 批量插入
    // pub fn batch_insert<'w, I, Ins>(&'w mut self, iter: I) -> InsertBatchIter<'w, I, Ins>
    // where
    //     I: Iterator<Item = Ins>,
    //     Ins: Bundle,
    // {
    //     InsertBatchIter::new(self, iter.into_iter())
    // }
    // /// 创建一个插入器 todo 移除
    // pub fn make_inserter<I: Bundle>(&mut self) -> Inserter<I> {
    //     let components = I::components(Vec::new());
    //     let ar = self.find_ar(components);
    //     let s = I::init_item(self, &ar);
    //     Inserter::new(self, InsertState::new(ar, s), self.tick())
    // }
    /// 获得实体的原型信息
    pub fn get_entity_prototype(&self, entity: Entity) ->  Option<(&Cow<'static, str>, ArchetypeIndex)> {
        self.entities.get(entity).map(|e| {
            let ar_index = e.archetype_index();
            let ar = self.archetype_arr.get(ar_index.index()).unwrap();
            (ar.name(), ar_index.into())
        })
    }
    /// 是否存在实体
    pub fn contains(&self, entity: Entity) -> bool {
        self.entities.contains_key(entity)
    }
    /// 获得指定组件的索引
    pub fn get_component_index(&self, component_type_id: &TypeId) -> ComponentIndex {
        self.component_map
            .get(component_type_id)
            .map_or(ComponentIndex::null(), |r| *r)
    }
    /// 获得指定组件的索引
    pub fn add_component_indexs(
        &mut self,
        components: Vec<ComponentInfo>,
        result: &mut Vec<(ComponentIndex, bool)>,
        result_add: bool,
    ) {
        for c in components {
            result.push((self.add_component_info(c).0, result_add));
        }
    }
    /// 获得指定组件的索引
    pub fn get_column_by_id(&self, component_type_id: &TypeId) -> Option<&Share<Column>> {
        self.get_column(self.get_component_index(component_type_id))
    }
    /// 获得指定组件的索引
    pub fn get_column(&self, index: ComponentIndex) -> Option<&Share<Column>> {
        self.component_arr.get(index.index())
    }
    /// 获得指定组件的索引
    pub unsafe fn get_column_unchecked(&self, index: ComponentIndex) -> &Share<Column> {
        self.component_arr.get_unchecked(index.index())
    }
    /// 添加组件信息，如果重复，则返回原有的索引及是否tick变化 todo 改成mut
    pub fn add_component_info(
        &mut self,
        mut info: ComponentInfo,
    ) -> (ComponentIndex, Share<Column>) {
        let tick_info = info.tick_info;
        let index: ComponentIndex = match self.component_map.entry(*info.type_id()) {
            StdEntry::Occupied(entry) => *entry.get(),
            StdEntry::Vacant(entry) => {
                let index = self.component_arr.len().into();
                info.index = index;
                let c = Share::new(Column::new(info));
                self.component_arr.push(c.clone());
                entry.insert(index);
                return (index, c);
            }
        };
        let column = unsafe { self.component_arr.get_unchecked_mut(index.index()) };
        let c = unsafe { Share::get_mut_unchecked(column) };
        let t = c.info.tick_info | tick_info;
        if t != c.info.tick_info {
            let tick = self.tick.load(Ordering::Relaxed).into();
            c.info.info.tick_info = tick_info;
            // 扫描当前列，将已有的实体设置tick
            c.update(&self.archetype_arr, |r, row, _| {
                r.set_tick_unchecked(row, tick);
            });
        }
        (index, column.clone())
    }
    /// 初始化指定组件
    pub fn init_component<T: 'static>(&mut self) -> ComponentIndex {
        self.add_component_info(ComponentInfo::of::<T>(0)).0
    }
    /// 计算所有原型信息，设置了所有组件的索引，按索引大小进行排序
    pub(crate) fn archetype_info(&mut self, components: Vec<ComponentInfo>) -> ArchetypeInfo {
        let vec: Vec<Share<Column>> = components
            .into_iter()
            .map(|c| self.add_component_info(c).1)
            .collect();
        ArchetypeInfo::sort(vec)
    }
    /// 创建一个插入器
    pub fn make_insert<B: Bundle>(&mut self) -> InsertState<B> {
        let components = B::components(Vec::new());
        let ar = self.find_ar(components);
        let s = B::init_item(self, &ar);
        InsertState::new(ar, s)
    }
    /// 兼容bevy的接口，提供query
    pub fn query<Q: FetchComponents + 'static, F: FilterComponents + 'static = ()>(
        &mut self,
    ) -> QueryState<Q, F> {
        self.make_query()
    }
    /// 创建一个查询器
    pub fn make_query<Q: FetchComponents + 'static, F: FilterComponents + 'static = ()>(
        &mut self,
    ) -> QueryState<Q, F> {
        let mut meta = SystemMeta::new(TypeInfo::of::<QueryState<Q, F>>());
        let mut state = QueryState::create(self, &mut meta);
        state.align(self);
        state
    }
    /// 创建一个改变器
    pub fn make_alter<
        Q: FetchComponents + 'static,
        F: FilterComponents + 'static,
        A: Bundle + 'static,
        D: Bundle + 'static,
    >(
        &mut self,
    ) -> QueryAlterState<Q, F, A, D> {
        let mut meta = SystemMeta::new(TypeInfo::of::<QueryAlterState<Q, F, A, D>>());
        let mut query_state = QueryState::create(self, &mut meta);
        let mut alter_state =
            AlterState::make(self, A::components(Vec::new()), D::components(Vec::new()));
        query_state.align(self);
        // 将新多出来的原型，创建原型空映射
        alter_state.align(self,  &query_state.archetypes);
        QueryAlterState(query_state, alter_state, PhantomData)
    }
    /// 创建一个实体编辑器
    pub fn make_entity_editor(&mut self) -> EntityEditor {
        EntityEditor::new(self)
    }

    pub fn unsafe_world<'a>(&self) -> ManuallyDrop<&'a mut World> {
        unsafe { transmute(self) }
    }

    pub(crate) fn empty_archetype(&self) -> &ShareArchetype {
        &self.empty_archetype
    }
    pub fn len<'a>(&'a self) -> usize {
        self.entities.len()
    }

    pub fn entities_iter<'a>(&'a self) -> Iter<'a, Entity, EntityAddr> {
        self.entities.iter()
    }
    /// 插入指定的单例资源，返回索引
    pub fn insert_single_res<T: 'static>(&mut self, value: T) -> usize {
        let tid = TypeId::of::<T>();
        let index = *self.single_res_map.entry(tid).or_insert_with(|| {
            let index = self.single_res_arr.len();
            self.single_res_arr.push(None);
            index
        });
        if self.single_res_arr[index].is_none() {
            self.single_res_arr[index] = Some(Share::new(TickRes::new(value)));
        }
        index
    }

    // 如果不存在单例类型， 则注册指定的单例资源（不插入具体值，只添加类型），返回索引
    pub fn or_register_single_res(&mut self, type_info: TypeInfo) -> usize {
        *self
            .single_res_map
            .entry(type_info.type_id)
            .or_insert_with(|| {
                let index = self.single_res_arr.len();
                self.single_res_arr.push(None);
                index
            })
    }

    pub fn register_single_res<T: 'static>(&mut self) -> usize {
        *self
            .single_res_map
            .entry(TypeId::of::<T>())
            .or_insert_with(|| {
                let index = self.single_res_arr.len();
                self.single_res_arr.push(None);
                index
            })
    }

    /// 注册单例资源， 如果已经注册，则忽略，返回索引
    pub fn init_single_res<T: 'static + FromWorld>(&mut self) -> usize {
        let tid = TypeId::of::<T>();
        if let Some(r) = self.single_res_map.get(&tid) {
            return *r;
        }
        let value = T::from_world(self);
        self.insert_single_res(value)
    }

    /// 用索引获得指定的只读单例资源
    #[inline]
    pub fn index_single_res<T: 'static>(&self, index: usize) -> Option<&TickRes<T>> {
        self.single_res_arr.get(index).map_or(None, |r| {
            match r {
                Some(r) => r.as_any().downcast_ref(),
                None => None,
            }
        })
    }
    /// 用索引获得指定的可变单例资源
    #[inline]
    pub fn index_single_res_mut<T: 'static>(
        &mut self,
        index: usize,
    ) -> Option<&mut TickRes<T>> {
        self.single_res_arr.get_mut(index).map_or(None, |r| {
            match r {
                Some(r) => unsafe { Share::get_mut_unchecked(r).as_any_mut().downcast_mut() },
                None => None,
            }
        })
    }
    /// 获得指定的单例资源
    #[inline]
    pub fn get_share_single_res<T: 'static>(&self) -> Option<Share<TickRes<T>>> {
        let tid = TypeId::of::<T>();
        self.get_single_res_any(&tid).map(|r| Share::downcast(r.clone().into_any()).unwrap())
    }

    /// 获得指定的单例资源
    #[inline]
    pub fn get_single_res<T: 'static>(&self) -> Option<&TickRes<T>> {
        let tid = TypeId::of::<T>();
        match self.single_res_map.get(&tid) {
            Some(index) => self.index_single_res(*index),
            None => return None,
        }
    }
    /// 获得指定的单例资源
    #[inline]
    pub fn get_single_res_mut<T: 'static>(&mut self) -> Option<&mut TickRes<T>> {
        let tid = TypeId::of::<T>();
        match self.single_res_map.get(&tid) {
            Some(index) => self.index_single_res_mut(*index),
            None => return None,
        }
    }
    pub(crate) fn get_single_res_any(&self, tid: &TypeId) -> Option<&Share<dyn TickMut>> {
        match self.single_res_map.get(tid) {
            Some(index) => self.index_single_res_any(*index),
            None => return None,
        }
    }
    pub(crate) fn index_single_res_any(&self, index: usize) -> Option<&Share<dyn TickMut>> {
        self.single_res_arr.get(index).map_or(None, |r| r.as_ref())
    }

    /// 初始化指定类型的多例资源
    pub fn init_multi_res(&mut self, type_id: TypeId, vec: Share<dyn Any + Send + Sync>) -> (Share<dyn Any + Send + Sync>, Share<ShareUsize>) {
        self.multi_res_map.entry(type_id).or_insert_with(|| (vec, Share::new(ShareUsize::new(0)))).clone()
    }
    /// 获得指定类型的多例资源
    pub fn get_multi_res<T>(&self) -> Option<(Share<ResVec<T>>, Share<ShareUsize>)> {
        let tid = TypeId::of::<T>();
        self.multi_res_map.get(&tid).map(|(r, t)| {
            (Share::downcast(r.clone()).unwrap(), t.clone())
        })
    }

    /// 初始化事件记录
    pub(crate) fn init_event_record(
        &mut self,
        type_id: TypeId,
        event_record: Share<dyn Settle>,
    ) -> Share<dyn Settle> {
        let r = self
            .event_map
            .entry(type_id)
            .or_insert_with(|| event_record);
        r.clone()
    }
    /// 获得事件记录
    pub(crate) fn get_event_record(&self, type_id: &TypeId) -> Option<Share<dyn Settle>> {
        self.event_map.get(type_id).map(|r| r.clone())
    }

    /// 获得指定实体的指定组件
    pub fn get_component<T: 'static>(&self, e: Entity) -> Result<&T, QueryError> {
        let index = self.get_component_index(&TypeId::of::<T>());
        self.get_component_by_index(e, index)
    }
    /// 获得指定实体的指定组件
    pub fn get_component_mut<T: 'static>(
        &mut self,
        e: Entity,
    ) -> Result<Mut<'static, T>, QueryError> {
        let index = self.get_component_index(&TypeId::of::<T>());
        self.get_component_mut_by_index(e, index)
    }

    /// 获得指定实体的指定组件
    pub fn get_component_by_index<T: 'static>(
        &self,
        e: Entity,
        index: ComponentIndex,
    ) -> Result<&T, QueryError> {
        let addr = match self.entities.get(e) {
            Some(v) => v,
            None => return Err(QueryError::NoSuchEntity(e)),
        };
        let column = match self.get_column(index) {
            Some(c) => c,
            None => return Err(QueryError::NoSuchComponent(index)),
        };
        let column = column.blob_ref(addr.archetype_index());
        match column {
            Some(c) => Ok(c.get::<T>(addr.row, e)),
            None => Err(QueryError::MissingComponent(index, addr.archetype_index())),
        }
    }
    /// 获得指定实体的指定组件
    pub fn get_component_mut_by_index<T: 'static>(
        &mut self,
        e: Entity,
        index: ComponentIndex,
    ) -> Result<Mut<'static, T>, QueryError> {
        let addr = match self.entities.get(e) {
            Some(v) => v,
            None => return Err(QueryError::NoSuchEntity(e)),
        };
        let column = match self.get_column(index) {
            Some(c) => c,
            None => return Err(QueryError::NoSuchComponent(index)),
        };
        let column = column.blob_ref(addr.archetype_index());
        match column {
            Some(c) => {
                let t = self.tick();
                let value: Mut<T> = Mut::new(&ColumnTick::new(c, t, t), e, addr.row);
                Ok(unsafe { transmute(value) })
            },
            None => Err(QueryError::MissingComponent(index, addr.archetype_index())),
        }
    }

    pub fn get_archetype(&self, index: ArchetypeIndex) -> Option<&ShareArchetype> {
        self.archetype_arr.get(index.0 as usize)
    }
    pub(crate) unsafe fn get_archetype_unchecked(&self, index: ArchetypeIndex) -> &ShareArchetype {
        self.archetype_arr.get_unchecked(index.0 as usize)
    }
    pub fn archetype_list<'a>(&'a self) -> SafeVecIter<'a, ShareArchetype> {
        self.archetype_arr.iter()
    }
    // 返回原型及是否新创建 todo 改成mut
    pub(crate) fn find_ar(&mut self, infos: Vec<ComponentInfo>) -> ShareArchetype {
        let info = self.archetype_info(infos);
        self.find_archtype(info)
    }
    // 返回原型及是否新创建
    pub(crate) fn find_archtype(&self, info: ArchetypeInfo) -> ShareArchetype {
        // 如果world上没有找到对应的原型，则创建并放入world中
        let (mut ar, b) = match self.archetype_map.entry(info.id) {
            Entry::Occupied(entry) => (entry.get().clone(), false),
            Entry::Vacant(entry) => {
                let ar = Share::new(Archetype::new(info));
                entry.insert(ar.clone());
                (ar, true)
            }
        };
        if b {
            // 通知原型创建，让各查询过滤模块初始化原型的记录列表，通知执行图更新
            self.listener_mgr
                .notify_event(self.archetype_init_key, ArchetypeInit(&ar, &self));
            // 通知后，让原型就绪， 其他线程也就可以获得该原型
            let ar_index = self.archtype_ok(&mut ar);
            // println!("add archtype: {:?}", (ar.name(), ar_index));
            self.listener_mgr
                .notify_event(self.archetype_ok_key, ArchetypeOk(&ar, ar_index, &self));
            ar
        } else {
            // 循环等待原型就绪
            loop {
                if !ar.ready() {
                    std::hint::spin_loop();
                }
                return ar;
            }
        }
    }
    // 先事件通知调度器，将原型放入数组，之后其他system可以看到该原型
    pub(crate) fn archtype_ok(&self, ar: &mut ShareArchetype) -> ArchetypeIndex {
        let entry = self.archetype_arr.alloc_entry();
        let index = entry.index();
        let mut_ar = unsafe { Share::get_mut_unchecked(ar) };
        mut_ar.set_index(index.into());
        mut_ar.init_blobs(); // 初始化原型中的blob
        ar.ready.store(true, Ordering::Relaxed);
        entry.insert(ar.clone()); // entry销毁后， 其他线程通过archetype_arr就可以看见该原型
        index.into()
    }
    /// 插入一个新的EntityAddr
    #[inline(always)]
    pub(crate) fn insert_addr(&self, ar_index: ArchetypeIndex, row: Row) -> Entity {
        self.entities.insert(EntityAddr::new(ar_index, row))
    }
    /// 替换Entity的原型及行
    #[inline(always)]
    pub(crate) fn replace(&self, e: Entity, ar_index: ArchetypeIndex, row: Row) -> EntityAddr {
        let addr = unsafe { self.entities.load_unchecked(e) };
        mem::replace(addr, EntityAddr::new(ar_index, row))
    }
    /// 判断指定的实体是否存在
    pub fn contains_entity(&self, e: Entity) -> bool {
        self.entities.get(e).is_some()
    }
    /// 销毁指定的实体
    pub fn destroy_entity(&mut self, e: Entity) -> Result<(), QueryError> {
        let addr = match self.entities.get(e) {
            Some(v) => *v,
            None => return Err(QueryError::NoSuchEntity(e)),
        };
        if addr.row.is_null() {
            self.entities.remove(e).unwrap();
            return Ok(());
        }
        let ar = unsafe {
            self.archetype_arr
                .get_unchecked(addr.archetype_index().index())
        };
        let e = ar.destroy(addr.row);
        if e.is_null() {
            return Err(QueryError::NoSuchRow(addr.row));
        }
        self.entities.remove(e).unwrap();
        Ok(())
    }

    /// 创建一个新的空实体
    pub fn spawn_empty(&self) -> Entity {
        self.entities
            .insert(EntityAddr::new(0usize.into(), Row::null()))
    }
    /// 替换Entity的原型及行
    #[inline(always)]
    pub(crate) fn replace_row(&self, e: Entity, row: Row) {
        let addr = unsafe { self.entities.load_unchecked(e) };
        addr.row = row;
    }
    /// 获得内存大小
    pub fn mem_size(&self) -> usize {
        let mut size = self.entities.mem_size();
        // size += self.component_arr.capacity() * size_of::<Column>();
        self.component_arr.iter().for_each(|item| {
            size += item.memsize();
        });
        size += (self.component_arr.capacity() - self.component_arr.len()) * size_of::<Column>();
        size += self.archetype_arr.capacity() * size_of::<Archetype>();
        for ar in self.archetype_arr.iter() {
            size += ar.mem_size();
        }
        size
    }
    /// 只有主调度完毕后，才能调用的整理方法，必须保证调用时没有其他线程读写world
    pub fn settle(&mut self) {
        self.settle_by(&mut Vec::new(), &mut FixedBitSet::new())
    }
    /// 只有全部的插件都注册完毕，准备开始运行前调用。如果单独有个注册过程，则add_component_info等都使用&mut self。 则可以使用普通vec，不再需要整理
    pub fn init_ok(&mut self) {
        // todo 整理 self.listener_mgr.settle(0);
    }
    /// 只有主调度完毕后，才能调用的整理方法，必须保证调用时没有其他线程读写world
    pub fn settle_by(&mut self, action: &mut Vec<(Row, Row)>, set: &mut FixedBitSet) {
        // 整理实体
        self.entities.settle(0);
        // 整理原型数组
        self.archetype_arr.settle(0);
        let len = self.archetype_arr.len();
        if self.archetype_arr_len < len {
            // 原型增加，则整理所有的列
            for c in self.component_arr.iter_mut() {
                let c = unsafe { Share::get_mut_unchecked(c) };
                c.settle();
            }
            self.archetype_arr_len = len;
        }
        // 整理事件列表
        for aer in self.event_map.values_mut() {
            let er = unsafe { Share::get_mut_unchecked(aer) };
            er.settle();
        }
        // 整理每个原型
        for ar in self.archetype_arr.iter() {
            let archetype = unsafe { Share::get_mut_unchecked(ar) };
            archetype.settle(self, action, set);
        }
    }
}
unsafe impl Send for World {}
unsafe impl Sync for World {}
impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Downcast {
    fn into_any(self: Share<Self>) -> Share<dyn Any + Send + Sync>;
    fn into_box_any(self: Box<Self>) -> Box<dyn Any>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub trait TickMut: Downcast {
    fn get_tick(&self) -> Tick;
    fn set_tick(&mut self, tick: Tick);
}

pub trait Settle: Downcast {
    fn settle(&mut self);
}

/// Creates an instance of the type this trait is implemented for
/// using data from the supplied [World].
///
/// This can be helpful for complex initialization or context-aware defaults.
pub trait FromWorld {
    /// Creates `Self` using data from the given [World]
    fn from_world(world: &mut World) -> Self;
}

impl<T: Default> FromWorld for T {
    fn from_world(_world: &mut World) -> Self {
        T::default()
    }
}

pub trait SetFromWorld {
    fn set_fn() -> Option<fn(&mut World, *mut u8)>;
}
impl<T> SetFromWorld for T {
    default fn set_fn() -> Option<fn(&mut World, *mut u8)> {
        None
    }
}
impl<T: FromWorld> SetFromWorld for T {
    fn set_fn() -> Option<fn(&mut World, *mut u8)> {
        Some(|world, ptr| unsafe { ptr::write(ptr as *mut T, T::from_world(world)) })
    }
}


#[derive(Debug, Default, Clone, Copy)]
pub struct EntityAddr {
    index: ArchetypeIndex,
    pub(crate) row: Row,
}
unsafe impl Sync for EntityAddr {}
unsafe impl Send for EntityAddr {}

impl EntityAddr {
    #[inline(always)]
    pub(crate) fn new(index: ArchetypeIndex, row: Row) -> Self {
        EntityAddr {
            index,
            row,
        }
    }
    #[inline(always)]
    pub(crate) fn is_mark(&self) -> bool {
        self.index.0 < 0
    }
    #[inline(always)]
    pub(crate) fn mark(&mut self) {
        self.index = ArchetypeIndex(-self.index.0 - 1);
    }
    #[inline(always)]
    pub fn archetype_index(&self) -> ArchetypeIndex {
        if self.index.0 >= 0 || self.index.is_null() {
            self.index
        } else {
            ArchetypeIndex(-self.index.0 - 1)
        }
    }
}
