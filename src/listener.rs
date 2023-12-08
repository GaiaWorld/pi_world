/// 容器线程安全的监听器，每个监听器可能被多个线程通知，需要自己保证线程安全
/// 容器销毁时，监听器也会被销毁，或移除监听器并销毁，如果这个时候还有事件通知，则也需要监听器自己保证安全
use core::fmt::*;
use std::{
    any::TypeId,
    mem::{self, transmute},
};

use dashmap::{mapref::entry::Entry, DashMap};
use pi_append_vec::*;
use pi_null::Null;
use pi_share::Share;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct ListenerListKey(usize);

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct EventListKey(usize);

pub trait Listener {
    type Event: Clone;
    fn listen(&self, args: Self::Event);
}

#[derive(Default, Debug)]
pub struct ListenerMgr {
    listener_list: AppendVec<*const ()>,
    listener_map: DashMap<TypeId, ListenerListKey>,
    event_list: AppendVec<*const ()>,
    event_map: DashMap<TypeId, EventListKey>,
}
impl ListenerMgr {
    pub fn init_register_listener<L: Listener<Event = E> + 'static, E: Clone>(
        &self,
    ) -> ListenerListKey {
        let entry = self.listener_map.entry(TypeId::of::<L>());
        match entry {
            Entry::Occupied(e) => {
                *e.get()
            }
            Entry::Vacant(e) => {
                let list = ListenerList::new();
                let ptr = Box::<ListenerList<L, E>>::into_raw(Box::new(list)) as *const ();
                let llk = ListenerListKey(self.listener_list.insert(ptr));
                e.insert(llk);
                llk
            }
        }
    }
    pub fn register_listener<L: Listener<Event = E> + 'static, E: Clone>(
        &self,
        listener: L,
    ) -> ListenerListKey {
        let entry = self.listener_map.entry(TypeId::of::<L>());
        match entry {
            Entry::Occupied(e) => {
                let llk = *e.get();
                let list: Box<ListenerList<L, E>> =
                    unsafe { Box::from_raw(transmute(*self.listener_list.get_unchecked(llk.0))) };
                list.vec.insert(Some(listener));
                mem::forget(list);
                llk
            }
            Entry::Vacant(e) => {
                let list = ListenerList::new();
                list.vec.insert(Some(listener));
                let ptr = Box::<ListenerList<L, E>>::into_raw(Box::new(list)) as *const ();
                let llk = ListenerListKey(self.listener_list.insert(ptr));
                e.insert(llk);
                llk
            }
        }
    }
    
    pub fn init_register_event<E: 'static + Clone>(
        &self,
    ) -> EventListKey {
        let entry = self.event_map.entry(TypeId::of::<E>());
        match entry {
            Entry::Occupied(e) => {
                *e.get()
            }
            Entry::Vacant(e) => {
                let list: EventList<E> = EventList::new();
                let ptr = Box::into_raw(Box::new(list)) as *const ();
                let elk = EventListKey(self.event_list.insert(ptr));
                e.insert(elk);
                elk
            }
        }
    }
    pub fn register_event<E: 'static + Clone>(
        &self,
        listener: Share<dyn Listener<Event = E>>,
    ) -> EventListKey {
        let entry = self.event_map.entry(TypeId::of::<E>());
        match entry {
            Entry::Occupied(e) => {
                let elk = *e.get();
                let list: Box<EventList<E>> =
                    unsafe { Box::from_raw(transmute(*self.event_list.get_unchecked(elk.0))) };
                list.vec.insert(Some(listener));
                mem::forget(list);
                elk
            }
            Entry::Vacant(e) => {
                let list = EventList::new();
                list.vec.insert(Some(listener));
                let ptr = Box::<EventList<E>>::into_raw(Box::new(list)) as *const ();
                let elk = EventListKey(self.event_list.insert(ptr));
                e.insert(elk);
                elk
            }
        }
    }
    pub fn get_listener_list_key<L: Listener<Event = E> + 'static, E>(&self) -> ListenerListKey {
        match self.listener_map.get(&TypeId::of::<L>()) {
            Some(k) => *k,
            None => ListenerListKey(usize::null()),
        }
    }
    pub fn get_event_list_key<E: 'static>(&self) -> EventListKey {
        match self.event_map.get(&TypeId::of::<E>()) {
            Some(k) => *k,
            None => EventListKey(usize::null()),
        }
    }
    pub fn notify_listener<L: Listener<Event = E>, E: Clone>(&self, key: ListenerListKey, event: E) {
        let list: &ListenerList<L, E> =
            unsafe { transmute(*self.listener_list.get(key.0).unwrap()) };
        list.notify(event);
    }
    pub fn notify_event<E: Clone>(&self, key: EventListKey, event: E) {
        let list: &EventList<E> = unsafe { transmute(*self.event_list.get(key.0).unwrap()) };
        list.notify(event);
    }
    pub fn notify_listener_by_type<L: Listener<Event = E> + 'static, E: Clone>(&self, event: E) {
        let k = self.get_listener_list_key::<L, E>();
        self.notify_listener::<L, E>(k, event);
    }
    pub fn notify_event_by_type<E: 'static>(&self, event: &E) {
        let k = self.get_event_list_key::<E>();
        self.notify_event(k, event);
    }
}
#[derive(Default)]
pub struct ListenerList<L: Listener<Event = E>, E> {
    vec: AppendVec<Option<L>>,
}
impl<L: Listener<Event = E>, E: Clone> ListenerList<L, E> {
    fn new() -> Self {
        Self {
            vec: AppendVec::with_capacity(0),
        }
    }
    fn notify(&self, event: E) {
        for (_, listener) in self.vec.iter() {
            listener.as_ref().unwrap().listen(event.clone())
        }
    }
}
#[derive(Default)]
pub struct EventList<E> {
    vec: AppendVec<Option<Share<dyn Listener<Event = E>>>>,
}
impl<E: Clone> EventList<E> {
    fn new() -> Self {
        Self {
            vec: AppendVec::with_capacity(0),
        }
    }
    fn notify(&self, event: E) {
        for (_, listener) in self.vec.iter() {
            listener.as_ref().unwrap().listen(event.clone())
        }
    }
}



#[cfg(test)]
mod test_mod {
    use crate::{listener::*, archetype::{ShareArchetype, Archetype, ComponentInfo}};

    #[test]
    fn test() {
        let listener_mgr = ListenerMgr::default();
        let archetype_init_key = listener_mgr.init_register_event::<ShareArchetype>();
        let ar = Share::new(Archetype::new(vec![ComponentInfo::of::<ShareArchetype>()]));
        listener_mgr.notify_event(archetype_init_key, ar);
    }

}
