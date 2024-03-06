//! 查询，支持子查询

use core::fmt::*;
use core::result::Result;
use std::any::TypeId;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::marker::PhantomData;

use crate::archetype::{
    Archetype, ArchetypeDepend, ArchetypeDependResult, ArchetypeWorldIndex, Flags, Row,
    ShareArchetype,
};
use crate::dirty::{DirtyIndex, EntityDirty};
use crate::fetch::FetchComponents;
use crate::filter::FilterComponents;
use crate::listener::Listener;
use crate::system::{ReadWrite, SystemMeta};
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

pub struct Query<
    'world,
    Q: FetchComponents + 'static,
    F: FilterComponents + 'static = (),
    S: FetchComponents + 'static = (),
> {
    pub(crate) world: &'world World,
    pub(crate) state: &'world mut QueryState<Q, F, S>,
    // 缓存上次的索引映射关系
    pub(crate) cache_mapping: UnsafeCell<(ArchetypeWorldIndex, ArchetypeLocalIndex)>,
}

impl<'world, Q: FetchComponents, F: FilterComponents, S: FetchComponents> Query<'world, Q, F, S> {
    #[inline]
    pub fn new(world: &'world World, state: &'world mut QueryState<Q, F, S>) -> Self {
        let cache_mapping = UnsafeCell::new(state.cache_mapping);
        Query {
            world,
            state,
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
        &'world self,
        e: Entity,
    ) -> Result<<<Q as FetchComponents>::ReadOnly as FetchComponents>::Item<'world>, QueryError>
    {
        self.state
            .as_readonly()
            .get(self.world, e, unsafe { &mut *self.cache_mapping.get() })
    }
    #[inline]
    pub fn get_mut(
        &'world mut self,
        e: Entity,
    ) -> Result<<Q as FetchComponents>::Item<'world>, QueryError> {
        self.state.get(self.world, e, self.cache_mapping.get_mut())
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
    pub fn iter(&self) -> QueryIter<'_, <Q as FetchComponents>::ReadOnly, F, S> {
        QueryIter::new(self.world, self.state.as_readonly())
    }
    pub fn iter_mut(&mut self) -> QueryIter<'_, Q, F, S> {
        QueryIter::new(self.world, &self.state)
    }
}

impl<
        Q: FetchComponents + 'static,
        F: FilterComponents + Send + Sync,
        S: FetchComponents + 'static,
    > SystemParam for Query<'_, Q, F, S>
{
    type State = QueryState<Q, F, S>;
    type Item<'w> = Query<'w, Q, F, S>;

    fn init_state(world: &World, system_meta: &mut SystemMeta) -> Self::State {
        let mut rw = ReadWrite::default();
        Q::init_read_write(world, &mut rw);
        let mut sub_rw = ReadWrite::default();
        S::init_read_write(world, &mut sub_rw);
        // 主查询必须包含子查询
        rw.contains(&sub_rw).unwrap();
        F::init_read_write(world, &mut rw);
        system_meta.add_rw(rw);
        let mut listeners = Default::default();
        F::init_listeners(world, &mut listeners);
        if F::LISTENER_COUNT > 0 {
            // 遍历已有的原型， 添加record
            let notify = Notify(
                listeners.clone(),
                PhantomData,
                PhantomData::<Q>,
                PhantomData::<F>,
                PhantomData::<S>,
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
            result.merge(ArchetypeDepend::Flag(Flags::WITHOUT));
        }
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
    ) -> Self::Item<'world> {
        Query::new(world, state)
    }
}

impl<'world, Q: FetchComponents, F: FilterComponents, S: FetchComponents> Drop
    for Query<'world, Q, F, S>
{
    fn drop(&mut self) {
        self.state.cache_mapping = *self.cache_mapping.get_mut();
    }
}
/// 监听原型创建， 添加record
pub struct Notify<'a, Q: FetchComponents, F: FilterComponents, S: FetchComponents>(
    SmallVec<[(TypeId, bool); 1]>,
    PhantomData<&'a ()>,
    PhantomData<Q>,
    PhantomData<F>,
    PhantomData<S>,
);
impl<
        'a,
        Q: FetchComponents + 'static,
        F: FilterComponents + 'static,
        S: FetchComponents + 'static,
    > Listener for Notify<'a, Q, F, S>
{
    type Event = ArchetypeInit<'a>;
    fn listen(&self, ar: Self::Event) {
        if !QueryState::<Q, F, S>::relate(ar.0) {
            return;
        }
        unsafe {
            ar.0.add_dirty_listeners(TypeId::of::<QueryState<Q, F, S>>(), &self.0)
        };
    }
}

