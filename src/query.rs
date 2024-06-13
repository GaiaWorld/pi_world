//! 查询

use core::fmt::*;
use core::result::Result;
use std::cell::SyncUnsafeCell;
use std::mem::{transmute, MaybeUninit};
use std::ops::{Deref, DerefMut};

use crate::archetype::{Archetype, ArchetypeIndex, Row, ShareArchetype};
use crate::fetch::FetchComponents;
use crate::filter::FilterComponents;
use crate::system::{relate, Related, SystemMeta};
use crate::system_params::SystemParam;
use crate::world::*;
use fixedbitset::FixedBitSet;
use pi_null::*;
use pi_share::Share;

#[derive(Debug, PartialEq, Eq)]
pub enum QueryError {
    MissingComponent(ComponentIndex, ArchetypeIndex),
    NoMatchArchetype,
    NoMatchEntity(Entity),
    NoSuchComponent(ComponentIndex),
    NoSuchEntity(Entity),
    NoSuchRow(Row),
    NoSuchRes,
    RepeatAlter,
}

pub struct Queryer<'w, Q: FetchComponents + 'static, F: FilterComponents + 'static = ()> {
    pub(crate) world: &'w World,
    pub(crate) state: QueryState<Q, F>,
    pub(crate) tick: Tick,
    // 缓存上次的索引映射关系
    cache_index: SyncUnsafeCell<ArchetypeIndex>,
    fetch_filter: SyncUnsafeCell<
        MaybeUninit<(
            <Q as FetchComponents>::Fetch<'w>,
            <F as FilterComponents>::Filter<'w>,
        )>,
    >,
}
impl<'w, Q: FetchComponents + 'static, F: FilterComponents + 'static> Queryer<'w, Q, F> {
    pub(crate) fn new(world: &'w World, state: QueryState<Q, F>) -> Self {
        let tick = world.increment_tick();
        Self {
            world,
            state,
            tick,
            cache_index: SyncUnsafeCell::new(ArchetypeIndex::null()),
            fetch_filter: SyncUnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    pub fn contains(&self, entity: Entity) -> bool {
        self.state.contains(self.world, entity)
    }

    pub fn get<'a>(
        &'a self,
        e: Entity,
    ) -> Result<<<Q as FetchComponents>::ReadOnly as FetchComponents>::Item<'_>, QueryError> {
        self.state
            .as_readonly()
            .get1(self.world, self.tick, e, &self.cache_index, unsafe {
                transmute(&self.fetch_filter)
            })
    }

    pub fn get_mut(&mut self, e: Entity) -> Result<<Q as FetchComponents>::Item<'_>, QueryError> {
        let r = self.state.get1(
            self.world,
            self.tick,
            e,
            &self.cache_index,
            &self.fetch_filter,
        );
        unsafe { transmute(r) }
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

pub struct Query<'w, Q: FetchComponents + 'static, F: FilterComponents + 'static = ()> {
    pub(crate) world: &'w World,
    pub(crate) state: &'w mut QueryState<Q, F>,
    pub(crate) tick: Tick,
    // 缓存上次的索引映射关系
    cache_index: SyncUnsafeCell<ArchetypeIndex>,
    fetch_filter: SyncUnsafeCell<
        MaybeUninit<(
            <Q as FetchComponents>::Fetch<'w>,
            <F as FilterComponents>::Filter<'w>,
        )>,
    >,
}
unsafe impl<'w, Q: FetchComponents, F: FilterComponents> Send for Query<'w, Q, F> {}
unsafe impl<'w, Q: FetchComponents, F: FilterComponents> Sync for Query<'w, Q, F> {}
impl<'w, Q: FetchComponents, F: FilterComponents> Query<'w, Q, F> {
    pub fn new(world: &'w World, state: &'w mut QueryState<Q, F>, tick: Tick) -> Self {
        Query {
            world,
            state,
            tick,
            cache_index: SyncUnsafeCell::new(ArchetypeIndex::null()),
            fetch_filter: SyncUnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    pub fn tick(&self) -> Tick {
        self.tick
    }

    pub fn last_run(&self) -> Tick {
        self.state.last_run
    }

    pub fn contains(&self, entity: Entity) -> bool {
        self.state.contains(self.world, entity)
    }

    pub fn get(
        &self,
        e: Entity,
    ) -> Result<<<Q as FetchComponents>::ReadOnly as FetchComponents>::Item<'_>, QueryError> {
        self.state
            .as_readonly()
            .get1(self.world, self.tick, e, &self.cache_index, unsafe {
                transmute(&self.fetch_filter) // unsafe transmute 要求所有的FetchComponents的ReadOnly和Fetch类型是相同的
            })
    }

    pub fn get_mut(&mut self, e: Entity) -> Result<<Q as FetchComponents>::Item<'_>, QueryError> {
        let r = self.state.get1(
            self.world,
            self.tick,
            e,
            &self.cache_index,
            &self.fetch_filter,
        );
        unsafe { transmute(r) }
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
        Self::State::create(world, system_meta)
    }
    fn align(world: &World, _system_meta: &SystemMeta, state: &mut Self::State) {
        state.align(world);
    }

