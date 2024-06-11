#![allow(warnings)]
use crate::prelude::*;

#[derive(Copy, Clone, Debug, Eq, Default,PartialEq, Component)]
pub struct Age0(usize);

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Component)]
pub struct Age1(usize);

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Component)]
pub struct Age2(usize);

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Component)]
pub struct Age3(usize);

#[derive(Component)]
pub struct Age4(usize);
#[derive(Component)]
pub struct Age5([usize; 16]);
#[derive(Component)]
pub struct Age6(usize);
#[derive(Component)]
pub struct Age7(usize);
#[derive(Component)]
pub struct Age8(usize); 
#[derive(Component, Debug, Default)]
pub struct Age9(usize);
#[derive(Component, Debug, Default)]
pub struct Age10(usize);
#[derive(Component)]
pub struct Age11(usize);
#[derive(Component)]
pub struct Age12(usize);
#[derive(Component)]
pub struct Age13(usize);
#[derive(Component)]
pub struct Age14(usize);
#[derive(Component)]
pub struct Age15(usize);
#[derive(Component)]
pub struct Age16(usize);
#[derive(Component)]
pub struct Age17(usize);
#[derive(Component)]
pub struct Age18(usize);
#[derive(Component)]
pub struct Age19(usize);
#[derive(Component)]
pub struct Age20(usize);

#[derive(Component, Debug)]
pub struct Age21(Vec<u64>);

impl Drop for Age21 {
    fn drop(&mut self) {
        println!("Age21 drop, {:p} {}", self, self.0.len());
    }
}
// #[derive(Bundle)]
// pub struct Bundle1{
//     a1: Age1,
//     a2: Age2,
// }

pub fn print_info(
    q: Query<(
        Entity,
        ArchetypeName,
    )>,
) {
    println!("print_info it:{:?}", q.iter().size_hint());
    let q = q.iter();
    for (e, a) in q
    {
        println!(" e:{:?}, a:{:?}", e, a);
    }
    println!("print_info over");
}

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
        ArchetypeName,
        // &Age2, &Age3, &Age4, &Age5, &Age6, &Age7, &Age8
    )>,
    // q1: Query<(Entity, &mut Age1)>,
    //q2: Query<(Entity, Option<&mut Age2>)>,
    // q3: Query<(Entity, &mut Age3)>,
) {
    println!("print_changed_entities it:{:?}", q0.iter().size_hint());
    // let q = q0.iter();
    // let s = q.size_hint();
    let q = q0.iter_mut();
    for (
        e,
        mut age0,
        age1,
        aname,
        // age2, age3, age4, age5, age6, age7, age8
    ) in q
    {
        // let a =1+age2.0+age3.0+age4.0+age6.0+age7.0+age8.0;
        age0.0 += 1 + age1.0;
        println!("print_changed_entities {:?}", (e, aname));
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
    mut i0: Alter<(), (), (Age3,), (Age4,)>,
    q0: Query<(Entity, &mut Age0, &mut Age1)>,
) {
    println!("alter1");
    for (e, _, _) in q0.iter() {
        let r = i0.alter(e, (Age3(2),));
        dbg!(e, r);
    }
    println!("alter1: end");
}
pub fn added_l(q0: Query<(Entity, &mut Age1, &mut Age0), (Changed<Age1>, Changed<Age2>)>) {
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
    for (age0, age1) in set.p0().iter_mut() {
        // dbg!(age0, age1);
    }
    println!("p_set1");
    set.p0().iter_mut().for_each(|(age1, age2)| {
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, Component, Default)]
struct A(u32);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Component, Default)]
struct B(u32);

#[derive(Copy, Clone, Debug, Component)]
struct Transform([f32; 16]);

#[derive(Copy, Clone, Debug, Component)]
struct Position([f32; 3]);

#[derive(Copy, Clone, Debug, Component)]
struct Rotation([f32; 3]);

#[derive(Copy, Clone, Debug, Component)]
struct Velocity([f32; 3]);
 
#[cfg(test)]
mod test_mod {
  
    use std::{any::TypeId, ops::Deref};

    use super::*;
    use crate::{
        // app::*,
        archetype::{Archetype, ComponentInfo, Row}, column::{BlobTicks, Column}, debug::{ArchetypeDebug, ColumnDebug}, editor::EntityEditor, schedule::Update, schedule_config::IntoSystemConfigs, system::{relate, Relation, SystemMeta, TypeInfo}, table::Table
    };
    use fixedbitset::FixedBitSet;
    // use bevy_utils::dbg;
    use pi_append_vec::AppendVec;
    // use pi_async_rt::rt::single_thread::SingleTaskRuntime;
    use pi_null::Null;
    use pi_share::Share;
    use rand::Rng;
    use test::Bencher;

    #[derive(ScheduleLabel, Hash, Eq, PartialEq, Clone, Debug)]
    pub struct AddSchedule;
 
