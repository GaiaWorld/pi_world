#![allow(warnings)]
use std::ops::Range;

use pi_world::prelude::*;
use pi_world::*;

#[derive(Copy, Clone, Debug, Eq, Default,PartialEq, Component)]
pub struct Age0(usize);

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Component)]
pub struct Age1(usize);

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Component)]
pub struct Age2(usize);

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Component)]
pub struct Age3(usize);

#[derive(Component, Default)]
pub struct Age4(usize);
#[derive(Component, Default)]
pub struct Age5([usize; 16], Range<u64>, Range<u64>);
#[derive(Component, Default)]
pub struct Age6(usize);
#[derive(Component, Default)]
pub struct Age7(usize, Range<u64>, Range<u64>, Option<Range<u64>>);
#[derive(Component, Default)]
pub struct Age8(Vec<u32>, Vec<f32>, usize, usize); 
#[derive(Component, Debug, Default)]
pub struct Age9(usize);
#[derive(Component, Debug, Default)]
pub struct Age10(usize);
#[derive(Component, Default)]
pub struct Age11(Vec<u32>, Vec<f32>, usize, usize);
#[derive(Component, Default)]
pub struct Age12([f32;8]);
#[derive(Component, Default)]
pub struct Age13([f32;8]);
#[derive(Component, Default)]
pub struct Age14([f32;8]);
#[derive(Component, Default)]
pub struct Age15([f32;8]);
#[derive(Component, Default)]
pub struct Age16([f32;12]);
#[derive(Component, Default)]
pub struct Age17([f32;16], [f32;4], bool);
#[derive(Component, Default)]
pub struct Age18([f32;16]);
#[derive(Component, Default)]
pub struct Age19([f32;16]);
#[derive(Component, Default)]
pub struct Age20([f32;16]);

#[derive(Component, Debug, Default)]
pub struct Age21(Vec<u64>);

#[derive(Component, Default)]
pub struct Age22([f32;16], [f32;4], bool, Vec<u64>);
#[derive(Component, Default)]
pub struct Age23([f32;16], [f32;4], bool, Vec<u64>);

#[derive(Component, Default)]
pub struct Age24([f32;16], [f32;4], bool, Vec<u64>);
#[derive(Component, Default)]
pub struct Age25([f32;16], [f32;4], bool, Vec<u64>);
#[derive(Component, Default)]
pub struct Age26([f32;16], [f32;4], bool, Vec<u64>);
#[derive(Component, Default)]
pub struct Age27([f32;16], [f32;4], bool, Vec<u64>);
#[derive(Component, Default)]
pub struct Age28([f32;16], [f32;4], bool, Vec<u64>);
#[derive(Component, Default)]
pub struct Age29([f32;16], [f32;4], bool, Vec<u64>);
#[derive(Component, Default)]
pub struct Age30(usize, Range<u64>, Range<u64>, Option<Range<u64>>);
#[derive(Component, Default)]
pub struct Age31(usize, Range<u64>, Range<u64>, Option<Range<u64>>);
#[derive(Component, Default)]
pub struct Age32(usize, Range<u64>, Range<u64>, Option<Range<u64>>);
#[derive(Component, Default)]
pub struct Age33(usize, Range<u64>, Range<u64>, Option<Range<u64>>);
#[derive(Component, Default)]
pub struct Age34(usize, Range<u64>, Range<u64>, Option<Range<u64>>);
#[derive(Component, Default)]
pub struct Age35(usize, Range<u64>, Range<u64>, Option<Range<u64>>);
#[derive(Component, Default)]
pub struct Age36(usize, Range<u64>, Range<u64>, Option<Range<u64>>);
#[derive(Component, Default)]
pub struct Age37(usize, Range<u64>, Range<u64>, Option<Range<u64>>);
#[derive(Component, Default)]
pub struct Age38(usize, Range<u64>, Range<u64>, Option<Range<u64>>);
#[derive(Component, Default)]
pub struct Age39(usize, Range<u64>, Range<u64>, Option<Range<u64>>);
#[derive(Component, Default)]
pub struct Age40(usize, Range<u64>, Range<u64>, Option<Range<u64>>);
#[derive(Component, Default)]
pub struct Age41(usize, Range<u64>, Range<u64>, Option<Range<u64>>);
#[derive(Component, Default)]
pub struct Age42(usize, Range<u64>, Range<u64>, Option<Range<u64>>);
#[derive(Component, Default)]
pub struct Age43(usize, Range<u64>, Range<u64>, Option<Range<u64>>);
#[derive(Component, Default)]
pub struct Age44(usize, Range<u64>, Range<u64>, Option<Range<u64>>);

