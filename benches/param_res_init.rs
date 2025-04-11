#![feature(test)]
extern crate test;



mod mod3 {
    use pi_world::{prelude::{App, Component, Query, Update, SingleRes}};
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
        q: SingleRes<Age>,
        q1: SingleRes<Age1>,
        q2: SingleRes<Age2>,
        q3: SingleRes<Age3>,
        q4: SingleRes<Age4>,
        q5: SingleRes<Age5>,
        q6: SingleRes<Age6>,
        q7: SingleRes<Age7>,
        q8: SingleRes<Age8>,
        q9: SingleRes<Age9>,
        q10: SingleRes<Age10>,
        q11: SingleRes<Age11>,
        q12: SingleRes<Age12>,
        q13: SingleRes<Age13>,
    
    ) {
    }

    #[bench]
    fn param_init_3(b: &mut Bencher) {
        let mut app = App::new();
        app.world.insert_single_res(Age(0));
        app.world.insert_single_res(Age1(0));
        app.world.insert_single_res(Age2(0));
        app.world.insert_single_res(Age3(0));
        app.world.insert_single_res(Age4(0));
        app.world.insert_single_res(Age5(0));
        app.world.insert_single_res(Age6(0));
        app.world.insert_single_res(Age7(0));
        app.world.insert_single_res(Age8(0));
        app.world.insert_single_res(Age9(0));
        app.world.insert_single_res(Age10(0));
        app.world.insert_single_res(Age11(0));
        app.world.insert_single_res(Age12(0));
        app.world.insert_single_res(Age13(0));

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
    use pi_world_2::prelude::{Component, Query, App, Update, SingleRes};
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
        q: SingleRes<Age>,
        q1: SingleRes<Age1>,
        q2: SingleRes<Age2>,
        q3: SingleRes<Age3>,
        q4: SingleRes<Age4>,
        q5: SingleRes<Age5>,
        q6: SingleRes<Age6>,
        q7: SingleRes<Age7>,
        q8: SingleRes<Age8>,
        q9: SingleRes<Age9>,
        q10: SingleRes<Age10>,
        q11: SingleRes<Age11>,
        q12: SingleRes<Age12>,
        q13: SingleRes<Age13>,
    
    ) {
    }

    #[bench]
    fn param_init_2(b: &mut Bencher) {
        let mut app = App::new();
        app.world.insert_single_res(Age(0));
        app.world.insert_single_res(Age1(0));
        app.world.insert_single_res(Age2(0));
        app.world.insert_single_res(Age3(0));
        app.world.insert_single_res(Age4(0));
        app.world.insert_single_res(Age5(0));
        app.world.insert_single_res(Age6(0));
        app.world.insert_single_res(Age7(0));
        app.world.insert_single_res(Age8(0));
        app.world.insert_single_res(Age9(0));
        app.world.insert_single_res(Age10(0));
        app.world.insert_single_res(Age11(0));
        app.world.insert_single_res(Age12(0));
        app.world.insert_single_res(Age13(0));

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