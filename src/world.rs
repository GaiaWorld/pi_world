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
use core::fmt::*;
use core::result::Result;
use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::cell::SyncUnsafeCell;
use std::mem::{transmute, ManuallyDrop};
use std::ops::Deref;
use std::ptr::{self, null_mut};
use std::sync::atomic::Ordering;

use crate::alter::{alter_row, clear, mapping_init, AlterState, Alterer, ArchetypeMapping};
use crate::archetype::{Archetype, ArchetypeWorldIndex, ComponentInfo, Row, ShareArchetype};
use crate::fetch::FetchComponents;
use crate::filter::FilterComponents;
use crate::insert::{Bundle, Inserter};
use crate::insert_batch::InsertBatchIter;
use crate::listener::{EventListKey, ListenerMgr};
use crate::query::{check, QueryError, QueryState, Queryer};
use crate::safe_vec::{SafeVec, SafeVecIter};
use dashmap::mapref::{entry::Entry, one::Ref};
use dashmap::DashMap;
use fixedbitset::FixedBitSet;
use pi_key_alloter::new_key_type;
use pi_null::Null;
use pi_share::{Share, ShareU32};
use pi_slot::{Iter, SlotMap};

new_key_type! {
    pub struct Entity;
}

pub type ComponentIndex = u32;

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(u32);
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
impl From<Tick> for u32 {
    fn from(value: Tick) -> Self {
        value.0
    }
}

#[derive(Clone, Debug)]
pub struct ArchetypeInit<'a>(pub &'a ShareArchetype, pub &'a World);
#[derive(Clone, Debug)]
pub struct ArchetypeOk<'a>(
    pub &'a ShareArchetype,
    pub ArchetypeWorldIndex,
    pub &'a World,
);

