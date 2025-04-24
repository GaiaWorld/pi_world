use bevy_11_ecs::prelude::*;
use bevy_11_ecs as bevy_ecs;
// use bevy_11_ecs_macros::Component;
// use bevy_5_ecs::system::EventReader;
use cgmath::*;

#[derive(Copy, Clone, Component)]
struct Transform(Matrix4<f32>);

#[derive(Copy, Clone, Component)]
struct Position(Vector3<f32>);

#[derive(Copy, Clone, Component)]
struct Rotation(Vector3<f32>);

#[derive(Copy, Clone, Component)]
struct Velocity(Vector3<f32>);

pub struct Benchmark(pub World);

impl Benchmark {
    pub fn new() -> Self {
        Self(World::new())
    }

    pub fn run(&mut self) {
		for _i in 0..10_000 {
			self.0.spawn((
                Transform(Matrix4::from_scale(1.0)),
                Position(Vector3::unit_x()),
                Rotation(Vector3::unit_x()),
                Velocity(Vector3::unit_x()),
            ));
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
// 	let mut world = World::new();

// 	let mut stage = SystemStage::parallel();
// 	stage.add_system(system1);

// 	stage.run(&mut world);

// 	// world.add_s
// 	for _i in 0..10_000 {
// 		world.spawn((
// 			Transform(Matrix4::from_scale(1.0)),
// 			Position(Vector3::unit_x()),
// 			Rotation(Vector3::unit_x()),
// 			Velocity(Vector3::unit_x()),
// 		));
// 	}

	
// }
