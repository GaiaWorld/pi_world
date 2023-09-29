/// 容器线程安全的监听器，每个监听器可能被多个线程通知，需要自己保证线程安全
/// 容器销毁时，监听器也会被销毁，或移除监听器并销毁，如果这个时候还有事件通知，则也需要监听器自己保证安全
use core::fmt::*;
use std::{any::TypeId, mem::transmute};

use dashmap::{mapref::entry::Entry, DashMap};
use pi_key_alloter::new_key_type;
use pi_null::Null;
use pi_slot::*;

new_key_type! {
    pub struct ListenerKey;
}
new_key_type! {
    pub struct EventListenerKey;
}
new_key_type! {
    pub struct ListenerListKey;
}
new_key_type! {
    pub struct EventListKey;
}
pub trait Listener {
    type Event: Send + Sync + 'static;
    fn listen(&self, args: &Self::Event);
}

pub struct ListenerMgr {
    listener_list: SlotMap<ListenerListKey, *const ()>,
    listener_map: DashMap<TypeId, ListenerListKey>,
    event_list: SlotMap<EventListKey, *const ()>,
    event_map: DashMap<TypeId, EventListKey>,
}
impl ListenerMgr {
    pub fn register_listener<L: Listener<Event = E> + 'static, E>(
        &self,
        listener: L,
    ) -> (ListenerListKey, ListenerKey) {
        let entry = self.listener_map.entry(TypeId::of::<L>());
        match entry {
            Entry::Occupied(e) => {
                let llk = *e.get();
                let list: &ListenerList<L, E> =
                    unsafe { transmute(self.listener_list.get_unchecked(llk)) };
                let lk = list.map.insert(listener);
                (llk, lk)
            }
            Entry::Vacant(e) => {
                let list = ListenerList::new();
                let lk = list.map.insert(listener);
                let ptr = Box::<ListenerList<L, E>>::into_raw(Box::new(list)) as *const ();
                let llk = self.listener_list.insert(ptr);
                e.insert(llk);
                (llk, lk)
            }
        }
    }
    // pub fn unregister_listener<L: Listener<Event=E>, E: Send + Sync + 'static>(&self, listener: ListenerKey) {
    //     todo!()
    // }
    pub fn register_event<E: Send + Sync + 'static>(
        &self,
        listener: Box<dyn Listener<Event = E>>,
    ) -> (EventListKey, EventListenerKey) {
        let entry = self.event_map.entry(TypeId::of::<E>());
        match entry {
            Entry::Occupied(e) => {
                let llk = *e.get();
                let list: &EventList<E> = unsafe { transmute(self.event_list.get_unchecked(llk)) };
                let lk = list.map.insert(listener);
                (llk, lk)
            }
            Entry::Vacant(e) => {
                let list = EventList::new();
                let lk = list.map.insert(listener);
                let ptr = Box::<EventList<E>>::into_raw(Box::new(list)) as *const ();
                let llk = self.event_list.insert(ptr);
                e.insert(llk);
                (llk, lk)
            }
        }
    }
    // pub fn unregister_event<E: Send + Sync + 'static>(&self, listener: EventListenerKey) {
    //     todo!()
    // }
    pub fn get_listener_list_key<L: Listener<Event = E> + 'static, E>(&self) -> ListenerListKey {
        match self.listener_map.get(&TypeId::of::<L>()) {
            Some(k) => *k,
            None => ListenerListKey::null(),
        }
    }
    pub fn get_event_list_key<E: Send + Sync + 'static>(&self) -> EventListKey {
        match self.event_map.get(&TypeId::of::<E>()) {
            Some(k) => *k,
            None => EventListKey::null(),
        }
    }
    pub fn notify_listener<L: Listener<Event = E>, E>(&self, key: ListenerListKey, event: &E) {
        let list: &ListenerList<L, E> = unsafe { transmute(self.listener_list.get(key).unwrap()) };
        list.notify(event);
    }
    pub fn notify_event<E: Send + Sync + 'static>(&self, key: EventListKey, event: &E) {
        let list: &EventList<E> = unsafe { transmute(self.event_list.get(key).unwrap()) };
        list.notify(event);
    }
    pub fn notify_listener_by_type<L: Listener<Event = E> + 'static, E>(&self, event: &E) {
        let k = self.get_listener_list_key::<L, E>();
        self.notify_listener::<L, E>(k, event);
    }
    pub fn notify_event_by_type<E: Send + Sync + 'static>(&self, event: &E) {
        let k = self.get_event_list_key::<E>();
        self.notify_event(k, event);
    }
}
#[derive(Default)]
pub struct ListenerList<L: Listener<Event = E>, E> {
    map: SlotMap<ListenerKey, L>,
}
impl<L: Listener<Event = E>, E> ListenerList<L, E> {
    fn new() -> Self {
        Self {
            map: SlotMap::with_capacity(0),
        }
    }
    fn notify(&self, event: &E) {
        for (_, listener) in self.map.iter() {
            listener.listen(event)
        }
    }
}
pub struct EventList<E: Send + Sync + 'static> {
    map: SlotMap<EventListenerKey, Box<dyn Listener<Event = E>>>,
}
impl<E: Send + Sync + 'static> EventList<E> {
    fn new() -> Self {
        Self {
            map: SlotMap::with_capacity(0),
        }
    }
    fn notify(&self, event: &E) {
        for (_, listener) in self.map.iter() {
            listener.listen(event)
        }
    }
}
