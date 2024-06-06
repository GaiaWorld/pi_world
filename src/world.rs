/// system上只能看到Query等SystemParm参数，SystemParm参数一般包含：单例和多例资源、实体、组件
/// world上包含了全部的资源和实体，及实体原型。 加一个监听管理器，
/// 查询过滤模块会注册监听器来监听新增的原型
/// world上的数据（资源、实体和原型）的线程安全的保护仅在于保护容器，
/// 由调度器生成的执行图，来保证正确的读写。
/// 比如一个原型不可被多线程同时读写，是由执行图时分析依赖，先执行写的sys，再执行读的sys。
/// 由于sys会进行组件的增删，导致实体对于的原型会变化，执行图可能会产生变化，执行图本身保证对原型的访问是安全的读写。
/// 整理操作时，一般是在整个执行图执行完毕后，进行进行相应的调整。举例：
///
/// 如果sys上通过Alter来增删组件，则可以在entity插入时，分析出sys的依赖。除了首次原型创建时，时序不确定，其余的增删，sys会保证先写后读。
/// 如果sys通过是MultiRes实现的CmdQueue来延迟动态增删组件，则sys就不会因此产生依赖，动态增删的结果就只能在可能在下一帧才会看到。
///
///



use crate::alter::{AlterState, Alterer};
use crate::archetype::{
    Archetype, ArchetypeInfo, ArchetypeIndex, ComponentInfo, Row, ShareArchetype,
    COMPONENT_CHANGED, COMPONENT_TICK,
};
use crate::column::{Column, ARCHETYPE_INDEX, COMPONENT_INDEX};
use crate::editor::{EditorState, EntityEditor};
use crate::fetch::{ColumnTick, FetchComponents};
use crate::filter::FilterComponents;
use crate::insert::{Bundle, Inserter};
use crate::insert_batch::InsertBatchIter;
use crate::listener::{EventListKey, ListenerMgr};
use crate::prelude::Mut;
use crate::query::{QueryError, QueryState, Queryer};
use crate::event::EventRecord;
use crate::safe_vec::{SafeVec, SafeVecIter};
use crate::system::{SystemMeta, TypeInfo};
use core::fmt::*;
use core::result::Result;
use std::collections::HashMap;
use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use fixedbitset::FixedBitSet;
use pi_append_vec::AppendVec;
use pi_key_alloter::new_key_type;
use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::cell::SyncUnsafeCell;
use std::mem::{self, transmute, ManuallyDrop};
use std::ops::Deref;
use std::ptr::{self, null_mut};
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
pub struct ArchetypeOk<'a>(
    pub &'a ShareArchetype,
    pub ArchetypeIndex,
    pub &'a World,
);

pub trait SetDefault {
    fn default_fn() -> Option<fn(*mut u8)>;
}
impl<T> SetDefault for T {
    default fn default_fn() -> Option<fn(*mut u8)> {
        None
    }
}
impl<T: Default> SetDefault for T {
    fn default_fn() -> Option<fn(*mut u8)> {
        Some(|ptr| unsafe { ptr::write(ptr as *mut T, T::default()) })
    }
}

