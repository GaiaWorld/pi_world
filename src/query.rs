//! 查询，支持子查询

use core::fmt::*;
use core::result::Result;
use std::any::TypeId;
use std::borrow::Cow;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::mem::{transmute, MaybeUninit};

use crate::archetype::{
    Archetype, ArchetypeDepend, ArchetypeDependResult, ArchetypeWorldIndex, Flags, Row,
    ShareArchetype,
};
use crate::dirty::{DirtyIndex, EntityDirty};
use crate::fetch::FetchComponents;
use crate::filter::{FilterComponents, ListenType};
use crate::listener::Listener;
use crate::param_set::ParamSetElement;
use crate::system::SystemMeta;
use crate::system_params::SystemParam;
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
        let cache_mapping = UnsafeCell::new(state.cache_mapping);
        Self {
            world,
            state,
            tick,
            cache_mapping,
        }
    }
    #[inline]
    pub fn contains(&self, entity: Entity) -> bool {
        check(
            self.world,
            entity,
            unsafe { &mut *self.cache_mapping.get() },
            &self.state.map,
        )
        .is_ok()
    }
    #[inline]
    pub fn get(
        &self,
        e: Entity,
    ) -> Result<<<Q as FetchComponents>::ReadOnly as FetchComponents>::Item<'_>, QueryError>
    {
        self.state
            .as_readonly()
            .get(self.world, self.tick, e, unsafe { &mut *self.cache_mapping.get() })
    }
    #[inline]
    pub fn get_mut(
        &mut self,
        e: Entity,
    ) -> Result<<Q as FetchComponents>::Item<'_>, QueryError> {
        self.state.get(self.world, self.tick, e, self.cache_mapping.get_mut())
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.state.is_empty()
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.state.len()
    }
    #[inline]
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
    pub(crate) cache_mapping: UnsafeCell<(ArchetypeWorldIndex, ArchetypeLocalIndex)>,
}
unsafe impl<'world, Q: FetchComponents, F: FilterComponents> Send for Query<'world, Q, F> {}
unsafe impl<'world, Q: FetchComponents, F: FilterComponents> Sync for Query<'world, Q, F> {}
impl<'world, Q: FetchComponents, F: FilterComponents> Query<'world, Q, F> {
    #[inline]
    pub fn new(world: &'world World, state: &'world mut QueryState<Q, F>, tick: Tick) -> Self {
        let cache_mapping = UnsafeCell::new(state.cache_mapping);
        Query {
            world,
            state,
            tick,
            cache_mapping,
        }
    }
    #[inline]
    pub fn tick(&self) -> Tick {
        self.tick
    }
    #[inline]
    pub fn last_run(&self) -> Tick {
        self.state.last_run
    }
    #[inline]
    pub fn contains(&self, entity: Entity) -> bool {
        check(
            self.world,
            entity,
            unsafe { &mut *self.cache_mapping.get() },
            &self.state.map,
        )
        .is_ok()
    }
    #[inline]
    pub fn get(
        &self,
        e: Entity,
    ) -> Result<<<Q as FetchComponents>::ReadOnly as FetchComponents>::Item<'_>, QueryError> {
        self.state
            .as_readonly()
            .get(self.world, self.tick, e, unsafe { &mut *self.cache_mapping.get() })
    }
    #[inline]
    pub fn get_mut(&mut self, e: Entity) -> Result<<Q as FetchComponents>::Item<'_>, QueryError> {
        self.state.get(self.world, self.tick, e, self.cache_mapping.get_mut())
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.state.is_empty()
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.state.len()
    }
    #[inline]
    pub fn iter(&self) -> QueryIter<'_, <Q as FetchComponents>::ReadOnly, F> {
        QueryIter::new(self.world, self.state.as_readonly(), self.tick)
    }
    pub fn iter_mut(&mut self) -> QueryIter<'_, Q, F> {
        QueryIter::new(self.world, &self.state, self.tick)
    }
}

