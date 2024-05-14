//! 查询，支持子查询

use core::fmt::*;
use core::result::Result;
use std::any::TypeId;
use std::borrow::Cow;
use std::cell::UnsafeCell;
// use std::collections::HashMap;
use std::marker::PhantomData;
use std::mem::{transmute, MaybeUninit};

use crate::archetype::{
    Archetype, ArchetypeDepend, ArchetypeDependResult, ArchetypeWorldIndex, Flags, Row,
    ShareArchetype,
};
use crate::dirty::{DirtyIndex, EntityRow};
use crate::fetch::FetchComponents;
use crate::filter::{FilterComponents, ListenType};
use crate::listener::Listener;
use crate::param_set::ParamSetElement;
use crate::system::SystemMeta;
use crate::system_params::SystemParam;
use crate::utils::VecExt;
use crate::world::*;
use fixedbitset::FixedBitSet;
use pi_arr::Iter;
use pi_null::*;
use pi_share::Share;
use smallvec::SmallVec;

#[derive(Debug, PartialEq, Eq)]
pub enum QueryError {
    MissingComponent,
    NoSuchArchetype,
    NoMatchEntity,
    NoSuchEntity,
    NoSuchRow,
    NoSuchRes,
}

pub struct Queryer<'world, Q: FetchComponents + 'static, F: FilterComponents + 'static = ()> {
    pub(crate) world: &'world World,
    pub(crate) state: QueryState<Q, F>,
    pub(crate) tick: Tick,
    // 缓存上次的索引映射关系
    pub(crate) cache_mapping: UnsafeCell<(ArchetypeWorldIndex, ArchetypeLocalIndex)>,
}
impl<'world, Q: FetchComponents + 'static, F: FilterComponents + 'static> Queryer<'world, Q, F> {
    pub(crate) fn new(world: &'world World, state: QueryState<Q, F>) -> Self {
        let tick = world.increment_tick();
        let cache_mapping = UnsafeCell::new((ArchetypeWorldIndex::null(), ArchetypeLocalIndex(0)));
        Self {
            world,
            state,
            tick,
            cache_mapping,
        }
    }
    
    pub fn contains(&self, entity: Entity) -> bool {
        if let Ok( (_addr, world_index, local_index)) = check(
            self.world,
            entity,
            // unsafe { &mut *self.cache_mapping.get() },
            &self.state.map,
        ){
            unsafe { *self.cache_mapping.get()  = (world_index, local_index)};
            return true;
        }else{
            return false;
        }
    }
    
    pub fn get(
        &self,
        e: Entity,
    ) -> Result<<<Q as FetchComponents>::ReadOnly as FetchComponents>::Item<'_>, QueryError> {
        // let  (_addr, world_index, local_index) = check(
        //     self.world,
        //     entity,
        //     // unsafe { &mut *self.cache_mapping.get() },
        //     &self.state.map,
        // )?;
        self.state
            .as_readonly()
            .get(self.world, self.tick, e, /* unsafe {
                &mut *self.cache_mapping.get()
            } */)
    }
    
    pub fn get_mut(&mut self, e: Entity) -> Result<<Q as FetchComponents>::Item<'_>, QueryError> {
        self.state
            .get(self.world, self.tick, e, /* self.cache_mapping.get_mut() */)
    }
    
    pub fn is_empty(&self) -> bool {
        self.state.is_empty()
    }
    
    pub fn len(&self) -> usize {
        self.state.len()
    }
    
    pub fn iter(&self) -> QueryIter<'_, <Q as FetchComponents>::ReadOnly, F> {
        QueryIter::new(self.world, self.state.as_readonly(), self.tick)
    }
    pub fn iter_mut(&mut self) -> QueryIter<'_, Q, F> {
        QueryIter::new(self.world, &self.state, self.tick)
    }
}

pub struct Query<'world, Q: FetchComponents + 'static, F: FilterComponents + 'static = ()> {
    pub(crate) world: &'world World,
    pub(crate) state: &'world mut QueryState<Q, F>,
    pub(crate) tick: Tick,
    // 缓存上次的索引映射关系
    // pub(crate) cache_mapping: UnsafeCell<(ArchetypeWorldIndex, ArchetypeLocalIndex)>,
}
unsafe impl<'world, Q: FetchComponents, F: FilterComponents> Send for Query<'world, Q, F> {}
unsafe impl<'world, Q: FetchComponents, F: FilterComponents> Sync for Query<'world, Q, F> {}
impl<'world, Q: FetchComponents, F: FilterComponents> Query<'world, Q, F> {
    
