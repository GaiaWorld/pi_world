//! () 为空过滤器
//! 2种原型过滤器 Without<C> With<C>
//! Or只支持多个With<C>，表示原型上只要有任何1个C就可以
//! Added Changed为迭代器，多个迭代器是或关系， 原型上只要有1个可迭代的组件就可以
//! Query<(&T, &mut C8>), (Without<C1>,With<C2>,With<C3>,Or<(With<C4>, With<C5>)>, Changed<C6>, Added<C7>)>
//!

use pi_proc_macros::all_tuples;
use smallvec::SmallVec;
use std::any::TypeId;
use std::marker::PhantomData;

use crate::archetype::Archetype;
use crate::system::ReadWrite;
use crate::world::World;

pub trait FilterArchetype {
    fn filter_archetype(_archetype: &Archetype) -> bool {
        false
    }
}
pub trait FilterComponents {
    const LISTENER_COUNT: usize;
    /// initializes ReadWrite for this [`FilterComponents`] type.
    fn init_read_write(_world: &World, _rw: &mut ReadWrite) {}
    /// initializes listener for this [`FilterComponents`] type
    fn init_listeners(_world: &World, _listeners: &mut SmallVec<[(TypeId, bool); 1]>) {}
    fn archetype_filter(_archetype: &Archetype) -> bool {
        false
    }
}

pub struct Without<T: 'static>(PhantomData<T>);
impl<T: 'static> FilterComponents for Without<T> {
    const LISTENER_COUNT: usize = 0;
    fn init_read_write(_world: &World, rw: &mut ReadWrite) {
        rw.withouts.insert(TypeId::of::<T>(), std::any::type_name::<T>().into());
    }
    fn archetype_filter(archetype: &Archetype) -> bool {
        archetype.get_column(&TypeId::of::<T>()).is_some()
    }
}

pub struct With<T: 'static>(PhantomData<T>);
impl<T: 'static> FilterArchetype for With<T> {
    fn filter_archetype(archetype: &Archetype) -> bool {
        Self::archetype_filter(archetype)
    }
}
impl<T: 'static> FilterComponents for With<T> {
    const LISTENER_COUNT: usize = 0;
    fn init_read_write(_world: &World, rw: &mut ReadWrite) {
        rw.withs.insert(TypeId::of::<T>(), std::any::type_name::<T>().into());
    }
    fn archetype_filter(archetype: &Archetype) -> bool {
        archetype.get_column(&TypeId::of::<T>()).is_none()
    }
}

pub struct Added<T: 'static>(PhantomData<T>);
impl<T: 'static> FilterComponents for Added<T> {
    const LISTENER_COUNT: usize = 1;
    fn init_listeners(_world: &World, listeners: &mut SmallVec<[(TypeId, bool); 1]>) {
        listeners.push((TypeId::of::<T>(), false));
    }
}

pub struct Changed<T: 'static>(PhantomData<T>);
impl<T: 'static> FilterComponents for Changed<T> {
    const LISTENER_COUNT: usize = 1;
    fn init_listeners(_world: &World, listeners: &mut SmallVec<[(TypeId, bool); 1]>) {
        listeners.push((TypeId::of::<T>(), true));
    }
}

macro_rules! impl_tuple_filter {
    ($(($name: ident, $state: ident)),*) => {
        #[allow(non_snake_case)]
        #[allow(clippy::unused_unit)]

        impl<$($name: FilterComponents),*> FilterComponents for ($($name,)*) {
            const LISTENER_COUNT: usize = $($name::LISTENER_COUNT + )* 0;
	        fn init_read_write(_world: &World, _rw: &mut ReadWrite) {
                ($($name::init_read_write(_world, _rw),)*);
            }
            fn init_listeners(_world: &World, _listeners: &mut SmallVec<[(TypeId, bool); 1]>) {
                ($($name::init_listeners(_world, _listeners),)*);
            }
            fn archetype_filter(_archetype: &Archetype) -> bool {
                $(
                    if $name::archetype_filter(_archetype){return true};
                )*
                false
            }
        }

    };
}
all_tuples!(impl_tuple_filter, 0, 15, F, S);

pub struct Or<T: 'static + FilterArchetype>(PhantomData<T>);
impl<T: 'static + FilterArchetype> FilterComponents for Or<T> {
    const LISTENER_COUNT: usize = 0;
    fn archetype_filter(archetype: &Archetype) -> bool {
        T::filter_archetype(archetype)
    }
}

macro_rules! impl_or_tuple_fetch {
    ($($name: ident),*) => {
        #[allow(non_snake_case)]
        #[allow(clippy::unused_unit)]
        impl<$($name: FilterArchetype),*> FilterArchetype for ($($name,)*) {

            fn filter_archetype(_archetype: &Archetype) -> bool {
                $(
                    if !$name::filter_archetype(_archetype){return false};
                )*
                true
            }
        }

    };
}
all_tuples!(impl_or_tuple_fetch, 1, 15, F);
