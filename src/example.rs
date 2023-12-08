use crate::{query::Query, world::Entity, insert::Insert, mutate::Mutate, filter::Added};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Age0(usize);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Age1(usize);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Age2(usize);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Age3(usize);

pub fn insert1(
    mut i0: Insert<(Age1,Age0,)>,
) {
    println!("insert1 is now");
    let e = i0.insert((Age1(1),Age0(0),));
    println!("insert1 is end, e:{:?}", e);
}
pub fn print_changed_entities(
    mut i0: Insert<(Age2,)>,
    q0: Query<(Entity, &mut Age0)>,
    q1: Query<(Entity, &mut Age1)>,
    q2: Query<(Entity, Option<&mut Age2>)>,
    q3: Query<(Entity, &mut Age3)>,
) {
    println!("print_changed_entities print1");
    let e = i0.insert((Age2(2),));
    println!("print_changed_entities insert: {:?}", e);
    {
        for (e, mut age) in q0.iter() {
        age.0 +=1;
        println!("print_changed_entities: e {:?}, c0: {:?} is now", e, age);
        }
    }
    // {q0.get(e).unwrap().1.0 +=1;}
    println!("print_changed_entities print3");
    for (e, mut age) in q1.iter() {
        age.0 +=1;
        println!("print_changed_entities1 e {:?}, c1: {:?} is now", e, age);
    }
    for (e, age) in q2.iter() {
        //age.0 +=1;
        println!("print_changed_entities2 e {:?}, c2: {:?} is now", e, age);
    }
    for (_, mut age) in q3.iter() {
        age.0 +=1;
        println!("print_changed_entities3 c: {:?} is now", age);
    }
    println!("print_changed_entities: end");
}
pub fn mutate1(
    mut i0: Mutate<((Age3,),)>,
    q0: Query<(Entity, &mut Age0, &mut Age1)>,
) {
    println!("mutate1");
    for (e, _, _) in q0.iter() {
        let r = i0.mutate(e, (Age3(2),));
        println!("e {:?}, r: {:?} is now", e, r);
    }
    println!("mutate1: end");
}
pub fn add_l(
    q0: Query<(Entity, &mut Age1, &mut Age0), (Added<Age1>, Added<Age2>)>,
) {
    println!("add_l");
    for (e, age1, _) in q0.iter() {
        println!("e {:?}, age1: {:?}", e, age1);
    }
 
    println!("add_l: end");
}




#[cfg(test)]
mod test_mod {
    use crate::{app::*, system::*};
    use test::Bencher;
    use super::*;

    #[test]
    fn test() {
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
    fn test_add() {
        let mut app = App::new();
        let s = Box::new(IntoSystem::into_system(insert1));
        app.register(s);
        let s = Box::new(IntoSystem::into_system(print_changed_entities));
        app.register(s);
        let s = Box::new(IntoSystem::into_system(mutate1));
        app.register(s);
        let s = Box::new(IntoSystem::into_system(add_l));
        app.register(s);

        app.run();
        app.run();
    }
    #[bench]
    fn bench_test(b: &mut Bencher) {
        let mut app = App::new();
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
    
    pub fn print_changed_entities(mut set: ParamSet<(Query<(Entity, &mut Age0)>,
        Query<(Entity, &mut Age1)>,
        Query<(Entity, &mut Age2)>,
        Query<(Entity, &mut Age3)>)>
    ) {
    // pub fn print_changed_entities(
    //     mut q0: Query<(Entity, &mut Age0)>,
    //     mut q1: Query<(Entity, &mut Age1)>,
    //     mut q2: Query<(Entity, &mut Age2)>,
    //     mut q3: Query<(Entity, &mut Age3)>,
    // ) {
        // for (_, mut age) in q0.iter_mut() {
        //     age.0 +=1;
        //     //println!("c: {:?} is now", age);
        // }
        // for (_, mut age) in q1.iter_mut() {
        //     age.0 +=1;
        //     //println!("c: {:?} is now", age);
        // }
        // for (_, mut age) in q2.iter_mut() {
        //     age.0 +=1;
        //     //println!("c: {:?} is now", age);
        // }
        // for (_, mut age) in q3.iter_mut() {
        //     age.0 +=1;
        //     //println!("c: {:?} is now", age);
        // }
    }
    #[bench]
    fn bench_bevy(b: &mut Bencher) {
        // Create a new empty World to hold our Entities, Components and Resources
        let mut world = World::new();

        // Add the counter resource to remember how many entities where spawned
        //world.insert_resource(EntityCounter { value: 0 });

        // Create a new Schedule, which stores systems and controls their relative ordering
        let mut schedule = Schedule::default();
        for _ in 0..500 {
            schedule.add_systems((print_changed_entities));
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

