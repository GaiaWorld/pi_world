/// system上只能看到Query等SystemParam参数，SystemParam参数一般包含：单例和多例资源、实体、组件
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
use std::mem::transmute;
use std::ptr::{self, null_mut};
use std::sync::atomic::Ordering;

use crate::alter::{AlterState, Alterer, DelComponents};
use crate::archetype::{Archetype, ArchetypeWorldIndex, ComponentInfo, Row, ShareArchetype};
use crate::fetch::FetchComponents;
use crate::filter::FilterComponents;
use crate::insert::{InsertComponents, Inserter};
use crate::insert_batch::InsertBatchIter;
use crate::listener::{EventListKey, ListenerMgr};
use crate::query::{QueryError, QueryState, Queryer};
use crate::safe_vec::{SafeVec, SafeVecIter};
use dashmap::mapref::{entry::Entry, one::Ref};
use dashmap::DashMap;
use fixedbitset::FixedBitSet;
use pi_key_alloter::new_key_type;
use pi_null::Null;
use pi_share::Share;
use pi_slot::{Iter, SlotMap};

new_key_type! {
    pub struct Entity;
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
    pub(crate) single_res_map: DashMap<TypeId, SingleResource>,
    pub(crate) multi_res_map: DashMap<TypeId, MultiResource>,
    pub(crate) entities: SlotMap<Entity, EntityAddr>,
    pub(crate) archetype_map: DashMap<u128, ShareArchetype>,
    pub(crate) archetype_arr: SafeVec<ShareArchetype>,
    pub(crate) empty_archetype: ShareArchetype,
    pub(crate) listener_mgr: ListenerMgr,
    archetype_init_key: EventListKey,
    archetype_ok_key: EventListKey,
}
impl World {
    pub fn new() -> Self {
        let listener_mgr = ListenerMgr::default();
        let archetype_init_key = listener_mgr.init_register_event::<ArchetypeInit>();
        let archetype_ok_key = listener_mgr.init_register_event::<ArchetypeOk>();
        Self {
            single_res_map: DashMap::default(),
            multi_res_map: DashMap::default(),
            entities: SlotMap::default(),
            archetype_map: DashMap::new(),
            archetype_arr: SafeVec::default(),
            empty_archetype: ShareArchetype::new(Archetype::new(vec![])),
            listener_mgr,
            archetype_init_key,
            archetype_ok_key,
        }
    }
    /// 批量插入
    pub fn batch_insert<'w, I, Ins>(&'w mut self, iter: I) -> InsertBatchIter<'w, I, Ins>
    where
        I: Iterator<Item = <Ins as InsertComponents>::Item>,
        Ins: InsertComponents,
    {
        InsertBatchIter::new(self, iter.into_iter())
    }
    /// 创建一个插入器
    pub fn make_inserter<I: InsertComponents>(&mut self) -> Inserter<I> {
        let components = I::components();
        let id = ComponentInfo::calc_id(&components);
        let (ar_index, ar) = self.find_archtype(id, components);
        let s = I::init_state(self, &ar);
        Inserter::new(self, (ar_index, ar, s))
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
        A: InsertComponents + 'static,
        D: DelComponents + 'static,
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

    pub(crate) fn empty_archetype(&self) -> &ShareArchetype {
        &self.empty_archetype
    }
    pub fn len<'a>(&'a self) -> usize {
        self.entities.len()
    }

    pub fn entities_iter<'a>(&'a self) -> Iter<'a, Entity, EntityAddr> {
        self.entities.iter()
    }
    /// 获得指定的单例资源，为了安全，必须保证不在ECS执行中调用
    pub fn register_single_res<T: 'static>(&mut self, value: T) {
        let tid = TypeId::of::<T>();
        assert!(self
            .single_res_map
            .insert(tid, SingleResource::new(value))
            .is_none());
    }
    /// 获得指定的单例资源，为了安全，必须保证不在ECS执行中调用
    pub fn get_single_res<T: 'static>(&self) -> Option<&T> {
        unsafe { transmute(self.get_single_res_ptr::<T>()) }
    }
    /// 获得指定的单例资源，为了安全，必须保证不在ECS执行中调用
    pub fn get_single_res_mut<T: 'static>(&mut self) -> Option<&mut T> {
        unsafe { transmute(self.get_single_res_ptr::<T>()) }
    }
    pub(crate) fn get_single_res_ptr<T: 'static>(&self) -> *mut T {
        let tid = TypeId::of::<T>();
        self.single_res_map
            .get(&tid)
            .map_or(null_mut(), |r| unsafe {
                transmute(r.value().0.downcast_ref_unchecked::<T>())
            })
    }
    pub(crate) fn get_single_res_any(&self, tid: &TypeId) -> Option<SingleResource> {
        self.single_res_map.get(tid).map(|r| r.value().clone())
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
    ) -> Option<SingleResource>
    where
        F: FnOnce() -> T,
    {
        let tid = TypeId::of::<T>();
        self.multi_res_map.get_mut(&tid).map(|mut r| r.insert(f()))
    }
    /// 获得指定的多例资源，为了安全，必须保证不在ECS执行中调用
    pub fn get_multi_res<T: 'static>(&self, index: usize) -> Option<&T> {
        let tid = TypeId::of::<T>();
        self.multi_res_map
            .get(&tid)
            .map(|v| unsafe { transmute(v.get::<T>(index)) })
    }
    /// 获得指定的多例资源，为了安全，必须保证不在ECS执行中调用
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
        if let Some(c) = ar.get_column(tid) {
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
        self.entities.collect();
        self.archetype_arr.collect();
        for ar in self.archetype_arr.iter() {
            let archetype = unsafe { Share::get_mut_unchecked(ar) };
            archetype.collect(self, action, set)
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

#[derive(Debug, Clone)]
pub struct SingleResource(Share<dyn Any>, Cow<'static, str>);
impl SingleResource {
    fn new<T: 'static>(value: T) -> Self {
        Self(Share::new(value), std::any::type_name::<T>().into())
    }
    pub fn name(&self) -> &Cow<'static, str> {
        &self.1
    }
    pub(crate) fn downcast<T: 'static>(&self) -> *mut T {
        unsafe { transmute(self.0.downcast_ref_unchecked::<T>()) }
    }
}
unsafe impl Send for SingleResource {}
unsafe impl Sync for SingleResource {}

