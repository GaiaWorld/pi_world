use pi_null::Null;
use pi_proc_macros::all_tuples;
use std::any::TypeId;
use std::borrow::Cow;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use crate::archetype::{
    Archetype, ArchetypeDependResult, ColumnIndex, ComponentInfo, Flags, Row
};
use crate::column::Column;
use crate::prelude::FromWorld;
use crate::system::{SystemMeta, TypeInfo};
use crate::world::{ComponentIndex, Entity, SingleResource, Tick, World};

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

    const TICK_COUNT: usize;

    /// initializes tick for this [`FetchComponents`] type
    fn init_ticks(_world: &World, _ticks: &mut Vec<ComponentIndex>) {}

    /// initializes ReadWrite for this [`FetchComponents`] type.
    fn init_read_write(_world: &mut World, _meta: &mut SystemMeta) {}
    fn archetype_depend(_world: &World, _archetype: &Archetype, _result: &mut ArchetypeDependResult) {}
    fn res_depend(
        _res_tid: &TypeId,
        _res_name: &Cow<'static, str>,
        _single: bool,
        _result: &mut Flags,
    ) {
    }

    /// Creates and initializes a [`State`](FetchComponents::State) for this [`FetchComponents`] type.
    fn init_state(world: &World, archetype: &Archetype) -> Self::State;

    /// Creates a new instance of this fetch.
    ///
    /// # Safety
    ///
    /// - `world` must have permission to access any of the components specified in `Self::update_archetype_component_access`.
    /// - `state` must have been initialized (via [`FetchComponents::init_state`]) using the same `world` passed
    ///   in to this function.
    fn init_fetch<'w>(
        world: &'w World,
        archetype: &'w Archetype,
        state: &'w Self::State,
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
    fn fetch<'w>(fetch: &mut Self::Fetch<'w>, row: Row, e: Entity) -> Self::Item<'w>;
}

impl FetchComponents for Entity {
    type Fetch<'w> = &'w Archetype;
    type Item<'w> = Entity;
    type ReadOnly = Entity;
    type State = ();
    const TICK_COUNT: usize = 0;

    fn init_state(_world: &World, _archetype: &Archetype) -> Self::State {}

    #[inline]
    fn init_fetch<'w>(
        _world: &'w World,
        archetype: &'w Archetype,
        _state: &'w Self::State,
        _tick: Tick,
        _last_run: Tick,
    ) -> Self::Fetch<'w> {
        archetype
    }

    
    fn fetch<'w>(_fetch: &mut Self::Fetch<'w>, _row: Row, e: Entity) -> Self::Item<'w> {
        e
    }
}

impl<T: 'static> FetchComponents for &T {
    type Fetch<'w> = &'w Column;
    type Item<'w> = &'w T;
    type ReadOnly = &'static T;
    type State = ColumnIndex;
    const TICK_COUNT: usize = 0;

    fn init_read_write(world: &mut World, meta: &mut SystemMeta) {
        world.add_component_info(ComponentInfo::of::<T>());
        meta.cur_param
            .reads
            .insert(TypeId::of::<T>(), std::any::type_name::<T>().into());
    }
    fn archetype_depend(world: &World, archetype: &Archetype, result: &mut ArchetypeDependResult) {
        result.depend(archetype, world, &TypeId::of::<T>(), Flags::WITHOUT, Flags::READ);
    }
    fn init_state(world: &World, archetype: &Archetype) -> Self::State {
        archetype.get_column_index_by_tid(&world, &TypeId::of::<T>())
    }

    
    fn init_fetch<'w>(
        _world: &'w World,
        archetype: &'w Archetype,
        state: &'w Self::State,
        _tick: Tick,
        _last_run: Tick,
    ) -> Self::Fetch<'w> {
        &archetype.get_column_unchecked(*state)
    }

    
    fn fetch<'w>(fetch: &mut Self::Fetch<'w>, row: Row, _e: Entity) -> Self::Item<'w> {
        fetch.get(row)
    }
}

