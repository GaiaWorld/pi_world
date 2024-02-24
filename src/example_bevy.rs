use crate::{query::Query, world::Entity, insert::Insert, mutate::Mutate, filter::{Added, Changed}};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Age0(usize);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Age1(usize);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Age2(usize);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Age3(usize);

pub struct Age4(usize);
pub struct Age5([usize;16]);
pub struct Age6(usize);
pub struct Age7(usize);
pub struct Age8(usize);
pub struct Age9(usize);
pub struct Age10(usize);
pub struct Age11(usize);
pub struct Age12(usize);
pub struct Age13(usize);
pub struct Age14(usize);
pub struct Age15(usize);
pub struct Age16(usize);
pub struct Age17(usize);
pub struct Age18(usize);
pub struct Age19(usize);
pub struct Age20(usize);

pub fn insert1(
    i0: Insert<(Age1,Age0,)>,
) {
    println!("insert1 is now");
    let e = i0.insert((Age1(1),Age0(0),));
    println!("insert1 is end, e:{:?}", e);
}
pub fn print_changed_entities(
    // i0: Insert<(Age2,)>,
    mut q0: Query<(Entity, &mut Age0, &mut Age1, &Age2, &Age3, &Age4, &Age5, &Age6, &Age7, &Age8)>,
    // q1: Query<(Entity, &mut Age1)>,
    //q2: Query<(Entity, Option<&mut Age2>)>,
    // q3: Query<(Entity, &mut Age3)>,
) {
    println!("print_changed_entities 1");

    // let q = q0.iter();
    // let s = q.size_hint();
    for (_, mut age0, mut age1, age2, age3, age4, age5, age6, age7, age8) in q0.iter() {
        // let a =1+age2.0+age3.0+age4.0+age6.0+age7.0+age8.0;
        age0.0 +=1+age2.0+age3.0+age4.0+age6.0+age7.0+age8.0;
        // age1.0 +=1+age5.0[0];
    }
    // let q = q0.iter();
    // let s = q.size_hint();
    // {q0.get(e).unwrap().1.0 +=1;}
    // for (_, mut age) in q1.iter() {
    //     age.0 +=1;
    // }
    // for (_, age) in q2.iter() {
    //     if let Some(mut a) = age {
    //         a.0 +=1;
    //     };
    // }
    // for (_, mut age) in q3.iter() {
    //     age.0 +=1;
    // }
    println!("print_changed_entities over");
}
pub fn mutate1(
    mut i0: Mutate<(Age3,), (Age4,)>,
    q0: Query<(Entity, &mut Age0, &mut Age1)>,
) {
    println!("mutate1");
    for (e, _, _) in q0.iter() {
        let r = i0.mutate(e, (Age3(2),));
        println!("e {:?}, r: {:?} is now", e, r);
    }
    println!("mutate1: end");
}
pub fn added_l(
    q0: Query<(Entity, &mut Age1, &mut Age0), (Added<Age1>, Added<Age2>)>,
) {
    println!("add_l");
    for (e, age1, _) in q0.iter() {
        println!("e {:?}, age1: {:?}", e, age1);
    }
 
    println!("add_l: end");
}
pub fn changed_l(
    q0: Query<(Entity, &mut Age1, &mut Age0), (Changed<Age1>, Changed<Age2>)>,
) {
    println!("changed_l");
    for (e, age1, _) in q0.iter() {
        println!("e {:?}, age1: {:?}", e, age1);
    }
 
    println!("changed_l: end");
}




#[cfg(test)]
mod test_mod {
    use crate::{app::*, system::*};
    use test::Bencher;
    use super::*;

