use std::borrow::Cow;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use pi_proc_macros::all_tuples;
use pi_share::Share;

use crate::archetype::{ArchetypeIndex, ComponentInfo, Row, COMPONENT_TICK};
use crate::column::{BlobRef, Column};
use crate::prelude::FromWorld;
use crate::single_res::TickRes;
use crate::system::{Relation, SystemMeta};
use crate::world::{ComponentIndex, Entity, Tick, World};

pub trait FetchComponents {
    /// The item returned by this [`FetchComponents`]
    type Item<'a>;
    /// ReadOnly
    type ReadOnly: FetchComponents;
    /// Per archetype/table state used by this [`FetchComponents`] to fetch [`Self::Item`](crate::query::FetchComponents::Item)
    type Fetch<'a>;

    /// State used to construct a [`Self::Fetch`](crate::query::FetchComponents::Fetch). This will be cached inside [`QueryState`](crate::query::QueryState),
    /// so it is best to move as much data / computation here as possible to reduce the cost of
    /// constructing [`Self::Fetch`](crate::query::FetchComponents::Fetch).
    type State: Send + Sync + Sized;

    /// initializes ReadWrite for this [`FetchComponents`] type.
    fn init_state(_world: &mut World, _meta: &mut SystemMeta) -> Self::State;

    /// Creates a new instance of this fetch.
    ///
    /// # Safety
    ///
    /// - `world` must have permission to access any of the components specified in `Self::update_archetype_component_access`.
    /// - `state` must have been initialized (via [`FetchComponents::init_statee`]) using the same `world` passed
    ///   in to this function.
    fn init_fetch<'w>(
        world: &'w World,
        state: &'w Self::State,
        index: ArchetypeIndex,
        tick: Tick,
        last_run: Tick,
    ) -> Self::Fetch<'w>;

    /// Fetch [`Self::Item`](`FetchComponents::Item`) for either the given `entity` in the current [`Table`],
    /// or for the given `entity` in the current [`Archetype`]. This must always be called after
    /// [`FetchComponents::set_table`] with a `table_row` in the range of the current [`Table`] or after
    /// [`FetchComponents::set_archetype`]  with a `entity` in the current archetype.
    ///
    /// # Safety
    ///
    /// Must always be called _after_ [`FetchComponents::set_table`] or [`FetchComponents::set_archetype`]. `entity` and
    /// `table_row` must be in the range of the current table and archetype.
    ///
    /// If `update_component_access` includes any mutable accesses, then the caller must ensure
    /// that `fetch` is called no more than once for each `entity`/`table_row` in each archetype.
    /// If `Self` implements [`ReadOnlyFetchComponents`], then this can safely be called multiple times.
    fn fetch<'w>(fetch: &Self::Fetch<'w>, row: Row, e: Entity) -> Self::Item<'w>;
}

impl FetchComponents for Entity {
    type Fetch<'w> = ();
    type Item<'w> = Entity;
    type ReadOnly = Self;
    type State = ();

    fn init_state(_world: &mut World, _meta: &mut SystemMeta) -> Self::State {}
    #[inline]
    fn init_fetch<'w>(
        _world: &'w World,
        _state: &'w Self::State,
        _index: ArchetypeIndex,
        _tick: Tick,
        _last_run: Tick,
    ) -> Self::Fetch<'w> {
        ()
    }

    #[inline(always)]
    fn fetch<'w>(_fetch: &Self::Fetch<'w>, _row: Row, e: Entity) -> Self::Item<'w> {
        e
    }
}