    fn get_param<'w>(
        world: &'w World,
        _system_meta: &'w SystemMeta,
        state: &'w mut Self::State,
        tick: Tick,
    ) -> Self::Item<'w> {
        Query::new(world, state, tick)
    }

    fn get_self<'w>(
        world: &'w World,
        system_meta: &'w SystemMeta,
        state: &'w mut Self::State,
        tick: Tick,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, system_meta, state, tick)) }
    }
}

impl<'w, Q: FetchComponents, F: FilterComponents> Drop for Query<'w, Q, F> {
    fn drop(&mut self) {
        self.state.last_run = self.tick;
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LocalIndex(u16);
impl LocalIndex {
    pub fn index(&self) -> usize {
        self.0 as usize
    }
}
impl From<u16> for LocalIndex {
    fn from(index: u16) -> Self {
        Self(index)
    }
}
impl From<usize> for LocalIndex {
    fn from(index: usize) -> Self {
        Self(index as u16)
    }
}
impl pi_null::Null for LocalIndex {
    fn null() -> Self {
        Self(u16::null())
    }

    fn is_null(&self) -> bool {
        self.0 == u16::MAX
    }
}
#[derive(Debug)]
pub struct QueryState<Q: FetchComponents + 'static, F: FilterComponents + 'static> {
    pub(crate) fetch_state: Q::State,
    pub(crate) filter_state: F::State,
    pub(crate) qstate: QState,
}

impl<Q: FetchComponents + 'static, F: FilterComponents + 'static> Deref for QueryState<Q, F> {
    type Target = QState;
    fn deref(&self) -> &Self::Target {
        &self.qstate
    }
}
impl<Q: FetchComponents + 'static, F: FilterComponents + 'static> DerefMut for QueryState<Q, F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.qstate
    }
}

