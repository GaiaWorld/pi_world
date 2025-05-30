use bevy_15_ecs::prelude::*;
use bevy_15_ecs as bevy_ecs;
use cgmath::*;

#[derive(Copy, Clone, Component)]
struct Transform(Matrix4<f32>);

#[derive(Copy, Clone, Component)]
struct Position(u8);

#[derive(Copy, Clone, Component)]
struct Rotation(Vector3<f32>);

#[derive(Copy, Clone, Component)]
struct Velocity(u8);

pub struct Benchmark(World, pub QueryState<( &'static Velocity, &'static mut Position), ()>);

impl Benchmark {
    pub fn new() -> Self {
        let mut world = World::new();
        world.spawn_batch((0..10_000).map(|_| {
            (
                Transform(Matrix4::from_scale(1.0)),
                Position(0),
                Rotation(Vector3::unit_x()),
                Velocity(0),
            )
        }));
        let query: QueryState<(&Velocity, &mut Position)> = world.query::<(&'static Velocity, &'static mut Position)>();
        Self(world, query)
    }

    pub fn run(&mut self) {
        for (velocity, mut position) in self.1.iter_mut(&mut self.0) {
            position.0 += velocity.0;
        }
    }
}

// #[test]
// fn tt() {
// 	let mut world = World::new();
// 	let i = world.spawn((
// 		Transform(Matrix4::from_scale(1.0)),
// 		Position(Vector3::unit_x()),
// 		Rotation(Vector3::unit_x()),
// 		Velocity(Vector3::unit_x()),
// 	)).id();
// 	world.spawn_batch((0..10_000).map(|_| {
// 		(
// 			Transform(Matrix4::from_scale(1.0)),
// 			Position(Vector3::unit_x()),
// 			Rotation(Vector3::unit_x()),
// 			Velocity(Vector3::unit_x()),
// 		)
// 	}));
	
// 	// let mut query = world.query::<(&mut Velocity, &mut Position)>();
// 	let mut query = world.query::<(&Velocity, &Position)>();
// 	let r = query.get(&world, i);
// 	let r = query.get(&world, i);
// 	// for (velocity, mut position) in query.iter_mut(&mut world) {
// 	// 	position.0 += velocity.0;
// 	// }
// }
