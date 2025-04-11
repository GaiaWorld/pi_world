//! 在system_parms的init_state时，会检查参数之间是否有读写冲突，比如组件读写冲突
//! 参数集， 用来容纳有读写冲突的参数，参数集内保证只有一个可以读写，所以参数集内的参数不彼此检查读写冲突

use std::mem::transmute;

use crate::function_system::SystemParamFetch2;
use crate::system::SystemMeta;
use crate::system_params::{SystemFetch, SystemParam};
use crate::world::*;
use pi_world_macros::impl_param_set;

pub use pi_world_macros::ParamSetElement;

pub type ParamSet<'w, T> = &'w mut ParamSetInner<'w, T>;
pub struct ParamSetInner<'w, T: 'static + SystemFetch>(<<T as SystemFetch>::Target as SystemParam>::Item<'w>);

impl_param_set!();


impl<T: 'static + SystemFetch> SystemParam for ParamSetInner<'_, T> {
    type State = <<T as SystemFetch>::Target as SystemParam>::State;

    type Item<'w> = ParamSetInner<'w, T>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        meta.param_set_start();
        let s = T::Target::init_state(world, meta);
        meta.param_set_end();
        // system_meta.param_set_ok();
        s
    }

    fn align(world: &World, state: &mut Self::State) {
        <<T as SystemFetch>::Target as SystemParam>::align(world, state)
    }
    fn get_param<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        ParamSetInner(<<T as SystemFetch>::Target as SystemParam>::get_param(world, state))
    }
    #[inline]
    fn get_self<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, state)) }
    }
}