impl<Q: FetchComponents, F: FilterComponents> QueryState<Q, F> {
    pub fn as_readonly(&self) -> &QueryState<Q::ReadOnly, F> {
        unsafe { &*(self as *const QueryState<Q, F> as *const QueryState<Q::ReadOnly, F>) }
    }
    pub fn create(world: &mut World, system_meta: &mut SystemMeta) -> Self {
        // let id = world.increment_tick();
        let fetch_state = Q::init_state(world, system_meta);
        let filter_state = F::init_state(world, system_meta);
        Self {
            fetch_state,
            filter_state,
            qstate: QState::new(system_meta),
        }
    }
    pub fn contains(&self, world: &World, entity: Entity) -> bool {
        self.check(world, entity).is_ok()
    }
    pub fn last_run(&self) -> Tick {
        self.last_run
    }
    pub fn get<'w>(
        &'w self,
        world: &'w World,
        e: Entity,
    ) -> Result<<<Q as FetchComponents>::ReadOnly as FetchComponents>::Item<'_>, QueryError> {
        let tick = world.tick();
        let cache_index = SyncUnsafeCell::new(ArchetypeIndex::null());
        let fetch_filter = SyncUnsafeCell::new(MaybeUninit::uninit());
        self.as_readonly().get1(
            world,
            tick,
            e,
            &cache_index,
            &fetch_filter,
        )
    }

    pub fn get_mut(
        &mut self,
        world: &mut World,
        e: Entity,
    ) -> Result<<Q as FetchComponents>::Item<'_>, QueryError> {
        let tick = world.tick();
        let cache_index = SyncUnsafeCell::new(ArchetypeIndex::null());
        let fetch_filter = SyncUnsafeCell::new(MaybeUninit::uninit());
        let r = self.get1(world, tick, e, &cache_index, &fetch_filter);
        unsafe { transmute(r) }
    }
    pub fn iter<'w>(
        &'w self,
        world: &'w World,
    ) -> QueryIter<'_, <Q as FetchComponents>::ReadOnly, F> {
        let tick = world.tick();
        QueryIter::new(world, self.as_readonly(), tick)
    }
    pub fn iter_mut<'w>(&'w mut self, world: &'w mut World) -> QueryIter<'_, Q, F> {
        let tick = world.tick();
        QueryIter::new(world, self, tick)
    }

    #[inline(always)]
    pub fn get1<'w>(
        &self,
        world: &'w World,
        tick: Tick,
        e: Entity,
        cache_index: &SyncUnsafeCell<ArchetypeIndex>,
        fetch_filter: &SyncUnsafeCell<
            MaybeUninit<(
                <Q as FetchComponents>::Fetch<'w>,
                <F as FilterComponents>::Filter<'w>,
            )>,
        >,
    ) -> Result<Q::Item<'w>, QueryError> {
        let addr = *self.check(world, e /* cache_mapping, */)?;

        // println!("get======{:?}", (entity, addr.archetype_index(), addr,  world.get_archetype(addr.archetype_index())));
        if addr.archetype_index() != unsafe { *cache_index.get() } {
            let fetch = Q::init_fetch(
                world,
                &self.fetch_state,
                addr.archetype_index(),
                tick,
                self.last_run,
            );
            let filter = F::init_filter(
                world,
                &self.filter_state,
                addr.archetype_index(),
                tick,
                self.last_run,
            );

            unsafe { *cache_index.get() = addr.archetype_index() };
            unsafe { (&mut *fetch_filter.get()).write(transmute((fetch, filter))) };
        };
        let (fetch, filter) = unsafe { (&*fetch_filter.get()).assume_init_ref() };
        if F::filter(filter, addr.row, e) {
            return Err(QueryError::NoMatchEntity(e));
        }
        Ok(Q::fetch(fetch, addr.row, e))
    }
}

#[derive(Debug)]
pub struct QState {
    pub(crate) related: Share<Related<ComponentIndex>>, // 组件关系表
    pub(crate) archetypes_len: usize, // 脏的最新的原型，如果world上有更新的，则检查是否和自己相关
    pub(crate) archetypes: Vec<ShareArchetype>, // 每原型
    pub(crate) bit_set: FixedBitSet,  // world上的原型索引是否在本地
    pub(crate) bit_set_start: usize,
    pub(crate) last_run: Tick, // 上次运行的tick
}

