// //! 查询

// use core::fmt::*;
// use core::result::Result;
// use std::cell::SyncUnsafeCell;
// use std::mem::{transmute, MaybeUninit};
// use std::ops::{Deref, DerefMut};

// use crate::archetype::{ArchetypeIndex, ShareArchetype};
// use crate::fetch::FetchComponents;
// use crate::filter::FilterComponents;
// use crate::query::QueryError;
// use crate::system::{relate, Related, SystemMeta};
// use crate::system_params::SystemParam;
// use crate::world::*;
// use crate::world_ptr::Ptr;
// use fixedbitset::FixedBitSet;
// use pi_null::*;
// use pi_share::Share;


// pub struct EntryQuery<'w, Q: FetchComponents + 'static, F: FilterComponents + 'static = ()> {
//     pub(crate) state: &'w mut QueryState<Q, F>,
    
// }
// unsafe impl<'w, Q: FetchComponents, F: FilterComponents> Send for EntryQuery<'w, Q, F> {}
// unsafe impl<'w, Q: FetchComponents, F: FilterComponents> Sync for EntryQuery<'w, Q, F> {}
// impl<'w, Q: FetchComponents, F: FilterComponents> EntryQuery<'w, Q, F> {
//     pub fn new(state: &'w mut QueryState<Q, F>) -> Self {
//         EntryQuery {
//             state
//         }
//     }

//     pub fn tick(&self) -> Tick {
//         self.state.qstate.tick
//     }

//     pub fn last_run(&self) -> Tick {
//         self.state.last_run
//     }

//     pub fn contains(&self, entity: Entity) -> bool {
//         self.state.contains(entity)
//     }

//     pub fn get(
//         &self,
//         e: Entity,
//     ) -> Result<<<Q as FetchComponents>::ReadOnly as FetchComponents>::Item<'_>, QueryError> {
//         self.state
//             .as_readonly()
//             .get_by_tick(e)
//     }

//     pub fn get_mut(&mut self, e: Entity) -> Result<<Q as FetchComponents>::Item<'_>, QueryError> {
//         let r = self.state.get_by_tick(
//             e
//         );
//         unsafe { transmute(r) }
//     }

//     pub fn is_empty(&self) -> bool {
//         self.state.is_empty()
//     }

//     pub fn len(&self) -> usize {
//         self.state.len()
//     }

//     pub fn archetypes_len(&self) -> usize {
//         self.state.archetypes_len()
//     }
// }

// impl<'a, Q: FetchComponents + 'static, F: FilterComponents + Send + Sync> SystemParam
//     for EntryQuery<'a, Q, F>
// {
//     type State = QueryState<Q, F>;
//     type Item<'w> = EntryQuery<'w, Q, F>;

//     fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
//         Self::State::create(world, system_meta)
//     }
//     #[inline(always)]
//     fn align(state: &mut Self::State) {
//         state.cache_index = SyncUnsafeCell::new(ArchetypeIndex::null()); // 重置cache_index，因为原型表的指针可能会改变
//         // 不需要对齐
//     }

//     #[inline(always)]
//     fn get_param<'w>(
//         world: &'w World,
//         state: &'w mut Self::State,
//     ) -> Self::Item<'w> {
//         EntryQuery::new(state)
//     }

//     #[inline(always)]
//     fn get_self<'w>(
//         world: &'w World,
//         state: &'w mut Self::State,
//     ) -> Self {
//         unsafe { transmute(Self::get_param(world, state)) }
//     }
// }

// impl<'w, Q: FetchComponents, F: FilterComponents> Drop for EntryQuery<'w, Q, F> {
//     fn drop(&mut self) {
//         self.state.last_run = self.state.tick;
//     }
// }

// #[derive(Debug, Clone, Copy, Default)]
// pub struct LocalIndex(u16);
// impl LocalIndex {
//     pub fn index(&self) -> usize {
//         self.0 as usize
//     }
// }
// impl From<u16> for LocalIndex {
//     fn from(index: u16) -> Self {
//         Self(index)
//     }
// }
// impl From<usize> for LocalIndex {
//     fn from(index: usize) -> Self {
//         Self(index as u16)
//     }
// }
// impl pi_null::Null for LocalIndex {
//     fn null() -> Self {
//         Self(u16::null())
//     }

