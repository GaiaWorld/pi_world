use pi_world::prelude::{World, App, Update, SingleRes, SingleResMut};

#[derive(Debug)]
pub struct Age(pub usize);
pub struct Id(pub usize);

#[test]
fn test() {
    let mut app = App::new();
    app.world.insert_single_res(Age(5));

    pub fn system1(age: SingleRes<Age>, age1: Option<SingleRes<Age>>, id: Option<SingleRes<Id>>) {
        debug_assert_eq!(age.0, 5);
        debug_assert_eq!(age1.is_some(), true);
        let age1 = age1.as_ref().unwrap();
        debug_assert_eq!(age1.0, 5);
        debug_assert_eq!(id.is_none(), true);
    }

    pub fn system2(age: SingleResMut<Age>, id: Option<SingleResMut<Id>>) {
        debug_assert_eq!(age.0, 5);
        debug_assert_eq!(id.is_none(), true);
    }

    pub fn system3(age1: Option<SingleResMut<Age>>) {
        debug_assert_eq!(age1.is_some(), true);
        let age1 = age1.as_ref().unwrap();
        debug_assert_eq!(age1.0, 5);
    }
    app.add_system(Update, system1);
    app.add_system(Update, system2);
    app.add_system(Update, system3);
    

    app.run();
}


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

    fn ce(_w: &World, c: SingleRes<C>, mut e: SingleResMut<E>, mut b: SingleResMut<B>) {
        e.0 += c.0 + 1.0;
        b.0 += c.0 + 1.0;
        println!("ce:{:?}", b.0);
    }
    
    let mut app = pi_world::prelude::App::new();
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


