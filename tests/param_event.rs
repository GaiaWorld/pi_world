
use pi_world::prelude::{Update, SingleRes, EventWriter, EventReader};



#[test]
fn test_event() { 
    struct A(f32);
    #[derive(Clone, Copy, Default)]
    struct B(f32);

    fn ab(a: SingleRes<A>, b: EventWriter<B>) {
        b.send(B(a.0 + 1.0));
    }

    fn cd(b: EventReader<B>) {
        println!("cd start");
        for i in b.iter() {
            assert_eq!(i.0, 2.0);
        }
        println!("cd end");
    }

    let mut app = pi_world::prelude::App::new();
    app.world.insert_single_res(A(1.0));
    app.add_system(Update, ab);
    app.add_system(Update, cd);

    app.run();
    app.run();
}