impl<T: 'static> FetchComponents for &mut T {
    type Fetch<'w> = ColumnTick<'w>;
    type Item<'w> = Mut<'w, T>;
    type ReadOnly = &'static T;
    type State = ColumnIndex;
    const TICK_COUNT: usize = 0;

    fn init_read_write(world: &mut World, meta: &mut SystemMeta) {
        world.add_component_info(ComponentInfo::of::<T>());
        meta.cur_param
            .writes
            .insert(TypeId::of::<T>(), std::any::type_name::<T>().into());
    }
    fn archetype_depend(world: &World, archetype: &Archetype, result: &mut ArchetypeDependResult) {
        result.depend(archetype, world, &TypeId::of::<T>(), Flags::WITHOUT, Flags::WRITE)
    }
    fn init_state(world: &World, archetype: &Archetype) -> Self::State {
        archetype.get_column_index_by_tid(&world, &TypeId::of::<T>())
    }

    
    fn init_fetch<'w>(
        _world: &'w World,
        archetype: &'w Archetype,
        state: &'w Self::State,
        tick: Tick,
        last_run: Tick,
    ) -> Self::Fetch<'w> {
        ColumnTick::new(&archetype.get_column_unchecked(*state), tick, last_run)
    }

    
    fn fetch<'w>(fetch: &mut Self::Fetch<'w>, row: Row, e: Entity) -> Self::Item<'w> {
        Mut::new(fetch, e, row)
    }
}

impl<T: 'static> FetchComponents for Ticker<'_, &'_ T> {
    type Fetch<'w> = ColumnTick<'w>;
    type Item<'w> = Ticker<'w, &'w T>;
    type ReadOnly = Ticker<'static, &'static T>;
    type State = ColumnIndex;
    const TICK_COUNT: usize = 1;

    /// initializes tick for this [`FetchComponents`] type
    fn init_ticks(world: &World, ticks: &mut Vec<ComponentIndex>) {
        ticks.push(world.get_component_index(&TypeId::of::<T>()));
    }
    fn init_read_write(world: &mut World, meta: &mut SystemMeta) {
        world.add_component_info(ComponentInfo::of::<T>());
        meta.cur_param
            .reads
            .insert(TypeId::of::<T>(), std::any::type_name::<T>().into());
    }
    fn archetype_depend(world: &World, archetype: &Archetype, result: &mut ArchetypeDependResult) {
        result.depend(archetype, world, &TypeId::of::<T>(), Flags::WITHOUT, Flags::READ)
    }
    fn init_state(world: &World, archetype: &Archetype) -> Self::State {
        archetype.get_column_index_by_tid(&world, &TypeId::of::<T>())
    }

    
    fn init_fetch<'w>(
        _world: &'w World,
        archetype: &'w Archetype,
        state: &'w Self::State,
        tick: Tick,
        last_run: Tick,
    ) -> Self::Fetch<'w> {
        ColumnTick::new(&archetype.get_column_unchecked(*state), tick, last_run)
    }

    
    fn fetch<'w>(fetch: &mut Self::Fetch<'w>, row: Row, e: Entity) -> Self::Item<'w> {
        Ticker::new(fetch, e, row)
    }
}

