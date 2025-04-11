
#[path = "./defined.rs"]
mod defined;
use defined::*;
use pi_world::{debug::{ArchetypeDebug, ColumnDebug}, prelude::{IntoSystemConfigs, Update}};


#[test]
fn test_added() { 
    let mut app = pi_world::prelude::App::new();
    app.add_system(Update, insert1);
    app.add_system(Update, print_changed_entities);
    app.add_system(Update, added_l);
    app.add_system(Update, alter1.in_schedule(AddSchedule));
    
    app.run_schedule(AddSchedule);

    let info = ArchetypeDebug {
        entitys: Some(0),
        columns_info: vec![
            Some(ColumnDebug{change_listeners: 1, name: Some("Age1")}),
            Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
        ],
        destroys_listeners: Some(0),
        removes: Some(0),
    };

    app.world.assert_archetype_arr(&[None, Some(info.clone())]);

    app.run_schedule(AddSchedule);

    app.world.assert_archetype_arr(&[None, Some(info.clone())]);
}