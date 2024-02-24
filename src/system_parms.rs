/// 系统参数的定义
///
use crate::{
    archetype::{Archetype, ArchetypeDependResult}, system::SystemMeta, world::{Tick, World}
};

use pi_proc_macros::all_tuples;


/// 本地记录的tick
pub struct LocalTick<'a>{
    pub last: &'a mut Tick,
    pub cur: Tick,
}

impl<'a> SystemParam for LocalTick<'a> {
    type State = Tick;
    type Item<'w> = LocalTick<'w>;

    fn init_state(_world: &World, _system_meta: &mut SystemMeta) -> Self::State {
        0
    }

    #[inline]
    fn get_param<'world>(
        state: &'world mut Self::State,
        _system_meta: &'world SystemMeta,
        _world: &'world World,
        change_tick: Tick,
    ) -> Self::Item<'world> {
        LocalTick {
            last: state,
            cur: change_tick,
        }
    }

}

pub trait SystemParam: Sized {
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
    fn init_state(world: &World, system_meta: &mut SystemMeta) -> Self::State;

    #[inline]
    #[allow(unused_variables)]
    fn depend(world: &World, system_meta: &SystemMeta, state: &Self::State, archetype: &Archetype, result: &mut ArchetypeDependResult){}

    /// Applies any deferred mutations stored in this [`SystemParam`]'s state.
    /// This is used to apply [`Commands`] during [`apply_deferred`](crate::prelude::apply_deferred).
    ///
    /// [`Commands`]: crate::prelude::Commands
    #[inline]
    #[allow(unused_variables)]
    fn before(
        state: &mut Self::State,
        system_meta: &mut SystemMeta,
        world: &World,
        change_tick: Tick,
    ) {
    }

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
        state: &'world mut Self::State,
        system_meta: &'world SystemMeta,
        world: &'world World,
        change_tick: Tick,
    ) -> Self::Item<'world>;
    #[inline]
    #[allow(unused_variables)]
    fn after(
        state: &mut Self::State,
        system_meta: &mut SystemMeta,
        world: &World,
        change_tick: Tick,
    ) {
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

            #[inline]
            fn init_state(_world: &World, _system_meta: &mut SystemMeta) -> Self::State {
                (($($param::init_state(_world, _system_meta),)*))
            }
            #[inline]
            fn depend(_world: &World, _system_meta: &SystemMeta, state: &Self::State, _archetype: &Archetype, _result: &mut ArchetypeDependResult) {
                let ($($param,)*) = state;
                $($param::depend(_world, _system_meta, $param, _archetype, _result);)*
            }


            #[inline]
            fn before(($($param,)*): &mut Self::State, _system_meta: &mut SystemMeta, _world: &World, _change_tick: Tick) {
                $($param::before($param, _system_meta, _world, _change_tick);)*
            }

            #[inline]
            #[allow(clippy::unused_unit)]
            fn get_param<'world>(
                state: &'world mut Self::State,
                _system_meta: &'world SystemMeta,
                _world: &'world World,
                _change_tick: Tick,
            ) -> Self::Item<'world> {
                let ($($param,)*) = state;
                ($($param::get_param($param, _system_meta, _world, _change_tick),)*)
            }
            #[inline]
            fn after(($($param,)*): &mut Self::State, _system_meta: &mut SystemMeta, _world: &World, _change_tick: Tick) {
                $($param::after($param, _system_meta, _world, _change_tick);)*
            }
        }
    };
}

all_tuples!(impl_system_param_tuple, 0, 16, P);
