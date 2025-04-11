use std::{mem::transmute, sync::OnceLock};

use pi_world::prelude::{App, Update, World};

pub static WORLD_PTR: OnceLock<usize> = OnceLock::new();

#[test]
fn test() {
    let mut app = App::new();
    let world_addr = unsafe {transmute::<_, usize>(&*app.world as &World)} ;
    WORLD_PTR.get_or_init(|| {world_addr});

    fn system1 (world: &World) {
        debug_assert_eq!(unsafe {transmute::<_, usize>(world)}, *WORLD_PTR.get().unwrap());
    }
    fn system2 (world: &mut World) {
        debug_assert_eq!(unsafe {transmute::<_, usize>(world)}, *WORLD_PTR.get().unwrap());
    }
    app.add_system(Update, system1);
    app.add_system(Update, system2);

    app.run();
}


