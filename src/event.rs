//! 事件，及组件移除
//!
use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::cell::SyncUnsafeCell;
use std::marker::PhantomData;
use std::mem::transmute;
use std::sync::atomic::Ordering;

use pi_share::{Share, ShareUsize};

use crate::archetype::{ComponentInfo, Flags};

use crate::safe_vec::{SafeVec, SafeVecIter};
use crate::system::{SystemMeta, TypeInfo};
use crate::system_params::SystemParam;
use crate::world::*;

pub trait Downcast {
    fn into_any(self: Share<Self>) -> Share<dyn Any + Send + Sync>;
    fn into_box_any(self: Box<Self>) -> Box<dyn Any>;
}

pub trait EventRecord: Downcast {
    fn settle(&mut self);
}

pub type ComponentRemovedRecord = EventRecordVec<Entity>;

#[derive(Debug, Default)]
pub struct EventRecordVec<E> {
    name: Cow<'static, str>,
    listeners: SyncUnsafeCell<Vec<ShareUsize>>, // 每个监听器的已读取的长度
    vec: SafeVec<E>,                            // 记录的事件
}
unsafe impl<E> Send for EventRecordVec<E> {}
unsafe impl<E> Sync for EventRecordVec<E> {}

impl<E> EventRecordVec<E> {
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self {
            name: name.into(),
            listeners: SyncUnsafeCell::new(Vec::new()),
            vec: SafeVec::default(),
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    /// 插入一个监听者，返回监听者的位置
    pub(crate) fn insert_listener(&self) -> usize {
        let listeners = unsafe { &mut *self.listeners.get() };
        let listener_index = listeners.len();
        listeners.push(ShareUsize::new(0));
        listener_index
    }
    #[inline(always)]
    pub(crate) fn record(&self, e: E) {
        self.vec.insert(e);
    }
    /// 获得指定监听者的读取长度
    pub(crate) fn len(&self, listener_index: usize) -> usize {
        let read_len = unsafe { (&*self.listeners.get()).get_unchecked(listener_index) };
        self.vec.len() - read_len.load(std::sync::atomic::Ordering::Relaxed)
    }
    /// 获得指定监听者的读取长度
    pub(crate) fn get_iter(&self, listener_index: usize) -> SafeVecIter<'_, E> {
        let end = self.vec.len();
        // 从上次读取到的位置开始读取
        let read_len = unsafe { (&*self.listeners.get()).get_unchecked(listener_index) };
        let start = read_len.swap(end, Ordering::Relaxed);
        self.vec.slice(start..end)
    }
    /// 判断是否能够清空脏列表
    pub(crate) fn can_clear(&mut self) -> Option<usize> {
        let len = self.vec.len();
        if len == 0 {
            return Some(0);
        }
        for read_len in (unsafe { &mut *self.listeners.get() }).iter_mut() {
            if *read_len.get_mut() < len {
                return None;
            }
        }
        // 只有所有的监听器都读取了全部的脏列表，才可以清空脏列表
        Some(len)
    }
    /// 清理方法
    pub(crate) fn clear(&mut self, len: usize) {
        self.vec.clear();
        // 以前用到了arr，所以扩容
        if self.vec.vec_capacity() < len {
            unsafe { self.vec.vec_reserve(len - self.vec.vec_capacity()) };
        }
        for read_len in (unsafe { &mut *self.listeners.get() }).iter_mut() {
            *read_len.get_mut() = 0;
        }
    }
    // 整理方法， 返回是否已经将脏列表清空，只有所有的监听器都读取了全部的脏列表，才可以清空脏列表
    pub(crate) fn settle(&mut self) -> bool {
        match self.can_clear() {
            Some(len) => {
                if len > 0 {
                    self.clear(len);
                }
                true
            }
            _ => false,
        }
    }
}

impl<E: 'static> EventRecord for EventRecordVec<E> {
    fn settle(&mut self) {
        self.settle();
    }
}
impl<E: 'static> Downcast for EventRecordVec<E> {
    fn into_any(self: Share<Self>) -> Share<dyn Any + Send + Sync> {
        self
    }

    fn into_box_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

pub struct Event<'w, E: 'static> {
    pub(crate) record: &'w Share<EventRecordVec<E>>,
    pub(crate) listener_index: usize,
}
unsafe impl<E> Send for Event<'_, E> {}
unsafe impl<E> Sync for Event<'_, E> {}

