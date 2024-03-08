//! App包含一个world，一个调度器，及一个运行时
//!
//!
use pi_async_rt::rt::{
    multi_thread::{MultiTaskRuntime, MultiTaskRuntimeBuilder, StealableTaskPool},
    single_thread::{SingleTaskPool, SingleTaskRunner, SingleTaskRuntime},
    AsyncRuntime, AsyncRuntimeExt,
};

use crate::{schedule::Schedule, world::World};

pub type SingleThreadApp = App<SingleTaskRuntime>;
pub type MultiThreadApp = App<MultiTaskRuntime>;

pub struct App<A: AsyncRuntime + AsyncRuntimeExt> {
    pub world: World,
    pub schedule: Schedule,
    pub rt: A,
}
impl App<SingleTaskRuntime> {
    pub fn new() -> Self {
        let pool = SingleTaskPool::default();
        let rt = SingleTaskRunner::<(), SingleTaskPool<()>>::new(pool).into_local();
        App {
            world: World::new(),
            schedule: Schedule::new(),
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
            schedule: Schedule::new(),
            rt,
        }
    }
}
impl<A: AsyncRuntime + AsyncRuntimeExt> App<A> {
    pub fn initialize(&mut self) {
        self.schedule.initialize(&mut self.world);
    }

    pub fn run(&mut self) {
        self.schedule.run(&mut self.world, &self.rt);
    }

    pub fn run_stage(&mut self, stage: &str) {
        self.schedule.run_stage(&mut self.world, &self.rt, stage);
    }
    pub async fn async_run(&mut self) {
        self.schedule.async_run(&mut self.world, &self.rt).await;
    }
    pub async fn async_run_stage(&mut self, stage: &str) {
        self.schedule
            .async_run_stage(&mut self.world, &self.rt, stage)
            .await;
    }
}