    #[test]
    fn test() {
        let mut app = App::new();
        let world = app.get_world();
        let state = world.make_insert_state::<(Age1,Age0,)>();
        let i = Insert::<'_, (Age1,Age0,)>::new(world, &state, world.increment_change_tick());
        let e1 = i.insert((Age1(1),Age0(0),));
        let e2 = i.insert((Age1(1),Age0(0),));
        let s = Box::new(IntoSystem::into_system(print_changed_entities));
        app.register(s);
        app.run();
        assert_eq!(app.get_world().get_component::<Age0>(e1).unwrap().0, 0);
        // assert_eq!(app.get_world().get_component::<Age0>(e2).unwrap().0, 1);
        // assert_eq!(app.get_world().get_component::<Age1>(e1).unwrap().0, 2);
        // assert_eq!(app.get_world().get_component::<Age1>(e2).unwrap().0, 2);
        // app.run();
        // assert_eq!(app.get_world().get_component::<Age0>(e1).unwrap().0, 2);
        // assert_eq!(app.get_world().get_component::<Age0>(e2).unwrap().0, 2);
        // assert_eq!(app.get_world().get_component::<Age1>(e1).unwrap().0, 3);
        // assert_eq!(app.get_world().get_component::<Age1>(e2).unwrap().0, 3);
    }
    #[test]
    fn test_insert() {
        let mut app = App::new();
        let s = Box::new(IntoSystem::into_system(insert1));
        app.register(s);
        let s = Box::new(IntoSystem::into_system(print_changed_entities));
        app.register(s);
        let s = Box::new(IntoSystem::into_system(mutate1));
        app.register(s);

        app.run();
        app.run();
    }
    #[test]
    fn test_added() {
        let mut app = App::new();
        let s = Box::new(IntoSystem::into_system(insert1));
        app.register(s);
        let s = Box::new(IntoSystem::into_system(print_changed_entities));
        app.register(s);
        let s = Box::new(IntoSystem::into_system(mutate1));
        app.register(s);
        let s = Box::new(IntoSystem::into_system(added_l));
        app.register(s);

        app.run();
        app.run();
    }
    #[test]
    fn test_changed() {
        let mut app = App::new();
        let s = Box::new(IntoSystem::into_system(insert1));
        app.register(s);
        let s = Box::new(IntoSystem::into_system(print_changed_entities));
        app.register(s);
        let s = Box::new(IntoSystem::into_system(mutate1));
        app.register(s);
        let s = Box::new(IntoSystem::into_system(changed_l));
        app.register(s);

        app.run();
        app.run();
    }
    #[bench]
    fn bench_test(b: &mut Bencher) {
        let mut app = App::new();
        let world = app.get_world();
        println!("bench_test insert");
        let state = world.make_insert_state::<(Age0,Age1,Age2,Age3,Age4,Age5,Age6,Age7,Age8,Age9,Age10,Age11,Age12,Age13,Age14)>();
        let i = Insert::<'_, (Age0,Age1,Age2,Age3,Age4,Age5,Age6,Age7,Age8,Age9,Age10,Age11,Age12,Age13,Age14,)>::new(world, &state, world.increment_change_tick());
        println!("bench_test insert");
        for _ in 0..90 {
            i.insert((Age0(0),Age1(0),Age2(0),Age3(0),Age4(0),Age5([0;16]),Age6(0),Age7(0),Age8(0),Age9(0),Age10(0),Age11(0),Age12(0),Age13(0),Age14(0)));
        }
        println!("bench_test insert ok");
        for _ in 0..500 {
            let s = Box::new(IntoSystem::into_system(print_changed_entities));
            app.register(s);
        }
        b.iter(move || {
            app.run();
        });
    }

}

#[cfg(test)]
mod test_bevy {
    use bevy_ecs::prelude::*;
    use test::Bencher;

