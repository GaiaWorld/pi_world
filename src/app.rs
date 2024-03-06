use std::collections::HashMap;

use fixedbitset::FixedBitSet;
use pi_async_rt::rt::{AsyncRuntime, AsyncRuntimeExt};
use pi_share::Share;

/// App包含一个world，和一个主执行器，及多个阶段执行器
///
use crate::{archetype::Row, exec_graph::ExecGraph, safe_vec::SafeVec, system::BoxedSystem, world::*};

pub struct App {
    world: World,
    systems: Share<SafeVec<BoxedSystem>>,
    graph: ExecGraph,
    stage_graph: HashMap<&'static str, ExecGraph>,
    action: Vec<(Row, Row)>,
    set: FixedBitSet,
}

impl App {
    pub fn new() -> Self {
        Self {
            world: World::new(),
            systems: Share::new(SafeVec::default()),
            graph: ExecGraph::default(),
            stage_graph: Default::default(),
            action: Vec::new(),
            set: FixedBitSet::new(),
        }
    }
    pub fn get_world(&self) -> &World {
        &self.world
    }
    pub fn get_systems(&self) -> &Share<SafeVec<BoxedSystem>> {
        &self.systems
    }

    pub fn get_graph(&self) -> &ExecGraph {
        &self.graph
    }
    pub fn get_stage_graph(&self, name: &'static str) -> Option<&ExecGraph> {
        self.stage_graph.get(name)
    }

    pub fn register(&mut self, system: BoxedSystem, stages: &[&'static str]) -> usize {
        let name = system.name().clone();
        let index = self.systems.insert(system);
        println!("register, sys_index: {:?}", index);
        let sys = unsafe { self.systems.load_unchecked_mut(index) };
        println!("register, sys: {:?}, len:{}", sys.name(), self.systems.len());
        self.graph.add_system((index, name.clone()));
        for stage in stages {
            let e = self.stage_graph.entry(*stage);
            let g = e.or_default();
            g.add_system((index, name.clone()));
        }
        index
    }
    pub fn initialize(&mut self) {
        Share::get_mut(&mut self.systems).unwrap().collect();
        // 首先初始化所有的system，有Insert的会产生对应的原型
        for sys in self.systems.iter() {
            println!("system initialize, {:?}", &sys.name());
            sys.initialize(&self.world);
        }
        println!("system initialized, len:{}", self.systems.len());
        // todo 遍历world上的单例，测试和system的读写关系
        
        self.graph.initialize(&mut self.world, self.systems.clone());
        for stage in self.stage_graph.values_mut() {
            stage.initialize(&mut self.world, self.systems.clone());
        }
        println!("system initialized2, len:{}", self.systems.len());
    }
    pub fn run<A: AsyncRuntime + AsyncRuntimeExt>(&mut self, rt: &A) {
        let g = self.graph.clone();
        self.run_graph(g, rt);
    }
    pub fn run_stage<A: AsyncRuntime + AsyncRuntimeExt>(&mut self, stage: &str, rt: &A) {
        let g = self.stage_graph.get(stage).unwrap().clone();
        self.run_graph(g, rt);
    }
    fn run_graph<A: AsyncRuntime + AsyncRuntimeExt>(&mut self, g: ExecGraph, rt: &A) {
        let rt1 = rt.clone();
        let w: &'static World = unsafe { std::mem::transmute(&self.world) };
        println!("run_graph, len:{}", self.systems.len());
        let s: &'static Share<SafeVec<BoxedSystem>> = unsafe { std::mem::transmute(&self.systems) };
        println!("run_graph2, len:{}", s.len());
        let mut gg = g.clone();
        let _ = rt.block_on(async move {
            let rt2 = rt1;
            g.run(&rt2, w, s).await.unwrap();
        });
        unsafe { self.world.collect(&mut self.action, &mut self.set) };
        gg.collect();
    }

}
