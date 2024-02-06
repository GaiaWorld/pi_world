// use crate::{query::Query, world::Entity, insert::Insert, mutate::Mutate, filter::{Added, Changed}};

// #[derive(Copy, Clone, Debug, Eq, PartialEq)]
// pub struct Age0(usize);

// #[derive(Copy, Clone, Debug, Eq, PartialEq)]
// pub struct Age1(usize);

// #[derive(Copy, Clone, Debug, Eq, PartialEq)]
// pub struct Age2(usize);

// #[derive(Copy, Clone, Debug, Eq, PartialEq)]
// pub struct Age3(usize);

// pub struct Age4(usize);
// pub struct Age5([usize;16]);
// pub struct Age6(usize);
// pub struct Age7(usize);
// pub struct Age8(usize);
// pub struct Age9(usize);
// pub struct Age10(usize);
// pub struct Age11(usize);
// pub struct Age12(usize);
// pub struct Age13(usize);
// pub struct Age14(usize);
// pub struct Age15(usize);
// pub struct Age16(usize);
// pub struct Age17(usize);
// pub struct Age18(usize);
// pub struct Age19(usize);
// pub struct Age20(usize);

// pub fn print_changed_entities(
//     mut q0: Query<(Entity, &mut Age0, &mut Age1, &Age2, &Age3, 
//         //&Age4, &Age5, &Age6, &Age7, &Age8
//     )>,
// ) {
//     for (_, mut age0, age1, age2, age3,
//         //age4, age5, age6, age7, age8
//     ) in q0.iter() {
//         //let a =1+age1.0+age2.0+age3.0+age4.0+age5.0[0]+age6.0+age7.0+age8.0;
//         age0.0 +=1+age1.0+age2.0+age3.0;//+age4.0+age5.0[0]+age6.0+age7.0+age8.0;
//     }
// }
// #[cfg(test)]
// mod test_mod {
//     use crate::{app::*, system::*};
//     use test::Bencher;
//     use super::*;

//     #[bench]
//     fn bench_test(b: &mut Bencher) {
//         let mut app = App::new();
//         let world = app.get_world();
//         let state = world.make_insert_state::<(Age0,Age1,Age2,Age3,
//          Age4,Age5,Age6,Age7,Age8,Age9,Age10,Age11,Age12,Age13,Age14
//         )>();
//         let i = Insert::<'_, (Age0,Age1,Age2,Age3,
//              Age4,Age5,Age6,Age7,Age8,Age9,Age10,Age11,Age12,Age13,Age14,
//         )>::new(world, &state, world.increment_change_tick());
//         for _ in 0..2000 {
//             i.insert((Age0(0),Age1(0),Age2(0),Age3(0),
//              Age4(0),Age5([0;16]),Age6(0),Age7(0),Age8(0),Age9(0),Age10(0),Age11(0),Age12(0),Age13(0),Age14(0)
//         ));
//         }
//         for _ in 0..500 {
//             let s = Box::new(IntoSystem::into_system(print_changed_entities));
//             app.register(s);
//         }
//         b.iter(move || {
//             app.run();
//         });
//     }

// }

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
    
    pub fn print_changed_entities(
        mut q0: Query<(Entity, &mut Age0, &mut Age1, &Age2, &Age3,
            //&Age4, &Age5, &Age6, &Age7, &Age8
            )>,
    ) {
        for (_, mut age0, age1, age2, age3,
            //age4, age5, age6, age7, age8
            ) in q0.iter_mut() {
            //let a =1+age1.0 +age2.0+age3.0+age4.0+age5.0[0]+age6.0+age7.0+age8.0;
            age0.0 +=1+age1.0 +age2.0+age3.0;//+age4.0+age5.0[0]+age6.0+age7.0+age8.0;
        }
    }
    #[bench]
    fn bench_bevy(b: &mut Bencher) {
        let mut world = World::new();
        for _ in 0..2000 {
        //     world.spawn((Age0(0),Age1(0),Age2(0),Age3(0),
        //     Age4(0),Age5([0;16]),Age6(0),Age7(0),Age8(0),Age9(0),Age10(0),Age11(0),Age12(0),Age13(0),Age14(0)
        // ));
        }
        let mut schedule = Schedule::default();
        for _ in 0..500 {
            schedule.add_systems(print_changed_entities);
        }
        b.iter(move || {
            schedule.run(&mut world);
        });
    }
}

