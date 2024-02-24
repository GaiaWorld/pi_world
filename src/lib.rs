#![feature(const_type_id)]
#![feature(get_mut_unchecked)]
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
pub mod alter;
pub mod dirty;
pub mod safe_vec;
pub mod exec_graph;
pub mod dot;

pub mod example;
// pub mod bench1;