#[derive(Debug)]
pub struct World {
    pub(crate) single_res_map: DashMap<TypeId, (Option<SingleResource>, usize, Cow<'static, str>)>,
    pub(crate) single_res_arr: SafeVec<Option<SingleResource>>,
    pub(crate) multi_res_map: DashMap<TypeId, MultiResource>,
    pub(crate) component_map: DashMap<TypeId, ComponentIndex>,
    pub(crate) component_arr: SafeVec<ComponentInfo>,
    pub(crate) entities: SlotMap<Entity, EntityAddr>,
    pub(crate) archetype_map: DashMap<u128, ShareArchetype>,
    pub(crate) archetype_arr: SafeVec<ShareArchetype>,
    pub(crate) empty_archetype: ShareArchetype,
    pub(crate) listener_mgr: ListenerMgr,
    archetype_init_key: EventListKey,
    archetype_ok_key: EventListKey,
    // 世界当前的tick
    tick: ShareU32,
}
impl World {
    pub fn new() -> Self {
        let listener_mgr = ListenerMgr::default();
        let archetype_init_key = listener_mgr.init_register_event::<ArchetypeInit>();
        let archetype_ok_key = listener_mgr.init_register_event::<ArchetypeOk>();
        let empty_archetype = ShareArchetype::new(Archetype::new(vec![]));
        let component_arr = SafeVec::with_capacity(1);
        let archetype_arr = SafeVec::with_capacity(1);
        archetype_arr.insert(empty_archetype.clone());
        Self {
            single_res_map: DashMap::default(),
            single_res_arr: SafeVec::default(),
            multi_res_map: DashMap::default(),
            entities: SlotMap::default(),
            component_map: DashMap::new(),
            component_arr,
            archetype_map: DashMap::new(),
            archetype_arr,
            empty_archetype,
            listener_mgr,
            archetype_init_key,
            archetype_ok_key,
            tick: ShareU32::new(1),
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
        I: Iterator<Item = <Ins as Bundle>::Item>,
        Ins: Bundle,
    {
        InsertBatchIter::new(self, iter.into_iter())
    }
    /// 创建一个插入器
    pub fn make_inserter<I: Bundle>(&mut self) -> Inserter<I> {
        let components = I::components();
        let id = ComponentInfo::calc_id(&components);
        let (ar_index, ar) = self.find_archtype(id, components);
        let s = I::init_state(self, &ar);
        Inserter::new(self, (ar_index, ar, s), self.tick())
    }

    /// 是否存在实体
    pub fn contains(&self, entity: Entity) -> bool {
        self.entities.contains_key(entity)
    }
    /// 获得指定组件的索引
    pub fn get_component_index(&self, component_type_id: &TypeId) -> ComponentIndex {
        self.component_map.get(component_type_id).map_or(ComponentIndex::null(), |r| *r.value())
    }
    /// 获得指定组件的索引
    pub fn get_component_info(&self, index: ComponentIndex) -> Option<&ComponentInfo> {
        self.component_arr.get(index as usize)
    }
    /// 添加组件信息，如果重复，则返回原有的索引
    pub fn add_component_info(&self, info: ComponentInfo) -> ComponentIndex {
        let r = self.component_map.entry(info.type_id).or_insert_with(|| {
            self.component_arr.insert(info) as ComponentIndex
        });
        *r.value()
    }
    /// 创建一个查询器
    pub fn make_queryer<Q: FetchComponents + 'static, F: FilterComponents + 'static>(
        &self,
    ) -> Queryer<Q, F> {
        let mut state = QueryState::create(self);
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
        let mut query_state = QueryState::create(self);
        let mut alter_state = AlterState::new(A::components(), D::components());
        query_state.align(self);
        // 将新多出来的原型，创建原型空映射
        Alterer::<Q, F, A, D>::state_align(self, &mut alter_state, &query_state);
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
    pub fn or_register_single_res<T: 'static>(&mut self) -> usize {
        let tid = TypeId::of::<T>();
        let r = self.single_res_map.entry(tid).or_insert_with(|| {
            let name = std::any::type_name::<T>().into();
            let index = self.single_res_arr.insert(None);
            (None, index, name)
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
    pub fn register_multi_res<T: 'static>(&mut self) {
        let tid = TypeId::of::<T>();
        assert!(self
            .multi_res_map
            .insert(tid, MultiResource::new::<T>())
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
    ) -> Option<(SingleResource, Share<ShareU32>)>
    where
        F: FnOnce() -> T,
    {
        let tid = TypeId::of::<T>();
        self.multi_res_map
            .get_mut(&tid)
            .map(|mut r| (r.insert(f()), r.1.clone()))
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

    /// 获得指定实体的指定组件，为了安全，必须保证不在ECS执行中调用
    pub fn get_component<T: 'static>(&self, e: Entity) -> Result<&T, QueryError> {
        unsafe { transmute(self.get_component_ptr::<T>(e)) }
    }
    /// 获得指定实体的指定组件，为了安全，必须保证不在ECS执行中调用
    pub fn get_component_mut<T: 'static>(&mut self, e: Entity) -> Result<&mut T, QueryError> {
        self.get_component_ptr::<T>(e)
    }
    pub fn alter_components(
        &mut self,
        e: Entity,
        components: &[(TypeId, bool)],
    ) -> Result<bool, QueryError> {
        todo!()
    }
    /// 获得指定实体的指定组件，为了安全，必须保证不在ECS执行中调用
    pub fn add_component<T: Bundle + 'static>(&self, e: Entity, value: T::Item) -> Result<(), QueryError> {
        // todo!()
        let components = T::components();
        let addr = match self.entities.get(e) {
            Some(v) => v,
            None => return Err(QueryError::NoSuchEntity),
        };
        let mut id: u128 = ComponentInfo::calc_id(&components);
        let ar_index = addr.archetype_index();
        let ar = unsafe { self.archetype_arr.get_unchecked(ar_index) };
        let (components, moving) = ar.alter(&components, &vec![]);

        let dst = self.find_archtype(id, components);
        let mut mapping = ArchetypeMapping::new(ar.clone(), dst.1);

        let mut moved_columns = vec![];
        let mut added_columns = vec![];
        let mut removed_columns = vec![];

        if mapping.dst.len() > 0{
            mapping_init(
                self,
                &mut mapping,
                &mut moved_columns,
                &mut added_columns,
                &mut removed_columns,
                &T::components(),
                &vec![],
                &mut id,
            );
        }
       let mut mapping_dirtys = vec![];
        let dst_row = alter_row(&mut mapping_dirtys, &mut mapping, ar_index, addr.row)?;
        let state = T::init_state(self, &ar);

        T::insert(
            &state,
            value,
            e,
            dst_row,
            self.tick(),
        );

        // clear(
        //     self,
        //     &mut self.state.vec,
        //     &mut mapping_dirtys,
        //     &moved_columns,
        //     &added_columns,
        //     &removed_columns,
        //     self.tick(),
        // );

        Ok(())
        // 原型改变
    }
    /// 获得指定实体的指定组件，为了安全，必须保证不在ECS执行中调用
    pub fn remove_component<T: Bundle + 'static>(&self, e: Entity) -> T {
        todo!()
        // 原型改变
    }
    /// 获得指定实体的指定组件，为了安全，必须保证不在ECS执行中调用
    pub(crate) fn get_component_ptr<T: 'static>(&self, e: Entity) -> Result<&mut T, QueryError> {
        unsafe { transmute(self.get_component_ptr_by_tid(e, &TypeId::of::<T>())) }
    }
    /// 获得指定实体的指定组件，为了安全，必须保证不在ECS执行中调用
    pub(crate) fn get_component_ptr_by_tid(
        &self,
        e: Entity,
        tid: &TypeId,
    ) -> Result<*mut u8, QueryError> {
        let addr = match self.entities.get(e) {
            Some(v) => v,
            None => return Err(QueryError::NoSuchEntity),
        };
        let ar = unsafe { self.archetype_arr.get_unchecked(addr.archetype_index()) };
        let index = self.get_component_index(tid);
        if let Some((c, _)) = ar.get_column(index) {
            Ok(c.get_row(addr.row))
        } else {
            Err(QueryError::MissingComponent)
        }
    }

    pub fn get_archetype(&self, id: u128) -> Option<Ref<u128, ShareArchetype>> {
        self.archetype_map.get(&id)
    }
    pub fn index_archetype(&self, index: ArchetypeWorldIndex) -> Option<&ShareArchetype> {
        self.archetype_arr.get(index as usize)
    }
    pub fn archetype_list<'a>(&'a self) -> SafeVecIter<'a, ShareArchetype> {
        self.archetype_arr.iter()
    }

