//! 多例资源， 每个独立system都维护该类型的自己独立的资源。
//! 这样多个独立system可以并行写，然后多读system并行读，读的时候遍历所有写system的独立资源。
//! 这样就实现了并行写，并行读。
//! 每个独立的资源都有自己的Tick， 并且多例资源有一个共享的Tick。 todo!()

use std::any::TypeId;
use std::borrow::Cow;
use std::marker::PhantomData;
use std::mem::{replace, transmute};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::Ordering;

use pi_share::{Share, ShareU32};

use crate::archetype::Flags;
use crate::single_res::{SingleRes, SingleResMut};
use crate::system::SystemMeta;
use crate::system_params::SystemParam;
use crate::world::*;

pub struct MultiRes<'w, T: 'static> {
    pub(crate) vec: &'w Vec<SingleResource>,
    changed_tick: Tick,
    last_run: Tick,
    tick: Tick,
    _p: PhantomData<T>,
}
unsafe impl<T> Send for MultiRes<'_, T> {}
unsafe impl<T> Sync for MultiRes<'_, T> {}

impl<'w, T: 'static> MultiRes<'w, T> {
    #[inline]
    pub(crate) fn new(state: &'w mut (MultiResource, Tick), tick: Tick) -> Self {
        let last_run = replace(&mut state.1, tick);
        MultiRes {
            vec: state.0.vec(),
            changed_tick: state.0.changed_tick(),
            last_run,
            tick,
            _p: PhantomData,
        }
    }
    pub fn len(&self) -> usize {
        self.vec.len()
    }
    #[inline(always)]
    pub fn tick(&self) -> Tick {
        self.tick
    }
    #[inline(always)]
    pub fn changed_tick(&self) -> Tick {
        self.changed_tick
    }
    #[inline(always)]
    pub fn is_changed(&self) -> bool {
        self.changed_tick > self.last_run
    }
    pub fn get(&self, index: usize) -> Option<SingleRes<T>> {
        self.vec
            .get(index)
            .map(|r| SingleRes::new(r, self.last_run))
    }
    pub unsafe fn get_unchecked(&self, index: usize) -> SingleRes<T> {
        SingleRes::new(unsafe { self.vec.get_unchecked(index) }, self.last_run)
    }
    pub fn iter(&self) -> impl Iterator<Item = SingleRes<T>> {
        self.vec.iter().map(|r| SingleRes::new(r, self.last_run))
    }
}
impl<T: 'static> SystemParam for MultiRes<'_, T> {
    type State = (MultiResource, Tick);
    type Item<'w> = MultiRes<'w, T>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let tid = TypeId::of::<T>();
        let name = std::any::type_name::<T>().into();
        system_meta.res_read(tid, name);
        (world.system_read_multi_res(&tid).unwrap(), Tick::default())
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
        if (!single) && &TypeId::of::<T>() == res_tid {
            result.set(Flags::READ, true)
        }
    }

    #[inline]
    fn get_param<'world>(
        _world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world> {
        MultiRes::new(state, tick)
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

pub struct MultiResMut<'w, T: Default + 'static> {
    pub(crate) value: SingleResMut<'w, T>,
    changed_tick: &'w Share<ShareU32>,
    tick: Tick,
}
unsafe impl<T: Default> Send for MultiResMut<'_, T> {}
unsafe impl<T: Default> Sync for MultiResMut<'_, T> {}
impl<'w, T: Default + 'static> MultiResMut<'w, T> {
    #[inline]
    fn new(state: &'w mut (SingleResource, Share<ShareU32>), tick: Tick) -> Self {
        MultiResMut {
            value: SingleResMut::new(&mut state.0, tick),
            changed_tick: &state.1,
            tick,
        }
    }
    #[inline(always)]
    pub fn changed_tick(&self) -> Tick {
        Tick::from(self.changed_tick.load(Ordering::Relaxed))
    }
}
impl<T: Default + 'static> SystemParam for MultiResMut<'_, T> {
    type State = (SingleResource, Share<ShareU32>);
    type Item<'w> = MultiResMut<'w, T>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let tid = TypeId::of::<T>();
        let name = std::any::type_name::<T>().into();
        system_meta.res_write(tid, name);
        world.system_init_write_multi_res(T::default).unwrap()
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
        if (!single) && &TypeId::of::<T>() == res_tid {
            result.set(Flags::SHARE_WRITE, true)
        }
    }

    #[inline]
    fn get_param<'world>(
        _world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world> {
        MultiResMut::new(state, tick)
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
impl<'w, T: Default + Sync + Send + 'static> Deref for MultiResMut<'w, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
impl<'w, T: Default + Sync + Send + 'static> DerefMut for MultiResMut<'w, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.changed_tick.store(self.tick.into(), Ordering::Relaxed);
        &mut self.value
    }
}

impl<T: 'static> SystemParam for Option<MultiRes<'_, T>> {
    type State = Option<(MultiResource, Tick)>;
    type Item<'w> = Option<MultiRes<'w, T>>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let tid = TypeId::of::<T>();
        let name = std::any::type_name::<T>().into();
        system_meta.res_read(tid, name);
        world
            .system_read_multi_res(&tid)
            .map(|r| (r, Tick::default()))
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
        if (!single) && &TypeId::of::<T>() == res_tid {
            result.set(Flags::READ, true)
        }
    }

    #[inline]
    fn get_param<'world>(
        _world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world> {
        match state {
            Some(s) => Some(MultiRes::new(s, tick)),
            None => None,
        }
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

impl<T: Default + 'static> SystemParam for Option<MultiResMut<'_, T>> {
    type State = Option<(SingleResource, Share<ShareU32>)>;
    type Item<'w> = Option<MultiResMut<'w, T>>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let tid = TypeId::of::<T>();
        let name = std::any::type_name::<T>().into();
        system_meta.res_write(tid, name);
        world.system_init_write_multi_res(T::default)
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
        if (!single) && &TypeId::of::<T>() == res_tid {
            result.set(Flags::WRITE, true)
        }
    }

    #[inline]
    fn get_param<'world>(
        _world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world> {
        match state {
            Some(s) => Some(MultiResMut::new(s, tick)),
            None => None,
        }
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
