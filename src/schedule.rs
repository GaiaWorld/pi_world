use std::{any::TypeId, borrow::Cow, collections::HashMap};

/// Schedule包含一个主执行器，及多个阶段执行器
///
use crate::{
    archetype::Row,
    exec_graph::{ExecGraph, NodeIndex},
    schedule_config::{
        BaseConfig, NodeType, ScheduleLabel, SetConfig, StageLabel, SystemConfig, SystemSet,
    },
    system::BoxedSystem,
    world::*,
};
use bevy_utils::intern::Interned;
use fixedbitset::FixedBitSet;
use pi_append_vec::SafeVec;
use pi_async_rt::prelude::{AsyncRuntime, AsyncRuntimeExt};
use pi_share::Share;

pub struct Schedule {
    system_configs: Vec<(Interned<dyn StageLabel>, SystemConfig)>,
    systems: Share<SafeVec<(BoxedSystem<()>, Vec<BoxedSystem<bool>>)>>,
    // graph: ExecGraph,
    schedule_graph:
        HashMap<Interned<dyn ScheduleLabel>, HashMap<Interned<dyn StageLabel>, ExecGraph>>,
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
            system_configs: Vec::new(),
            systems: Share::new(SafeVec::default()),
            // graph: ExecGraph::default(),
            schedule_graph: Default::default(),
            stage_sort: vec![
                First.intern(),
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
                before: Vec::new(),
                after: Vec::new(),
            },
            add_listener,
            dirty_mark: false,
        }
    }

    /// 配置系统集
    pub fn configure_set(&mut self, config: SetConfig) {
        log::debug!(
            "configure_set {:?}",
            (
                &config.set,
                &config.config.sets,
                &config.config.before,
                &config.config.after
            )
        );
        for in_set in config.config.sets.iter() {
            if !self.set_configs.contains_key(in_set) {
                self.set_configs.insert(*in_set, BaseConfig::default());
            }
        }

        for set in config.config.after.iter() {
            if let NodeType::Set(set) = set {
                if !self.set_configs.contains_key(set) {
                    self.set_configs.insert(*set, BaseConfig::default());
                }
            }
        }

        for set in config.config.before.iter() {
            if let NodeType::Set(set) = set {
                if !self.set_configs.contains_key(set) {
                    self.set_configs.insert(*set, BaseConfig::default());
                }
            }
        }

        match self.set_configs.entry(config.set) {
            std::collections::hash_map::Entry::Occupied(mut r) => {
                let r = r.get_mut();
                r.sets.extend_from_slice(config.config.sets.as_slice());
                r.schedules
                    .extend_from_slice(config.config.schedules.as_slice());
                r.before.extend_from_slice(config.config.before.as_slice());
                r.after.extend_from_slice(config.config.after.as_slice());
            }
            std::collections::hash_map::Entry::Vacant(r) => {
                r.insert(config.config);
            }
        }
    }

    /// 添加系统
    pub fn add_system(
        &mut self,
        stage_label: Interned<dyn StageLabel>,
        system_config: SystemConfig,
    ) {
        // 添加到系统配置列表中， 延迟处理（在try_initialize中处理）
        self.system_configs.push((stage_label, system_config));
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

        #[cfg(feature = "trace")]
        let update_span = tracing::warn_span!("update").entered();
        // println!("run:{:?}", (schedule, self.schedule_graph.get_mut(schedule).is_some()));
        // let g = self.schedule_graph.get_mut(schedule).unwrap();
        // 每次运行，增加1次tick
        world.increment_tick();
        // 按顺序运行stage
        for stage in self.stage_sort.iter() {
            if let Some(stage) = g.get_mut(stage) {
                Self::run_graph(world, rt, stage, &self.systems);
            }
        }

        #[cfg(feature = "trace")]
        let settle_by = tracing::warn_span!("settle_by").entered();
        if schedule == &MainSchedule.intern() {
            world.settle_by(&mut self.action, &mut self.set);
        }
    }
    fn run_graph<A: AsyncRuntime + AsyncRuntimeExt>(
        world: &mut World,
        rt: &A,
        g: &mut ExecGraph,
        systems: &Share<SafeVec<(BoxedSystem<()>, Vec<BoxedSystem<bool>>)>>,
    ) {
        #[cfg(feature = "trace")]
        let run_span = tracing::warn_span!("run {:?}", name = &g.1).entered();
        let w: &'static World = unsafe { std::mem::transmute(world) };
        let g: &'static mut ExecGraph = unsafe { std::mem::transmute(g) };
        let s: &'static Share<SafeVec<(BoxedSystem<()>, Vec<BoxedSystem<bool>>)>> = unsafe { std::mem::transmute(systems) };
        let rt1 = rt.clone();
        let _ = rt.block_on(async move {
            let rt2 = rt1;
            g.run(s, &rt2, w).await.unwrap();
            #[cfg(feature = "trace")]
            {
                let _collect_span = tracing::warn_span!("settle").entered();
                g.settle();
            }
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
            world.settle_by(&mut self.action, &mut self.set);
        }
    }
    async fn async_run_graph<A: AsyncRuntime + AsyncRuntimeExt>(
        world: &mut World,
        rt: &A,
        g: &mut ExecGraph,
        systems: &Share<SafeVec<(BoxedSystem<()>, Vec<BoxedSystem<bool>>)>>,
    ) {
        let w: &'static World = unsafe { std::mem::transmute(world) };
        let s: &'static Share<SafeVec<(BoxedSystem<()>, Vec<BoxedSystem<bool>>)>> = unsafe { std::mem::transmute(&systems) };
        g.run(s, rt, w).await.unwrap();

        g.settle();
    }

    fn try_initialize(&mut self, world: &mut World) {
        if self.system_configs.is_empty() {
            return;
        }

        let mut temp_map = HashMap::default();
        let mut temp_map2 = HashMap::default();
        let mut system_configs = std::mem::take(&mut self.system_configs);
        let rr = system_configs
            .drain(..)
            .map(|(stage_label, system_config)| {
                (
                    stage_label,
                    self.add_system_config(
                        stage_label,
                        system_config,
                        &mut temp_map,
                        &mut temp_map2,
                    ),
                )
            })
            .collect::<Vec<(Interned<dyn StageLabel>, (BaseConfig, TypeId))>>();

        self.link_set(&mut temp_map, &mut temp_map2);
        for (stage_label, (config, id)) in rr {
            self.link_system_config(stage_label, id, config, &mut temp_map, &mut temp_map2);
        }
        // for (stage_label
        //     self.add_system_config(stage_label, system_config), system_config) in system_configs.drain(..) {
        //     self.add_system_config(stage_label, system_config);
        // }

        Share::get_mut(&mut self.systems).unwrap().settle(0);
        // 首先初始化所有的system，有Insert的会产生对应的原型
        // for sys in self.systems.iter() {
        //     sys.initialize(world);
        // }

        // 初始化图
        for (_name, schedule) in self.schedule_graph.iter_mut() {
            // println!("stage:{:?} initialize", name);
            for (_, stage) in schedule.iter_mut() {
                stage.initialize(self.systems.clone(), world, self.add_listener);
            }
        }

        // println!("schedule initialize");

        self.dirty_mark = false;
    }

    fn add_system_config(
        &mut self,
        stage_label: Interned<dyn StageLabel>,
        system_config: SystemConfig,
        temp_map: &mut HashMap<
            Interned<dyn ScheduleLabel>,
            HashMap<
                Interned<dyn StageLabel>,
                (
                    HashMap<TypeId, NodeIndex>,
                    HashMap<Interned<dyn SystemSet>, ((NodeIndex, bool), (NodeIndex, bool))>,
                ),
            >,
        >,
        temp_map2: &mut HashMap<Interned<dyn SystemSet>, Vec<TypeId>>,
    ) -> (BaseConfig, TypeId) {
        let sys = system_config.system;
        let conditions = system_config.conditions;
        let name = sys.name().clone();
        let id = sys.id();
        let index = self.systems.insert((sys, conditions));
        // 根据配置，添加到对应的派发器中
        Self::add_system_config_inner(
            None,
            None,
            &system_config.config,
            &stage_label,
            index,
            &name,
            id,
            &mut self.schedule_graph,
            temp_map,
            temp_map2,
            &self.set_configs,
        );
        // 添加到主派发器中
        Self::add_system_config_inner(
            None,
            None,
            &self.mian_config,
            &stage_label,
            index,
            &name,
            id,
            &mut self.schedule_graph,
            temp_map,
            temp_map2,
            &self.set_configs,
        );
        (system_config.config, id)
    }

    fn add_system_config_inner(
        set: Option<Interned<dyn SystemSet>>,
        pre_set: Option<Interned<dyn SystemSet>>,
        config: &BaseConfig,
        stage_label: &Interned<dyn StageLabel>,
        index: usize,
        system_name: &Cow<'static, str>,
        system_type_id: std::any::TypeId,
        schedule_graph: &mut HashMap<
            Interned<dyn ScheduleLabel>,
            HashMap<Interned<dyn StageLabel>, ExecGraph>,
        >,
        temp_map: &mut HashMap<
            Interned<dyn ScheduleLabel>,
            HashMap<
                Interned<dyn StageLabel>,
                (
                    HashMap<TypeId, NodeIndex>,
                    HashMap<Interned<dyn SystemSet>, ((NodeIndex, bool), (NodeIndex, bool))>,
                ),
            >,
        >,
        temp_map2: &mut HashMap<Interned<dyn SystemSet>, Vec<TypeId>>,
        set_configs: &HashMap<Interned<dyn SystemSet>, BaseConfig>,
    ) {
        // 如果系统集配置了before或after， 则应该插入set为一个图节点
        if let Some(set) = set {
            let set_map = match temp_map2.entry(set) {
                std::collections::hash_map::Entry::Occupied(r) => r.into_mut(),
                std::collections::hash_map::Entry::Vacant(r) => {
                    // let before_set_index = stage.add_set(format!("{:?}_before", set).into());
                    // let after_set_index = stage.add_set(format!("{:?}_after", set).into());
                    r.insert(vec![])
                }
            };
            // 第一层系统集(递归下去不在push)
            if pre_set.is_none() {
                set_map.push(system_type_id);
            }
        }

        if config.schedules.len() > 0 {
            // println!("add_system_inner:{:?}", &config.schedules);
            // println!("add_system_inner11:{:?}", &schedule_graph.get(&MainSchedule.intern()).is_some());
            for schedule_label in config.schedules.iter() {
                let schedule = match schedule_graph.entry(*schedule_label) {
                    std::collections::hash_map::Entry::Occupied(r) => r.into_mut(),
                    std::collections::hash_map::Entry::Vacant(r) => {
                        temp_map.insert(*schedule_label, Default::default());
                        r.insert(Default::default())
                    }
                };
                let map = temp_map.get_mut(schedule_label).unwrap();

                // let mut stage = schedule.entry(stage_label.clone());
                let stage = match schedule.entry(stage_label.clone()) {
                    std::collections::hash_map::Entry::Occupied(r) => r.into_mut(),
                    std::collections::hash_map::Entry::Vacant(r) => {
                        map.insert(
                            stage_label.clone(),
                            (HashMap::default(), HashMap::default()),
                        );
                        r.insert(ExecGraph::new(format!(
                            "{:?}&{:?}",
                            schedule_label, stage_label
                        )))
                    }
                };
                let map = map.get_mut(stage_label).unwrap();

                // 添加节点
                let node_index = stage.add_system(index, system_name.clone());

                // system 类型id与图节点id映射
                map.0.insert(system_type_id, node_index);
            }
        }

        if config.sets.len() == 0 {
            return;
        }

        // log::warn!("set_configs===={:?}", (set_configs, system_name, config));

        for in_set in config.sets.iter() {
            let config = match set_configs.get(in_set) {
                Some(r) => r,
                None => continue,
            };
            Self::add_system_config_inner(
                Some(*in_set),
                set,
                config,
                &stage_label,
                index,
                &system_name,
                system_type_id,
                schedule_graph,
                temp_map,
                temp_map2,
                set_configs,
            )
        }
    }

    fn link_system_config(
        &mut self,
        stage_label: Interned<dyn StageLabel>,
        id: TypeId,
        config: BaseConfig,
        temp_map: &mut HashMap<
            Interned<dyn ScheduleLabel>,
            HashMap<
                Interned<dyn StageLabel>,
                (
                    HashMap<TypeId, NodeIndex>,
                    HashMap<Interned<dyn SystemSet>, ((NodeIndex, bool), (NodeIndex, bool))>,
                ),
            >,
        >,
        temp_map2: &mut HashMap<Interned<dyn SystemSet>, Vec<TypeId>>,
    ) {
        // 根据配置，添加到对应的派发器中
        Self::link_system_inner(
            &config,
            &config.before,
            &config.after,
            &stage_label,
            id,
            &mut self.schedule_graph,
            temp_map,
            temp_map2,
            &self.set_configs,
        );
        // 添加到主派发器中

        Self::link_system_inner(
            &self.mian_config,
            &config.before,
            &config.after,
            &stage_label,
            id,
            &mut self.schedule_graph,
            temp_map,
            temp_map2,
            &self.set_configs,
        );
    }

    fn get_set_node(
        set: Interned<dyn SystemSet>,
        is_before: bool,
        map: &mut HashMap<Interned<dyn SystemSet>, ((NodeIndex, bool), (NodeIndex, bool))>,
        map2: &HashMap<TypeId, NodeIndex>,
        map3: &HashMap<Interned<dyn SystemSet>, Vec<TypeId>>,
        graph: &mut ExecGraph,
    ) -> NodeIndex {
        // println!("get_set_node===={:?}", (set, is_before));
        let set_nodes = match map.entry(set.clone()) {
            std::collections::hash_map::Entry::Occupied(r) => r.into_mut(),
            std::collections::hash_map::Entry::Vacant(r) => {
                let before_set_index = graph.add_set(format!("{:?}_before", set).into());
                let after_set_index = graph.add_set(format!("{:?}_after", set).into());
                graph.add_edge(before_set_index, after_set_index);
                r.insert(((before_set_index, false), (after_set_index, false)))
            }
        };

        if is_before {
            let r = set_nodes.0;
            if !r.1 {
                if let Some(m) = map3.get(&set) {
                    for i in m.iter() {
                        if let Some(i) = map2.get(i) {
                            graph.add_edge(r.0, *i);
                        }
                    }
                }
            }

            r.0
        } else {
            let r = set_nodes.1;
            if !r.1 {
                if let Some(m) = map3.get(&set) {
                    for i in m.iter() {
                        if let Some(i) = map2.get(i) {
                            graph.add_edge(*i, r.0);
                        }
                    }
                }
            }
            r.0
        }
    }

    // 连接system的边
    fn link_system_inner(
        config: &BaseConfig,
        before: &Vec<NodeType>,
        after: &Vec<NodeType>,
        stage_label: &Interned<dyn StageLabel>,
        system_type_id: std::any::TypeId,
        schedule_graph: &mut HashMap<
            Interned<dyn ScheduleLabel>,
            HashMap<Interned<dyn StageLabel>, ExecGraph>,
        >,
        temp_map: &mut HashMap<
            Interned<dyn ScheduleLabel>,
            HashMap<
                Interned<dyn StageLabel>,
                (
                    HashMap<TypeId, NodeIndex>,
                    HashMap<Interned<dyn SystemSet>, ((NodeIndex, bool), (NodeIndex, bool))>,
                ),
            >,
        >,
        temp_map2: &mut HashMap<Interned<dyn SystemSet>, Vec<TypeId>>,
        set_configs: &HashMap<Interned<dyn SystemSet>, BaseConfig>,
    ) {
        if (before.len() > 0 || after.len() > 0) && config.schedules.len() > 0 {
            // println!("add_system_inner:{:?}", &config.schedules);
            // println!("add_system_inner11:{:?}", &schedule_graph.get(&MainSchedule.intern()).is_some());
            for schedule_label in config.schedules.iter() {
                let schedule = schedule_graph.get_mut(schedule_label).unwrap();
                let map = temp_map.get_mut(schedule_label).unwrap();

                // let mut stage = schedule.entry(stage_label.clone());
                let stage = schedule.get_mut(stage_label).unwrap();
                let map = map.get_mut(stage_label).unwrap();

                // 添加该节点与其他节点的顺序关系
                let node_index = map.0.get(&system_type_id).unwrap().clone();
                if before.len() > 0 {
                    for before in before.iter() {
                        let before_index = match before {
                            NodeType::Set(set) => Self::get_set_node(
                                set.clone(),
                                true,
                                &mut map.1,
                                &map.0,
                                temp_map2,
                                stage,
                            ),
                            NodeType::System(r) => match map.0.get(r) {
                                Some(r) => r.clone(),
                                None => continue,
                            },
                        };
                        stage.add_edge(node_index, before_index);
                    }
                }

                if after.len() > 0 {
                    for after in after.iter() {
                        let after_index = match after {
                            NodeType::Set(set) => Self::get_set_node(
                                set.clone(),
                                false,
                                &mut map.1,
                                &map.0,
                                temp_map2,
                                stage,
                            ),
                            NodeType::System(r) => match map.0.get(r) {
                                Some(r) => r.clone(),
                                None => continue,
                            },
                        };
                        stage.add_edge(after_index, node_index);
                    }
                }
            }
        }

        if config.sets.len() == 0 {
            return;
        }

        // log::warn!("set_configs===={:?}", (set_configs, system_name, config));

        for in_set in config.sets.iter() {
            let config = match set_configs.get(in_set) {
                Some(r) => r,
                None => continue,
            };
            Self::link_system_inner(
                config,
                before,
                after,
                &stage_label,
                system_type_id,
                schedule_graph,
                temp_map,
                temp_map2,
                set_configs,
            )
        }
    }

    // 连接set的边
    fn link_set(
        &mut self,
        temp_map: &mut HashMap<
            Interned<dyn ScheduleLabel>,
            HashMap<
                Interned<dyn StageLabel>,
                (
                    HashMap<TypeId, NodeIndex>,
                    HashMap<Interned<dyn SystemSet>, ((NodeIndex, bool), (NodeIndex, bool))>,
                ),
            >,
        >,
        temp_map2: &HashMap<Interned<dyn SystemSet>, Vec<TypeId>>,
    ) {
        for (set, config) in self.set_configs.iter() {
            for (schedule_label, schedule) in self.schedule_graph.iter_mut() {
                let map = temp_map.get_mut(schedule_label).unwrap();
                for (stage_label, stage) in schedule.iter_mut() {
                    let map = map.get_mut(stage_label).unwrap();

                    let set_before =
                        Self::get_set_node(set.clone(), true, &mut map.1, &map.0, temp_map2, stage);
                    let set_after = Self::get_set_node(
                        set.clone(),
                        false,
                        &mut map.1,
                        &map.0,
                        temp_map2,
                        stage,
                    );

                    for in_set in config.sets.iter() {
                        let in_set_before = Self::get_set_node(
                            in_set.clone(),
                            true,
                            &mut map.1,
                            &map.0,
                            temp_map2,
                            stage,
                        );
                        let in_set_after = Self::get_set_node(
                            in_set.clone(),
                            false,
                            &mut map.1,
                            &map.0,
                            temp_map2,
                            stage,
                        );
                        stage.add_edge(in_set_before, set_before);
                        stage.add_edge(set_after, in_set_after);
                    }

                    if config.before.len() > 0 {
                        for before in config.before.iter() {
                            let before_index = match before {
                                NodeType::Set(set) => Self::get_set_node(
                                    set.clone(),
                                    true,
                                    &mut map.1,
                                    &map.0,
                                    temp_map2,
                                    stage,
                                ),
                                NodeType::System(r) => map.0.get(r).unwrap().clone(),
                            };
                            stage.add_edge(set_after, before_index);
                        }
                    }

                    if config.after.len() > 0 {
                        for after in config.after.iter() {
                            let after_index = match after {
                                NodeType::Set(set) => Self::get_set_node(
                                    set.clone(),
                                    false,
                                    &mut map.1,
                                    &map.0,
                                    temp_map2,
                                    stage,
                                ),
                                NodeType::System(r) => match map.0.get(r) {
                                    Some(r) => r.clone(),
                                    None => continue,
                                },
                            };
                            stage.add_edge(after_index, set_before);
                        }
                    }
                }
            }
        }
    }
}

/// 只运行一次的system
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Startup;

#[derive(StageLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct First;

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
