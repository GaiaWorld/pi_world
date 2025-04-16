
//! 比较Query和EntryQuery的初始化性能和迭代性能
#![feature(test)]
#![feature(random)]
extern crate test;



mod mod3 {
    use pi_world::{prelude::{App, Component, Query, Update}, query::EntryQuery, single_res::SingleRes, world::{Entity, World}};
    use  test::Bencher;
    use super::*;

    #[derive(Debug, Component)]
    pub struct Age1(usize);
    #[derive(Debug, Component)]
    pub struct Age2(usize);
    #[derive(Debug, Component)]
    pub struct Age3(usize);
    #[derive(Debug, Component)]
    pub struct Age4(usize);
    #[derive(Debug, Component)]
    pub struct Age5(usize);
    #[derive(Debug, Component)]
    pub struct Age6(usize);
    #[derive(Debug, Component)]
    pub struct Age7(usize);
    #[derive(Debug, Component)]
    pub struct Age8(usize);
    #[derive(Debug, Component)]
    pub struct Age9(usize);
    #[derive(Debug, Component)]
    pub struct Age10(usize);
    #[derive(Debug, Component)]
    pub struct Age11(usize);
    #[derive(Debug, Component)]
    pub struct Age12(usize);
    #[derive(Debug, Component)]
    pub struct Age13(usize);

    pub type Entitys = Vec<Entity>;

    fn init(world: &mut World) {
        let mut editor = world.make_entity_editor();
        let count = 50;
        let mut entitys: Entitys = Vec::new();
        use rand::Rng;
        let mut rng = rand::thread_rng();
        for _ in 0..count {
            let r = rng.gen_range(0..5);
            match r {
                0 => entitys.push(editor.insert_entity((Age1(1), Age2(1), Age3(1), Age4(1), Age5(1)))),
                1 => entitys.push(editor.insert_entity((Age1(1), Age2(1), Age3(1), Age4(1), Age6(1)))),
                2 => entitys.push(editor.insert_entity((Age1(1), Age2(1), Age3(1), Age4(1), Age7(1)))),
                3 => entitys.push(editor.insert_entity((Age1(1), Age2(1), Age3(1), Age4(1), Age8(1)))),
                _ => entitys.push(editor.insert_entity((Age1(1), Age2(1), Age3(1), Age4(1), Age9(1)))),
            }
            // entitys.push(editor.insert_entity((Age1(1), Age2(1), Age3(1), Age4(1), Age5(1))));
            // entitys.push(editor.insert_entity((Age1(1), Age2(1), Age3(1), Age4(1), Age6(1))));
            // entitys.push(editor.insert_entity((Age1(1), Age2(1), Age3(1), Age4(1), Age7(1))));
            // entitys.push(editor.insert_entity((Age1(1), Age2(1), Age3(1), Age4(1), Age8(1))));
            // entitys.push(editor.insert_entity((Age1(1), Age2(1), Age3(1), Age4(1), Age9(1))));
        }

        world.insert_single_res(entitys);
        
    }
    
    fn system(
        mut q: Query<( &mut Age1, &Age2,&Age3,&Age4)>,
        entitys: SingleRes<Entitys>,
    ) {
        for e in entitys.iter() {
            if let Ok((mut age1, age2, age3, age4)) = q.get_mut(*e) {
                age1.0 += 1;
            }
        }
    }

    #[bench]
    fn param_query(b: &mut Bencher) {
        let mut app = App::new();
        init(&mut app.world);
        let count = 500;
        for i in 0..count {
            app.add_system(Update, system);
        }
        
        // app.add_system(Update, p_set);
        
        app.run();

        b.iter(move || {
            app.run();
        });
    }




    fn system1(
        mut q: EntryQuery<( &mut Age1, &Age2,&Age3,&Age4)>,
        entitys: SingleRes<Entitys>,
    ) {
        for e in entitys.iter() {
            if let Ok((mut age1, age2, age3, age4)) = q.get_mut(*e) {
                age1.0 += 1;
            }
        }
    }

    #[bench]
    fn param_entryquery(b: &mut Bencher) {
        let mut app = App::new();
        init(&mut app.world);
        let count = 500;
        for i in 0..count {
            app.add_system(Update, system1);
        }
        
        // app.add_system(Update, p_set);
        
        app.run();

        b.iter(move || {
            app.run();
        });
    }


    fn system2(
        mut q: EntryQuery<( &mut Age1, &Age2,&Age3,&Age4)>,
        entitys: SingleRes<Entitys>,
    ) {
        let e = entitys[0];
        let (mut age1, _age2, _age3, _age4) = q.get_mut(e).unwrap();
        for _e in entitys.iter() {
            age1.0 += 1;
        }
    }

    #[bench]
    fn param_not_entryquery(b: &mut Bencher) {
        let mut app = App::new();
        init(&mut app.world);
        let count = 500;
        for i in 0..count {
            app.add_system(Update, system2);
        }
        
        // app.add_system(Update, p_set);
        
        app.run();

        b.iter(move || {
            app.run();
        });
    }

    fn system3(
        mut q: Query<( &mut Age1, &Age2,&Age3,&Age4)>,
        entitys: SingleRes<Entitys>,
    ) {
        let e = entitys[0];
        let (mut age1, _age2, _age3, _age4) = q.get_mut(e).unwrap();
        for _e in entitys.iter() {
            age1.0 += 1;
        }
    }

    #[bench]
    fn param_not_query(b: &mut Bencher) {
        let mut app = App::new();
        init(&mut app.world);
        let count = 500;
        for i in 0..count {
            app.add_system(Update, system3);
        }
        
        // app.add_system(Update, p_set);
        
        app.run();

        b.iter(move || {
            app.run();
        });
    }
}

