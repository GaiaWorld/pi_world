
#[path = "./defined.rs"]
mod defined;
use defined::*;
use pi_null::Null;
use pi_world::{debug::{ArchetypeDebug, ColumnDebug}, prelude::{Changed, ComponentRemoved, Entity, EntityEditor, Query, Update, With}};


#[test]
fn test_editor() {
    let mut app = pi_world::prelude::App::new();
    pub fn alter_add(mut edit: EntityEditor) {
        println!("alter_add start!!");
        let scomponents = [
            edit.init_component::<Age0>(), 
            edit.init_component::<Age1>()
        ];

        let e = edit.insert_entity_by_index(&scomponents).unwrap();
        println!("alter_add end!! e: {:?}", e);
    }

    pub fn alter_add2(
        mut edit: EntityEditor,
        q: Query<(Entity, &Age1, &Age0), (Changed<Age1>, Changed<Age0>)>,
    ) {
        println!("alter_add2 start!!");
        // assert_eq!(q.is_empty(), false);
        let mut iter = q.iter();
        let r = iter.next(); 
        assert_eq!(r.is_some(), true);
        let (e, age1, age0) = r.unwrap();

        println!("alter_add2!! e: {:?}, age1: {:?}, age0:{:?}", e, age1, age0);
        let arr = [
            (edit.init_component::<Age2>(), true),
            (edit.init_component::<Age3>(), true),
            (edit.init_component::<Age0>(), false),
        ];
        edit.alter_components_by_index(e, &arr).unwrap();

        println!("alter_add2 end");
    }

    pub fn alter_add3(
        _edit: EntityEditor,
        q: Query<(Entity, &Age0), Changed<Age1>>,
    ) {
        println!("alter_add3 start!!");
        let iter = q.iter().next();
        assert_eq!(iter.is_none(), true);

        println!("alter_add3 end");
    }

    pub fn query(q: Query<(&Age1, &Age2, &Age3)>, removed: ComponentRemoved<Age0>) {
        println!("query start!!!");
        let re = removed.iter().next();
        assert_eq!(re.is_some(), true);
        println!("query entity!!!,{:?}", re);
        let (_age1, _age2, _age3) = q.get(*re.unwrap()).unwrap();
        println!("query end!!!");
    }

    pub fn query2(q: Query<(Entity, &Age1, &Age2, &Age3, ), Changed<Age3>>, editor: EntityEditor) {
        println!("query2 start!!!");
        let iter = q.iter().next();
        assert_eq!(iter.is_some(), true);
        let (e, _age1, _age2, _age3) = iter.unwrap();
        let _ = editor.destroy(e);
        println!("query2 end!!!");
    }

    pub fn query3(q: Query<(Entity, &Age1, &Age2, &Age3,)>,) {
        println!("query3 start!!!");
        let iter = q.iter().next();
        assert_eq!(iter.is_null(), true);
        println!("query3 end!!!");
    }

    app.add_system(Update, alter_add);
    app.add_system(Update, alter_add2);
    app.add_system(Update, alter_add3);
    app.add_system(Update, query);
    app.add_system(Update, query2);
    app.add_system(Update, query3);

    app.run();

    let info1 = ArchetypeDebug {
        entitys: Some(0),
        columns_info: vec![
            Some(ColumnDebug{change_listeners: 2, name: Some("Age1")}), 
            Some(ColumnDebug{change_listeners: 1, name: Some("Age0")}), 
        ],
        destroys_listeners: Some(0),
        removes: Some(0),
    };
    let info2 = ArchetypeDebug {
        entitys: Some(0),
        columns_info: vec![
            Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}), 
            Some(ColumnDebug{change_listeners: 0, name: Some("Age2")}), 
            Some(ColumnDebug{change_listeners: 1, name: Some("Age3")}), 
        ],
        destroys_listeners: Some(0),
        removes: Some(0),
    };

    app.world.assert_archetype_arr(&[None, Some(info1), Some(info2)]);
    app.run();
    app.run();
    app.run();
    app.run();
}