    #[derive(Copy, Clone, Debug, Eq, PartialEq, Component)]
    pub struct Age0(usize);
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Component)]
    pub struct Age1(usize);
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Component)]
    pub struct Age2(usize);
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Component)]
    pub struct Age3(usize);
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Component)]
    pub struct Age4(usize);
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Component)]
    pub struct Age5([usize;16]);
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Component)]
    pub struct Age6(usize);
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Component)]
    pub struct Age7(usize);
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Component)]
    pub struct Age8(usize);
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Component)]
    pub struct Age9(usize);
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Component)]
    pub struct Age10(usize);
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Component)]
    pub struct Age11(usize);
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Component)]
    pub struct Age12(usize);
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Component)]
    pub struct Age13(usize);
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Component)]
    pub struct Age14(usize);
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Component)]
    pub struct Age15(usize);
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Component)]
    pub struct Age16(usize);
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Component)]
    pub struct Age17(usize);
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Component)]
    pub struct Age18(usize);
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Component)]
    pub struct Age19(usize);
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Component)]
    pub struct Age20(usize);
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Component)]
    pub struct Age21(usize);
    
    // pub fn print_changed_entities(mut set: ParamSet<(Query<(Entity, &mut Age0)>,
    //     Query<(Entity, &mut Age1)>,
    //     Query<(Entity, &mut Age2)>,
    //     Query<(Entity, &mut Age3)>)>
    // ) {
    pub fn print_changed_entities(
        mut q0: Query<(Entity, &mut Age0, &mut Age1, &Age2, &Age3, &Age4, &Age5, &Age6, &Age7, &Age8)>,
        //mut q1: Query<(Entity, &mut Age1)>,
        //mut q2: Query<(Entity, Option<&mut Age2>)>,
        //mut q3: Query<(Entity, &mut Age3)>,
    ) {
        // let q = q0.iter_mut();
        // let s = q.size_hint();
        // for (_, age) in q0.iter_mut() {
        //     //age.0 +=1;
        // }
        //let q = q0.iter_mut();
        //let s = q.size_hint();
        for (_, mut age0, mut age1, age2, age3, age4, age5, age6, age7, age8) in q0.iter_mut() {
            let a =1+age2.0+age3.0+age4.0+age6.0+age7.0+age8.0;
            // age0.0 +=1+age2.0+age3.0+age4.0+age6.0+age7.0+age8.0;
            // age1.0 +=1+age5.0[0];
        }
        // for (_, age) in q2.iter_mut() {
        //     if let Some(mut a) = age {
        //         a.0 +=1;
        //     };
        // }
        // for (_, mut age) in q3.iter_mut() {
        //     age.0 +=1;
        // }
    }
    #[bench]
    fn bench_bevy(b: &mut Bencher) {
        // Create a new empty World to hold our Entities, Components and Resources
        let mut world = World::new();
        for _ in 0..900 {
            world.spawn((Age0(0),Age1(0),Age2(0),Age3(0),Age4(0),Age5([0;16]),Age6(0),Age7(0),Age8(0),Age9(0),Age10(0),Age11(0),Age12(0),Age13(0),Age14(0)));
        }
        // Add the counter resource to remember how many entities where spawned
        //world.insert_resource(EntityCounter { value: 0 });

        // Create a new Schedule, which stores systems and controls their relative ordering
        let mut schedule = Schedule::default();
        for _ in 0..500 {
            schedule.add_systems(print_changed_entities);
        }
        // Add systems to the Schedule to execute our app logic
        // We can label our systems to force a specific run-order between some of them
        // schedule.add_systems((
        //     spawn_entities.in_set(SimulationSet::Spawn),
        //     print_counter_when_changed.after(SimulationSet::Spawn),
        //     age_all_entities.in_set(SimulationSet::Age),
        //     remove_old_entities.after(SimulationSet::Age),
        //     print_changed_entities.after(SimulationSet::Age),
        // ));

        // Simulate 10 frames in our world
        // for iteration in 1..=10 {
        //     println!("Simulating frame {iteration}/10");
        //     schedule.run(&mut world);
        // }
        b.iter(move || {
            schedule.run(&mut world);
        });
    }
}

