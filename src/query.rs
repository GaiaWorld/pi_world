//! 查询，支持子查询

use core::fmt::*;
use core::result::Result;
use std::any::TypeId;
use std::borrow::Cow;
use std::cell::UnsafeCell;
// use std::collections::HashMap;
use std::marker::PhantomData;
use std::mem::{transmute, MaybeUninit};
use std::ops::Range;
use std::sync::atomic::Ordering;

use crate::archetype::{
    Archetype, ArchetypeDepend, ArchetypeDependResult, ArchetypeWorldIndex, ColumnIndex, Flags, Row, ShareArchetype
};
use crate::dirty::{DirtyIndex, DirtyIter, EntityRow};
use crate::fetch::FetchComponents;
use crate::filter::{FilterComponents, ListenType};
use crate::listener::Listener;
use crate::param_set::ParamSetElement;
use crate::system::SystemMeta;
use crate::system_params::SystemParam;
use crate::utils::VecExt;
use crate::{column, world::*};
use fixedbitset::FixedBitSet;
use pi_append_vec::AppendVec;
use pi_null::*;
use pi_share::{Share, ShareUsize};
use smallvec::SmallVec;

#[derive(Debug, PartialEq, Eq)]
pub enum QueryError {
    MissingComponent,
    NoMatchArchetype,
    NoMatchEntity(Entity),
    NoSuchEntity,
    NoSuchRow,
    NoSuchRes,
    RepeatAlter,
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
        if let Ok( (addr, local_index)) = check(
            self.world,
            entity,
            // unsafe { &mut *self.cache_mapping.get() },
            &self.state.map,
        ){
            unsafe { *self.cache_mapping.get()  = (addr.archetype_index(), local_index)};
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
        Query {
            world,
            state,
            tick,
        }
    }
    
    pub fn tick(&self) -> Tick {
        self.tick
    }
    
    pub fn last_run(&self) -> Tick {
        self.state.last_run
    }
    
    pub fn contains(&self, entity: Entity) -> bool {
        check(self.world, entity, &self.state.map).is_ok()
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

    pub fn archetypes_len(&self) -> usize {
        self.state.archetypes_len()
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
        if let Some(t) = &self.state.share_last_run {
            t.store(self.tick.index(), Ordering::Relaxed);
        }
    }
}
/// 监听原型创建， 添加record
pub struct Notify<'a, Q: FetchComponents + 'static, F: FilterComponents + 'static> {
    id: Tick,
    listeners: Vec<ComponentIndex>,
    tick: Share<ShareUsize>,
    _a: (PhantomData<&'a ()>,PhantomData<Q>,PhantomData<F>),
}

impl<'a, Q: FetchComponents + 'static, F: FilterComponents + 'static> Listener
    for Notify<'a, Q, F>
{
    type Event = ArchetypeInit<'a>;
    fn listen(&self, e: Self::Event) {
        if !QueryState::<Q, F>::relate(e.1, e.0) {
            return;
        }
        unsafe {
            add_dirty_listeners(&e.0, self.id, &self.listeners, &self.tick)
        };
    }
}

// 根据脏监听列表，添加监听，该方法要么在初始化时调用，要么就是在原型刚创建时调用
unsafe fn add_dirty_listeners(
    ar: &Archetype,
    owner: Tick,
    changed_listeners: &Vec<ComponentIndex>,
    tick: &Share<ShareUsize>,
) {
    for index in changed_listeners.iter() {
        ar.add_listener(*index, owner, tick.clone());
    }
}

// 根据监听列表，重新找到add_dirty_listeners前面放置脏监听列表的位置
unsafe fn find_dirty_listeners(
    ar: &Archetype,
    owner: Tick,
    changed_listeners: &Vec<ComponentIndex>,
    vec: &mut Vec<(ColumnIndex, Share<ShareUsize>, Share<AppendVec<EntityRow>>)>,
) {
    for index in changed_listeners.iter() {
        ar.find_listener(*index, owner, vec);
    }
}
#[derive(Debug, Clone, Copy, Default)]
pub struct ArchetypeLocalIndex(u16);
impl ArchetypeLocalIndex {
    pub fn index(&self) -> usize {
        self.0 as usize
    }
}
impl From <u16> for ArchetypeLocalIndex {
    fn from(index: u16) -> Self {
        Self(index)
    }
}
impl From <usize> for ArchetypeLocalIndex {
    fn from(index: usize) -> Self {
        Self(index as u16)
    }
}
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
    pub(crate) id: Tick, // 由world上分配的唯一tick
    pub(crate) listeners: Vec<ComponentIndex>,
    pub(crate) vec: Vec<ArchetypeQueryState<Q::State>>, // 每原型、查询状态及对应的脏监听
    pub(crate) archetypes: Vec<(Archetype, Range<u32>)>, // 每原型及对应的脏监听范围
    pub(crate) archetype_listeners: Vec<(ColumnIndex, Share<ShareUsize>, Share<AppendVec<EntityRow>>)>, // 所有原型的脏监听列表， 迭代时先判断Archetype的长度，然后判断AppendVec的长度
    pub(crate) archetypes_len: usize, // 脏的最新的原型，如果world上有更新的，则检查是否和自己相关
    pub(crate) map: Vec<ArchetypeLocalIndex>, // world上的原型索引对于本地的原型索引
    pub(crate) last_run: Tick,                                         // 上次运行的tick
    pub(crate) share_last_run: Option<Share<ShareUsize>>,                                         // 给脏列表共享的上次运行的tick
    _k: PhantomData<F>,
}

