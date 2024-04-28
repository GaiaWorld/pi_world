use std::{borrow::Cow, collections::HashMap};

use fixedbitset::FixedBitSet;
use pi_async_rt::rt::{AsyncRuntime, AsyncRuntimeExt};
use pi_share::Share;
use bevy_utils::intern::Interned;
/// Schedule包含一个主执行器，及多个阶段执行器
///
use crate::{
    archetype::Row, exec_graph::ExecGraph, safe_vec::SafeVec, schedule_config::{BaseConfig, ScheduleLabel, SetConfig, StageLabel, SystemConfig, SystemSet}, system::BoxedSystem, world::*
};

pub struct Schedule {
    systems: Share<SafeVec<BoxedSystem>>,
    // graph: ExecGraph,
    schedule_graph: HashMap<Interned<dyn ScheduleLabel>, HashMap<Interned<dyn StageLabel>, ExecGraph>>,
    // 阶段执行顺序
    stage_sort: Vec<Interned<dyn StageLabel>>,
    action: Vec<(Row, Row)>,
    set: FixedBitSet,

    set_configs: HashMap<Interned<dyn SystemSet>, BaseConfig>,
    mian_config: BaseConfig,

    add_listener: bool,

    dirty_mark: bool,

}

impl Schedule {
    pub fn new(add_listener: bool) -> Self {
        Self {
            systems: Share::new(SafeVec::default()),
            // graph: ExecGraph::default(),
            schedule_graph: Default::default(),
            stage_sort: vec![
                PreUpdate.intern(),
                Update.intern(),
                PostUpdate.intern(),
                Last.intern(),
            ],

            action: Vec::new(),
            set: FixedBitSet::new(),

            set_configs: HashMap::new(),
            mian_config: BaseConfig {
                sets: Default::default(),
                schedules: vec![MainSchedule.intern()],
            },
            add_listener,
            dirty_mark: false,
        }
    }


    pub fn get_systems(&self) -> &Share<SafeVec<BoxedSystem>> {
        &self.systems
    }
    // pub fn get_graph(&self) -> &ExecGraph {
    //     &self.graph
    // }
    // pub fn get_stage_graph(&self, name: &Interned<dyn StageLabel>) -> Option<&ExecGraph> {
    //     self.stage_graph.get(name)
    // }
    // pub fn add_system<M>(&mut self, stage: Interned<dyn StageLabel>, system: BoxedSystem) -> usize {
    //     self.add_box_system(system, stages)
    // }
    // pub fn add_system_stages<M>(
    //     &mut self,
    //     system: impl IntoSystem<M>,
    //     stages: &[&'static str],
    // ) -> usize {
    //     let s = Box::new(IntoSystem::into_system(system));
    //     self.add_box_system(BoxedSystem::Sync(s), stages)
    // }
    // pub fn add_async_system<M>(&mut self, system: impl IntoAsyncSystem<M>) -> usize {
    //     self.add_async_system_stages(system, &[])
    // }
    // pub fn add_async_system_stages<M>(
    //     &mut self,
    //     system: impl IntoAsyncSystem<M>,
    //     stages: &[&'static str],
    // ) -> usize {
    //     let s = Box::new(IntoAsyncSystem::into_async_system(system));
    //     self.add_box_system(BoxedSystem::Async(s), stages)
    // }

    // pub fn add_system(&mut self, system: BoxedSystem) -> usize {
    //     todo!()
    //     // let name = system.name().clone();
    //     // let index = self.systems.insert(system);
    //     // // self.graph.add_system(index, name.clone());
    //     // for stage in stages {
    //     //     let e = self.stage_graph.entry(*stage);
    //     //     let g = e.or_default();
    //     //     g.add_system(index, name.clone());
    //     // }
    //     // index
    // }

    pub fn configure_set(&mut self, config: SetConfig) {
        match self.set_configs.entry(config.set) {
            std::collections::hash_map::Entry::Occupied(mut r) => {
                let r = r.get_mut();
                r.sets.extend_from_slice(config.config.sets.as_slice());
                r.schedules.extend_from_slice(config.config.schedules.as_slice());
            },
            std::collections::hash_map::Entry::Vacant(r) => {
                r.insert(config.config);
            },
        }
    }

    pub fn add_system(&mut self, stage_label: Interned<dyn StageLabel>, system_config: SystemConfig) -> usize {
        let sys = system_config.system;
        let name = sys.name().clone(); 
        let index = self.systems.insert(sys);
        // 根据配置，添加到对应的派发器中
        Self::add_system_inner(&system_config.config, &stage_label, index, &name, &mut self.schedule_graph, &self.set_configs);
        // 添加到主派发器中
        Self::add_system_inner(&self.mian_config, &stage_label, index, &name, &mut self.schedule_graph, &self.set_configs);

        // 设置脏
        self.dirty_mark = true;
        index
    }

