//! App包含一个world，一个调度器，及一个运行时
//!
//!
use pi_async_rt::prelude::{AsyncRuntime, AsyncRuntimeExt};

use crate::{schedule::{MainSchedule, Schedule}, schedule_config::{IntoSystemConfigs, IntoSystemSetConfigs, ScheduleLabel, StageLabel}, world::World};


pub type App = AppInner<runtime::Runtime>;

pub struct AppInner<A: AsyncRuntime + AsyncRuntimeExt> {
    pub world: World,
    pub schedule: Schedule,
    pub startup_schedule: Schedule,
    pub rt: A,
    pub is_first_run: bool,
}
impl AppInner<runtime::Runtime> {
    pub fn new() -> Self {
        App {
            world: World::new(),
            schedule: Schedule::new(true),
            startup_schedule: Schedule::new(false),
            rt: runtime::create_runtime(),
            is_first_run: true,
        }
    }
}

impl<A: AsyncRuntime + AsyncRuntimeExt> AppInner<A> {

    /// 配置系统集
    pub fn configure_set(&mut self, _stage_label: impl StageLabel, config: impl IntoSystemSetConfigs) -> &mut Self {
        self.schedule.configure_set(config.into_configs());
        self
    }

    // 添加system
    pub fn add_system<M>(&mut self, stage_label: impl StageLabel, system: impl IntoSystemConfigs<M>) -> &mut Self {
        let stage_label = stage_label.intern();
        let system_config = system.into_configs();
                
        self.schedule.add_system(stage_label, system_config);
        self
    }

    // 添加system
    pub fn add_startup_system<M>(&mut self, stage_label: impl StageLabel, system: impl IntoSystemConfigs<M>) -> &mut Self {
        let stage_label = stage_label.intern();
        let system_config = system.into_configs();
                
        self.startup_schedule.add_system(stage_label, system_config);
        self
    }

    /// 同步运行日程
    /// schedule_label为None时， 表示运行所有的system
    /// 否则运行指定日程中的system
    pub fn run(&mut self) {
        // for item in self.world.archetype_arr.iter(){
        //     println!("item: {:?}", item);
        // }
        if self.is_first_run {
            self.startup_schedule.run(&mut self.world, &self.rt, &MainSchedule.intern());
            self.is_first_run = false;
        }
        
        self.schedule.run(&mut self.world, &self.rt, &MainSchedule.intern());
    }

    /// 同步运行日程
    /// schedule_label为None时， 表示运行所有的system
    /// 否则运行指定日程中的system
    pub fn run_schedule(&mut self, schedule_label: impl ScheduleLabel) {
        if self.is_first_run {
            self.startup_schedule.run(&mut self.world, &self.rt, &MainSchedule.intern());
            self.is_first_run = false;
        }

        self.schedule.run(&mut self.world, &self.rt, &schedule_label.intern());
    }

    /// 异步运行日程
    /// schedule_label为None时， 表示运行所有的system
    /// 否则运行指定日程中的system
    pub async fn async_run(&mut self, schedule_label: impl ScheduleLabel) {
        if self.is_first_run {
            self.startup_schedule.async_run(&mut self.world, &self.rt, &MainSchedule.intern()).await;
            self.is_first_run = false;
        }

        self.schedule.async_run(&mut self.world, &self.rt, &schedule_label.intern()).await;
    }

    /// 异步运行日程
    /// schedule_label为None时， 表示运行所有的system
    /// 否则运行指定日程中的system
    pub async fn async_run_schedule(&mut self) {
        if self.is_first_run {
            self.startup_schedule.async_run(&mut self.world, &self.rt, &MainSchedule.intern()).await;
            self.is_first_run = false;
        }

        self.schedule.async_run(&mut self.world, &self.rt, &MainSchedule.intern()).await;
    }
}

#[cfg(not(any(feature="single_thread", target_arch = "wasm32")))]
mod runtime {
    pub use pi_async_rt::rt::multi_thread::MultiTaskRuntime as Runtime;
    use pi_async_rt::rt::multi_thread::{MultiTaskRuntimeBuilder, StealableTaskPool};


    use crate::{schedule::Schedule, world::World};

    pub fn create_runtime() -> Runtime {
        let pool = StealableTaskPool::with(4, 100000, [1, 254], 3000);
        let builer = MultiTaskRuntimeBuilder::new(pool)
            .set_timer_interval(1)
            .init_worker_size(4)
            .set_worker_limit(4, 4);
        builer.build()
    }
}

#[cfg(all(feature="single_thread", not(target_arch = "wasm32")))]
mod runtime {
    pub use pi_async_rt::prelude::SingleTaskRuntime as Runtime;
    use pi_async_rt::prelude::{SingleTaskPool, SingleTaskRunner};

    pub fn create_runtime() -> Runtime {
        let pool = SingleTaskPool::default();
        SingleTaskRunner::<(), SingleTaskPool<()>>::new(pool).into_local()
    }
}

#[cfg(target_arch = "wasm32")]
mod runtime {
    pub use pi_async_rt::rt::serial_local_compatible_wasm_runtime::LocalTaskRuntime as Runtime;
    use pi_async_rt::rt::serial_local_compatible_wasm_runtime::LocalTaskRunner;

    pub fn create_runtime() -> Runtime {
        let runner = LocalTaskRunner::new();
        runner.get_runtime()
    }
}