impl<T: 'static> FetchComponents for Ticker<'_, &'_ mut T> {
    type Fetch<'w> = ColumnTick<'w>;
    type Item<'w> = Ticker<'w, &'w mut T>;
    type ReadOnly = Ticker<'static, &'static T>;
    type State = ColumnIndex;
    const TICK_COUNT: usize = 1;

    /// initializes tick for this [`FetchComponents`] type
    fn init_ticks(world: &World, ticks: &mut Vec<ComponentIndex>) {
        ticks.push(world.get_component_index(&TypeId::of::<T>()));
    }
    fn init_read_write(world: &mut World, meta: &mut SystemMeta) {
        world.add_component_info(ComponentInfo::of::<T>());
        meta.cur_param
            .writes
            .insert(TypeId::of::<T>(), std::any::type_name::<T>().into());
    }
    fn archetype_depend(world: &World, archetype: &Archetype, result: &mut ArchetypeDependResult) {
        result.depend(archetype, world, &TypeId::of::<T>(), Flags::WITHOUT, Flags::WRITE)
    }
    fn init_state(world: &World, archetype: &Archetype) -> Self::State {
        archetype.get_column_index_by_tid(&world, &TypeId::of::<T>())
    }
    fn init_fetch<'w>(
        _world: &'w World,
        archetype: &'w Archetype,
        state: &'w Self::State,
        tick: Tick,
        last_run: Tick,
    ) -> Self::Fetch<'w> {
        ColumnTick::new(&archetype.get_column_unchecked(*state), tick, last_run)
    }

    
    fn fetch<'w>(fetch: &mut Self::Fetch<'w>, row: Row, e: Entity) -> Self::Item<'w> {
        Ticker::new(fetch, e, row)
    }
}

impl<T: 'static> FetchComponents for Option<Ticker<'_, &'_ T>> {
    type Fetch<'w> = Option<ColumnTick<'w>>;
    type Item<'w> = Option<Ticker<'w, &'w T>>;
    type ReadOnly = Option<Ticker<'static, &'static T>>;
    type State = ColumnIndex;
    const TICK_COUNT: usize = 1;

    /// initializes tick for this [`FetchComponents`] type
    fn init_ticks(world: &World, ticks: &mut Vec<ComponentIndex>) {
        ticks.push(world.get_component_index(&TypeId::of::<T>()));
    }
    fn init_read_write(world: &mut World, meta: &mut SystemMeta) {
        world.add_component_info(ComponentInfo::of::<T>());
        meta.cur_param
            .reads
            .insert(TypeId::of::<T>(), std::any::type_name::<T>().into());
    }
    fn archetype_depend(world: &World, archetype: &Archetype, result: &mut ArchetypeDependResult) {
        result.depend(archetype, world, &TypeId::of::<T>(), Flags::empty(), Flags::READ)
    }
    fn init_state(world: &World, archetype: &Archetype) -> Self::State {
        archetype.get_column_index_by_tid(&world, &TypeId::of::<T>())
    }

    
    fn init_fetch<'w>(
        _world: &'w World,
        archetype: &'w Archetype,
        state: &'w Self::State,
        tick: Tick,
        last_run: Tick,
    ) -> Self::Fetch<'w> {
        if state.is_null() {
            return None
        }
        Some(ColumnTick::new(
            &archetype.get_column_unchecked(*state),
            tick,
            last_run,
        ))
    }

    
    fn fetch<'w>(fetch: &mut Self::Fetch<'w>, row: Row, e: Entity) -> Self::Item<'w> {
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
    type State = ColumnIndex;
    const TICK_COUNT: usize = 1;

    /// initializes tick for this [`FetchComponents`] type
    fn init_ticks(world: &World, ticks: &mut Vec<ComponentIndex>) {
        ticks.push(world.get_component_index(&TypeId::of::<T>()));
    }
    fn init_read_write(world: &mut World, meta: &mut SystemMeta) {
        world.add_component_info(ComponentInfo::of::<T>());
        meta.cur_param
            .writes
            .insert(TypeId::of::<T>(), std::any::type_name::<T>().into());
    }
    fn archetype_depend(world: &World, archetype: &Archetype, result: &mut ArchetypeDependResult) {
        result.depend(archetype, world, &TypeId::of::<T>(), Flags::empty(), Flags::WRITE)
    }
    fn init_state(world: &World, archetype: &Archetype) -> Self::State {
        archetype.get_column_index_by_tid(&world, &TypeId::of::<T>())
    }

    
    fn init_fetch<'w>(
        _world: &'w World,
        archetype: &'w Archetype,
        state: &'w Self::State,
        tick: Tick,
        last_run: Tick,
    ) -> Self::Fetch<'w> {
        if !state.is_null() {
            Some(ColumnTick::new(
                &archetype.get_column_unchecked(*state),
                tick,
                last_run,
            ))
        } else {
            None
        }
    }


    fn fetch<'w>(fetch: &mut Self::Fetch<'w>, row: Row, e: Entity) -> Self::Item<'w> {
        match fetch {
            Some(f) => Some(Ticker::new(f, e, row)),
            None => None,
        }
    }
}

