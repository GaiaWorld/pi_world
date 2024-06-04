use std::{any::TypeId, borrow::Cow, mem::transmute, ops::{Deref, DerefMut}};

/// 系统参数的定义
///
use crate::{
    archetype::{Archetype, ArchetypeDependResult, Flags}, prelude::FromWorld, system::SystemMeta, world::{Tick, World}
};

use pi_proc_macros::all_tuples;
pub use pi_world_macros::SystemParam;

pub trait SystemParam: Sized + Send + Sync {
    /// Used to store data which persists across invocations of a system.
    type State: Send + Sync + 'static;

    type Item<'world>: SystemParam<State = Self::State>;

    /// The item type returned when constructing this system param.
    /// The value of this associated type should be `Self`, instantiated with new lifetimes.
    ///
    /// You could think of `SystemParam::Item<'w, 's>` as being an *operation* that changes the lifetimes bound to `Self`.
    // type Item<'world, 'state>: SystemParam<State = Self::State>;

    /// Registers any [`World`] access used by this [`SystemParam`]
    /// and creates a new instance of this param's [`State`](Self::State).
    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State;

    // #[inline]
    // #[allow(unused_variables)]
    // fn archetype_depend(
    //     world: &World,
    //     system_meta: &SystemMeta,
    //     state: &Self::State,
    //     archetype: &Archetype,
    //     result: &mut ArchetypeDependResult,
    // ) {
    // }
    // /// system depend the res.
    // #[inline]
    // #[allow(unused_variables)]
    // fn res_depend(
    //     world: &World,
    //     system_meta: &SystemMeta,
    //     state: &Self::State,
    //     res_tid: &TypeId,
    //     res_name: &Cow<'static, str>,
    //     single: bool,
    //     result: &mut Flags,
    // ) {
    // }

    /// system align the world archetypes.
    #[inline]
    #[allow(unused_variables)]
    fn align(world: &World, system_meta: &SystemMeta, state: &mut Self::State) {}

    /// Creates a parameter to be passed into a [`SystemParamFunction`].
    ///
    /// [`SystemParamFunction`]: super::SystemParamFunction
    ///
    /// # Safety
    ///
    /// - The passed [`UnsafeWorldCell`] must have access to any world data
    ///   registered in [`init_state`](SystemParam::init_state).
    /// - `world` must be the same `World` that was used to initialize [`state`](SystemParam::init_state).
    fn get_param<'world>(
        world: &'world World,
        system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world>;
    fn get_self<'world>(
        world: &'world World,
        system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self;
}

pub struct Local<'a, T>(&'a mut T, Tick);

impl<'a, T: Sized> Deref for Local<'a, T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'a, T: Sized> DerefMut for Local<'a, T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}
impl <'a, T: Sized> Local<'a, T> {
    pub fn tick(&self) -> Tick {
        self.1
    }
}
impl<T: Send + Sync + 'static + FromWorld> SystemParam for Local<'_, T> {
    type State = T;

    type Item<'world> = Local<'world, T>;

    fn init_state(world: &mut World, _system_meta: &mut SystemMeta) -> Self::State {
        T::from_world(world)
    }

    fn get_param<'world>(
        _world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world> {
        Local(state, tick)
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

impl SystemParam for &World {
    type State = ();

    type Item<'world> = &'world World;

    fn init_state(_world: &mut World, _system_meta: &mut SystemMeta) -> Self::State {
        ()
    }

    fn get_param<'world>(
        world: &'world World,
        _system_meta: &'world SystemMeta,
        _state: &'world mut Self::State,
        _tick: Tick,
    ) -> Self::Item<'world> {
        world
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

impl SystemParam for &mut World {
    type State = ();

    type Item<'world> = &'world mut World;

    fn init_state(_world: &mut World, _system_meta: &mut SystemMeta) -> Self::State {
        //TODO
        ()
    }

    fn get_param<'world>(
        world: &'world World,
        _system_meta: &'world SystemMeta,
        _state: &'world mut Self::State,
        _tick: Tick,
    ) -> Self::Item<'world> {
        unsafe { &mut *(world as *const World as usize as *mut World) }
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

macro_rules! impl_system_param_tuple {
    ($($param: ident),*) => {
        // SAFETY: implementors of each `SystemParam` in the tuple have validated their impls
        #[allow(clippy::undocumented_unsafe_blocks)] // false positive by clippy
        #[allow(non_snake_case)]
        impl<$($param: SystemParam),*> SystemParam for ($($param,)*) {
            type State = ($($param::State,)*);
            type Item<'w> = ($($param::Item::<'w>,)*);

            fn init_state(_world: &mut World, _system_meta: &mut SystemMeta) -> Self::State {
                (($($param::init_state(_world, _system_meta),)*))
            }
            // #[inline]
            // fn archetype_depend(_world: &World, _system_meta: &SystemMeta, state: &Self::State, _archetype: &Archetype, _result: &mut ArchetypeDependResult) {
            //     let ($($param,)*) = state;
            //     $($param::archetype_depend(_world, _system_meta, $param, _archetype, _result);)*
            // }
            // #[inline]
            // fn res_depend(_world: &World, _system_meta: &SystemMeta, state: &Self::State, _res_tid: &TypeId, _res_name: &Cow<'static, str>, _single: bool, _result: &mut Flags) {
            //     let ($($param,)*) = state;
            //     $($param::res_depend(_world, _system_meta, $param, _res_tid, _res_name, _single, _result);)*
            // }
            fn align(_world: &World, _system_meta: &SystemMeta, state: &mut Self::State) {
                let ($($param,)*) = state;
                $($param::align(_world, _system_meta, $param);)*
            }

            #[allow(clippy::unused_unit)]
            fn get_param<'world>(
                _world: &'world World,
                _system_meta: &'world SystemMeta,
                state: &'world mut Self::State,
                _tick: Tick,
            ) -> Self::Item<'world> {
                let ($($param,)*) = state;
                ($($param::get_param(_world, _system_meta, $param, _tick),)*)
            }
            fn get_self<'world>(
                _world: &'world World,
                _system_meta: &'world SystemMeta,
                state: &'world mut Self::State,
                _tick: Tick,
            ) -> Self {
                let ($($param,)*) = state;
                ($($param::get_self(_world, _system_meta, $param, _tick),)*)
            }
        }
    };
}

all_tuples!(impl_system_param_tuple, 0, 32, P);
