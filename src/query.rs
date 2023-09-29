

use core::fmt::*;
use std::any::TypeId;
use std::marker::PhantomData;
use std::mem::{needs_drop, size_of, transmute};
use std::ops::{Index, IndexMut, Range};
use std::ptr::{copy, null_mut, write};
use std::slice;
use core::result::Result;
use pi_share::Share;

use crate::archetype::Archetype;
use crate::world::*;




pub unsafe trait WorldQuery {

    /// The item returned by this [`WorldQuery`]
    type Item<'a>;

    /// Per archetype/table state used by this [`WorldQuery`] to fetch [`Self::Item`](crate::query::WorldQuery::Item)
    //type Fetch<'a>: Clone;

    /// State used to construct a [`Self::Fetch`](crate::query::WorldQuery::Fetch). This will be cached inside [`QueryState`](crate::query::QueryState),
    /// so it is best to move as much data / computation here as possible to reduce the cost of
    /// constructing [`Self::Fetch`](crate::query::WorldQuery::Fetch).
    type State: Send + Sync + Sized;

    /// Creates and initializes a [`State`](WorldQuery::State) for this [`WorldQuery`] type.
    fn init_state(world: &mut World) -> Self::State;
    fn init_archetype(a: &Archetype);
    fn next(s: &Self::State) -> Option<Self::Item<'_>>;

}
pub struct Query<'world, 'state, Q: WorldQuery> {
    world: &'world World,
    state: &'state Q::State,
    this_run: Tick,
    _k: PhantomData<Q>,
}

impl<'world, 'state, Q: WorldQuery> Query<'world, 'state, Q> {
    pub fn new(world: &'world World, state: &'state Q::State, tick: Tick) -> Self {
        Query {
            world,
            state,
            this_run: tick,
            _k: PhantomData,
        }
    }
    pub fn get<T>(e: Entity) -> Result<&'static T, QueryComponentError> {
        todo!()
    }
    pub fn get_mut<T>(e: Entity) -> Result<&'static mut T, QueryComponentError> {
        todo!()
    }
}
impl<'world, 'state, Q: WorldQuery> Iterator for Query<'world, 'state, Q> {
    type Item = Q::Item<'state>;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        Q::next(self.state)
    }
}
pub struct QueryState<const C: usize> {
    vec: Vec<(Share<Archetype>, [usize; C])>,
    last_run: Tick,
    idx: usize,
}
impl <const C: usize> QueryState<C> {
    pub fn new() -> Self {
        QueryState {
            vec: Vec::new(),
            last_run: Default::default(),
            idx: 0,
        }
    }
    pub fn get_next(&self) -> (*mut u8, &'static [usize; C]) {
        todo!()
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Age(usize);

fn print_changed_entities(
    entity_with_mutated_component: Query<AQ1>,
) {
    for (entity, value) in entity_with_mutated_component {
        println!("entity: {:?} is now {:?} frames old", entity, value);
    }
}

/// 生成的代码
pub struct AQ1();
/// 生成的代码
unsafe impl WorldQuery for AQ1 {
    type Item<'a> = (Entity, &'a Age,);

    type State = QueryState<2>;

    fn init_state(world: &mut World) -> Self::State {
        QueryState::new()
    }

    fn init_archetype(a: &Archetype) {
        todo!()
    }

    fn next(s: &Self::State) -> Option<Self::Item<'_>>{
        let (ptr, offsets) = s.get_next();
        unsafe {Some((
            *(ptr.add(offsets[0]) as *mut Entity).clone(),
            & *(ptr.add(offsets[1]) as *mut Age),
        ))}
    }
}

/// 生成的代码
pub struct SysParms {
    p1: QueryState<2>,
}
impl SysParms {
    pub fn run(&self, world: &World, tick: Tick) {
        print_changed_entities(Query::new(world, &self.p1, tick));
    }
}