impl<T: 'static> FetchComponents for Option<&T> {
    type Fetch<'w> = Option<&'w Column>;
    type Item<'w> = Option<&'w T>;
    type ReadOnly = Option<&'static T>;
    type State = ColumnIndex;
    const TICK_COUNT: usize = 0;

    fn init_read_write(world: &mut World, meta: &mut SystemMeta) {
        world.add_component_info(ComponentInfo::of::<T>());
        meta.cur_param
            .reads
            .insert(TypeId::of::<T>(), std::any::type_name::<T>().into());
    }
    fn archetype_depend(world: &World, archetype: &Archetype, result: &mut ArchetypeDependResult) {
        result.depend(archetype, world, &TypeId::of::<T>(), Flags::empty(), Flags::READ)
    }
    fn init_state(world: &World, archetype: &Archetype) -> Self::State {
        archetype.get_column_index_by_tid(&world, &TypeId::of::<T>())
    }

    
    fn init_fetch<'w>(
        _world: &'w World,
        archetype: &'w Archetype,
        state: &'w Self::State,
        _tick: Tick,
        _last_run: Tick,
    ) -> Self::Fetch<'w> {
        if !state.is_null() {
            Some(&archetype.get_column_unchecked(*state))
        } else {
            None
        }
    }


    fn fetch<'w>(fetch: &mut Self::Fetch<'w>, row: Row, _e: Entity) -> Self::Item<'w> {
        fetch.and_then(|c| Some(c.get(row)))
    }
}

