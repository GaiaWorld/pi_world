use cgmath::*;
use pi_ecs::prelude::*;
use map::vecmap::VecMap;

#[derive(Copy, Clone)]
pub struct Transform(Matrix4<f32>);
#[derive(Copy, Clone)]
pub struct Position(Vector3<f32>);

#[derive(Copy, Clone)]
pub struct Rotation(Vector3<f32>);

#[derive(Copy, Clone)]
pub struct Velocity(Vector3<f32>);


pub struct Node;

pub struct SampleBenchmark;

impl SampleBenchmark {
    pub fn new() -> Self {
        Self
    }

    pub fn run(&mut self) {
        let mut world = World::new();

		for _i in 0..10000 {
			world.spawn::<Node>()
				.insert(Transform(Matrix4::<f32>::from_scale(1.0)))
				.insert(Position(Vector3::unit_x()))
				.insert(Rotation(Vector3::unit_x()))
				.insert(Velocity(Vector3::unit_x()));
		}
    }
}

pub struct QuickBenchmark;

impl QuickBenchmark {
    pub fn new() -> Self {
        Self
    }

    pub fn run(&mut self) {
        let mut world = World::new();

		let query: QueryState<Node, (Write<Transform>, Write<Position>, Write<Rotation>, Write<Velocity>)> = world.query();
		for _i in 0..10000 {
			let e = world.spawn::<Node>().id();
			let mut r = unsafe { query.get_unchecked(&mut world, e) };
			r.0.write(Transform(Matrix4::<f32>::from_scale(1.0)));
			r.1.write(Position(Vector3::unit_x()));
			r.2.write(Rotation(Vector3::unit_x()));
			r.3.write(Velocity(Vector3::unit_x()));
		}
    }
}