    // 返回原型及是否新创建
    pub(crate) fn find_archtype(
        &self,
        id: u128,
        components: Vec<ComponentInfo>,
    ) -> (ArchetypeWorldIndex, ShareArchetype) {
        // 如果world上没有找到对应的原型，则创建并放入world中
        let (ar, b) = match self.archetype_map.entry(id) {
            Entry::Occupied(entry) => (entry.get().clone(), false),
            Entry::Vacant(entry) => {
                let ar = Share::new(Archetype::new(components));
                entry.insert(ar.clone());
                (ar, true)
            }
        };
        if b {
            // 通知原型创建，让各查询过滤模块初始化原型的记录列表，通知执行图更新
            self.listener_mgr
                .notify_event(self.archetype_init_key, ArchetypeInit(&ar, &self));
            // 通知后，让原型就绪， 其他线程也就可以获得该原型
            let ar_index = self.archtype_ok(&ar);
            self.listener_mgr
                .notify_event(self.archetype_ok_key, ArchetypeOk(&ar, ar_index, &self));
            (ar_index, ar)
        } else {
            // 循环等待原型就绪
            loop {
                let index = ar.index();
                if index.is_null() {
                    std::hint::spin_loop();
                }
                return (index, ar);
            }
        }
    }
    // 先事件通知调度器，将原型放入数组，之后其他system可以看到该原型
    pub(crate) fn archtype_ok(&self, ar: &ShareArchetype) -> ArchetypeWorldIndex {
        let entry = self.archetype_arr.alloc_entry();
        let index = entry.index() as u32;
        ar.index.store(index, Ordering::Relaxed);
        entry.insert(ar.clone()); // 确保其他线程一定可以看见
        index
    }
    /// 插入一个新的Entity
    #[inline(always)]
    pub(crate) fn insert(&self, ar_index: ArchetypeWorldIndex, row: Row) -> Entity {
        self.entities.insert(EntityAddr::new(ar_index, row))
    }
    /// 替换Entity的原型及行
    #[inline(always)]
    pub(crate) fn replace(&self, e: Entity, ar_index: ArchetypeWorldIndex, row: Row) {
        let addr = unsafe { self.entities.load_unchecked(e) };
        addr.index = ar_index;
        addr.row = row;
    }
    /// 判断指定的实体是否存在
    pub fn contains_entity(&self, e: Entity) -> bool {
        self.entities.get(e).is_some()
    }
    /// 销毁指定的实体
    pub fn destroy_entity(&mut self, e: Entity) -> Result<(), QueryError> {
        let addr = match self.entities.get(e) {
            Some(v) => *v,
            None => return Err(QueryError::NoSuchEntity),
        };
        let ar = unsafe { self.archetype_arr.get_unchecked(addr.index as usize) };
        let e = ar.mark_destroy(addr.row);
        if e.is_null() {
            return Err(QueryError::NoSuchRow);
        }
        self.entities.remove(e).unwrap();
        Ok(())
    }
    /// 替换Entity的原型及行
    #[inline(always)]
    pub(crate) fn replace_row(&self, e: Entity, row: Row) {
        let addr = unsafe { self.entities.load_unchecked(e) };
        addr.row = row;
    }
    /// 只有主调度完毕后，才能调用的整理方法，必须保证调用时没有其他线程读写world
    pub fn collect(&mut self) {
        self.collect_by(&mut Vec::new(), &mut FixedBitSet::new())
    }
    /// 只有主调度完毕后，才能调用的整理方法，必须保证调用时没有其他线程读写world
    pub fn collect_by(&mut self, action: &mut Vec<(Row, Row)>, set: &mut FixedBitSet) {
        // self.entities.collect();
        // self.archetype_arr.collect();
        // for ar in self.archetype_arr.iter() {
        //     let archetype = unsafe { Share::get_mut_unchecked(ar) };
        //     archetype.collect(self, action, set)
        // }
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
pub struct MultiResource(
    Share<SyncUnsafeCell<Vec<SingleResource>>>,
    pub(crate) Share<ShareU32>,
    Cow<'static, str>,
);
impl MultiResource {
    fn new<T: 'static>() -> Self {
        Self(
            Share::new(SyncUnsafeCell::new(Vec::new())),
            Share::new(ShareU32::new(0)),
            std::any::type_name::<T>().into(),
        )
    }
    pub fn name(&self) -> &Cow<'static, str> {
        &self.2
    }
    pub fn insert<T: 'static>(&mut self, value: T) -> SingleResource {
        let r = SingleResource::new(value);
        let vec = unsafe { &mut *self.0.get() };
        vec.push(r.clone());
        r
    }
    pub fn len(&self) -> usize {
        let vec = unsafe { &*self.0.get() };
        vec.len()
    }
    pub fn vec(&self) -> &Vec<SingleResource> {
        unsafe { &*self.0.get() }
    }
    pub fn changed_tick(&self) -> Tick {
        self.1.load(Ordering::Relaxed).into()
    }
    pub(crate) fn get<T: 'static>(&self, index: usize) -> *mut T {
        let vec = unsafe { &*self.0.get() };
        vec.get(index).map_or(ptr::null_mut(), |r| r.downcast())
    }
    pub(crate) fn get_unchecked<T: 'static>(&self, index: usize) -> *mut T {
        let vec = unsafe { &*self.0.get() };
        unsafe { vec.get_unchecked(index).downcast() }
    }
}

unsafe impl Send for MultiResource {}
unsafe impl Sync for MultiResource {}

#[derive(Debug, Default, Clone, Copy)]
pub struct EntityAddr {
    pub(crate) index: ArchetypeWorldIndex,
    pub(crate) row: Row,
}
unsafe impl Sync for EntityAddr {}
unsafe impl Send for EntityAddr {}

impl EntityAddr {
    #[inline(always)]
    pub fn new(ar_index: ArchetypeWorldIndex, row: Row) -> Self {
        EntityAddr {
            index: ar_index,
            row,
        }
    }
    #[inline(always)]
    pub fn archetype_index(&self) -> usize {
        self.index as usize
    }
}