impl<T: 'static> FetchComponents for Option<&mut T> {
    type Fetch<'w> = Option<ColumnTick<'w>>;
    type Item<'w> = Option<Mut<'w, T>>;
    type ReadOnly = Option<&'static T>;
    type State = ColumnIndex;
    const TICK_COUNT: usize = 0;

    fn init_read_write(world: &mut World, meta: &mut SystemMeta) {
        world.add_component_info(ComponentInfo::of::<T>());
        meta.cur_param
            .writes
            .insert(TypeId::of::<T>(), std::any::type_name::<T>().into());
    }
    fn archetype_depend(world: &World, archetype: &Archetype, result: &mut ArchetypeDependResult) {
        result.depend(archetype, world, &TypeId::of::<T>(), Flags::empty(), Flags::WRITE)
    }
    fn init_state(world: &World, archetype: &Archetype) -> Self::State {
        archetype.get_column_index_by_tid(&world, &TypeId::of::<T>())
    }

    
    fn init_fetch<'w>(
        _world: &'w World,
        archetype: &'w Archetype,
        state: &'w Self::State,
        tick: Tick,
        last_run: Tick,
    ) -> Self::Fetch<'w> {
        if !state.is_null() {
            Some(ColumnTick::new(
                &archetype.get_column_unchecked(*state),
                tick,
                last_run,
            ))
        } else {
            None
        }
    }


    fn fetch<'w>(fetch: &mut Self::Fetch<'w>, row: Row, e: Entity) -> Self::Item<'w> {
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
    type Fetch<'w> = Result<&'w Column, &'w T>;
    type Item<'w> = &'w T;
    type ReadOnly = OrDefault<T>;
    type State = Result<ColumnIndex, SingleResource>;
    const TICK_COUNT: usize = 0;

    fn init_read_write(world: &mut World, meta: &mut SystemMeta) {
        let info = TypeInfo::of::<T>();
        world.add_component_info(ComponentInfo::of::<T>());
        meta.res_read(&info);
        meta.cur_param.reads.insert(info.type_id, info.name);

        world.init_single_res::<T>();
    }
    fn archetype_depend(world: &World, archetype: &Archetype, result: &mut ArchetypeDependResult) {
        result.depend(archetype, world, &TypeId::of::<T>(), Flags::empty(), Flags::READ)
    }
    fn res_depend(
        res_tid: &TypeId,
        _res_name: &Cow<'static, str>,
        single: bool,
        result: &mut Flags,
    ) {
        if single && &TypeId::of::<T>() == res_tid {
            result.set(Flags::WRITE, true)
        }
    }

    fn init_state(world: &World, archetype: &Archetype) -> Self::State {
        let index = archetype.get_column_index_by_tid(&world, &TypeId::of::<T>());
        if index.is_null() {
            Err(world.get_single_res_any(&TypeId::of::<T>()).unwrap())
        } else {
            Ok(index)
        }
    }

    
    fn init_fetch<'w>(
        _world: &'w World,
        archetype: &'w Archetype,
        state: &'w Self::State,
        _tick: Tick,
        _last_run: Tick,
    ) -> Self::Fetch<'w> {
        match state {
            Ok(s) => Ok(&archetype.get_column_unchecked(*s)),
            Err(r) => Err(unsafe { &mut *r.downcast::<T>() }),
        }
    }

    
    fn fetch<'w>(fetch: &mut Self::Fetch<'w>, row: Row, _e: Entity) -> Self::Item<'w> {
        match fetch {
            Ok(c) => c.get(row),
            Err(r) => r,
        }
    }
}

pub struct Has<T: 'static>(PhantomData<T>);
impl<T: 'static> FetchComponents for Has<T> {
    type Fetch<'w> = bool;
    type Item<'w> = bool;
    type ReadOnly = Has<T>;
    type State = bool;
    const TICK_COUNT: usize = 0;

    fn init_state(world: &World, archetype: &Archetype) -> Self::State {
        !archetype.get_column_index_by_tid(&world, &TypeId::of::<T>()).is_null()
    }

    
    fn init_fetch<'w>(
        _world: &'w World,
        _archetype: &'w Archetype,
        state: &'w Self::State,
        _tick: Tick,
        _last_run: Tick,
    ) -> Self::Fetch<'w> {
        *state
    }

    
    fn fetch<'w>(fetch: &mut Self::Fetch<'w>, _row: Row, _e: Entity) -> Self::Item<'w> {
        *fetch
    }
}

#[derive(Debug)]
pub struct ArchetypeInfo<'a>(pub &'a Cow<'static, str>, pub Row);
impl FetchComponents for ArchetypeInfo<'_> {
    type Fetch<'w> = &'w Cow<'static, str>;
    type Item<'w> = ArchetypeInfo<'w>;
    type ReadOnly = ArchetypeInfo<'static>;
    type State = ();
    const TICK_COUNT: usize = 0;

    fn init_state(_world: &World, _: &Archetype) -> Self::State {
        ()
    }

    
    fn init_fetch<'w>(
        _world: &'w World,
        archetype: &'w Archetype,
        _state: &'w Self::State,
        _tick: Tick,
        _last_run: Tick,
    ) -> Self::Fetch<'w> {
        archetype.name()
    }

    
    fn fetch<'w>(fetch: &mut Self::Fetch<'w>, row: Row, _e: Entity) -> Self::Item<'w> {
        ArchetypeInfo(fetch, row)
    }
}