pub type ArchetypeLocalIndex = usize;

#[derive(Debug)]
pub struct QueryState<
    Q: FetchComponents + 'static,
    F: FilterComponents + 'static,
    S: FetchComponents + 'static,
> {
    pub(crate) listeners: SmallVec<[(TypeId, bool); 1]>,
    pub(crate) vec: Vec<(
        ShareArchetype,
        Q::State,
        S::State,
        SmallVec<[DirtyIndex; 1]>,
    )>, // 每原型、查询状态及对应的脏监听
    pub(crate) archetype_len: usize, // 脏的最新的原型，如果world上有更新的，则检查是否和自己相关
    pub(crate) map: HashMap<ArchetypeWorldIndex, ArchetypeLocalIndex>, // 脏world上的原型索引对于本地的原型索引
    pub(crate) cache_mapping: (ArchetypeWorldIndex, ArchetypeLocalIndex), // 缓存上次的索引映射关系
    empty: Q::State,
    _k: PhantomData<F>,
}

impl<Q: FetchComponents, F: FilterComponents, S: FetchComponents> QueryState<Q, F, S> {
    pub fn as_readonly(&self) -> &QueryState<Q::ReadOnly, F, S> {
        unsafe { &*(self as *const QueryState<Q, F, S> as *const QueryState<Q::ReadOnly, F, S>) }
    }
    pub fn new(listeners: SmallVec<[(TypeId, bool); 1]>, empty: Q::State) -> Self {
        Self {
            listeners,
            vec: Vec::new(),
            archetype_len: 0,
            map: Default::default(),
            cache_mapping: (ArchetypeWorldIndex::null(), 0),
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
        result.flag.bits() != 0 && !result.flag.contains(Flags::WITHOUT)
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
        let mut vec = SmallVec::new();
        if F::LISTENER_COUNT > 0 {
            ar.find_dirty_listeners(TypeId::of::<Self>(), &self.listeners, &mut vec);
            if vec.len() == 0 {
                // 表示该原型没有监听的组件，本查询可以不关心该原型
                return;
            }
        }
        self.map
            .insert(index, self.vec.len() as ArchetypeLocalIndex);
        self.vec.push((
            ar.clone(),
            Q::init_state(world, ar),
            S::init_state(world, ar),
            vec,
        ));
    }
    pub fn get<'w>(
        &'w self,
        world: &'w World,
        entity: Entity,
        cache_mapping: &mut (ArchetypeWorldIndex, ArchetypeLocalIndex),
    ) -> Result<Q::Item<'w>, QueryError> {
        let addr = check(world, entity, cache_mapping, &self.map)?;
        let ar = unsafe { &self.vec.get_unchecked(self.cache_mapping.1) };
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

// 检查entity是否正确，包括对应的原型是否在本查询内，并将查询到的原型本地位置记到cache_mapping上
pub(crate) fn check<'w>(
    world: &'w World,
    entity: Entity,
    cache_mapping: &mut (ArchetypeWorldIndex, ArchetypeLocalIndex),
    map: &HashMap<ArchetypeWorldIndex, ArchetypeLocalIndex>,
) -> Result<EntityAddr, QueryError> {
    let addr = match world.entitys.get(entity) {
        Some(v) => *v,
        None => return Err(QueryError::NoSuchEntity),
    };
    if cache_mapping.0 != addr.index {
        cache_mapping.1 = match map.get(&addr.index) {
            Some(v) => *v,
            None => return Err(QueryError::NoSuchArchetype),
        };
    }
    Ok(addr)
}

pub struct QueryIter<
    'w,
    Q: FetchComponents + 'static,
    F: FilterComponents + 'static,
    S: FetchComponents + 'static,
