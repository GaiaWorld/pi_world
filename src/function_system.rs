use std::{any::TypeId, borrow::Cow};

use crate::{
    archetype::{Archetype, ArchetypeDependResult, Flags},
    system::{IntoSystem, RunSystem, System, SystemMeta, TypeInfo},
    system_params::SystemParam,
    world::*,
};

use pi_proc_macros::all_tuples;

/// Shorthand way of accessing the associated type [`SystemParam::Item`] for a given [`SystemParam`].
pub type SystemParamItem<'w, P> = <P as SystemParam>::Item<'w>;

pub trait SystemParamFunction<Marker>: Send + Sync + 'static {
    /// The [`SystemParam`]/s used by this system to access the [`World`].
    type Param: SystemParam;

    /// Executes this system once. See [`System::run`] or [`System::run_unsafe`].
    fn run(&mut self, _param_value: SystemParamItem<Self::Param>) {}
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
pub struct FunctionSystem<Marker: 'static, F>
where
    F: SystemParamFunction<Marker>,
{
    func: F,
    param: ParamSystem<F::Param>,
}

impl<Marker: 'static, F> IntoSystem<Marker> for F
where
    F: SystemParamFunction<Marker>,
{
    type System = FunctionSystem<Marker, F>;
    fn into_system(self) -> Self::System {
        FunctionSystem {
            func: self,
            param: ParamSystem::new(SystemMeta::new(TypeInfo::of::<F>())),
        }
    }
}

impl<Marker, F> System for FunctionSystem<Marker, F>
where
    F: SystemParamFunction<Marker>,
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
impl<Marker, F> RunSystem for FunctionSystem<Marker, F>
where
    F: SystemParamFunction<Marker>,
{
    #[inline]
    fn run(&mut self, world: &World) {
        let params = self.param.get_param(world);
        self.func.run(params);
    }
}
pub struct ParamSystem<P: SystemParam> {
    pub(crate) param_state: Option<P::State>,
    pub(crate) system_meta: SystemMeta,
}
impl<P: SystemParam> ParamSystem<P> {
    pub fn new(system_meta: SystemMeta) -> Self {
        Self {
            param_state: None,
            system_meta,
        }
    }
    #[inline]
    pub(crate) fn name(&self) -> &Cow<'static, str> {
        &self.system_meta.type_info.name
    }

    #[inline]
    pub(crate) fn type_id(&self) -> TypeId {
        self.system_meta.type_info.type_id
    }
    #[inline]
    pub(crate) fn initialize(&mut self, world: &mut World) {
        if self.param_state.is_none() {
            self.param_state = Some(P::init_state(world, &mut self.system_meta));
        }
    }
    /// system depend the archetype.
    pub(crate) fn archetype_depend(
        &self,
        world: &World,
        archetype: &Archetype,
        result: &mut ArchetypeDependResult,
    ) {
        P::archetype_depend(
            world,
            &self.system_meta,
            self.param_state.as_ref().unwrap(),
            archetype,
            result,
        )
    }
    /// system depend the res.
    pub(crate) fn res_depend(
        &self,
        world: &World,
        res_tid: &TypeId,
        res_name: &Cow<'static, str>,
        single: bool,
        result: &mut Flags,
    ) {
        P::res_depend(
            world,
            &self.system_meta,
            self.param_state.as_ref().unwrap(),
            res_tid,
            res_name,
            single,
            result,
        )
    }
    #[inline]
    pub(crate) fn align(&mut self, world: &World) {
        let param_state = self.param_state.as_mut().unwrap();
        P::align(world, &mut self.system_meta, param_state);
    }
    #[inline]
    pub fn get_param<'w>(&'w mut self, world: &'w World) -> SystemParamItem<'w, P> {
        let tick = world.increment_tick();
        let param_state = self.param_state.as_mut().unwrap();
        P::get_param(world, &mut self.system_meta, param_state, tick)
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
all_tuples!(impl_system_function, 0, 32, F);