//     fn is_null(&self) -> bool {
//         self.0 == u16::MAX
//     }
// }
// #[derive(Debug)]
// pub struct QueryState<Q: FetchComponents + 'static, F: FilterComponents + 'static> {
//     pub(crate) fetch_state: Q::State,
//     pub(crate) filter_state: F::State,
//     pub(crate) qstate: QState,
//     // 缓存上次的索引映射关系
//     cache_index: SyncUnsafeCell<ArchetypeIndex>,
//     fetch_filter: SyncUnsafeCell<
//         MaybeUninit<(
//             <Q as FetchComponents>::Fetch<'static>,
//             <F as FilterComponents>::Filter<'static>,
//         )>,
//     >,
// }

// unsafe impl<Q: FetchComponents + 'static, F: FilterComponents + 'static> Send for QueryState<Q, F> {}
// unsafe impl<Q: FetchComponents + 'static, F: FilterComponents + 'static> Sync for QueryState<Q, F> {}

// impl<Q: FetchComponents + 'static, F: FilterComponents + 'static> Deref for QueryState<Q, F> {
//     type Target = QState;
//     fn deref(&self) -> &Self::Target {
//         &self.qstate
//     }
// }
// impl<Q: FetchComponents + 'static, F: FilterComponents + 'static> DerefMut for QueryState<Q, F> {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.qstate
//     }
// }

// impl<Q: FetchComponents, F: FilterComponents> QueryState<Q, F> {
//     pub fn as_readonly(&self) -> &QueryState<Q::ReadOnly, F> {
//         unsafe { &*(self as *const QueryState<Q, F> as *const QueryState<Q::ReadOnly, F>) }
//     }
//     pub fn create(world: &mut World, system_meta: &mut SystemMeta) -> Self {
//         let fetch_state = Q::init_state(world, system_meta);
//         let filter_state = F::init_state(world, system_meta);
//         Self {
//             fetch_state,
//             filter_state,
//             qstate: QState::new(system_meta, world),
            
//             cache_index: SyncUnsafeCell::new(ArchetypeIndex::null()),
//             fetch_filter: SyncUnsafeCell::new(MaybeUninit::uninit()),
//         }
//     }
//     pub fn contains(&self, entity: Entity) -> bool {
//         self.check(&self.world, entity).is_ok()
//     }
//     pub fn last_run(&self) -> Tick {
//         self.last_run
//     }
//     pub fn get_param<'w>(&'w mut self) -> EntryQuery<Q, F> {
//         EntryQuery::new(self)
//     }
//     pub fn get<'w>(
//         &'w self,
//         e: Entity,
//     ) -> Result<<<Q as FetchComponents>::ReadOnly as FetchComponents>::Item<'_>, QueryError> {
//         self.as_readonly()
//             .get_by_tick(e)
//     }

//     pub fn get_mut(
//         &mut self,
//         e: Entity,
//     ) -> Result<<Q as FetchComponents>::Item<'_>, QueryError> {
//         let r = self.get_by_tick( e);
//         unsafe { transmute(r) }
//     }

//     #[inline(always)]
//     pub fn get_by_tick<'w>(
//         &self,
//         e: Entity,
//     ) -> Result<Q::Item<'w>, QueryError> {
//         let addr = *self.check(&self.qstate.world, e /* cache_mapping, */)?;

//         // println!("get======{:?}", (entity, addr.archetype_index(), addr,  world.get_archetype(addr.archetype_index())));
//         if addr.archetype_index() != unsafe { *self.cache_index.get() } {
//             let fetch = Q::init_fetch(
//                 &self.qstate.world,
//                 &self.fetch_state,
//                 addr.archetype_index(),
//                 self.qstate.tick,
//                 self.last_run,
//             );
//             let filter = F::init_filter(
//                 &self.qstate.world,
//                 &self.filter_state,
//                 addr.archetype_index(),
//                 self.qstate.tick,
//                 self.last_run,
//             );

