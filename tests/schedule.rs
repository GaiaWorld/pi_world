use pi_world::{prelude::App, schedule::Update, schedule_config::{IntoSystemConfigs, IntoSystemSetConfigs}, single_res::SingleResMut};
use pi_world_macros::SystemSet;
use pi_world::schedule_config;

#[test]
fn test_set_condition() {
    let mut app = App::new();
    app.world.insert_single_res(RunSystem::default());

    app.add_system(Update, system1.in_set(Set::Set1));
    app.add_system(Update, system2.in_set(Set::Set1));
    app.configure_set(Update, Set::Set1.run_if(condition_true).run_if(condition_false));
    
    app.add_system(Update, system3.in_set(Set::Set3));
    app.add_system(Update, system4.in_set(Set::Set3));
    app.configure_set(Update, Set::Set3.run_if(condition_true).run_if(condition_true));

    app.add_system(Update, system5.in_set(Set::Set5));
    app.configure_set(Update, Set::Set5.run_if(condition_true).run_if(condition_true));

    app.add_system(Update, system6.in_set(Set::Set6));
    app.configure_set(Update, Set::Set6.run_if(condition_false));

    app.add_system(Update, system7.in_set(Set::Set7));
    app.configure_set(Update, Set::Set7.run_if(condition_false).run_if(condition_true));

    app.run();

    let run_systems = &**app.world.get_single_res::<RunSystem>().unwrap();
    debug_assert_eq!(run_systems.0.iter().position(|r| {r == &"system1"}).is_some(), false);
    debug_assert_eq!(run_systems.0.iter().position(|r| {r == &"system2"}).is_some(), false);
    debug_assert_eq!(run_systems.0.iter().position(|r| {r == &"system3"}).is_some(), true);
    debug_assert_eq!(run_systems.0.iter().position(|r| {r == &"system4"}).is_some(), true);
    debug_assert_eq!(run_systems.0.iter().position(|r| {r == &"system5"}).is_some(), true);
    debug_assert_eq!(run_systems.0.iter().position(|r| {r == &"system6"}).is_some(), false);
    debug_assert_eq!(run_systems.0.iter().position(|r| {r == &"system7"}).is_some(), false);

    // println!("run systems: {:?}", );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum Set {
    Set1,
    Set2,
    Set3,
    Set4,
    Set5,
    Set6,
    Set7,
}

pub fn condition_true() -> bool {
    return true;
}

pub fn condition_false() -> bool {
    return false;
}

#[derive(Debug, Default)]
pub struct RunSystem(Vec<&'static str>);
pub fn system1(mut run_system: SingleResMut<RunSystem>) {
    run_system.0.push("system1");
}

pub fn system2(mut run_system: SingleResMut<RunSystem>) {
    run_system.0.push("system2");
}

pub fn system3(mut run_system: SingleResMut<RunSystem>) {
    run_system.0.push("system3");
}

pub fn system4(mut run_system: SingleResMut<RunSystem>) {
    run_system.0.push("system4");
}

pub fn system5(mut run_system: SingleResMut<RunSystem>) {
    run_system.0.push("system5");
}

pub fn system6(mut run_system: SingleResMut<RunSystem>) {
    run_system.0.push("system6");
}

pub fn system7(mut run_system: SingleResMut<RunSystem>) {
    run_system.0.push("system7");
}