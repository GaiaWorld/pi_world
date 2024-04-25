use bevy_utils::{define_label, intern::Interned};

use crate::system::{BoxedSystem, IntoSystem};
pub use bevy_utils::label::{DynEq, DynHash};


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
}

impl<T: SystemSet> IntoSystemSetConfigs for T {
    fn into_configs(self) -> SetConfig {
        SetConfig{
            set: self.intern(),
            config: BaseConfig {
                sets: Vec::new(),
                schedules: Vec::new(),
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
}

impl<Marker, T: IntoSystem<Marker>> IntoSystemConfigs<Marker> for T  {
    fn into_configs(self) -> SystemConfig {
        SystemConfig {
            system: BoxedSystem::Sync(Box::new(self.into_system())),
            config: BaseConfig {
                sets: Vec::new(),
                schedules: Vec::new(),
            },
        }
    }
}


pub struct BaseConfig {
    pub(crate) sets: Vec<Interned<dyn SystemSet>>,
    pub(crate) schedules: Vec<Interned<dyn ScheduleLabel>>, // 需要添加到哪些日程中
}