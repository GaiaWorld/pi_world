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

pub type SingleRes<'w, T> = &'w mut SingleResInner<'w, T>;
pub struct SingleResInner<'w, T: 'static> {
    pub(crate) value: &'w TickRes<T>,
    pub(crate) state: &'w ResState,
}
impl<T: 'static + Debug> Debug for SingleResInner<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SingleRes").field("value", self.value).finish()
    }
}
unsafe impl<T> Send for SingleResInner<'_, T> {}
unsafe impl<T> Sync for SingleResInner<'_, T> {}
impl<'w, T: 'static> SingleResInner<'w, T> {
    pub(crate) fn new(value: &'w TickRes<T>, state: &'w ResState) -> Self {
        SingleResInner { value, state }
    }

    pub fn changed_tick(&self) -> Tick {
        self.value.changed_tick
    }

    pub fn is_changed(&self) -> bool {
        self.value.changed_tick > self.state.system_meta.last_run
    }
}


pub type OptionSingleRes<'w, T> = &'w mut OptionSingleResInner<'w, T>;
pub struct OptionSingleResInner<'w, T: 'static> {
    pub(crate) value: Option<SingleResInner<'w, T>>,
    pub(crate) state: &'w ResState,
    pub(crate) world: &'w World,
    mark: std::marker::PhantomData<&'w T>,
}
impl<T: 'static + Debug> Debug for OptionSingleResInner<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO
        f.debug_struct("OptionSingleRes").finish()
    }
}

impl<'w, T: 'static> Deref for OptionSingleResInner<'w, T> {
    type Target = Option<SingleResInner<'w, T>>;

    fn deref(&self) -> &Self::Target {
        let r = get_opt::<T>(self.state, self.world);
        let s = unsafe { transmute::<_, &mut Self>(transmute::<_, usize>(self)) };
        match r {
            Some(r) => {
                s.value = Some(SingleResInner::new(r, self.state));
            },
            None => {
                s.value = None;
            },
        };
        &s.value
    }
}

unsafe impl<T> Send for OptionSingleResInner<'_, T> {}
unsafe impl<T> Sync for OptionSingleResInner<'_, T> {}
impl<'w, T: 'static> OptionSingleResInner<'w, T> {
    pub(crate) fn new(state: &'w ResState, world: &'w World) -> Self {
        OptionSingleResInner {value: None, state, world, mark: PhantomData }
    }
}


pub type OptionSingleResMut<'w, T> = &'w mut OptionSingleResMutInner<'w, T>;
pub struct OptionSingleResMutInner<'w, T: 'static> {
    pub(crate) value: Option<SingleResMutInner<'w, T>>,
    pub(crate) state: &'w ResState,
    pub(crate) world: &'w World,
    mark: std::marker::PhantomData<&'w T>,
}
impl<T: 'static + Debug> Debug for OptionSingleResMutInner<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO
        f.debug_struct("OptionSingleResMut").finish()
    }
}

impl<'w, T: 'static> Deref for OptionSingleResMutInner<'w, T> {
    type Target = Option<SingleResMutInner<'w, T>>;

    fn deref(&self) -> &Self::Target {
        let s = unsafe { transmute::<_, &mut Self>(transmute::<_, usize>(self)) };
        let r = get_opt::<T>(self.state, self.world);
        match r {
            Some(r) => {
                s.value = Some(SingleResMutInner::new(r, self.state));
            },
            None => {
                s.value = None;
            },
        };
        &s.value
    }
}

impl<'w, T: 'static> DerefMut for OptionSingleResMutInner<'w, T> {

    fn deref_mut(&mut self) -> &mut Self::Target {
        let r = get_opt::<T>(self.state, self.world);
        match r {
            Some(r) => {
                self.value = Some(SingleResMutInner::new(r, self.state));
            },
            None => {
                self.value = None;
            },
        };
        &mut self.value
    }
}

unsafe impl<T> Send for OptionSingleResMutInner<'_, T> {}
unsafe impl<T> Sync for OptionSingleResMutInner<'_, T> {}
impl<'w, T: 'static> OptionSingleResMutInner<'w, T> {
    pub(crate) fn new(state: &'w mut ResState, world: &'w World) -> Self {
        OptionSingleResMutInner {value: None, state, world, mark: PhantomData }
    }
}