impl<T: 'static> FetchComponents for &T {
    type Fetch<'w> = ColumnTick<'w>; // 必须和&mut T的Fetch一致，因为Query做了Fetch的缓冲
    type Item<'w> = &'w T;
    type ReadOnly = Self;
    type State = Share<Column>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        meta.component_relate(
            world,
            ComponentInfo::of::<T>(0),
            Relation::Read(0usize.into()),
        )
        .1
    }
    #[inline]
    fn init_fetch<'w>(
        _world: &'w World,
        state: &'w Self::State,
        index: ArchetypeIndex,
        tick: Tick,
        last_run: Tick,
    ) -> Self::Fetch<'w> {
        ColumnTick::new(state.blob_ref_unchecked(index), tick, last_run)
    }

    #[inline(always)]
    fn fetch<'w>(fetch: &Self::Fetch<'w>, row: Row, e: Entity) -> Self::Item<'w> {
        fetch.column.get::<T>(row, e)
    }
}

impl<T: 'static> FetchComponents for &mut T {
    type Fetch<'w> = ColumnTick<'w>;
    type Item<'w> = Mut<'w, T>;
    type ReadOnly = &'static T;
    type State = Share<Column>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        meta.component_relate(
            world,
            ComponentInfo::of::<T>(0),
            Relation::Write(0usize.into()),
        )
        .1
    }
    #[inline]
    fn init_fetch<'w>(
        _world: &'w World,
        state: &'w Self::State,
        index: ArchetypeIndex,
        tick: Tick,
        last_run: Tick,
    ) -> Self::Fetch<'w> {
        ColumnTick::new(state.blob_ref_unchecked(index), tick, last_run)
    }

    #[inline(always)]
    fn fetch<'w>(fetch: &Self::Fetch<'w>, row: Row, e: Entity) -> Self::Item<'w> {
        Mut::new(fetch, e, row)
    }
}

pub struct Ref<T: 'static>(PhantomData<T>);
impl<T: 'static> FetchComponents for Ref<T> {
    type Fetch<'w> = ColumnTick<'w>;
    type Item<'w> = TickRef<'w, T>;
    type ReadOnly = Self;
    type State = Share<Column>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        meta.component_relate(
            world,
            ComponentInfo::of::<T>(COMPONENT_TICK),
            Relation::Read(0usize.into()),
        )
        .1
    }
    #[inline]
    fn init_fetch<'w>(
        _world: &'w World,
        state: &'w Self::State,
        index: ArchetypeIndex,
        tick: Tick,
        last_run: Tick,
    ) -> Self::Fetch<'w> {
        ColumnTick::new(state.blob_ref_unchecked(index), tick, last_run)
    }

    #[inline(always)]
    fn fetch<'w>(fetch: &Self::Fetch<'w>, row: Row, e: Entity) -> Self::Item<'w> {
        TickRef::new(fetch, row, e)
    }
}

impl<T: 'static> FetchComponents for Option<Ref<T>> {
    type Fetch<'w> = Option<ColumnTick<'w>>;
    type Item<'w> = Option<TickRef<'w, T>>;
    type ReadOnly = Self;
    type State = Share<Column>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        meta.component_relate(
            world,
            ComponentInfo::of::<T>(COMPONENT_TICK),
            Relation::OptRead(0usize.into()),
        )
        .1
    }
    #[inline]
    fn init_fetch<'w>(
        _world: &'w World,
        state: &'w Self::State,
        index: ArchetypeIndex,
        tick: Tick,
        last_run: Tick,
    ) -> Self::Fetch<'w> {
        if let Some(column) = state.blob_ref(index) {
            Some(ColumnTick::new(column, tick, last_run))
        } else {
            None
        }
    }
    #[inline(always)]
    fn fetch<'w>(fetch: &Self::Fetch<'w>, row: Row, e: Entity) -> Self::Item<'w> {
        match fetch {
            Some(c) => Some(TickRef::new(c, row, e)),
            None => None,
        }
    }
}

