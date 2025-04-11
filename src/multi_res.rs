//! 多例资源， 每个独立system都维护该类型的自己独立的资源。
//! 这样多个独立system可以并行写，然后多个system并行读，读的时候遍历所有写system的独立资源。
//! 这样就实现了并行写，并行读。
//! 每个独立的资源都有自己的Tick， 并且多例资源有一个共享的Tick。

use std::any::TypeId;
use std::mem::{replace, transmute};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::Ordering;

use pi_share::{Share, ShareUsize};

use crate::single_res::{SingleResInner, SingleResMutInner, TickRes};
use crate::system::{Relation, SystemMeta};
use crate::system_params::SystemParam;
use crate::world::*;

#[derive(Debug)]
pub struct ResVec<T: 'static> {
    vec: Vec<TickRes<T>>,
}
impl<T: 'static> ResVec<T> {
    pub fn new() -> Self {
        Self { vec: Vec::with_capacity(256) }
    }
    pub fn insert(&mut self, value: T) -> usize {
        let index = self.vec.len();
        self.vec.push(TickRes::new(value));
        index
    }
    pub fn len(&self) -> usize {
        self.vec.len()
    }
    #[inline(always)]
    pub fn get(&self, index: usize) -> Option<&TickRes<T>> {
        self.vec.get(index)
    }
    #[inline(always)]
    pub unsafe fn get_unchecked(&self, index: usize) -> &TickRes<T> {
        self.vec.get_unchecked(index)
    }
    pub fn iter(&self) -> impl Iterator<Item = &TickRes<T>> {
        self.vec.iter()
    }
}

unsafe impl<T: 'static> Send for ResVec<T> {}
unsafe impl<T: 'static> Sync for ResVec<T> {}

pub struct MultiRes<'w, T: 'static> {
    pub(crate) vec: &'w ResVec<T>,
    changed_tick: Tick,
    last_run: Tick,
    tick: Tick,
}
unsafe impl<T: 'static> Send for MultiRes<'_, T> {}
unsafe impl<T: 'static> Sync for MultiRes<'_, T> {}

// impl<'w, T: 'static> MultiRes<'w, T> {
//     #[inline]
//     pub(crate) fn new(vec: &'w ResVec<T>, changed_tick: Tick, last_run: Tick, tick: Tick) -> Self {
//         MultiRes {
//             vec,
//             changed_tick,
//             last_run,
//             tick,
//         }
//     }
//     pub fn len(&self) -> usize {
//         self.vec.len()
//     }
//     #[inline(always)]
//     pub fn tick(&self) -> Tick {
//         self.tick
//     }
//     #[inline(always)]
//     pub fn changed_tick(&self) -> Tick {
//         self.changed_tick
//     }
//     #[inline(always)]
//     pub fn is_changed(&self) -> bool {
//         self.changed_tick > self.last_run
//     }
//     pub fn get(&self, index: usize) -> Option<SingleRes<T>> {
//         self.vec
//             .get(index)
//             .map(|r| SingleRes::new(r, self.last_run))
//     }
//     pub unsafe fn get_unchecked(&self, index: usize) -> SingleRes<T> {
//         SingleRes::new(unsafe { self.vec.get_unchecked(index) }, self.last_run)
//     }
//     pub fn iter(&self) -> impl Iterator<Item = SingleRes<T>> {
//         self.vec.iter().map(|r| SingleRes::new(r, self.last_run))
//     }
// }
// impl<T: 'static> SystemParam for MultiRes<'_, T> {
//     type State = (Share<ResVec<T>>, Share<ShareUsize>, Tick);
//     type Item<'w> = MultiRes<'w, T>;

//     fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
//         let id: TypeId = TypeId::of::<T>();
//         meta.add_res(Relation::Read(id));
//         let (r, changed_tick) = world.init_multi_res(id, Share::new(ResVec::<T>::new()));
//         (Share::downcast(r).unwrap(), changed_tick, Tick::default())
//     }

//     #[inline]
//     fn get_param<'world>(
//         _world: &'world World,
//         _system_meta: &'world SystemMeta,
//         state: &'world mut Self::State,
//         tick: Tick,
//     ) -> Self::Item<'world> {
//         let last_run = replace(&mut state.2, tick);
//         MultiRes::new(
//             &state.0,
//             state.1.load(Ordering::Relaxed).into(),
//             last_run,
//             tick,
//         )
//     }
//     #[inline]
//     fn get_self<'world>(
//         world: &'world World,
//         system_meta: &'world SystemMeta,
//         state: &'world mut Self::State,
//         tick: Tick,
//     ) -> Self {
//         unsafe { transmute(Self::get_param(world, system_meta, state, tick)) }
//     }
// }

// pub struct MultiResMut<'w, T: FromWorld + 'static> {
//     pub(crate) value: SingleResMut<'w, T>,
//     changed_tick: &'w Share<ShareUsize>,
// }
// unsafe impl<T: FromWorld> Send for MultiResMut<'_, T> {}
// unsafe impl<T: FromWorld> Sync for MultiResMut<'_, T> {}
// impl<'w, T: FromWorld + 'static> MultiResMut<'w, T> {
//     #[inline]
//     fn new(value: SingleResMut<'w, T>, changed_tick: &'w Share<ShareUsize>) -> Self {
//         MultiResMut {
//             value,
//             changed_tick,
//         }
//     }
//     #[inline(always)]
//     pub fn changed_tick(&self) -> Tick {
//         Tick::from(self.changed_tick.load(Ordering::Relaxed))
//     }
// }
// impl<T: FromWorld + 'static> SystemParam for MultiResMut<'_, T> {
//     type State = (Share<ResVec<T>>, Share<ShareUsize>, usize);
//     type Item<'w> = MultiResMut<'w, T>;

//     fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
//         let id: TypeId = TypeId::of::<T>();
//         meta.add_res(Relation::ShareWrite(id));
//         let (r, changed_tick) = world.init_multi_res(id, Share::new(ResVec::<T>::new()));
//         let mut res_vec: Share<ResVec<T>> = Share::downcast(r).unwrap();
//         let vec = unsafe { Share::get_mut_unchecked(&mut res_vec) };
//         let index = vec.insert(T::from_world(world));
//         (res_vec, changed_tick, index)
//     }
//     #[inline]
//     fn get_param<'world>(
//         _world: &'world World,
//         _system_meta: &'world SystemMeta,
//         state: &'world mut Self::State,
//         tick: Tick,
//     ) -> Self::Item<'world> {
//         let vec = unsafe { Share::get_mut_unchecked(&mut state.0) };
//         let res = unsafe { vec.vec.get_unchecked_mut(state.2) };
//         let value = SingleResMut::new(res, tick);
//         MultiResMut::new(value, &state.1)
//     }
//     #[inline]
//     fn get_self<'world>(
//         world: &'world World,
//         system_meta: &'world SystemMeta,
//         state: &'world mut Self::State,
//         tick: Tick,
//     ) -> Self {
//         unsafe { transmute(Self::get_param(world, system_meta, state, tick)) }
//     }
// }
// impl<'w, T: FromWorld + Sync + Send + 'static> Deref for MultiResMut<'w, T> {
//     type Target = T;
//     #[inline]
//     fn deref(&self) -> &Self::Target {
//         &self.value
//     }
// }
// impl<'w, T: FromWorld + Sync + Send + 'static> DerefMut for MultiResMut<'w, T> {
//     #[inline]
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         self.changed_tick
//             .store(self.value.tick().index(), Ordering::Relaxed);
//         &mut self.value
//     }
// }
