use pi_world::{param_set::ParamSet, prelude::{App, Component, Query, Update}};
// use pi_world_macros::{Component, SystemSet};
// use pi_world::schedule_config;

#[derive(Debug, Component)]
pub struct Age(pub usize);

// fn aa<Marker: 'static, Out: 'static + Send + Sync, F: SystemParamFunction<Marker, Out>> (xx: F) {}

// fn bb<Func: Send + Sync + 'static, Out, P: SystemFetch> (xx: Func)
//         where
//         Func:
//                 FnMut(P) -> Out +
//                 FnMut(SystemParamFetch1<P>) -> Out
//         {}

#[test]
fn test() {
    let mut app = App::new();
    let e = app.world.spawn_empty();
    let r = app.world.make_entity_editor().add_components(e, Age(5));

    debug_assert_eq!(r.is_ok(), true);
    pub fn system1(query_set: ParamSet<(Query<&Age>, Query<&mut Age>)>) {
        let mut count = 0;
        let query = query_set.p0();
        for i in  query.iter() {
            count += 1;
            println!("i: {:?}", i);
            debug_assert_eq!(i.0, 5);
        }

        let query = query_set.p1();
        for i in  query.iter() {
            count += 1;
            println!("i: {:?}", i);
            debug_assert_eq!(i.0, 5);
        }

        debug_assert_eq!(count, 2);
    }
    app.add_system(Update, system1);
    

    app.run();
}


