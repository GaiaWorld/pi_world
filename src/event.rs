//! 事件，及组件移除
//!
use std::any::Any;
use std::borrow::Cow;
use std::marker::PhantomData;
use std::mem::transmute;
use std::ops::Deref;
use std::sync::atomic::Ordering;

use pi_append_vec::{SafeVec, SafeVecIter};
use pi_share::{Share, ShareUsize};

use crate::archetype::{ComponentInfo, COMPONENT_TICK};

use crate::column::{Column, ColumnInfo};
use crate::system::{SystemMeta, TypeInfo};
use crate::system_params::SystemParam;
use crate::world::*;

pub type ComponentEventVec = EventVec<Entity>;

#[derive(Debug, Default)]
pub struct EventVec<E> {
    name: Cow<'static, str>,
    listeners: Vec<ShareUsize>, // 每个监听器的已读取的长度
    vec: SafeVec<E>,            // 记录的事件
}
unsafe impl<E> Send for EventVec<E> {}
unsafe impl<E> Sync for EventVec<E> {}

impl<E> EventVec<E> {
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self {
            name: name.into(),
            listeners: Vec::new(),
            vec: SafeVec::default(),
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    /// 插入一个监听者，返回监听者的位置
    pub(crate) fn insert_listener(&mut self) -> usize {
        // let listeners = unsafe { &mut *self.listeners.get() };
        let listener_index = self.listeners.len();
        self.listeners.push(ShareUsize::new(0));
        listener_index
    }
    #[inline(always)]
    pub(crate) fn record(&self, e: E) {
        self.vec.insert(e);
    }
    /// 获得指定监听者的读取长度
    pub(crate) fn len(&self, listener_index: usize) -> usize {
        let read_len = unsafe { self.listeners.get_unchecked(listener_index) };
        self.vec.len() - read_len.load(std::sync::atomic::Ordering::Relaxed)
    }
    /// 获得指定监听者的读取长度
    pub(crate) fn get_iter(&self, listener_index: usize) -> SafeVecIter<'_, E> {
        let end = self.vec.len();
        // 从上次读取到的位置开始读取
        let read_len = unsafe { self.listeners.get_unchecked(listener_index) };
        let start = read_len.swap(end, Ordering::Relaxed);
        self.vec.slice(start..end)
    }
    /// 判断是否能够清空脏列表
    pub(crate) fn can_clear(&mut self) -> Option<usize> {
        let len = self.vec.len();
        if len == 0 {
            return Some(0);
        }
        for read_len in self.listeners.iter_mut() {
            if *read_len.get_mut() < len {
                return None;
            }
        }
        // 只有所有的监听器都读取了全部的脏列表，才可以清空脏列表
        Some(len)
    }
    /// 清理方法
    pub(crate) fn clear(&mut self) {
        self.vec.clear(0);
        for read_len in self.listeners.iter_mut() {
            *read_len.get_mut() = 0;
        }
    }
    // 整理方法， 返回是否已经将脏列表清空，只有所有的监听器都读取了全部的脏列表，才可以清空脏列表
    pub(crate) fn settle(&mut self) -> bool {
        match self.can_clear() {
            Some(len) => {
                if len > 0 {
                    self.clear();
                }
                true
            }
            _ => false,
        }
    }
}

impl<E: 'static> Settle for EventVec<E> {
    fn settle(&mut self) {
        self.settle();
    }
}
impl<E: 'static> Downcast for EventVec<E> {
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

pub type EventReader<'w, E> = Event<'w, E>;

pub struct Event<'w, E: 'static> {
    pub(crate) record: &'w Share<EventVec<E>>,
    pub(crate) listener_index: usize,
}
unsafe impl<E> Send for Event<'_, E> {}
unsafe impl<E> Sync for Event<'_, E> {}