#[derive(Component, Default)]
pub struct PassReset;

#[derive(Component, Default)]
pub struct PassModelID(pub Entity);

#[derive(Component, Default)]
pub struct PassRendererID(pub Entity);

#[derive(Component, Default)]
pub struct PassMaterialID(pub Entity);

#[derive(Component, Default)]
pub struct PassGeometryID(pub Entity);

#[derive(Component, Default)]
pub struct PassPipelineStateDirty;

#[derive(Component, Default)]
pub struct PassBindGroupsDirty;

#[derive(Component, Default)]
pub struct PassDrawDirty;

#[derive(Component, Default)]
pub struct PassIDs(pub [Entity;8]);

#[derive(Clone, Component, Default)]
pub struct RecordPassDraw(pub [Option<Entity>; 8]);

#[derive(Component, Default)]
pub struct PassFlagShader;

#[derive(Component, Default)]
pub struct RenderState([f32;11]);

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

type Bundle0 = (Age0, Age1, Age2, Age3, Age4, Age5, Age6, Age7);
type Bundle1 = (Age8, Age9, Age10, Age11, Age12, Age13, Age14, Age15);
type Bundle2 = (Age16, Age17, Age18, Age19, Age20, Age21);
type Bundle3 = (Age22, Age23, Age24, Age25, Age26, Age27);
type Bundle4 = (Age28, Age29, Age30, Age31, Age32, Age33, Age34, Age35, Age36, Age37, Age38, Age39);
type Bundle5 = (Age40, Age41, Age42, Age43, Age44, PassObjInitBundle);

pub fn main() {
    let mut app = pi_world::prelude::App::new();
    
    app.run();

    let mut alter1 = app.world.make_alter::<(), (), Bundle0, ()>();
    let mut alter2 = app.world.make_alter::<(), (), Bundle1, ()>();
    let mut alter3 = app.world.make_alter::<(), (), Bundle2, ()>();
    let mut alter4 = app.world.make_alter::<(), (), (Bundle2, Bundle3), ()>();
    let mut alter5 = app.world.make_alter::<(), (), (Bundle1, Bundle2, Bundle3, Bundle4, Bundle5), ()>();
    let mut query5 = app.world.make_query::<(&Age8, &Age9, &Age10, &Age11, &Age12), ()>();
    loop {
        for i in 0..100 {
            let entity = app.world.spawn_empty();
            let mut alter = alter1.get_param(&app.world);
            alter.alter(entity, Bundle0::default());

            let entity = app.world.spawn_empty();
            let mut alter = alter2.get_param(&app.world);
            alter.alter(entity, Bundle1::default());

            let entity = app.world.spawn_empty();
            let mut alter = alter3.get_param(&app.world);
            alter.alter(entity, Bundle2::default());

            let entity = app.world.spawn_empty();
            let mut alter = alter4.get_param(&app.world);
            alter.alter(entity, (Bundle2::default(), Bundle3::default()));

            let entity = app.world.spawn_empty();
            let mut alter = alter5.get_param(&app.world);
            alter.alter(entity, (Bundle1::default(), Bundle2::default(), Bundle3::default(), Bundle4::default(), Bundle5::default()));

            query5.align(&app.world);
        }
        app.run();
    }
}