    #[test] 
    fn test_columns() {
        let mut cc = Column::new(ComponentInfo::of::<Transform>(0));
        cc.init_blob(0usize.into());
        let mut c = cc.blob_ref_unchecked(0usize.into());
        c.write(Row(0), Entity::null(), Transform([0.0; 16]));
        c.write(Row(1), Entity::null(), Transform([1.0; 16]));
        dbg!(c.get::<Transform>(Row(0), Entity::null()));
        dbg!(c.get::<Transform>(Row(1), Entity::null()));
        let mut action = Default::default();
        cc.settle_by_index(0usize.into(), 2, 0, &mut action);
        let mut c = cc.blob_ref_unchecked(0usize.into());
        dbg!(c.get::<Transform>(Row(0), Entity::null()));
        dbg!(c.get::<Transform>(Row(1), Entity::null()));
    }
    #[test]
    fn test_removes_action() {
        let mut action = Default::default();
        let mut set: FixedBitSet = Default::default();
        let mut removes: AppendVec<Row> = Default::default();
        
        let mut rng = rand::thread_rng();
        let size = 20;
        set.grow(size);
        let range = rng.gen_range(0..size);
        for _ in 0..range {
            let x= rng.gen_range(0..size);
            if set.contains(x) {
                continue;
            }
            set.set(x, true);
            removes.insert(x.into());
        }
        let asset_len = size - removes.len();
        let len = Table::removes_action(&removes, removes.len(), size, &mut action, &mut set);
        assert_eq!(len, asset_len, "{:?}", action);
        println!("action: {:?}", action)
    }
    #[test]
    fn test_system_meta() {
        let mut meta: SystemMeta = SystemMeta::new(TypeInfo::of::<SystemMeta>());
        meta.relate(Relation::Read(1usize.into()));
        meta.relate(Relation::Write(2usize.into()));
        meta.related_ok();
        meta.check_conflict(); // 检查自身
        let mut meta: SystemMeta = SystemMeta::new(TypeInfo::of::<SystemMeta>());
        meta.relate(Relation::Read(1usize.into()));
        meta.relate(Relation::Write(2usize.into()));
        meta.related_ok();
        meta.relate(Relation::Read(1usize.into()));
        meta.relate(Relation::Write(3usize.into()));
        meta.related_ok();
        meta.check_conflict(); // 检查读写
        let mut meta: SystemMeta = SystemMeta::new(TypeInfo::of::<SystemMeta>());
        meta.relate(Relation::Read(1usize.into()));
        meta.relate(Relation::Write(2usize.into()));
        meta.relate(Relation::Without(3usize.into()));
        meta.related_ok();
        meta.relate(Relation::Read(1usize.into()));
        meta.relate(Relation::Write(2usize.into()));
        meta.relate(Relation::With(3usize.into()));
        meta.related_ok();
        meta.check_conflict(); // 检查without读写
        let mut meta: SystemMeta = SystemMeta::new(TypeInfo::of::<SystemMeta>());
        meta.param_set_start();
        meta.relate(Relation::Read(1usize.into()));
        meta.relate(Relation::Write(2usize.into()));
        meta.relate(Relation::Without(3usize.into()));
        meta.related_ok();
        meta.relate(Relation::Write(1usize.into()));
        meta.relate(Relation::Write(2usize.into()));
        meta.relate(Relation::Without(3usize.into()));
        meta.related_ok();
        meta.param_set_end();
        meta.relate(Relation::Read(1usize.into()));
        meta.relate(Relation::Write(2usize.into()));
        meta.relate(Relation::With(3usize.into()));
        meta.related_ok();
        meta.check_conflict(); // 检查ParamSet读写
        let mut meta: SystemMeta = SystemMeta::new(TypeInfo::of::<SystemMeta>());
        meta.relate(Relation::Read(1usize.into()));
        meta.relate(Relation::Write(2usize.into()));
        meta.relate(Relation::Without(3usize.into()));
        meta.related_ok();
        meta.relate(Relation::WriteAll);
        meta.related_ok();
    }
    #[test]
    fn test_system_meta2() {
        let mut app = crate::prelude::App::new();
        let w = &mut app.world;
        let info = w.archetype_info(vec![ComponentInfo::of::<Transform>(0), ComponentInfo::of::<Position>(0), ComponentInfo::of::<Velocity>(0), ComponentInfo::of::<Rotation>(0)]);
        let ar = Archetype::new(info);

        // 测试空Fetch
        let mut meta: SystemMeta = SystemMeta::new(TypeInfo::of::<SystemMeta>());
        let r = meta.related_ok();
        assert_eq!(true, relate(&r, &ar, 0));
        assert_eq!(true, relate(&r, &w.empty_archetype, 0));

        let mut meta: SystemMeta = SystemMeta::new(TypeInfo::of::<SystemMeta>());
        meta.relate(Relation::Read(w.get_component_index(&TypeId::of::<Transform>())));
        meta.relate(Relation::Write(w.get_component_index(&TypeId::of::<Position>())));
        let r = meta.related_ok();
        assert_eq!(true, relate(&r, &ar, 0));
        let mut meta: SystemMeta = SystemMeta::new(TypeInfo::of::<SystemMeta>());
        meta.relate(Relation::Read(w.get_component_index(&TypeId::of::<Transform>())));
        meta.relate(Relation::Read(w.get_component_index(&TypeId::of::<A>())));
        let r = meta.related_ok();
        assert_eq!(false, relate(&r, &ar, 0));

        let mut meta: SystemMeta = SystemMeta::new(TypeInfo::of::<SystemMeta>());
        meta.relate(Relation::Read(w.get_component_index(&TypeId::of::<Transform>())));
        meta.relate(Relation::Without(w.get_component_index(&TypeId::of::<Velocity>())));
        let r = meta.related_ok();
        assert_eq!(false, relate(&r, &ar, 0));

        let mut meta: SystemMeta = SystemMeta::new(TypeInfo::of::<SystemMeta>());
        meta.relate(Relation::Read(w.get_component_index(&TypeId::of::<Transform>())));
        meta.relate(Relation::Or);
        meta.relate(Relation::With(w.get_component_index(&TypeId::of::<Velocity>())));
        meta.relate(Relation::With(w.get_component_index(&TypeId::of::<A>())));
        meta.relate(Relation::End);
        let r = meta.related_ok();
        assert_eq!(true, relate(&r, &ar, 0));
        assert_eq!(false, relate(&r, &w.empty_archetype, 0));

    }
 
