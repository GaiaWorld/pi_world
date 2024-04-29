//! 多例资源， 每个独立system都维护该类型的自己独立的资源。
//! 这样多个独立system可以并行写，然后多读system并行读，读的时候遍历所有写system的独立资源。
//! 这样就实现了并行写，并行读。

use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::marker::PhantomData;
use std::mem::transmute;
use std::ops::{Deref, DerefMut};

use pi_share::Share;

use crate::archetype::Flags;
use crate::system::SystemMeta;
use crate::system_params::SystemParam;
use crate::world::*;

pub struct MultiRes<'w, T: 'static> {
    pub(crate) vec: &'w Vec<Share<dyn Any>>,
    tick: Tick,
    _p: PhantomData<T>,
}
unsafe impl<T> Send for MultiRes<'_, T> {}
unsafe impl<T> Sync for MultiRes<'_, T> {}

impl<T> MultiRes<'_, T> {
    pub fn len(&self) -> usize {
        self.vec.len()
    }
    pub fn get(&self, index: usize) -> Option<&T> {
        self.vec
            .get(index)
            .map(|r| unsafe { r.downcast_ref_unchecked::<T>() })
    }
    pub unsafe fn get_unchecked(&self, index: usize) -> &T {
        unsafe { self.vec.get_unchecked(index).downcast_ref_unchecked::<T>() }
    }
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.vec
            .iter()
            .map(|r| unsafe { r.downcast_ref_unchecked::<T>() })
    }
}
impl<T: 'static> SystemParam for MultiRes<'_, T> {
    type State = MultiResource;
    type Item<'w> = MultiRes<'w, T>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let tid = TypeId::of::<T>();
        let name = std::any::type_name::<T>().into();
        system_meta.res_read(tid, name);
        world.system_read_multi_res(&tid).unwrap()
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
        MultiRes {
            vec: state.vec(),
            tick,
            _p: PhantomData,
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

pub struct MultiResMut<'w, T: Default + 'static> {
    pub(crate) value: &'w mut T,
    tick: Tick,
}
unsafe impl<T: Default> Send for MultiResMut<'_, T> {}
unsafe impl<T: Default> Sync for MultiResMut<'_, T> {}

impl<T: Default + 'static> SystemParam for MultiResMut<'_, T> {
    type State = SingleResource;
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
        MultiResMut {
            value: unsafe { &mut *state.downcast::<T>() },
            tick,
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
impl<'w, T: Default + Sync + Send + 'static> Deref for MultiResMut<'w, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.value
    }
}
impl<'w, T: Default + Sync + Send + 'static> DerefMut for MultiResMut<'w, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value
    }
}

impl<T: 'static> SystemParam for Option<MultiRes<'_, T>> {
    type State = Option<MultiResource>;
    type Item<'w> = Option<MultiRes<'w, T>>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let tid = TypeId::of::<T>();
        let name = std::any::type_name::<T>().into();
        system_meta.res_read(tid, name);
        world.system_read_multi_res(&tid)
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
            Some(s) => Some(MultiRes {
                vec: s.vec(),
                tick,
                _p: PhantomData,
            }),
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
    type State = Option<SingleResource>;
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
            Some(s) => Some(MultiResMut {
                value: unsafe { &mut *s.downcast::<T>() },
                tick,
            }),
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
