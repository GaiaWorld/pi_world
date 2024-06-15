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

use crate::archetype::{ArchetypeIndex, ComponentInfo, Row, COMPONENT_TICK};
use crate::column::{BlobRef, Column};
use crate::prelude::{Entity, Tick};
use crate::system::SystemMeta;
use crate::world::{ComponentIndex, World};

pub trait FilterComponents {
    // const LISTENER_COUNT: usize;
    type Filter<'w>;
    type State: Send + Sync + Sized;
    /// initializes ReadWrite for this [`FilterComponents`] type.
    fn init_state(_world: &mut World, _meta: &mut SystemMeta) -> Self::State;

    fn init_filter<'w>(
        world: &'w World,
        state: &'w Self::State,
        index: ArchetypeIndex,
        tick: Tick,
        last_run: Tick,
    ) -> Self::Filter<'w>;

    fn filter<'w>(_filter: &Self::Filter<'w>, _row: Row, _e: Entity) -> bool {
        false
    }
}

pub struct Without<T: 'static>(PhantomData<T>);
impl<T: 'static> FilterComponents for Without<T> {

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

    #[inline]
    fn init_filter<'w>(
        _world: &'w World,
        _state: &'w Self::State,
        _index: ArchetypeIndex,
        _tick: Tick,
        _last_run: Tick,
    ) -> Self::Filter<'w> {
        ()
    }
}

pub struct With<T: 'static>(PhantomData<T>);
impl<T: 'static> FilterComponents for With<T> {

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
    #[inline]
    fn init_filter<'w>(
        _world: &'w World,
        _state: &'w Self::State,
        _index: ArchetypeIndex,
        _tick: Tick,
        _last_run: Tick,
    ) -> Self::Filter<'w> {
        ()
    }

}

pub struct Changed<T: 'static>(PhantomData<T>);
impl<T: 'static> FilterComponents for Changed<T> {

    type Filter<'w> = (Option<BlobRef<'w>>, Tick);
    type State = Share<Column>;
    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        meta.component_relate(
            world,
            ComponentInfo::of::<T>(COMPONENT_TICK),
            crate::system::Relation::Read(0usize.into()),
        )
        .1
    }

    #[inline(always)]
    fn init_filter<'w>(
        _world: &'w World,
        state: &'w Self::State,
        index: ArchetypeIndex,
        _tick: Tick,
        last_run: Tick,
    ) -> Self::Filter<'w> {
        (state.blob_ref(index), last_run)
    }

    #[inline(always)]
    fn filter<'w>(filter: &Self::Filter<'w>, row: Row, _e: Entity) -> bool {
        if let Some(r) = &filter.0 {
            r.get_tick_unchecked(row) <= filter.1
        }else{
            true
        }
    }
}
pub struct Or<T: 'static>(PhantomData<T>);


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

            #[allow(clippy::unused_unit)]
            #[inline]
            fn init_filter<'w>(
                _world: &'w World,
                _state: &'w Self::State,
                _index: ArchetypeIndex,
                _tick: Tick,
                _last_run: Tick,
                ) -> Self::Filter<'w> {
                let ($($state,)*) = _state;
                ($($name::init_filter(_world, $state, _index, _tick, _last_run),)*)
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

        }

        #[allow(non_snake_case)]
        #[allow(clippy::unused_unit)]

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

            #[allow(clippy::unused_unit)]
            #[inline]
            fn init_filter<'w>(
                _world: &'w World,
                _state: &'w Self::State,
                _index: ArchetypeIndex,
                _tick: Tick,
                _last_run: Tick,
                ) -> Self::Filter<'w> {
                let ($($state,)*) = _state;
                ($($name::init_filter(_world, $state, _index, _tick, _last_run),)*)
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
        }

    };
}
all_tuples!(impl_tuple_filter, 0, 15, F, S);