    pub fn new(world: &'world World, state: &'world mut QueryState<Q, F>, tick: Tick) -> Self {
        // let cache_mapping = UnsafeCell::new((ArchetypeWorldIndex::null(), ArchetypeLocalIndex(0)));
        Query {
            world,
            state,
            tick,
            // cache_mapping,
        }
    }
    
    pub fn tick(&self) -> Tick {
        self.tick
    }
    
    pub fn last_run(&self) -> Tick {
        self.state.last_run
    }
    
    pub fn contains(&self, entity: Entity) -> bool {
        if let Ok( (_addr, world_index, local_index)) = check(
            self.world,
            entity,
            // unsafe { &mut *self.cache_mapping.get() },
            &self.state.map,
        ){
            // unsafe { *self.cache_mapping.get()  = (world_index, local_index)};
            return true;
        }else{
            return false;
        }
    }
    
    pub fn get(
        &self,
        e: Entity,
    ) -> Result<<<Q as FetchComponents>::ReadOnly as FetchComponents>::Item<'_>, QueryError> {
        self.state
            .as_readonly()
            .get(self.world, self.tick, e, /* unsafe {
                &mut *self.cache_mapping.get()
            } */)
    }
    
    pub fn get_mut(&mut self, e: Entity) -> Result<<Q as FetchComponents>::Item<'_>, QueryError> {
        self.state
            .get(self.world, self.tick, e, /* self.cache_mapping.get_mut() */)
    }
    
    pub fn is_empty(&self) -> bool {
        self.state.is_empty()
    }
    
    pub fn len(&self) -> usize {
        self.state.len()
    }
    
    pub fn iter(&self) -> QueryIter<'_, <Q as FetchComponents>::ReadOnly, F> {
        QueryIter::new(self.world, self.state.as_readonly(), self.tick)
    }
    pub fn iter_mut(&mut self) -> QueryIter<'_, Q, F> {
        QueryIter::new(self.world, &self.state, self.tick)
    }
}

impl<'a, Q: FetchComponents + 'static, F: FilterComponents + Send + Sync> SystemParam
    for Query<'a, Q, F>
{
    type State = QueryState<Q, F>;
    type Item<'w> = Query<'w, Q, F>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        Q::init_read_write(world, system_meta);
        F::init_read_write(world, system_meta);
        system_meta.cur_param_ok();
        Self::State::create(world)
    }
    fn archetype_depend(
        world: &World,
        _system_meta: &SystemMeta,
        _state: &Self::State,
        archetype: &Archetype,
        result: &mut ArchetypeDependResult,
    ) {
        Q::archetype_depend(world, archetype, result);
        if F::archetype_filter(world, archetype) {
            result.merge(ArchetypeDepend::Flag(Flags::WITHOUT));
        }
    }
    fn res_depend(
        _world: &World,
        _system_meta: &SystemMeta,
        _state: &Self::State,
        res_tid: &TypeId,
        res_name: &Cow<'static, str>,
        single: bool,
        result: &mut Flags,
    ) {
        Q::res_depend(res_tid, res_name, single, result);
    }

    
    fn align(world: &World, _system_meta: &SystemMeta, state: &mut Self::State) {
        state.align(world);
    }

    
    fn get_param<'world>(
        world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world> {
        Query::new(world, state, tick)
    }
    
    fn get_self<'world>(
        world: &'world World,
        system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, system_meta, state, tick)) }
    }
}
impl<Q: FetchComponents + 'static, F: FilterComponents + Send + Sync> ParamSetElement
    for Query<'_, Q, F>
{
    fn init_set_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        Q::init_read_write(world, system_meta);
        F::init_read_write(world, system_meta);
        system_meta.param_set_check();
        Self::State::create(world)
    }
}
impl<'world, Q: FetchComponents, F: FilterComponents> Drop for Query<'world, Q, F> {
    fn drop(&mut self) {
        self.state.last_run = self.tick;
        // self.state.cache_mapping = *self.cache_mapping.get_mut();
    }
}
/// 监听原型创建， 添加record
pub struct Notify<'a, Q: FetchComponents, F: FilterComponents>(
    Vec<ComponentIndex>,
    SmallVec<[ListenType; 1]>,
    PhantomData<&'a ()>,
    PhantomData<Q>,
    PhantomData<F>,
);
impl<'a, Q: FetchComponents + 'static, F: FilterComponents + 'static> Listener
    for Notify<'a, Q, F>
{
    type Event = ArchetypeInit<'a>;
    fn listen(&self, e: Self::Event) {
        if !QueryState::<Q, F>::relate(e.1, e.0) {
            return;
        }
        unsafe {
            add_ticks(&e.0, &self.0);
            add_dirty_listeners(&e.0, TypeId::of::<QueryState<Q, F>>(), &self.1)
        };
    }
}

