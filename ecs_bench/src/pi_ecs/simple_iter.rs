use cgmath::*;
use pi_ecs::prelude::*;
use map::vecmap::VecMap;
use atom::Atom;


#[derive(Copy, Clone)]
pub struct Transform(Matrix4<f32>);
#[derive(Copy, Clone)]
pub struct Position(Vector3<f32>);

#[derive(Copy, Clone)]
pub struct Rotation(Vector3<f32>);

#[derive(Copy, Clone)]
pub struct Velocity(Vector3<f32>);

pub struct Node;

pub struct Benchmark(World);

impl Benchmark {
    pub fn new() -> Self {
        let mut world = World::new();
        for _i in 0..10000 {
			world.spawn::<Node>()
				.insert(Transform(Matrix4::<f32>::from_scale(1.0)))
				.insert(Position(Vector3::unit_x()))
				.insert(Rotation(Vector3::unit_x()))
				.insert(Velocity(Vector3::unit_x()));
		}

        Self(world)
    }

    pub fn run(&mut self) {
        for (velocity, mut position) in self.0.query::<Node, (&Velocity, &mut Position)>().iter_mut(&mut self.0) {
            position.0 += velocity.0;
        }
    }
}