> {
    pub(crate) world: &'w World,
    state: &'w QueryState<Q, F, S>,
    // 原型的位置
    pub(crate) ar_index: ArchetypeLocalIndex,
    // 原型
    pub(crate) ar: &'w Archetype,
    fetch: Q::Fetch<'w>,
    pub(crate) e: Entity,
    pub(crate) row: Row,
    // 脏迭代器，监听多个组件变化，可能entity相同，需要进行去重
    dirty: (Iter<'w, EntityDirty>, usize),
    // 用来脏查询时去重row
    bitset: FixedBitSet,
    // 缓存上次的索引映射关系
    cache_mapping: (ArchetypeWorldIndex, ArchetypeLocalIndex),
}
impl<'w, Q: FetchComponents, F: FilterComponents, S: FetchComponents> QueryIter<'w, Q, F, S> {
    /// # Safety
    /// - `world` must have permission to access any of the components registered in `query_state`.
    /// - `world` must be the same one used to initialize `query_state`.
    #[inline(always)]
    pub(crate) fn new(world: &'w World, state: &'w QueryState<Q, F, S>) -> Self {
        let len = state.vec.len();
        // 该查询没有关联的原型
        if len == 0 {
            return QueryIter {
                world,
                state,
                ar: world.empty_archetype(),
                ar_index: 0,
                fetch: Q::init_fetch(world, world.empty_archetype(), &state.empty),
                e: Entity::null(),
                row: 0,
                dirty: (Iter::empty(), 0),
                bitset: FixedBitSet::new(),
                cache_mapping: state.cache_mapping,
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
                e: Entity::null(),
                row: ar.0.table.len(),
                dirty: (Iter::empty(), 0),
                bitset: FixedBitSet::new(),
                cache_mapping: state.cache_mapping,
            }
        } else {
            // 该查询有组件变化监听器， 倒序迭代所脏的原型
            let len = ar.3.len() - 1;
            let d_index = unsafe { *ar.3.get_unchecked(len) };
            QueryIter {
                world,
                state,
                ar: &ar.0,
                ar_index,
                fetch,
                e: Entity::null(),
                row: u32::null(),
                dirty: (d_index.get_iter(&ar.0), len),
                bitset: FixedBitSet::with_capacity(ar.0.len()),
                cache_mapping: state.cache_mapping,
            }
        }
    }

    #[inline(always)]
    fn iter_normal(&mut self) -> Option<Q::Item<'w>> {
        loop {
            if self.row > 0 {
                self.row -= 1;
                self.e = self.ar.table.get(self.row);
                // 要求条目不为空
                if !self.e.is_null() {
                    let item = Q::fetch(&mut self.fetch, self.row, self.e);
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
            if let Some(d) = self.dirty.0.next() {
                if self.bitset.contains(d.row as usize) {
                    continue;
                }
                self.bitset.set(d.row as usize, true);
                self.row = d.row;
                self.e = self.ar.table.get(self.row);
                // 要求条目不为空
                if !self.e.is_null() {
                    let item = Q::fetch(&mut self.fetch, self.row, self.e);
                    return Some(item);
                } else {
                    // 如果为null，则用d.e去查，e是否存在，所在的原型是否在本查询范围内
                    match self.state.get(self.world, d.e, &mut self.cache_mapping) {
                        Ok(item) => return Some(item),
                        Err(_) => (),
                    }
                }
                continue;
            }
            // 检查当前原型的下一个被脏组件
            if self.dirty.1 > 0 {
                let len = self.dirty.1 - 1;
                let ar = unsafe { &self.state.vec.get_unchecked(self.ar_index) };
                let d_index = unsafe { *ar.3.get_unchecked(len) };
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
            let len = ar.3.len() - 1;
            let d_index = unsafe { *ar.3.get_unchecked(len) };
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
        let count = it.map(|(ar, _, _, _)| ar.table.len()).count();
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
        c += self.size_hint_ar_column_dirty(&ar.0, &ar.3, self.dirty.1);
        for i in 0..ar_index {
            let ar = unsafe { &self.state.vec.get_unchecked(i) };
            // 获得剩余原型的全部列的脏长度
            c += self.size_hint_ar_column_dirty(&ar.0, &ar.3, ar.3.len());
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

impl<'w, Q: FetchComponents, F: FilterComponents, S: FetchComponents> Iterator
    for QueryIter<'w, Q, F, S>
{
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
