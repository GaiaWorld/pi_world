/// 容器线程安全的监听器，每个监听器可能被多个线程通知，需要自己保证线程安全
/// 容器销毁时，监听器也会被销毁，或移除监听器并销毁，如果这个时候还有事件通知，则也需要监听器自己保证安全
use core::fmt::*;
use std::{
    any::TypeId,
    mem::{forget, transmute},
};

use dashmap::{mapref::entry::Entry, DashMap};
use pi_null::Null;
use pi_share::Share;

use crate::safe_vec::SafeVec;

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
    listener_list: SafeVec<Element>,
    listener_map: DashMap<TypeId, ListenerListKey>,
    event_list: SafeVec<Element>,
    event_map: DashMap<TypeId, EventListKey>,
}
impl ListenerMgr {
    pub fn init_register_listener<L: Listener<Event = E> + 'static, E: Clone>(
        &self,
    ) -> ListenerListKey {
        let entry = self.listener_map.entry(TypeId::of::<L>());
        match entry {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => {
                let list: ListenerList<L, E> = ListenerList::new();
                let ptr = Box::<ListenerList<L, E>>::into_raw(Box::new(list)) as *mut u8;
                let el = Element {
                    ptr,
                    drop_fn: get_drop::<ListenerList<L, E>>(),
                };
                let llk = ListenerListKey(self.listener_list.insert(el));
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
                let list: Box<ListenerList<L, E>> = unsafe {
                    let el = self.listener_list.get_unchecked(llk.0);
                    Box::from_raw(transmute(el.ptr))
                };
                list.vec.insert(listener);
                forget(list);
                llk
            }
            Entry::Vacant(e) => {
                let list = ListenerList::default();
                list.vec.insert(listener);
                let ptr = Box::<ListenerList<L, E>>::into_raw(Box::new(list)) as *mut u8;
                let el = Element {
                    ptr,
                    drop_fn: get_drop::<ListenerList<L, E>>(),
                };
                let llk = ListenerListKey(self.listener_list.insert(el));
                e.insert(llk);
                llk
            }
        }
    }

    pub fn init_register_event<E: 'static + Clone>(&self) -> EventListKey {
        let entry = self.event_map.entry(TypeId::of::<E>());
        match entry {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => {
                let list: EventList<E> = EventList::new();
                let ptr = Box::into_raw(Box::new(list)) as *mut u8;
                let el = Element {
                    ptr,
                    drop_fn: get_drop::<EventList<E>>(),
                };
                let elk = EventListKey(self.event_list.insert(el));
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
                let list: Box<EventList<E>> = unsafe {
                    let el = self.event_list.get_unchecked(elk.0);
                    Box::from_raw(transmute(el.ptr))
                };
                list.vec.insert(listener);
                forget(list);
                elk
            }
            Entry::Vacant(e) => {
                let list = EventList::new();
                list.vec.insert(listener);
                let ptr = Box::<EventList<E>>::into_raw(Box::new(list)) as *mut u8;
                let el = Element {
                    ptr,
                    drop_fn: get_drop::<EventList<E>>(),
                };
                let elk = EventListKey(self.event_list.insert(el));
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
    pub fn notify_listener<L: Listener<Event = E>, E: Clone>(
        &self,
        key: ListenerListKey,
        event: E,
    ) {
        let list: &ListenerList<L, E> =
            unsafe { transmute(self.listener_list.get(key.0).unwrap().ptr) };
        list.notify(event);
    }
    pub fn notify_event<E: Clone>(&self, key: EventListKey, event: E) {
        let list: &EventList<E> = unsafe { transmute(self.event_list.get(key.0).unwrap().ptr) };
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
    pub fn settle(&mut self) {
        self.listener_list.settle();
        self.event_list.settle();
    }
}

pub struct ListenerList<L: Listener<Event = E>, E> {
    vec: SafeVec<L>,
}
impl<L: Listener<Event = E>, E: Clone> ListenerList<L, E> {
    pub fn new() -> Self {
        Self {
            vec: SafeVec::default(),
        }
    }
    pub fn register(&self, listener: L) {
        self.vec.insert(listener);
    }
    pub fn notify(&self, event: E) {
        for l in self.vec.iter() {
            l.listen(event.clone());
        }
    }
    pub fn settle(&mut self) {
        self.vec.settle();
    }
}
impl<L: Listener<Event = E>, E: Clone> Default for ListenerList<L, E> {
    fn default() -> Self {
        ListenerList {
            vec: Default::default(),
        }
    }
}
#[derive(Default)]
pub struct EventList<E> {
    vec: SafeVec<Share<dyn Listener<Event = E>>>,
}
impl<E: Clone> EventList<E> {
    pub fn new() -> Self {
        Self {
            vec: SafeVec::default(),
        }
    }
    pub fn register(&self, listener: Share<dyn Listener<Event = E>>) {
        self.vec.insert(listener);
    }
    pub fn notify(&self, event: E) {
        for l in self.vec.iter() {
            l.listen(event.clone());
        }
    }
    pub fn settle(&mut self) {
        self.vec.settle();
    }
}

#[derive(Debug)]
struct Element {
    ptr: *mut u8,
    drop_fn: fn(*mut u8),
}
impl Drop for Element {
    fn drop(&mut self) {
        (self.drop_fn)(self.ptr);
    }
}

/// 获得指定类型的释放函数
pub fn get_drop<T>() -> fn(*mut u8) {
    |ptr: *mut u8| unsafe {
        let _ = Box::from_raw(ptr as *mut T);
    }
}

#[cfg(test)]
mod test_mod {
    use crate::{
        archetype::{Archetype, ShareArchetype},
        listener::*,
    };
    struct A();
    impl Drop for A {
        fn drop(&mut self) {
            println!("A drop");
        }
    }
    #[test]
    fn test() {
        {
            let a = Box::<A>::into_raw(Box::new(A())) as *mut u8;
            let e = Element {
                ptr: a,
                drop_fn: get_drop::<A>(),
            };
            println!("e:{:?}", e)
        }
        let s = Box::new("ass".to_string());
        let ptr = Box::into_raw(s);
        let pp = ptr as *mut String;
        let ppp = unsafe { &mut *pp };
        println!("asdf:{:?}", ppp);
        let listener_mgr = ListenerMgr::default();
        let archetype_init_key = listener_mgr.init_register_event::<ShareArchetype>();
        let ar = Share::new(Archetype::new(Default::default()));
        listener_mgr.notify_event(archetype_init_key, ar);
    }
}
