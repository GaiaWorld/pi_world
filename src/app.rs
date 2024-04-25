//! App包含一个world，一个调度器，及一个运行时
//!
//!
use pi_async_rt::rt::{
    multi_thread::{MultiTaskRuntime, MultiTaskRuntimeBuilder, StealableTaskPool},
    single_thread::{SingleTaskPool, SingleTaskRunner, SingleTaskRuntime},
    AsyncRuntime, AsyncRuntimeExt,
};

use crate::{schedule::{MainSchedule, Schedule, Startup}, schedule_config::{IntoSystemConfigs, IntoSystemSetConfigs, ScheduleLabel, StageLabel}, world::World};

pub type SingleThreadApp = App<SingleTaskRuntime>;
pub type MultiThreadApp = App<MultiTaskRuntime>;

pub struct App<A: AsyncRuntime + AsyncRuntimeExt> {
    pub world: World,
    pub schedule: Schedule,
    pub startup_schedule: Schedule,
    pub rt: A,
}
impl App<SingleTaskRuntime> {
    pub fn new() -> Self {
        let pool = SingleTaskPool::default();
        let rt = SingleTaskRunner::<(), SingleTaskPool<()>>::new(pool).into_local();
        App {
            world: World::new(),
            schedule: Schedule::new(true),
            startup_schedule: Schedule::new(false),
            rt,
        }
    }
}
impl App<MultiTaskRuntime> {
    pub fn new() -> Self {
        let pool = StealableTaskPool::with(4, 100000, [1, 254], 3000);
        let builer = MultiTaskRuntimeBuilder::new(pool)
            .set_timer_interval(1)
            .init_worker_size(4)
            .set_worker_limit(4, 4);
        let rt = builer.build();
        App {
            world: World::new(),
            schedule: Schedule::new(true),
            startup_schedule: Schedule::new(false),
            rt,
        }
    }
}
impl<A: AsyncRuntime + AsyncRuntimeExt> App<A> {

    /// 配置系统集
    pub fn configure_set(&mut self, _stage_label: impl StageLabel, config: impl IntoSystemSetConfigs) {
        self.schedule.configure_set(config.into_configs());
    }

    // 添加system
    pub fn add_systems<M>(&mut self, stage_label: impl StageLabel, system: impl IntoSystemConfigs<M>) -> usize {
        let stage_label = stage_label.intern();
        let system_config = system.into_configs();
                
        if stage_label == Startup.intern() {
            self.startup_schedule.add_system(stage_label, system_config)
        } else {
            self.schedule.add_system(stage_label, system_config)
        }
    }

    /// 同步运行日程
    /// schedule_label为None时， 表示运行所有的system
    /// 否则运行指定日程中的system
    pub fn run(&mut self, schedule_label: Option<impl ScheduleLabel>) {
        self.startup_schedule.run(&mut self.world, &self.rt, &MainSchedule.intern());

        let schedule_label = match schedule_label {
            Some(r) => r.intern(),
            None => MainSchedule.intern(),
        };
        self.schedule.run(&mut self.world, &self.rt, &schedule_label);
    }

    /// 异步运行日程
    /// schedule_label为None时， 表示运行所有的system
    /// 否则运行指定日程中的system
    pub async fn async_run(&mut self, schedule_label: Option<impl ScheduleLabel>) {
        self.startup_schedule.async_run(&mut self.world, &self.rt, &MainSchedule.intern()).await;

        let schedule_label = match schedule_label {
            Some(r) => r.intern(),
            None => MainSchedule.intern(),
        };
        self.schedule.async_run(&mut self.world, &self.rt, &schedule_label).await;
    }
}

