use pi_world::prelude::{App, Local, Update};

pub struct L(pub usize);

impl  Default for L  {
    fn default() -> Self {
        Self(10)
    }
}
#[test]
fn test() {
    let mut app = App::new();

    pub fn system1(local: Local<L>) {
        debug_assert_eq!(local.0, 10);
    }
    app.add_system(Update, system1);

    app.run();
}


