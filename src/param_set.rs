//! 在system_parms的init_state时，会检查参数之间是否有读写冲突，比如组件读写冲突
//! 参数集， 用来容纳有读写冲突的参数，参数集内保证只有一个可以读写，所以参数集内的参数不彼此检查读写冲突

use std::any::TypeId;
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

use crate::archetype::{Archetype, ArchetypeDependResult, Flags};

use crate::system::SystemMeta;
use crate::system_parms::SystemParam;
use crate::world::*;
use pi_proc_macros::all_tuples;

pub trait ParamSetElement: SystemParam {
    fn init_set_state(world: &World, system_meta: &mut SystemMeta) -> Self::State;
}

pub struct ParamSet<'w, T: 'static + ParamSetElement>(<T as SystemParam>::Item<'w>);
impl<'w, T: ParamSetElement + 'static> Deref for ParamSet<'w, T> {
    type Target = <T as SystemParam>::Item<'w>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'w, T: ParamSetElement + 'static> DerefMut for ParamSet<'w, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: 'static + ParamSetElement> SystemParam for ParamSet<'_, T> {
    type State = <T as SystemParam>::State;

    type Item<'w> = ParamSet<'w, T>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let s = T::init_set_state(world, system_meta);
        system_meta.param_set_ok();
        s
    }
    #[inline]
    fn archetype_depend(
        world: &World,
        system_meta: &SystemMeta,
        state: &Self::State,
        archetype: &Archetype,
        result: &mut ArchetypeDependResult,
    ) {
        <T as SystemParam>::archetype_depend(world, system_meta, state, archetype, result)
    }
    #[inline]
    fn res_depend(
        world: &World,
        system_meta: &SystemMeta,
        state: &Self::State,
        res_tid: &TypeId,
        res_name: &Cow<'static, str>,
        single: bool,
        result: &mut Flags,
    ) {
        <T as SystemParam>::res_depend(world, system_meta, state, res_tid, res_name, single, result)
    }
    #[inline]
    fn align(world: &World, system_meta: &SystemMeta, state: &mut Self::State) {
        <T as SystemParam>::align(world, system_meta, state)
    }
    fn get_param<'world>(
        world: &'world World,
        system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        ParamSet(<T as SystemParam>::get_param(world, system_meta, state))
    }
}

macro_rules! impl_param_set_tuple_fetch {
    ($($param: ident),*) => {
        #[allow(non_snake_case)]
        #[allow(clippy::unused_unit)]
        impl<$($param: ParamSetElement),*> ParamSetElement for ($($param,)*) {

            fn init_set_state(_world: &World, _system_meta: &mut SystemMeta) -> Self::State{
                (($($param::init_set_state(_world, _system_meta),)*))
            }
        }
    };
}
all_tuples!(impl_param_set_tuple_fetch, 1, 15, F);