impl<'w, E: 'static> Event<'w, E> {
    #[inline]
    pub(crate) fn new(record: &'w Share<EventVec<E>>, listener_index: usize) -> Self {
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
    type State = (Share<EventVec<E>>, usize);
    type Item<'w> = Event<'w, E>;

    fn init_state(world: &mut World, _system_meta: &mut SystemMeta) -> Self::State {
        let mut vec = init_state(world);
        let index = unsafe { Share::get_mut_unchecked(&mut vec).insert_listener() };
        (vec, index)
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

pub type EventWriter<'w, E> = EventSender<'w, E>;

pub struct EventSender<'w, E: 'static>(&'w Share<EventVec<E>>);
unsafe impl<E> Send for EventSender<'_, E> {}
unsafe impl<E> Sync for EventSender<'_, E> {}

impl<'w, E: 'static> EventSender<'w, E> {
    pub fn send(&self, e: E) {
        self.0.record(e)
    }
}

impl<E: 'static> SystemParam for EventSender<'_, E> {
    type State = Share<EventVec<E>>;
    type Item<'w> = EventSender<'w, E>;

    fn init_state(world: &mut World, _system_meta: &mut SystemMeta) -> Self::State {
        init_state(world)
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

pub struct ComponentChanged<'w, T: 'static>(ComponentEvent<'w, T>);
unsafe impl<T> Send for ComponentChanged<'_, T> {}
unsafe impl<T> Sync for ComponentChanged<'_, T> {}
impl<'w, T> Deref for ComponentChanged<'w, T> {
    type Target = ComponentEvent<'w, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: 'static> SystemParam for ComponentChanged<'_, T> {
    type State = (Share<ComponentEventVec>, usize);
    type Item<'w> = ComponentChanged<'w, T>;

    fn init_state(world: &mut World, _meta: &mut SystemMeta) -> Self::State {
        let info = ComponentInfo::of::<T>(COMPONENT_TICK);
        init_changed_state(world, info)
    }

    #[inline]
    fn get_param<'world>(
        _world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        _tick: Tick,
    ) -> Self::Item<'world> {
        ComponentChanged(ComponentEvent::new(&state.0, state.1))
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

pub struct ComponentAdded<'w, T: 'static>(ComponentEvent<'w, T>);
unsafe impl<T> Send for ComponentAdded<'_, T> {}
unsafe impl<T> Sync for ComponentAdded<'_, T> {}
impl<'w, T> Deref for ComponentAdded<'w, T> {
    type Target = ComponentEvent<'w, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: 'static> SystemParam for ComponentAdded<'_, T> {
    type State = (Share<ComponentEventVec>, usize);
    type Item<'w> = ComponentAdded<'w, T>;

    fn init_state(world: &mut World, _meta: &mut SystemMeta) -> Self::State {
        let info = ComponentInfo::of::<T>(0);
        init_added_state(world, info)
    }

    #[inline]
    fn get_param<'world>(
        _world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        _tick: Tick,
    ) -> Self::Item<'world> {
        ComponentAdded(ComponentEvent::new(&state.0, state.1))
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
pub struct ComponentRemoved<'w, T: 'static>(ComponentEvent<'w, T>);
unsafe impl<T> Send for ComponentRemoved<'_, T> {}
unsafe impl<T> Sync for ComponentRemoved<'_, T> {}
impl<'w, T> Deref for ComponentRemoved<'w, T> {
    type Target = ComponentEvent<'w, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T: 'static> SystemParam for ComponentRemoved<'_, T> {
    type State = (Share<ComponentEventVec>, usize);
    type Item<'w> = ComponentRemoved<'w, T>;

    fn init_state(world: &mut World, _meta: &mut SystemMeta) -> Self::State {
        let info = ComponentInfo::of::<T>(0);
        init_removed_state(world, info)
    }

    #[inline]
    fn get_param<'world>(
        _world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        _tick: Tick,
    ) -> Self::Item<'world> {
        ComponentRemoved(ComponentEvent::new(&state.0, state.1))
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

pub struct ComponentEvent<'w, T: 'static> {
    pub(crate) record: &'w Share<ComponentEventVec>,
    pub(crate) listener_index: usize,
    _p: PhantomData<T>,
}
impl<'w, T: 'static> ComponentEvent<'w, T> {
    #[inline]
    pub(crate) fn new(record: &'w Share<ComponentEventVec>, listener_index: usize) -> Self {
        Self {
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

fn init_state<E: 'static>(world: &mut World) -> Share<EventVec<E>> {
    let info = TypeInfo::of::<Event<E>>();
    let r = world.get_event_record(&info.type_id);
    if let Some(er) = r {
        Share::downcast::<EventVec<E>>(er.into_any()).unwrap()
    } else {
        let r = Share::new(EventVec::<E>::new(info.type_name.clone()));
        world.init_event_record(info.type_id, r.clone());
        r
    }
}

fn init_changed_state(world: &mut World, info: ComponentInfo) -> (Share<ComponentEventVec>, usize) {
    let (r, c) = init_component_state(world, info, |info| match &info.changed {
        Some(r) => r.clone(),
        None => {
            let r = Share::new(ComponentEventVec::new(info.info.type_name().clone()));
            info.changed = Some(r.clone());
            r
        }
    });
    // 首次创建监听器，将所有相关原型的实体都放入到事件列表中
    if r.1 == 0 {
        c.update(&world.archetype_arr, |_, row, ar| {
            r.0.record(ar.get_unchecked(row));
        })
    }
    r
}
fn init_added_state(world: &mut World, info: ComponentInfo) -> (Share<ComponentEventVec>, usize) {
    init_component_state(world, info, |info| match &info.added {
        Some(r) => r.clone(),
        None => {
            let r = Share::new(ComponentEventVec::new(info.info.type_name().clone()));
            info.added = Some(r.clone());
            r
        }
    })
    .0
}

fn init_removed_state(world: &mut World, info: ComponentInfo) -> (Share<ComponentEventVec>, usize) {
    init_component_state(world, info, |info| match &info.removed {
        Some(r) => r.clone(),
        None => {
            let r = Share::new(ComponentEventVec::new(info.info.type_name().clone()));
            info.removed = Some(r.clone());
            r
        }
    })
    .0
}

fn init_component_state<F>(
    world: &mut World,
    info: ComponentInfo,
    get_fn: F,
) -> ((Share<ComponentEventVec>, usize), Share<Column>)
where
    F: FnOnce(&mut ColumnInfo) -> Share<ComponentEventVec>,
{
    let mut column = world.add_component_info(info).1;
    let c = unsafe { Share::get_mut_unchecked(&mut column) };
    let mut vec = get_fn(&mut c.info);
    let index = unsafe { Share::get_mut_unchecked(&mut vec) }.insert_listener();
    ((vec, index), column)
}