// 根据components列表，添加tick，该方法要么在初始化时调用，要么就是在原型刚创建时调用
pub(crate) unsafe fn add_ticks(ar: &Archetype, components: &Vec<ComponentIndex>) {
    for index in components.iter() {
        if let Some((c, _)) = ar.get_column_mut(*index) {
            c.is_record_tick = true;
        }
    }
}

// 根据脏监听列表，添加监听，该方法要么在初始化时调用，要么就是在原型刚创建时调用
unsafe fn add_dirty_listeners(
    ar: &Archetype,
    owner: TypeId,
    listeners: &SmallVec<[ListenType; 1]>,
) {
    for ltype in listeners.iter() {
        match ltype {
            ListenType::Changed(index) => ar.add_changed_listener(*index, owner),
            ListenType::Removed(index) => ar.add_removed_listener(*index, owner),
            ListenType::Destroyed => ar.add_destroyed_listener(owner),
        }
    }
}

// 根据监听列表，重新找到add_dirty_listeners前面放置脏监听列表的位置
unsafe fn find_dirty_listeners(
    ar: &Archetype,
    owner: TypeId,
    listeners: &SmallVec<[ListenType; 1]>,
    vec: &mut SmallVec<[DirtyIndex; 1]>,
) {
    for ltype in listeners.iter() {
        match ltype {
            ListenType::Changed(index) => ar.find_changed_listener(*index, owner, vec),
            ListenType::Removed(index) => ar.find_removed_listener(*index, owner, vec),
            ListenType::Destroyed => ar.find_destroyed_listener(owner, vec),
        }
    }
}
#[derive(Debug, Clone, Copy, Default)]
pub struct ArchetypeLocalIndex(pub(crate) u16);

impl pi_null::Null for ArchetypeLocalIndex{
    fn null() -> Self {
        Self(u16::null())
    }

    fn is_null(&self) -> bool {
        self.0 == u16::MAX
    }
}

#[derive(Debug)]
pub struct QueryState<Q: FetchComponents + 'static, F: FilterComponents + 'static> {
    pub(crate) listeners: SmallVec<[ListenType; 1]>,
    pub(crate) vec: Vec<ArchetypeQueryState<Q::State>>, // 每原型、查询状态及对应的脏监听
    pub(crate) archetype_len: usize, // 脏的最新的原型，如果world上有更新的，则检查是否和自己相关
    pub(crate) map: Vec<ArchetypeLocalIndex>, // world上的原型索引对于本地的原型索引
    pub(crate) last_run: Tick,                                         // 上次运行的tick
    // pub(crate) cache_mapping: (ArchetypeWorldIndex, ArchetypeLocalIndex), // 缓存上次的索引映射关系
    _k: PhantomData<F>,
}

impl<Q: FetchComponents, F: FilterComponents> QueryState<Q, F> {
    pub fn as_readonly(&self) -> &QueryState<Q::ReadOnly, F> {
        unsafe { &*(self as *const QueryState<Q, F> as *const QueryState<Q::ReadOnly, F>) }
    }
    pub fn create(world: &World) -> Self {
        let mut components = Default::default();
        if Q::TICK_COUNT > 0 {
            Q::init_ticks(world, &mut components);
        }
        let mut listeners = Default::default();
        if F::LISTENER_COUNT > 0 {
            F::init_listeners(world, &mut listeners);
        }
        if Q::TICK_COUNT > 0 || F::LISTENER_COUNT > 0 {
            // 遍历已有的原型， 添加record
            let notify = Notify(
                components,
                listeners.clone(),
                PhantomData,
                PhantomData::<Q>,
                PhantomData::<F>,
            );
            for r in world.archetype_arr.iter() {
                notify.listen(ArchetypeInit(r, world))
            }
            // 监听原型创建， 添加dirty
            world.listener_mgr.register_event(Share::new(notify));
        }
        QueryState::new(listeners)
    }
    pub fn new(listeners: SmallVec<[ListenType; 1]>) -> Self {
        Self {
            listeners,
            vec: Vec::new(),
            archetype_len: 0,
            map: Default::default(),
            last_run: Tick::default(),
            // cache_mapping: (ArchetypeWorldIndex::null(), ArchetypeLocalIndex(0)),
            _k: PhantomData,
        }
    }

