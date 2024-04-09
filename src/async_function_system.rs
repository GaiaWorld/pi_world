use std::{any::TypeId, borrow::Cow, future::Future, pin::Pin, mem::transmute};

use crate::{
    archetype::{Archetype, ArchetypeDependResult, Flags}, function_system::ParamSystem, system::{AsyncRunSystem, IntoAsyncSystem, System, SystemMeta}, system_parms::SystemParam, world::*
};

use pi_proc_macros::all_tuples;

/// Shorthand way of accessing the associated type [`SystemParam::Item`] for a given [`SystemParam`].
pub type SystemParamItem<'w, P> = <P as SystemParam>::Item<'w>;

pub trait AsyncSystemParamFunction<Marker>: Clone + Send + Sync + 'static {
    /// The [`SystemParam`]/s used by this system to access the [`World`].
    type Param: SystemParam;

    /// Executes this system once. See [`System::run`] or [`System::run_unsafe`].
    fn run<'w>(
        self,
        _param_value: SystemParamItem<'w, Self::Param>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
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
pub struct AsyncFunctionSystem<Marker, F>
where
    F: AsyncSystemParamFunction<Marker>,
{
    pub func: F,
    pub param: ParamSystem<F::Param>,
}

impl<Marker: 'static, F> IntoAsyncSystem<Marker> for F
where
    F: AsyncSystemParamFunction<Marker>,
{
    type System = AsyncFunctionSystem<Marker, F>;
    fn into_system(func: Self) -> Self::System {
        AsyncFunctionSystem {
            func,
            param: ParamSystem::new(SystemMeta::new::<F>()),
        }
    }
}

impl<Marker: 'static, F> System for AsyncFunctionSystem<Marker, F>
where
    F: AsyncSystemParamFunction<Marker>,
{
    #[inline]
    fn name(&self) -> &Cow<'static, str> {
        self.param.name()
    }

    #[inline]
    fn type_id(&self) -> TypeId {
        self.param.type_id()
    }
    #[inline]
    fn initialize(&mut self, world: &mut World) {
        self.param.initialize(world)
    }
    /// system depend the archetype.
    fn archetype_depend(
        &self,
        world: &World,
        archetype: &Archetype,
        result: &mut ArchetypeDependResult,
    ) {
        self.param.archetype_depend(world, archetype, result)
    }
    /// system depend the res.
    fn res_depend(
        &self,
        world: &World,
        res_tid: &TypeId,
        res_name: &Cow<'static, str>,
        single: bool,
        result: &mut Flags,
    ) {
        self.param
            .res_depend(world, res_tid, res_name, single, result)
    }
    #[inline]
    fn align(&mut self, world: &World) {
        self.param.align(world)
    }
}
impl<Marker: 'static, F> AsyncRunSystem for AsyncFunctionSystem<Marker, F>
where
    F: AsyncSystemParamFunction<Marker>,
{
    #[inline]
    fn run(&mut self, world: &'static World) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> {
        let param_state = self.param.param_state.as_mut().unwrap();
        let params = F::Param::get_param(world, &mut self.param.system_meta, param_state);
        self.func.clone().run(params)
    }
}

macro_rules! impl_async_system_function {
    ($($param: ident),*) => {
        #[allow(non_snake_case)]
        impl<Func: Clone + Send + Sync + 'static, R, $($param: SystemParam),*> AsyncSystemParamFunction<fn($($param,)*)> for Func
        where Func:
                FnMut($($param),*) -> R +
                FnMut($(SystemParamItem<$param>),*) -> R,
                R: Future<Output=()> + Send + 'static,
        {
            type Param = ($($param,)*);
            #[inline]
            fn run<'w>(self, param_value: SystemParamItem<'w, ($($param,)*)>) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> {
                // Yes, this is strange, but `rustc` fails to compile this impl
                // without using this function. It fails to recognize that `func`
                // is a function, potentially because of the multiple impls of `FnMut`
                #[allow(clippy::too_many_arguments)]
                fn call_inner<$($param,)* R>(
                    mut f: impl FnMut($($param,)*) -> R,
                    $($param: $param,)*
                ) -> Pin<Box<R>> {
                    Box::pin(f($($param,)*))
                }
                let ($($param,)*) = unsafe { param_value};
                call_inner(self, $($param),*)
            }
        }
    };
}

// Note that we rely on the highest impl to be <= the highest order of the tuple impls
// of `SystemParam` created.
all_tuples!(impl_async_system_function, 0, 16, F);
