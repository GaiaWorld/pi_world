//! () 为空过滤器，表示不做过滤
//! Empty表示取World的空原型
//! 2种原型过滤器 Without<C> With<C>
//! Or只支持多个With<C>，表示原型上只要有任何1个C就可以
//! Changed Removed Destroyed为迭代器，多个迭代器是或关系， 原型上只要有1个可迭代的组件就可以
//! Query<(&T, &mut C8>), (Without<C1>,With<C2>,With<C3>,Or<(With<C4>, With<C5>)>, Changed<C6>, Destroyed, Removed<C8>)>
//!

use pi_null::Null;
use pi_proc_macros::all_tuples;
use smallvec::SmallVec;
use std::any::TypeId;
use std::marker::PhantomData;

use crate::archetype::{Archetype, ComponentInfo, COMPONENT_CHANGED, COMPONENT_TICK};
use crate::system::SystemMeta;
use crate::world::{ComponentIndex, World};

#[derive(Default, Clone, Copy, Debug)]
pub enum ListenType {
    #[default]
    Destroyed, // 实体销毁，用列表来记录变化
    Changed(ComponentIndex), // 组件改变，包括新增，用列表来记录变化
    // Removed(ComponentIndex), // 组件删除，用列表来记录变化
}

pub trait FilterArchetype {
    fn filter_archetype(_world: &World, _archetype: &Archetype) -> bool {
        false
    }
}
pub trait FilterComponents {
    const LISTENER_COUNT: usize;
    /// initializes ReadWrite for this [`FilterComponents`] type.
    fn init_read_write(_world: &mut World, _meta: &mut SystemMeta) {}
    /// initializes listener for this [`FilterComponents`] type
    fn init_listeners(_world: &mut World, _listeners: &mut SmallVec<[ListenType; 1]>) {}
    fn archetype_filter(_world: &World, _archetype: &Archetype) -> bool {
        false
    }
}

/// Empty表示取World的空原型
pub struct Empty;
impl FilterComponents for Empty {
    const LISTENER_COUNT: usize = 0;
    fn archetype_filter(_world: &World, archetype: &Archetype) -> bool {
        archetype.id() != &0
    }
}

pub struct Without<T: 'static>(PhantomData<T>);
impl<T: 'static> FilterComponents for Without<T> {
    const LISTENER_COUNT: usize = 0;
    fn init_read_write(world: &mut World, meta: &mut SystemMeta) {
        world.add_component_info(ComponentInfo::of::<T>(0));
        meta.cur_param.withouts.insert(TypeId::of::<T>(), std::any::type_name::<T>().into());
    }
    fn archetype_filter(world: &World, archetype: &Archetype) -> bool {
        !archetype.get_column_index_by_tid(world, &TypeId::of::<T>()).is_null()
    }
}

pub struct With<T: 'static>(PhantomData<T>);
impl<T: 'static> FilterArchetype for With<T> {
    fn filter_archetype(world: &World, archetype: &Archetype) -> bool {
        Self::archetype_filter(world, archetype)
    }
}
impl<T: 'static> FilterComponents for With<T> {
    const LISTENER_COUNT: usize = 0;
    fn init_read_write(world: &mut World, meta: &mut SystemMeta) {
        world.add_component_info(ComponentInfo::of::<T>(0));
        meta.cur_param.withs.insert(TypeId::of::<T>(), std::any::type_name::<T>().into());
    }
    fn archetype_filter(world: &World, archetype: &Archetype) -> bool {
        archetype.get_column_index_by_tid(world, &TypeId::of::<T>()).is_null()
    }
}

pub struct Changed<T: 'static>(PhantomData<T>);
impl<T: 'static> FilterComponents for Changed<T> {
    const LISTENER_COUNT: usize = 1;
    fn init_listeners(world: &mut World, listeners: &mut SmallVec<[ListenType; 1]>) {
        listeners.push(ListenType::Changed(world.add_component_info(ComponentInfo::of::<T>(COMPONENT_TICK | COMPONENT_CHANGED)).0));
    }
}

// pub struct Removed<T: 'static>(PhantomData<T>);
// impl<T: 'static> FilterComponents for Removed<T> {
//     const LISTENER_COUNT: usize = 1;
//     fn init_listeners(world: &mut World, listeners: &mut SmallVec<[ListenType; 1]>) {
//         listeners.push(ListenType::Removed(world.add_component_info(ComponentInfo::of::<T>(COMPONENT_REMOVED)).0));
//     }
// }
pub struct Destroyed;
impl FilterComponents for Destroyed {
    const LISTENER_COUNT: usize = 1;
    fn init_listeners(_world: &mut World, listeners: &mut SmallVec<[ListenType; 1]>) {
        listeners.push(ListenType::Destroyed);
    }
}
macro_rules! impl_tuple_filter {
    ($(($name: ident, $state: ident)),*) => {
        #[allow(non_snake_case)]
        #[allow(clippy::unused_unit)]

        impl<$($name: FilterComponents),*> FilterComponents for ($($name,)*) {
            const LISTENER_COUNT: usize = $($name::LISTENER_COUNT + )* 0;
	        fn init_read_write(_world: &mut World, _meta: &mut SystemMeta) {
                ($($name::init_read_write(_world, _meta),)*);
            }
            fn init_listeners(_world: &mut World, _listeners: &mut SmallVec<[ListenType; 1]>) {
                ($($name::init_listeners(_world, _listeners),)*);
            }
            fn archetype_filter(_world: &World, _archetype: &Archetype) -> bool {
                $(
                    if $name::archetype_filter(_world, _archetype){return true};
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
    fn archetype_filter(world: &World, archetype: &Archetype) -> bool {
        T::filter_archetype(world, archetype)
    }
}

macro_rules! impl_or_tuple_fetch {
    ($($name: ident),*) => {
        #[allow(non_snake_case)]
        #[allow(clippy::unused_unit)]
        impl<$($name: FilterArchetype),*> FilterArchetype for ($($name,)*) {

            fn filter_archetype(_world: &World, _archetype: &Archetype) -> bool {
                $(
                    if !$name::filter_archetype(_world, _archetype){return false};
                )*
                true
            }
        }

    };
}
all_tuples!(impl_or_tuple_fetch, 1, 15, F);
