//! () 为空过滤器，表示不做过滤
//! Empty表示取World的空原型
//! 2种原型过滤器 Without<C> With<C>
//! Or只支持多个With<C>，表示原型上只要有任何1个C就可以
//! Changed Removed Destroyed为迭代器，多个迭代器是或关系， 原型上只要有1个可迭代的组件就可以
//! Query<(&T, &mut C8>), (Without<C1>,With<C2>,With<C3>,Or<(With<C4>, With<C5>)>, Changed<C6>, Destroyed, Removed<C8>)>
//!

use pi_proc_macros::all_tuples;
use pi_share::Share;
use std::marker::PhantomData;

use crate::archetype::{Archetype, ComponentInfo, Row, COMPONENT_TICK};
use crate::column::{BlobRef, Column};
use crate::prelude::{Entity, Tick};
use crate::system::SystemMeta;
use crate::world::{ComponentIndex, World};

// #[derive(Default, Clone, Copy, Debug)]
// pub enum ListenType {
//     #[default]
//     Destroyed, // 实体销毁，用列表来记录变化
//     Changed(ComponentIndex), // 组件改变，包括新增，用列表来记录变化
//     // Removed(ComponentIndex), // 组件删除，用列表来记录变化
// }

// pub trait FilterArchetype {
//     fn filter_archetype(_world: &World, _archetype: &Archetype) -> bool {
//         false
//     }
// }
pub trait FilterComponents {
    // const LISTENER_COUNT: usize;
    type Filter<'w>;
    type State: Send + Sync + Sized;
    /// initializes ReadWrite for this [`FilterComponents`] type.
    fn init_state(_world: &mut World, _meta: &mut SystemMeta) -> Self::State;

    // fn filter_archetype(_world: &World, _state: &Self::State, _archetype: &Archetype) -> bool {
    //     false
    // }
    // /// initializes listener for this [`FilterComponents`] type
    // fn init_listeners(_world: &mut World, _listeners: &mut Vec<ComponentIndex>) {}
    fn init_filter<'w>(
        world: &'w World,
        state: &'w Self::State,
        archetype: &'w Archetype,
        tick: Tick,
        last_run: Tick,
    ) -> Self::Filter<'w>;

    fn filter<'w>(_fetch: &Self::Filter<'w>, _row: Row, _e: Entity) -> bool {
        true
    }
}

// /// Empty表示取World的空原型
// pub struct Empty;
// impl FilterComponents for Empty {
//     const LISTENER_COUNT: usize = 0;
//     fn archetype_filter(_world: &World, archetype: &Archetype) -> bool {
//         archetype.id() != &0
//     }
// }

pub struct Without<T: 'static>(PhantomData<T>);
impl<T: 'static> FilterComponents for Without<T> {
    // const LISTENER_COUNT: usize = 0;
    type Filter<'w> = ();
    type State = ComponentIndex;
    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        meta.component_relate(
            world,
            ComponentInfo::of::<T>(0),
            crate::system::Relation::Without(0usize.into()),
        )
        .0
    }
    // fn filter_archetype(_world: &World, state: &Self::State, archetype: &Archetype) -> bool {
    //     archetype.contains(*state)
    // }
    #[inline]
    fn init_filter<'w>(
        _world: &'w World,
        _state: &'w Self::State,
        _archetype: &'w Archetype,
        _tick: Tick,
        _last_run: Tick,
    ) -> Self::Filter<'w> {
        ()
    }
    // fn init_state(world: &mut World, meta: &mut SystemMeta) {
    //     world.add_component_info(ComponentInfo::of::<T>(0));
    //     meta.cur_param.withouts.insert(TypeId::of::<T>(), std::any::type_name::<T>().into());
    // }
    // fn archetype_filter(world: &World, archetype: &Archetype) -> bool {
    //     !archetype.get_column_index_by_tid(world, &TypeId::of::<T>()).is_null()
    // }
}

