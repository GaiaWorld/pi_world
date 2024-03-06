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
    mut q0: Query<(Entity, &mut Age0, &mut Age1,
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
    for (e, mut age0, age1,
        // age2, age3, age4, age5, age6, age7, age8
        ) in q {
        // let a =1+age2.0+age3.0+age4.0+age6.0+age7.0+age8.0;
        age0.0 +=1+age1.0;
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
        println!("e {:?}, r: {:?} is now", e, r);
    }
    println!("alter1: end");
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
    q0: Query<(Entity, &mut Age0, &mut Age1), (Changed<Age0>, Changed<Age2>)>,
) {
    println!("changed_l");
    for (e, age0, _) in q0.iter() {
        println!("e {:?}, age0: {:?}", e, age0);
    }
 
    println!("changed_l: end");
}

pub fn print_e(
    // i0: Insert<(Age2,)>,
    q0: Query<(Entity, &Age0, &Age1,
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


#[cfg(test)]
mod test_mod {
    use crate::{app::*, archetype::Row, system::*, table::Table};
    use pi_append_vec::AppendVec;
    use test::Bencher;
    use super::*;
    use pi_async_rt::prelude::{SingleTaskPool, SingleTaskRunner};
    

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
        removes.clear(1);
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
        let pool = SingleTaskPool::default();
        let rt = SingleTaskRunner::<(), SingleTaskPool<()>>::new(pool).into_local();
        let mut app = App::new();
        let world = app.get_world();
        let i = world.make_inserter::<(Age1,Age0,)>();
        let e1 = i.insert((Age1(1),Age0(0),));
        let e2 = i.insert((Age1(1),Age0(0),));
        let s = Box::new(IntoSystem::into_system(print_changed_entities));
        app.register(s, &[]);
        app.initialize();
        app.run(&rt);
        app.run(&rt);
        assert_eq!(app.get_world().get_component::<Age0>(e1).unwrap().0, 4);
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
        let pool = SingleTaskPool::default();
        let rt = SingleTaskRunner::<(), SingleTaskPool<()>>::new(pool).into_local();
        let mut app = App::new();
        let s = Box::new(IntoSystem::into_system(insert1));
        app.register(s, &[]);
        let s = Box::new(IntoSystem::into_system(print_changed_entities));
        app.register(s, &[]);
        app.initialize();
        app.run(&rt);
        app.run(&rt);
        // assert_eq!(app.get_world().get_component::<Age0>(e1).unwrap().0, 0);
    }
    #[test]
    fn test_alter() {
        let pool = SingleTaskPool::default();
        let rt = SingleTaskRunner::<(), SingleTaskPool<()>>::new(pool).into_local();
        let mut app = App::new();
        let s = Box::new(IntoSystem::into_system(insert1));
        app.register(s, &[]);
        let s = Box::new(IntoSystem::into_system(print_changed_entities));
        app.register(s, &[]);
        let s = Box::new(IntoSystem::into_system(alter1));
        app.register(s, &[]);
        let s = Box::new(IntoSystem::into_system(print_e));
        app.register(s, &[]);
        app.initialize();
        app.run(&rt);
        app.run(&rt);
        app.run(&rt);
    }
    #[test] 
    fn test_added() {
        let pool = SingleTaskPool::default();
        let rt = SingleTaskRunner::<(), SingleTaskPool<()>>::new(pool).into_local();
        let mut app = App::new();
        let s = Box::new(IntoSystem::into_system(insert1));
        app.register(s, &[]);
        let s = Box::new(IntoSystem::into_system(print_changed_entities));
        app.register(s, &[]);
        let s = Box::new(IntoSystem::into_system(added_l));
        app.register(s, &[]);
        let s = Box::new(IntoSystem::into_system(alter1));
        app.register(s, &["add"]);
        app.initialize();
        app.run_stage("add", &rt);
        app.run_stage("add", &rt);
    }
    #[test]
    fn test_changed() {
        let pool = SingleTaskPool::default();
        let rt = SingleTaskRunner::<(), SingleTaskPool<()>>::new(pool).into_local();
        let mut app = App::new();
        let s = Box::new(IntoSystem::into_system(insert1));
        app.register(s, &[]);
        let s = Box::new(IntoSystem::into_system(print_changed_entities));
        app.register(s, &[]);
        let s = Box::new(IntoSystem::into_system(alter1));
        app.register(s, &[]);
        let s = Box::new(IntoSystem::into_system(changed_l));
        app.register(s, &[]);
        app.initialize();
        app.run(&rt);
        app.run(&rt);
    }
    #[bench]
    fn bench_test(b: &mut Bencher) {
        let pool = SingleTaskPool::default();
        let rt = SingleTaskRunner::<(), SingleTaskPool<()>>::new(pool).into_local();
        let mut app = App::new();
        let world = app.get_world();
        println!("bench_test insert");
        let i = world.make_inserter::<(Age0,Age1,Age2,Age3,Age4,Age5,Age6,Age7,Age8,Age9,Age10,Age11,Age12,Age13,Age14)>();
        println!("bench_test insert");
        for _ in 0..90 {
            i.insert((Age0(0),Age1(0),Age2(0),Age3(0),Age4(0),Age5([0;16]),Age6(0),Age7(0),Age8(0),Age9(0),Age10(0),Age11(0),Age12(0),Age13(0),Age14(0)));
        }
        println!("bench_test insert ok");
        for _ in 0..500 {
            let s = Box::new(IntoSystem::into_system(print_changed_entities));
            app.register(s, &[]);
        }
        b.iter(move || {
            let rt1 = rt.clone();
            app.run(&rt1);
        });
    }

}

