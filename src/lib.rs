#![feature(arbitrary_self_types)]
#![feature(const_type_id)]
#![feature(get_mut_unchecked)]
#![feature(associated_type_defaults)]
#![allow(invalid_reference_casting)]
#![allow(incomplete_features)]
#![feature(downcast_unchecked)]
#![feature(sync_unsafe_cell)]
#![feature(test)]
#![feature(specialization)]
#![feature(arbitrary_self_types)]
#![allow(invalid_type_param_default)]
#![allow(elided_named_lifetimes)]


// // 当编译wasm时启用重新编译Rust标准库使用test做基准测试会出现重复链接的编译错误
// #[cfg(not(target_arch = "wasm32"))]
// extern crate test as test1;
/// Most commonly used re-exported types.
pub mod prelude {

    #[doc(hidden)]
    pub use crate::{
        param_unready::ParamUnReady,
        app::App,
        query::{Query, QueryUnReady, QueryError, EntryQuery},
        insert::{Insert, Bundle, Component},
        alter::Alter,
        editor::EntityEditor,
        event:: {Event, EventReader, EventWriter, ComponentChanged, ComponentAdded, ComponentRemoved},
        param_set::{ParamSet, ParamSetElement},
        single_res::{SingleRes, SingleResMut},
        // multi_res::{MultiRes, MultiResMut},
        filter::{Changed, With, Without, Or, FilterComponents},
        fetch::{Has, Ref, Mut, OrDefault, OrDefaultRef, Ticker, ComponentId, ArchetypeName},
        system::{BoxedSystem, IntoSystem, IntoAsyncSystem, SystemMeta},
        system_params::{SystemParam, Local, ComponentDebugIndex},
        world::{Entity, World, FromWorld, Tick},
        listener::Listener,
        plugin::{Plugin, Plugins},
        plugin_group::WorldPluginExtent,
        schedule::{Schedule, Update, PreUpdate, Startup, PostUpdate, Last, First},
        schedule_config::{ScheduleLabel, StageLabel, SystemSet, IntoSystemSetConfigs, IntoSystemConfigs},
        exec_graph::ExecGraph,
        dot::{Dot, Config},
        commands::{Command, CommandQueue},
    };
}

pub mod param_unready;
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
pub mod alter;
// pub mod safe_vec;
pub mod exec_graph;
pub mod dot;
pub mod schedule;
pub mod editor;
pub mod commands;
pub mod entry_query;
pub mod world_ptr;

mod test;
pub mod schedule_config;
mod plugin;
mod plugin_group;
pub mod utils;
pub mod debug;
