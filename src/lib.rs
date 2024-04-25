#![feature(const_type_id)]
#![feature(get_mut_unchecked)]
#![allow(invalid_reference_casting)]
#![feature(downcast_unchecked)]
#![feature(sync_unsafe_cell)]
#![feature(test)]
#[warn(async_fn_in_trait)]

extern crate test;

pub mod column;
pub mod table;
pub mod archetype;
pub mod query;
pub mod fetch;
pub mod filter;
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

pub mod commands;

// pub mod example;
pub mod schedule_config;


/// Most commonly used re-exported types.
pub mod prelude {
    #[cfg(target_arch = "wasm32")]
    pub type App = crate::app::SingleThreadApp;

    #[cfg(not(target_arch = "wasm32"))]
    pub type App = crate::app::MultiThreadApp;

    #[doc(hidden)]
    pub use crate::{
        query::{Query, QueryError},
        insert::{Insert, Bundle},
        insert_batch::InsertBatchIter,
        alter::Alter,
        param_set::ParamSet,
        single_res::{SingleRes, SingleResMut},
        multi_res::{MultiRes, MultiResMut},
        filter::{Added, Changed, Removed, With, Without, Or},
        fetch::{Has, Mut},
        system::{BoxedSystem, IntoSystem, IntoAsyncSystem},
        system_params::{SystemParam, Local},
        world::{Entity, World, FromWorld},
        listener::Listener,
        app::{SingleThreadApp, MultiThreadApp},
        schedule::Schedule,
        schedule_config::{ScheduleLabel, StageLabel, SystemSet},
        exec_graph::ExecGraph,
        dot::{Dot, Config},
        safe_vec::SafeVec,
        commands::{Command, CommandQueue},
    };
}
