//! 单例资源， 先system依次写，然后多system并行读

use std::any::TypeId;
use std::borrow::Cow;
use std::mem::{replace, transmute};
use std::ops::{Deref, DerefMut};

use crate::archetype::Flags;
use crate::system::{SystemMeta, TypeInfo};
use crate::system_params::SystemParam;
use crate::world::{SingleResource, Tick, World};

#[derive(Debug)]
pub struct SingleRes<'w, T: 'static> {
    pub(crate) value: &'w T,
    pub(crate) changed_tick: Tick,
    pub(crate) last_run: Tick,
}
unsafe impl<T> Send for SingleRes<'_, T> {}
unsafe impl<T> Sync for SingleRes<'_, T> {}
impl<'w, T: 'static> SingleRes<'w, T> {
    
    pub(crate) fn new(state: &'w SingleResource, last_run: Tick) -> Self {
        SingleRes {
            value: unsafe { &*state.downcast::<T>() },
            changed_tick: state.1,
            last_run,
        }
    }
    
    pub fn changed_tick(&self) -> Tick {
        self.changed_tick
    }
    
    pub fn is_changed(&self) -> bool {
        self.changed_tick > self.last_run
    }
}

impl<T: 'static> SystemParam for SingleRes<'_, T> {
    type State = (SingleResource, Tick);
    type Item<'w> = SingleRes<'w, T>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let info = TypeInfo::of::<T>();
        (single_resource(world, system_meta, &info, true).unwrap(), Tick::default())
    }
    fn res_depend(
        _world: &World,
        _system_meta: &SystemMeta,
        _state: &Self::State,
        res_tid: &TypeId,
        _res_name: &Cow<'static, str>,
        single: bool,
        result: &mut Flags,
    ) {
        if single && &TypeId::of::<T>() == res_tid {
            result.set(Flags::READ, true)
        }
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
        self.value
    }
}

#[derive(Debug)]
pub struct SingleResMut<'w, T: 'static> {
    pub(crate) value: &'w mut T,
    pub(crate) changed_tick: &'w mut Tick,
    pub(crate) tick: Tick,
}
unsafe impl<T> Send for SingleResMut<'_, T> {}
unsafe impl<T> Sync for SingleResMut<'_, T> {}
impl<'w, T: 'static> SingleResMut<'w, T> {
    
    pub(crate) fn new(state: &'w mut SingleResource, tick: Tick) -> Self {
        SingleResMut {
            value: unsafe { &mut *state.downcast::<T>() },
            changed_tick: &mut state.1,
            tick,
        }
    }
    
    pub fn changed_tick(&self) -> Tick {
        *self.changed_tick
    }
}
impl<T: 'static> SystemParam for SingleResMut<'_, T> {
    type State = SingleResource;
    type Item<'w> = SingleResMut<'w, T>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let info = TypeInfo::of::<T>();
        match single_resource(world, system_meta, &info, false) {
            Some(r) => r,
            None => panic!("init SingleRes fail, {:?} is not exist", std::any::type_name::<T>()),
        }
    }
    fn res_depend(
        _world: &World,
        _system_meta: &SystemMeta,
        _state: &Self::State,
        res_tid: &TypeId,
        _res_name: &Cow<'static, str>,
        single: bool,
        result: &mut Flags,
    ) {
        if single && &TypeId::of::<T>() == res_tid {
            result.set(Flags::WRITE, true)
        }
    }

    
    fn get_param<'world>(
        _world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world> {
        SingleResMut::new(state, tick)
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
        self.value
    }
}
impl<'w, T: Sync + Send + 'static> DerefMut for SingleResMut<'w, T> {
    
    fn deref_mut(&mut self) -> &mut Self::Target {
        *self.changed_tick = self.tick;
        self.value
    }
}

impl<T: 'static> SystemParam for Option<SingleRes<'_, T>> {
    type State = (usize, Tick);
    type Item<'w> = Option<SingleRes<'w, T>>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let info = TypeInfo::of::<T>();
        (or_single_resource(world, system_meta, info, true), Tick::default())
    }
    fn res_depend(
        _world: &World,
        _system_meta: &SystemMeta,
        _state: &Self::State,
        res_tid: &TypeId,
        _res_name: &Cow<'static, str>,
        single: bool,
        result: &mut Flags,
    ) {
        if single && &TypeId::of::<T>() == res_tid {
            result.set(Flags::READ, true)
        }
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
                Some(SingleRes::new(r, last_run))
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

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let info = TypeInfo::of::<T>();
        or_single_resource(world, system_meta, info, false)
    }
    fn res_depend(
        _world: &World,
        _system_meta: &SystemMeta,
        _state: &Self::State,
        res_tid: &TypeId,
        _res_name: &Cow<'static, str>,
        single: bool,
        result: &mut Flags,
    ) {
        if single && &TypeId::of::<T>() == res_tid {
            result.set(Flags::WRITE, true)
        }
    }

    
    fn get_param<'world>(
        world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world> {
        match world.index_single_res_any(*state) {
            Some(r) => Some(SingleResMut::new(r, tick)),
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

fn single_resource(world: &World, system_meta: &mut SystemMeta, info: &TypeInfo, read: bool) -> Option<SingleResource> {
    if read {
        system_meta.res_read(&info);
    }else{
        system_meta.res_write(&info);
    }
    world.get_single_res_any(&info.type_id)
}
fn or_single_resource(world: &mut World, system_meta: &mut SystemMeta, info: TypeInfo, read: bool) -> usize {
    if read {
        system_meta.res_read(&info);
    }else{
        system_meta.res_write(&info);
    }
    world.or_register_single_res(info)
}
