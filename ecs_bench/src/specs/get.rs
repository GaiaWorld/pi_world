use specs::prelude::*;
use specs_derive::*;

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

struct SimpleGetSystem(Entities);

impl<'a> System<'a> for SimpleGetSystem {
    type SystemData = (WriteStorage<'a, Age1>, ReadStorage<'a, Age2>, ReadStorage<'a, Age3>, ReadStorage<'a, Age4>);

    fn run(&mut self, (mut age1_storage, age2_storage, age3_storage, age4_storage): Self::SystemData) {
        for e in self.0.iter() {
            if let (Some(age1), _age2, _age3, _age4) = (age1_storage.get_mut( *e), age2_storage.get(*e), age3_storage.get(*e), age4_storage.get(*e)) {
                age1.0 += 1; 
            }
        }
    }
}

pub struct Benchmark(pub World, SimpleGetSystem);

impl Benchmark {

    pub fn new() -> Self {
        let mut world = World::new();
        world.register::<Age1>();
        world.register::<Age2>();
        world.register::<Age3>();
        world.register::<Age4>();
        world.register::<Age5>();
        world.register::<Age6>();
        world.register::<Age7>();
        world.register::<Age8>();
        world.register::<Age9>();
        let count = 50;
        let mut entitys: Entities = Vec::new();
        use rand::Rng;
        let mut rng = rand::thread_rng();
        for _ in 0..count {
            let r = rng.gen_range(0..5);
            match r {
                0 => entitys.push(world
                    .create_entity()
                    .with(Age1(1))
                    .with(Age2(1))
                    .with(Age3(1))
                    .with(Age4(1))
                    .with(Age5(1))
                    .build()),
                1 => entitys.push(world
                    .create_entity()
                    .with(Age1(1))
                    .with(Age2(1))
                    .with(Age3(1))
                    .with(Age4(1))
                    .with(Age6(1))
                    .build()),
                2 => entitys.push(world
                    .create_entity()
                    .with(Age1(1))
                    .with(Age2(1))
                    .with(Age3(1))
                    .with(Age4(1))
                    .with(Age7(1))
                    .build()),
                3 => entitys.push(world
                    .create_entity()
                    .with(Age1(1))
                    .with(Age2(1))
                    .with(Age3(1))
                    .with(Age4(1))
                    .with(Age8(1))
                    .build()),
                _ => entitys.push(world
                    .create_entity()
                    .with(Age1(1))
                    .with(Age2(1))
                    .with(Age3(1))
                    .with(Age4(1))
                    .with(Age9(1))
                    .build())
            }
        }
        Self(world, SimpleGetSystem(entitys))
        
    }
    pub fn run(&mut self) {
        self.1.run_now(&self.0);
    }
}
