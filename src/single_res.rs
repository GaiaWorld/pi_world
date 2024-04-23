//! 单例资源， 先system依次写，然后多system并行读

use std::any::TypeId;
use std::borrow::Cow;
use std::mem::transmute;
use std::ops::{Deref, DerefMut};

use crate::archetype::Flags;
use crate::system::SystemMeta;
use crate::system_params::SystemParam;
use crate::world::*;

pub struct SingleRes<'w, T: 'static> {
    pub(crate) value: &'w T,
}
unsafe impl<T> Send for SingleRes<'_, T> {}
unsafe impl<T> Sync for SingleRes<'_, T> {}

impl<T: 'static> SystemParam for SingleRes<'_, T> {
    type State = SingleResource;
    type Item<'w> = SingleRes<'w, T>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let tid = TypeId::of::<T>();
        let name = std::any::type_name::<T>().into();
        system_meta.res_read(tid, name);
        world.get_single_res_any(&tid).unwrap()
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

    #[inline]
    fn get_param<'world>(
        _world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        SingleRes {
            value: unsafe { &*state.downcast::<T>() },
        }
    }
    #[inline]
    fn get_self<'world>(
        world: &'world World,
        system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, system_meta, state)) }
    }
}

impl<'w, T: Sync + Send + 'static> Deref for SingleRes<'w, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

pub struct SingleResMut<'w, T: 'static> {
    pub(crate) value: &'w mut T,
}
unsafe impl<T> Send for SingleResMut<'_, T> {}
unsafe impl<T> Sync for SingleResMut<'_, T> {}

impl<T: 'static> SystemParam for SingleResMut<'_, T> {
    type State = SingleResource;
    type Item<'w> = SingleResMut<'w, T>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let tid = TypeId::of::<T>();
        let name = std::any::type_name::<T>().into();
        system_meta.res_write(tid, name);
        world.get_single_res_any(&TypeId::of::<T>()).unwrap()
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

    #[inline]
    fn get_param<'world>(
        _world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        SingleResMut {
            value: unsafe { &mut *state.downcast::<T>() },
        }
    }
    #[inline]
    fn get_self<'world>(
        world: &'world World,
        system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, system_meta, state)) }
    }
}
impl<'w, T: Sync + Send + 'static> Deref for SingleResMut<'w, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.value
    }
}
impl<'w, T: Sync + Send + 'static> DerefMut for SingleResMut<'w, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value
    }
}

impl<T: 'static> SystemParam for Option<SingleRes<'_, T>> {
    type State = Option<SingleResource>;
    type Item<'w> = Option<SingleRes<'w, T>>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let tid = TypeId::of::<T>();
        let name = std::any::type_name::<T>().into();
        system_meta.res_read(tid, name);
        world.get_single_res_any(&tid)
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

    #[inline]
    fn get_param<'world>(
        _world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        match state {
            Some(s) => Some(SingleRes {
                value: unsafe { &*s.downcast::<T>() },
            }),
            None => None,
        }
    }
    #[inline]
    fn get_self<'world>(
        world: &'world World,
        system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, system_meta, state)) }
    }
}

impl<T: 'static> SystemParam for Option<SingleResMut<'_, T>> {
    type State = Option<SingleResource>;
    type Item<'w> = Option<SingleResMut<'w, T>>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let tid = TypeId::of::<T>();
        let name = std::any::type_name::<T>().into();
        system_meta.res_write(tid, name);
        world.get_single_res_any(&tid)
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

    #[inline]
    fn get_param<'world>(
        _world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        match state {
            Some(s) => Some(SingleResMut {
                value: unsafe { &mut *s.downcast::<T>() },
            }),
            None => None,
        }
    }
    #[inline]
    fn get_self<'world>(
        world: &'world World,
        system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, system_meta, state)) }
    }
}
