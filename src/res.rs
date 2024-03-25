use std::any::TypeId;
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

use crate::archetype::Flags;
use crate::system::SystemMeta;
use crate::system_parms::SystemParam;
use crate::world::*;

pub struct Res<'w, T: 'static> {
    pub(crate) value: &'w T,
}

impl<T: 'static> SystemParam for Res<'_, T> {
    type State = ResValue;
    type Item<'w> = Res<'w, T>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let tid = TypeId::of::<T>();
        let name = std::any::type_name::<T>().into();
        system_meta.res_read(tid, name);
        world.get_res_any(&tid).unwrap()
    }
    fn res_depend(
        _world: &World,
        _system_meta: &SystemMeta,
        _state: &Self::State,
        res_tid: &TypeId,
        _res_name: &Cow<'static, str>,
        result: &mut Flags,
    ) {
        if &TypeId::of::<T>() == res_tid {
            result.set(Flags::READ, true)
        }
    }

    #[inline]
    fn get_param<'world>(
        _world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        Res {
            value: unsafe { &*state.downcast::<T>() },
        }
    }
}

impl<'w, T: Sync + Send + 'static> Deref for Res<'w, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

pub struct ResMut<'w, T: 'static> {
    pub(crate) value: &'w mut T,
}

impl<T: 'static> SystemParam for ResMut<'_, T> {
    type State = ResValue;
    type Item<'w> = ResMut<'w, T>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let tid = TypeId::of::<T>();
        let name = std::any::type_name::<T>().into();
        system_meta.res_write(tid, name);
        world.get_res_any(&TypeId::of::<T>()).unwrap()
    }
    fn res_depend(
        _world: &World,
        _system_meta: &SystemMeta,
        _state: &Self::State,
        res_tid: &TypeId,
        _res_name: &Cow<'static, str>,
        result: &mut Flags,
    ) {
        if &TypeId::of::<T>() == res_tid {
            result.set(Flags::WRITE, true)
        }
    }

    #[inline]
    fn get_param<'world>(
        _world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        ResMut {
            value: unsafe { &mut *state.downcast::<T>() },
        }
    }
}
impl<'w, T: Sync + Send + 'static> Deref for ResMut<'w, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.value
    }
}
impl<'w, T: Sync + Send + 'static> DerefMut for ResMut<'w, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value
    }
}

impl<T: 'static> SystemParam for Option<Res<'_, T>> {
    type State = Option<ResValue>;
    type Item<'w> = Option<Res<'w, T>>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let tid = TypeId::of::<T>();
        let name = std::any::type_name::<T>().into();
        system_meta.res_read(tid, name);
        world.get_res_any(&tid)
    }
    fn res_depend(
        _world: &World,
        _system_meta: &SystemMeta,
        _state: &Self::State,
        res_tid: &TypeId,
        _res_name: &Cow<'static, str>,
        result: &mut Flags,
    ) {
        if &TypeId::of::<T>() == res_tid {
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
            Some(s) => Some(Res {
                value: unsafe { &*s.downcast::<T>() },
            }),
            None => None,
        }
    }
}

impl<T: 'static> SystemParam for Option<ResMut<'_, T>> {
    type State = Option<ResValue>;
    type Item<'w> = Option<ResMut<'w, T>>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let tid = TypeId::of::<T>();
        let name = std::any::type_name::<T>().into();
        system_meta.res_write(tid, name);
        world.get_res_any(&tid)
    }
    fn res_depend(
        _world: &World,
        _system_meta: &SystemMeta,
        _state: &Self::State,
        res_tid: &TypeId,
        _res_name: &Cow<'static, str>,
        result: &mut Flags,
    ) {
        if &TypeId::of::<T>() == res_tid {
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
            Some(s) => Some(ResMut {
                value: unsafe { &mut *s.downcast::<T>() },
            }),
            None => None,
        }
    }
}