//             unsafe { *self.cache_index.get() = addr.archetype_index() };
//             unsafe { (&mut *self.fetch_filter.get()).write(transmute((fetch, filter))) };
//         };
//         let (fetch, filter) = unsafe { (&*self.fetch_filter.get()).assume_init_ref() };
//         if F::filter(filter, addr.row, e) {
//             return Err(QueryError::NoMatchEntity(e));
//         }
//         Ok(unsafe { transmute( Q::fetch(fetch, addr.row, e)) })
//     }
// }

// #[derive(Debug)]
// pub struct QState {
//     pub(crate) related: Share<Related<ComponentIndex>>, // 组件关系表
//     pub(crate) archetypes_len: usize, // 脏的最新的原型，如果world上有更新的，则检查是否和自己相关
//     pub(crate) archetypes: Vec<ShareArchetype>, // 每原型
//     pub(crate) bit_set: FixedBitSet,  // world上的原型索引是否在本地
//     pub(crate) bit_set_start: usize,
//     pub(crate) last_run: Tick, // 上次运行的tick
//     pub(crate) world: Ptr<World>,
//     pub(crate) tick: Tick, // world上的tick
// }

// impl QState {
//     pub fn new(system_meta: &mut SystemMeta, world: &mut World) -> Self {
//         let related = system_meta.related_ok();
//         Self {
//             // id,
//             related,
//             archetypes_len: 0,
//             archetypes: Vec::with_capacity(256),
//             bit_set: Default::default(),
//             bit_set_start: 0,
//             last_run: Tick::default(),
//             tick: world.tick(),
//             world: Ptr::new(world),
//         }
//     }

//     // 对齐world上新增的原型
//     pub fn align(&mut self, world: &World) {
//         let len = world.archetype_arr.len();
//         if len == self.archetypes_len {
//             return;
//         }
//         // 检查新增的原型
//         for i in self.archetypes_len..len {
//             let ar = unsafe { world.archetype_arr.get_unchecked(i) };
//             self.add_archetype(ar, i.into());
//         }
//         self.archetypes_len = len;
//     }
//     // 新增的原型
//     pub fn add_archetype(&mut self, ar: &ShareArchetype, index: ArchetypeIndex) {
//         // 判断原型是否和查询相关
//         // println!("add_archetype======{:?}", (ar.name(), self.related.relate(ar, 0), &self.related));
//         if !relate(&self.related, ar, 0) {
//             return;
//         }
//         if self.archetypes.len() == 0 {
//             self.bit_set_start = index.index();
//         }
//         let index = index.index() - self.bit_set_start;
//         self.bit_set.grow(index + 1);
//         unsafe { self.bit_set.set_unchecked(index, true) };
//         self.archetypes.push(ar.clone());
//     }
//     // 检查entity是否正确，包括对应的原型是否在本查询内，并将查询到的原型本地位置记到cache_mapping上
//     #[inline]
//     pub(crate) fn check<'w>(
//         &self,
//         world: &'w World,
//         entity: Entity,
//     ) -> Result<&'w mut EntityAddr, QueryError> {
//         // assert!(!entity.is_null());
//         let addr = match world.entities.load(entity) {
//             Some(v) => v,
//             None => return Err(QueryError::NoSuchEntity(entity)),
//         };
//         // println!("check addr======{:?}", (entity, &addr));
//         if !self.bit_set.contains(
//             addr.archetype_index()
//                 .index()
//                 .wrapping_sub(self.bit_set_start),
//         ) {
//             return Err(QueryError::NoMatchArchetype);
//         }
//         Ok(addr)
//     }
//     pub fn is_empty(&self) -> bool {
//         if self.archetypes.is_empty() {
//             return true;
//         }
//         self.len() == 0
//     }

//     pub fn len(&self) -> usize {
//         let mut len = 0;
//         for ar in &self.archetypes {
//             len += ar.len().index();
//         }
//         len
//     }

//     pub fn archetypes_len(&self) -> usize {
//         self.archetypes.len()
//     }
// }