#[derive(Debug, Clone)]
pub struct ColumnTick<'a> {
    pub(crate) column: &'a Column,
    pub(crate) tick: Tick,
    pub(crate) last_run: Tick,
}
impl<'a> ColumnTick<'a> {
    fn new(column: &'a Column, tick: Tick, last_run: Tick) -> Self {
        Self {
            column,
            tick,
            last_run,
        }
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
        self.c.column.get::<T>(self.row)
    }
}

impl<'a, T: 'static> Deref for Ticker<'a, &'_ mut T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        self.c.column.get::<T>(self.row)
    }
}

impl<'a, T: 'static> Ticker<'a, &'_ mut T> {
    
    pub fn bypass_change_detection(&mut self) -> &mut T {
        self.c.column.get_mut::<T>(self.row)
    }
    
    pub fn set_changed(&mut self) {
        self.c.column.change_record(self.e, self.row, self.c.tick);
    }
}

impl<'a, T: 'static> DerefMut for Ticker<'a, &'_ mut T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.c.column.change_record(self.e, self.row, self.c.tick);
        self.c.column.get_mut::<T>(self.row)
    }
}

/// Unique mutable borrow of an entity's component
#[derive(Debug)]
pub struct Mut<'a, T: 'static> {
    pub(crate) column: &'a Column,
    pub e: Entity,
    pub(crate) row: Row,
    pub(crate) tick: Tick,
    _p: PhantomData<T>,
}
impl<'a, T: Sized> Mut<'a, T> {
    
    pub fn new(c: &ColumnTick<'a>, e: Entity, row: Row) -> Self {
        Self {
            column: c.column,
            e,
            row,
            tick: c.tick,
            _p: PhantomData,
        }
    }
    
    pub fn into_inner(self) -> &'a mut T {
        self.column.change_record(self.e, self.row, self.tick);
        self.column.get_mut::<T>(self.row)
    }

    
    pub fn bypass_change_detection(&mut self) -> &mut T {
        self.column.get_mut::<T>(self.row)
    }
    
    pub fn set_changed(&mut self) {
        self.column.change_record(self.e, self.row, self.tick);
    }
}
impl<'a, T: 'static> Deref for Mut<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        self.column.get::<T>(self.row)
    }
}
impl<'a, T: 'static> DerefMut for Mut<'a, T> {
    
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.column.change_record(self.e, self.row, self.tick);
        self.column.get_mut::<T>(self.row)
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
            const TICK_COUNT: usize = $($name::TICK_COUNT + )* 0;

            fn init_ticks(_world: &World, _ticks: &mut Vec<ComponentIndex>) {
                ($($name::init_ticks(_world, _ticks),)*);
            }
            fn init_read_write(_world: &mut World, _meta: &mut SystemMeta) {
                ($($name::init_read_write(_world, _meta),)*);
            }
            fn archetype_depend(_world: &World, _archetype: &Archetype, _result: &mut ArchetypeDependResult) {
                ($($name::archetype_depend(_world, _archetype, _result),)*);
            }
            fn res_depend(_res_tid: &TypeId, _res_name: &Cow<'static, str>, _single: bool, _result: &mut Flags) {
                ($($name::res_depend(_res_tid, _res_name, _single, _result),)*);
            }

            fn init_state(_world: &World, _archetype: &Archetype) -> Self::State {
                ($(
                    $name::init_state(_world, _archetype),
                )*)
            }

            
            #[allow(clippy::unused_unit)]
            fn init_fetch<'w>(
                _world: &'w World,
                _archetype: &'w Archetype,
                _state: &'w Self::State,
                _tick: Tick,
                _last_run: Tick,
                ) -> Self::Fetch<'w> {
                let ($($state,)*) = _state;
                ($($name::init_fetch(_world, _archetype, $state, _tick, _last_run),)*)
            }

            
            #[allow(clippy::unused_unit)]
            fn fetch<'w>(
                _fetch: &mut Self::Fetch<'w>,
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
