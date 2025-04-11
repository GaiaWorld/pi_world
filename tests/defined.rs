    #![allow(warnings)]
    use std::{any::TypeId, mem, ops::Deref, ptr};
 
    // use super::*;
    use pi_world::{
        // app::*,
        alter::Alter, archetype::{Archetype, ComponentInfo, Row}, column::Column, editor::EntityEditor, insert::Insert, schedule::Update, schedule_config::IntoSystemConfigs, system::{relate, Relation, SystemMeta, TypeInfo}, table::Table, world
    };
    use fixedbitset::FixedBitSet;
    // use bevy_utils::dbg;
    use pi_append_vec::AppendVec;
    // use pi_async_rt::rt::single_thread::SingleTaskRuntime;
    use pi_null::Null;
    use pi_share::Share;
    use rand::Rng;

    #[derive(ScheduleLabel, Hash, Eq, PartialEq, Clone, Debug)]
    pub struct AddSchedule;


use std::ops::Range;

use pi_world::prelude::*;

#[derive(Copy, Clone, Debug, Eq, Default,PartialEq, Component)]
pub struct Age0(pub usize);

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Component)]
pub struct Age1(pub usize);

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Component)]
pub struct Age2(pub usize);

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Component)]
pub struct Age3(pub usize);

#[derive(Component, Default, Clone)]
pub struct Age4(pub usize);
#[derive(Component, Default, Clone)]
pub struct Age5(pub [usize; 16], pub Range<u64>, pub Range<u64>);
#[derive(Component, Default, Clone)]
pub struct Age6(pub usize);
#[derive(Component, Default, Clone)]
pub struct Age7(pub usize, pub Range<u64>, pub Range<u64>, pub Option<Range<u64>>);
#[derive(Component, Default, Clone)]
pub struct Age8(pub Vec<u32>, pub Vec<f32>, pub usize, pub usize); 
#[derive(Component, Debug, Default, Clone)]
pub struct Age9(pub usize);
#[derive(Component, Debug, Default, Clone)]
pub struct Age10(pub usize);
#[derive(Component, Default, Clone)]
pub struct Age11(pub Vec<u32>, pub Vec<f32>, pub usize, pub usize);
#[derive(Component, Default, Clone)]
pub struct Age12(pub [f32;8]);
#[derive(Component, Default, Clone)]
pub struct Age13(pub [f32;8]);
#[derive(Component, Default, Clone)]
pub struct Age14(pub [f32;8]);
#[derive(Component, Default, Clone)]
pub struct Age15(pub [f32;8]);
#[derive(Component, Default, Clone)]
pub struct Age16(pub [f32;12]);
#[derive(Component, Default, Clone)]
pub struct Age17(pub [f32;16], pub [f32;4], pub bool);
#[derive(Component, Default, Clone)]
pub struct Age18(pub [f32;16]);
#[derive(Component, Default, Clone)]
pub struct Age19(pub [f32;16]);
#[derive(Component, Default, Clone)]
pub struct Age20(pub [f32;16]);

#[derive(Clone, Component, Debug, Default)]
pub struct Age21(pub Vec<u64>);

#[derive(Component, Default, Clone)]
pub struct Age22(pub [f32;16], pub [f32;4], pub bool, pub Vec<u64>);
#[derive(Component, Default, Clone)]
pub struct Age23(pub [f32;16], pub [f32;4], pub bool, pub Vec<u64>);