impl<T: 'static> FetchComponents for Ticker<'_, &'_ T> {
    type Fetch<'w> = ColumnTick<'w>;
    type Item<'w> = Ticker<'w, &'w T>;
    type ReadOnly = Ticker<'static, &'static T>;
    type State = Share<Column>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        meta.component_relate(
            world,
            ComponentInfo::of::<T>(COMPONENT_TICK),
            Relation::Read(0usize.into()),
        )
        .1
    }
    #[inline]
    fn init_fetch<'w>(
        _world: &'w World,
        state: &'w Self::State,
        index: ArchetypeIndex,
        tick: Tick,
        last_run: Tick,
    ) -> Self::Fetch<'w> {
        ColumnTick::new(state.blob_ref_unchecked(index), tick, last_run)
    }
    #[inline(always)]
    fn fetch<'w>(fetch: &Self::Fetch<'w>, row: Row, e: Entity) -> Self::Item<'w> {
        Ticker::new(fetch, e, row)
    }
}

impl<T: 'static> FetchComponents for Ticker<'_, &'_ mut T> {
    type Fetch<'w> = ColumnTick<'w>;
    type Item<'w> = Ticker<'w, &'w mut T>;
    type ReadOnly = Ticker<'static, &'static T>;
    type State = Share<Column>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        meta.component_relate(
            world,
            ComponentInfo::of::<T>(COMPONENT_TICK),
            Relation::Write(0usize.into()),
        )
        .1
    }
    #[inline]
    fn init_fetch<'w>(
        _world: &'w World,
        state: &'w Self::State,
        index: ArchetypeIndex,
        tick: Tick,
        last_run: Tick,
    ) -> Self::Fetch<'w> {
        ColumnTick::new(state.blob_ref_unchecked(index), tick, last_run)
    }
    #[inline(always)]
    fn fetch<'w>(fetch: &Self::Fetch<'w>, row: Row, e: Entity) -> Self::Item<'w> {
        Ticker::new(fetch, e, row)
    }
}

impl<T: 'static> FetchComponents for Option<Ticker<'_, &'_ T>> {
    type Fetch<'w> = Option<ColumnTick<'w>>;
    type Item<'w> = Option<Ticker<'w, &'w T>>;
    type ReadOnly = Option<Ticker<'static, &'static T>>;
    type State = Share<Column>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        meta.component_relate(
            world,
            ComponentInfo::of::<T>(COMPONENT_TICK),
            Relation::OptRead(0usize.into()),
        )
        .1
    }
    #[inline]
    fn init_fetch<'w>(
        _world: &'w World,
        state: &'w Self::State,
        index: ArchetypeIndex,
        tick: Tick,
        last_run: Tick,
    ) -> Self::Fetch<'w> {
        if let Some(column) = state.blob_ref(index) {
            Some(ColumnTick::new(column, tick, last_run))
        } else {
            None
        }
    }
    #[inline(always)]
    fn fetch<'w>(fetch: &Self::Fetch<'w>, row: Row, e: Entity) -> Self::Item<'w> {
        match fetch {
            Some(f) => Some(Ticker::new(f, e, row)),
            None => None,
        }
    }
}

impl<T: 'static> FetchComponents for Option<Ticker<'_, &'_ mut T>> {
    type Fetch<'w> = Option<ColumnTick<'w>>;
    type Item<'w> = Option<Ticker<'w, &'w mut T>>;
    type ReadOnly = Option<Ticker<'static, &'static T>>;
    type State = Share<Column>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        meta.component_relate(
            world,
            ComponentInfo::of::<T>(COMPONENT_TICK),
            Relation::OptWrite(0usize.into()),
        )
        .1
    }
    #[inline]
    fn init_fetch<'w>(
        _world: &'w World,
        state: &'w Self::State,
        index: ArchetypeIndex,
        tick: Tick,
        last_run: Tick,
    ) -> Self::Fetch<'w> {
        if let Some(column) = state.blob_ref(index) {
            Some(ColumnTick::new(column, tick, last_run))
        } else {
            None
        }
    }
    #[inline(always)]
    fn fetch<'w>(fetch: &Self::Fetch<'w>, row: Row, e: Entity) -> Self::Item<'w> {
        match fetch {
            Some(f) => Some(Ticker::new(f, e, row)),
            None => None,
        }
    }
}

