//! 单例资源， 先system依次写，然后多system并行读

use std::any::Any;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::mem::transmute;
use std::ops::{Deref, DerefMut};

use pi_share::Share;

use crate::system::{Relation, SystemMeta, TypeInfo};
use crate::system_params::SystemParam;
use crate::world::{Downcast, Tick, TickMut, World};
use crate::world_ptr::Ptr;

#[derive(Debug, Default)]
pub struct TickRes<T> {
    pub(crate) res: T,
    pub(crate) changed_tick: Tick,
}
unsafe impl<T> Send for TickRes<T> {}
unsafe impl<T> Sync for TickRes<T> {}
impl<T: 'static> Downcast for TickRes<T> {
    fn into_any(self: Share<Self>) -> Share<dyn Any + Send + Sync> {
        self
    }
    fn into_box_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
impl<T: 'static> TickMut for TickRes<T> {
    fn get_tick(&self) -> Tick {
        self.changed_tick
    }
    fn set_tick(&mut self, tick: Tick) {
        self.changed_tick = tick;
    }
}
impl<T: 'static> TickRes<T> {
    pub fn new(res: T) -> Self {
        TickRes {
            res,
            changed_tick: Tick::default(),
        }
    }
}
impl<T: 'static> Deref for TickRes<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.res
    }
}
impl<T: 'static> DerefMut for TickRes<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.res
    }
}

// pub type SingleRes<'w, T> = &'w mut SingleRes<'w, T>;
pub struct SingleRes<'w, T: 'static + Send + Sync> {
    pub(crate) state: &'w ResState<T>,
}
impl<T: 'static + Debug + Send + Sync> Debug for SingleRes<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SingleRes").field("value", &**self).finish()
    }
}
unsafe impl<T: 'static + Send + Sync> Send for SingleRes<'_, T> {}
unsafe impl<T: 'static + Send + Sync> Sync for SingleRes<'_, T> {}
impl<'w, T: 'static + Send + Sync> SingleRes<'w, T> {
    pub(crate) fn new(state: &'w ResState<T>) -> Self {
        SingleRes { state }
    }

    pub fn changed_tick(&self) -> Tick {
        unsafe {&*self.state.value}.changed_tick
    }

    pub fn is_changed(&self) -> bool {
        unsafe {&*self.state.value}.changed_tick > self.state.state.system_meta.last_run
    }
}


// pub type OptionSingleRes<'w, T> = &'w mut OptionSingleRes<'w, T>;
// pub struct Option<> {
//     pub(crate) value: Option<SingleRes<'w, T>>,
//     pub(crate) state: &'w ResState<T>,
//     pub(crate) world: &'w World,
//     mark: std::marker::PhantomData<&'w T>,
// }
// impl<T: 'static + Debug> Debug for Option<SingleRes<'_, T>> {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         // TODO
//         f.debug_struct("OptionSingleRes").finish()
//     }
// }

// impl<'w, T: 'static> Deref for Option<SingleRes<'w, T>> {
//     type Target = Option<SingleRes<'w, T>>;

//     fn deref(&self) -> &Self::Target {
//         let r = get_opt::<T>(self.state, self.world);
//         let s = unsafe { transmute::<_, &mut Self>(transmute::<_, usize>(self)) };
//         match r {
//             Some(r) => {
//                 s.value = Some(SingleRes::new(self.state));
//             },
//             None => {
//                 s.value = None;
//             },
//         };
//         &s.value
//     }
// }

// unsafe impl<T> Send for OptionSingleRes<'_, T> {}
// unsafe impl<T> Sync for OptionSingleRes<'_, T> {}
// impl<'w, T: 'static> OptionSingleRes<'w, T> {
//     pub(crate) fn new(state: &'w ResState, world: &'w World) -> Self {
//         OptionSingleRes {value: None, state, world, mark: PhantomData }
//     }
// }


// pub type OptionSingleResMut<'w, T> = &'w mut OptionSingleResMut<'w, T>;
// pub struct OptionSingleResMut<'w, T: 'static> {
//     pub(crate) state: &'w ResState,
//     mark: std::marker::PhantomData<&'w T>,
// }
// impl<T: 'static + Debug> Debug for Option<SingleResMut<'_, T>> {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         // TODO
//         f.debug_struct("OptionSingleResMut").finish()
//     }
// }

// impl<'w, T: 'static> Deref for OptionSingleResMut<'w, T> {
//     type Target = Option<SingleResMut<'w, T>>;

//     fn deref(&self) -> &Self::Target {
//         let s = unsafe { transmute::<_, &mut Self>(transmute::<_, usize>(self)) };
//         let r = get_opt::<T>(self.state, self.world);
//         match r {
//             Some(r) => {
//                 s.value = Some(SingleResMut::new(self.state));
//             },
//             None => {
//                 s.value = None;
//             },
//         };
//         &s.value
//     }
// }

// impl<'w, T: 'static> DerefMut for OptionSingleResMut<'w, T> {

//     fn deref_mut(&mut self) -> &mut Self::Target {
//         let r = get_opt::<T>(self.state, self.world);
//         match r {
//             Some(r) => {
//                 self.value = Some(SingleResMut::new(self.state));
//             },
//             None => {
//                 self.value = None;
//             },
//         };
//         &mut self.value
//     }
// }

// unsafe impl<T> Send for OptionSingleResMut<'_, T> {}
// unsafe impl<T> Sync for OptionSingleResMut<'_, T> {}
// impl<'w, T: 'static> OptionSingleResMut<'w, T> {
//     pub(crate) fn new(state: &'w mut ResState<T>, world: &'w World) -> Self {
//         OptionSingleResMut {value: None, state, world, mark: PhantomData }
//     }
// }

