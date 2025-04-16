
#[path = "./defined.rs"]
mod defined;
use defined::*;
use pi_world::{debug::{ArchetypeDebug, ColumnDebug}, prelude::{Insert, Ticker, Entity, Query, Update}};


#[test]
fn test_ticker() {
    pub fn insert(i0: Insert<(Age3, Age1, Age0)>) {
        println!("insert1 is now");
        let e = i0.insert((Age3(3), Age1(1), Age0(0)));
        println!("insert1 is end, e:{:?}", e);
    }
    pub fn print_changed_entities(
        mut q0: Query<(
            Entity,
            Ticker<&mut Age0>,
            Ticker<&mut Age1>,
        )>,
    ) {
        println!("print_changed_entities it:{:?}", q0.iter().size_hint());
        let q = q0.iter_mut();
        for (e, mut age0, age1,) in q {
            age0.0 += 1 + age1.0;
            println!("print_changed_entities {:?}", e);
        }
        println!("print_changed_entities over");
    }
    pub fn print_changed2(
        q0: Query<(
            Ticker<&Age0>,
            Ticker<&Age1>,
        )>,
    ) {
        println!("print_changed2 tick:{:?}, last_run:{:?} it:{:?}", q0.tick(), q0.last_run(), q0.iter().size_hint());
        let q = q0.iter();
        for (
            age0,
            _age1,
        ) in q
        {
            println!("tick: {:?}, {:?}", age0.tick(), age0.last_tick());
            assert!(age0.is_changed());
        }
        println!("print_changed2 over");
    }

    let mut app = pi_world::prelude::App::new();
    app.add_system(Update, insert);
    app.add_system(Update, print_changed_entities);
    app.add_system(Update, print_changed2);

    app.world.assert_archetype_arr(&[None]);

    app.run();

    let mut info = ArchetypeDebug {
        entitys: Some(1),
        columns_info: vec![
            Some(ColumnDebug{change_listeners: 0, name: Some("Age3")}), 
            Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}), 
            Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}), 
        ],
        destroys_listeners: Some(0),
        removes: Some(0),
    };
    app.world.assert_archetype_arr(&[None, Some(info.clone())]);

    app.run();

    info.entitys = Some(2);

    app.world.assert_archetype_arr(&[None, Some(info)]);
}