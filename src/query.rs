//! 查询

use core::fmt::*;
use core::result::Result;
use std::cell::UnsafeCell;
use std::mem::{transmute, MaybeUninit};
use std::ops::{Deref, DerefMut};

use crate::archetype::{Archetype, ArchetypeIndex, Row, ShareArchetype};
use crate::fetch::FetchComponents;
use crate::filter::{self, FilterComponents};
use crate::system::{Related, SystemMeta};
use crate::system_params::SystemParam;
use crate::utils::VecExt;
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

pub struct Queryer<'world, Q: FetchComponents + 'static, F: FilterComponents + 'static = ()> {
    pub(crate) world: &'world World,
    pub(crate) state: QueryState<Q, F>,
    pub(crate) tick: Tick,
    // 缓存上次的索引映射关系
    pub(crate) cache_mapping: UnsafeCell<(ArchetypeIndex, ArchetypeLocalIndex)>,
}
impl<'world, Q: FetchComponents + 'static, F: FilterComponents + 'static> Queryer<'world, Q, F> {
    pub(crate) fn new(world: &'world World, state: QueryState<Q, F>) -> Self {
        let tick = world.increment_tick();
        let cache_mapping = UnsafeCell::new((ArchetypeIndex::null(), ArchetypeLocalIndex(0)));
        Self {
            world,
            state,
            tick,
            cache_mapping,
        }
    }

    pub fn contains(&self, entity: Entity) -> bool {
        self.state.check(self.world, entity).is_ok()
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
        self.state.as_readonly().get(
            self.world, self.tick,
            e, /* unsafe {
                  &mut *self.cache_mapping.get()
              } */
        )
    }

    pub fn get_mut(&mut self, e: Entity) -> Result<<Q as FetchComponents>::Item<'_>, QueryError> {
        self.state.get(
            self.world, self.tick, e, /* self.cache_mapping.get_mut() */
        )
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
        Query { world, state, tick }
    }

    pub fn tick(&self) -> Tick {
        self.tick
    }

    pub fn last_run(&self) -> Tick {
        self.state.last_run
    }

    pub fn contains(&self, entity: Entity) -> bool {
        self.state.check(self.world, entity).is_ok()
    }

    pub fn get(
        &self,
        e: Entity,
    ) -> Result<<<Q as FetchComponents>::ReadOnly as FetchComponents>::Item<'_>, QueryError> {
        self.state.as_readonly().get(
            self.world, self.tick,
            e, /* unsafe {
                  &mut *self.cache_mapping.get()
              } */
        )
    }

    pub fn get_mut(&mut self, e: Entity) -> Result<<Q as FetchComponents>::Item<'_>, QueryError> {
        self.state.get(
            self.world, self.tick, e, /* self.cache_mapping.get_mut() */
        )
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

impl<'world, Q: FetchComponents, F: FilterComponents> Drop for Query<'world, Q, F> {
    fn drop(&mut self) {
        self.state.last_run = self.tick;
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ArchetypeLocalIndex(u16);
impl ArchetypeLocalIndex {
    pub fn index(&self) -> usize {
        self.0 as usize
    }
}
impl From<u16> for ArchetypeLocalIndex {
    fn from(index: u16) -> Self {
        Self(index)
    }
}
impl From<usize> for ArchetypeLocalIndex {
    fn from(index: usize) -> Self {
        Self(index as u16)
    }
}
impl pi_null::Null for ArchetypeLocalIndex {
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
    pub fn get<'w>(
        &'w self,
        world: &'w World,
        tick: Tick,
        entity: Entity,
        // cache_mapping: &mut (ArchetypeWorldIndex, ArchetypeLocalIndex),
    ) -> Result<Q::Item<'w>, QueryError> {
        let addr = *self.check(world, entity /* cache_mapping, */)?;

        // println!("get======{:?}", (entity, addr.archetype_index(), addr,  ar.name()));
        let filter = F::init_filter(
            world,
            &self.filter_state,
            addr.archetype_index(),
            tick,
            self.last_run,
        );
        if F::filter(&filter, addr.row, entity) {
            return Err(QueryError::NoMatchEntity(entity));
        }
        let mut fetch = Q::init_fetch(
            world,
            &self.fetch_state,
            addr.archetype_index(),
            tick,
            self.last_run,
        );
        Ok(Q::fetch(&mut fetch, addr.row, entity))
    }
}

#[derive(Debug)]
pub struct QState {
    pub(crate) related: Share<Related>,         // 组件关系表
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
        if !self.related.relate(ar, 0) {
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
        println!("check addr======{:?}", (entity, &addr));
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

// // 每原型、查询状态，每组件的tick，及每组件的脏监听
// #[derive(Debug)]
// pub struct ArchetypeQueryState<S> {
//     pub(crate) ar: ShareArchetype,
//     state: S,
//     range: Range<u32>,
// }

pub struct QueryIter<'w, Q: FetchComponents + 'static, F: FilterComponents + 'static> {
    pub(crate) world: &'w World,
    pub(crate) state: &'w QueryState<Q, F>,
    pub(crate) tick: Tick,
    // 原型的位置
    pub(crate) ar_index: ArchetypeLocalIndex,
    // 原型
    pub(crate) ar: &'w Archetype,
    fetch: MaybeUninit<(Q::Fetch<'w>, F::Filter<'w>)>,
    pub(crate) e: Entity,
    pub(crate) row: Row,
    // // 脏迭代器，监听多个组件变化，可能entity相同，需要进行去重
    // dirty: DirtyIter<'w>,
    // // 所在原型的脏监听索引
    // dirty_range: Range<u32>,
    // // 用来脏查询时去重row
    // bitset: FixedBitSet,
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
            fetch: MaybeUninit::uninit(),
            e: Entity::null(),
            row: Row(0),
            // dirty: DirtyIter::empty(),
            // dirty_range: 0..0,
            // bitset: FixedBitSet::new(),
        }
    }
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
            self.fetch = MaybeUninit::new((fetch, filter));
        }
    }
    fn iter_normal(&mut self) -> Option<Q::Item<'w>> {
        loop {
            // println!("iter_normal: {:?}", (self.e, self.row, self.ar.name()));
            if self.row.0 > 0 {
                self.row.0 -= 1;
                self.e = self.ar.get(self.row);
                // 要求条目不为空
                // println!("iter_normal: {:?}", (self.e, self.row));
                if !self.e.is_null() {
                    // println!("iter_normal1111: {:?}", (self.e, self.row));
                    if F::filter(unsafe { &self.fetch.assume_init_mut().1 }, self.row, self.e) {
                        continue;
                    }
                    // println!("iter_normal2222: {:?}", (self.e, self.row));
                    let item =
                        Q::fetch(unsafe { &self.fetch.assume_init_mut().0 }, self.row, self.e);
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
    // fn size_hint_dirty(&self) -> (usize, Option<usize>) {
    //     let mut c: usize = 0;
    //     for (_, _, vec) in self.state.archetype_listeners.iter() {
    //         c += vec.len();
    //     }
    //     (0, Some(c))
    // }
}

impl<'w, Q: FetchComponents, F: FilterComponents> Iterator for QueryIter<'w, Q, F> {
    type Item = Q::Item<'w>;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        // if F::LISTENER_COUNT == 0 {
        //     self.iter_normal()
        // // } else if F::LISTENER_COUNT == 1 {
        // //     self.iter_dirty()
        // } else {
        self.iter_normal()
        // }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        // if F::LISTENER_COUNT == 0 {
        //     self.size_hint_normal()
        // } else {
        self.size_hint_normal()
        // }
    }
}
