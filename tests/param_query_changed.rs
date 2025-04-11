
#[path = "./defined.rs"]
mod defined;
use defined::*;
use pi_world::{debug::{ArchetypeDebug, ColumnDebug}, prelude::{Changed, Entity, Query, Update}};

#[test]
fn test_changed() { 
    let mut app = pi_world::prelude::App::new();
    app.add_system(Update, insert1);
    app.add_system(Update, print_changed_entities);
    app.add_system(Update, alter1);
    app.add_system(Update, changed_l);
    
    app.world.assert_archetype_arr(&[None]);

    app.run();

    let mut info = ArchetypeDebug {
        entitys: Some(0),
        columns_info: vec![
            Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
            Some(ColumnDebug{change_listeners: 1, name: Some("Age0")}),
        ],
        destroys_listeners: Some(0),
        removes: Some(0),
    };

    app.world.assert_archetype_arr(&[None, Some(info.clone()), None]);
    info.entitys = Some(1);
    info.columns_info = vec![
        Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
        Some(ColumnDebug{change_listeners: 1, name: Some("Age0")}),
        Some(ColumnDebug{change_listeners: 0, name: Some("Age3")}),
    ];
    info.removes = Some(0);
    app.world.assert_archetype_arr(&[None, None, Some(info.clone()),]);
    app.run();
}

pub fn changed_l(q0: Query<(Entity, &mut Age0, &mut Age1), (Changed<Age0>, Changed<Age2>)>) {
    println!("changed_l");
    for (e, age0, _) in q0.iter() {
        println!("e {:?}, age0: {:?}", e, age0);
    }

    println!("changed_l: end");
}