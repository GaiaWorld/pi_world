use core::sync::atomic::Ordering;
use std::{any::TypeId, borrow::Cow};

use crate::{
    archetype::{Archetype, ArchetypeDependResult},
    system::{IntoSystem, System, SystemMeta, SystemStatus},
    system_parms::SystemParam,
    world::*,
};

use pi_proc_macros::all_tuples;

/// Shorthand way of accessing the associated type [`SystemParam::Item`] for a given [`SystemParam`].
pub type SystemParamItem<'w, P> = <P as SystemParam>::Item<'w>;

pub trait SystemParamFunction<Marker>: Send + Sync + 'static {
    /// The [`SystemParam`]/s used by this system to access the [`World`].
    type Param: SystemParam;

    /// Executes this system once. See [`System::run`] or [`System::run_unsafe`].
    fn run(&mut self, param_value: SystemParamItem<Self::Param>);
}

/// The [`System`] counter part of an ordinary function.
///
/// You get this by calling [`IntoSystem::into_system`]  on a function that only accepts
/// [`SystemParam`]s. The output of the system becomes the functions return type, while the input
/// becomes the functions [`In`] tagged parameter or `()` if no such parameter exists.
///
/// [`FunctionSystem`] must be `.initialized` before they can be run.
///
/// The [`Clone`] implementation for [`FunctionSystem`] returns a new instance which
/// is NOT initialized. The cloned system must also be `.initialized` before it can be run.
pub struct FunctionSystem<Marker, F>
where
    F: SystemParamFunction<Marker>,
{
    func: F,
    param_state: Option<<F::Param as SystemParam>::State>,
    system_meta: SystemMeta,
}

impl<Marker, F> IntoSystem<Marker> for F
where
    F: SystemParamFunction<Marker>,
{
    type System = FunctionSystem<Marker, F>;
    fn into_system(func: Self) -> Self::System {
        let system_meta = SystemMeta::new::<F>();
        FunctionSystem {
            func,
            param_state: None,
            system_meta,
        }
    }
}

impl<Marker, F> System for FunctionSystem<Marker, F>
where
    F: SystemParamFunction<Marker>,
{
    #[inline]
    fn name(&self) -> &Cow<'static, str> {
        &self.system_meta.name
    }

    #[inline]
    fn type_id(&self) -> TypeId {
        TypeId::of::<F>()
    }
    #[inline]
    fn initialize(&mut self, world: &World) {
        if self.get_status() != SystemStatus::Init {
            panic!("Double Initialization Not Allowed")
        }
        self.param_state = Some(F::Param::init_state(world, &mut self.system_meta));
        self.set_status(SystemStatus::Over)
    }
    /// get the system status.
    fn get_status(&self) -> SystemStatus {
        self.system_meta.get_status()
    }
    /// set the system status.
    fn set_status(&self, status: SystemStatus) {
        self.system_meta.set_status(status, Ordering::Relaxed);
    }

    /// system depend the archetype.
    fn depend(&self, world: &World, archetype: &Archetype, result: &mut ArchetypeDependResult) {
        F::Param::depend(
            world,
            &self.system_meta,
            self.param_state.as_ref().unwrap(),
            archetype,
            result,
        )
    }

    #[inline]
    fn run(&mut self, world: &World) {
        let change_tick = world.increment_change_tick();
        let param_state = self.param_state.as_mut().unwrap();
        F::Param::before(param_state, &mut self.system_meta, world, change_tick);
        self.system_meta
            .set_status(SystemStatus::Running, Ordering::Relaxed);
        // SAFETY:
        // - All world accesses used by `F::Param` have been registered, so the caller
        //   will ensure that there are no data access conflicts.
        let params = F::Param::get_param(param_state, &mut self.system_meta, world, change_tick);
        self.func.run(params);
        F::Param::after(param_state, &mut self.system_meta, world, change_tick);
        self.system_meta
            .set_status(SystemStatus::Over, Ordering::Release);
    }
}

macro_rules! impl_system_function {
    ($($param: ident),*) => {
        #[allow(non_snake_case)]
        impl<Func: Send + Sync + 'static, $($param: SystemParam),*> SystemParamFunction<fn($($param,)*)> for Func
        where
        for <'a> &'a mut Func:
                FnMut($($param),*) +
                FnMut($(SystemParamItem<$param>),*),
        {
            type Param = ($($param,)*);
            #[inline]
            fn run(&mut self, param_value: SystemParamItem< ($($param,)*)>) {
                // Yes, this is strange, but `rustc` fails to compile this impl
                // without using this function. It fails to recognize that `func`
                // is a function, potentially because of the multiple impls of `FnMut`
                #[allow(clippy::too_many_arguments)]
                fn call_inner<$($param,)*>(
                    mut f: impl FnMut($($param,)*),
                    $($param: $param,)*
                ) {
                    f($($param,)*)
                }
                let ($($param,)*) = param_value;
                call_inner(self, $($param),*)
            }
        }
    };
}

// Note that we rely on the highest impl to be <= the highest order of the tuple impls
// of `SystemParam` created.
all_tuples!(impl_system_function, 0, 16, F);