impl<Q: FetchComponents, F: FilterComponents> QueryState<Q, F> {
    pub fn as_readonly(&self) -> &QueryState<Q::ReadOnly, F> {
        unsafe { &*(self as *const QueryState<Q, F> as *const QueryState<Q::ReadOnly, F>) }
    }
    pub fn create(world: &mut World) -> Self {
        let id = world.increment_tick();
        let mut listeners = Default::default();
        if F::LISTENER_COUNT > 0 {
            F::init_listeners(world, &mut listeners);
        }
        let mut share_last_run = None;
        if F::LISTENER_COUNT > 0 {
            share_last_run = Some(Share::new(ShareUsize::new(0)));
            // 遍历已有的原型， 添加record
            let notify = Notify{
                id,
                listeners: listeners.clone(),
                tick: share_last_run.clone().unwrap(),
                _a: (PhantomData,
                PhantomData::<Q>,
                PhantomData::<F>),
            };
            for r in world.archetype_arr.iter() {
                notify.listen(ArchetypeInit(r, world))
            }
            // 监听原型创建， 添加dirty
            world.listener_mgr.register_event(Share::new(notify));
        }
        QueryState::new(id, listeners, share_last_run)
    }
    pub fn new(id: Tick, listeners: Vec<ComponentIndex>, share_last_run: Option<Share<ShareUsize>>) -> Self {
        Self {
            id,
            listeners,
            vec: Vec::new(),
            archetypes: Vec::new(),
            archetype_listeners: Vec::new(),
            archetypes_len: 0,
            map: Default::default(),
            last_run: Tick::default(),
            share_last_run,
            _k: PhantomData,
        }
    }

    // 判断该原型是否和本查询相关
    fn relate(world: &World, archetype: &Archetype) -> bool {
        // if archetype.index().is_null() {
        //     println!("QueryState::relate====={:?}", (archetype.index(), std::any::type_name::<Self>(), F::archetype_filter(world, archetype),  &archetype.name(),));
        // }
        if F::archetype_filter(world, archetype) {
            return false;
        }
        let mut result = ArchetypeDependResult::new();
        Q::archetype_depend(world, archetype, &mut result);
        !result.flag.contains(Flags::WITHOUT)
    }
    // 对齐world上新增的原型
    pub fn align(&mut self, world: &World) {
        let len = world.archetype_arr.len();
        if len == self.archetypes_len {
            return;
        }
        // 检查新增的原型
        for i in self.archetypes_len..len {
            let ar = unsafe { world.archetype_arr.get_unchecked(i) };
            self.add_archetype(world, ar, i.into());
        }
        self.archetypes_len = len;
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
        let start = self.archetype_listeners.len() as u32;
        if F::LISTENER_COUNT > 0 {
            unsafe {
                find_dirty_listeners(&ar, self.id, &self.listeners, &mut self.archetype_listeners)
            };
            // if vec.len() == 0 {
            //     // 表示该原型没有监听的组件，本查询可以不关心该原型
            //     return;
            // }
        }
        self.map
            .insert_value(index.index(), self.vec.len().into());
        self.vec.push(ArchetypeQueryState {
            ar: ar.clone(),
            state: Q::init_state(world, ar),
            range: start..self.archetype_listeners.len() as u32,
        });
    }
    pub fn get<'w>(
        &'w self,
        world: &'w World,
        tick: Tick,
        entity: Entity,
        // cache_mapping: &mut (ArchetypeWorldIndex, ArchetypeLocalIndex),
    ) -> Result<Q::Item<'w>, QueryError> {
        // println!("get1======{:?}", (entity, self.map.len()));
        let (addr, local_index) = check(world, entity, /* cache_mapping, */ &self.map)?;

        let arqs = unsafe { &self.vec.get_unchecked(local_index.index()) };
        // println!("get======{:?}", (entity, addr.archetype_index(), addr,  arch.name()));
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
            len += arqs.ar.len().index();
        }
        len
    }

    pub fn archetypes_len(&self) -> usize {
        self.vec.len()
    }
}

