use std::{any::TypeId, borrow::Cow, future::Future, marker::PhantomData, mem::transmute, pin::Pin};

use crate::{
    function_system::ParamSystem,
    system::{AsyncRunSystem, IntoAsyncSystem, System, SystemMeta, TypeInfo},
    system_params::SystemParam,
    world::*,
};

use pi_proc_macros::all_tuples;

pub trait AsyncSystemParamFunction<Marker, Out>: Clone + Send + Sync + 'static {
    /// The [`SystemParam`]/s used by this system to access the [`World`].
    type Param: SystemParam;

    /// Executes this system once. See [`System::run`] or [`System::run_unsafe`].
    fn run(self, _param_value: Self::Param) -> Pin<Box<dyn Future<Output = Out> + Send + 'static>>;
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
pub struct AsyncFunctionSystem<Marker, Out,  F>
where
    F: AsyncSystemParamFunction<Marker, Out>,
{
    pub func: F,
    pub param: ParamSystem<F::Param>,
    pub marker: PhantomData<Out>,
    pub(crate) is_first: bool,
}

impl<Marker: 'static, F, Out: 'static + Send + Sync> IntoAsyncSystem<Marker, Out> for F
where
    F: AsyncSystemParamFunction<Marker, Out>,
{
    type System = AsyncFunctionSystem<Marker, Out, F>;
    fn into_async_system(self) -> Self::System {
        AsyncFunctionSystem {
            func: self,
            param: ParamSystem::new(SystemMeta::new(TypeInfo::of::<F>())),
            marker: PhantomData,
            is_first: true,
        }
    }
}

impl<Marker: 'static, F, Out: 'static + Send + Sync> System for AsyncFunctionSystem<Marker, Out, F>
where
    F: AsyncSystemParamFunction<Marker, Out>,
{
    type Out = Out;
    #[inline]
    fn name(&self) -> &Cow<'static, str> {
        self.param.name()
    }

    #[inline]
    fn id(&self) -> TypeId {
        self.param.type_id()
    }
    #[inline]
    fn initialize(&mut self, world: &mut World) {
        self.param.initialize(world)
    }
    // /// system depend the archetype.
    // fn archetype_depend(
    //     &self,
    //     world: &World,
    //     archetype: &Archetype,
    //     result: &mut ArchetypeDependResult,
    // ) {
    //     self.param.archetype_depend(world, archetype, result)
    // }
    // /// system depend the res.
    // fn res_depend(
    //     &self,
    //     world: &World,
    //     res_tid: &TypeId,
    //     res_name: &Cow<'static, str>,
    //     single: bool,
    //     result: &mut Flags,
    // ) {
    //     self.param
    //         .res_depend(world, res_tid, res_name, single, result)
    // }
    #[inline]
    fn align(&mut self, world: &World) {
        if self.param.archetype_align_len < world.archetype_arr.len() {
            self.param.align();
            self.param.archetype_align_len = world.archetype_arr.len();
        }
    }
}
impl<Marker: 'static, Out: 'static + Send + Sync, F> AsyncRunSystem for AsyncFunctionSystem<Marker, Out, F>
where
    F: AsyncSystemParamFunction<Marker, Out>,
{
    #[inline]
    fn run(&mut self, world: &'static World) -> Pin<Box<dyn Future<Output = Out> + Send + 'static>> {
        self.param.system_meta.last_run = self.param.system_meta.this_run;
        if self.param.archetype_align_len < world.archetype_arr.len() {
            self.param.align();
            self.param.archetype_align_len = world.archetype_arr.len();
        }
        let param_state = self.param.param_state.as_mut().unwrap();
        let params = F::Param::get_self(param_state);
        if self.is_first {
            F::Param::init( param_state);
            self.is_first = false;
        }
        self.func.clone().run(params)
    }
}

macro_rules! impl_async_system_function {
    ($($param: ident),*) => {
        #[allow(non_snake_case)]
        impl<Func: Clone + Send + Sync + 'static, Out, R, $($param: SystemParam),*> AsyncSystemParamFunction<fn($($param,)*)->R, Out> for Func
        where Func:
                FnMut($($param),*) -> R,
                R: Future<Output=Out>,
        {
            type Param = ($($param,)*);
            #[inline]
            fn run(mut self, param_value: ($($param,)*)) -> Pin<Box<dyn Future<Output = Out> + Send + 'static>> {
                let ($($param,)*) = param_value;
                let r: Pin<Box<dyn Future<Output = Out>>> = Box::pin(self($($param,)*));
                unsafe {transmute(r)}
            }
        }
    };
}

// Note that we rely on the highest impl to be <= the highest order of the tuple impls
// of `SystemParam` created.
all_tuples!(impl_async_system_function, 0, 16, F);