    // 判断该原型是否和本查询相关
    fn relate(world: &World, archetype: &Archetype) -> bool {
        if F::archetype_filter(world, archetype) {
            return false;
        }
        let mut result = ArchetypeDependResult::new();
        Q::archetype_depend(world, archetype, &mut result);
        result.flag.bits() != 0 && !result.flag.contains(Flags::WITHOUT)
    }
    // 对齐world上新增的原型
    pub fn align(&mut self, world: &World) {
        let len = world.archetype_arr.len();
        if len == self.archetype_len {
            return;
        }
        // 检查新增的原型
        for i in self.archetype_len..len {
            let ar = unsafe { world.archetype_arr.get_unchecked(i) };
            self.add_archetype(world, ar, ArchetypeWorldIndex(i as u32) );
        }
        self.archetype_len = len;
        // println!("align1===={:?}", (std::any::type_name::<Self>(), len, self.archetype_len));
    }
    // 新增的原型
    pub fn add_archetype(
        &mut self,
        world: &World,
        ar: &ShareArchetype,
        index: ArchetypeWorldIndex,
    ) {
        // 判断原型是否和查询相关
        if !Self::relate(world, ar) {
            return;
        }
        let mut listeners = SmallVec::new();
        if F::LISTENER_COUNT > 0 {
            unsafe {
                find_dirty_listeners(&ar, TypeId::of::<Self>(), &self.listeners, &mut listeners)
            };
            // if vec.len() == 0 {
            //     // 表示该原型没有监听的组件，本查询可以不关心该原型
            //     return;
            // }
        }
        self.map
            .insert_value(index.0 as usize, ArchetypeLocalIndex(self.vec.len() as u16));
        self.vec.push(ArchetypeQueryState {
            ar: ar.clone(),
            state: Q::init_state(world, ar),
            listeners,
        });
    }
    pub fn get<'w>(
        &'w self,
        world: &'w World,
        tick: Tick,
        entity: Entity,
        // cache_mapping: &mut (ArchetypeWorldIndex, ArchetypeLocalIndex),
    ) -> Result<Q::Item<'w>, QueryError> {
        let (addr, _world_index, local_index) = check(world, entity, /* cache_mapping, */ &self.map)?;
        // let arch = world.archetype_arr.get(cache_mapping.0 as usize).unwrap();

        let arqs = unsafe { &self.vec.get_unchecked(local_index.0 as usize) };
        // println!("get======{:?}", (entity, arqs.index, addr, cache_mapping.0, cache_mapping.1,  arch.name()));
        let mut fetch = Q::init_fetch(world, &arqs.ar, &arqs.state, tick, self.last_run);
        Ok(Q::fetch(&mut fetch, addr.row, entity))
    }
    
    pub fn is_empty(&self) -> bool {
        if self.vec.is_empty() {
            return true;
        }
        self.len() == 0
    }
    
    pub fn len(&self) -> usize {
        let mut len = 0;
        for arqs in &self.vec {
            len += arqs.ar.len().0 as usize;
        }
        len
    }
}

// 每原型、查询状态，每组件的tick，及每组件的脏监听
#[derive(Debug)]
pub struct ArchetypeQueryState<S> {
    pub(crate) ar: ShareArchetype,
    state: S,
    listeners: SmallVec<[DirtyIndex; 1]>,
}