pub struct World {
    pub(crate) single_res_map: DashMap<TypeId, (Option<SingleResource>, usize, Cow<'static, str>)>, // 似乎只需要普通hashmap
    pub(crate) single_res_arr: AppendVec<Option<SingleResource>>, // todo 改成AppendVec<SingleResource>
    pub(crate) multi_res_map: DashMap<TypeId, MultiResource>,
    pub(crate) event_map: HashMap<TypeId, Share<dyn EventRecord>>, // 事件表
    pub(crate) component_map: DashMap<TypeId, ComponentIndex>, // 似乎只需要普通hashmap
    pub(crate) component_arr: SafeVec<Share<Column>>, // todo 似乎只需要普通vec
    pub(crate) entities: SlotMap<Entity, EntityAddr>,
    pub(crate) archetype_map: DashMap<u64, ShareArchetype>,
    pub(crate) archetype_arr: SafeVec<ShareArchetype>,
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
                let r = r.split(",").map(|r| {r.parse::<usize>().unwrap()}).collect::<Vec<usize>>();
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
        let component_arr = SafeVec::with_capacity(1);
        let archetype_map = DashMap::new();
        archetype_map.insert(0, empty_archetype.clone());
        let archetype_arr = SafeVec::with_capacity(1);
        archetype_arr.insert(empty_archetype.clone());
        Self {
            single_res_map: DashMap::default(),
            single_res_arr: Default::default(),
            multi_res_map: DashMap::default(),
            event_map: Default::default(),
            entities: SlotMap::default(),
            component_map: DashMap::new(),
            component_arr,
            // component_removed_map: Default::default(),
            archetype_map,
            archetype_arr,
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
    /// 批量插入
    pub fn batch_insert<'w, I, Ins>(&'w mut self, iter: I) -> InsertBatchIter<'w, I, Ins>
    where
        I: Iterator<Item = Ins>,
        Ins: Bundle,
    {
        InsertBatchIter::new(self, iter.into_iter())
    }
    /// 创建一个插入器
    pub fn make_inserter<I: Bundle>(&mut self) -> Inserter<I> {
        let components = I::components(Vec::new());
        let ar = self.find_ar(components);
        let s = I::init_item(self, &ar);
        Inserter::new(self, (ar, s), self.tick())
    }

    /// 创建一个实体编辑器
    pub fn make_entity_editor(&mut self) -> EntityEditor {
        EntityEditor::new(self)
    }
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
            .map_or(ComponentIndex::null(), |r| *r.value())
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
    pub fn get_column(&self, index: ComponentIndex) -> Option<&Share<Column>> {
        self.component_arr.get(index.index())
    }
    /// 添加组件信息，如果重复，则返回原有的索引及是否tick变化 todo 改成mut
    pub fn add_component_info(&self, mut info: ComponentInfo) -> (ComponentIndex, Share<Column>) {
        let tick_info = info.tick_info;
        let index: ComponentIndex = match self.component_map.entry(*info.type_id()) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                let e = self.component_arr.alloc_entry();
                let index = e.index().into();
                println!("add component: {:?}", (info.type_name(), index));
                info.index = index;
                let c= Share::new(Column::new(info));
                e.insert(c.clone());
                entry.insert(index);
                return (index, c);
            }
        };
        let column = unsafe { self.component_arr.load_unchecked(index.index()) };
        let c = unsafe { Share::get_mut_unchecked(column) };
        let t = c.info.tick_info | tick_info;
        if t != c.info.tick_info {
            // 扫描当前列，将已有的实体设置tick，或放入到对应的事件列表中
            self.update_tick_info(c, t);
            c.info.tick_info = t;
        }
        (index, column.clone())
    }
    /// 扫描当前列，将已有的实体设置tick，或放入到对应的事件列表中
    pub(crate) fn update_tick_info(&self, c: &mut Column, tick_info: u8) {
        for _ar in self.archetype_arr.iter() {
            // todo 判断该原型含有该组件
            // if let Some((c, _)) = unsafe { ar.get_column_mut(index) } {
            //     let info = &c.info;
            //     let old = info.tick_info;
            //     if tick_info & COMPONENT_TICK != 0 && old & COMPONENT_TICK == 0 {
            //         let tick = self.increment_tick();
            //         // 将已经存在的实体修改tick
            //         for i in 0..ar.len().index() {
            //             *c.ticks.load_alloc(i) = tick;
            //         }
            //     }
            //     if tick_info & COMPONENT_CHANGED != 0 && old & COMPONENT_CHANGED == 0 {
            //         // 将已经存在的实体强行放入脏列表，因为添加新的组件信息时，监听器可能尚未安装
            //         for i in 0..ar.len().index() {
            //             let row = i.into();
            //             let e = ar.get(row);
            //             if !e.is_null() {
            //                 c.dirty.record_unchecked(e, row);
            //             }
            //         }
            //     }
            // }
        }
    }
    /// 计算所有原型信息，设置了所有组件的索引，按索引大小进行排序
    pub(crate) fn archetype_info(&self, components: Vec<ComponentInfo>) -> ArchetypeInfo {
        // for c in components.iter_mut() {
        //     let (index, change) = self.add_component_info(c.clone());
        //     c.world_index = index;
        //     if let Some(tick_removed) = change {
        //         c.tick_info = tick_removed;
        //     }
        // }
        let vec: Vec<Share<Column>> = components.into_iter().map(|c|{
            self.add_component_info(c).1
        }).collect();
        ArchetypeInfo::sort(vec)
    }
    /// 创建一个查询器
    pub fn make_queryer<Q: FetchComponents + 'static, F: FilterComponents + 'static>(
        &mut self,
    ) -> Queryer<Q, F> {
        let mut meta = SystemMeta::new(TypeInfo::of::<Queryer<Q, F>>());
        let mut state = QueryState::create(self, &mut meta);
        state.align(self);
        Queryer::new(self, state)
    }
    /// 创建一个改变器
    pub fn make_alterer<
        Q: FetchComponents + 'static,
        F: FilterComponents + 'static,
        A: Bundle + 'static,
        D: Bundle + 'static,
    >(
        &mut self,
    ) -> Alterer<Q, F, A, D> {
        let mut meta = SystemMeta::new(TypeInfo::of::<Queryer<Q, F>>());
        let mut query_state = QueryState::create(self, &mut meta);
        let mut alter_state =
            AlterState::make(self, A::components(Vec::new()), D::components(Vec::new()));
        query_state.align(self);
        // 将新多出来的原型，创建原型空映射
        alter_state.align(self,  &query_state.archetypes);
        Alterer::new(self, query_state, alter_state)
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
    /// 插入指定的单例资源，为了安全，必须保证不在ECS执行中调用，返回索引
    pub fn insert_single_res<T: 'static>(&mut self, value: T) -> usize {
        let tid = TypeId::of::<T>();
        let r = self.single_res_map.entry(tid).or_insert_with(|| {
            let r = SingleResource::new(value);
            let name = std::any::type_name::<T>().into();
            let index = self.single_res_arr.insert(Some(r.clone()));
            (Some(r), index, name)
        });
        r.value().1
    }

    // 如果不存在单例类型， 则注册指定的单例资源（不插入具体值，只添加类型），为了安全，必须保证不在ECS执行中调用，返回索引
    pub fn or_register_single_res(&mut self, type_info: TypeInfo) -> usize {
        let r = self
            .single_res_map
            .entry(type_info.type_id)
            .or_insert_with(|| {
                let index = self.single_res_arr.insert(None);
                (None, index, type_info.type_name)
            });
        r.value().1
    }

    /// 注册单例资源， 如果已经注册，则忽略，为了安全，必须保证不在ECS执行中调用，返回索引
    pub fn init_single_res<T: 'static + FromWorld>(&mut self) -> usize {
        let tid = TypeId::of::<T>();
        let mut index = 0;
        let mut is_add = true;
        if let Some(r) = self.single_res_map.get(&tid) {
            index = r.value().1;
            if r.value().0.is_some() {
                is_add = false;
            }
        }
        if is_add {
            let r = SingleResource::new(T::from_world(self));
            let name = std::any::type_name::<T>().into();
            let r = self.single_res_map.entry(tid).or_insert_with(|| {
                let index = self.single_res_arr.insert(Some(r.clone()));
                (Some(r), index, name)
            });
            index = r.value().1;
        }
        index
    }

    /// 用索引获得指定的单例资源，为了安全，必须保证不在ECS执行中调用
    /// todo!() 改成返回SingleRes
    #[inline]
    pub fn index_single_res<T: 'static>(&self, index: usize) -> Option<(&T, &Tick)> {
        unsafe { transmute(self.index_single_res_ptr::<T>(index)) }
    }
    /// 用索引获得指定的单例资源，为了安全，必须保证不在ECS执行中调用
    /// todo!() 改成返回SingleRes
    #[inline]
    pub fn index_single_res_mut<T: 'static>(
        &mut self,
        index: usize,
    ) -> Option<(&mut T, &mut Tick)> {
        unsafe { transmute(self.index_single_res_ptr::<T>(index)) }
    }
    #[inline]
    pub(crate) fn index_single_res_ptr<T: 'static>(&self, index: usize) -> (*mut T, *mut Tick) {
        self.single_res_arr
            .get(index)
            .map_or((null_mut(), null_mut()), |r| unsafe {
                match r {
                    Some(r) => (
                        transmute(r.0.downcast_ref_unchecked::<T>()),
                        transmute(&r.1),
                    ),
                    None => (null_mut(), null_mut()),
                }
            })
    }

    /// 获得指定的单例资源，为了安全，必须保证不在ECS执行中调用
    /// todo!() 改成返回SingleRes
    #[inline]
    pub fn get_single_res<T: 'static>(&self) -> Option<&T> {
        unsafe { transmute(self.get_single_res_ptr::<T>()) }
    }
    /// 获得指定的单例资源，为了安全，必须保证不在ECS执行中调用
    /// todo!() 改成返回SingleRes
    #[inline]
    pub fn get_single_res_mut<T: 'static>(&mut self) -> Option<&mut T> {
        unsafe { transmute(self.get_single_res_ptr::<T>()) }
    }
    #[inline]
    pub(crate) fn get_single_res_ptr<T: 'static>(&self) -> *mut T {
        let tid = TypeId::of::<T>();
        self.single_res_map
            .get(&tid)
            .map_or(null_mut(), |r| unsafe {
                match &r.value().0 {
                    Some(r) => transmute(r.0.downcast_ref_unchecked::<T>()),
                    None => null_mut(),
                }
            })
    }
    pub(crate) fn get_single_res_any(&self, tid: &TypeId) -> Option<SingleResource> {
        self.single_res_map
            .get(tid)
            .map_or(None, |r| r.value().0.clone())
    }
    pub(crate) fn index_single_res_any(&self, index: usize) -> Option<&mut SingleResource> {
        self.single_res_arr.load(index).map_or(None, |r| r.as_mut())
    }
    /// 注册指定类型的多例资源，为了安全，必须保证不在ECS执行中调用
    pub fn register_multi_res(&mut self, type_info: TypeInfo) {
        assert!(self
            .multi_res_map
            .insert(type_info.type_id, MultiResource::new(type_info.type_name))
            .is_none());
    }

    /// system系统读取多例资源
    pub(crate) fn system_read_multi_res(&self, tid: &TypeId) -> Option<MultiResource> {
        self.multi_res_map.get(&tid).map(|r| r.clone())
    }
    /// system系统初始化自己写入的多例资源
    pub(crate) fn system_init_write_multi_res<T: 'static, F>(
        &mut self,
        f: F,
    ) -> Option<(SingleResource, Share<ShareUsize>)>
    where
        F: FnOnce() -> T,
    {
        let tid = TypeId::of::<T>();
        self.multi_res_map
            .get_mut(&tid)
            .map(|mut r| (r.insert(f()), r.tick.clone()))
    }
    /// 获得指定的多例资源，为了安全，必须保证不在ECS执行中调用
    /// todo!() 改成返回SingleRes
    pub fn get_multi_res<T: 'static>(&self, index: usize) -> Option<&T> {
        let tid = TypeId::of::<T>();
        self.multi_res_map
            .get(&tid)
            .map(|v| unsafe { transmute(v.get::<T>(index)) })
    }
    /// 获得指定的多例资源，为了安全，必须保证不在ECS执行中调用
    /// todo!() 改成返回MultiResMut
    pub fn get_multi_res_mut<T: 'static>(&mut self, index: usize) -> Option<&mut T> {
        let tid = TypeId::of::<T>();
        self.multi_res_map
            .get(&tid)
            .map(|v| unsafe { transmute(v.get::<T>(index)) })
    }
    pub unsafe fn get_multi_res_unchecked<T: 'static>(&self, index: usize) -> Option<&T> {
        let tid = TypeId::of::<T>();
        self.multi_res_map
            .get(&tid)
            .map(|v| unsafe { transmute(v.get_unchecked::<T>(index)) })
    }
    /// 获得指定的多例资源，为了安全，必须保证不在ECS执行中调用
    pub unsafe fn get_multi_res_mut_unchecked<T: 'static>(
        &mut self,
        index: usize,
    ) -> Option<&mut T> {
        let tid = TypeId::of::<T>();
        self.multi_res_map
            .get(&tid)
            .map(|v| unsafe { transmute(v.get_unchecked::<T>(index)) })
    }
    // /// system初始化组件移除记录
    // pub(crate) fn init_component_removed_record(&mut self, index: ComponentIndex, type_name: Cow<'static, str>) -> Share<ComponentRemovedRecord> {
    //     let r = self.component_removed_map.entry(index).or_insert_with(|| {
    //         Share::new(ComponentRemovedRecord::new(type_name))
    //     });
    //     r.clone()
    // }
    // /// system初始化组件移除记录
    // pub(crate) fn get_component_removed_record(&self, index: ComponentIndex) -> Option<Share<ComponentRemovedRecord>> {
    //     self.component_removed_map.get(&index).map(|r| r.clone())
    // }
    /// 初始化事件记录
    pub(crate) fn init_event_record(&mut self, type_id: TypeId, event_record: Share<dyn EventRecord>) -> Share<dyn EventRecord> {
        let r = self.event_map.entry(type_id).or_insert_with(|| {
            event_record
        });
        r.clone()
    }
    /// 获得事件记录
    pub(crate) fn get_event_record(&self, type_id: &TypeId) -> Option<Share<dyn EventRecord>> {
        self.event_map.get(type_id).map(|r| r.clone())
    }

    /// 获得指定实体的指定组件，为了安全，必须保证不在ECS执行中调用
    pub fn get_component<T: 'static>(&self, e: Entity) -> Result<&T, QueryError> {
        let index = self.init_component::<T>();
        self.get_component_by_index(e, index)
    }
    /// 获得指定实体的指定组件，为了安全，必须保证不在ECS执行中调用
    pub fn get_component_mut<T: 'static>(
        &mut self,
        e: Entity,
    ) -> Result<Mut<'static, T>, QueryError> {
        //    self.get_component_info(index)
        let index = self.init_component::<T>();
        self.get_component_mut_by_index(e, index)
    }

    // /// 获得指定实体的指定组件，为了安全，必须保证不在ECS执行中调用
    // pub(crate) fn get_component_mut1<T: 'static>(
    //     &mut self,
    //     e: Entity,
    // ) -> Result<Mut<'static, T>, QueryError> {
    //     let index = self.init_component::<T>();
    //     self.get_component_mut_index_impl(e, index)
    // }

    // pub(crate) fn get_component_mut_index_impl<T: 'static>(
    //     &self,
    //     e: Entity,
    //     index: ComponentIndex,
    // ) -> Result<Mut<'static, T>, QueryError> {
    //     let (_ptr, row) =  self.get_component_ptr_by_index(e, index)?;
    //     let t = self.tick();
    //     let value: Mut<T> = Mut::new(&ColumnTick::new(c, t, t), e, row);
    //     Ok(unsafe { transmute(value) })
    // }

    fn get_component_ptr_by_index(
        &self,
        e: Entity,
        index: ComponentIndex,
    ) -> Result<(*mut u8, Row), QueryError> {
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
            Some(c) => Ok((c.get_row(addr.row), addr.row)),
            None => Err(QueryError::MissingComponent(index, addr.archetype_index())),
        }
    }

    /// 获得指定实体的指定组件，为了安全，必须保证不在ECS执行中调用
    pub fn get_component_by_index<T: 'static>(
        &self,
        e: Entity,
        index: ComponentIndex,
    ) -> Result<&T, QueryError> {
        let (ptr, _row) =  self.get_component_ptr_by_index(e, index)?;
        Ok(unsafe { transmute(ptr) })
    }
    /// 获得指定实体的指定组件，为了安全，必须保证不在ECS执行中调用
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

    // /// 增加和删除实体
    // pub fn alter_components_by_index(
    //     &mut self,
    //     e: Entity,
    //     components: &[(ComponentIndex, bool)],
    // ) -> Result<(), QueryError> {
    //     let mut components = components.to_vec();
    //     components.reverse(); // 相同ComponentIndex的多个增删操作，让最后的操作执行
    //     components.sort_by(|a, b| a.cmp(b)); // 只比较ComponentIndex，并且保持原始顺序的排序
    //     let components = components.as_slice();

    //     let addr = match self.entities.get(e) {
    //         Some(v) => v,
    //         None => return Err(QueryError::NoSuchEntity),
    //     };

    //     let ar_index = addr.archetype_index();
    //     let mut ar = self.empty_archetype();

    //     if !addr.index.is_null() {
    //         ar = unsafe { self.archetype_arr.get_unchecked(ar_index as usize)};
    //         let ae = ar.mark_remove(addr.row);
    //         if e != ae {
    //             return Err(QueryError::NoMatchEntity(ae));
    //         }
    //     }

    //     // let mut sort_add = vec![];
    //     // let mut sort_remove = vec![];

    //     // TODO, 性能
    //     // let mut array =  Vec::new();
    //     // for (index, is_add) in components.iter() {
    //     //     let v = if *is_add {
    //     //         1u16
    //     //     } else {
    //     //         0
    //     //     };
    //     //     // 去除已经有了需要添加的和没有需要删除的组件
    //     //     let column = ar.get_column_index(*index);
    //     //     if *is_add == column.is_null() {
    //     //         array.insert_value(index.index(), v);
    //     //     }
    //     // }
    //     // for index in 0..array.len() {
    //     //     if array[index] != u16::MAX{
    //     //         if let Some(info) = self.get_component_info(index.into()){
    //     //             if array[index] == 0{
    //     //                 sort_remove.push(info.clone());
    //     //             } else if array[index] == 1{
    //     //                 // if ar.get(row)
    //     //                 sort_add.push(info.clone());
    //     //             }
    //     //         }
    //     //     }
    //     // }
    //     // sort_add.sort();
    //     // sort_remove.sort();
    //     // let mut id = ComponentInfo::calc_id(&sort_add);

    //     // println!("components: {:?}", components);
    //     // if sort_add.len() > 0{

    //     // }
    //     let mut mapping = ArchetypeMapping::new(ar.clone(), self.empty_archetype().clone());
    //     // println!("mapping1: {:?}", mapping);
    //     // let mut moved_columns = vec![];
    //     // let mut added_columns = vec![];
    //     // let mut removed_columns = vec![];
    //     let mut adding = Default::default();
    //     let mut moving = Default::default();
    //     let mut removing = Default::default();
    //     let mut removed_columns = Default::default();
    //     let mut move_removed_columns = Default::default();

    //     mapping_init(
    //         self,
    //         &mut mapping,
    //         components,
    //         &mut adding,
    //         &mut moving,
    //         &mut removing,
    //         &mut removed_columns,
    //         &mut move_removed_columns,
    //         true,
    //     );
    //     // println!("moved_columns: {:?}", moved_columns);
    //     // println!("added_columns: {:?}", added_columns);
    //     // println!("removed_columns: {:?}", removed_columns);

    //     let _ = alloc_row(&mut mapping, addr.row, e);
    //     // let (_add_index, add)  = self.find_ar(sort_add);

    //     // for col in add.get_columns().iter() {
    //     //     col.add_record(e, dst_row, self.tick());
    //     // }
    //     // log::warn!("mapping3: {:?}, {:?}, {:?}, {:?}, =={:?}", e, addr.row, dst_row, mapping.src.name(), mapping.dst.name());
    //     // 处理标记移除的条目， 将要移除的组件释放，将相同的组件拷贝
    //     let tick = self.tick();
    //     insert_columns(&mut mapping, &adding, tick.clone());
    //     move_columns(&mut mapping, &moving);
    //     remove_columns(&mut mapping, &removed_columns, tick);
    //     move_remove_columns(&mut mapping, &move_removed_columns);
    //     // add_columns(&mut mapping, self.tick());
    //     update_table_world(&self, &mut mapping);

    //     Ok(())
    // }

    // /// 获得指定实体的指定组件，为了安全，必须保证不在ECS执行中调用
    // pub(crate) fn get_component_ptr<T: 'static>(&self, e: Entity) -> Result<&mut T, QueryError> {
    //     unsafe { transmute(self.get_component_ptr_by_tid(e, &TypeId::of::<T>())) }
    // }
    // /// 获得指定实体的指定组件，为了安全，必须保证不在ECS执行中调用
    // pub(crate) fn get_component_ptr_by_tid(
    //     &self,
    //     e: Entity,
    //     tid: &TypeId,
    // ) -> Result<*mut u8, QueryError> {
    //     let addr = match self.entities.get(e) {
    //         Some(v) => v,
    //         None => return Err(QueryError::NoSuchEntity),
    //     };
    //     let ar = unsafe {
    //         self.archetype_arr
    //             .get_unchecked(addr.archetype_index().index())
    //     };
    //     let index = self.get_component_index(tid);
    //     if let Some((c, _)) = ar.get_column(index) {
    //         Ok(c.get_row(addr.row))
    //     } else {
    //         Err(QueryError::MissingComponent)
    //     }
    // }

    // pub fn get_archetype(&self, id: u128) -> Option<Ref<u128, ShareArchetype>> {
    //     self.archetype_map.get(&id)
    // }
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
    pub(crate) fn find_ar(
        &self,
        infos: Vec<ComponentInfo>,
    ) -> ShareArchetype {
        let info = self.archetype_info(infos);
        self.find_archtype(info)
    }
    // 返回原型及是否新创建
    pub(crate) fn find_archtype(
        &self,
        info: ArchetypeInfo,
    ) -> ShareArchetype {
        // 如果world上没有找到对应的原型，则创建并放入world中
        let (mut ar, b) = match self.archetype_map.entry(info.hash) {
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
            println!("add archtype: {:?}", (ar.name(), ar_index));
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
    /// 插入一个新的Entity
    #[inline(always)]
    pub(crate) fn insert(&self, ar_index: ArchetypeIndex, row: Row) -> Entity {
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

    /// 创建一个新的实体
    pub(crate) fn alloc_entity(&self) -> Entity {
        self.entities
            .insert(EntityAddr::new(0usize.into(), Row::null()))
    }
    /// 初始化指定组件
    pub fn init_component<T: 'static>(&self) -> ComponentIndex {
        self.add_component_info(ComponentInfo::of::<T>(0)).0
    }
    /// 替换Entity的原型及行
    #[inline(always)]
    pub(crate) fn replace_row(&self, e: Entity, row: Row) {
        let addr = unsafe { self.entities.load_unchecked(e) };
        addr.row = row;
    }
    /// 只有主调度完毕后，才能调用的整理方法，必须保证调用时没有其他线程读写world
    pub fn settle(&mut self) {
        self.settle_by(&mut Vec::new(), &mut FixedBitSet::new())
    }
    /// 只有全部的插件都注册完毕，准备开始运行前调用。如果单独有个注册过程，则add_component_info等都使用&mut self。 则可以使用普通vec，不再需要整理
    pub fn init_ok(&mut self) {
        self.single_res_arr.settle(0);
        // todo 整理 self.listener_mgr.settle(0);
    }
    /// 只有主调度完毕后，才能调用的整理方法，必须保证调用时没有其他线程读写world
    pub fn settle_by(&mut self, action: &mut Vec<(Row, Row)>, set: &mut FixedBitSet) {
        println!("World settle_by: {:?}", self.tick());
        // 整理实体
        self.entities.settle(0);
        // 整理原型数组
        self.archetype_arr.settle(0);
        // 整理列数组
        for c in self.component_arr.iter() {
            let c = unsafe { Share::get_mut_unchecked(c) };
            println!("settle_by archetype_arr: {:?}", (c.info.index, c.arr.vec_capacity(), self.archetype_arr.len()));
            c.arr.settle(self.archetype_arr.len(), 0, 1);
            println!("settle_by111111111 archetype_arr: {:?}", (c.info.index, c.arr.vec_capacity()));
        }
        // 整理事件列表
        for aer in self.event_map.values_mut() {
            let er = unsafe { Share::get_mut_unchecked(aer) };
            er.settle();
        }
        // 整理每个原型
        for ar in self.archetype_arr.iter() {
            let archetype = unsafe { Share::get_mut_unchecked(ar) };
            archetype.settle(self, action, set)
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

#[derive(Debug, Clone)]
pub struct SingleResource(Share<dyn Any>, pub(crate) Tick);
impl SingleResource {
    fn new<T: 'static>(value: T) -> Self {
        Self(Share::new(value), Tick::default())
    }
    // pub fn name(&self) -> &Cow<'static, str> {
    //     &self.1
    // }
    pub(crate) fn downcast<T: 'static>(&self) -> *mut T {
        unsafe { transmute(self.0.downcast_ref_unchecked::<T>()) }
    }
}
unsafe impl Send for SingleResource {}
unsafe impl Sync for SingleResource {}

#[derive(Debug, Clone)]
pub struct MultiResource{
    vec: Share<SyncUnsafeCell<Vec<SingleResource>>>,
    tick: Share<ShareUsize>,
    name: Cow<'static, str>,
}
impl MultiResource {
    fn new(name: Cow<'static, str>) -> Self {
        Self{
            vec: Share::new(SyncUnsafeCell::new(Vec::new())),
            tick: Share::new(ShareUsize::new(0)),
            name,
        }
    }
    pub fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
    pub fn insert<T: 'static>(&mut self, value: T) -> SingleResource {
        let r = SingleResource::new(value);
        let vec = unsafe { &mut *self.vec.get() };
        vec.push(r.clone());
        r
    }
    pub fn len(&self) -> usize {
        let vec = unsafe { &*self.vec.get() };
        vec.len()
    }
    pub fn vec(&self) -> &Vec<SingleResource> {
        unsafe { &*self.vec.get() }
    }
    pub fn changed_tick(&self) -> Tick {
        self.tick.load(Ordering::Relaxed).into()
    }
    pub(crate) fn get<T: 'static>(&self, index: usize) -> *mut T {
        let vec = unsafe { &*self.vec.get() };
        vec.get(index).map_or(ptr::null_mut(), |r| r.downcast())
    }
    pub(crate) fn get_unchecked<T: 'static>(&self, index: usize) -> *mut T {
        let vec = unsafe { &*self.vec.get() };
        unsafe { vec.get_unchecked(index).downcast() }
    }
}

unsafe impl Send for MultiResource {}
unsafe impl Sync for MultiResource {}

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
