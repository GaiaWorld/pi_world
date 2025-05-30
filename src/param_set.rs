//! 在system_parms的init_state时，会检查参数之间是否有读写冲突，比如组件读写冲突
//! 参数集， 用来容纳有读写冲突的参数，参数集内保证只有一个可以读写，所以参数集内的参数不彼此检查读写冲突

use std::mem::transmute;

use crate::function_system::SystemParamItem;
use crate::system::SystemMeta;
use crate::system_params::SystemParam;
use crate::world::*;
use pi_world_macros::impl_param_set;

pub use pi_world_macros::ParamSetElement;

// pub type ParamSet<'w, T> = &'w mut ParamSet<'w, T>;
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
    

    fn align(state: &mut Self::State) {
        <T as SystemParam>::align(state)
    }
    fn get_param<'world>(
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        ParamSet(<T as SystemParam>::get_param(state))
    }
    #[inline]
    fn get_self<'world>(
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param( state)) }
    }
}
