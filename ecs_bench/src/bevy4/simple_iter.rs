use bevy_4_ecs::prelude::*;
use cgmath::*;

#[derive(Copy, Clone)]
struct Transform(Matrix4<f32>);

#[derive(Copy, Clone)]
struct Position(Vector3<f32>);

#[derive(Copy, Clone)]
struct Rotation(Vector3<f32>);

#[derive(Copy, Clone)]
struct Velocity(Vector3<f32>);

pub struct Benchmark(World);

impl Benchmark {
    pub fn new() -> Self {
        let mut world = World::new();
        world.spawn_batch((0..10_000).map(|_| {
            (
                Transform(Matrix4::from_scale(1.0)),
                Position(Vector3::unit_x()),
                Rotation(Vector3::unit_x()),
                Velocity(Vector3::unit_x()),
            )
        }));

        Self(world)
    }

    pub fn run(&mut self) {
        for (velocity, mut position) in self.0.query_mut::<(&Velocity, &mut Position)>() {
            position.0 += velocity.0;
        }
    }
}

#[test]
fn tt() {
	let mut world = World::new();
	world.spawn_batch((0..10_000).map(|_| {
		(
			Transform(Matrix4::from_scale(1.0)),
			Position(Vector3::unit_x()),
			Rotation(Vector3::unit_x()),
			Velocity(Vector3::unit_x()),
		)
	}));
	
	// let mut query = world.query::<(&mut Velocity, &mut Position)>();
	for (velocity, mut position) in world.query_mut::<(&Velocity, &mut Position)>() {
		position.0 += velocity.0;
	}
}