    fn add_system_inner(
        config: &BaseConfig, 
        stage_label: &Interned<dyn StageLabel>, 
        index: usize, 
        name: &Cow<'static, str>,
        schedule_graph: &mut HashMap<Interned<dyn ScheduleLabel>, HashMap<Interned<dyn StageLabel>, ExecGraph>>,
        set_configs: &HashMap<Interned<dyn SystemSet>, BaseConfig>,
    ) {
        if config.schedules.len() > 0 {
            // println!("add_system_inner:{:?}", &config.schedules);
            // println!("add_system_inner11:{:?}", &schedule_graph.get(&MainSchedule.intern()).is_some());
            for schedule_label in config.schedules.iter() {
                let schedule = schedule_graph.entry(*schedule_label);
                let schedule = schedule.or_default();
                
                let stage = schedule.entry(stage_label.clone());
                let stage = stage.or_default();

                stage.add_system(index, name.clone());

                // println!("add_system_inner:{:?}", (schedule_label, &schedule_graph.get(&MainSchedule.intern()).is_some()));
            }
        }

        if config.sets.len() == 0 {
            return;
        }

        for config in set_configs.values() {
            Self::add_system_inner(config, &stage_label, index, &name, schedule_graph, set_configs)
        }
    }

    pub fn try_initialize(&mut self, world: &mut World) {
        if self.dirty_mark { // 偶数表示不脏
           return;
        }
        Share::get_mut(&mut self.systems).unwrap().collect();
        // 首先初始化所有的system，有Insert的会产生对应的原型
        for sys in self.systems.iter() {
            sys.initialize(world);
        }
        
        // 初始化图
        for (_name, schedule) in self.schedule_graph.iter_mut() {
            // println!("stage:{:?} initialize", name);
            for (_, stage) in schedule.iter_mut() {
                stage.initialize(self.systems.clone(), world, self.add_listener);
            }
        }


        self.dirty_mark = true;
    }

    pub fn run<A: AsyncRuntime + AsyncRuntimeExt>(
        &mut self,
        world: &mut World,
        rt: &A,
        schedule: &Interned<dyn ScheduleLabel>,
    ) {
        // println!("run:{:?}", (schedule, self.schedule_graph.get_mut(schedule).is_some(), self.schedule_graph.len()));
        self.try_initialize(world);

        let g = match self.schedule_graph.get_mut(schedule) {
            Some(r) => r,
            None => return,
        };

        // println!("run:{:?}", (schedule, self.schedule_graph.get_mut(schedule).is_some()));
        // let g = self.schedule_graph.get_mut(schedule).unwrap();

        // 按顺序运行stage
        for stage in self.stage_sort.iter() {
            if let Some(stage) = g.get_mut(stage) {
                Self::run_graph(world, rt, stage, &self.systems);
            }
        }

        if schedule == &MainSchedule.intern() {
            world.collect_by(&mut self.action, &mut self.set);
        }
    }
    fn run_graph<A: AsyncRuntime + AsyncRuntimeExt>(
        world: &mut World,
        rt: &A,
        g: &mut ExecGraph,
        systems: &Share<SafeVec<BoxedSystem>>
    ) {
        let w: &'static World = unsafe { std::mem::transmute(world) };
        let g: &'static mut ExecGraph = unsafe { std::mem::transmute(g) };
        let s: &'static Share<SafeVec<BoxedSystem>> = unsafe { std::mem::transmute(systems) };
        let rt1 = rt.clone();
        let _ = rt.block_on(async move {
            let rt2 = rt1;
            g.run(s, &rt2, w).await.unwrap();
            g.collect();
        });
    }
    // pub async fn async_run<A: AsyncRuntime + AsyncRuntimeExt>(
    //     &mut self,
    //     world: &mut World,
    //     rt: &A,
    // ) {
    //     let g = self.graph.clone();
    //     self.async_run_graph(world, rt, g).await;
    // }
    pub async fn async_run<A: AsyncRuntime + AsyncRuntimeExt>(
        &mut self,
        world: &mut World,
        rt: &A,
        schedule: &Interned<dyn ScheduleLabel>,
    ) {
        self.try_initialize(world);

        // println!("async_run_stage, stage:{:?}", stage);
        let g = self.schedule_graph.get_mut(schedule).unwrap();
        // 按顺序运行stage
        for stage in self.stage_sort.iter() {
            if let Some(stage) = g.get_mut(stage) {
                Self::async_run_graph(world, rt, stage, &mut self.systems).await;
            }
        }

        if schedule == &MainSchedule.intern() {
            world.collect_by(&mut self.action, &mut self.set);
        }
    }
    async fn async_run_graph<A: AsyncRuntime + AsyncRuntimeExt>(
        world: &mut World,
        rt: &A,
        g: &mut ExecGraph,
        systems: &Share<SafeVec<BoxedSystem>>,
    ) { 
        let w: &'static World = unsafe { std::mem::transmute(world) };
        let s: &'static Share<SafeVec<BoxedSystem>> = unsafe { std::mem::transmute(&systems) };
        g.run(s, rt, w).await.unwrap();

        g.collect();
    }
}

/// 只运行一次的system
#[derive(StageLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Startup;



#[derive(StageLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PreUpdate;

#[derive(StageLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Update;

#[derive(StageLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PostUpdate;

#[derive(StageLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Last;

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct MainSchedule;


#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SystemSet1;