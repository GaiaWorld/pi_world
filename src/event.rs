//! 事件，及组件移除
//!
use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::marker::PhantomData;
use std::mem::{size_of, transmute};
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
    pub fn capacity(&self) -> usize {
        self.listeners.capacity() * 8 + self.vec.capacity() * size_of::<E>()
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

    /// 标记为已读
    pub(crate) fn mark_read(&self, listener_index: usize) {
        let len = self.vec.len();
        if len > 0 {
            let read_len = unsafe { self.listeners.get_unchecked(listener_index) };
            read_len.store(len, std::sync::atomic::Ordering::Relaxed);
        }
    }
    /// 获得指定监听者的读取长度
    pub(crate) fn get_iter(&self, listener_index: usize) -> SafeVecIter<'_, E> {
        let end = self.vec.len();
        // 从上次读取到的位置开始读取
        let read_len = unsafe { self.listeners.get_unchecked(listener_index) };
        let start = read_len.swap(end, Ordering::Relaxed);
        self.vec.slice(start..end)
    }
    /// 判断是否能够清空事件列表， 如果所有的监听器都读取了全部的事件列表，才可以清空事件列表， 返回Ok(len)表示可以清空，事件列表长度为len，返回Err((len, index))表示不能清空，len表示事件列表的长度，index表示监听器的最小读取长度，即index之前的监听器已经读取完毕，index及之后的监听器还未读取完毕
    pub(crate) fn can_clear(&mut self) -> Result<usize, (usize, usize)> {
        let len = self.vec.len();
        if len == 0 {
            return Ok(0);
        }
        let mut min = 0;
        for read_len in self.listeners.iter_mut() {
            min = min.max(*read_len.get_mut());
        }
        if min < len {
            return Err((len, min));
        }
        // 只有所有的监听器都读取了全部的事件列表，才可以清空事件列表
        Ok(len)
    }
    /// 清理方法
    pub(crate) fn clear(&mut self) {
        self.vec.clear(0);
        for read_len in self.listeners.iter_mut() {
            *read_len.get_mut() = 0;
        }
    }
    /// 清理部分已读的事件列表
    pub(crate) fn clear_part(&mut self, index: usize) {
        self.vec.remain_settle(index..usize::MAX, 0);
        if index == 0 {
            return;
        }
        for read_len in self.listeners.iter_mut() {
            *read_len.get_mut() -= index;
        }
    }
    // 整理方法， 返回是否已经将事件列表清空，只有所有的监听器都读取了全部的事件列表，才可以清空事件列表
    pub(crate) fn settle(&mut self) -> bool {
        match self.can_clear() {
            Ok(len) => {
                if len > 0 {
                    self.clear();
                }
                true
            }
            Err((len, index)) => {
                if len.saturating_add(len) > self.vec.vec_capacity() {
                    // 如果事件列表的数据大于事件列表内的快速槽位的一半，则清理部分事件列表
                    self.clear_part(index);
                }
                false
            },
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

pub type EventReader<'w, E> = &'w mut Event<'w, E>;

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

    pub fn mark_read(&self) {
        self.record.mark_read(self.listener_index);
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
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        Event::new(&state.0, state.1)
    }
    #[inline]
    fn get_self<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, state)) }
    }
}

pub type EventWriter<'w, E> = &'w mut EventSender<'w, E>;

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
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        EventSender(state)
    }
    #[inline]
    fn get_self<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, state)) }
    }
}

pub type ComponentChanged<'w, T> = &'w mut ComponentChangedInner<'w, T>;
pub struct ComponentChangedInner<'w, T: 'static>(ComponentEvent<'w, T>);
unsafe impl<T> Send for ComponentChangedInner<'_, T> {}
unsafe impl<T> Sync for ComponentChangedInner<'_, T> {}
impl<'w, T> Deref for ComponentChangedInner<'w, T> {
    type Target = ComponentEvent<'w, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: 'static> SystemParam for ComponentChangedInner<'_, T> {
    type State = (Share<ComponentEventVec>, usize);
    type Item<'w> = ComponentChangedInner<'w, T>;

    fn init_state(world: &mut World, _meta: &mut SystemMeta) -> Self::State {
        let info = ComponentInfo::of::<T>(COMPONENT_TICK);
        init_changed_state(world, TypeId::of::<ComponentChangedInner<'static, T>>(), info)
    }

    #[inline]
    fn get_param<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        ComponentChangedInner(ComponentEvent::new(&state.0, state.1))
    }
    #[inline]
    fn get_self<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, state)) }
    }
}

