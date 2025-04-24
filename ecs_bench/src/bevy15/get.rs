use bevy_15_ecs::prelude::*;
use bevy_15_ecs as bevy_ecs;

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

// pub fn system(
//     mut q: Query<( &mut Age1, &Age2,&Age3,&Age4)>,
//     entitys: Res<Entitys>,
// ) {
//     for e in entitys.0.iter() {
//         if let Ok((mut age1, _age2, _age3, _age4)) = q.get_mut(*e) {
//             age1.0 += 1; 
//         }
//     }
// }

pub struct Benchmark(pub World, pub Entities, pub QueryState<( &'static mut Age1, &'static Age2, &'static Age3,&'static Age4)>);

impl Benchmark {

    pub fn new() -> Self {
        let mut world = World::new();
        let count = 50;
        let mut entitys: Vec<Entity> = Vec::new();
        use rand::Rng;
        let mut rng = rand::thread_rng();
        for _ in 0..count {
            let r = rng.gen_range(0..5);
            match r {
                0 => entitys.push(world.spawn((Age1(1), Age2(1), Age3(1), Age4(1), Age5(1))).id()),
                1 => entitys.push(world.spawn((Age1(1), Age2(1), Age3(1), Age4(1), Age6(1))).id()),
                2 => entitys.push(world.spawn((Age1(1), Age2(1), Age3(1), Age4(1), Age7(1))).id()),
                3 => entitys.push(world.spawn((Age1(1), Age2(1), Age3(1), Age4(1), Age8(1))).id()),
                _ => entitys.push(world.spawn((Age1(1), Age2(1), Age3(1), Age4(1), Age9(1))).id()),
            }
            // entitys.push(editor.insert_entity((Age1(1), Age2(1), Age3(1), Age4(1), Age5(1))));
            // entitys.push(editor.insert_entity((Age1(1), Age2(1), Age3(1), Age4(1), Age6(1))));
            // entitys.push(editor.insert_entity((Age1(1), Age2(1), Age3(1), Age4(1), Age7(1))));
            // entitys.push(editor.insert_entity((Age1(1), Age2(1), Age3(1), Age4(1), Age8(1))));
            // entitys.push(editor.insert_entity((Age1(1), Age2(1), Age3(1), Age4(1), Age9(1))));
        }
        let state = world.query::<(&'static mut Age1, &'static Age2, &'static Age3,&'static Age4)>();
        // world.insert_resource(Entitys(entitys));
        Self(world, entitys, state)
        
    }
    pub fn run(&mut self) {
        let world = &mut self.0;
        for e in self.1.iter() {
            if let Ok((mut age1, _age2, _age3, _age4)) = self.2.get_mut(world, *e) {
                age1.0 += 1; 
            }
        }
    }
    



}