pub struct With<T: 'static>(PhantomData<T>);
// impl<T: 'static> FilterArchetype for With<T> {
//     fn filter_archetype(world: &World, archetype: &Archetype) -> bool {
//         Self::archetype_filter(world, archetype)
//     }
// }
impl<T: 'static> FilterComponents for With<T> {
    // const LISTENER_COUNT: usize = 0;
    type Filter<'w> = ();
    type State = ComponentIndex;
    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        meta.component_relate(
            world,
            ComponentInfo::of::<T>(0),
            crate::system::Relation::With(0usize.into()),
        )
        .0
    }
    // fn filter_archetype(_world: &World, state: &Self::State, archetype: &Archetype) -> bool {
    //     !archetype.contains(*state)
    // }
    #[inline]
    fn init_filter<'w>(
        _world: &'w World,
        _state: &'w Self::State,
        _archetype: &'w Archetype,
        _tick: Tick,
        _last_run: Tick,
    ) -> Self::Filter<'w> {
        ()
    }
    // fn init_state(world: &mut World, meta: &mut SystemMeta) {
    //     world.add_component_info(ComponentInfo::of::<T>(0));
    //     meta.cur_param.withs.insert(TypeId::of::<T>(), std::any::type_name::<T>().into());
    // }
    // fn archetype_filter(world: &World, archetype: &Archetype) -> bool {
    //     archetype.get_column_index_by_tid(world, &TypeId::of::<T>()).is_null()
    // }
}

