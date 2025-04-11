#![feature(test)]
#[path = "./defined.rs"]
mod defined;

extern crate test;
use defined::*;
use test::Bencher;
use pi_world::{debug::{ArchetypeDebug, ColumnDebug}, prelude::{World, Entity, EntityEditor, ParamSet, Query, Update}};

type Bundle0 = (Age0, Age1, Age2, Age3, Age4, Age5, Age6, Age7);
type Bundle1 = (Age8, Age9, Age10, Age11, Age12, Age13, Age14, Age15);
type Bundle2 = (Age16, Age17, Age18, Age19, Age20, Age21);
type Bundle3 = (Age22, Age23, Age24, Age25, Age26, Age27);
type Bundle4 = (Age28, Age29, Age30, Age31, Age32, Age33, Age34, Age35, Age36, Age37, Age38, Age39);
type Bundle5 = (Age40, Age41, Age42, Age43, Age44, PassObjInitBundle);
// type Bundle6 = (Bundle1, Bundle2, Bundle3, Bundle4, Bundle5);
#[bench]
fn test_alter(b: &mut Bencher) {
    let mut app = pi_world::prelude::App::new();
    app.add_system(Update, insert1);
    app.add_system(Update, print_changed_entities);
    app.add_system(Update, alter1);
    // app.add_system(Update, p_set);
    
    app.run();

    let mut alter1 = app.world.make_alter::<(), (), Bundle0, ()>();
    let mut alter2 = app.world.make_alter::<(), (), Bundle1, ()>();
    let mut alter3 = app.world.make_alter::<(), (), Bundle2, ()>();
    let mut alter4 = app.world.make_alter::<(), (), (Bundle2, Bundle3), ()>();
    let mut alter5 = app.world.make_alter::<(), (), (Bundle1, Bundle2, Bundle3, Bundle4, Bundle5), ()>();
    let mut query5 = app.world.make_query::<(&Age8, &Age9, &Age10, &Age11, &Age12), ()>();

    b.iter(move || {
        let len = 10000;
        let mut entries: Vec<(Bundle1, Bundle2, Bundle3, Bundle4, Bundle5)> = vec![(Bundle1::default(), Bundle2::default(), Bundle3::default(), Bundle4::default(), Bundle5::default());10000];

        // let mut entries: Vec<Bundle5> = vec![Bundle5::default();10000];

        // let mut entries: Vec<(Bundle1, Bundle2, Bundle3, Bundle4, Bundle5)> = Vec::with_capacity(len);
        // entries.resize_with(entries.capacity(), || (Bundle1::default(), Bundle2::default(), Bundle3::default(), Bundle4::default(), Bundle5::default()));
        // let temp = Bundle6::default();
        // for count in 0..len {
        //     unsafe {
        //         let end = entries.as_mut_ptr().add(count);
        //         core::ptr::write(end, temp.clone());
        //     }
        // }
        unsafe {
            entries.set_len(len);
        }

        // for i in 0..100 {
        //     let entity = app.world.spawn_empty();
        //     let mut alter = alter1.get_param(&app.world);
        //     alter.alter(entity, Bundle0::default());

        //     let entity = app.world.spawn_empty();
        //     let mut alter = alter2.get_param(&app.world);
        //     alter.alter(entity, Bundle1::default());

        //     // let entity = app.world.spawn_empty();
        //     // let mut alter = alter3.get_param(&app.world);
        //     // alter.alter(entity, Bundle2::default());

        //     // let entity = app.world.spawn_empty();
        //     // let mut alter = alter4.get_param(&app.world);
        //     // alter.alter(entity, (Bundle2::default(), Bundle3::default()));

        //     let entity = app.world.spawn_empty();
        //     let mut alter = alter5.get_param(&app.world);
        //     alter.alter(entity, (Bundle1::default(), Bundle2::default(), Bundle3::default(), Bundle4::default(), Bundle5::default()));

        //     query5.align(&app.world);
        // }
    });

    // let mut info = ArchetypeDebug {
    //     entitys: Some(0),
    //     columns_info: vec![
    //         Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
    //         Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
    //     ],
    //     destroys_listeners: Some(0),
    //     removes: Some(0),
    // };

    // let mut info1 = ArchetypeDebug {
    //     entitys: Some(1),
    //     columns_info: vec![
    //         Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
    //         Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
    //         Some(ColumnDebug{change_listeners: 0, name: Some("Age3")}),
    //     ],
    //     destroys_listeners: Some(0),
    //     removes: Some(0),
    // };

    // app.world.assert_archetype_arr(&[None, Some(info.clone()), Some(info1)]);

    // app.run();
    // app.run();
}
#[test]
fn test_alter0() {
    let mut app = pi_world::prelude::App::new();
    app.add_system(Update, insert1);
    app.add_system(Update, print_changed_entities);
    app.add_system(Update, alter1);
    // app.add_system(Update, p_set);
    
    app.run();

    let mut alter1 = app.world.make_alter::<(), (), Bundle0, ()>();
    let mut alter2 = app.world.make_alter::<(), (), Bundle1, ()>();
    let mut alter3 = app.world.make_alter::<(), (), Bundle2, ()>();
    let mut alter4 = app.world.make_alter::<(), (), (Bundle2, Bundle3), ()>();
    let mut alter5 = app.world.make_alter::<(), (), (Bundle1, Bundle2, Bundle3, Bundle4, Bundle5), ()>();
    let mut query5 = app.world.make_query::<(&Age8, &Age9, &Age10, &Age11, &Age12), ()>();
    loop {
        for i in 0..100 {
            let entity = app.world.spawn_empty();
            let mut alter = alter1.get_param(&app.world);
            alter.alter(entity, Bundle0::default());

            let entity = app.world.spawn_empty();
            let mut alter = alter2.get_param(&app.world);
            alter.alter(entity, Bundle1::default());

            let entity = app.world.spawn_empty();
            let mut alter = alter3.get_param(&app.world);
            alter.alter(entity, Bundle2::default());

            let entity = app.world.spawn_empty();
            let mut alter = alter4.get_param(&app.world);
            alter.alter(entity, (Bundle2::default(), Bundle3::default()));

            let entity = app.world.spawn_empty();
            let mut alter = alter5.get_param(&app.world);
            alter.alter(entity, (Bundle1::default(), Bundle2::default(), Bundle3::default(), Bundle4::default(), Bundle5::default()));

            query5.align(&app.world);
        }
        app.run();
    }
}
#[test]
fn test_alter1() {
    let mut world = World::create();
    let i = world.make_insert::<(Age1, Age0)>();
    let e1 = i.insert(&world, (Age1(2), Age0(1)));
    let e2 = i.insert(&world, (Age1(4), Age0(2)));
    world.settle();
    {
        let mut editor = world.make_entity_editor();
        let index = editor.init_component::<Age2>();

        // editor.add_components_by_index(e1, &[index]);
        editor.add_components_by_index(e2, &[index]);
    }
    world.settle();

    let mut info = ArchetypeDebug {
        entitys: Some(1),
        columns_info: vec![
            Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
            Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
        ],
        destroys_listeners: Some(0),
        removes: Some(0),
    };

    let mut info1 = ArchetypeDebug {
        entitys: Some(1),
        columns_info: vec![
            Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
            Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
            Some(ColumnDebug{change_listeners: 0, name: Some("Age2")}),
        ],
        destroys_listeners: Some(0),
        removes: Some(0),
    };

    world.assert_archetype_arr(&[None, Some(info.clone()), Some(info1)]);

    assert_eq!(world.get_component::<Age0>(e1).unwrap().0, 1);
    assert_eq!(world.get_component::<Age2>(e1).is_err(), true);
    assert_eq!(world.get_component::<Age0>(e2).unwrap().0, 2);
    assert_eq!(world.get_component::<Age1>(e2).unwrap().0, 4);
    assert_eq!(world.get_component::<Age2>(e2).unwrap().0, 0);
}
#[test]
fn test_alter2() {
    println!("0");
    let mut world = World::create();
    let i = world.make_insert::<(Age0,)>();
    let _entities = (0..1)
        .map(|_| i.insert(&world, (Age0(0),)))
        .collect::<Vec<Entity>>();
    world.settle();
    let mut editor = world.make_entity_editor();
    let index = editor.init_component::<Age1>();
    {
        println!("1");
        
        for e in &_entities{
                editor.add_components_by_index(*e, &[index]);
        }
        println!("2");
    }
    {
        for e in &_entities {
            editor.remove_components_by_index(*e, &[index]);
        }
    }

    let mut info = ArchetypeDebug {
        entitys: Some(2),
        columns_info: vec![
            Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
        ],
        destroys_listeners: Some(0),
        removes: Some(1),
    };

    world.assert_archetype_arr(&[None, Some(info.clone()), None]);

    info.entitys = Some(1);
    info.columns_info = vec![
        Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
        Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
    ];
    world.assert_archetype_arr(&[None, None, Some(info.clone()), ]);
}

