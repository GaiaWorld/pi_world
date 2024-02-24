use fixedbitset::FixedBitSet;

/// App包含一个world，和执行器
/// 

use crate::{archetype::Row, exec_graph::ExecGraph, system::BoxedSystem, world::*};


#[derive(Default)]
pub struct App {
    world: World,
    graph: ExecGraph,
    action: Vec<(Row, Row)>,
    set: FixedBitSet,
}

impl App {

    pub fn new() -> Self {
        Self {world: World::new(), graph: ExecGraph::new(), action: Vec::new(), set: FixedBitSet::new()}
    }
    pub fn get_world(&self) -> &World {
        &self.world
    }
    pub fn register(&mut self, system: BoxedSystem) {
        self.graph.add_system(system);
    }
    pub fn initialize(&mut self) {
        self.graph.initialize(&mut self.world);
    }
    pub fn run(&mut self) {
        self.graph.run(&mut self.world);
        unsafe { self.world.collect(&mut self.action, &mut self.set) };
        self.graph.collect();
    }
}