#[test] 
fn test_editor2() {
    

    let mut app = pi_world::prelude::App::new();

    pub fn insert_entity(mut editor: EntityEditor ) {
        println!("insert_entity start!!!");
        let e1 = editor.insert_entity( (Age0(10),));
        let e2 = editor.insert_entity((Age0(20),));

        println!("insert_entity end!!! e: {:?}", (e1, e2));
    }

    pub fn add_components(q: Query<(Entity, &Age0)>, mut editor: EntityEditor){
        println!("query1 start!!!");
        let mut len = 0;
        q.iter().for_each(|(e, a0)|{
            len += 1;
            println!("v: {:?}", (e, a0));
            editor.add_components(e, (Age10(10), (Age1(1), Age2(2)))).unwrap();
            editor.add_components(e, (Age9(9),)).unwrap();

            let e2 = editor.alloc_entity();
            editor.add_components(e2, (Age10(10),(Age1(1), Age2(2)))).unwrap();
            editor.add_components(e2, (Age9(9),)).unwrap();
        });
        println!("query1 end!!!");
        assert_eq!(len, 2);
    }

    pub fn query3(q: Query<(Entity, &Age9, &Age1), (With<Age10>, Changed<Age2>)>){
        println!("query3 start!!!");
        let mut len = 0;
        q.iter().for_each(|(e, a9, a1 )|{
            len += 1;
            println!("v: {:?}", (e, a9, a1));
        });
        println!("query3 end!!!");
        assert_eq!(len, 4);
    }

    
    app.add_system(Update, insert_entity);
    app.add_system(Update, add_components);
    app.add_system(Update, query3);
    app.run();

}

#[test] 
fn test_editor3() {

    let mut app = pi_world::prelude::App::new();

    pub fn insert_entity(mut editor: EntityEditor ) {
        println!("insert_entity start!!!");
        let e1 = editor.insert_entity((Age21(Vec::with_capacity(256)),));
        let e2 = editor.insert_entity((Age21(Vec::with_capacity(256)),));

        println!("insert_entity end!!! e: {:?}", (e1, e2));
    }

    pub fn add_components(mut q: Query<(Entity, &mut Age21)>, _editor: EntityEditor){
        println!("query5 start!!!");
        let mut len = 0;
        q.iter_mut().for_each(|(e, mut a21)|{
            len += 1;
            println!("v: {:?}", (e, &a21.0));
            a21.0 = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];
            a21.0.push(21);
            a21.0.push(22);
        });
        println!("query5 end!!!");
        assert_eq!(len, 2);
    }

    pub fn query3(q: Query<(Entity, &Age21)>){
        println!("query3 start!!!");
        let mut len = 0;
        q.iter().for_each(|(e, a21,  )|{
            len += 1;
            println!("v: {:?}", (e, a21));
        });
        println!("query3 end!!!");
        assert_eq!(len, 2);
    }

    
    app.add_system(Update, insert_entity);
    app.add_system(Update, add_components);
    app.add_system(Update, query3);
    app.run();
    println!("=======================end");
}

#[test] 
fn test_editor_settle() {
    let mut app = pi_world::prelude::App::new();
    pub fn alter_add(mut edit: EntityEditor) {
        let scomponents = [
            edit.init_component::<Age0>(), 
            edit.init_component::<Age1>()
        ];

        let _e = edit.insert_entity_by_index(&scomponents).unwrap();
    }

    pub fn alter_add2(
        mut edit: EntityEditor,
        q: Query<(Entity, &Age1, &Age0), (Changed<Age1>, Changed<Age0>)>,
    ) {
        // assert_eq!(q.is_empty(), false);
        let mut iter = q.iter();
        let r = iter.next(); 
        assert_eq!(r.is_some(), true);
        let (e, _age1, _age0) = r.unwrap();
        assert_eq!(iter.next(), None);
        let arr = [
            (edit.init_component::<Age2>(), true),
            (edit.init_component::<Age3>(), true),
            (edit.init_component::<Age0>(), false),
        ];
        edit.alter_components_by_index(e, &arr).unwrap();
    }

    pub fn alter_add3(
        _edit: EntityEditor,
        q: Query<(Entity, &Age0), Changed<Age1>>,
    ) {
        // assert_eq!(q.is_empty(), false);
        let iter = q.iter().next();
        assert_eq!(iter.is_null(), true);
    }

    pub fn query(_q: Query<(&Age1, &Age2, &Age3)>, removed: ComponentRemoved<Age0>) {
        let re = removed.iter().next();
        assert_eq!(re.is_some(), true);
        // let (age1, age2, age3) = q.get(*re.unwrap()).unwrap();
    }

    pub fn query2(q: Query<(Entity, &Age1, &Age2, &Age3, ), Changed<Age3>>, editor: EntityEditor) {
        let iter = q.iter().next();
        assert_eq!(iter.is_some(), true);
        let (e, _age1, _age2, _age3) = iter.unwrap();
        let _ = editor.destroy(e);
    }

    pub fn query3(q: Query<(Entity, &Age1, &Age2, &Age3,)>,) {
        let iter = q.iter().next();
        assert_eq!(iter.is_null(), true);
    }

    app.add_system(Update, alter_add);
    app.add_system(Update, alter_add2);
    app.add_system(Update, alter_add3);
    app.add_system(Update, query);
    app.add_system(Update, query2);
    app.add_system(Update, query3);

    app.run();

    let info1 = ArchetypeDebug {
        entitys: Some(0),
        columns_info: vec![
            Some(ColumnDebug{change_listeners: 2, name: Some("Age1")}), 
            Some(ColumnDebug{change_listeners: 1, name: Some("Age0")}), 
        ],
        destroys_listeners: Some(0),
        removes: Some(0),
    };
    let info2 = ArchetypeDebug {
        entitys: Some(0),
        columns_info: vec![
            Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}), 
            Some(ColumnDebug{change_listeners: 0, name: Some("Age2")}), 
            Some(ColumnDebug{change_listeners: 1, name: Some("Age3")}), 
        ],
        destroys_listeners: Some(0),
        removes: Some(0),
    };

    app.world.assert_archetype_arr(&[None, Some(info1), Some(info2)]);
    for _ in 0..10_000 {
        app.run();
    }
}