pub struct Changed<T: 'static>(PhantomData<T>);
impl<T: 'static> FilterComponents for Changed<T> {
    // const LISTENER_COUNT: usize = 1;
    type Filter<'w> = (BlobRef<'w>, Tick);
    type State = Share<Column>;
    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        meta.component_relate(
            world,
            ComponentInfo::of::<T>(COMPONENT_TICK),
            crate::system::Relation::Read(0usize.into()),
        )
        .1
    }
    // fn init_listeners(world: &mut World, listeners: &mut Vec<ComponentIndex>) {
    //     listeners.push(world.add_component_info(ComponentInfo::of::<T>(COMPONENT_TICK | COMPONENT_CHANGED)).0);
    // }
    #[inline(always)]
    fn init_filter<'w>(
        _world: &'w World,
        state: &'w Self::State,
        archetype: &'w Archetype,
        _tick: Tick,
        last_run: Tick,
    ) -> Self::Filter<'w> {
        (state.blob_ref(archetype.index()), last_run)
    }

    #[inline(always)]
    fn filter<'w>(fetch: &Self::Filter<'w>, row: Row, _e: Entity) -> bool {
        fetch.0.get_tick_unchecked(row) <= fetch.1
    }
}
pub struct Or<T: 'static>(PhantomData<T>);
// impl<T: 'static + FilterComponents> FilterComponents for Or<T> {
//     // const LISTENER_COUNT: usize = 0;
//     type Fetch<'w> = T::Fetch<'w>;
//     type State = T::State;
//     fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
//         meta.relate(crate::system::Relation::Or);
//         let s = T::init_state(world, meta);
//         meta.relate(crate::system::Relation::End);
//         s
//     }
//     #[inline]
//     fn init_fetch<'w>(
//         world: &'w World,
//         state: &'w Self::State,
//         archetype: &'w Archetype,
//         tick: Tick,
//         last_run: Tick,
//     ) -> Self::Fetch<'w> {
//         T::init_fetch(world, state, archetype, tick, last_run)
//     }
// }
// pub struct Removed<T: 'static>(PhantomData<T>);
// impl<T: 'static> FilterComponents for Removed<T> {
//     const LISTENER_COUNT: usize = 1;
//     fn init_listeners(world: &mut World, listeners: &mut Vec<ComponentIndex>) {
//         listeners.push(ListenType::Removed(world.add_component_info(ComponentInfo::of::<T>(COMPONENT_REMOVED)).0));
//     }
// }
// pub struct Destroyed;
// impl FilterComponents for Destroyed {
//     const LISTENER_COUNT: usize = 1;
//     fn init_listeners(_world: &mut World, listeners: &mut Vec<ComponentIndex>) {
//         listeners.push(ListenType::Destroyed);
//     }
// }
macro_rules! impl_tuple_filter {
    ($(($name: ident, $state: ident)),*) => {
        #[allow(non_snake_case)]
        #[allow(clippy::unused_unit)]

        impl<$($name: FilterComponents),*> FilterComponents for ($($name,)*) {
            type Filter<'w> = ($($name::Filter<'w>,)*);
            type State = ($($name::State,)*);

            // const LISTENER_COUNT: usize = $($name::LISTENER_COUNT + )* 0;
	        fn init_state(_world: &mut World, _meta: &mut SystemMeta) -> Self::State {
                ($($name::init_state(_world, _meta),)*)
            }
            // fn filter_archetype(_world: &World, _state: &Self::State, _archetype: &Archetype) -> bool {
            //     let ($($state,)*) = _state;
            //     $(
            //         if $name::filter_archetype(_world, $state, _archetype){return true};
            //     )*
            //     false
            // }

            #[allow(clippy::unused_unit)]
            #[inline]
            fn init_filter<'w>(
                _world: &'w World,
                _state: &'w Self::State,
                _archetype: &'w Archetype,
                _tick: Tick,
                _last_run: Tick,
                ) -> Self::Filter<'w> {
                let ($($state,)*) = _state;
                ($($name::init_filter(_world, $state, _archetype, _tick, _last_run),)*)
            }

            #[allow(clippy::unused_unit)]
            #[inline(always)]
            fn filter<'w>(_filter: &Self::Filter<'w>, _row: Row, _e: Entity) -> bool {
                let ($($name,)*) = _filter;
                $(
                    if $name::filter($name, _row, _e){return true};
                )*
                false
            }

            // fn init_listeners(_world: &mut World, _listeners: &mut Vec<ComponentIndex>) {
            //     ($($name::init_listeners(_world, _listeners),)*);
            // }
            // fn archetype_filter(_world: &World, _archetype: &Archetype) -> bool {
            //     $(
            //         if $name::archetype_filter(_world, _archetype){return true};
            //     )*
            //     false
            // }
        }
        impl<$($name: FilterComponents),*> FilterComponents for Or<($($name,)*)> {
            type Filter<'w> = ($($name::Filter<'w>,)*);
            type State = ($($name::State,)*);

            // const LISTENER_COUNT: usize = $($name::LISTENER_COUNT + )* 0;
	        fn init_state(_world: &mut World, _meta: &mut SystemMeta) -> Self::State {
                _meta.relate(crate::system::Relation::Or);
                let s = ($($name::init_state(_world, _meta),)*);
                _meta.relate(crate::system::Relation::End);
                s
            }
            // fn filter_archetype(_world: &World, _state: &Self::State, _archetype: &Archetype) -> bool {
            //     let ($($state,)*) = _state;
            //     $(
            //         if !$name::filter_archetype(_world, $state, _archetype){return false};
            //     )*
            //     true
            // }

            #[allow(clippy::unused_unit)]
            #[inline]
            fn init_filter<'w>(
                _world: &'w World,
                _state: &'w Self::State,
                _archetype: &'w Archetype,
                _tick: Tick,
                _last_run: Tick,
                ) -> Self::Filter<'w> {
                let ($($state,)*) = _state;
                ($($name::init_filter(_world, $state, _archetype, _tick, _last_run),)*)
            }

            #[allow(clippy::unused_unit)]
            #[inline(always)]
            fn filter<'w>(_filter: &Self::Filter<'w>, _row: Row, _e: Entity) -> bool {
                let ($($name,)*) = _filter;
                $(
                    if !$name::filter($name, _row, _e){return false};
                )*
                true
            }

            // fn init_listeners(_world: &mut World, _listeners: &mut Vec<ComponentIndex>) {
            //     ($($name::init_listeners(_world, _listeners),)*);
            // }
            // fn archetype_filter(_world: &World, _archetype: &Archetype) -> bool {
            //     $(
            //         if $name::archetype_filter(_world, _archetype){return true};
            //     )*
            //     false
            // }
        }

    };
}
all_tuples!(impl_tuple_filter, 0, 15, F, S);

// macro_rules! impl_or_tuple_fetch {
//     ($($name: ident),*) => {
//         #[allow(non_snake_case)]
//         #[allow(clippy::unused_unit)]
//         impl<$($name: FilterArchetype),*> FilterArchetype for ($($name,)*) {

//             fn filter_archetype(_world: &World, _archetype: &Archetype) -> bool {
//                 $(
//                     if !$name::filter_archetype(_world, _archetype){return false};
//                 )*
//                 true
//             }
//         }

//     };
// }
// all_tuples!(impl_or_tuple_fetch, 1, 15, F);
