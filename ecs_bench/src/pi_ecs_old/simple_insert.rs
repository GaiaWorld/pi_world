use cgmath::*;
use ecs::*;
use ecs_derive::*;
use map::vecmap::VecMap;

#[derive(Copy, Clone, Component)]
pub struct Transform(Matrix4<f32>);
#[derive(Copy, Clone, Component)]
pub struct Position(Vector3<f32>);

#[derive(Copy, Clone, Component)]
pub struct Rotation(Vector3<f32>);

#[derive(Copy, Clone, Component)]
pub struct Velocity(Vector3<f32>);

pub struct SampleBenchmark;

pub struct Node;

impl SampleBenchmark {
    pub fn new() -> Self {
        Self
    }

    pub fn run(&mut self) {
        let mut world = World::default();
		world.register_entity::<Node>();
        world.register_multi::<Node, Transform>();
        world.register_multi::<Node, Position>();
        world.register_multi::<Node, Rotation>();
        world.register_multi::<Node, Velocity>();

		for _i in 0..10000 {
			let entity = world.create_entity::<Node>();
			world.fetch_multi::<Node, Transform>().unwrap().lend_mut().insert(entity, Transform(Matrix4::<f32>::from_scale(1.0)));
			world.fetch_multi::<Node, Position>().unwrap().lend_mut().insert(entity, Position(Vector3::unit_x()));
			world.fetch_multi::<Node, Rotation>().unwrap().lend_mut().insert(entity, Rotation(Vector3::unit_x()));
			world.fetch_multi::<Node, Velocity>().unwrap().lend_mut().insert(entity, Velocity(Vector3::unit_x()));
		}
    }
}


pub struct QuickBenchmark;
impl QuickBenchmark {
    pub fn new() -> Self {
        Self
    }

    pub fn run(&mut self) {
        let mut world = World::default();
		world.register_entity::<Node>();
        world.register_multi::<Node, Transform>();
        world.register_multi::<Node, Position>();
        world.register_multi::<Node, Rotation>();
        world.register_multi::<Node, Velocity>();

		let transforms = world.fetch_multi::<Node, Transform>().unwrap();
		let positions = world.fetch_multi::<Node, Position>().unwrap();
		let rotations = world.fetch_multi::<Node, Rotation>().unwrap();
		let velocitys = world.fetch_multi::<Node, Velocity>().unwrap();
		for _i in 0..10000 {
			let entity = world.create_entity::<Node>();
            transforms.lend_mut().insert(entity, Transform(Matrix4::<f32>::from_scale(1.0)));
			positions.lend_mut().insert(entity, Position(Vector3::unit_x()));
			rotations.lend_mut().insert(entity, Rotation(Vector3::unit_x()));
			velocitys.lend_mut().insert(entity, Velocity(Vector3::unit_x()));

			// world.fetch_multi::<Node, Transform>().unwrap().lend_mut().insert(entity, Transform(Matrix4::<f32>::from_scale(1.0)));
			// world.fetch_multi::<Node, Position>().unwrap().lend_mut().insert(entity, Position(Vector3::unit_x()));
			// world.fetch_multi::<Node, Rotation>().unwrap().lend_mut().insert(entity, Rotation(Vector3::unit_x()));
			// world.fetch_multi::<Node, Velocity>().unwrap().lend_mut().insert(entity, Velocity(Vector3::unit_x()));
		}
    }
}