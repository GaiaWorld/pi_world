use std::{
    marker::PhantomData, mem::transmute, ops::{Deref, DerefMut}
};

/// 系统参数的定义
///
use crate::{
    world_ptr::Ptr,
    archetype::ComponentInfo, prelude::FromWorld, system::{Relation, SystemMeta}, world::{ComponentIndex, World}
};

use pi_proc_macros::all_tuples;
pub use pi_world_macros::SystemParam;

pub trait SystemFetch: Sized + Send + Sync  {
    type Target: SystemParam;
    fn from_item(item: &mut Self::Target) -> Self;
    fn copy(item: &Self) -> Self;
}

impl<'w, T: SystemParam> SystemFetch for &'w mut T {
    type Target = T;

    fn from_item<'s>(item: &'s mut Self::Target) -> Self {
        unsafe {transmute(item)}
    }
    fn copy(item: &Self) -> Self {
        unsafe {transmute::<_, Self>(transmute::<_, usize>(&**item as &T))} 
    }
}



pub trait SystemParam: Sized + Send + Sync {
    /// Used to store data which persists across invocations of a system.
    type State: Send + Sync + 'static;

    type Item<'world>: SystemParam<State = Self::State> + 'world;

    type Fetch<'world>: SystemFetch<Target = Self::Item<'world>> = &'world mut Self::Item<'world>;

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
    fn align(world:&World, state: &mut Self::State) {}

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
        state: &'world mut Self::State,
    ) -> Self::Item<'world>;
    fn get_self<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self;
}

pub type Local<'a, T> = &'a mut LocalInner<'a, T>;
pub struct LocalInner<'a, T>(&'a mut T);

impl<'a, T: Sized> Deref for LocalInner<'a, T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'a, T: Sized> DerefMut for LocalInner<'a, T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}
// impl<'a, T: Sized> Local<'a, T> {
//     pub fn tick(&self) -> Tick {
//         self.1
//     }
// }

impl<T: Send + Sync + 'static + FromWorld> SystemParam for LocalInner<'_, T> {
    type State = T;

    type Item<'world> = LocalInner<'world, T>;

    fn init_state(world: &mut World, _system_meta: &mut SystemMeta) -> Self::State {
        T::from_world(world)
    }

    fn get_param<'world>(
        _world: &'world World,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        LocalInner(state)
    }
    #[inline]
    fn get_self<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, state)) }
    }
}

pub type ComponentDebugIndex<'a, T> = &'a mut ComponentDebugIndexInner<T>;
pub struct ComponentDebugIndexInner<T: 'static + Send + Sync>(pub ComponentIndex, PhantomData<T>);

impl<T: 'static + Send + Sync> SystemParam for ComponentDebugIndexInner<T> {
    type State = ComponentIndex;

    type Item<'world> = ComponentDebugIndexInner<T>;

    fn init_state(world: &mut World, _meta: &mut SystemMeta) -> Self::State {
        let info = ComponentInfo::of::<T>(0);
        let rc = world.add_component_info(info);
        rc.0
    }

    fn get_param<'world>(
        _world: &'world World,
        _state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        ComponentDebugIndexInner(_state.clone(), PhantomData)
    }
    #[inline]
    fn get_self<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, state)) }
    }
}

impl<'a > SystemFetch for &'a World {
    type Target = Self;

    fn from_item<'s>(item: &mut Self::Target) -> Self {
        unsafe {transmute(*item)}
    }
    fn copy(item: &Self) -> Self {
        *item
    }
}

impl SystemParam for &World {
    type State = Ptr<World>;

    type Item<'world> = &'world World;
    type Fetch<'world> = &'world World;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        // meta上增加world的类型
        meta.relate(Relation::ReadAll);
        meta.related_ok();
        meta.add_res(Relation::ReadAll);
        Ptr::new(world)
    }

    fn get_param<'world>(
        _world: &'world World,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        &*state
    }
    #[inline]
    fn get_self<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, state)) }
    }
}

impl<'a > SystemFetch for &'a mut World{
    type Target = Self;

    fn from_item<'s>(item: &mut Self::Target) -> Self {
        unsafe {transmute::<>(transmute::<_, usize>(&**item as &World))}
    }

    fn copy(item: &Self) -> Self {
        unsafe {transmute::<>(transmute::<_, usize>(&**item as &World))}
    }
}

impl SystemParam for &mut World {
    type State = Ptr<World>;

    type Item<'world> = &'world mut World;
    type Fetch<'world> = &'world mut World;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        // meta上增加world的类型
        meta.relate(Relation::WriteAll);
        meta.related_ok();
        meta.add_res(Relation::WriteAll);
        Ptr::new(world)
    }

    fn get_param<'world>(
        _world: &'world World,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        &mut *state
    }
    #[inline]
    fn get_self<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, state)) }
    }
}

macro_rules! impl_system_param_tuple {
    ($($param: ident),*) => {
        #[allow(non_snake_case)]
        impl<'w, $($param: SystemFetch),*> SystemFetch for ($($param,)*) {
            type Target = ($($param::Target,)*);

            fn from_item<'s>(item: &mut Self::Target) -> Self {
                let ($($param,)*) = item;
                ($($param::from_item($param),)*)
            }

            fn copy(item: &Self) -> Self {
                let ($($param,)*) = item;
                ($($param::copy($param),)*)
            }
        }
        // SAFETY: implementors of each `SystemParam` in the tuple have validated their impls
        #[allow(clippy::undocumented_unsafe_blocks)] // false positive by clippy
        #[allow(non_snake_case)]
        impl<$($param: SystemParam),*> SystemParam for ($($param,)*) {
            type State = ($($param::State,)*);
            type Item<'w> = ($($param::Item::<'w>,)*);
            type Fetch<'w> = ($($param::Fetch::<'w>,)*);

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
            fn align(_world: &World, state: &mut Self::State) {
                let ($($param,)*) = state;
                $($param::align(_world, $param);)*
            }

            #[allow(clippy::unused_unit)]
            fn get_param<'world>(
                _world: &'world World,
                state: &'world mut Self::State,
            ) -> Self::Item<'world> {
                let ($($param,)*) = state;
                ($($param::get_param(_world, $param),)*)
            }
            fn get_self<'world>(
                _world: &'world World,
                state: &'world mut Self::State,
            ) -> Self {
                let ($($param,)*) = state;
                ($($param::get_self(_world, $param),)*)
            }
        }
    };
}

all_tuples!(impl_system_param_tuple, 0, 32, P);
