use crate::prelude::*;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Age0(usize);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Age1(usize);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Age2(usize);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Age3(usize);

pub struct Age4(usize);
pub struct Age5([usize; 16]);
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

pub fn insert1(i0: Insert<(Age1, Age0)>) {
    println!("insert1 is now");
    let e = i0.insert((Age1(1), Age0(0)));
    println!("insert1 is end, e:{:?}", e);
}
pub fn print_changed_entities(
    // i0: Insert<(Age2,)>,
    mut q0: Query<(
        Entity,
        &mut Age0,
        &mut Age1,
        // &Age2, &Age3, &Age4, &Age5, &Age6, &Age7, &Age8
    )>,
    // q1: Query<(Entity, &mut Age1)>,
    //q2: Query<(Entity, Option<&mut Age2>)>,
    // q3: Query<(Entity, &mut Age3)>,
) {
    println!("print_changed_entities {:?}", q0.iter().size_hint());
    // let q = q0.iter();
    // let s = q.size_hint();
    let q = q0.iter_mut();
    for (
        e,
        mut age0,
        age1,
        // age2, age3, age4, age5, age6, age7, age8
    ) in q
    {
        // let a =1+age2.0+age3.0+age4.0+age6.0+age7.0+age8.0;
        age0.0 += 1 + age1.0;
        //+age2.0+age3.0+age4.0+age6.0+age7.0+age8.0;
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
pub fn alter1(
    mut i0: Alter<&Age2, (), (Age3,), (Age4,)>,
    q0: Query<(Entity, &mut Age0, &mut Age1)>,
) {
    println!("alter1");
    for (e, _, _) in q0.iter() {
        let r = i0.alter(e, (Age3(2),));
        dbg!(e, r);
    }
    println!("alter1: end");
}
pub fn added_l(q0: Query<(Entity, &mut Age1, &mut Age0), (Added<Age1>, Added<Age2>)>) {
    println!("add_l");
    for (e, age1, _) in q0.iter() {
        println!("e {:?}, age1: {:?}", e, age1);
    }
    println!("add_l: end");
}
pub fn changed_l(q0: Query<(Entity, &mut Age0, &mut Age1), (Changed<Age0>, Changed<Age2>)>) {
    println!("changed_l");
    for (e, age0, _) in q0.iter() {
        println!("e {:?}, age0: {:?}", e, age0);
    }

    println!("changed_l: end");
}
pub fn p_set(
    mut set: ParamSet<(Query<(&mut Age0, &mut Age1)>, Query<(&mut Age1, &mut Age2)>)>,
    // r10: Res<Age10>,
    // r11: Res<Age11>,
) {
    println!("p_set");
    for (age0, age1) in set.0.iter_mut() {
        // dbg!(age0, age1);
    }
    println!("p_set1");
    set.1.iter_mut().for_each(|(age1, age2)| {
        // dbg!(age1, age2);
    });
    println!("p_set: end");
}
pub fn print_e(
    // i0: Insert<(Age2,)>,
    q0: Query<(
        Entity,
        &Age0,
        &Age1,
        // &Age2, &Age3, &Age4, &Age5, &Age6, &Age7, &Age8
    )>,
    // q1: Query<(Entity, &mut Age1)>,
    //q2: Query<(Entity, Option<&mut Age2>)>,
    // q3: Query<(Entity, &mut Age3)>,
) {
    println!("print_e");
    for (e, age0, age1) in q0.iter() {
        println!("print_e: e {:?}, age0: {:?}, age1: {:?}", e, age0, age1);
    }
    println!("print_e: end");
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]

struct A(u32);
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct B(u32);

#[derive(Copy, Clone, Debug)]
struct Transform([f32; 16]);

#[derive(Copy, Clone)]
struct Position([f32; 3]);

#[derive(Copy, Clone)]
struct Rotation([f32; 3]);

#[derive(Copy, Clone)]
struct Velocity([f32; 3]);

#[cfg(test)]
mod test_mod {

    use super::*;
    use crate::{
        app::*,
        archetype::{ComponentInfo, Row},
        column::Column,
        table::Table,
    };
    use pi_append_vec::AppendVec;
    use pi_async_rt::{
        rt::single_thread::SingleTaskRuntime,
    };
    use pi_null::Null;
    use test::Bencher;

    #[test]
    fn test_columns() {
        let mut c = Column::new(ComponentInfo::of::<Transform>());
        c.write(0, Transform([0.0; 16]));
        c.write(1, Transform([1.0; 16]));
        dbg!(c.get::<Transform>(0));
        dbg!(c.get::<Transform>(1));
        let mut action = Default::default();
        c.collect(2, &mut action);
        dbg!(c.get::<Transform>(0));
        dbg!(c.get::<Transform>(1));
    }

    #[test]
    fn test_removes() {
        let mut action = Default::default();
        let mut set = Default::default();
        let mut removes: AppendVec<Row> = Default::default();
        removes.insert(1);
        removes.insert(2);
        //removes.insert(0);
        let len = Table::removes_action(&removes, removes.len(), 7, &mut action, &mut set);
        assert_eq!(len, 5);
        assert_eq!(action.len(), 2);
        assert_eq!(action[0], (6, 1));
        assert_eq!(action[1], (5, 2));
        removes.clear();
        removes.insert(1);
        removes.insert(6);
        //removes.insert(0);
        let len = Table::removes_action(&removes, removes.len(), 7, &mut action, &mut set);
        assert_eq!(len, 5);
        assert_eq!(action.len(), 1);
        assert_eq!(action[0], (5, 1));
    }
    #[test]
    fn test() {
        let mut app = App::<SingleTaskRuntime>::new();
        let i = app.world.make_inserter::<(Age1, Age0)>();
        let e1 = i.insert((Age1(1), Age0(0)));
        let e2 = i.insert((Age1(1), Age0(0)));
        app.schedule.add_system(print_changed_entities);
        app.initialize();
        app.run();
        app.run();
        assert_eq!(app.world.get_component::<Age0>(e1).unwrap().0, 4);
        // assert_eq!(app.world.get_component::<Age0>(e2).unwrap().0, 1);
        // assert_eq!(app.world.get_component::<Age1>(e1).unwrap().0, 2);
        // assert_eq!(app.world.get_component::<Age1>(e2).unwrap().0, 2);
        // app.run();
        // assert_eq!(app.world.get_component::<Age0>(e1).unwrap().0, 2);
        // assert_eq!(app.world.get_component::<Age0>(e2).unwrap().0, 2);
        // assert_eq!(app.world.get_component::<Age1>(e1).unwrap().0, 3);
        // assert_eq!(app.world.get_component::<Age1>(e2).unwrap().0, 3);
    }
    #[test]
    fn test_insert() {
        let mut app = App::<SingleTaskRuntime>::new();
        app.schedule.add_system(insert1);
        app.schedule.add_system(print_changed_entities);
        app.initialize();
        app.run();
        app.run();
    }
    #[test]
    fn test_add_remove() {
        let mut world = World::new();
        let i = world.make_inserter::<(A,)>();
        let entities = (0..10_000).map(|_| i.insert((A(0),))).collect::<Vec<_>>();
        world.collect();
        {
            let mut alter = world.make_alterer::<(), (With<A>,), (B,), ()>();
            let mut it = alter.iter_mut();
            while let Some(_) = it.next() {
                let _ = it.alter((B(0),));
            }
        }
        for e in &entities {
            assert_eq!(world.get_component::<B>(*e).is_ok(), true)
        }
        {
            let mut alter = world.make_alterer::<(), (With<A>, With<B>), (), (B,)>();
            let mut it = alter.iter_mut();
            while let Some(_) = it.next() {
                let _ = it.alter(());
            }
        }
        for e in entities {
            assert_eq!(world.get_component::<B>(e).is_err(), true)
        }
    }
    #[bench]
    fn bench_heavy_compute(b: &mut Bencher) {
        use cgmath::*;

        #[derive(Copy, Clone)]
        struct Mat(Matrix4<f32>);

        #[derive(Copy, Clone)]
        struct Position(Vector3<f32>);

        #[derive(Copy, Clone)]
        struct Rotation(Vector3<f32>);

        #[derive(Copy, Clone)]
        struct Velocity(Vector3<f32>);
        let mut world = World::new();
        let i = world.make_inserter::<(Mat, Position, Rotation, Velocity)>();
        i.batch((0..1000).map(|_| {
            (
                Mat(Matrix4::from_scale(1.0)),
                Position(Vector3::unit_x()),
                Rotation(Vector3::unit_x()),
                Velocity(Vector3::unit_x()),
            )
        }));
        world.collect();
        let query = world.make_queryer::<(&mut Position, &mut Mat), ()>();
        println!("query, {:?}", query.iter().size_hint());
        b.iter(move || {
            let mut query = world.make_queryer::<(&mut Position, &mut Mat), ()>();

            query.iter_mut().for_each(|(mut pos, mut mat)| {
                //let mat = &mut *mat;
                for _ in 0..100 {
                    *mat = Mat(mat.0.invert().unwrap());
                }

                pos.0 = mat.0.transform_vector(pos.0);
            });
        });
    }
    #[bench]
    fn bench_simple_insert(b: &mut Bencher) {
        b.iter(move || {
            let mut world = World::new();
            let iter = (0..10_000).map(|a| {
                (
                    Transform([a as f32; 16]),
                    Position([a as f32; 3]),
                    Rotation([a as f32; 3]),
                    Velocity([a as f32; 3]),
                )
            });
            let i = world.make_inserter::<(Transform, Position, Rotation, Velocity)>();
            i.batch(iter);
            // let i = world.make_inserter::<(Transform,Position,Rotation, Velocity)>();
            // for a in 0..9990 {
            //     i.insert((
            //         Transform([a as f32; 16]),
            //         Position([a as f32; 3]),
            //         Rotation([a as f32; 3]),
            //         Velocity([a as f32; 3]),
            //     ));
            // };
        });
    }
    #[test]
    pub fn simple_insert() {
        for _ in 0..1 {
            let mut world = World::new();
            let i = world.make_inserter::<(Transform, Position, Rotation, Velocity)>();
            let mut e = Entity::null();
            for a in 0..10_000 {
                e = i.insert((
                    Transform([a as f32; 16]),
                    Position([a as f32; 3]),
                    Rotation([a as f32; 3]),
                    Velocity([a as f32; 3]),
                ));
            }
            assert_eq!(world.get_component::<Transform>(e).unwrap().0[0], 9999f32);
        }
    }

    #[test]
    fn test_query() {
        let world = World::new();
        let mut w = world.unsafe_world();
        let mut w1 = world.unsafe_world();
        let i = w.make_inserter::<(Age1, Age0)>();
        let _i1 = w1.make_inserter::<(Age2, Age3)>();
        let e1 = i.insert((Age1(1), Age0(0)));
        let e2 = i.insert((Age1(1), Age0(0)));
        // world.collect();
        let mut q = world.make_queryer::<(&Age1, &mut Age0), ()>();
        for (a, mut b) in q.iter_mut() {
            b.0 += a.0;
        }
        assert_eq!(world.get_component::<Age0>(e1).unwrap().0, 1);
        assert_eq!(world.get_component::<Age0>(e2).unwrap().0, 1);
    }

    #[test]
    fn test_alter() {
        let mut app = SingleThreadApp::new();
        app.schedule.add_system(insert1);
        app.schedule.add_system(print_changed_entities);
        app.schedule.add_system(alter1);
        app.schedule.add_system(p_set);
        app.initialize();
        app.run();
        app.run();
        app.run();
    }
    #[test]
    fn test_alter1() {
        let mut world = World::new();
        let i = world.make_inserter::<(Age1, Age0)>();
        let e1 = i.insert((Age1(2), Age0(1)));
        let e2 = i.insert((Age1(4), Age0(2)));
        world.collect();
        {
            let mut alter = world.make_alterer::<(&Age1, &mut Age0), (), (Age2,), ()>();
            let mut it = alter.iter_mut();
            while let Some((a, mut b)) = it.next() {
                if a.0 == 2 {
                    b.0 += 1;
                } else {
                    it.alter((Age2(a.0),)).unwrap();
                }
            }
        }
        world.collect();
        assert_eq!(world.get_component::<Age0>(e1).unwrap().0, 2);
        assert_eq!(world.get_component::<Age2>(e1).is_err(), true);
        assert_eq!(world.get_component::<Age0>(e2).unwrap().0, 2);
        assert_eq!(world.get_component::<Age1>(e2).unwrap().0, 4);
        assert_eq!(world.get_component::<Age2>(e2).unwrap().0, 4);
    }
    #[test]
    fn test_alter2() {
        let mut world = World::new();
        let i = world.make_inserter::<(Age0,)>();
        let _entities = (0..10_000)
            .map(|_| i.insert((Age0(0),)))
            .collect::<Vec<_>>();
        world.collect();
        {
            let mut alter = world.make_alterer::<(&Age0,), (), (Age1,), ()>();
            let mut it = alter.iter_mut();
            while let Some(_) = it.next() {
                let _ = it.alter((Age1(0),));
            }
        }
        {
            let mut alter = world.make_alterer::<(), (With<Age0>, With<Age1>), (), (Age1,)>();
            let mut it = alter.iter_mut();
            while let Some(_) = it.next() {
                let _ = it.alter(());
            }
        }
    }

    #[test]
    fn test_added() {
        let mut app = App::<SingleTaskRuntime>::new();
        app.schedule.add_system(insert1);
        app.schedule.add_system(print_changed_entities);
        app.schedule.add_system(added_l);
        app.schedule.add_system_stages(alter1, &["add"]);
        app.initialize();
        app.run_stage("add");
        app.run_stage("add");
    }
    #[test]
    fn test_changed() {
        let mut app = App::<SingleTaskRuntime>::new();
        app.schedule.add_system(insert1);
        app.schedule.add_system(print_changed_entities);
        app.schedule.add_system(alter1);
        app.schedule.add_system(changed_l);
        app.initialize();
        app.run();
        app.run();
    }

    #[test]
    fn test_schedule() {
        struct A(f32);
        struct B(f32);
        struct C(f32);
        struct D(f32);
        struct E(f32);

        fn ab(mut query: Query<(&mut A, &mut B)>) {
            for (mut a, mut b) in query.iter_mut() {
                std::mem::swap(&mut a.0, &mut b.0);
            }
        }

        fn cd(mut query: Query<(&mut C, &mut D)>) {
            for (mut c, mut d) in query.iter_mut() {
                std::mem::swap(&mut c.0, &mut d.0);
            }
        }

        fn ce(mut query: Query<(&mut C, &mut E)>) {
            for (mut c, mut e) in query.iter_mut() {
                std::mem::swap(&mut c.0, &mut e.0);
            }
        }
        let mut app = MultiThreadApp::new();
        let i = app.world.make_inserter::<(A, B)>();
        let it = (0..10_000).map(|_| (A(0.0), B(0.0)));
        i.batch(it);

        let i = app.world.make_inserter::<(A, B, C)>();
        let it = (0..10_000).map(|_| (A(0.0), B(0.0), C(0.0)));
        i.batch(it);

        let i = app.world.make_inserter::<(A, B, C, D)>();
        let it = (0..10_000).map(|_| (A(0.0), B(0.0), C(0.0), D(0.0)));
        i.batch(it);

        let i = app.world.make_inserter::<(A, B, C, E)>();
        let it = (0..10_000).map(|_| (A(0.0), B(0.0), C(0.0), E(0.0)));
        i.batch(it);

        app.world.collect();
        app.schedule.add_system(ab);
        app.schedule.add_system(cd);
        app.schedule.add_system(ce);
        app.initialize();
        app.run();
        for _ in 0..1000 {
            app.run();
        }
    }

    #[test]
    fn test_async_schedule() {
        struct A(f32);
        struct B(f32);
        struct C(f32);
        struct D(f32);
        struct E(f32);

        fn ab(
            mut local: Local<usize>,
            mut query: Query<(&mut A, &mut B)>,
        ) {
            for (mut a, mut b) in query.iter_mut() {
                std::mem::swap(&mut a.0, &mut b.0);
            }
            *local += 1;
        }
        async fn ab1<'w>(
            mut local: Local<'w, usize>,
            mut query: Query<'w, (&mut A, &mut B)>,
        ) {
            for (mut a, mut b) in query.iter_mut() {
                std::mem::swap(&mut a.0, &mut b.0);
            }
            *local += 1;
        }
        async fn ab5(
            mut local: Local<'static, usize>,
            mut query: Query<'static, (&mut A, &mut B)>,
        ) {
            for (mut a, mut b) in query.iter_mut() {
                std::mem::swap(&mut a.0, &mut b.0);
            }
            *local += 1;
        }
        fn cd(mut query: Query<(&mut C, &mut D)>) {
            for (mut c, mut d) in query.iter_mut() {
                std::mem::swap(&mut c.0, &mut d.0);
            }
        }

        fn ce(mut query: Query<(&mut C, &mut E)>) {
            for (mut c, mut e) in query.iter_mut() {
                std::mem::swap(&mut c.0, &mut e.0);
            }
        }

        
        let mut app = MultiThreadApp::new();
        let i = app.world.make_inserter::<(A, B)>();
        let it = (0..10_000).map(|_| (A(0.0), B(0.0)));
        i.batch(it);

        let i = app.world.make_inserter::<(A, B, C)>();
        let it = (0..10_000).map(|_| (A(0.0), B(0.0), C(0.0)));
        i.batch(it);

        let i = app.world.make_inserter::<(A, B, C, D)>();
        let it = (0..10_000).map(|_| (A(0.0), B(0.0), C(0.0), D(0.0)));
        i.batch(it);

        let i = app.world.make_inserter::<(A, B, C, E)>();
        let it = (0..10_000).map(|_| (A(0.0), B(0.0), C(0.0), E(0.0)));
        i.batch(it);

        app.world.collect();
        app.schedule.add_async_system(ab5);
        // app.schedule.add_system(ab);
        app.schedule.add_system(cd);
        app.schedule.add_system(ce);
        app.initialize();
        app.run();
        for _ in 0..1000 {
            app.run();
        }
    }

    #[test]
    fn test_res() {
        struct A(f32);
        struct B(f32);
        struct C(f32);
        struct D(f32);
        struct E(f32);

        fn ab(a: SingleRes<A>, mut b: SingleResMut<B>) {
            b.0 += a.0 + 1.0;
        }

        fn cd(c: SingleRes<C>, mut d: SingleResMut<D>) {
            d.0 += c.0 + 1.0;
        }

        fn ce(c: SingleRes<C>, mut e: SingleResMut<E>, mut b: SingleResMut<B>) {
            e.0 += c.0 + 1.0;
            b.0 += c.0 + 1.0;
        }
        let mut app = MultiThreadApp::new();
        app.world.register_single_res(A(0.0));
        app.world.register_single_res(B(0.0));
        app.world.register_single_res(C(0.0));
        app.world.register_single_res(D(0.0));
        app.world.register_single_res(E(0.0));
        app.schedule.add_system(ab);
        app.schedule.add_system(cd);
        app.schedule.add_system(ce);
        app.initialize();
        app.run();
        app.run();
        assert_eq!(app.world.get_single_res::<B>().unwrap().0, 4.0);
        assert_eq!(app.world.get_single_res::<D>().unwrap().0, 2.0);
        assert_eq!(app.world.get_single_res::<E>().unwrap().0, 2.0);
    }

    #[test]
    fn test_multi_res() {
        struct A(f32);
        #[derive(Clone, Copy, Default)]
        struct B(f32);
        #[derive(Clone, Copy, Default)]
        struct C(f32);
        #[derive(Clone, Copy, Default)]
        struct D(f32);
        #[derive(Clone, Copy, Default)]
        struct E(f32);

        fn ab(a: SingleRes<A>, mut b: MultiResMut<B>) {
            b.0 += a.0 + 1.0;
        }

        fn cd(c: MultiRes<C>, mut d: MultiResMut<D>) {
            d.0 += c.iter().next().unwrap().0 + 1.0;
        }

        fn ce(b: MultiRes<B>, mut e: MultiResMut<E>, mut c: MultiResMut<C>) {
            e.0 += b.iter().count() as f32 + 1.0;
            c.0 += b.iter().count() as f32 + 1.0;
        }
        let mut app = MultiThreadApp::new();
        app.world.register_single_res(A(1.0));
        app.world.register_multi_res::<B>();
        app.world.register_multi_res::<C>();
        app.world.register_multi_res::<D>();
        app.world.register_multi_res::<E>();
        app.schedule.add_system(ab);
        app.schedule.add_system(cd);
        app.schedule.add_system(ce);
        app.initialize();
        app.run();
        app.run();
        assert_eq!(app.world.get_multi_res::<B>(0).unwrap().0, 4.0);
        assert_eq!(app.world.get_multi_res::<C>(0).unwrap().0, 4.0);
        assert_eq!(app.world.get_multi_res::<D>(0).unwrap().0, 8.0);
        assert_eq!(app.world.get_multi_res::<E>(0).unwrap().0, 4.0);
    }
}