#[test] 
fn test_alter3() {
    #[allow(dead_code)]
    pub struct EntityRes(Entity);

    let mut app = pi_world::prelude::App::new();
    let i = app.world.make_insert::<(Age0, Age1, Age2)>();
    let e = i.insert(&app.world, (Age0(0), Age1(1), Age2(2)));
    println!("========== e: {:?}", e);
    app.world.insert_single_res(EntityRes(e));

    pub fn query(p: ParamSet<(Query<(Entity, &Age0, &Age1, &Age2)>, EntityEditor)> ) {
        println!("query start!!!");
        let mut entity = None;
        {
            let q = p.p0();
            q.iter().for_each(|(e, a0, a1, a2)|{
                entity = Some(e);
                println!("v: {:?}", (e, a0, a1, a2));
            });
        }
        assert_eq!(entity.is_some(), true);
        {
            let editor = p.p1();
            let index = editor.init_component::<Age0>();
            let _ = editor.remove_components_by_index(entity.unwrap(), &[index]);
        }

        println!("query end!!!");
    }

    pub fn query2(q: Query<(Entity, &Age0, &Age1, &Age2)>){
        let mut len = 0;
        q.iter().for_each(|(e, a0, a1, a2)|{
            len += 1;
            println!("v: {:?}", (e, a0, a1, a2));
        });
        assert_eq!(len, 0);
    }

    
    app.add_system(Update, query);
    app.add_system(Update, query2);

    let mut info = ArchetypeDebug {
        entitys: Some(1),
        columns_info: vec![
            Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}), 
            Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}), 
            Some(ColumnDebug{change_listeners: 0, name: Some("Age2")}), 
        ],
        destroys_listeners: Some(0),
        removes: Some(0),
    };

    app.world.assert_archetype_arr(&[None, Some(info.clone())]);
    app.run();

    info.columns_info[0].as_mut().unwrap().change_listeners = 0;
    info.entitys = Some(0);
    info.removes = Some(0);

    app.world.assert_archetype_arr(&[None, Some(info), None]);
}