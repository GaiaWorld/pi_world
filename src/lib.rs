#![feature(const_type_id)]
#![feature(get_mut_unchecked)]
#![allow(invalid_reference_casting)]
#![allow(incomplete_features)]
#![feature(downcast_unchecked)]
#![feature(sync_unsafe_cell)]
#![feature(test)]
#![feature(specialization)]
#![allow(invalid_type_param_default)]
#[warn(async_fn_in_trait)]

extern crate test;
/// Most commonly used re-exported types.
pub mod prelude {

    #[doc(hidden)]
    pub use crate::{
        app::App,
        query::{Query, QueryError},
        insert::{Insert, Bundle, Component},
        insert_batch::InsertBatchIter,
        alter::Alter,
        editor::EntityEditor,
        event:: {Event, EventSender, ComponentChanged, ComponentAdded, ComponentRemoved},
        param_set::{ParamSet, ParamSetElement},
        single_res::{SingleRes, SingleResMut},
        multi_res::{MultiRes, MultiResMut},
        filter::{Changed, With, Without, Or, FilterComponents},
        fetch::{Has, Ref, Mut, OrDefault, OrDefaultRef, Ticker, ComponentId, ArchetypeName},
        system::{BoxedSystem, IntoSystem, IntoAsyncSystem, SystemMeta},
        system_params::{SystemParam, Local},
        world::{Entity, World, FromWorld, Tick},
        listener::Listener,
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
pub mod event;
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