// #[test]
// fn alter(){ 
//     let mut world = World::new();
    
//     let i = world.make_inserter::<(Age1, Age0)>();
//     let e1 = i.insert((Age1(2), Age0(1)));
//     println!("===========1114: {:?}", e1);
//     let mut editor = world.make_entity_editor();
//     let components  =[
//         (editor.init_component::<Age3>(), true), 
//         (editor.init_component::<Age1>(), false)
//     ];
//     editor.alter_components_by_index(e1, &components).unwrap();
    
//     let age3 = world.get_component::<Age3>(e1);
//     assert_eq!(age3.is_ok(), true);

//     let age1 = world.get_component::<Age1>(e1);
//     assert_eq!(age1.is_err(), true);

//     println!(", age1: {:?}, age3: {:?}", age1, age3);
// }

// #[test]
// fn app_alter(){
//     let mut app = SingleThreadApp::new();
//     // let mut world = &app.world;
//     pub fn insert(i0: Insert<(Age1, Age0)>) {
//         println!("insert1 is now");
//         let e = i0.insert((Age1(1), Age0(0)));
//         println!("insert1 is end, e:{:?}", e);
//     }

//     pub fn alter(mut w: EntityEditor, q: Query<(Entity, &Age1, &Age0)>) {
//        q.iter().for_each(|(e, age1, age0)|{
//         println!("alter!! e: {:?}, age1: {:?}, age0: {:?}", e, age1, age0);
//             w.alter_components_by_index(e, &[
//                 (w.init_component::<Age2>(), true), 
//                 (w.init_component::<Age1>(), false)
//             ]).unwrap();
//        });
//     }

//     pub fn query(w: &World, q: Query<(Entity, &Age2, &Age0)>) {
//         println!("query start!!!");
//         q.iter().for_each(|(e, age2, age0)|{
//             println!("query!!! e: {:?}, age2: {:?}, age0: {:?}", e, age2, age0);
//         });
//      }

//     app.add_system(Update, insert);
//     app.add_system(Update, alter);
//     app.add_system(Update, query);

//     app.world.assert_archetype_arr(&[None]);

//     app.run();

//     let mut info = ArchetypeDebug {
//         entitys: Some(0),
//         columns_info: vec![
//             Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}), 
//             Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}), 
//         ],
//         destroys_listeners: Some(0),
//         removes: Some(0),
//     };
//     let mut info1 = ArchetypeDebug {
//         entitys: Some(1),
//         columns_info: vec![   
//             Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
//             Some(ColumnDebug{change_listeners: 0, name: Some("Age2")}), 
//         ],
//         destroys_listeners: Some(0),
//         removes: Some(0),
//     };
//     app.world.assert_archetype_arr(&[None, Some(info), Some(info1)]);

//     app.run();
// }