impl<T: 'static> FetchComponents for Option<&T> {
    type Fetch<'w> = Option<ColumnTick<'w>>;
    type Item<'w> = Option<&'w T>;
    type ReadOnly = Self;
    type State = Share<Column>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        meta.component_relate(
            world,
            ComponentInfo::of::<T>(0),
            Relation::OptRead(0usize.into()),
        )
        .1
    }
    #[inline]
    fn init_fetch<'w>(
        _world: &'w World,
        state: &'w Self::State,
        index: ArchetypeIndex,
        tick: Tick,
        last_run: Tick,
    ) -> Self::Fetch<'w> {
        if let Some(column) = state.blob_ref(index) {
            Some(ColumnTick::new(column, tick, last_run))
        } else {
            None
        }
    }
    #[inline(always)]
    fn fetch<'w>(fetch: &Self::Fetch<'w>, row: Row, e: Entity) -> Self::Item<'w> {
        match fetch {
            Some(c) => Some(c.column.get::<T>(row, e)),
            None => None,
        }
    }
}

impl<T: 'static> FetchComponents for Option<&mut T> {
    type Fetch<'w> = Option<ColumnTick<'w>>;
    type Item<'w> = Option<Mut<'w, T>>;
    type ReadOnly = Option<&'static T>;
    type State = Share<Column>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        meta.component_relate(
            world,
            ComponentInfo::of::<T>(0),
            Relation::OptWrite(0usize.into()),
        )
        .1
    }
    #[inline]
    fn init_fetch<'w>(
        _world: &'w World,
        state: &'w Self::State,
        index: ArchetypeIndex,
        tick: Tick,
        last_run: Tick,
    ) -> Self::Fetch<'w> {
        if let Some(column) = state.blob_ref(index) {
            Some(ColumnTick::new(column, tick, last_run))
        } else {
            None
        }
    }
    #[inline(always)]
    fn fetch<'w>(fetch: &Self::Fetch<'w>, row: Row, e: Entity) -> Self::Item<'w> {
        match fetch {
            Some(f) => Some(Mut::new(f, e, row)),
            None => None,
        }
    }
}
/// 不存在T时，使用默认值。
/// 默认值取Res<DefaultValue<T>>
/// DefaultValue<T>默认为DefaultValue::from_world的返回值，也可被应用程序覆盖
pub struct OrDefault<T: 'static + FromWorld>(PhantomData<T>);
impl<T: 'static + FromWorld> FetchComponents for OrDefault<T> {
    type Fetch<'w> = Result<BlobRef<'w>, &'w T>;
    type Item<'w> = &'w T;
    type ReadOnly = Self;
    type State = (Share<Column>, Share<TickRes<T>>);

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        let info = ComponentInfo::of::<T>(0);
        meta.add_res(Relation::Read(*info.type_id()));
        let column = meta
            .component_relate(world, info, Relation::OptRead(0usize.into()))
            .1;
        let _index = world.init_single_res::<T>();
        (column, world.get_share_single_res::<T>().unwrap())
    }
    #[inline]
    fn init_fetch<'w>(
        _world: &'w World,
        state: &'w Self::State,
        index: ArchetypeIndex,
        _tick: Tick,
        _last_run: Tick,
    ) -> Self::Fetch<'w> {
        if let Some(column) = state.0.blob_ref(index) {
            Ok(column)
        } else {
            Err(&state.1)
        }
    }
    #[inline(always)]
    fn fetch<'w>(fetch: &Self::Fetch<'w>, row: Row, e: Entity) -> Self::Item<'w> {
        match fetch {
            Ok(c) => c.get::<T>(row, e),
            Err(r) => *r,
        }
    }
}
/// 不存在T时，使用默认值。
/// 默认值取Res<DefaultValue<T>>
/// DefaultValue<T>默认为DefaultValue::from_world的返回值，也可被应用程序覆盖
pub struct OrDefaultRef<T: 'static + FromWorld>(PhantomData<T>);
impl<T: 'static + FromWorld> FetchComponents for OrDefaultRef<T> {
    type Fetch<'w> = Result<ColumnTick<'w>, (&'w T, Tick)>;
    type Item<'w> = ValueRef<'w, T>;
    type ReadOnly = Self;
    type State = (Share<Column>, Share<TickRes<T>>);

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        let info = ComponentInfo::of::<T>(0);
        meta.add_res(Relation::Read(*info.type_id()));
        let column = meta
            .component_relate(world, info, Relation::OptRead(0usize.into()))
            .1;
        let _index = world.init_single_res::<T>();
        (column, world.get_share_single_res::<T>().unwrap())
    }
    #[inline]
    fn init_fetch<'w>(
        _world: &'w World,
        state: &'w Self::State,
        index: ArchetypeIndex,
        tick: Tick,
        last_run: Tick,
    ) -> Self::Fetch<'w> {
        if let Some(column) = state.0.blob_ref(index) {
            Ok(ColumnTick::new(column, tick, last_run))
        } else {
            Err((&state.1, last_run))
        }
    }
    #[inline(always)]
    fn fetch<'w>(fetch: &Self::Fetch<'w>, row: Row, e: Entity) -> Self::Item<'w> {
        match fetch {
            Ok(c) => {
                let tick = c.column.get_tick_unchecked(row);
                ValueRef::new(c.column.get::<T>(row, e), tick, c.last_run)
            }
            Err(r) => ValueRef::new(r.0, 0usize.into(), r.1),
        }
    }
}

