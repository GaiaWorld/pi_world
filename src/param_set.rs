//! 在system_parms的init_state时，会检查参数之间是否有读写冲突，比如组件读写冲突
//! 参数集， 用来容纳有读写冲突的参数，参数集内保证只有一个可以读写，所以参数集内的参数不彼此检查读写冲突

use std::mem::transmute;

use crate::function_system::SystemParamItem;
use crate::system::SystemMeta;
use crate::system_params::SystemParam;
use crate::world::*;
use pi_world_macros::impl_param_set;

pub use pi_world_macros::ParamSetElement;


pub struct ParamSet<'w, T: 'static + SystemParam>(<T as SystemParam>::Item<'w>);

impl_param_set!();


impl<T: 'static + SystemParam> SystemParam for ParamSet<'_, T> {
    type State = <T as SystemParam>::State;

    type Item<'w> = ParamSet<'w, T>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        meta.param_set_start();
        let s = T::init_state(world, meta);
        meta.param_set_end();
        // system_meta.param_set_ok();
        s
    }

    fn align(world: &World, system_meta: &SystemMeta, state: &mut Self::State) {
        <T as SystemParam>::align(world, system_meta, state)
    }
    fn get_param<'world>(
        world: &'world World,
        system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world> {
        ParamSet(<T as SystemParam>::get_param(world, system_meta, state, tick))
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
