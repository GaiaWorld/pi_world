//! 单例资源， 先system依次写，然后多system并行读

use std::any::Any;
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
    type State = (Option<Share<TickRes<T>>>, usize, Tick);
    type Item<'w> = SingleRes<'w, T>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        init_read_state(world, meta)
    }
    fn get_param<'world>(
        world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world> {
        if state.0.is_none() {
            init_opt_state(world, &mut state.0, state.1);
        }
        let r = state.0.as_ref().unwrap();
        let last_run = replace(&mut state.2, tick);
        SingleRes::new(&r, last_run)
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
    type State = (Option<Share<TickRes<T>>>, usize);
    type Item<'w> = SingleResMut<'w, T>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        init_write_state(world, meta)
    }

    fn get_param<'world>(
        world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world> {
        if state.0.is_none() {
            init_opt_state(world, &mut state.0, state.1);
        }
        let r = state.0.as_mut().unwrap();
        SingleResMut::new(unsafe { Share::get_mut_unchecked(r) }, tick)
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
    type State = (Option<Share<TickRes<T>>>, usize, Tick);
    type Item<'w> = Option<SingleRes<'w, T>>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        init_read_state(world, meta)
    }
    fn get_param<'world>(
        world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world> {
        if state.0.is_none() {
            let s = match world.index_single_res_any(state.1) {
                Some(r) => r.clone(),
                None => return None
            };
            state.0 = Some(Share::downcast::<TickRes<T>>(s.into_any()).unwrap());
        }
        if let Some(r) = &state.0 {
            let last_run = replace(&mut state.2, tick);
            Some(SingleRes::new(r.as_any().downcast_ref().unwrap(), last_run))
        } else {
            None
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
    type State = (Option<Share<TickRes<T>>>, usize);
    type Item<'w> = Option<SingleResMut<'w, T>>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        init_write_state(world, meta)
    }

    fn get_param<'world>(
        world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world> {
        if state.0.is_none() {
            if state.0.is_none() {
                let s = match world.index_single_res_any(state.1) {
                    Some(r) => r.clone(),
                    None => return None
                };
                state.0 = Some(Share::downcast::<TickRes<T>>(s.into_any()).unwrap());
            }
        }
        if let Some(r) = state.0.as_mut() {
            Some(SingleResMut::new(
                unsafe { Share::get_mut_unchecked(r) },
                tick,
            ))
        } else {
            None
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

fn init_read_state<T: 'static>(
    world: &mut World,
    meta: &mut SystemMeta,
) -> (Option<Share<TickRes<T>>>, usize, Tick) {
    let t = TypeInfo::of::<T>();
    let r = Relation::Read(t.type_id);
    let index = meta.add_single_res(world, t, r);
    let s = world.index_single_res_any(index);
    if let Some(s) = s {
        (
            Some(Share::downcast::<TickRes<T>>(s.clone().into_any()).unwrap()),
            index,
            Tick::default(),
        )
    } else {
        (None, index, Tick::default())
    }
}
fn init_write_state<T: 'static>(
    world: &mut World,
    meta: &mut SystemMeta,
) -> (Option<Share<TickRes<T>>>, usize) {
    let t = TypeInfo::of::<T>();
    let r = Relation::Write(t.type_id);
    let index = meta.add_single_res(world, t, r);
    let s = world.index_single_res_any(index);
    if let Some(s) = s {
        (
            Some(Share::downcast::<TickRes<T>>(s.clone().into_any()).unwrap()),
            index,
        )
    } else {
        (None, index)
    }
}

fn init_opt_state<T: 'static>(world: &World, state: &mut Option<Share<TickRes<T>>>, index: usize) {
    let s = world.index_single_res_any(index).unwrap().clone();
    *state = Some(Share::downcast::<TickRes<T>>(s.into_any()).unwrap());
}
