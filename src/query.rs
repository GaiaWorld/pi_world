use core::fmt::*;
use core::result::Result;
use std::any::TypeId;
use std::collections::HashMap;
use std::marker::PhantomData;

use crate::archetype::{
    Archetype, ArchetypeDepend, ArchetypeDependResult, Flags, Row, ShareArchetype,
    ArchetypeWorldIndex,
};
use crate::dirty::DirtyIndex;
use crate::fetch::FetchComponents;
use crate::filter::FilterComponents;
use crate::listener::Listener;
use crate::system::SystemMeta;
use crate::system_parms::SystemParam;
use crate::world::*;
use fixedbitset::FixedBitSet;
use pi_arr::Iter;
use pi_null::*;
use pi_share::Share;
use smallvec::SmallVec;

#[derive(Debug)]
pub enum QueryError {
    MissingComponent,
    NoSuchArchetype,
    NoMatchEntity,
    NoSuchEntity,
    NoSuchRow,
}

pub struct Query<'world, Q: FetchComponents + 'static, F: FilterComponents + 'static = ()> {
    pub(crate) world: &'world World,
    pub(crate) state: &'world mut QueryState<Q, F>,
    pub(crate) tick: Tick,
}

impl<'world, Q: FetchComponents, F: FilterComponents> Query<'world, Q, F> {
    #[inline]
    pub fn new(world: &'world World, state: &'world mut QueryState<Q, F>, tick: Tick) -> Self {
        Query { world, state, tick }
    }
    #[inline]
    pub fn get(
        &'world mut self,
        e: Entity,
    ) -> Result<<Q as FetchComponents>::Item<'world>, QueryError> {
        self.state.get(self.world, e)
    }
    #[inline]
    pub fn tick(&self) -> Tick {
        self.tick
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
    pub fn iter(&'world self) -> QueryIter<'world, Q, F> {
        QueryIter::new(self.world, &self.state)
    }
    // pub fn iter_mut(&'world mut self) -> QueryIterMut<'world, Q, F> {
    //     todo!()
    // }
}

// SAFETY: Relevant query ComponentId and ArchetypeComponentId access is applied to SystemMeta. If
// this Query conflicts with any prior access, a panic will occur.
impl<Q: FetchComponents + 'static, F: FilterComponents + Send + Sync + 'static> SystemParam
    for Query<'_, Q, F>
{
    type State = QueryState<Q, F>;
    type Item<'w> = Query<'w, Q, F>;

    fn init_state(world: &World, _system_meta: &mut SystemMeta) -> Self::State {
        let mut listeners = Default::default();
        F::init_listeners(world, &mut listeners);
        if F::LISTENER_COUNT > 0 {
            // 遍历已有的原型， 添加record
            let notify = Notify(
                listeners.clone(),
                PhantomData,
                PhantomData::<Q>,
                PhantomData::<F>,
            );
            for r in world.archetype_arr.iter() {
                notify.listen(ArchetypeInit(r, world))
            }
            // 监听原型创建， 添加record
            world.listener_mgr.register_event(Share::new(notify));
        }
        QueryState::new(listeners, Q::init_state(world, &world.empty_archetype))
    }
    fn depend(
        _world: &World,
        _system_meta: &SystemMeta,
        _state: &Self::State,
        archetype: &Archetype,
        result: &mut ArchetypeDependResult,
    ) {
        Q::archetype_depend(archetype, result);
        if F::archetype_filter(archetype) {
            result.merge(ArchetypeDepend::Flag(Flags::WRITE));
        }
    }

    #[inline]
    fn before(
        state: &mut Self::State,
        _system_meta: &mut SystemMeta,
        world: &World,
        _change_tick: Tick,
    ) {
        state.align(world);
    }

    #[inline]
    fn get_param<'world>(
        state: &'world mut Self::State,
        _system_meta: &'world SystemMeta,
        world: &'world World,
        change_tick: Tick,
    ) -> Self::Item<'world> {
        // SAFETY: We have registered all of the query's world accesses,
        // so the caller ensures that `world` has permission to access any
        // world data that the query needs.
        Query::new(world, state, change_tick)
    }
}

