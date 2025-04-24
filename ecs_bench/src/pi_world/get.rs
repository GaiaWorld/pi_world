use pi_world::prelude::*;
use pi_world::query::QueryState;

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

pub type Entities = Vec<Entity>;


pub struct Benchmark(pub App, pub Entities, pub QueryState<( &'static mut Age1, &'static Age2, &'static Age3,&'static Age4), ()>);

impl Benchmark {

    pub fn new() -> Self {
        let mut app = App::new();
        let mut editor = app.world.make_entity_editor();
        let count = 50;
        let mut entitys: Entities = Vec::new();
        use rand::Rng;
        let mut rng = rand::thread_rng();
        for _ in 0..count {
            let r = rng.gen_range(0..1);
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
        app.world.settle();

        let state = app.world.make_query::<( &'static mut Age1, &'static Age2, &'static Age3,&'static Age4)>();
        // app.world.insert_single_res();
        Self(app, entitys, state)
        
    }
    pub fn run(&mut self) {
        let world = &mut *self.0.world;
        self.2.align();
        for e in self.1.iter() {
            if let Ok((mut age1, _age2, _age3, _age4)) = self.2.get_mut(world, *e) {
                age1.0 += 1; 
            }
        }
        self.0.run();
    }
    



}
