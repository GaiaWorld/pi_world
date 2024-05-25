#![feature(const_type_id)]
#![feature(get_mut_unchecked)]
#![allow(invalid_reference_casting)]
#![allow(incomplete_features)]
#![feature(downcast_unchecked)]
#![feature(sync_unsafe_cell)]
#![feature(test)]
#![feature(specialization)]
#[warn(async_fn_in_trait)]

extern crate test;
/// Most commonly used re-exported types.
pub mod prelude {
    #[cfg(any(feature="single_thread", target_arch = "wasm32"))]
    pub type App = crate::app::SingleThreadApp;

    #[cfg(not(any(feature="single_thread", target_arch = "wasm32")))]
    pub type App = crate::app::MultiThreadApp;

    #[doc(hidden)]
    pub use crate::{
        query::{Query, QueryError},
        insert::{Insert, Bundle, Component},
        insert_batch::InsertBatchIter,
        alter::Alter,
        removed:: ComponentRemoved,
        param_set::{ParamSet, ParamSetElement},
        single_res::{SingleRes, SingleResMut},
        multi_res::{MultiRes, MultiResMut},
        filter::{Changed, Destroyed, Empty, With, Without, Or, FilterComponents},
        fetch::{Has, Mut, OrDefault, Ticker, ArchetypeName},
        system::{BoxedSystem, IntoSystem, IntoAsyncSystem},
        system_params::{SystemParam, Local},
        world::{Entity, World, FromWorld, Tick},
        listener::Listener,
        app::{SingleThreadApp, MultiThreadApp},
        plugin::{Plugin, Plugins},
        plugin_group::WorldPluginExtent,
        schedule::{Schedule, Update, PreUpdate, Startup, PostUpdate, Last, First},
        schedule_config::{ScheduleLabel, StageLabel, SystemSet, IntoSystemSetConfigs, IntoSystemConfigs},
        exec_graph::ExecGraph,
        dot::{Dot, Config},
        safe_vec::SafeVec,
        commands::{Command, CommandQueue},
    };
}

pub mod column;
pub mod table;
pub mod archetype;
pub mod query;
pub mod fetch;
pub mod filter;
pub mod removed;
pub mod param_set;
pub mod single_res;
pub mod multi_res;
pub mod world;
pub mod listener;
pub mod app;
pub mod system;
pub mod system_params;
pub mod function_system;
pub mod async_function_system;
pub mod insert;
pub mod insert_batch;
pub mod alter;
pub mod dirty;
pub mod safe_vec;
pub mod exec_graph;
pub mod dot;
pub mod schedule;
pub mod editor;
pub mod commands;

pub mod example;
pub mod schedule_config;
mod plugin;
mod plugin_group;
pub mod utils;
mod debug;