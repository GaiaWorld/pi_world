
#[path = "./defined.rs"]
mod defined;
use defined::*;
use pi_world::{debug::{ArchetypeDebug, ColumnDebug}, prelude::{Insert, Alter, Query, Entity, ComponentRemoved, Update}};


#[test]
fn test_removed() { 
    pub fn insert(i0: Insert<(Age3, Age1, Age0)>) {
        println!("insert1 is now");
        let e = i0.insert((Age3(3), Age1(1), Age0(0)));
        println!("insert1 is end, e:{:?}", e);
    }
    pub fn alter(
        mut i0: Alter<&Age1, (), (), (Age3,)>,
        q0: Query<(Entity, &mut Age0, &Age1), ()>,
    ) {
        println!("alter1 it:{:?}", q0.iter().size_hint());
        for (e, _, _) in q0.iter() {
            let r = i0.alter(e, ());
            println!("alter1 it1111:{:?}", r);
        }
        println!("alter1: end");
    }
    pub fn removed_l(mut q0: Query<(&mut Age0, &mut Age1)>, removed: ComponentRemoved<Age3>) {
        println!("removed_l");
        for e in removed.iter() {
            println!("e:{:?}, q0: {:?}", e, q0.get_mut(*e));
        }
        println!("removed_l: end");
    }
    let mut app = pi_world::prelude::App::new();
    app.add_system(Update, insert);
    // app.add_system(Update, print_changed_entities);
    app.add_system(Update, alter);
    app.add_system(Update, removed_l);
    app.add_system(Update, print_info);
    
    app.world.assert_archetype_arr(&[None]);

    app.run();

    let info = ArchetypeDebug {
        entitys: Some(0),
        columns_info: vec![
            Some(ColumnDebug{change_listeners: 0, name: Some("Age3")}), 
            Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
            Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
        ],
        destroys_listeners: Some(0),
        removes: Some(0),
    };
    app.world.assert_archetype_arr(&[None, Some(info.clone()), None]);

    app.run();

    let info1 = ArchetypeDebug {
        entitys: Some(2),
        columns_info: vec![ 
            Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
            Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
        ],
        destroys_listeners: Some(0),
        removes: Some(0),
    };

    app.world.assert_archetype_arr(&[None, Some(info.clone()), Some(info1)]);
}