    #[test]
    fn test_removes() {
        let mut action = Default::default();
        let mut set = Default::default();
        let mut removes: AppendVec<Row> = Default::default();
        removes.insert(Row(1));
        removes.insert(Row(2));
        //removes.insert(0);
        let len = Table::removes_action(&removes, removes.len(), 7, &mut action, &mut set);
        assert_eq!(len, 5);
        assert_eq!(action.len(), 2);
        assert_eq!(action[0], (Row(6), Row(1)));
        assert_eq!(action[1], (Row(5), Row(2)));
        removes.clear(0);
        removes.insert(Row(1));
        removes.insert(Row(6));
        //removes.insert(0);
        let len = Table::removes_action(&removes, removes.len(), 7, &mut action, &mut set);
        assert_eq!(len, 5);
        assert_eq!(action.len(), 1);
        assert_eq!(action[0], (Row(5), Row(1)));
    }
    #[test]
    fn test() { 
        let mut app = crate::prelude::App::new();
        dbg!("data0");
        let i = app.world.make_inserter::<(Age1, Age0)>();
        println!("data1");
        let e1 = i.insert((Age1(1), Age0(0)));
        println!("data2");
        let e2 = i.insert((Age1(1), Age0(0)));
        println!("data3");
        app.add_system(Update, print_changed_entities);
        println!("data");
        app.run();
        
        let mut info = ArchetypeDebug {
            entitys: Some(2),
            columns_info: vec![
                Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
                Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
            ],
            destroys_listeners: Some(0),
            removes: Some(0),
        };

        app.world.assert_archetype_arr(&[None, Some(info.clone())]);

        app.run();
        
        app.world.assert_archetype_arr(&[None, Some(info.clone())]);
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
        let mut app = crate::prelude::App::new();
        app.add_system(Update, insert1);
        app.add_system(Update, print_changed_entities);
        
        app.run();


        let mut info = ArchetypeDebug {
            entitys: Some(1),
            columns_info: vec![
                Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
                Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
            ],
            destroys_listeners: Some(0),
            removes: Some(0),
        };

        app.world.assert_archetype_arr(&[None, Some(info.clone())]);

        app.run();
    }
    #[test]
    fn test_add_remove() {
        let mut world = World::new();
        let i = world.make_inserter::<(A,)>();
        let entities = (0..10).map(|_| i.insert((A(0),))).collect::<Vec<_>>();
        world.settle();
        let index= world.init_component::<B>();
        {
            let mut editor = world.make_entity_editor();
            
            // let mut it = 
            for entity in &entities {
                editor.add_components_by_index(*entity, &[index]);
            }
        }
        for e in &entities {
            assert_eq!(world.get_component::<B>(*e).is_ok(), true, "{:?}", world.get_component::<B>(*e))
        }
        {
            let mut editor = world.make_entity_editor();
            for entity in &entities {
                let r = editor.remove_components_by_index(*entity, &[index]);
                assert_eq!(r.is_ok(), true, "{:?}", r);
            }
        }
        for e in entities {
            assert_eq!(world.get_component::<B>(e).is_err(), true)
        }

        let mut info = ArchetypeDebug {
            entitys: Some(20),
            columns_info: vec![
                Some(ColumnDebug{change_listeners: 0, name: Some("A")}),
            ],
            destroys_listeners: Some(0),
            removes: Some(10),
        };

        let mut info1 = ArchetypeDebug {
            entitys: Some(10),
            columns_info: vec![
                Some(ColumnDebug{change_listeners: 0, name: Some("A")}),
                Some(ColumnDebug{change_listeners: 0, name: Some("B")}),
            ],
            destroys_listeners: Some(0),
            removes: Some(10),
        };

        world.assert_archetype_arr(&[None, Some(info.clone()), Some(info1)]);
    }
    #[bench]
    fn bench_heavy_compute(b: &mut Bencher) {
        use cgmath::*;

        #[derive(Copy, Clone, Component)]
        struct Mat(Matrix4<f32>);

        #[derive(Copy, Clone, Component)]
        struct Position(Vector3<f32>);

        #[derive(Copy, Clone, Component)]
        struct Rotation(Vector3<f32>);

        #[derive(Copy, Clone, Component)]
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
        world.settle();
        let query = world.make_queryer::<(&mut Position, &mut Mat), ()>();
        println!("query, {:?}", query.iter().size_hint());
        b.iter( || {
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
            let i = world.make_inserter::<(Transform,Position,Rotation, Velocity)>();
            for a in 0..9990 {
                i.insert((
                    Transform([a as f32; 16]),
                    Position([a as f32; 3]),
                    Rotation([a as f32; 3]),
                    Velocity([a as f32; 3]),
                ));
            };
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

            let mut info = ArchetypeDebug {
                entitys: Some(10_000),
                columns_info: vec![
                    Some(ColumnDebug{change_listeners: 0, name: Some("Transform")}),
                    Some(ColumnDebug{change_listeners: 0, name: Some("Position")}),
                    Some(ColumnDebug{change_listeners: 0, name: Some("Rotation")}),
                    Some(ColumnDebug{change_listeners: 0, name: Some("Velocity")}),
                ],
                destroys_listeners: Some(0),
                removes: Some(0),
            };
    
            world.assert_archetype_arr(&[None, Some(info.clone())]);
        }
    }

    #[test] 
    fn test_query() {
        let mut world = World::new();
        let mut w = world.unsafe_world();
        let mut w1 = world.unsafe_world();
        let i = w.make_inserter::<(Age1, Age0)>();
        let _i1 = w1.make_inserter::<(Age2, Age3)>();
        let e1 = i.insert((Age1(1), Age0(0)));
        let e2 = i.insert((Age1(1), Age0(0)));
        world.settle();
        let mut q = world.make_queryer::<(&Age1, &mut Age0), ()>();
        for (a, mut b) in q.iter_mut() {
            b.0 += a.0;
        }

        let mut info = ArchetypeDebug {
            entitys: Some(2),
            columns_info: vec![
                Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
                Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
            ],
            destroys_listeners: Some(0),
            removes: Some(0),
        };

        let mut info1 = ArchetypeDebug {
            entitys: Some(0),
            columns_info: vec![
                Some(ColumnDebug{change_listeners: 0, name: Some("Age2")}),
                Some(ColumnDebug{change_listeners: 0, name: Some("Age3")}),
            ],
            destroys_listeners: Some(0),
            removes: Some(0),
        };

        world.assert_archetype_arr(&[None, Some(info.clone()), Some(info1)]);

        assert_eq!(world.get_component::<Age0>(e1).unwrap().0, 1);
        assert_eq!(world.get_component::<Age0>(e2).unwrap().0, 1);
    }

    #[test]
    fn test_alter() {
        let mut app = crate::prelude::App::new();
        app.add_system(Update, insert1);
        app.add_system(Update, print_changed_entities);
        app.add_system(Update, alter1);
        // app.add_system(Update, p_set);
        
        app.run();

        let mut info = ArchetypeDebug {
            entitys: Some(0),
            columns_info: vec![
                Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
                Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
            ],
            destroys_listeners: Some(0),
            removes: Some(0),
        };

        let mut info1 = ArchetypeDebug {
            entitys: Some(1),
            columns_info: vec![
                Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
                Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
                Some(ColumnDebug{change_listeners: 0, name: Some("Age3")}),
            ],
            destroys_listeners: Some(0),
            removes: Some(0),
        };

        app.world.assert_archetype_arr(&[None, Some(info.clone()), Some(info1)]);

        app.run();
        app.run();
    }
    #[test]
    fn test_alter1() {
        let mut world = World::new();
        let i = world.make_inserter::<(Age1, Age0)>();
        let e1 = i.insert((Age1(2), Age0(1)));
        let e2 = i.insert((Age1(4), Age0(2)));
        world.settle();
        {
            let mut editor = world.make_entity_editor();
            let index = editor.init_component::<Age2>();

            // editor.add_components_by_index(e1, &[index]);
            editor.add_components_by_index(e2, &[index]);
        }
        world.settle();

        let mut info = ArchetypeDebug {
            entitys: Some(1),
            columns_info: vec![
                Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
                Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
            ],
            destroys_listeners: Some(0),
            removes: Some(0),
        };

        let mut info1 = ArchetypeDebug {
            entitys: Some(1),
            columns_info: vec![
                Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
                Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
                Some(ColumnDebug{change_listeners: 0, name: Some("Age2")}),
            ],
            destroys_listeners: Some(0),
            removes: Some(0),
        };

        world.assert_archetype_arr(&[None, Some(info.clone()), Some(info1)]);

        assert_eq!(world.get_component::<Age0>(e1).unwrap().0, 1);
        assert_eq!(world.get_component::<Age2>(e1).is_err(), true);
        assert_eq!(world.get_component::<Age0>(e2).unwrap().0, 2);
        assert_eq!(world.get_component::<Age1>(e2).unwrap().0, 4);
        assert_eq!(world.get_component::<Age2>(e2).unwrap().0, 0);
    }
    #[test]
    fn test_alter2() {
        println!("0");
        let mut world = World::new();
        let i = world.make_inserter::<(Age0,)>();
        let _entities = (0..1)
            .map(|_| i.insert((Age0(0),)))
            .collect::<Vec<Entity>>();
        world.settle();
        let mut editor = world.make_entity_editor();
        let index = editor.init_component::<Age1>();
        {
            println!("1");
            
            for e in &_entities{
                    editor.add_components_by_index(*e, &[index]);
            }
            println!("2");
        }
        {
            for e in &_entities {
                editor.remove_components_by_index(*e, &[index]);
            }
        }

        let mut info = ArchetypeDebug {
            entitys: Some(2),
            columns_info: vec![
                Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
            ],
            destroys_listeners: Some(0),
            removes: Some(1),
        };

        world.assert_archetype_arr(&[None, Some(info.clone()), None]);

        info.entitys = Some(1);
        info.columns_info = vec![
            Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
            Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
        ];
        world.assert_archetype_arr(&[None, None, Some(info.clone()), ]);
    }

    #[test]
    fn test_added() { 
        let mut app = crate::prelude::App::new();
        app.add_system(Update, insert1);
        app.add_system(Update, print_changed_entities);
        app.add_system(Update, added_l);
        app.add_system(Update, alter1.in_schedule(AddSchedule));
        
        app.run_schedule(AddSchedule);

        let mut info = ArchetypeDebug {
            entitys: Some(0),
            columns_info: vec![
                Some(ColumnDebug{change_listeners: 1, name: Some("Age1")}),
                Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
            ],
            destroys_listeners: Some(0),
            removes: Some(0),
        };

        app.world.assert_archetype_arr(&[None, Some(info.clone())]);

        app.run_schedule(AddSchedule);

        app.world.assert_archetype_arr(&[None, Some(info.clone())]);
    }
    #[test]
    fn test_changed() { 
        let mut app = crate::prelude::App::new();
        app.add_system(Update, insert1);
        app.add_system(Update, print_changed_entities);
        app.add_system(Update, alter1);
        app.add_system(Update, changed_l);
        
        app.world.assert_archetype_arr(&[None]);

        app.run();

        let mut info = ArchetypeDebug {
            entitys: Some(0),
            columns_info: vec![
                Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
                Some(ColumnDebug{change_listeners: 1, name: Some("Age0")}),
            ],
            destroys_listeners: Some(0),
            removes: Some(0),
        };

        app.world.assert_archetype_arr(&[None, Some(info.clone()), None]);
        info.entitys = Some(1);
        info.columns_info = vec![
            Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
            Some(ColumnDebug{change_listeners: 1, name: Some("Age0")}),
            Some(ColumnDebug{change_listeners: 0, name: Some("Age3")}),
        ];
        info.removes = Some(0);
        app.world.assert_archetype_arr(&[None, None, Some(info.clone()),]);
        app.run();
    }

    #[test]
    fn test_removed() { 
        pub fn insert(i0: Insert<(Age3, Age1, Age0)>) {
            println!("insert1 is now");
            let e = i0.insert((Age3(3), Age1(1), Age0(0)));
            println!("insert1 is end, e:{:?}", e);
        }
        pub fn alter(
            mut i0: Alter<&Age1, (), (), (Age3,)>,
            q0: Query<(Entity, &mut Age0, &Age1), ()>,
        ) {
            println!("alter1 it:{:?}", q0.iter().size_hint());
            for (e, _, _) in q0.iter() {
                let r = i0.alter(e, ());
                println!("alter1 it1111:{:?}", r);
            }
            println!("alter1: end");
        }
        pub fn removed_l(mut q0: Query<(&mut Age0, &mut Age1)>, removed: ComponentRemoved<Age3>) {
            println!("removed_l");
            for e in removed.iter() {
                println!("e:{:?}, q0: {:?}", e, q0.get_mut(*e));
            }
            println!("removed_l: end");
        }
        let mut app = crate::prelude::App::new();
        app.add_system(Update, insert);
        // app.add_system(Update, print_changed_entities);
        app.add_system(Update, alter);
        app.add_system(Update, removed_l);
        app.add_system(Update, print_info);
        
        app.world.assert_archetype_arr(&[None]);

        app.run();

        let mut info = ArchetypeDebug {
            entitys: Some(0),
            columns_info: vec![
                Some(ColumnDebug{change_listeners: 0, name: Some("Age3")}), 
                Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
                Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
            ],
            destroys_listeners: Some(0),
            removes: Some(0),
        };
        app.world.assert_archetype_arr(&[None, Some(info.clone()), None]);

        app.run();

        let mut info1 = ArchetypeDebug {
            entitys: Some(2),
            columns_info: vec![ 
                Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}),
                Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
            ],
            destroys_listeners: Some(0),
            removes: Some(0),
        };

