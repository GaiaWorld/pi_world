use std::any::TypeId;
use std::marker::PhantomData;
use std::mem::transmute;

use pi_proc_macros::all_tuples;
use pi_slot::SlotMap;
// use pi_world_macros::ParamSetElement;

use crate::archetype::*;
use crate::column::Column;
use crate::param_set::ParamSetElement;
use crate::system::SystemMeta;
use crate::system_params::SystemParam;
use crate::world::*;
pub use pi_world_macros::Bundle;
pub use pi_world_macros::Component;

// 插入器， 一般是给外部的应用通过world上的make_inserter来创建和使用
pub struct Inserter<'world, I: Bundle> {
    world: &'world World,
    state: (ArchetypeWorldIndex, ShareArchetype, I::State),
    tick: Tick,
}

impl<'world, I: Bundle> Inserter<'world, I> {
    #[inline(always)]
    pub fn new(
        world: &'world World,
        state: (ArchetypeWorldIndex, ShareArchetype, I::State),
        tick: Tick,
    ) -> Self {
        Self { world, state, tick }
    }
    #[inline(always)]
    pub fn insert(&self, components: <I as Bundle>::Item) -> Entity {
        Insert::<I>::new(self.world, &self.state, self.tick).insert(components)
    }
    #[inline(always)]
    pub fn batch(&self, iter: impl IntoIterator<Item = <I as Bundle>::Item>) {
        let iter = iter.into_iter();
        let (lower, upper) = iter.size_hint();
        let length = upper.unwrap_or(lower);
        let ptr: *mut SlotMap<Entity, EntityAddr> = unsafe { transmute(&self.world.entities) };
        unsafe { &mut *ptr }.reserve(length);
        // 强行将原型转为可写
        let ptr = ShareArchetype::as_ptr(&self.state.1);
        let ar_mut: &mut Archetype = unsafe { transmute(ptr) };
        ar_mut.reserve(length);
        for item in iter {
            self.insert(item);
        }
    }
}


pub struct Insert<'world, I: Bundle> {
    pub(crate) world: &'world World,
    state: &'world (ArchetypeWorldIndex, ShareArchetype, I::State),
    tick: Tick,
}

impl<'world, I: Bundle> Insert<'world, I> {
    #[inline(always)]
    pub(crate) fn new(
        world: &'world World,
        state: &'world (ArchetypeWorldIndex, ShareArchetype, I::State),
        tick: Tick,
    ) -> Self {
        Insert { world, state, tick }
    }
    #[inline]
    pub fn tick(&self) -> Tick {
        self.tick
    }
    #[inline]
    pub fn insert(&self, components: <I as Bundle>::Item) -> Entity {
        let row = self.state.1.alloc();
        let e = self.world.insert(self.state.0, row);
        I::insert(&self.state.2, components, e, row, self.tick);
        self.state.1.set(row, e);
        e
    }
}

impl<I: Bundle + 'static> SystemParam for Insert<'_, I> {
    type State = (ArchetypeWorldIndex, ShareArchetype, I::State);
    type Item<'w> = Insert<'w, I>;

    fn init_state(world: &mut World, _system_meta: &mut SystemMeta) -> Self::State {
        // 如果world上没有找到对应的原型，则创建并放入world中
        let components = I::components();
        let id = ComponentInfo::calc_id(&components);
        let (ar_index, ar) = world.find_archtype(id, components);
        let s = I::init_state(world, &ar);
        (ar_index, ar, s)
    }
    fn archetype_depend(
        _world: &World,
        _system_meta: &SystemMeta,
        _state: &Self::State,
        archetype: &Archetype,
        depend: &mut ArchetypeDependResult,
    ) {
        let components = I::components();
        let id = ComponentInfo::calc_id(&components);
        if &id == archetype.id() {
            depend.merge(ArchetypeDepend::Flag(Flags::WRITE));
        }
    }

    #[inline]
    fn get_param<'world>(
        world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world> {
        Insert::new(world, state, tick)
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

impl<I: Bundle + 'static> ParamSetElement for Insert<'_, I>  {
    fn init_set_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State{
        let components = I::components();
        let id = ComponentInfo::calc_id(&components);

        for component in &components{
            system_meta.cur_param
            .writes
            .insert(component.type_id, component.type_name.clone());
        }
     
        let (ar_index, ar) = world.find_archtype(id, components);
        let s = I::init_state(world, &ar);
        system_meta.param_set_check();

        (ar_index, ar, s)
    }
}


pub trait Bundle {
    type Item;

    type State: Send + Sync + Sized;

    fn components() -> Vec<ComponentInfo>;

    fn init_state(world: &World, archetype: &Archetype) -> Self::State;

    fn insert(state: &Self::State, components: Self::Item, e: Entity, row: Row, tick: Tick);
}

pub struct TState<T: 'static>(pub *const Column, PhantomData<T>);
unsafe impl<T> Sync for TState<T> {}
unsafe impl<T> Send for TState<T> {}
impl<T: 'static> TState<T> {
    #[inline(always)]
    pub fn new(c: &Column) -> Self {
        TState(unsafe { transmute(c) }, PhantomData)
    }
    #[inline(always)]
    pub fn write(&self, e: Entity, row: Row, val: T, tick: Tick) {
        let c: &mut Column = unsafe { transmute(self.0) };
        c.write(row, val);
        c.add_record(e, row, tick);
    }
}

macro_rules! impl_tuple_insert {
    ($(($name: ident, $state: ident)),*) => {
        #[allow(non_snake_case)]
        #[allow(clippy::unused_unit)]
        impl<$($name: 'static),*> Bundle for ($($name,)*) {
            type Item = ($($name,)*);
            type State = ($(TState<$name>,)*);

            fn components() -> Vec<ComponentInfo> {
                vec![$(ComponentInfo::of::<$name>(),)*]
            }
            fn init_state(_world: &World, _archetype: &Archetype) -> Self::State {
                ($(TState::new(_archetype.get_column(&TypeId::of::<$name>()).unwrap().0),)*)
            }

            fn insert(
                _state: &Self::State,
                _components: Self::Item,
                _e: Entity,
                _row: Row,
                _tick: Tick,
            ) {
                let ($($name,)*) = _components;
                let ($($state,)*) = _state;
                $(
                    {$state.write(_e, _row, $name, _tick)}
                )*
            }
        }
    };
}
all_tuples!(impl_tuple_insert, 0, 32, F, S);

// impl<T: 'static> Bundle for T {
//     type Item = T;
//     type State = TState<T>;
//     fn components() -> Vec<ComponentInfo> {
//         vec![(ComponentInfo::of::<T>())]
//     }
//     fn init_state(_world: &World, archetype: &Archetype) -> Self::State {
//         TState::new(archetype.get_column(&TypeId::of::<T>()).unwrap())
//     }

//     fn insert(
//         state: &Self::State,
//         components: Self::Item,
//         e: Entity,
//         row: Row,
//     ) {
//         state.write(e, row, components);
//     }
// }
