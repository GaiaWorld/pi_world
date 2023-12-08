/// App包含一个world，和执行器
/// 

use crate::{world::*, exec::Runnble, system::BoxedSystem};

use pi_share::Share;



/// A value that tracks when a system ran relative to other systems.
/// This is used to power change detection.
// #[derive(Default)]
pub struct App {
    world: World,
    vec: Vec<BoxedSystem>,
}

impl App {

    pub fn new() -> Self {
        Self {world: World::new(), vec: Vec::new()}
    }
    pub fn register(&mut self, mut system: BoxedSystem) {
        system.initialize(&self.world);
        self.vec.push(system);
    }
    pub fn run(&mut self) {
        for system in self.vec.iter_mut() {
            system.run(&self.world)
        }
    }
}