pub struct Has<T: 'static>(PhantomData<T>);
impl<T: 'static> FetchComponents for Has<T> {
    type Fetch<'w> = bool;
    type Item<'w> = bool;
    type ReadOnly = Self;
    type State = Share<Column>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        meta.component_relate(
            world,
            ComponentInfo::of::<T>(0),
            Relation::OptRead(0usize.into()),
        )
        .1
    }
    #[inline]
    fn init_fetch<'w>(
        _world: &'w World,
        state: &'w Self::State,
        index: ArchetypeIndex,
        _tick: Tick,
        _last_run: Tick,
    ) -> Self::Fetch<'w> {
        state.contains(index)
    }
    #[inline(always)]
    fn fetch<'w>(fetch: &Self::Fetch<'w>, _row: Row, _e: Entity) -> Self::Item<'w> {
        *fetch
    }
}

#[derive(Debug)]
pub struct ComponentId<T: 'static>(pub ComponentIndex, PhantomData<T>);
impl<T: 'static> FetchComponents for ComponentId<T> {
    type Fetch<'w> = ComponentIndex;
    type Item<'w> = ComponentIndex;
    type ReadOnly = Self;
    type State = ComponentIndex;

    fn init_state(world: &mut World, _meta: &mut SystemMeta) -> Self::State {
        world.init_component::<T>()
    }

    fn init_fetch<'w>(
        _world: &'w World,
        state: &'w Self::State,
        _index: ArchetypeIndex,
        _tick: Tick,
        _last_run: Tick,
    ) -> Self::Fetch<'w> {
        *state
    }

    fn fetch<'w>(fetch: &Self::Fetch<'w>, _row: Row, _e: Entity) -> Self::Item<'w> {
        *fetch
    }
}

#[derive(Debug)]
pub struct ArchetypeName<'a>(pub &'a Cow<'static, str>, pub ArchetypeIndex, pub Row);
impl FetchComponents for ArchetypeName<'_> {
    type Fetch<'w> = (&'w Cow<'static, str>, ArchetypeIndex);
    type Item<'w> = ArchetypeName<'w>;
    type ReadOnly = Self;
    type State = ();

    fn init_state(_world: &mut World, _meta: &mut SystemMeta) -> Self::State {}

    fn init_fetch<'w>(
        world: &'w World,
        _state: &'w Self::State,
        index: ArchetypeIndex,
        _tick: Tick,
        _last_run: Tick,
    ) -> Self::Fetch<'w> {
        let archetype = world.get_archetype(index).unwrap();
        (archetype.name(), archetype.index())
    }

    fn fetch<'w>(fetch: &Self::Fetch<'w>, row: Row, _e: Entity) -> Self::Item<'w> {
        ArchetypeName(fetch.0, fetch.1, row)
    }
}