        app.world.assert_archetype_arr(&[None, Some(info.clone()), Some(info1)]);
    }

    #[test]
    fn test_schedule() {
        #[derive(Component)]
        struct A(f32);
        #[derive(Component)]
        struct B(f32);
        #[derive(Component)]
        struct C(f32);
        #[derive(Component)]
        struct D(f32);
        #[derive(Component)]
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
        let mut app = crate::prelude::App::new();
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

        app.world.settle();
        app.add_system(Update, ab);
        app.add_system(Update, cd);
        app.add_system(Update, ce);
        
        let mut info = ArchetypeDebug {
            entitys: Some(10000),
            columns_info: vec![
                Some(ColumnDebug{change_listeners: 0, name: Some("A")}), 
                Some(ColumnDebug{change_listeners: 0, name: Some("B")}), 
            ],
            destroys_listeners: Some(0),
            removes: Some(0),
        };
        app.world.assert_archetype_arr(&[None, Some(info.clone()), None, None, None,]);

        app.run();

        info.columns_info = vec![
            Some(ColumnDebug{change_listeners: 0, name: Some("A")}), 
            Some(ColumnDebug{change_listeners: 0, name: Some("B")}), 
            Some(ColumnDebug{change_listeners: 0, name: Some("C")}), 
        ];

        app.world.assert_archetype_arr(&[None, None, Some(info.clone()), None, None,]);

        for _ in 0..1000 {
            app.run();
        }
    }

    // #[test]
    // fn test_async_schedule() {
    //     #[derive(Component)]
    //     struct A(f32);
    //     #[derive(Component)]
    //     struct B(f32);
    //     #[derive(Component)]
    //     struct C(f32);
    //     #[derive(Component)]
    //     struct D(f32);
    //     #[derive(Component)]
    //     struct E(f32);

    //     fn ab(
    //         mut local: Local<usize>,
    //         mut query: Query<(&mut A, &mut B)>,
    //     ) {
    //         for (mut a, mut b) in query.iter_mut() {
    //             std::mem::swap(&mut a.0, &mut b.0);
    //         }
    //         *local += 1;
    //     }
    //     async fn ab1<'w>(
    //         mut local: Local<'w, usize>,
    //         mut query: Query<'w, (&mut A, &mut B)>,
    //     ) {
    //         for (mut a, mut b) in query.iter_mut() {
    //             std::mem::swap(&mut a.0, &mut b.0);
    //         }
    //         *local += 1;
    //     }
    //     async fn ab5(
    //         mut local: Local<'static, usize>,
    //         mut query: Query<'static, (&mut A, &mut B)>,
    //     ) {
    //         for (mut a, mut b) in query.iter_mut() {
    //             std::mem::swap(&mut a.0, &mut b.0);
    //         }
    //         *local += 1;
    //     }
    //     fn cd(mut query: Query<(&mut C, &mut D)>) {
    //         for (mut c, mut d) in query.iter_mut() {
    //             std::mem::swap(&mut c.0, &mut d.0);
    //         }
    //     }

    //     fn ce(mut query: Query<(&mut C, &mut E)>) {
    //         for (mut c, mut e) in query.iter_mut() {
    //             std::mem::swap(&mut c.0, &mut e.0);
    //         }
    //     }

        
    //     let mut app = MultiThreadApp::new();
    //     let i = app.world.make_inserter::<(A, B)>();
    //     let it = (0..10_000).map(|_| (A(0.0), B(0.0)));
    //     i.batch(it);

    //     let i = app.world.make_inserter::<(A, B, C)>();
    //     let it = (0..10_000).map(|_| (A(0.0), B(0.0), C(0.0)));
    //     i.batch(it);

    //     let i = app.world.make_inserter::<(A, B, C, D)>();
    //     let it = (0..10_000).map(|_| (A(0.0), B(0.0), C(0.0), D(0.0)));
    //     i.batch(it);

    //     let i = app.world.make_inserter::<(A, B, C, E)>();
    //     let it = (0..10_000).map(|_| (A(0.0), B(0.0), C(0.0), E(0.0)));
    //     i.batch(it);

    //     app.world.settle();
    //     // app.schedule.add_async_system(ab5);
    //     // app.add_system(Update, ab);
    //     // app.add_system(Update, cd);
    //     // app.add_system(Update, ce);
        
    //     let mut info = ArchetypeDebug {
    //         entitys: Some(10000),
    //         columns_info: vec![
    //             Some(ColumnDebug{change_listeners: 0, name: Some("A")}), 
    //             Some(ColumnDebug{change_listeners: 0, name: Some("B")}), 
    //         ],
    //         destroys_listeners: Some(0),
    //         removes: Some(0),
    //     };
    //     app.world.assert_archetype_arr(&[None, Some(info.clone()), None, None, None,]);

    //     app.run();

    //     info.columns_info = vec![
    //         Some(ColumnDebug{change_listeners: 0, name: Some("A")}), 
    //         Some(ColumnDebug{change_listeners: 0, name: Some("B")}), 
    //         Some(ColumnDebug{change_listeners: 0, name: Some("C")}), 
    //     ];

    //     app.world.assert_archetype_arr(&[None, None, Some(info.clone()), None, None,]);

    //     for _ in 0..1000 {
    //         app.run();
    //     }
    // }

    #[test]
    fn test_res() { 
        struct A(f32);
        struct B(f32);
        struct C(f32);
        struct D(f32);
        struct E(f32);

        fn ab(a: SingleRes<A>, mut b: SingleResMut<B>) {
            println!("ab:{:?}", b.0);
            b.0 += a.0 + 1.0;
            println!("ab:{:?}", b.0);
        }

        fn cd(c: SingleRes<C>, mut d: SingleResMut<D>) {
            d.0 += c.0 + 1.0;
        }

        fn ce(w: &World, c: SingleRes<C>, mut e: SingleResMut<E>, mut b: SingleResMut<B>) {
            e.0 += c.0 + 1.0;
            b.0 += c.0 + 1.0;
            println!("ce:{:?}", b.0);
        }
        
        let mut app = crate::prelude::App::new();
        app.world.insert_single_res(A(0.0));
        app.world.insert_single_res(B(0.0));
        app.world.insert_single_res(C(0.0));
        app.world.insert_single_res(D(0.0));
        app.world.insert_single_res(E(0.0));
        app.add_system(Update, ab);
        app.add_system(Update, cd);
        app.add_system(Update, ce);
        
        app.world.assert_archetype_arr(&[None]);

        app.run();

        app.world.assert_archetype_arr(&[None]);

        app.run();

        app.world.assert_archetype_arr(&[None]);

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
        let mut app = crate::prelude::App::new();
        app.world.insert_single_res(A(1.0));
        app.add_system(Update, ab);
        app.add_system(Update, cd);
        app.add_system(Update, ce);
        
        app.world.assert_archetype_arr(&[None]);

        app.run();

        app.world.assert_archetype_arr(&[None]);

        app.run();

        app.world.assert_archetype_arr(&[None]);

        let r = app.world.get_multi_res::<B>().unwrap().0.get(0).unwrap().deref().0;
        assert_eq!(r, 4.0);
        let rs = app.world.get_multi_res::<C>().unwrap().0.get(0).unwrap().deref().0;
        assert_eq!(r, 4.0);
        let rs = app.world.get_multi_res::<D>().unwrap().0.get(0).unwrap().deref().0;
        assert_eq!(r, 4.0);
        let rs = app.world.get_multi_res::<E>().unwrap().0.get(0).unwrap().deref().0;
        assert_eq!(r, 4.0);
    }

    #[test]
    fn test_event() { 
        struct A(f32);
        #[derive(Clone, Copy, Default)]
        struct B(f32);

        fn ab(a: SingleRes<A>, mut b: EventSender<B>) {
            b.send(B(a.0 + 1.0));
        }

        fn cd(b: Event<B>) {
            println!("cd start");
            for i in b.iter() {
                assert_eq!(i.0, 2.0);
            }
            println!("cd end");
        }

        let mut app = crate::prelude::App::new();
        app.world.insert_single_res(A(1.0));
        app.add_system(Update, ab);
        app.add_system(Update, cd);

        app.run();
        app.run();
    }
    #[test]
    fn test_ticker() {
        pub fn insert(i0: Insert<(Age3, Age1, Age0)>) {
            println!("insert1 is now");
            let e = i0.insert((Age3(3), Age1(1), Age0(0)));
            println!("insert1 is end, e:{:?}", e);
        }
        pub fn print_changed_entities(
            mut q0: Query<(
                Entity,
                Ticker<&mut Age0>,
                Ticker<&mut Age1>,
            )>,
        ) {
            println!("print_changed_entities it:{:?}", q0.iter().size_hint());
            let q = q0.iter_mut();
            for (e, mut age0, age1,) in q {
                age0.0 += 1 + age1.0;
                println!("print_changed_entities {:?}", e);
            }
            println!("print_changed_entities over");
        }
        pub fn print_changed2(
            q0: Query<(
                Ticker<&Age0>,
                Ticker<&Age1>,
            )>,
        ) {
            println!("print_changed2 tick:{:?}, last_run:{:?} it:{:?}", q0.tick(), q0.last_run(), q0.iter().size_hint());
            let q = q0.iter();
            for (
                age0,
                age1,
            ) in q
            {
                println!("tick: {:?}, {:?}", age0.tick(), age0.last_tick());
                assert!(age0.is_changed());
            }
            println!("print_changed2 over");
        }
 
        let mut app = crate::prelude::App::new();
        app.add_system(Update, insert);
        app.add_system(Update, print_changed_entities);
        app.add_system(Update, print_changed2);

        app.world.assert_archetype_arr(&[None]);

        app.run();

        let mut info = ArchetypeDebug {
            entitys: Some(1),
            columns_info: vec![
                Some(ColumnDebug{change_listeners: 0, name: Some("Age3")}), 
                Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}), 
                Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}), 
            ],
            destroys_listeners: Some(0),
            removes: Some(0),
        };
        app.world.assert_archetype_arr(&[None, Some(info.clone())]);

        app.run();

        info.entitys = Some(2);

        app.world.assert_archetype_arr(&[None, Some(info)]);
    }

    // #[test]
    // fn test_destroyed() { 
    //     pub fn insert(i0: Insert<(Age3, Age1, Age0)>) {
    //         println!("insert1 is now");
    //         let e = i0.insert((Age3(3), Age1(1), Age0(0)));
    //         println!("insert1 is end, e:{:?}", e);
    //     }
    //     pub fn alter(
    //         mut i0: Alter<&Age1, (), (), ()>,
    //         q0: Query<(Entity, &mut Age0, &Age1), ()>,
    //     ) {
    //         println!("alter1 it:{:?}", q0.iter().size_hint());
    //         let item = q0.iter().next();
    //         assert_eq!(item.is_some(), true);
    //         let (e, _, _) = item.unwrap(); 
    //         let _r = i0.destroy(e);
    //         dbg!(_r);
            
    //         println!("alter1: end");
    //     }
    //     pub fn destroyed(q0: Query<(Entity, &mut Age0, &mut Age1), Destroyed>) {
    //         println!("destroyed");
    //         let item = q0.iter().next();
    //         assert_eq!(item.is_some(), true);
    //         let (e, age0, _) = item.unwrap();
    //         println!("e {:?}, age0: {:?}", e, age0);
        
    //         println!("destroyed: end");
    //     }
    //     let mut app = SingleThreadApp::new();
    //     app.add_system(Update, insert);
    //     app.add_system(Update, print_changed_entities);
    //     app.add_system(Update, alter);
    //     app.add_system(Update, destroyed);
    //     app.add_system(Update, print_info);
        
    //     app.world.assert_archetype_arr(&[None]);

    //     app.run();

    //     let mut info = ArchetypeDebug {
    //         entitys: Some(1),
    //         columns_info: vec![
    //             Some(ColumnDebug{change_listeners: 0, name: Some("Age3")}), 
    //             Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}), 
    //             Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}), 
    //         ],
    //         destroys_listeners: Some(1),
    //         removes: Some(0),
    //     };

    //     app.world.assert_archetype_arr(&[None, Some(info.clone())]);

    //     app.run();

    //     info.entitys = Some(2);

    //     app.world.assert_archetype_arr(&[None, Some(info)]);
    // }

    // #[test]
    // fn alter(){ 
    //     let mut world = World::new();
        
    //     let i = world.make_inserter::<(Age1, Age0)>();
    //     let e1 = i.insert((Age1(2), Age0(1)));
    //     println!("===========1114: {:?}", e1);
    //     let mut editor = world.make_entity_editor();
    //     let components  =[
    //         (editor.init_component::<Age3>(), true), 
    //         (editor.init_component::<Age1>(), false)
    //     ];
    //     editor.alter_components_by_index(e1, &components).unwrap();
        
    //     let age3 = world.get_component::<Age3>(e1);
    //     assert_eq!(age3.is_ok(), true);

    //     let age1 = world.get_component::<Age1>(e1);
    //     assert_eq!(age1.is_err(), true);

    //     println!(", age1: {:?}, age3: {:?}", age1, age3);
    // }

    // #[test]
    // fn app_alter(){
    //     let mut app = SingleThreadApp::new();
    //     // let mut world = &app.world;
    //     pub fn insert(i0: Insert<(Age1, Age0)>) {
    //         println!("insert1 is now");
    //         let e = i0.insert((Age1(1), Age0(0)));
    //         println!("insert1 is end, e:{:?}", e);
    //     }

    //     pub fn alter(mut w: EntityEditor, q: Query<(Entity, &Age1, &Age0)>) {
    //        q.iter().for_each(|(e, age1, age0)|{
    //         println!("alter!! e: {:?}, age1: {:?}, age0: {:?}", e, age1, age0);
    //             w.alter_components_by_index(e, &[
    //                 (w.init_component::<Age2>(), true), 
    //                 (w.init_component::<Age1>(), false)
    //             ]).unwrap();
    //        });
    //     }

    //     pub fn query(w: &World, q: Query<(Entity, &Age2, &Age0)>) {
    //         println!("query start!!!");
    //         q.iter().for_each(|(e, age2, age0)|{
    //             println!("query!!! e: {:?}, age2: {:?}, age0: {:?}", e, age2, age0);
    //         });
    //      }

    //     app.add_system(Update, insert);
    //     app.add_system(Update, alter);
    //     app.add_system(Update, query);

    //     app.world.assert_archetype_arr(&[None]);

    //     app.run();

    //     let mut info = ArchetypeDebug {
    //         entitys: Some(0),
    //         columns_info: vec![
    //             Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}), 
    //             Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}), 
    //         ],
    //         destroys_listeners: Some(0),
    //         removes: Some(0),
    //     };
    //     let mut info1 = ArchetypeDebug {
    //         entitys: Some(1),
    //         columns_info: vec![   
    //             Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}),
    //             Some(ColumnDebug{change_listeners: 0, name: Some("Age2")}), 
    //         ],
    //         destroys_listeners: Some(0),
    //         removes: Some(0),
    //     };
    //     app.world.assert_archetype_arr(&[None, Some(info), Some(info1)]);

    //     app.run();
    // }

    // #[test]
    // fn world_allow(){
    //     struct ResEntity(Entity);
    //     // std::thread::sleep_ms(1000 * 5);
    //     let mut world = World::new();
    //     let e = world.alloc_entity();
    //     world.assert_archetype_arr(&[None]);
    //     let mut components = [
    //         (world.init_component::<Age0>(), true),
    //         (world.init_component::<Age1>(), true)
    //     ];

    //     world.alter_components_by_index(e, &components).unwrap();

    //     let mut info = ArchetypeDebug {
    //         entitys: Some(1),
    //         columns_info: vec![
    //             Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}), 
    //             Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}), 
    //         ],
    //         destroys_listeners: Some(0),
    //         removes: Some(0),
    //     };
    //     world.assert_archetype_arr(&[None, Some(info)]);

    //     let age0 = world.get_component::<Age0>(e);
    //     assert_eq!(age0.is_ok(), true);
    //     println!("age0: {:?}", age0.unwrap());
    //     let age1 = world.get_component::<Age1>(e);
    //     assert_eq!(age1.is_ok(), true);
    //     println!("age1: {:?}", age1.unwrap());
        
    // }

    // #[test]
    // fn app_allow(){
    //     let mut app = SingleThreadApp::new();
    //     pub fn alter_add(w: &mut World) {
    //         let e = w.alloc_entity();
    //         println!("alter_add!! e: {:?}", e);

    //         let mut sort_components = [
    //             (w.init_component::<Age0>(), true), 
    //             (w.init_component::<Age1>(), true)
    //         ];
    //         sort_components.sort_by(|a, b| a.cmp(&b));
    
    //         w.alter_components_by_index(e, &sort_components).unwrap();

    //         println!("alter_add end");
    //      }

    //      pub fn alter_remove(w: &mut World, q: Query<(Entity, &Age1, &Age0)>) {
    //         // let e = w.alloc_entity();
    //         println!("alter_remove start!! ");
    //         let item =  q.iter().next();
    //         assert_eq!(item.is_some(), true);
    //         let (e, age2, age0) = item.unwrap();

    //         println!("alter_remove!! e: {:?}", e);
    //         let mut sort_components = [
    //             (w.init_component::<Age2>(), true), 
    //             (w.init_component::<Age1>(), false)
    //         ];
    //         sort_components.sort_by(|a, b| a.cmp(&b));

    //         w.alter_components_by_index(e, &sort_components).unwrap();

    //         println!("alter_remove end");
    //      }

    //     pub fn query(w: &World, q: Query<(Entity, &Age0, &Age2)>) {
    //         println!("query start!!!");

    //         let item =  q.iter().next();
    //         assert_eq!(item.is_some(), true);

    //         let (e, age0, age2) = item.unwrap();
    //         println!("query!!! e: {:?}, age0: {:?}, age2: {:?}", e, age0, age2);

    //         println!("query end!!!");
    //      }
         

    //      app.add_startup_system(Update, alter_add);
    //      app.add_startup_system(Update, alter_remove);
    //      app.add_system(Update, query);

    //      app.world.assert_archetype_arr(&[None]);

    //      app.run();

    //      let mut info1 = ArchetypeDebug {
    //         entitys: Some(1),
    //         columns_info: vec![
    //             Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}), 
    //             Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}), 
    //         ],
    //         destroys_listeners: Some(0),
    //         removes: Some(1),
    //     };
    //     let mut info2 = ArchetypeDebug {
    //         entitys: Some(1),
    //         columns_info: vec![
    //             Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}), 
    //             Some(ColumnDebug{change_listeners: 0, name: Some("Age2")}), 
    //         ],
    //         destroys_listeners: Some(0),
    //         removes: Some(0),
    //     };

    //     app.world.assert_archetype_arr(&[None, Some(info1), Some(info2)]);
    // }

    // #[test]
    // fn test_changed2() {
    //     let mut app = SingleThreadApp::new();
    //     pub fn alter_add(w: &mut World) {
    //         let e = w.alloc_entity();
    //         println!("alter_add!! e: {:?}", e);
    //         let mut sort_components = [
    //             (w.init_component::<Age0>(), true),
    //             (w.init_component::<Age1>(), true),
    //         ];
    //         sort_components.sort_by(|a, b| a.0.cmp(&b.0));

    //         w.alter_components_by_index(e, &sort_components).unwrap();

    //         println!("alter_add end");
    //     }

    //     pub fn alter_add2(w: &mut World, q: Query<(Entity, &Age1, &Age0), (Changed<Age1>)>) {
    //         // let e = w.alloc_entity();
    //         println!("alter_add2 start!!");
    //         // assert_eq!(q.len(), 1);
    //         let iter = q.iter().next();
    //         assert_eq!(iter.is_some(), true);
    //         let (e, age1, age0) = iter.unwrap();
    //         let mut sort_components = [(w.init_component::<Age2>(), true)];
    //         println!("alter_add2!! e: {:?}, age1: {:?}, age0:{:?}", e, age1, age0);
    //         w.alter_components_by_index(e, &sort_components).unwrap();

    //         println!("alter_add2 end");
    //     }

    //     pub fn query(q: Query<(Entity, &Age0, &Age2), (Changed<Age2>)>) {
    //         println!("query start!!!");

    //         let iter = q.iter().next();
    //         assert_eq!(iter.is_some(), true);
    //         let (e, age0, age2) = iter.unwrap();
    //         println!("query!!! e: {:?}, age0: {:?}, age2: {:?}", e, age0, age2);

    //         println!("query end!!!");
    //     }
    //     app.add_system(Update, alter_add);
    //     app.add_system(Update, alter_add2);
    //     app.add_system(Update, query);

        

    //     app.world.assert_archetype_arr(&[None]);

    //     app.run();

    //     let mut info1 = ArchetypeDebug {
    //         entitys: Some(1),
    //         columns_info: vec![
    //             Some(ColumnDebug{change_listeners: 1, name: Some("Age1")}), 
    //             Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}), 
    //         ],
    //         destroys_listeners: Some(0),
    //         removes: Some(1),
    //     };
    //     let mut info2 = ArchetypeDebug {
    //         entitys: Some(1),
    //         columns_info: vec![
    //             Some(ColumnDebug{change_listeners: 1, name: Some("Age1")}), 
    //             Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}), 
    //             Some(ColumnDebug{change_listeners: 1, name: Some("Age2")}), 
    //         ],
    //         destroys_listeners: Some(0),
    //         removes: Some(0),
    //     };

    //     app.world.assert_archetype_arr(&[None, Some(info1), Some(info2)]);
    //     // app.run();
    // }
    #[test]
    fn test_editor() {
        let mut app = crate::prelude::App::new();
        pub fn alter_add(mut edit: EntityEditor) {
            println!("alter_add start!!");
            let mut scomponents = [
                edit.init_component::<Age0>(), 
                edit.init_component::<Age1>()
            ];

            let e = edit.insert_entity_by_index(&scomponents).unwrap();
            println!("alter_add end!! e: {:?}", e);
        }

        pub fn alter_add2(
            mut edit: EntityEditor,
            q: Query<(Entity, &Age1, &Age0), (Changed<Age1>, Changed<Age0>)>,
        ) {
            println!("alter_add2 start!!");
            // assert_eq!(q.is_empty(), false);
            let mut iter = q.iter();
            let r = iter.next(); 
            assert_eq!(r.is_some(), true);
            let (e, age1, age0) = r.unwrap();

            println!("alter_add2!! e: {:?}, age1: {:?}, age0:{:?}", e, age1, age0);
            let arr = [
                (edit.init_component::<Age2>(), true),
                (edit.init_component::<Age3>(), true),
                (edit.init_component::<Age0>(), false),
            ];
            edit.alter_components_by_index(e, &arr).unwrap();
 
            println!("alter_add2 end");
        }

        pub fn alter_add3(
            mut edit: EntityEditor,
            q: Query<(Entity, &Age0), (Changed<Age1>)>,
        ) {
            println!("alter_add3 start!!");
            let iter = q.iter().next();
            assert_eq!(iter.is_none(), true);
 
            println!("alter_add3 end");
        }

        pub fn query(q: Query<(&Age1, &Age2, &Age3)>, removed: ComponentRemoved<Age0>) {
            println!("query start!!!");
            let re = removed.iter().next();
            assert_eq!(re.is_some(), true);
            let (age1, age2, age3) = q.get(*re.unwrap()).unwrap();
            println!("query end!!!");
        }

        pub fn query2(q: Query<(Entity, &Age1, &Age2, &Age3, ), (Changed<Age3>)>, editor: EntityEditor) {
            println!("query2 start!!!");
            let iter = q.iter().next();
            assert_eq!(iter.is_some(), true);
            let (e, age1, age2, age3) = iter.unwrap();
            editor.destroy(e);
            println!("query2 end!!!");
        }

        pub fn query3(q: Query<(Entity, &Age1, &Age2, &Age3,)>,) {
            println!("query3 start!!!");
            let iter = q.iter().next();
            assert_eq!(iter.is_null(), true);
            println!("query3 end!!!");
        }

        app.add_system(Update, alter_add);
        app.add_system(Update, alter_add2);
        app.add_system(Update, alter_add3);
        app.add_system(Update, query);
        app.add_system(Update, query2);
        app.add_system(Update, query3);

        app.run();

        let mut info1 = ArchetypeDebug {
            entitys: Some(0),
            columns_info: vec![
                Some(ColumnDebug{change_listeners: 2, name: Some("Age1")}), 
                Some(ColumnDebug{change_listeners: 1, name: Some("Age0")}), 
            ],
            destroys_listeners: Some(0),
            removes: Some(0),
        };
        let mut info2 = ArchetypeDebug {
            entitys: Some(0),
            columns_info: vec![
                Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}), 
                Some(ColumnDebug{change_listeners: 0, name: Some("Age2")}), 
                Some(ColumnDebug{change_listeners: 1, name: Some("Age3")}), 
            ],
            destroys_listeners: Some(0),
            removes: Some(0),
        };

        app.world.assert_archetype_arr(&[None, Some(info1), Some(info2)]);
        app.run();
        app.run();
        app.run();
        app.run();
    }

    #[test] 
    fn test_editor_settle() {
        let mut app = crate::prelude::App::new();
        pub fn alter_add(mut edit: EntityEditor) {
            let mut scomponents = [
                edit.init_component::<Age0>(), 
                edit.init_component::<Age1>()
            ];

            let e = edit.insert_entity_by_index(&scomponents).unwrap();
        }

        pub fn alter_add2(
            mut edit: EntityEditor,
            q: Query<(Entity, &Age1, &Age0), (Changed<Age1>, Changed<Age0>)>,
        ) {
            // assert_eq!(q.is_empty(), false);
            let mut iter = q.iter();
            let r = iter.next(); 
            assert_eq!(r.is_some(), true);
            let (e, age1, age0) = r.unwrap();
            assert_eq!(iter.next(), None);
            let arr = [
                (edit.init_component::<Age2>(), true),
                (edit.init_component::<Age3>(), true),
                (edit.init_component::<Age0>(), false),
            ];
            edit.alter_components_by_index(e, &arr).unwrap();
        }

        pub fn alter_add3(
            mut edit: EntityEditor,
            q: Query<(Entity, &Age0), (Changed<Age1>)>,
        ) {
            // assert_eq!(q.is_empty(), false);
            let iter = q.iter().next();
            assert_eq!(iter.is_null(), true);
        }

        pub fn query(q: Query<(&Age1, &Age2, &Age3)>, removed: ComponentRemoved<Age0>) {
            let re = removed.iter().next();
            assert_eq!(re.is_some(), true);
            let (age1, age2, age3) = q.get(*re.unwrap()).unwrap();
        }

        pub fn query2(q: Query<(Entity, &Age1, &Age2, &Age3, ), (Changed<Age3>)>, editor: EntityEditor) {
            let iter = q.iter().next();
            assert_eq!(iter.is_some(), true);
            let (e, age1, age2, age3) = iter.unwrap();
            editor.destroy(e);
        }

        pub fn query3(q: Query<(Entity, &Age1, &Age2, &Age3,)>,) {
            let iter = q.iter().next();
            assert_eq!(iter.is_null(), true);
        }

        app.add_system(Update, alter_add);
        app.add_system(Update, alter_add2);
        app.add_system(Update, alter_add3);
        app.add_system(Update, query);
        app.add_system(Update, query2);
        app.add_system(Update, query3);

        app.run();

        let mut info1 = ArchetypeDebug {
            entitys: Some(0),
            columns_info: vec![
                Some(ColumnDebug{change_listeners: 2, name: Some("Age1")}), 
                Some(ColumnDebug{change_listeners: 1, name: Some("Age0")}), 
            ],
            destroys_listeners: Some(0),
            removes: Some(0),
        };
        let mut info2 = ArchetypeDebug {
            entitys: Some(0),
            columns_info: vec![
                Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}), 
                Some(ColumnDebug{change_listeners: 0, name: Some("Age2")}), 
                Some(ColumnDebug{change_listeners: 1, name: Some("Age3")}), 
            ],
            destroys_listeners: Some(0),
            removes: Some(0),
        };

        app.world.assert_archetype_arr(&[None, Some(info1), Some(info2)]);
        for _ in 0..10_000 {
            app.run();
        }
    }

    #[test] 
    fn test_alter3() {
        pub struct EntityRes(Entity);

        let mut app = crate::prelude::App::new();
        let i = app.world.make_inserter::<(Age0, Age1, Age2)>();
        let e = i.insert((Age0(0), Age1(1), Age2(2)));
        println!("========== e: {:?}", e);
        app.world.insert_single_res(EntityRes(e));

        pub fn query(mut p: ParamSet<(Query<(Entity, &Age0, &Age1, &Age2)>, EntityEditor)> ) {
            println!("query start!!!");
            let mut entity = None;
            {
                let q = p.p0();
                q.iter().for_each(|(e, a0, a1, a2)|{
                    entity = Some(e);
                    println!("v: {:?}", (e, a0, a1, a2));
                });
            }
            assert_eq!(entity.is_some(), true);
            {
                let editor = p.p1();
                let index = editor.init_component::<Age0>();
                editor.remove_components_by_index(entity.unwrap(), &[index]);
            }

            println!("query end!!!");
        }

        pub fn query2(q: Query<(Entity, &Age0, &Age1, &Age2)>){
            let mut len = 0;
            q.iter().for_each(|(e, a0, a1, a2)|{
                len += 1;
                println!("v: {:?}", (e, a0, a1, a2));
            });
            assert_eq!(len, 0);
        }

       
        app.add_system(Update, query);
        app.add_system(Update, query2);

        let mut info = ArchetypeDebug {
            entitys: Some(1),
            columns_info: vec![
                Some(ColumnDebug{change_listeners: 0, name: Some("Age0")}), 
                Some(ColumnDebug{change_listeners: 0, name: Some("Age1")}), 
                Some(ColumnDebug{change_listeners: 0, name: Some("Age2")}), 
            ],
            destroys_listeners: Some(0),
            removes: Some(0),
        };

        app.world.assert_archetype_arr(&[None, Some(info.clone())]);
        app.run();

        info.columns_info[0].as_mut().unwrap().change_listeners = 0;
        info.entitys = Some(0);
        info.removes = Some(0);

        app.world.assert_archetype_arr(&[None, Some(info), None]);
    }

   
    #[test] 
    fn test_editor2() {
        pub struct EntityRes(Entity);

        let mut app = crate::prelude::App::new();

        pub fn insert_entity(mut editor: EntityEditor ) {
            println!("insert_entity start!!!");
            let e1 = editor.insert_entity((Age0(10),));
            let e2 = editor.insert_entity((Age0(20),));

            println!("insert_entity end!!! e: {:?}", (e1, e2));
        }

        pub fn add_components(q: Query<(Entity, &Age0)>, mut editor: EntityEditor){
            println!("query2 start!!!");
            let mut len = 0;
            q.iter().for_each(|(e, a0)|{
                len += 1;
                println!("v: {:?}", (e, a0));
                editor.add_components(e, (Age10(10), (Age1(1), Age2(2)))).unwrap();
                editor.add_components(e, (Age9(9),)).unwrap();

                let e2 = editor.alloc_entity();
                editor.add_components(e2, (Age10(10),(Age1(1), Age2(2)))).unwrap();
                editor.add_components(e2, (Age9(9),)).unwrap();
            });
            println!("query2 end!!!");
            assert_eq!(len, 2);
        }

        pub fn query3(q: Query<(Entity, &Age9, &Age1), (With<Age10>, Changed<Age2>)>){
            println!("query3 start!!!");
            let mut len = 0;
            q.iter().for_each(|(e, a9, a1 )|{
                len += 1;
                println!("v: {:?}", (e, a9, a1));
            });
            println!("query3 end!!!");
            assert_eq!(len, 4);
        }

       
        app.add_system(Update, insert_entity);
        app.add_system(Update, add_components);
        app.add_system(Update, query3);
        app.run();

    }

    #[test] 
    fn test_editor3() {
        pub struct EntityRes(Entity);

        let mut app = crate::prelude::App::new();

        pub fn insert_entity(mut editor: EntityEditor ) {
            println!("insert_entity start!!!");
            let e1 = editor.insert_entity((Age21(Vec::new()),));
            let e2 = editor.insert_entity((Age21(Vec::new()),));

            println!("insert_entity end!!! e: {:?}", (e1, e2));
        }

        pub fn add_components(mut q: Query<(Entity, &mut Age21)>, mut editor: EntityEditor){
            println!("query2 start!!!");
            let mut len = 0;
            q.iter_mut().for_each(|(e, mut a21)|{
                len += 1;
                println!("v: {:?}", (e, &a21.0));
                a21.0 = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];
                a21.0.push(21);
                a21.0.push(22);
            });
            println!("query2 end!!!");
            assert_eq!(len, 2);
        }

        pub fn query3(q: Query<(Entity, &Age21)>){
            println!("query3 start!!!");
            let mut len = 0;
            q.iter().for_each(|(e, a21,  )|{
                len += 1;
                println!("v: {:?}", (e, a21));
            });
            println!("query3 end!!!");
            assert_eq!(len, 2);
        }

       
        app.add_system(Update, insert_entity);
        app.add_system(Update, add_components);
        app.add_system(Update, query3);
        app.run();
        println!("=======================end");
    }
}