/// 监听原型创建， 添加record
pub struct Notify<'a, Q: FetchComponents, F: FilterComponents>(
    SmallVec<[(TypeId, bool); 1]>,
    PhantomData<&'a ()>,
    PhantomData<Q>,
    PhantomData<F>,
);
impl<'a, Q: FetchComponents + 'static, F: FilterComponents + 'static> Listener
    for Notify<'a, Q, F>
{
    type Event = ArchetypeInit<'a>;
    fn listen(&self, ar: Self::Event) {
        if !QueryState::<Q, F>::relate(ar.0) {
            return;
        }
        unsafe {
            ar.0.add_dirty_listeners(TypeId::of::<QueryState<Q, F>>(), &self.0)
        };
    }
}

pub type ArchetypeLocalIndex = usize;

#[derive(Debug)]
pub struct QueryState<Q: FetchComponents + 'static, F: FilterComponents + 'static> {
    pub(crate) listeners: SmallVec<[(TypeId, bool); 1]>,
    pub(crate) vec: Vec<(ShareArchetype, Q::State, SmallVec<[DirtyIndex; 1]>)>, // 每原型、查询状态及对应的脏监听
    pub(crate) archetype_len: usize, // 脏的最新的原型，如果world上有更新的，则检查是否和自己相关
    pub(crate) map: HashMap<ArchetypeWorldIndex, ArchetypeLocalIndex>, // 脏world上的原型索引对于本地的原型索引
    pub(crate) last: (ArchetypeWorldIndex, ArchetypeLocalIndex), // todo!() 改到Query上来支持并发
    empty: Q::State,
    _k: PhantomData<F>,
}
impl<Q: FetchComponents, F: FilterComponents> QueryState<Q, F> {
    pub fn new(listeners: SmallVec<[(TypeId, bool); 1]>, empty: Q::State) -> Self {
        Self {
            listeners,
            vec: Vec::new(),
            archetype_len: 0,
            map: Default::default(),
            last: (ArchetypeWorldIndex::null(), 0),
            empty,
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
        result.flag.bits() != 0 && (result.flag & Flags::WITHOUT).bits() == 0
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
    pub fn add_archetype(&mut self, world: &World, ar: &ShareArchetype, index: ArchetypeWorldIndex) {
        // 判断原型是否和查询相关
        if !Self::relate(ar) {
            return;
        }
        let mut vec = SmallVec::new();
        if F::LISTENER_COUNT > 0 {
            ar.find_dirty_listeners(TypeId::of::<Self>(), &self.listeners, &mut vec);
            if vec.len() == 0 {
                // 表示该原型没有监听的组件，本查询可以不关心该原型
                return;
            }
        }
        self.map.insert(index, self.vec.len() as ArchetypeLocalIndex);
        self.vec.push((ar.clone(), Q::init_state(world, ar), vec));
    }
    pub fn get<'w>(
        &'w mut self,
        world: &'w World,
        entity: Entity,
    ) -> Result<Q::Item<'w>, QueryError> {
        let addr = check(world, entity, &mut self.last, &self.map)?;
        let ar = unsafe { &self.vec.get_unchecked(self.last.1) };
        let mut fetch = Q::init_fetch(world, &ar.0, &ar.1);
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
        for ar in &self.vec {
            len += ar.0.len();
        }
        len
    }
}

// 检查entity是否正确，包括对应的原型是否在本查询内，并将查询到的原型本地位置记到last上
pub(crate) fn check<'w>(
    world: &'w World,
    entity: Entity,
    last: &mut (ArchetypeWorldIndex, ArchetypeLocalIndex),
    map: &HashMap<ArchetypeWorldIndex, ArchetypeLocalIndex>,
) -> Result<EntityAddr, QueryError> {
    let addr = match world.entitys.get(entity) {
        Some(v) => *v,
        None => return Err(QueryError::NoSuchEntity),
    };
    if last.0 != addr.ar_index {
        last.1 = match map.get(&addr.ar_index) {
            Some(v) => *v,
            None => return Err(QueryError::NoSuchArchetype),
        };
    }
    Ok(addr)
}