#[derive(Debug, Clone)]
pub struct ColumnTick<'a> {
    pub(crate) column: BlobRef<'a>,
    pub(crate) tick: Tick,
    pub(crate) last_run: Tick,
}
impl<'a> ColumnTick<'a> {
    #[inline(always)]
    pub(crate) fn new(column: BlobRef<'a>, tick: Tick, last_run: Tick) -> Self {
        Self {
            column,
            tick,
            last_run,
        }
    }
}

#[derive(Debug)]
pub struct TickRef<'a, T> {
    pub(crate) c: ColumnTick<'a>,
    pub(crate) row: Row,
    pub(crate) e: Entity,
    _p: PhantomData<T>,
}

impl<'a, T> TickRef<'a, T> {
    #[inline(always)]
    pub fn new(c: &ColumnTick<'a>, row: Row, e: Entity) -> Self {
        Self {
            c: c.clone(),
            row,
            e,
            _p: PhantomData,
        }
    }
    #[inline(always)]
    pub fn entity(&self) -> Entity {
        self.e
    }
    pub fn tick(&self) -> Tick {
        self.c.column.get_tick_unchecked(self.row)
    }
    #[inline(always)]
    pub fn last_tick(&self) -> Tick {
        self.c.last_run
    }
    #[inline(always)]
    pub fn is_changed(&self) -> bool {
        self.c.column.get_tick_unchecked(self.row) > self.c.last_run
    }
}
impl<'a, T: 'static> Deref for TickRef<'a, T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.c.column.get::<T>(self.row, self.e)
    }
}

#[derive(Debug)]
pub struct ValueRef<'a, T> {
    pub(crate) value: &'a T,
    pub(crate) tick: Tick,
    pub(crate) last_run: Tick,
}

impl<'a, T> ValueRef<'a, T> {
    #[inline(always)]
    pub fn new(value: &'a T, tick: Tick, last_run: Tick) -> Self {
        Self {
            value,
            tick,
            last_run,
        }
    }

    pub fn tick(&self) -> Tick {
        self.tick
    }
    #[inline(always)]
    pub fn last_tick(&self) -> Tick {
        self.last_run
    }
    #[inline(always)]
    pub fn is_changed(&self) -> bool {
        self.tick > self.last_run
    }
}
impl<'a, T: 'static> Deref for ValueRef<'a, T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

#[derive(Debug)]
pub struct Ticker<'a, T> {
    pub(crate) c: ColumnTick<'a>,
    pub(crate) e: Entity,
    pub(crate) row: Row,
    _p: PhantomData<T>,
}

impl<'a, T> Ticker<'a, T> {
    pub fn new(c: &ColumnTick<'a>, e: Entity, row: Row) -> Self {
        Self {
            c: c.clone(),
            e,
            row,
            _p: PhantomData,
        }
    }

    pub fn entity(&self) -> Entity {
        self.e
    }

    pub fn tick(&self) -> Tick {
        self.c.column.get_tick_unchecked(self.row)
    }

    pub fn last_tick(&self) -> Tick {
        self.c.last_run
    }

    pub fn is_changed(&self) -> bool {
        self.c.column.get_tick_unchecked(self.row) > self.c.last_run
    }
}
impl<'a, T: 'static> Deref for Ticker<'a, &'_ T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.c.column.get::<T>(self.row, self.e)
    }
}

impl<'a, T: 'static> Deref for Ticker<'a, &'_ mut T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.c.column.get::<T>(self.row, self.e)
    }
}