impl QState {
    pub fn new(system_meta: &mut SystemMeta) -> Self {
        let related = system_meta.related_ok();
        Self {
            // id,
            related,
            archetypes_len: 0,
            archetypes: Vec::new(),
            bit_set: Default::default(),
            bit_set_start: 0,
            last_run: Tick::default(),
        }
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
            self.add_archetype(ar, i.into());
        }
        self.archetypes_len = len;
    }
    // 新增的原型
    pub fn add_archetype(&mut self, ar: &ShareArchetype, index: ArchetypeIndex) {
        // 判断原型是否和查询相关
        // println!("add_archetype======{:?}", (ar.name(), self.related.relate(ar, 0), &self.related));
        if !relate(&self.related, ar, 0) {
            return;
        }
        if self.archetypes.len() == 0 {
            self.bit_set_start = index.index();
        }
        let index = index.index() - self.bit_set_start;
        self.bit_set.grow(index + 1);
        unsafe { self.bit_set.set_unchecked(index, true) };
        self.archetypes.push(ar.clone());
    }
    // 检查entity是否正确，包括对应的原型是否在本查询内，并将查询到的原型本地位置记到cache_mapping上
    #[inline]
    pub(crate) fn check<'w>(
        &self,
        world: &'w World,
        entity: Entity,
    ) -> Result<&'w mut EntityAddr, QueryError> {
        // assert!(!entity.is_null());
        let addr = match world.entities.load(entity) {
            Some(v) => v,
            None => return Err(QueryError::NoSuchEntity(entity)),
        };
        // println!("check addr======{:?}", (entity, &addr));
        if !self.bit_set.contains(
            addr.archetype_index()
                .index()
                .wrapping_sub(self.bit_set_start),
        ) {
            return Err(QueryError::NoMatchArchetype);
        }
        Ok(addr)
    }
    pub fn is_empty(&self) -> bool {
        if self.archetypes.is_empty() {
            return true;
        }
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        let mut len = 0;
        for ar in &self.archetypes {
            len += ar.len().index();
        }
        len
    }

    pub fn archetypes_len(&self) -> usize {
        self.archetypes.len()
    }
}

pub struct QueryIter<'w, Q: FetchComponents + 'static, F: FilterComponents + 'static> {
    pub(crate) world: &'w World,
    pub(crate) state: &'w QueryState<Q, F>,
    pub(crate) tick: Tick,
    // 原型的位置
    pub(crate) ar_index: LocalIndex,
    // 原型
    pub(crate) ar: &'w Archetype,
    fetch_filter: MaybeUninit<(Q::Fetch<'w>, F::Filter<'w>)>,
    pub(crate) e: Entity,
    pub(crate) row: Row,
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
            ar_index: state.archetypes.len().into(),
            fetch_filter: MaybeUninit::uninit(),
            e: Entity::null(),
            row: Row(0),
        }
    }
    #[inline(always)]
    pub fn entity(&self) -> Entity {
        self.e
    }
    fn next_archetype(&mut self) {
        // 下一个原型
        self.ar_index.0 -= 1;
        self.ar = unsafe { &self.state.archetypes.get_unchecked(self.ar_index.index()) };
        self.row = self.ar.len();
        if self.row.0 > 0 {
            let fetch = Q::init_fetch(
                self.world,
                &self.state.fetch_state,
                self.ar.index(),
                self.tick,
                self.state.last_run,
            );
            let filter = F::init_filter(
                self.world,
                &self.state.filter_state,
                self.ar.index(),
                self.tick,
                self.state.last_run,
            );
            self.fetch_filter = MaybeUninit::new((fetch, filter));
        }
    }
    #[inline(always)]
    fn iter_normal(&mut self) -> Option<Q::Item<'w>> {
        loop {
            // println!("iter_normal: {:?}", (self.e, self.row, self.ar.index(), self.ar.name()));
            if self.row.0 > 0 {
                self.row.0 -= 1;
                self.e = self.ar.get(self.row);
                // 要求条目不为空
                // println!("iter_normal1: {:?}", (self.e, self.row));
                if !self.e.is_null() {
                    // println!("iter_normal1111: {:?}", (self.e, self.row));
                    if F::filter(
                        unsafe { &self.fetch_filter.assume_init_mut().1 },
                        self.row,
                        self.e,
                    ) {
                        continue;
                    }
                    // println!("iter_normal2222: {:?}", (self.e, self.row));
                    let item = Q::fetch(
                        unsafe { &self.fetch_filter.assume_init_mut().0 },
                        self.row,
                        self.e,
                    );
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
            self.next_archetype();
        }
    }

    fn size_hint_normal(&self) -> (usize, Option<usize>) {
        let it = self.state.archetypes[0..self.ar_index.index()].iter();
        let count = it.map(|ar| ar.len()).count();
        (self.row.index(), Some(self.row.index() + count))
    }
}

impl<'w, Q: FetchComponents, F: FilterComponents> Iterator for QueryIter<'w, Q, F> {
    type Item = Q::Item<'w>;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter_normal()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.size_hint_normal()
    }
}
