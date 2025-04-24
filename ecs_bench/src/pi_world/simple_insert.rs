use pi_world::{prelude::*, world::ComponentIndex};
// use pi_world_macros::Component;
// use bevy_5_ecs::system::EventReader;
use cgmath::*;

#[derive(Copy, Clone, Component)]
struct Transform(Matrix4<f32>);

impl Default for Transform {
    fn default() -> Self {
        Self(Matrix4::identity())
    }
}

#[derive(Copy, Clone, Component)]
struct Position(Vector3<f32>);

impl Default for Position {
    fn default() -> Self {
        Self(Vector3::new(0.0, 0.0, 0.0))
    }
}

#[derive(Copy, Clone, Component)]
struct Rotation(Vector3<f32>);

impl Default for Rotation {
    fn default() -> Self {
        Self(Vector3::new(0.0, 0.0, 0.0))
    }
}

#[derive(Copy, Clone, Component)]
struct Velocity(Vector3<f32>);
impl Default for Velocity {
    fn default() -> Self {
        Self(Vector3::new(0.0, 0.0, 0.0))
    }
}

pub struct Benchmark(Box<World>);

impl Benchmark {
    pub fn new() -> Self {
        Self(World::create())
    }

    pub fn run(&mut self) {
        
		let editor = self.0.make_insert();
		for _i in 0..10_000 {
			editor.insert(&mut self.0, (
                Transform(Matrix4::from_scale(1.0)),
                Position(Vector3::unit_x()),
                Rotation(Vector3::unit_x()),
                Velocity(Vector3::unit_x()),
            ));
		}
    }
}

pub struct BenchmarkDyn(Box<World>, Vec<ComponentIndex>);

impl BenchmarkDyn {
    pub fn new() -> Self {
        let mut world = World::create();
        let mut arr = Vec::new();
        let r = (
            world.init_component::<Transform>(),
            world.init_component::<Rotation>(),
            world.init_component::<Velocity>(),
            world.init_component::<Position>(),
        );
        // arr.push(r.0);
        // arr.push(r.2);
        // arr.push(r.3);
        // arr.push(r.1);

        arr.push(r.0);
        arr.push(r.1);
        arr.push(r.2);
        arr.push(r.3);
        Self(world, arr)
    }

    pub fn run(&mut self) {
        
		let mut editor = self.0.make_entity_editor();
		for _i in 0..10_000 {
			editor.insert_entity_by_index(&self.1).unwrap();
		}
    }
}


// fn system(mut reader: EventReader<Transform>) {
//     for event in reader.iter() {
//     }
// }

// fn system1(mut reader: Commands) {
 
// }
// #[test]
// fn t() {
// 	let mut mark = BenchmarkDyn::new();
//     mark.run();
	
// }