// 每原型、查询状态，每组件的tick，及每组件的脏监听
#[derive(Debug)]
pub struct ArchetypeQueryState<S> {
    pub(crate) ar: ShareArchetype,
    state: S,
    range: Range<u32>,
}

// 检查entity是否正确，包括对应的原型是否在本查询内，并将查询到的原型本地位置记到cache_mapping上
pub(crate) fn check<'w>(
    world: &'w World,
    entity: Entity,
    map: &Vec<ArchetypeLocalIndex>,
) -> Result<(EntityAddr, ArchetypeLocalIndex), QueryError> {
    // assert!(!entity.is_null());
    let addr = match world.entities.get(entity) {
        Some(v) => *v,
        None => return Err(QueryError::NoSuchEntity),
    };
    // println!("addr======{:?}", (entity, addr));

    let local_index  = match map.get(addr.archetype_index().index()) {
        Some(v) => if v.is_null() {
            return Err(QueryError::NoMatchArchetype);
        } else {
            *v
        },
        None => return Err(QueryError::NoMatchArchetype),
    };
    Ok((addr, local_index))
}

pub struct QueryIter<'w, Q: FetchComponents + 'static, F: FilterComponents + 'static> {
    pub(crate) world: &'w World,
    pub(crate) state: &'w QueryState<Q, F>,
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
    // 所在原型的脏监听索引
    dirty_range: Range<u32>,
    // 用来脏查询时去重row
    bitset: FixedBitSet,
}
impl<'w, Q: FetchComponents, F: FilterComponents> QueryIter<'w, Q, F> {
    /// # Safety
    /// - `world` must have permission to access any of the components registered in `query_state`.
    /// - `world` must be the same one used to initialize `query_state`.
    pub fn new(world: &'w World, state: &'w QueryState<Q, F>, tick: Tick) -> Self {
        QueryIter {
            world,
            state,
            tick,
            ar: world.empty_archetype(),
            ar_index: state.vec.len().into(),
            fetch: MaybeUninit::uninit(),
            e: Entity::null(),
            row: Row(0),
            dirty: DirtyIter::empty(),
            dirty_range: 0..0,
            bitset: FixedBitSet::new(),
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
            loop {
                // 当前的原型已经迭代完毕
                if self.ar_index.0 == 0 {
                    // 所有原型都迭代过了
                    return None;
                }
                // 下一个原型
                self.ar_index.0 -= 1;
                let arqs = unsafe { &self.state.vec.get_unchecked(self.ar_index.index()) };
                self.ar = &arqs.ar;
                self.row = self.ar.len();
                if self.row.0 > 0 {
                    self.fetch = MaybeUninit::new(Q::init_fetch(
                        self.world,
                        self.ar,
                        &arqs.state,
                        self.tick,
                        self.state.last_run,
                    ));
                    break;
                }
            }
        }
    }

