#[path = "./defined.rs"]
mod defined;
use defined::*;
use pi_key_alloter::KeyData;
use pi_world::{debug::{ArchetypeDebug, ColumnDebug}, prelude::{App, Component, Query, Update, World}, query::EntryQuery, world::Entity};
// use pi_world_macros::{Component, SystemSet};
// use pi_world::schedule_config;

#[derive(Debug, Component)]
pub struct Age(pub usize);

// fn aa<Marker: 'static, Out: 'static + Send + Sync, F: SystemParamFunction<Marker, Out>> (xx: F) {}

// fn bb<Func: Send + Sync + 'static, Out, P: SystemFetch> (xx: Func)
//         where
//         Func:
//                 FnMut(P) -> Out +
//                 FnMut(SystemParamFetch1<P>) -> Out
//         {}

#[test]
fn test() {
    let mut app = App::new();
    let e = app.world.spawn_empty();
    let r = app.world.make_entity_editor().add_components(e, Age(5));
    println!("r: {:?}", e);
    

    debug_assert_eq!(r.is_ok(), true);
    pub fn system1(query: EntryQuery<&Age>) {
        let r=  query.get(Entity::from(KeyData::from_ffi(2u64 << 32))).unwrap();
        debug_assert_eq!(r.0, 5);
    }
    app.add_system(Update, system1);
    

    app.run();
}

#[test] 
fn test_query() {
    let mut world = World::create();
    let mut w = world.unsafe_world();
    let mut w1 = world.unsafe_world();
    let i = w.make_insert::<(Age1, Age0)>();
    let _i1 = w1.make_insert::<(Age2, Age3)>();
    let e1 = i.insert(&world, (Age1(1), Age0(0)));
    let e2 = i.insert(&world, (Age1(1), Age0(0)));
    world.settle();
    let mut q = world.make_query::<(&Age1, &mut Age0), ()>();
    for (a, mut b) in q.iter_mut(&mut world) {
        b.0 += a.0;
    }

    let info = ArchetypeDebug {
        entitys: Some(2),
        columns_info: vec![
            Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
            Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
        ],
        destroys_listeners: Some(0),
        removes: Some(0),
    };

    let info1 = ArchetypeDebug {
        entitys: Some(0),
        columns_info: vec![
            Some(ColumnDebug{change_listeners: 0, name: Some("Age2")}),
            Some(ColumnDebug{change_listeners: 0, name: Some("Age3")}),
        ],
        destroys_listeners: Some(0),
        removes: Some(0),
    };

    world.assert_archetype_arr(&[None, Some(info.clone()), Some(info1)]);

    assert_eq!(world.get_component::<Age0>(e1).unwrap().0, 1);
    assert_eq!(world.get_component::<Age0>(e2).unwrap().0, 1);
}
