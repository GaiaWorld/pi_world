/// system上只能看到Query，Sys参数，资源 实体 组件 事件 命令

use core::fmt::*;
use std::any::{TypeId, Any};
use std::borrow::Cow;
use std::marker::PhantomData;
use std::mem::{needs_drop, size_of, transmute};
use std::ops::{Index, IndexMut, Range};
use std::ptr::{copy, null_mut, write};
use std::slice;
use core::result::Result;

use crate::archetype::{ArchetypeKey, Archetype};
use crate::listener::ListenerMgr;
use dashmap::DashMap;
use pi_arr::Arr;
use pi_key_alloter::{new_key_type, is_older_version};
use pi_share::Share;
use pi_slot::*;
use pi_null::Null;

new_key_type! {
    pub struct Entity;
}
pub enum QueryComponentError {
    /// The [`Query`] does not have read access to the requested component.
    ///
    /// This error occurs when the requested component is not included in the original query.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ecs::{prelude::*, system::QueryComponentError};
    /// #
    /// # #[derive(Component)]
    /// # struct OtherComponent;
    /// #
    /// # #[derive(Component, PartialEq, Debug)]
    /// # struct RequestedComponent;
    /// #
    /// # #[derive(Resource)]
    /// # struct SpecificEntity {
    /// #     entity: Entity,
    /// # }
    /// #
    /// fn get_missing_read_access_error(query: Query<&OtherComponent>, res: Res<SpecificEntity>) {
    ///     assert_eq!(
    ///         query.get_component::<RequestedComponent>(res.entity),
    ///         Err(QueryComponentError::MissingReadAccess),
    ///     );
    ///     println!("query doesn't have read access to RequestedComponent because it does not appear in Query<&OtherComponent>");
    /// }
    /// # bevy_ecs::system::assert_is_system(get_missing_read_access_error);
    /// ```
    MissingReadAccess,
    /// The [`Query`] does not have write access to the requested component.
    ///
    /// This error occurs when the requested component is not included in the original query, or the mutability of the requested component is mismatched with the original query.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy_ecs::{prelude::*, system::QueryComponentError};
    /// #
    /// # #[derive(Component, PartialEq, Debug)]
    /// # struct RequestedComponent;
    /// #
    /// # #[derive(Resource)]
    /// # struct SpecificEntity {
    /// #     entity: Entity,
    /// # }
    /// #
    /// fn get_missing_write_access_error(mut query: Query<&RequestedComponent>, res: Res<SpecificEntity>) {
    ///     assert_eq!(
    ///         query.get_component::<RequestedComponent>(res.entity),
    ///         Err(QueryComponentError::MissingWriteAccess),
    ///     );
    ///     println!("query doesn't have write access to RequestedComponent because it doesn't have &mut in Query<&RequestedComponent>");
    /// }
    /// # bevy_ecs::system::assert_is_system(get_missing_write_access_error);
    /// ```
    MissingWriteAccess,
    /// The given [`Entity`] does not have the requested component.
    MissingComponent,
    /// The requested [`Entity`] does not exist.
    NoSuchEntity,
}

/// A value that tracks when a system ran relative to other systems.
/// This is used to power change detection.
#[derive(Copy, Default, Clone, Debug, Eq, PartialEq)]
pub struct Tick(pub u32);

impl Tick {
    pub fn is_new_then(self, other: Tick) -> bool {
        is_older_version(self.0, other.0)
    }
}

pub trait Notify {
    // type Args: Send + Sync + 'static;
}
/// A value that tracks when a system ran relative to other systems.
/// This is used to power change detection.
// #[derive(Default)]
pub struct World {
    res_map: DashMap<u128, Box<dyn Any>>,
    entitys: SlotMap<Entity, EntityValue>,
    archetype_map: DashMap<u128, Share<Archetype>>,
    archetype_arr: Arr<Option<Share<Archetype>>>,
    archetype_arr_len: Share<u32>,
    listener_mgr: ListenerMgr,
    tick: Share<Tick>,
}
impl World {

}

#[derive(Debug)]
pub(crate) struct EntityValue(*mut Archetype, ArchetypeKey, *mut u8);
impl EntityValue {
    pub fn get_archetype(&self) -> &Archetype {
        unsafe { & *self.0 }
    }
    pub fn key(&self) -> ArchetypeKey {
        self.1
    }
    pub fn value(&self) -> *mut u8 {
        self.2
    }
}
impl Default for EntityValue {
    fn default() -> Self {
        Self(null_mut(), ArchetypeKey::default(), null_mut())
    }
}

/// Unique mutable borrow of an entity's component
pub struct Mut<'a, T: ?Sized> {
    pub(crate) value: &'a mut T,
    pub(crate) ticks: TicksMut<'a>,
}
pub(crate) struct TicksMut<'a> {
    pub(crate) added: &'a mut Tick,
    pub(crate) changed: &'a mut Tick,
    pub(crate) last_run: Tick,
    pub(crate) this_run: Tick,
}
/// Records when a component was added and when it was last mutably dereferenced (or added).
#[derive(Copy, Clone, Debug)]
pub struct ComponentTicks {
    pub(crate) added: Tick,
    pub(crate) changed: Tick,
}

/// The metadata of a [`System`].
#[derive(Clone)]
pub struct SystemMeta {
    pub(crate) name: Cow<'static, str>,
    pub(crate) last_run: Tick,
}

impl SystemMeta {
    pub(crate) fn new<T>() -> Self {
        Self {
            name: std::any::type_name::<T>().into(),
            last_run: Tick(0),
        }
    }

    /// Returns the system's name
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }
}

pub unsafe trait SystemParam: Sized {
    /// Used to store data which persists across invocations of a system.
    type State: Send + Sync + 'static;

    /// The item type returned when constructing this system param.
    /// The value of this associated type should be `Self`, instantiated with new lifetimes.
    ///
    /// You could think of `SystemParam::Item<'w, 's>` as being an *operation* that changes the lifetimes bound to `Self`.
    // type Item<'world, 'state>: SystemParam<State = Self::State>;

    /// Registers any [`World`] access used by this [`SystemParam`]
    /// and creates a new instance of this param's [`State`](Self::State).
    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State;

    /// For the specified [`Archetype`], registers the components accessed by this [`SystemParam`] (if applicable).
    #[inline]
    fn new_archetype(
        _state: &mut Self::State,
        _archetype: &Archetype,
        _system_meta: &mut SystemMeta,
    ) {
    }

    /// Applies any deferred mutations stored in this [`SystemParam`]'s state.
    /// This is used to apply [`Commands`] during [`apply_deferred`](crate::prelude::apply_deferred).
    ///
    /// [`Commands`]: crate::prelude::Commands
    #[inline]
    #[allow(unused_variables)]
    fn apply(state: &mut Self::State, system_meta: &SystemMeta, world: &mut World) {}

    /// Creates a parameter to be passed into a [`SystemParamFunction`].
    ///
    /// [`SystemParamFunction`]: super::SystemParamFunction
    ///
    /// # Safety
    ///
    /// - The passed [`UnsafeWorldCell`] must have access to any world data
    ///   registered in [`init_state`](SystemParam::init_state).
    /// - `world` must be the same `World` that was used to initialize [`state`](SystemParam::init_state).
    unsafe fn get_param<'world, 'state>(
        state: &'state mut Self::State,
        system_meta: &SystemMeta,
        world: &'world World,
        change_tick: Tick,
    ) -> Self;
}
