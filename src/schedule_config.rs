use std::{any::TypeId, hash::Hash};

use bevy_utils::{define_label, intern::Interned};

use crate::system::{BoxedSystem, IntoSystem};
pub use bevy_utils::label::{DynEq, DynHash};
pub use pi_world_macros::{ScheduleLabel, StageLabel, SystemSet};


define_label!(
    SystemSet,
    SYSTEM_SET_INTERNER
);

define_label!(
    StageLabel,
    STAGE_LABEL_INTERNER
);

define_label!(
    ScheduleLabel,
    SCHEDULE_LABEL_INTERNER
);

pub trait IntoNodeType<Marker>
where
    Self: Sized,
{
    fn into_node_type(self) -> NodeType;
}


/****************************系统集配置********************************/
pub trait IntoSystemSetConfigs
where
    Self: Sized,
{
    fn into_configs(self) -> SetConfig;

    /// 设置所在的系统集
    fn in_set(self, set: impl SystemSet) -> SetConfig {
        self.into_configs().in_set(set)
    }

    /// 设置可在哪个日程中运行
    fn in_schedule(self, schedule: impl ScheduleLabel) -> SetConfig {
        self.into_configs().in_schedule(schedule)
    }

    fn before<M>(self, after: impl IntoNodeType<M>) -> SetConfig {
        self.into_configs().before(after)
    }

    fn after<M>(self, before: impl IntoNodeType<M>) -> SetConfig {
        self.into_configs().after(before)
    }
}

pub struct SetConfig {
    pub set: Interned<dyn SystemSet>,
    pub(crate) config: BaseConfig,
}

impl IntoSystemSetConfigs for SetConfig {
    fn into_configs(self) -> SetConfig {
        self
    }

    fn in_set(mut self, set: impl SystemSet) -> SetConfig {
        self.config.sets.push(set.intern());
        self
    }

    fn in_schedule(mut self, schedule: impl ScheduleLabel) -> SetConfig {
        self.config.schedules.push(schedule.intern());
        self
    }

    fn before<M>(mut self, after: impl IntoNodeType<M>) -> SetConfig {
        self.config.before.push(after.into_node_type());
        self
    }

    fn after<M>(mut self, before: impl IntoNodeType<M>) -> SetConfig {
        self.config.after.push(before.into_node_type());
        self
    }
}

impl<T: SystemSet> IntoNodeType<()> for T {
    fn into_node_type(self) -> NodeType {
        NodeType::Set(self.intern())
    }
}

impl<T: SystemSet> IntoSystemSetConfigs for T {
    fn into_configs(self) -> SetConfig {
        SetConfig{
            set: self.intern(),
            config: BaseConfig {
                sets: Vec::new(),
                schedules: Vec::new(),
                before: Vec::new(),
                after: Vec::new(),
            },
        }
        
    }
}


/****************************系统配置********************************/
pub trait IntoSystemConfigs<Marker>
where
    Self: Sized,
{
    fn into_configs(self) -> SystemConfig;

    /// 设置所在的系统集
    fn in_set(self, set: impl SystemSet) -> SystemConfig {
        self.into_configs().in_set(set)
    }

    /// 设置可在哪个日程中运行
    fn in_schedule(self, schedule: impl ScheduleLabel) -> SystemConfig {
        self.into_configs().in_schedule(schedule)
    }

    fn before<M>(self, after: impl IntoNodeType<M>) -> SystemConfig {
        self.into_configs().before(after)
    }

    fn after<M>(self, before: impl IntoNodeType<M>) -> SystemConfig {
        self.into_configs().after(before)
    }
}

pub struct SystemConfig {
    pub(crate) system: BoxedSystem,
    pub(crate) config: BaseConfig,
}

impl IntoSystemConfigs<()> for SystemConfig {
    #[inline]
    fn into_configs(self) -> SystemConfig {
        self
    }
    
    fn in_set(mut self, set: impl SystemSet) -> SystemConfig {
        self.config.sets.push(set.intern());
        self
    }
    
    fn in_schedule(mut self, schedule: impl ScheduleLabel) -> SystemConfig {
        self.config.schedules.push(schedule.intern());
        self
    }

    fn before<M>(mut self, after: impl IntoNodeType<M>) -> SystemConfig {
        self.config.before.push(after.into_node_type());
        self
    }

    fn after<M>(mut self, before: impl IntoNodeType<M>) -> SystemConfig {
        self.config.after.push(before.into_node_type());
        self
    }
}

impl<Marker, T: IntoSystem<Marker>> IntoSystemConfigs<Marker> for T  {
    fn into_configs(self) -> SystemConfig {
        SystemConfig {
            system: BoxedSystem::Sync(Box::new(self.into_system())),
            config: BaseConfig {
                sets: Vec::new(),
                schedules: Vec::new(),
                before: Vec::new(),
                after: Vec::new(),
            },
        }
    }
}

impl<Marker, T: IntoSystem<Marker> + 'static> IntoNodeType<(usize, Marker)> for T {
    fn into_node_type(self) -> NodeType {
        NodeType::System(TypeId::of::<Self>())
    }
}


#[derive(Debug, Default)]
pub struct BaseConfig {
    pub(crate) sets: Vec<Interned<dyn SystemSet>>,
    pub(crate) schedules: Vec<Interned<dyn ScheduleLabel>>, // 需要添加到哪些日程中
    pub(crate) before: Vec<NodeType>, // 节点顺序
    pub(crate) after: Vec<NodeType>, // 节点顺序
}

#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub enum NodeType {
    Set(Interned<dyn SystemSet>),
    System(std::any::TypeId),
}



