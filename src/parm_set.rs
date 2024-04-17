//! 在system_parms的init_state时，会检查参数之间是否有读写冲突，比如组件读写冲突
//! 参数集， 用来容纳有读写冲突的参数，参数集内保证只有一个可以读写，所以参数集内的参数不彼此检查读写冲突

use std::any::TypeId;
use std::borrow::Cow;
use std::mem::transmute;
use std::ops::{Deref, DerefMut};

use crate::archetype::{Archetype, ArchetypeDependResult, Flags};

use crate::system::SystemMeta;
use crate::system_parms::SystemParm;
use crate::world::*;
use pi_proc_macros::all_tuples;

pub trait ParmSetElement: SystemParm {
    fn init_set_state(world: &World, system_meta: &mut SystemMeta) -> Self::State;
}

pub struct ParmSet<'w, T: 'static + ParmSetElement>(<T as SystemParm>::Item<'w>);
impl<'w, T: ParmSetElement + 'static> Deref for ParmSet<'w, T> {
    type Target = <T as SystemParm>::Item<'w>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'w, T: ParmSetElement + 'static> DerefMut for ParmSet<'w, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: 'static + ParmSetElement> SystemParm for ParmSet<'_, T> {
    type State = <T as SystemParm>::State;

    type Item<'w> = ParmSet<'w, T>;

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
        <T as SystemParm>::archetype_depend(world, system_meta, state, archetype, result)
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
        <T as SystemParm>::res_depend(world, system_meta, state, res_tid, res_name, single, result)
    }
    #[inline]
    fn align(world: &World, system_meta: &SystemMeta, state: &mut Self::State) {
        <T as SystemParm>::align(world, system_meta, state)
    }
    fn get_param<'world>(
        world: &'world World,
        system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        ParmSet(<T as SystemParm>::get_param(world, system_meta, state))
    }
    #[inline]
    fn get_self<'world>(
        world: &'world World,
        system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, system_meta, state)) }
    }
}

macro_rules! impl_param_set_tuple_fetch {
    ($($param: ident),*) => {
        #[allow(non_snake_case)]
        #[allow(clippy::unused_unit)]
        impl<$($param: ParmSetElement),*> ParmSetElement for ($($param,)*) {

            fn init_set_state(_world: &World, _system_meta: &mut SystemMeta) -> Self::State{
                (($($param::init_set_state(_world, _system_meta),)*))
            }
        }
    };
}
all_tuples!(impl_param_set_tuple_fetch, 1, 15, F);
