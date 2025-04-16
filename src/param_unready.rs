//! 在system_parms的init_state时，会检查参数之间是否有读写冲突，比如组件读写冲突
//! 参数集， 用来容纳有读写冲突的参数，参数集内保证只有一个可以读写，所以参数集内的参数不彼此检查读写冲突

use std::mem::transmute;

use crate::system::SystemMeta;
use crate::system_params::SystemParam;
use crate::world::*;

pub use pi_world_macros::ParamSetElement;

pub struct ParamUnReady<'w, T: 'static + SystemParam>(pub(crate) &'w mut <T as SystemParam>::State);


impl<T: 'static + SystemParam> SystemParam for ParamUnReady<'_, T> {
    type State = <T as SystemParam>::State;

    type Item<'w> = ParamUnReady<'w, T>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        T::init_state(world, meta)
    }
    
    fn get_param<'world>(
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        ParamUnReady(state)
    }
    #[inline]
    fn get_self<'world>(
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param( state)) }
    }
}

impl<'w, T: 'static + SystemParam> ParamUnReady<'w, T> {
    pub fn ready(self) -> <T as SystemParam>::Item<'w> {
        <T as SystemParam>::align(self.0);
        <T as SystemParam>::get_param(self.0)
    }
}
