//! 单例资源， 先system依次写，然后多system并行读

use std::any::{Any, TypeId};
use std::mem::{replace, transmute};
use std::ops::{Deref, DerefMut};

use pi_share::Share;

use crate::system::{Relation, SystemMeta, TypeInfo};
use crate::system_params::SystemParam;
use crate::world::{Downcast, Tick, TickMut, World};

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
#[derive(Debug)]
pub struct SingleRes<'w, T: 'static> {
    pub(crate) value: &'w TickRes<T>,
    pub(crate) last_run: Tick,
}
unsafe impl<T> Send for SingleRes<'_, T> {}
unsafe impl<T> Sync for SingleRes<'_, T> {}
impl<'w, T: 'static> SingleRes<'w, T> {
    pub(crate) fn new(value: &'w TickRes<T>, last_run: Tick) -> Self {
        SingleRes { value, last_run }
    }

    pub fn changed_tick(&self) -> Tick {
        self.value.changed_tick
    }

    pub fn is_changed(&self) -> bool {
        self.value.changed_tick > self.last_run
    }
}

impl<T: 'static + Send + Sync> SystemParam for SingleRes<'_, T> {
    type State = (Share<TickRes<T>>, Tick);
    type Item<'w> = SingleRes<'w, T>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        let s = meta
            .add_single_res(
                world,
                TypeInfo::of::<T>(),
                Relation::Read(TypeId::of::<()>()),
            )
            .unwrap()
            .clone();
        (
            Share::downcast::<TickRes<T>>(s.into_any()).unwrap(),
            Tick::default(),
        )
    }
    fn get_param<'world>(
        _world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world> {
        let last_run = replace(&mut state.1, tick);
        SingleRes::new(&state.0, last_run)
    }

    fn get_self<'world>(
        world: &'world World,
        system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, system_meta, state, tick)) }
    }
}

impl<'w, T: Sync + Send + 'static> Deref for SingleRes<'w, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value.res
    }
}

#[derive(Debug)]
pub struct SingleResMut<'w, T: 'static> {
    pub(crate) value: &'w mut TickRes<T>,
    pub(crate) tick: Tick,
}
unsafe impl<T> Send for SingleResMut<'_, T> {}
unsafe impl<T> Sync for SingleResMut<'_, T> {}
impl<'w, T: 'static> SingleResMut<'w, T> {
    pub(crate) fn new(value: &'w mut TickRes<T>, tick: Tick) -> Self {
        SingleResMut { value, tick }
    }

    pub fn changed_tick(&self) -> Tick {
        self.value.changed_tick
    }
    pub fn tick(&self) -> Tick {
        self.tick
    }
}
impl<T: 'static> SystemParam for SingleResMut<'_, T> {
    type State = Share<TickRes<T>>;
    type Item<'w> = SingleResMut<'w, T>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        let s = meta
            .add_single_res(
                world,
                TypeInfo::of::<T>(),
                Relation::Read(TypeId::of::<()>()),
            )
            .unwrap()
            .clone();
        Share::downcast::<TickRes<T>>(s.into_any()).unwrap()
    }

    fn get_param<'world>(
        _world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world> {
        SingleResMut::new(unsafe { Share::get_mut_unchecked(state) }, tick)
    }

    fn get_self<'world>(
        world: &'world World,
        system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, system_meta, state, tick)) }
    }
}
impl<'w, T: Sync + Send + 'static> Deref for SingleResMut<'w, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value.res
    }
}
impl<'w, T: Sync + Send + 'static> DerefMut for SingleResMut<'w, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value.changed_tick = self.tick;
        &mut self.value.res
    }
}

impl<T: 'static> SystemParam for Option<SingleRes<'_, T>> {
    type State = (usize, Tick);
    type Item<'w> = Option<SingleRes<'w, T>>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        let s = meta.add_or_single_res(
            world,
            TypeInfo::of::<T>(),
            Relation::Read(TypeId::of::<()>()),
        );
        (s, Tick::default())
    }
    fn get_param<'world>(
        world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world> {
        match world.index_single_res_any(state.0) {
            Some(r) => {
                let last_run = replace(&mut state.1, tick);
                Some(SingleRes::new(r.as_any().downcast_ref().unwrap(), last_run))
            }
            None => None,
        }
    }

    fn get_self<'world>(
        world: &'world World,
        system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, system_meta, state, tick)) }
    }
}

impl<T: 'static> SystemParam for Option<SingleResMut<'_, T>> {
    type State = usize;
    type Item<'w> = Option<SingleResMut<'w, T>>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        meta.add_or_single_res(
            world,
            TypeInfo::of::<TickRes<T>>(),
            Relation::Write(TypeId::of::<()>()),
        )
    }

    fn get_param<'world>(
        world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world> {
        match world.index_single_res_any(*state) {
            Some(r) => {
                let value = unsafe {
                    &mut *(r.as_any().downcast_ref_unchecked() as *const TickRes<T> as usize as *mut TickRes<T>)
                };
                Some(SingleResMut::new(value, tick))
            }
            None => None,
        }
    }

    fn get_self<'world>(
        world: &'world World,
        system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, system_meta, state, tick)) }
    }
}