// 检查entity是否正确，包括对应的原型是否在本查询内，并将查询到的原型本地位置记到cache_mapping上
pub(crate) fn check<'w>(
    world: &'w World,
    entity: Entity,
    // cache_mapping: &mut (ArchetypeWorldIndex, ArchetypeLocalIndex),
    map: &Vec<ArchetypeLocalIndex>,
) -> Result<(EntityAddr, ArchetypeWorldIndex, ArchetypeLocalIndex), QueryError> {
    // assert!(!entity.is_null());
    let addr = match world.entities.get(entity) {
        Some(v) => *v,
        None => return Err(QueryError::NoSuchEntity),
    };

    let local_index  = match map.get(addr.index.0 as usize) {
        Some(v) => *v,
        None => return Err(QueryError::NoSuchArchetype),
    };
    return Ok((addr, addr.index, local_index));

    // Ok(addr)
}
struct DirtyIter<'a> {
    it: Iter<'a, EntityRow>,
    check_e: bool, // 是否对entity进行校验
    index: usize,  // 所在原型的脏监听索引
}
impl<'a> DirtyIter<'a> {
    fn empty() -> Self {
        Self {
            it: Iter::empty(),
            check_e: true,
            index: 0,
        }
    }
    fn new(it: (Iter<'a, EntityRow>, bool), index: usize) -> Self {
        Self {
            it: it.0,
            check_e: it.1,
            index,
        }
    }
}

pub struct QueryIter<'w, Q: FetchComponents + 'static, F: FilterComponents + 'static> {
    pub(crate) world: &'w World,
    state: &'w QueryState<Q, F>,
    pub(crate) tick: Tick,
    // 原型的位置
    pub(crate) ar_index: ArchetypeLocalIndex,
    // 原型
    pub(crate) ar: &'w Archetype,
    fetch: MaybeUninit<Q::Fetch<'w>>,
    pub(crate) e: Entity,
    pub(crate) row: Row,
    // 脏迭代器，监听多个组件变化，可能entity相同，需要进行去重
    dirty: DirtyIter<'w>,
    // 用来脏查询时去重row
    bitset: FixedBitSet,
    // 缓存上次的索引映射关系
    // cache_mapping: (ArchetypeWorldIndex, ArchetypeLocalIndex),
}
impl<'w, Q: FetchComponents, F: FilterComponents> QueryIter<'w, Q, F> {
    /// # Safety
    /// - `world` must have permission to access any of the components registered in `query_state`.
    /// - `world` must be the same one used to initialize `query_state`.
    pub fn new(world: &'w World, state: &'w QueryState<Q, F>, tick: Tick) -> Self {
        let mut ar_index = state.vec.len();
        while ar_index > 0 {
            ar_index -= 1;
            let arqs = unsafe { state.vec.get_unchecked(ar_index) };
            let fetch = MaybeUninit::new(Q::init_fetch(
                world,
                &arqs.ar,
                &arqs.state,
                tick,
                state.last_run,
            ));
            if F::LISTENER_COUNT == 0 {
                // 该查询没有监听组件变化，倒序迭代原型的行
                return QueryIter {
                    world,
                    state,
                    tick,
                    ar: &arqs.ar,
                    ar_index: ArchetypeLocalIndex(ar_index as u16),
                    fetch,
                    e: Entity::null(),
                    row: arqs.ar.len(),
                    dirty: DirtyIter::empty(),
                    bitset: FixedBitSet::new(),
                    // cache_mapping: state.cache_mapping,
                };
            } else if arqs.listeners.len() > 0 {
                let bitset = if F::LISTENER_COUNT == 1 {
                    FixedBitSet::new()
                } else {
                    FixedBitSet::with_capacity(arqs.ar.len().0 as usize)
                };
                // 该查询有组件变化监听器， 倒序迭代所脏的原型
                let len = arqs.listeners.len() - 1;
                let d_index = unsafe { *arqs.listeners.get_unchecked(len) };
                return QueryIter {
                    world,
                    state,
                    tick,
                    ar: &arqs.ar,
                    ar_index: ArchetypeLocalIndex(ar_index as u16),
                    fetch,
                    e: Entity::null(),
                    row:  Row::null(),
                    dirty: DirtyIter::new(arqs.ar.get_iter(&d_index), len),
                    bitset,
                    // cache_mapping: state.cache_mapping,
                };
            }
        }
        // 该查询没有关联的原型
        QueryIter {
            world,
            state,
            tick,
            ar: world.empty_archetype(),
            ar_index: ArchetypeLocalIndex(0),
            fetch: MaybeUninit::uninit(),
            e: Entity::null(),
            row: Row(0),
            dirty: DirtyIter::empty(),
            bitset: FixedBitSet::new(),
            // cache_mapping: state.cache_mapping,
        }
    }
    pub fn entity(&self) -> Entity {
        self.e
    }

