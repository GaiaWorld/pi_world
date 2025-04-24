use pi_world::prelude::*;

#[derive(Debug, Component)]
struct A(f32);
#[derive(Debug, Component)]
struct B(f32);
#[derive(Debug, Component)]
struct C(f32);
#[derive(Debug, Component)]
struct D(f32);
#[derive(Debug, Component)]
struct E(f32);

fn ab(mut query: Query<(&mut A, &mut B)>) {
    for (mut a, mut b) in query.iter_mut() {
        std::mem::swap(&mut a.0, &mut b.0);
    }
}

fn cd(mut query: Query<(&mut C, &mut D)>) {
    for (mut d, mut c) in query.iter_mut() {
        std::mem::swap(&mut c.0, &mut d.0);
    }
}

fn ce(mut query: Query<(&mut C, &mut E)>) {
    for (mut c, mut e) in query.iter_mut() {
        std::mem::swap(&mut c.0, &mut e.0);
    }
}

pub struct Benchmark(App);

impl Benchmark {
    pub fn new() -> Self {
        let mut app = App::new();
        let mut editor = EntityEditor::new(&mut *app.world);

        for i in 0..10000 {
            editor.insert_entity((A(0.0), B(0.0)));
            editor.insert_entity((A(0.0), B(0.0), C(0.0)));
            editor.insert_entity((A(0.0), B(0.0), C(0.0), D(0.0)));
            editor.insert_entity((A(0.0), B(0.0), C(0.0), E(0.0)));
        }


        app.add_system(Update, ab);
        app.add_system(Update, cd);
        app.add_system(Update, ce);

        Self(app)
    }

    pub fn run(&mut self) {
        self.0.run();
    }
}