pub struct ResState<T:'static + Send + Sync> {
    value: *mut TickRes<T>,
    state: ResState1,
}

unsafe impl<T:'static + Send + Sync> Send for ResState<T> {}
unsafe impl<T:'static + Send + Sync> Sync for ResState<T> {}
pub struct ResState1{
    system_meta: Ptr<SystemMeta>,
    world: Ptr<World>,
    index: usize, 
}

impl<T: 'static + Send + Sync> SystemParam for SingleRes<'_, T> {
    type State = ResState<T>;
    type Item<'w> = SingleRes<'w, T>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        init_state::<T>(world, meta)
    }
    #[inline(always)]
    fn init<'world>(state: &'world mut Self::State) {
        init_opt_state::<T>(state);
    }
    fn get_param<'world>(
        // world: &'world World,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        SingleRes::new(state)
    }

    fn get_self<'world>(
        // world: &'world World,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(state)) }
    }
}

impl<'w, T: Sync + Send + 'static> Deref for SingleRes<'w, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe {&*self.state.value}
    }
}

// pub type SingleResMut<'w, T> = &'w mut SingleResMut<'w, T>;
pub struct SingleResMut<'w, T: 'static + Send + Sync> {
    pub(crate) state: &'w ResState<T>,
    mark: PhantomData<T>
}

impl<T: 'static + Send + Sync + Debug> Debug for SingleResMut<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SingleResMut").field("value", &**self).finish()
    }
}
unsafe impl<T: 'static + Send + Sync> Send for SingleResMut<'_, T> {}
unsafe impl<T: 'static + Send + Sync> Sync for SingleResMut<'_, T> {}
impl<'w, T: 'static + Send + Sync> SingleResMut<'w, T> {
    pub(crate) fn new(state: &'w ResState<T>) -> Self {
        SingleResMut { state, mark: PhantomData }
    }

    pub fn changed_tick(&self) -> Tick {
        unsafe {&*self.state.value}.changed_tick
    }
    pub fn tick(&self) -> Tick {
        self.state.state.system_meta.this_run
    }
}
impl<T: 'static + Send + Sync> SystemParam for SingleResMut<'_, T> {
    type State = ResState<T>;
    type Item<'w> = SingleResMut<'w, T>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        init_state::<T>(world, meta)
    }

    #[inline(always)]
    fn init<'world>(state: &'world mut Self::State) {
        init_opt_state::<T>(state);
    }

    #[inline(always)]
    fn get_param<'world>(
        // world: &'world World,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        SingleResMut::new( state)
    }

    #[inline(always)]
    fn get_self<'world>(
        // world: &'world World,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(state)) }
    }
}
impl<'w, T: Sync + Send + 'static> Deref for SingleResMut<'w, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe {&*self.state.value}
    }
}
impl<'w, T: Sync + Send + 'static> DerefMut for SingleResMut<'w, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {&mut *self.state.value}.changed_tick = self.state.state.system_meta.this_run;
        unsafe {&mut **self.state.value}
    }
}

impl<T: 'static + Send + Sync> SystemParam for Option<SingleRes<'_, T>> {
    type State = ResState<T>;
    type Item<'w> = Option<SingleRes<'w, T>>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        init_state::<T>(world, meta)
    }
    fn get_param<'world>(
        // world: &'world World,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        match get_opt::<T>(&state.state) {
            Some(res) => {
                state.value = res;
                Some(SingleRes::new(state))
            },
            None => None,
        }
    }
    
    fn get_self<'world>(
        // world: &'world World,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param( state )) }
    }
}

impl<T: 'static + Send + Sync> SystemParam for Option<SingleResMut<'_, T>> {
    type State = ResState<T>;
    type Item<'w> = Option<SingleResMut<'w, T>>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        init_state::<T>(world, meta)
    }

    #[inline(always)]
    fn get_param<'world>(
        // world: &'world World,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        match get_opt::<T>(&state.state) {
            Some(res) => {
                state.value = res;
                Some(SingleResMut::new(state))
            },
            None => None,
        }
    }

    #[inline(always)]
    fn get_self<'world>(
        // world: &'world World,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(state)) }
    }
}

fn init_state<T: 'static + Send + Sync>(
    world: &mut World,
    meta: &mut SystemMeta,
) -> ResState<T> {
    let t = TypeInfo::of::<T>();
    let r = Relation::Read(t.type_id);
    let index = meta.add_single_res(world, t, r);
    ResState {
        value: 0 as *mut TickRes<T>,
        state: ResState1 {
            index,
            world: Ptr::new(world),
            system_meta: Ptr::new(meta)
        }
    }
}

#[inline(always)]
fn init_opt_state<T: 'static + Send + Sync>(state: &mut ResState<T>) {
    let s = state.state.world.index_single_res_any(state.state.index).unwrap();
    state.value = Share::as_ptr(&Share::downcast::<TickRes<T>>(s.clone().into_any()).unwrap()) as usize as *mut TickRes<T>;
}

#[inline(always)]
fn get_opt<'w, T: 'static + Send + Sync>(state: &'w ResState1) -> Option<*mut TickRes<T>> {
    // println!("get_opt======{:?}", (state.index, std::any::type_name::<T>()));
    match state.world.index_single_res_any(state.index) {
        Some(s) => Some(Share::as_ptr(&Share::downcast::<TickRes<T>>(s.clone().into_any()).unwrap()) as usize as *mut TickRes<T>),
        None => None,
    }
    // let s = .unwrap().clone();
    // let r = Share::downcast::<TickRes<T>>(s.into_any()).unwrap();
    // unsafe {
    //     &mut *( Share::as_ptr(&r) as usize as *mut TickRes<T>)
    // }
}
