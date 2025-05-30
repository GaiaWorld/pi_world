use bevy_11_ecs::prelude::*;
// use bevy_11_ecs::system::EventReader;
use cgmath::*;

#[derive(Copy, Clone)]
struct Transform(Matrix4<f32>);

#[derive(Copy, Clone)]
struct Position(Vector3<f32>);

#[derive(Copy, Clone)]
struct Rotation(Vector3<f32>);

#[derive(Copy, Clone)]
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

fn system1(mut reader: Commands) {
 
}
#[test]
fn t() {
	let mut world = World::new();

	let mut stage = SystemStage::parallel();
	stage.add_system(system1.system());

	stage.run(&mut world);

	// world.add_s
	for _i in 0..10_000 {
		world.spawn().insert_bundle((
			Transform(Matrix4::from_scale(1.0)),
			Position(Vector3::unit_x()),
			Rotation(Vector3::unit_x()),
			Velocity(Vector3::unit_x()),
		));
	}

	
}
