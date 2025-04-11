
#[path = "./defined.rs"]
mod defined;
use defined::*;
use pi_world::{debug::{ArchetypeDebug, ColumnDebug}, prelude::Update};

#[test]
fn test_insert() {
    let mut app = pi_world::prelude::App::new();
    app.add_system(Update, insert1);
    app.add_system(Update, print_changed_entities);
    
    app.run();


    let info = ArchetypeDebug {
        entitys: Some(1),
        columns_info: vec![
            Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
            Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
        ],
        destroys_listeners: Some(0),
        removes: Some(0),
    };

    app.world.assert_archetype_arr(&[None, Some(info.clone())]);

    app.run();
}