pub type ComponentAdded<'w, T> = &'w mut ComponentAddedInner<'w, T>;
pub struct ComponentAddedInner<'w, T: 'static>(ComponentEvent<'w, T>);
unsafe impl<T> Send for ComponentAddedInner<'_, T> {}
unsafe impl<T> Sync for ComponentAddedInner<'_, T> {}
impl<'w, T> Deref for ComponentAddedInner<'w, T> {
    type Target = ComponentEvent<'w, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: 'static> SystemParam for ComponentAddedInner<'_, T> {
    type State = (Share<ComponentEventVec>, usize);
    type Item<'w> = ComponentAddedInner<'w, T>;

    fn init_state(world: &mut World, _meta: &mut SystemMeta) -> Self::State {
        let info = ComponentInfo::of::<T>(0);
        init_added_state(world, TypeId::of::<ComponentAddedInner<'static, T>>(), info)
    }

    #[inline]
    fn get_param<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        ComponentAddedInner(ComponentEvent::new(&state.0, state.1))
    }
    #[inline]
    fn get_self<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, state)) }
    }
}

pub type ComponentRemoved<'w, T> = &'w mut ComponentRemovedInner<'w, T>;
pub struct ComponentRemovedInner<'w, T: 'static>(ComponentEvent<'w, T>);
unsafe impl<T> Send for ComponentRemovedInner<'_, T> {}
unsafe impl<T> Sync for ComponentRemovedInner<'_, T> {}
impl<'w, T> Deref for ComponentRemovedInner<'w, T> {
    type Target = ComponentEvent<'w, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T: 'static> SystemParam for ComponentRemovedInner<'_, T> {
    type State = (Share<ComponentEventVec>, usize);
    type Item<'w> = ComponentRemovedInner<'w, T>;

    fn init_state(world: &mut World, _meta: &mut SystemMeta) -> Self::State {
        let info = ComponentInfo::of::<T>(0);
        init_removed_state(world, TypeId::of::<ComponentRemovedInner<'static, T>>(), info)
    }

    #[inline]
    fn get_param<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        ComponentRemovedInner(ComponentEvent::new(&state.0, state.1))
    }
    #[inline]
    fn get_self<'world>(
        world: &'world World,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, state)) }
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
    pub fn capacity(&self) -> usize {
        self.record.capacity()
    }
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.record.len(self.listener_index)
    }
    pub fn iter(&self) -> SafeVecIter<'_, Entity> {
        self.record.get_iter(self.listener_index)
    }
    /// 标记为已读
    pub fn mark_read(&self) {
        self.record.mark_read(self.listener_index);
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

fn init_changed_state(world: &mut World, typeid: TypeId, info: ComponentInfo) -> (Share<ComponentEventVec>, usize) {
    let (r, c) = init_component_state(world, info, |info| match &info.changed {
        Some(r) => r.clone(),
        None => {
            let r = Share::new(ComponentEventVec::new(info.info.type_name().clone()));
            info.changed = Some(r.clone());
            r
        }
    });
    world.init_event_record(typeid, r.0.clone());
    // 首次创建监听器，将所有相关原型的实体都放入到事件列表中
    if r.1 == 0 {
        c.update(&world.archetype_arr, |_, row, ar| {
            r.0.record(ar.get_unchecked(row));
        })
    }
    r
}
fn init_added_state(world: &mut World, typeid: TypeId, info: ComponentInfo) -> (Share<ComponentEventVec>, usize) {
    let r = init_component_state(world, info, |info| match &info.added {
        Some(r) => r.clone(),
        None => {
            let r = Share::new(ComponentEventVec::new(info.info.type_name().clone()));
            info.added = Some(r.clone());
            r
        }
    })
    .0;
    world.init_event_record(typeid, r.0.clone());
    r
}

fn init_removed_state(world: &mut World, typeid: TypeId, info: ComponentInfo) -> (Share<ComponentEventVec>, usize) {
    let r = init_component_state(world, info, |info| match &info.removed {
        Some(r) => r.clone(),
        None => {
            let r = Share::new(ComponentEventVec::new(info.info.type_name().clone()));
            info.removed = Some(r.clone());
            r
        }
    })
    .0;
    world.init_event_record(typeid, r.0.clone());
    r
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
