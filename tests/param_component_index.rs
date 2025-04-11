
use pi_world::{prelude::{App, Update, Component, ComponentDebugIndex}, world::ComponentIndex};

#[derive(Debug, Component)]
pub struct Age(pub usize);
#[test]
fn test() {
    let mut app = App::new();

    fn system1 (index: ComponentDebugIndex<Age>) {
        debug_assert_eq!((*index).0, ComponentIndex::from(0 as u32));
    }

    app.add_system(Update, system1);

    app.run();
}