    fn iter_normal(&mut self) -> Option<Q::Item<'w>> {
        loop {
            if self.row.0 > 0 {
                self.row.0 -= 1;
                self.e = self.ar.get(self.row);
                // 要求条目不为空
                if !self.e.is_null() {
                    let item = Q::fetch(unsafe { self.fetch.assume_init_mut() }, self.row, self.e);
                    return Some(item);
                }
                continue;
            }
            // 当前的原型已经迭代完毕
            if self.ar_index.0 == 0 {
                // 所有原型都迭代过了
                return None;
            }
            // 下一个原型
            self.ar_index.0 -= 1;
            let arqs = unsafe { &self.state.vec.get_unchecked(self.ar_index.0 as usize) };
            self.ar = &arqs.ar;
            self.fetch = MaybeUninit::new(Q::init_fetch(
                self.world,
                self.ar,
                &arqs.state,
                self.tick,
                self.state.last_run,
            ));
            self.row = self.ar.len();
        }
    }

    fn iter_dirty(&mut self) -> Option<Q::Item<'w>> {
        loop {
            if let Some(d) = self.dirty.it.next() {
                self.row = d.row;
                if self.dirty.check_e {
                    // 如果不检查对应row的e，则是查询被标记销毁的实体
                    self.e = d.e;
                    let item = Q::fetch(unsafe { self.fetch.assume_init_mut() }, self.row, self.e);
                    return Some(item);
                }
                self.e = self.ar.get(self.row);
                // 要求条目不为空
                if !self.e.is_null() {
                    let item = Q::fetch(unsafe { self.fetch.assume_init_mut() }, self.row, self.e);
                    return Some(item);
                }
                // 如果为null，则用d.e去查，e是否存在，所在的原型是否在本查询范围内
                match self
                    .state
                    .get(self.world, self.tick, d.e, /* &mut self.cache_mapping */)
                {
                    Ok(item) => return Some(item),
                    Err(_) => (),
                }
                continue;
            }
            // 检查当前原型的下一个被脏组件
            if self.dirty.index > 0 {
                let len = self.dirty.index - 1;
                let arqs = unsafe { &self.state.vec.get_unchecked(self.ar_index.0 as usize) };
                let d_index = unsafe { *arqs.listeners.get_unchecked(len) };
                let iter = arqs.ar.get_iter(&d_index);
                self.dirty = DirtyIter::new(iter, len);
                continue;
            }
            // 当前的原型已经迭代完毕
            if self.ar_index.0 == 0 {
                // 所有原型都迭代过了
                return None;
            }
            // 下一个原型
            self.ar_index.0 -= 1;
            let arqs = unsafe { &self.state.vec.get_unchecked(self.ar_index.0 as usize ) };
            if arqs.listeners.len() == 0 {
                continue;
            }
            self.ar = &arqs.ar;
            self.fetch = MaybeUninit::new(Q::init_fetch(
                self.world,
                self.ar,
                &arqs.state,
                self.tick,
                self.state.last_run,
            ));
            // 监听被脏组件
            let len = arqs.listeners.len() - 1;
            let d_index = unsafe { arqs.listeners.get_unchecked(len) };
            let iter = arqs.ar.get_iter(&d_index);
            self.dirty = DirtyIter::new(iter, len);
        }
    }

    fn iter_dirtys(&mut self) -> Option<Q::Item<'w>> {
        loop {
            if let Some(d) = self.dirty.it.next() {
                if self.bitset.contains(d.row.0 as usize) {
                    continue;
                }
                self.bitset.set(d.row.0 as usize, true);
                self.row = d.row;
                if self.dirty.check_e {
                    // 如果不检查对应row的e，则是查询被标记销毁的实体
                    self.e = d.e;
                    let item = Q::fetch(unsafe { self.fetch.assume_init_mut() }, self.row, self.e);
                    return Some(item);
                }
                self.e = self.ar.get(self.row);
                // 要求条目不为空
                if !self.e.is_null() {
                    let item = Q::fetch(unsafe { self.fetch.assume_init_mut() }, self.row, self.e);
                    return Some(item);
                }
                // 如果为null，则用d.e去查，e是否存在，所在的原型是否在本查询范围内
                match self
                    .state
                    .get(self.world, self.tick, d.e, /* &mut self.cache_mapping */)
                {
                    Ok(item) => return Some(item),
                    Err(_) => (),
                }
                continue;
            }
            // 检查当前原型的下一个被脏组件
            if self.dirty.index > 0 {
                let len = self.dirty.index - 1;
                let arqs = unsafe { &self.state.vec.get_unchecked(self.ar_index.0 as usize) };
                let d_index = unsafe { *arqs.listeners.get_unchecked(len) };
                let iter = arqs.ar.get_iter(&d_index);
                self.dirty = DirtyIter::new(iter, len);
                continue;
            }
            // 当前的原型已经迭代完毕
            if self.ar_index.0 == 0 {
                // 所有原型都迭代过了
                return None;
            }
            // 下一个原型
            self.ar_index.0 -= 1;
            let arqs = unsafe { &self.state.vec.get_unchecked(self.ar_index.0 as usize) };
            if arqs.listeners.len() == 0 {
                continue;
            }
            self.ar = &arqs.ar;
            self.fetch = MaybeUninit::new(Q::init_fetch(
                self.world,
                self.ar,
                &arqs.state,
                self.tick,
                self.state.last_run,
            ));
            // 监听被脏组件
            let len = arqs.listeners.len() - 1;
            let d_index = unsafe { arqs.listeners.get_unchecked(len) };
            let iter = arqs.ar.get_iter(&d_index);
            self.dirty = DirtyIter::new(iter, len);
            let len = arqs.ar.len().0 as usize;
            if self.bitset.len() < len {
                self.bitset = FixedBitSet::with_capacity(len);
            } else {
                self.bitset.clear();
            }
        }
    }

    fn size_hint_normal(&self) -> (usize, Option<usize>) {
        let it = self.state.vec[0..self.ar_index.0 as usize].iter();
        let count = it.map(|arqs| arqs.ar.len()).count();
        (self.row.0 as usize, Some(self.row.0 as usize + count))
    }
    fn size_hint_dirty(&self) -> (usize, Option<usize>) {
        // 获得当前原型的当前列的脏长度
        let mut c: usize = self.dirty.it.size_hint().1.unwrap_or_default();
        c += self.size_hint_ar_dirty(self.ar_index.0 as usize);
        (0, Some(c))
    }
    fn size_hint_ar_dirty(&self, ar_index: usize) -> usize {
        let mut c: usize = self.dirty.it.size_hint().1.unwrap_or_default();
        let arqs = unsafe { &self.state.vec.get_unchecked(self.ar_index.0 as usize) };
        // 获得当前原型的剩余列的脏长度
        c += self.size_hint_ar_column_dirty(&arqs.ar, &arqs.listeners, self.dirty.index);
        for i in 0..ar_index {
            let arqs = unsafe { &self.state.vec.get_unchecked(i) };
            // 获得剩余原型的全部列的脏长度
            c += self.size_hint_ar_column_dirty(&arqs.ar, &arqs.listeners, arqs.listeners.len());
        }
        c
    }
    fn size_hint_ar_column_dirty(
        &self,
        ar: &Archetype,
        vec: &SmallVec<[DirtyIndex; 1]>,
        len: usize,
    ) -> usize {
        let mut c: usize = 0;
        for i in 0..len {
            let d_index = unsafe { vec.get_unchecked(i) };
            let iter = ar.get_iter(d_index);
            c += iter.0.size_hint().1.unwrap_or_default();
        }
        c
    }
}

impl<'w, Q: FetchComponents, F: FilterComponents> Iterator for QueryIter<'w, Q, F> {
    type Item = Q::Item<'w>;

    fn next(&mut self) -> Option<Self::Item> {
        if F::LISTENER_COUNT == 0 {
            self.iter_normal()
        } else if F::LISTENER_COUNT == 1 {
            self.iter_dirty()
        } else {
            self.iter_dirtys()
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        if F::LISTENER_COUNT == 0 {
            self.size_hint_normal()
        } else {
            self.size_hint_dirty()
        }
    }
}
