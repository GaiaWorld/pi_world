use cgmath::*;
use ecs::*;
use ecs_derive::*;
use map::vecmap::VecMap;
use atom::Atom;


#[derive(Copy, Clone, Component)]
pub struct Transform(Matrix4<f32>);
#[derive(Copy, Clone, Component)]
pub struct Position(Vector3<f32>);

#[derive(Copy, Clone, Component)]
pub struct Rotation(Vector3<f32>);

#[derive(Copy, Clone, Component)]
pub struct Velocity(Vector3<f32>);

pub struct SimpleIterSystem;

impl<'a> Runner<'a> for SimpleIterSystem<> {
    type ReadData = ();
    type WriteData = (
        &'a mut MultiCaseImpl<Node, Velocity>,
        &'a mut MultiCaseImpl<Node, Position>
    );
    fn run(&mut self, _read: Self::ReadData, (velocity, position): Self::WriteData) {
        for entity in 1..10001 {
            position[entity].0 += velocity[entity].0;
        }
	}
}

impl_system! {
    SimpleIterSystem,
    true,
    {

    }
}
pub struct Benchmark(World, Atom);

impl Benchmark {
    pub fn new() -> Self {
        let mut world = World::default();
		world.register_entity::<Node>();
        world.register_multi::<Node, Transform>();
        world.register_multi::<Node, Position>();
        world.register_multi::<Node, Rotation>();
        world.register_multi::<Node, Velocity>();
		world.register_system(Atom::from("zzz"), CellSimpleIterSystem::new(SimpleIterSystem));
        for _i in 0..10000 {
			let entity = world.create_entity::<Node>();
			world.fetch_multi::<Node, Transform>().unwrap().lend_mut().insert(entity, Transform(Matrix4::<f32>::from_scale(1.0)));
			world.fetch_multi::<Node, Position>().unwrap().lend_mut().insert(entity, Position(Vector3::unit_x()));
			world.fetch_multi::<Node, Rotation>().unwrap().lend_mut().insert(entity, Rotation(Vector3::unit_x()));
			world.fetch_multi::<Node, Velocity>().unwrap().lend_mut().insert(entity, Velocity(Vector3::unit_x()));
		}

		let mut dispatch = SeqDispatcher::default();
		dispatch.build(
			"zzz".to_string(),
			&world,
		);
		let n = Atom::from("zzz");
		world.add_dispatcher(n.clone(), dispatch);
		Self(world, n)
	}

    pub fn run(&mut self) {
        self.0.run(&self.1);
    }
}