impl<'a, Q: FetchComponents + 'static, F: FilterComponents + Send + Sync> SystemParam for Query<'a, Q, F>
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
        _world: &World,
        _system_meta: &SystemMeta,
        _state: &Self::State,
        archetype: &Archetype,
        result: &mut ArchetypeDependResult,
    ) {
        Q::archetype_depend(archetype, result);
        if F::archetype_filter(archetype) {
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

    #[inline]
    fn align(world: &World, _system_meta: &SystemMeta, state: &mut Self::State) {
        state.align(world);
    }

    #[inline]
    fn get_param<'world>(
        world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world> {
        Query::new(world, state, tick)
    }
    #[inline]
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
    fn init_set_state(world: &World, system_meta: &mut SystemMeta) -> Self::State {
        Q::init_read_write(world, system_meta);
        F::init_read_write(world, system_meta);
        system_meta.param_set_check();
        Self::State::create(world)
    }
}
impl<'world, Q: FetchComponents, F: FilterComponents> Drop for Query<'world, Q, F> {
    fn drop(&mut self) {
        self.state.last_run = self.tick;
        self.state.cache_mapping = *self.cache_mapping.get_mut();
    }
}
/// 监听原型创建， 添加record
pub struct Notify<'a, Q: FetchComponents, F: FilterComponents>(
    Vec<TypeId>,
    SmallVec<[(TypeId, ListenType); 1]>,
    PhantomData<&'a ()>,
    PhantomData<Q>,
    PhantomData<F>,
);
impl<'a, Q: FetchComponents + 'static, F: FilterComponents + 'static> Listener
    for Notify<'a, Q, F>
{
    type Event = ArchetypeInit<'a>;
    fn listen(&self, e: Self::Event) {
        if !QueryState::<Q, F>::relate(e.0) {
            return;
        }
        unsafe {
            add_ticks(&e.0, &self.0);
            add_dirty_listeners(&e.0, TypeId::of::<QueryState<Q, F>>(), &self.1)
        };
    }
}

// 根据tick列表，添加tick，该方法要么在初始化时调用，要么就是在原型刚创建时调用
pub(crate) unsafe fn add_ticks(ar: &Archetype, ticks: &Vec<TypeId>) {
    for tid in ticks.iter() {
        if let Some((c, _)) = ar.get_column_mut(tid) {
            c.is_tick = true;
        }
    }
}

// 根据脏监听列表，添加监听，该方法要么在初始化时调用，要么就是在原型刚创建时调用
unsafe fn add_dirty_listeners(
    ar: &Archetype,
    owner: TypeId,
    listeners: &SmallVec<[(TypeId, ListenType); 1]>,
) {
    for (tid, ltype) in listeners.iter() {
        if let Some((c, _)) = ar.get_column_mut(tid) {
            c.is_tick = true;
            match ltype {
                ListenType::Changed => c.dirty.insert_listener(owner),
                ListenType::Removed => c.dirty.insert_listener(owner),
                ListenType::Destroyed => c.dirty.insert_listener(owner),
            }
        }
    }
}

// 根据监听列表，重新找到add_dirty_listeners前面放置脏监听列表的位置
unsafe fn find_dirty_listeners(
    ar: &Archetype,
    owner: TypeId,
    listens: &SmallVec<[(TypeId, ListenType); 1]>,
    vec: &mut SmallVec<[DirtyIndex; 1]>,
) {
    for (tid, ltype) in listens.iter() {
        if let Some((c, index)) = ar.get_column_mut(tid) {
            let d = match ltype {
                ListenType::Changed => &c.dirty,
                ListenType::Removed => &c.dirty,
                ListenType::Destroyed => &c.dirty,
            };
            d.find(index, owner, *ltype, vec);
        }
    }
}

pub type ArchetypeLocalIndex = usize;

#[derive(Debug)]
pub struct QueryState<Q: FetchComponents + 'static, F: FilterComponents + 'static> {
    pub(crate) listeners: SmallVec<[(TypeId, ListenType); 1]>,
    pub(crate) vec: Vec<ArchetypeQueryState<Q::State>>, // 每原型、查询状态及对应的脏监听
    pub(crate) archetype_len: usize, // 脏的最新的原型，如果world上有更新的，则检查是否和自己相关
    pub(crate) map: HashMap<ArchetypeWorldIndex, ArchetypeLocalIndex>, // world上的原型索引对于本地的原型索引
    pub(crate) last_run: Tick, // 上次运行的tick
    pub(crate) cache_mapping: (ArchetypeWorldIndex, ArchetypeLocalIndex), // 缓存上次的索引映射关系
    _k: PhantomData<F>,
}