impl<'w, E: 'static> Event<'w, E> {
    #[inline]
    pub(crate) fn new(record: &'w Share<EventRecordVec<E>>, listener_index: usize) -> Self {
        Event {
            record,
            listener_index,
        }
    }
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.record.len(self.listener_index)
    }
    pub fn iter(&self) -> SafeVecIter<'_, E> {
        self.record.get_iter(self.listener_index)
    }
}

impl<E: 'static> SystemParam for Event<'_, E> {
    type State = (Share<EventRecordVec<E>>, usize);
    type Item<'w> = Event<'w, E>;

    fn init_state(world: &mut World, _system_meta: &mut SystemMeta) -> Self::State {
        let err = init_state(world, _system_meta);
        let index = err.insert_listener();
        (err, index)
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
        if (!single) && &TypeId::of::<E>() == res_tid {
            result.set(Flags::READ, true)
        }
    }

    #[inline]
    fn get_param<'world>(
        _world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        _tick: Tick,
    ) -> Self::Item<'world> {
        Event::new(&state.0, state.1)
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

pub struct EventSender<'w, E: 'static>(&'w Share<EventRecordVec<E>>);
unsafe impl<E> Send for EventSender<'_, E> {}
unsafe impl<E> Sync for EventSender<'_, E> {}

impl<'w, E: 'static> EventSender<'w, E> {
    pub fn send(&self, e: E) {
        self.0.record(e)
    }
}

impl<E: 'static> SystemParam for EventSender<'_, E> {
    type State = Share<EventRecordVec<E>>;
    type Item<'w> = EventSender<'w, E>;

    fn init_state(world: &mut World, _system_meta: &mut SystemMeta) -> Self::State {
        init_state(world, _system_meta)
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
        if (!single) && &TypeId::of::<E>() == res_tid {
            result.set(Flags::SHARE_WRITE, true)
        }
    }

    #[inline]
    fn get_param<'world>(
        _world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        _tick: Tick,
    ) -> Self::Item<'world> {
        EventSender(state)
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

pub struct ComponentRemoved<'w, T: 'static> {
    pub(crate) record: &'w Share<ComponentRemovedRecord>,
    pub(crate) listener_index: usize,
    _p: PhantomData<T>,
}
unsafe impl<T> Send for ComponentRemoved<'_, T> {}
unsafe impl<T> Sync for ComponentRemoved<'_, T> {}

impl<'w, T: 'static> ComponentRemoved<'w, T> {
    #[inline]
    pub(crate) fn new(record: &'w Share<ComponentRemovedRecord>, listener_index: usize) -> Self {
        ComponentRemoved {
            record,
            listener_index,
            _p: PhantomData,
        }
    }
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.record.len(self.listener_index)
    }
    pub fn iter(&self) -> SafeVecIter<'_, Entity> {
        self.record.get_iter(self.listener_index)
    }
}

impl<T: 'static> SystemParam for ComponentRemoved<'_, T> {
    type State = (Share<ComponentRemovedRecord>, usize);
    type Item<'w> = ComponentRemoved<'w, T>;

    fn init_state(world: &mut World, _system_meta: &mut SystemMeta) -> Self::State {
        let info = ComponentInfo::of::<T>(0);
        init_removed_state(world, info)
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
        _tick: Tick,
    ) -> Self::Item<'world> {
        ComponentRemoved::new(&state.0, state.1)
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

fn init_state<E: 'static>(
    world: &mut World,
    _system_meta: &mut SystemMeta,
) -> Share<EventRecordVec<E>> {
    let info = TypeInfo::of::<Event<E>>();
    let r = world.get_event_record(&info.type_id);
    if let Some(er) = r {
        Share::downcast::<EventRecordVec<E>>(er.into_any()).unwrap()
    } else {
        let r = Share::new(EventRecordVec::<E>::new(info.name.clone()));
        world.init_event_record(info.type_id, r.clone());
        r
    }
}

fn init_removed_state(
    world: &mut World,
    info: ComponentInfo,
) -> (Share<ComponentRemovedRecord>, usize) {
    let r = world.get_event_record(&info.type_id);
    let crr = if let Some(r) = r {
        Share::downcast::<ComponentRemovedRecord>(r.into_any()).unwrap()
    } else {
        let r = Share::new(ComponentRemovedRecord::new(info.type_name.clone()));
        world.init_event_record(info.type_id, r.clone());
        r
    };
    world.add_component_info(info);
    let index = crr.insert_listener();
    (crr, index)
}
