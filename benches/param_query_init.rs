#![feature(test)]
extern crate test;



mod mod3 {
    use pi_world::prelude::{Component, Query, App, Update};
    use  test::Bencher;
    use super::*;

    #[derive(Debug)]
    pub struct Age(usize);
    #[derive(Debug)]
    pub struct Age1(usize);
    #[derive(Debug)]
    pub struct Age2(usize);
    #[derive(Debug)]
    pub struct Age3(usize);
    #[derive(Debug)]
    pub struct Age4(usize);
    #[derive(Debug)]
    pub struct Age5(usize);
    #[derive(Debug)]
    pub struct Age6(usize);
    #[derive(Debug)]
    pub struct Age7(usize);
    #[derive(Debug)]
    pub struct Age8(usize);
    #[derive(Debug)]
    pub struct Age9(usize);
    #[derive(Debug)]
    pub struct Age10(usize);
    #[derive(Debug)]
    pub struct Age11(usize);
    #[derive(Debug)]
    pub struct Age12(usize);
    #[derive(Debug)]
    pub struct Age13(usize);
    
    fn system(
        q: Query<&Age>,
        q1: Query<&Age1>,
        q2: Query<&Age2>,
        q3: Query<&Age3>,
        q4: Query<&Age4>,
        q5: Query<&Age5>,
        q6: Query<&Age6>,
        q7: Query<&Age7>,
        q8: Query<&Age8>,
        q9: Query<&Age9>,
        q10: Query<&Age10>,
        q11: Query<&Age11>,
        q12: Query<&Age12>,
        q13: Query<&Age13>,
    
    ) {
    }

    #[bench]
    fn param_init_3(b: &mut Bencher) {
        let mut app = App::new();

        let count = 500;
        for i in 0..count {
            app.add_system(Update, system);
        }
        
        // app.add_system(Update, p_set);
        
        app.run();

        b.iter(move || {
            app.run();
        });
    }
}

mod mod2 {
    use pi_world_2::prelude::{Component, Query, App, Update};
    use  test::Bencher;
    use super::*;
    #[derive(Debug)]
    pub struct Age(usize);
    #[derive(Debug)]
    pub struct Age1(usize);
    #[derive(Debug)]
    pub struct Age2(usize);
    #[derive(Debug)]
    pub struct Age3(usize);
    #[derive(Debug)]
    pub struct Age4(usize);
    #[derive(Debug)]
    pub struct Age5(usize);
    #[derive(Debug)]
    pub struct Age6(usize);
    #[derive(Debug)]
    pub struct Age7(usize);
    #[derive(Debug)]
    pub struct Age8(usize);
    #[derive(Debug)]
    pub struct Age9(usize);
    #[derive(Debug)]
    pub struct Age10(usize);
    #[derive(Debug)]
    pub struct Age11(usize);
    #[derive(Debug)]
    pub struct Age12(usize);
    #[derive(Debug)]
    pub struct Age13(usize);
    
    fn system(
        q: Query<&Age>,
        q1: Query<&Age1>,
        q2: Query<&Age2>,
        q3: Query<&Age3>,
        q4: Query<&Age4>,
        q5: Query<&Age5>,
        q6: Query<&Age6>,
        q7: Query<&Age7>,
        q8: Query<&Age8>,
        q9: Query<&Age9>,
        q10: Query<&Age10>,
        q11: Query<&Age11>,
        q12: Query<&Age12>,
        q13: Query<&Age13>,
    
    ) {
    }

    #[bench]
    fn param_init_2(b: &mut Bencher) {
        let mut app = App::new();

        let count = 500;
        for i in 0..count {
            app.add_system(Update, system);
        }
        
        // app.add_system(Update, p_set);
        
        app.run();

        b.iter(move || {
            app.run();
        });
    }
}