#[derive(Debug, Clone)]
pub struct MultiResource(
    Share<SyncUnsafeCell<Vec<Share<dyn Any>>>>,
    Cow<'static, str>,
);
impl MultiResource {
    fn new<T: 'static>() -> Self {
        Self(
            Share::new(SyncUnsafeCell::new(Vec::new())),
            std::any::type_name::<T>().into(),
        )
    }
    pub fn name(&self) -> &Cow<'static, str> {
        &self.1
    }
    pub fn insert<T: 'static>(&mut self, value: T) -> SingleResource {
        let r = Share::new(value);
        let vec = unsafe { &mut *self.0.get() };
        vec.push(r.clone());
        SingleResource(r, self.1.clone())
    }
    pub fn len(&self) -> usize {
        let vec = unsafe { &*self.0.get() };
        vec.len()
    }
    pub fn vec(&self) -> &Vec<Share<dyn Any>> {
        unsafe { &*self.0.get() }
    }
    pub(crate) fn get<T: 'static>(&self, index: usize) -> *mut T {
        let vec = unsafe { &*self.0.get() };
        vec.get(index).map_or(ptr::null_mut(), |r| unsafe {
            transmute(r.downcast_ref_unchecked::<T>())
        })
    }
    pub(crate) fn get_unchecked<T: 'static>(&self, index: usize) -> *mut T {
        let vec = unsafe { &*self.0.get() };
        unsafe { transmute(vec.get_unchecked(index).downcast_ref_unchecked::<T>()) }
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