#[derive(Component, Default, Clone)]
pub struct Age24(pub [f32;16], pub [f32;4], pub bool, pub Vec<u64>);
#[derive(Component, Default, Clone)]
pub struct Age25(pub [f32;16], pub [f32;4], pub bool, pub Vec<u64>);
#[derive(Component, Default, Clone)]
pub struct Age26(pub [f32;16], pub [f32;4], pub bool, pub Vec<u64>);
#[derive(Component, Default, Clone)]
pub struct Age27(pub [f32;16], pub [f32;4], pub bool, pub Vec<u64>);
#[derive(Component, Default, Clone)]
pub struct Age28(pub [f32;16], pub [f32;4], pub bool, pub Vec<u64>);
#[derive(Component, Default, Clone)]
pub struct Age29(pub [f32;16], pub [f32;4], pub bool, pub Vec<u64>);
#[derive(Component, Default, Clone)]
pub struct Age30(pub usize, pub Range<u64>, pub Range<u64>, pub Option<Range<u64>>);
#[derive(Component, Default, Clone)]
pub struct Age31(pub usize, pub Range<u64>, pub Range<u64>, pub Option<Range<u64>>);
#[derive(Component, Default, Clone)]
pub struct Age32(pub usize, pub Range<u64>, pub Range<u64>, pub Option<Range<u64>>);
#[derive(Component, Default, Clone)]
pub struct Age33(pub usize, pub Range<u64>, pub Range<u64>, pub Option<Range<u64>>);
#[derive(Component, Default, Clone)]
pub struct Age34(pub usize, pub Range<u64>, pub Range<u64>, pub Option<Range<u64>>);
#[derive(Component, Default, Clone)]
pub struct Age35(pub usize, pub Range<u64>, pub Range<u64>, pub Option<Range<u64>>);
#[derive(Component, Default, Clone)]
pub struct Age36(pub usize, pub Range<u64>, pub Range<u64>, pub Option<Range<u64>>);
#[derive(Component, Default, Clone)]
pub struct Age37(pub usize, pub Range<u64>, pub Range<u64>, pub Option<Range<u64>>);
#[derive(Component, Default, Clone)]
pub struct Age38(pub usize, pub Range<u64>, pub Range<u64>, pub Option<Range<u64>>);
#[derive(Component, Default, Clone)]
pub struct Age39(pub usize, pub Range<u64>, pub Range<u64>, pub Option<Range<u64>>);
#[derive(Component, Default, Clone)]
pub struct Age40(pub usize, pub Range<u64>, pub Range<u64>, pub Option<Range<u64>>);
#[derive(Component, Default, Clone)]
pub struct Age41(pub usize, pub Range<u64>, pub Range<u64>, pub Option<Range<u64>>);
#[derive(Component, Default, Clone)]
pub struct Age42(pub usize, pub Range<u64>, pub Range<u64>, pub Option<Range<u64>>);
#[derive(Component, Default, Clone)]
pub struct Age43(pub usize, pub Range<u64>, pub Range<u64>, pub Option<Range<u64>>);
#[derive(Component, Default, Clone)]
pub struct Age44(pub usize, pub Range<u64>, pub Range<u64>, pub Option<Range<u64>>);

#[derive(Component, Default, Clone)]
pub struct PassReset;

#[derive(Component, Default, Clone)]
pub struct PassModelID(pub Entity);

#[derive(Component, Default, Clone)]
pub struct PassRendererID(pub Entity);

#[derive(Component, Default, Clone)]
pub struct PassMaterialID(pub Entity);

#[derive(Component, Default, Clone)]
pub struct PassGeometryID(pub Entity);

#[derive(Component, Default, Clone)]
pub struct PassPipelineStateDirty;

#[derive(Component, Default, Clone)]
pub struct PassBindGroupsDirty;

#[derive(Component, Default, Clone)]
pub struct PassDrawDirty;

#[derive(Component, Default, Clone)]
pub struct PassIDs(pub [Entity;8]);

#[derive(Clone, Component, Default)]
pub struct RecordPassDraw(pub [Option<Entity>; 8]);

#[derive(Component, Default, Clone)]
pub struct PassFlagShader;

#[derive(Component, Default, Clone)]
pub struct RenderState(pub [f32;11]);

pub type PassObjInitBundle = (
    PassModelID,
    PassMaterialID,
    PassGeometryID,
    PassRendererID,
    PassPipelineStateDirty,
    PassDrawDirty,
    RenderState,
    PassReset,
    PassBindGroupsDirty,
    PassFlagShader,
);

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
 