impl<Q: FetchComponents, F: FilterComponents> QueryState<Q, F> {
    pub fn as_readonly(&self) -> &QueryState<Q::ReadOnly, F> {
        unsafe { &*(self as *const QueryState<Q, F> as *const QueryState<Q::ReadOnly, F>) }
    }
    pub fn create(world: &World) -> Self {
        let mut ticks = Default::default();
        if Q::TICK_COUNT > 0 {
            Q::init_ticks(world, &mut ticks);
        }
        let mut listeners = Default::default();
        if F::LISTENER_COUNT > 0 {
            F::init_listeners(world, &mut listeners);
        }
        if  Q::TICK_COUNT > 0 || F::LISTENER_COUNT > 0 {
            // 遍历已有的原型， 添加record
            let notify = Notify(
                ticks.clone(),
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
    pub fn new(listeners: SmallVec<[(TypeId, ListenType); 1]>) -> Self {
        Self {
            listeners,
            vec: Vec::new(),
            archetype_len: 0,
            map: Default::default(),
            last_run: Tick::default(),
            cache_mapping: (ArchetypeWorldIndex::null(), 0),
            _k: PhantomData,
        }
    }

    // 判断该原型是否和本查询相关
    fn relate(archetype: &Archetype) -> bool {
        if F::archetype_filter(archetype) {
            return false;
        }
        let mut result = ArchetypeDependResult::new();
        Q::archetype_depend(archetype, &mut result);
        !result.flag.contains(Flags::WITHOUT)
    }
    // 对齐world上新增的原型
    #[inline]
    pub fn align(&mut self, world: &World) {
        let len = world.archetype_arr.len();
        if len == self.archetype_len {
            return;
        }
        // 检查新增的原型
        for i in self.archetype_len..len {
            let ar = unsafe { world.archetype_arr.get_unchecked(i) };
            self.add_archetype(world, ar, i as ArchetypeWorldIndex);
        }
        self.archetype_len = len;
    }
    // 新增的原型
    pub fn add_archetype(
        &mut self,
        world: &World,
        ar: &ShareArchetype,
        index: ArchetypeWorldIndex,
    ) {
        // 判断原型是否和查询相关
        if !Self::relate(ar) {
            return;
        }
        let mut listeners = SmallVec::new();
        if F::LISTENER_COUNT > 0 {
            unsafe { find_dirty_listeners(&ar, TypeId::of::<Self>(), &self.listeners, &mut listeners) };
            // if vec.len() == 0 {
            //     // 表示该原型没有监听的组件，本查询可以不关心该原型
            //     return;
            // }
        }
        self.map
            .insert(index, self.vec.len() as ArchetypeLocalIndex);
        self.vec.push(ArchetypeQueryState::new(ar.clone(), Q::init_state(world, ar), listeners));
    }
    pub fn get<'w>(
        &'w self,
        world: &'w World,
        tick: Tick,
        entity: Entity,
        cache_mapping: &mut (ArchetypeWorldIndex, ArchetypeLocalIndex),
    ) -> Result<Q::Item<'w>, QueryError> {
        let addr = check(world, entity, cache_mapping, &self.map)?;
        let arqs = unsafe { &self.vec.get_unchecked(self.cache_mapping.1) };
        let mut fetch = Q::init_fetch(world, &arqs.ar, &arqs.state, tick, self.last_run);
        Ok(Q::fetch(&mut fetch, addr.row, entity))
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        if self.vec.is_empty() {
            return true;
        }
        self.len() == 0
    }
    #[inline]
    pub fn len(&self) -> usize {
        let mut len = 0;
        for arqs in &self.vec {
            len += arqs.ar.len();
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
impl <S> ArchetypeQueryState<S> {
    pub fn new(ar: ShareArchetype, state: S, listeners: SmallVec<[DirtyIndex; 1]>) -> Self {
        Self {
            ar,
            state,
            listeners,
        }
    }
}

// 检查entity是否正确，包括对应的原型是否在本查询内，并将查询到的原型本地位置记到cache_mapping上
pub(crate) fn check<'w>(
    world: &'w World,
    entity: Entity,
    cache_mapping: &mut (ArchetypeWorldIndex, ArchetypeLocalIndex),
    map: &HashMap<ArchetypeWorldIndex, ArchetypeLocalIndex>,
) -> Result<EntityAddr, QueryError> {
    let addr = match world.entities.get(entity) {
        Some(v) => *v,
        None => return Err(QueryError::NoSuchEntity),
    };
    if cache_mapping.0 != addr.index {
        cache_mapping.1 = match map.get(&addr.index) {
            Some(v) => *v,
            None => return Err(QueryError::NoSuchArchetype),
        };
        cache_mapping.0 = addr.index;
    }
    Ok(addr)
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
    dirty: (Iter<'w, EntityDirty>, usize),
    // 用来脏查询时去重row
    bitset: FixedBitSet,
    // 缓存上次的索引映射关系
    cache_mapping: (ArchetypeWorldIndex, ArchetypeLocalIndex),
}
impl<'w, Q: FetchComponents, F: FilterComponents> QueryIter<'w, Q, F> {
    /// # Safety
    /// - `world` must have permission to access any of the components registered in `query_state`.
    /// - `world` must be the same one used to initialize `query_state`.
    #[inline(always)]
    pub(crate) fn new(world: &'w World, state: &'w QueryState<Q, F>, tick: Tick) -> Self {
        let mut ar_index = state.vec.len();
        while ar_index > 0 {
            ar_index -= 1;
            let arqs = unsafe { state.vec.get_unchecked(ar_index) };
            let fetch = MaybeUninit::new(Q::init_fetch(world, &arqs.ar, &arqs.state, tick, state.last_run));
            if F::LISTENER_COUNT == 0 {
                // 该查询没有监听组件变化，倒序迭代原型的行
                return QueryIter {
                    world,
                    state,
                    tick,
                    ar: &arqs.ar,
                    ar_index,
                    fetch,
                    e: Entity::null(),
                    row: arqs.ar.table.len(),
                    dirty: (Iter::empty(), 0),
                    bitset: FixedBitSet::new(),
                    cache_mapping: state.cache_mapping,
                };
            } else if arqs.listeners.len() > 0 {
                // 该查询有组件变化监听器， 倒序迭代所脏的原型
                let len = arqs.listeners.len() - 1;
                let d_index = unsafe { *arqs.listeners.get_unchecked(len) };
                return QueryIter {
                    world,
                    state,
                    tick,
                    ar: &arqs.ar,
                    ar_index,
                    fetch,
                    e: Entity::null(),
                    row: u32::null(),
                    dirty: (d_index.get_iter(&arqs.ar), len),
                    bitset: FixedBitSet::with_capacity(arqs.ar.len()),
                    cache_mapping: state.cache_mapping,
                };
            }
        }
        // 该查询没有关联的原型
        QueryIter {
            world,
            state,
            tick,
            ar: world.empty_archetype(),
            ar_index: 0,
            fetch: MaybeUninit::uninit(),
            e: Entity::null(),
            row: 0,
            dirty: (Iter::empty(), 0),
            bitset: FixedBitSet::new(),
            cache_mapping: state.cache_mapping,
        }
    }
    #[inline(always)]
    pub fn entity(&self) -> Entity {
        self.e
    }

    #[inline(always)]
    fn iter_normal(&mut self) -> Option<Q::Item<'w>> {
        loop {
            if self.row > 0 {
                self.row -= 1;
                self.e = self.ar.table.get(self.row);
                // 要求条目不为空
                if !self.e.is_null() {
                    let item = Q::fetch(unsafe { self.fetch.assume_init_mut() }, self.row, self.e);
                    return Some(item);
                }
                continue;
            }
            // 当前的原型已经迭代完毕
            if self.ar_index == 0 {
                // 所有原型都迭代过了
                return None;
            }
            // 下一个原型
            self.ar_index -= 1;
            let arqs = unsafe { &self.state.vec.get_unchecked(self.ar_index) };
            self.ar = &arqs.ar;
            self.fetch = MaybeUninit::new(Q::init_fetch(self.world, self.ar, &arqs.state, self.tick, self.state.last_run));
            self.row = self.ar.table.len();
        }
    }

    #[inline]
    fn iter_dirty(&mut self) -> Option<Q::Item<'w>> {
        loop {
            if let Some(d) = self.dirty.0.next() {
                if self.bitset.contains(d.row as usize) {
                    continue;
                }
                self.bitset.set(d.row as usize, true);
                self.row = d.row;
                self.e = self.ar.table.get(self.row);
                // 要求条目不为空
                if !self.e.is_null() {
                    let item = Q::fetch(unsafe { self.fetch.assume_init_mut() }, self.row, self.e);
                    return Some(item);
                } else {
                    // 如果为null，则用d.e去查，e是否存在，所在的原型是否在本查询范围内
                    match self.state.get(self.world, self.tick, d.e, &mut self.cache_mapping) {
                        Ok(item) => return Some(item),
                        Err(_) => (),
                    }
                }
                continue;
            }
            // 检查当前原型的下一个被脏组件
            if self.dirty.1 > 0 {
                let len = self.dirty.1 - 1;
                let arqs = unsafe { &self.state.vec.get_unchecked(self.ar_index) };
                let d_index = unsafe { *arqs.listeners.get_unchecked(len) };
                let iter = d_index.get_iter(&arqs.ar);
                self.dirty = (iter, len);
                continue;
            }
            // 当前的原型已经迭代完毕
            if self.ar_index == 0 {
                // 所有原型都迭代过了
                return None;
            }
            // 下一个原型
            self.ar_index -= 1;
            let arqs = unsafe { &self.state.vec.get_unchecked(self.ar_index) };
            if arqs.listeners.len() == 0 {
                continue;
            }
            self.ar = &arqs.ar;
            self.fetch = MaybeUninit::new(Q::init_fetch(self.world, self.ar, &arqs.state, self.tick, self.state.last_run));
            // 监听被脏组件
            let len = arqs.listeners.len() - 1;
            let d_index = unsafe { *arqs.listeners.get_unchecked(len) };
            let iter = d_index.get_iter(&arqs.ar);
            self.dirty = (iter, len);
            let len = arqs.ar.len();
            if self.bitset.len() < len {
                self.bitset = FixedBitSet::with_capacity(len);
            } else {
                self.bitset.clear();
            }
        }
    }

    #[inline(always)]
    fn size_hint_normal(&self) -> (usize, Option<usize>) {
        let it = self.state.vec[0..self.ar_index as usize].iter();
        let count = it.map(|arqs| arqs.ar.table.len()).count();
        (self.row as usize, Some(self.row as usize + count))
    }
    #[inline(always)]
    fn size_hint_dirty(&self) -> (usize, Option<usize>) {
        // 获得当前原型的当前列的脏长度
        let mut c: usize = self.dirty.0.size_hint().1.unwrap_or_default();
        c += self.size_hint_ar_dirty(self.ar_index);
        (0, Some(c))
    }
    #[inline(always)]
    fn size_hint_ar_dirty(&self, ar_index: usize) -> usize {
        let mut c: usize = self.dirty.0.size_hint().1.unwrap_or_default();
        let arqs = unsafe { &self.state.vec.get_unchecked(self.ar_index) };
        // 获得当前原型的剩余列的脏长度
        c += self.size_hint_ar_column_dirty(&arqs.ar, &arqs.listeners, self.dirty.1);
        for i in 0..ar_index {
            let arqs = unsafe { &self.state.vec.get_unchecked(i) };
            // 获得剩余原型的全部列的脏长度
            c += self.size_hint_ar_column_dirty(&arqs.ar, &arqs.listeners, arqs.listeners.len());
        }
        c
    }
    #[inline(always)]
    fn size_hint_ar_column_dirty(
        &self,
        ar: &Archetype,
        vec: &SmallVec<[DirtyIndex; 1]>,
        len: usize,
    ) -> usize {
        let mut c: usize = 0;
        for i in 0..len {
            let d_index = unsafe { vec.get_unchecked(i) };
            let iter = d_index.get_iter(ar);
            c += iter.size_hint().1.unwrap_or_default();
        }
        c
    }
}

impl<'w, Q: FetchComponents, F: FilterComponents> Iterator for QueryIter<'w, Q, F> {
    type Item = Q::Item<'w>;
    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if F::LISTENER_COUNT == 0 {
            self.iter_normal()
        } else {
            self.iter_dirty()
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
