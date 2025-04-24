use bevy_15_ecs::component::ComponentId;
use bevy_15_ecs::prelude::*;
use bevy_15_ecs::ptr::OwningPtr;
use bevy_15_ecs as bevy_ecs;
// use bevy_15_ecs_macros::Component;
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

pub struct BenchmarkDyn(pub World, Vec<ComponentId>);

impl BenchmarkDyn {
    pub fn new() -> Self {
        let mut world = World::new();
        let mut arr = Vec::new();
        let r = (
            world.register_component::<Transform>(),
            world.register_component::<Rotation>(),
            world.register_component::<Velocity>(),
            world.register_component::<Position>(),
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
		for _i in 0..10_000 {
            let mut entity = self.0.spawn_empty();
            OwningPtr::make(Transform(Matrix4::from_scale(1.0)), |ptr1| {
                // SAFETY: `ptr` matches the component id
                OwningPtr::make(Position(Vector3::unit_x()), |ptr2| {
                    OwningPtr::make(Rotation(Vector3::unit_x()), |ptr3| {
                        // SAFETY: `ptr1` and `ptr2` match the component ids
                        OwningPtr::make(Velocity(Vector3::unit_x()), |ptr4| {
                            // SAFETY: `ptr1` and `ptr2` match the component ids
                            unsafe { entity.insert_by_ids(&self.1, vec![ptr1, ptr2, ptr3, ptr4].into_iter()) };
                        });
                    });
                });
            });
        
            // let components: Vec<_> = world.query::<&TestComponent>().iter(&world).collect();

			// self.0.spawn((
            //     Transform(Matrix4::from_scale(1.0)),
            //     Position(Vector3::unit_x()),
            //     Rotation(Vector3::unit_x()),
            //     Velocity(Vector3::unit_x()),
            // ));
		}
    }
}


    // OwningPtr::make(TestComponent(84), |ptr| {
    //     // SAFETY: `ptr` matches the component id
    //     unsafe { entity.insert_by_ids(&[test_component_id], vec![ptr].into_iter()) };
    // });

    // let components: Vec<_> = world.query::<&TestComponent>().iter(&world).collect();

    // assert_eq!(components, vec![&TestComponent(42), &TestComponent(84)]);


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