pub struct ResState {
    system_meta: Ptr<SystemMeta>,
    index: usize, 
}

impl<T: 'static + Send + Sync> SystemParam for SingleResInner<'_, T> {
    type State = ResState;
    type Item<'w> = SingleResInner<'w, T>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        init_state::<T>(world, meta)
    }
    fn get_param<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        let r = init_opt_state::<T>(state, world);
        SingleResInner::new(r, state)
    }

    fn get_self<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, state)) }
    }
}

impl<'w, T: Sync + Send + 'static> Deref for SingleResInner<'w, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value.res
    }
}

pub type SingleResMut<'w, T> = &'w mut SingleResMutInner<'w, T>;
pub struct SingleResMutInner<'w, T: 'static> {
    pub(crate) value: &'w mut TickRes<T>,
    pub(crate) state: &'w ResState,
}

impl<T: 'static + Debug> Debug for SingleResMutInner<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SingleResMut").field("value", self.value).finish()
    }
}
unsafe impl<T> Send for SingleResMutInner<'_, T> {}
unsafe impl<T> Sync for SingleResMutInner<'_, T> {}
impl<'w, T: 'static> SingleResMutInner<'w, T> {
    pub(crate) fn new(value: &'w mut TickRes<T>, state: &'w ResState) -> Self {
        SingleResMutInner { value, state }
    }

    pub fn changed_tick(&self) -> Tick {
        self.value.changed_tick
    }
    pub fn tick(&self) -> Tick {
        self.state.system_meta.this_run
    }
}
impl<T: 'static> SystemParam for SingleResMutInner<'_, T> {
    type State = ResState;
    type Item<'w> = SingleResMutInner<'w, T>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        init_state::<T>(world, meta)
    }

    fn get_param<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        let r = init_opt_state::<T>(state, world);
        SingleResMutInner::new(r, state)
    }

    fn get_self<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, state)) }
    }
}
impl<'w, T: Sync + Send + 'static> Deref for SingleResMutInner<'w, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value.res
    }
}
impl<'w, T: Sync + Send + 'static> DerefMut for SingleResMutInner<'w, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value.changed_tick = self.state.system_meta.this_run;
        &mut self.value.res
    }
}

impl<T: 'static> SystemParam for OptionSingleResInner<'_, T> {
    type State = ResState;
    type Item<'w> = OptionSingleResInner<'w, T>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        init_state::<T>(world, meta)
    }
    fn get_param<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        Self::Item::new(state, world)
    }

    fn get_self<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param( world, state )) }
    }
}

impl<T: 'static> SystemParam for OptionSingleResMutInner<'_, T> {
    type State = ResState;
    type Item<'w> = OptionSingleResMutInner<'w, T>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        init_state::<T>(world, meta)
    }

    fn get_param<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        OptionSingleResMutInner::new(state, world)
    }

    fn get_self<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, state)) }
    }
}

fn init_state<T: 'static>(
    world: &mut World,
    meta: &mut SystemMeta,
) -> ResState {
    let t = TypeInfo::of::<T>();
    let r = Relation::Read(t.type_id);
    let index = meta.add_single_res(world, t, r);

    ResState {
        index,
        system_meta: Ptr::new(meta)
    }
}

fn init_opt_state<T: 'static>(state: &ResState, world: &World) -> &'static mut TickRes<T> {
    let s = world.index_single_res_any(state.index).unwrap().clone();
    let r = Share::downcast::<TickRes<T>>(s.into_any()).unwrap();
    unsafe {
        &mut *( Share::as_ptr(&r) as usize as *mut TickRes<T>)
    }
}

fn get_opt<'w, T: 'static>(state: &'w ResState, world: &'w World) -> Option<&'static mut TickRes<T>> {
    match world.index_single_res_any(state.index) {
        Some(s) => Some(unsafe {
            &mut *( Share::as_ptr(&Share::downcast::<TickRes<T>>(s.clone().into_any()).unwrap()) as usize as *mut TickRes<T>)
        }),
        None => None,
    }
    // let s = .unwrap().clone();
    // let r = Share::downcast::<TickRes<T>>(s.into_any()).unwrap();
    // unsafe {
    //     &mut *( Share::as_ptr(&r) as usize as *mut TickRes<T>)
    // }
}
