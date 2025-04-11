use pi_world::{debug::{ArchetypeDebug, ColumnDebug}, prelude::{App, Component, Entity, Query}, schedule::Update, schedule_config::{IntoSystemConfigs, IntoSystemSetConfigs}, single_res::SingleResMut};
use pi_world_macros::SystemSet;

#[test]
fn test_schedule() {
    #[derive(Component)]
    struct A(f32);
    #[derive(Component)]
    struct B(f32);
    #[derive(Component)]
    struct C(f32);
    #[derive(Component)]
    struct D(f32);
    #[derive(Component)]
    struct E(f32);

    fn ab(query: Query<(&mut A, &mut B)>) {
        for (mut a, mut b) in query.iter_mut() {
            std::mem::swap(&mut a.0, &mut b.0);
        }
    }

    fn cd(query: Query<(&mut C, &mut D)>) {
        for (mut c, mut d) in query.iter_mut() {
            std::mem::swap(&mut c.0, &mut d.0);
        }
    }

    fn ce(query: Query<(&mut C, &mut E)>) {
        for (mut c, mut e) in query.iter_mut() {
            std::mem::swap(&mut c.0, &mut e.0);
        }
    }
    let mut app = pi_world::prelude::App::new();
    let i = app.world.make_insert::<(A, B)>();
    let it = (0..10_000).map(|_| (A(0.0), B(0.0)));
    let _ = i.batch(&app.world, it).collect::<Vec<Entity>>();

    let i = app.world.make_insert::<(A, B, C)>();
    let it = (0..10_000).map(|_| (A(0.0), B(0.0), C(0.0)));
    let _ = i.batch(&app.world, it).collect::<Vec<Entity>>();

    let i = app.world.make_insert::<(A, B, C, D)>();
    let it = (0..10_000).map(|_| (A(0.0), B(0.0), C(0.0), D(0.0)));
    let _ = i.batch(&app.world, it).collect::<Vec<Entity>>();

    let i = app.world.make_insert::<(A, B, C, E)>();
    let it = (0..10_000).map(|_| (A(0.0), B(0.0), C(0.0), E(0.0)));
    let _ = i.batch(&app.world, it).collect::<Vec<Entity>>();

    app.world.settle();
    app.add_system(Update, ab);
    app.add_system(Update, cd);
    app.add_system(Update, ce);
    
    let mut info = ArchetypeDebug {
        entitys: Some(10000),
        columns_info: vec![
            Some(ColumnDebug{change_listeners: 0, name: Some("A")}), 
            Some(ColumnDebug{change_listeners: 0, name: Some("B")}), 
        ],
        destroys_listeners: Some(0),
        removes: Some(0),
    };
    app.world.assert_archetype_arr(&[None, Some(info.clone()), None, None, None,]);

    app.run();

    info.columns_info = vec![
        Some(ColumnDebug{change_listeners: 0, name: Some("A")}), 
        Some(ColumnDebug{change_listeners: 0, name: Some("B")}), 
        Some(ColumnDebug{change_listeners: 0, name: Some("C")}), 
    ];

    app.world.assert_archetype_arr(&[None, None, Some(info.clone()), None, None,]);

    for _ in 0..1000 {
        app.run();
    }
}

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
pub fn system1(run_system: SingleResMut<RunSystem>) {
    run_system.0.push("system1");
}

pub fn system2(run_system: SingleResMut<RunSystem>) {
    run_system.0.push("system2");
}

pub fn system3(run_system: SingleResMut<RunSystem>) {
    run_system.0.push("system3");
}

pub fn system4(run_system: SingleResMut<RunSystem>) {
    run_system.0.push("system4");
}

pub fn system5(run_system: SingleResMut<RunSystem>) {
    run_system.0.push("system5");
}

pub fn system6(run_system: SingleResMut<RunSystem>) {
    run_system.0.push("system6");
}

pub fn system7(run_system: SingleResMut<RunSystem>) {
    run_system.0.push("system7");
}