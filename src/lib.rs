#![feature(const_type_id)]
#![feature(get_mut_unchecked)]
#![allow(invalid_reference_casting)]
#![feature(test)]
extern crate test;

pub mod column;
pub mod table;
pub mod archetype;
pub mod query;
pub mod fetch;
pub mod filter;
pub mod world;
pub mod listener;
pub mod app;
pub mod system;
pub mod system_parms;
pub mod function_system;
pub mod insert;
pub mod insert_batch;
pub mod alter;
pub mod dirty;
pub mod safe_vec;
pub mod exec_graph;
pub mod dot;
pub mod schedule;

pub mod example;

/// Most commonly used re-exported types.
pub mod prelude {
    #[doc(hidden)]
    pub use crate::{
        query::{Query, QueryError},
        insert::Insert,
        insert_batch::InsertBatchIter,
        alter::Alter,
        filter::{Added, Changed, With, Without, Or},
        fetch::{Has, Mut},
        system::{BoxedSystem, IntoSystem},
        system_parms::SystemParam,
        world::{Entity, World},
        listener::Listener,
        app::{App, SingleThreadApp, MultiThreadApp},
        schedule::Schedule,
        exec_graph::ExecGraph,
        dot::{Dot, Config},
        safe_vec::SafeVec,
    };
}