pub struct QueryIter<'w, Q: FetchComponents + 'static, F: FilterComponents + 'static> {
    pub(crate) world: &'w World,
    state: &'w QueryState<Q, F>,
    // 原型的位置
    pub(crate) ar_index: ArchetypeLocalIndex,
    // 原型
    pub(crate) ar: &'w Archetype,
    fetch: Q::Fetch<'w>,
    pub(crate) row: Row,
    // 脏迭代器，监听多个组件变化，可能entity相同，需要进行去重
    dirty: (Iter<'w, Row>, usize),
    bitset: FixedBitSet, // 用来脏查询时去重row
}
impl<'w, Q: FetchComponents, F: FilterComponents> QueryIter<'w, Q, F> {
    /// # Safety
    /// - `world` must have permission to access any of the components registered in `query_state`.
    /// - `world` must be the same one used to initialize `query_state`.
    #[inline(always)]
    pub(crate) fn new(world: &'w World, state: &'w QueryState<Q, F>) -> Self {
        let len = state.vec.len();
        // 该查询没有关联的原型
        if len == 0 {
            return QueryIter {
                world,
                state,
                ar: world.empty_archetype(),
                ar_index: 0,
                fetch: Q::init_fetch(world, world.empty_archetype(), &state.empty),
                row: 0,
                dirty: (Iter::empty(), 0),
                bitset: FixedBitSet::new(),
            };
        }
        let ar_index = len - 1;
        let ar = unsafe { state.vec.get_unchecked(ar_index) };
        let fetch = Q::init_fetch(world, &ar.0, &ar.1);
        if F::LISTENER_COUNT == 0 {
            // 该查询没有监听组件变化，倒序迭代原型的行
            QueryIter {
                world,
                state,
                ar: &ar.0,
                ar_index,
                fetch,
                row: ar.0.table.len(),
                dirty: (Iter::empty(), 0),
                bitset: FixedBitSet::new(),
            }
        } else {
            // 该查询有组件变化监听器， 倒序迭代所脏的原型
            let len = ar.2.len() - 1;
            let d_index = unsafe { *ar.2.get_unchecked(len) };
            QueryIter {
                world,
                state,
                ar: &ar.0,
                ar_index,
                fetch,
                row: u32::null(),
                dirty: (d_index.get_iter(&ar.0), len),
                bitset: FixedBitSet::with_capacity(ar.0.len()),
            }
        }
    }

    #[inline(always)]
    fn iter_normal(&mut self) -> Option<Q::Item<'w>> {
        loop {
            if self.row > 0 {
                self.row -= 1;
                let e = self.ar.table.get(self.row);
                // 要求条目不为空
                if !e.is_null() {
                    let item = Q::fetch(&mut self.fetch, self.row, e);
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
            let ar = unsafe { &self.state.vec.get_unchecked(self.ar_index) };
            self.ar = &ar.0;
            self.fetch = Q::init_fetch(self.world, self.ar, &ar.1);
            self.row = self.ar.table.len();
        }
    }

    #[inline]
    fn iter_dirty(&mut self) -> Option<Q::Item<'w>> {
        loop {
            if let Some(row) = self.dirty.0.next() {
                if self.bitset.contains(*row as usize) {
                    continue;
                }
                self.bitset.set(*row as usize, true);
                self.row = *row;
                let e = self.ar.table.get(self.row);
                // 要求条目不为空
                if !e.is_null() {
                    let item = Q::fetch(&mut self.fetch, self.row, e);
                    return Some(item);
                }
                continue;
            }
            // 检查当前原型的下一个被脏组件
            if self.dirty.1 > 0 {
                let len = self.dirty.1 - 1;
                let ar = unsafe { &self.state.vec.get_unchecked(self.ar_index) };
                let d_index = unsafe { *ar.2.get_unchecked(len) };
                let iter = d_index.get_iter(&ar.0);
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
            let ar = unsafe { &self.state.vec.get_unchecked(self.ar_index) };
            self.ar = &ar.0;
            self.fetch = Q::init_fetch(self.world, self.ar, &ar.1);
            // 监听被脏组件
            let len = ar.2.len() - 1;
            let d_index = unsafe { *ar.2.get_unchecked(len) };
            let iter = d_index.get_iter(&ar.0);
            self.dirty = (iter, len);
            let len = ar.0.len();
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
        let count = it.map(|(ar, _, _)| ar.table.len()).count();
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
        let ar = unsafe { &self.state.vec.get_unchecked(self.ar_index) };
        // 获得当前原型的剩余列的脏长度
        c += self.size_hint_ar_column_dirty(&ar.0, &ar.2, self.dirty.1);
        for i in 0..ar_index {
            let ar = unsafe { &self.state.vec.get_unchecked(i) };
            // 获得剩余原型的全部列的脏长度
            c += self.size_hint_ar_column_dirty(&ar.0, &ar.2, ar.2.len());
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