    fn iter_dirtys(&mut self) -> Option<Q::Item<'w>> {
        'outer: loop {
            if let Some(d) = self.dirty.it.next() {
                self.e = self.ar.get(d.row);
                // 要求条目不为空
                if !self.e.is_null() {
                    // 检查tick
                    let tick = self.dirty.ticks.load_i(d.row.index()).unwrap();
                    if self.state.last_run < *tick {
                        if self.bitset.contains(d.row.index()) {
                            continue;
                        }
                        self.bitset.set(d.row.index(), true);
                        let item = Q::fetch(unsafe { self.fetch.assume_init_mut() }, d.row, self.e);
                        return Some(item);
                    }
                    self.row = d.row;
                }
                continue;
            }
            let mut entitys_len = 0; // 记录新原型的实体长度
            loop {
                // 检查当前原型的下一个被脏组件
                while !self.dirty_range.is_empty() {
                    self.dirty_range.end -= 1;
                    let (column_index, read_len, vec) = unsafe { self.state.archetype_listeners.get_unchecked(self.dirty_range.end as usize) };
                    let end = vec.len(); // 取脏列表的长度
                    if end > 0 {
                        let start = read_len.swap(end,  Ordering::Relaxed); // 交换读取的长度
                        if start < end {
                            self.dirty.it = vec.slice(start..end);
                            let arqs = unsafe { &self.state.vec.get_unchecked(self.ar_index.index()) };
                            let c = arqs.ar.get_column_unchecked(*column_index);
                            self.dirty.ticks = &c.ticks;
                            if entitys_len > 0 { // 表示换了新的原型，并且原型有脏数据
                                // 获取该原型的fetch
                                self.fetch = MaybeUninit::new(Q::init_fetch(
                                    self.world,
                                    self.ar,
                                    &arqs.state,
                                    self.tick,
                                    self.state.last_run,
                                ));
                                if self.bitset.len() < entitys_len {
                                    self.bitset = FixedBitSet::with_capacity(entitys_len);
                                } else {
                                    self.bitset.clear();
                                }
                            }
                            continue 'outer;
                        }
                    }
                }
                loop {
                    // 当前的原型已经迭代完毕
                    if self.ar_index.0 == 0 {
                        // 所有原型都迭代过了
                        return None;
                    }
                    // 下一个原型
                    self.ar_index.0 -= 1;
                    let arqs = unsafe { &self.state.vec.get_unchecked(self.ar_index.index()) };
                    entitys_len = arqs.ar.len().index();
                    if entitys_len > 0 { // 当前原型有实体
                        self.ar = &arqs.ar;
                        self.dirty_range = arqs.range.clone();
                        break
                    }
                }
            }
        }
    }

    fn size_hint_normal(&self) -> (usize, Option<usize>) {
        let it = self.state.vec[0..self.ar_index.index()].iter();
        let count = it.map(|arqs| arqs.ar.len()).count();
        (self.row.index(), Some(self.row.index() + count))
    }
    // fn size_hint_dirty(&self) -> (usize, Option<usize>) {
    //     // 获得当前原型的当前列的脏长度
    //     let mut c: usize = self.dirty.it.size_hint().1.unwrap_or_default();
    //     c += self.size_hint_ar_dirty(self.ar_index.index());
    //     (0, Some(c))
    // }
    // fn size_hint_ar_dirty(&self, ar_index: usize) -> usize {
    //     let mut c: usize = self.dirty.it.size_hint().1.unwrap_or_default();
    //     let arqs = unsafe { &self.state.vec.get_unchecked(self.ar_index.index()) };
    //     // 获得当前原型的剩余列的脏长度
    //     c += self.size_hint_ar_column_dirty(&arqs.ar, &arqs.listeners, self.dirty_index);
    //     for i in 0..ar_index {
    //         let arqs = unsafe { &self.state.vec.get_unchecked(i) };
    //         // 获得剩余原型的全部列的脏长度
    //         c += self.size_hint_ar_column_dirty(&arqs.ar, &arqs.listeners, arqs.listeners.len());
    //     }
    //     c
    // }
    // fn size_hint_ar_column_dirty(
    //     &self,
    //     ar: &Archetype,
    //     vec: &SmallVec<[DirtyIndex; 1]>,
    //     len: usize,
    // ) -> usize {
    //     let mut c: usize = 0;
    //     for i in 0..len {
    //         let d_index = unsafe { vec.get_unchecked(i) };
    //         let iter = ar.get_dirty_iter(d_index, self.tick);
    //         c += iter.it.size_hint().1.unwrap_or_default();
    //     }
    //     c
    // }
}

impl<'w, Q: FetchComponents, F: FilterComponents> Iterator for QueryIter<'w, Q, F> {
    type Item = Q::Item<'w>;

    fn next(&mut self) -> Option<Self::Item> {
        if F::LISTENER_COUNT == 0 {
            self.iter_normal()
        // } else if F::LISTENER_COUNT == 1 {
        //     self.iter_dirty()
        } else {
            self.iter_dirtys()
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        if F::LISTENER_COUNT == 0 {
            self.size_hint_normal()
        } else {
            self.size_hint_normal()
            // elf.size_hint_dirty()
        }
    }
}