impl<'a, T: 'static> Ticker<'a, &'_ mut T> {
    pub fn bypass_change_detection(&mut self) -> &mut T {
        self.c.column.get_mut::<T>(self.row, self.e)
    }

    pub fn set_changed(&mut self) {
        self.c.column.changed_tick(self.e, self.row, self.c.tick);
    }
}

impl<'a, T: 'static> DerefMut for Ticker<'a, &'_ mut T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.c.column.changed_tick(self.e, self.row, self.c.tick);
        self.c.column.get_mut::<T>(self.row, self.e)
    }
}

/// Unique mutable borrow of an entity's component
#[derive(Debug)]
pub struct Mut<'a, T: 'static> {
    pub(crate) c: ColumnTick<'a>,
    pub(crate) e: Entity,
    pub(crate) row: Row,
    _p: PhantomData<T>,
}
impl<'a, T: Sized> Mut<'a, T> {
    #[inline(always)]
    pub fn new(c: &ColumnTick<'a>, e: Entity, row: Row) -> Self {
        Self {
            c: c.clone(),
            e,
            row,
            _p: PhantomData,
        }
    }
    #[inline(always)]
    pub fn entity(&self) -> Entity {
        self.e
    }
    pub fn tick(&self) -> Tick {
        self.c.column.get_tick_unchecked(self.row)
    }
    #[inline(always)]
    pub fn last_tick(&self) -> Tick {
        self.c.last_run
    }
    #[inline(always)]
    pub fn is_changed(&self) -> bool {
        self.c.column.get_tick_unchecked(self.row) > self.c.last_run
    }
    pub fn bypass_change_detection(&mut self) -> &mut T {
        self.c.column.get_mut::<T>(self.row, self.e)
    }

    pub fn set_changed(&mut self) {
        self.c.column.changed_tick(self.e, self.row, self.c.tick);
    }

    pub fn into_inner(self) -> &'a mut T {
        self.c.column.changed_tick(self.e, self.row, self.c.tick);
        self.c.column.get_mut::<T>(self.row, self.e)
    }
}
impl<'a, T: 'static> Deref for Mut<'a, T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.c.column.get::<T>(self.row, self.e)
    }
}
impl<'a, T: 'static> DerefMut for Mut<'a, T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.c.column.changed_tick(self.e, self.row, self.c.tick);
        self.c.column.get_mut::<T>(self.row, self.e)
    }
}

macro_rules! impl_tuple_fetch {
    ($(($name: ident, $state: ident)),*) => {
        #[allow(non_snake_case)]
        #[allow(clippy::unused_unit)]

        impl<$($name: FetchComponents),*> FetchComponents for ($($name,)*) {
            type Fetch<'w> = ($($name::Fetch<'w>,)*);
            type Item<'w> = ($($name::Item<'w>,)*);
            type ReadOnly = ($($name::ReadOnly,)*);
            type State = ($($name::State,)*);

            fn init_state(_world: &mut World, _meta: &mut SystemMeta) -> Self::State {
                ($($name::init_state(_world, _meta),)*)
            }
            #[allow(clippy::unused_unit)]
            #[inline(always)]
            fn init_fetch<'w>(
                _world: &'w World,
                _state: &'w Self::State,
                _index: ArchetypeIndex,
                _tick: Tick,
                _last_run: Tick,
                ) -> Self::Fetch<'w> {
                let ($($state,)*) = _state;
                ($($name::init_fetch(_world, $state, _index, _tick, _last_run),)*)
            }

            #[allow(clippy::unused_unit)]
            #[inline(always)]
            fn fetch<'w>(
                _fetch: &Self::Fetch<'w>,
                _row: Row,
                _e: Entity,
            ) -> Self::Item<'w> {
                let ($($name,)*) = _fetch;
                ($($name::fetch($name, _row, _e),)*)
            }
        }

    };
}
all_tuples!(impl_tuple_fetch, 0, 15, F, S);
