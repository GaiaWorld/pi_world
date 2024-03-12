use std::collections::HashMap;

use fixedbitset::FixedBitSet;
use pi_async_rt::rt::{AsyncRuntime, AsyncRuntimeExt};
use pi_share::Share;

/// Schedule包含一个主执行器，及多个阶段执行器
///
use crate::{
    archetype::Row, exec_graph::ExecGraph, safe_vec::SafeVec, system::BoxedSystem, world::*,
};

pub struct Schedule {
    systems: Share<SafeVec<BoxedSystem>>,
    graph: ExecGraph,
    stage_graph: HashMap<&'static str, ExecGraph>,
    action: Vec<(Row, Row)>,
    set: FixedBitSet,
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            systems: Share::new(SafeVec::default()),
            graph: ExecGraph::default(),
            stage_graph: Default::default(),
            action: Vec::new(),
            set: FixedBitSet::new(),
        }
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
        self.graph.add_system((index, name.clone()));
        for stage in stages {
            let e = self.stage_graph.entry(*stage);
            let g = e.or_default();
            g.add_system((index, name.clone()));
        }
        index
    }
    pub fn initialize(&mut self, world: &mut World) {
        Share::get_mut(&mut self.systems).unwrap().collect();
        // 首先初始化所有的system，有Insert的会产生对应的原型
        for sys in self.systems.iter() {
            sys.initialize(world);
        }
        // todo 遍历world上的单例，测试和system的读写关系
        self.graph.initialize(self.systems.clone(), world);
        for (_name, stage) in self.stage_graph.iter_mut() {
            // println!("stage:{:?} initialize", name);
            stage.initialize(self.systems.clone(), world);
        }
    }
    pub fn run<A: AsyncRuntime + AsyncRuntimeExt>(&mut self, world: &mut World, rt: &A) {
        let g = self.graph.clone();
        self.run_graph(world, rt, g);
    }
    pub fn run_stage<A: AsyncRuntime + AsyncRuntimeExt>(
        &mut self,
        world: &mut World,
        rt: &A,
        stage: &str,
    ) {
        // println!("run_stage, stage:{:?}", stage);
        let g = self.stage_graph.get(stage).unwrap().clone();
        self.run_graph(world, rt, g);
    }
    fn run_graph<A: AsyncRuntime + AsyncRuntimeExt>(
        &mut self,
        world: &mut World,
        rt: &A,
        mut g: ExecGraph,
    ) {
        let w: &'static World = unsafe { std::mem::transmute(world) };
        let s: &'static Share<SafeVec<BoxedSystem>> = unsafe { std::mem::transmute(&self.systems) };
        let rt1 = rt.clone();
        let _ = rt.block_on(async move {
            let rt2 = rt1;
            g.run(s, &rt2, w).await.unwrap();
            g.collect();
        });
    }
    pub async fn async_run<A: AsyncRuntime + AsyncRuntimeExt>(
        &mut self,
        world: &mut World,
        rt: &A,
    ) {
        let g = self.graph.clone();
        self.async_run_graph(world, rt, g).await;
    }
    pub async fn async_run_stage<A: AsyncRuntime + AsyncRuntimeExt>(
        &mut self,
        world: &mut World,
        rt: &A,
        stage: &str,
    ) {
        // println!("async_run_stage, stage:{:?}", stage);
        let g = self.stage_graph.get(stage).unwrap().clone();
        self.async_run_graph(world, rt, g).await;
    }
    async fn async_run_graph<A: AsyncRuntime + AsyncRuntimeExt>(
        &mut self,
        world: &mut World,
        rt: &A,
        mut g: ExecGraph,
    ) {
        world.collect_by(&mut self.action, &mut self.set);
        let w: &'static World = unsafe { std::mem::transmute(world) };
        let s: &'static Share<SafeVec<BoxedSystem>> = unsafe { std::mem::transmute(&self.systems) };
        g.run(s, rt, w).await.unwrap();
        g.collect();
    }
}
