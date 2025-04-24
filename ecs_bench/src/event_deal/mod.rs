pub struct ImmediateBenchmark(usize, Vec<fn(&mut usize)>);
impl ImmediateBenchmark {
    pub fn new() -> Self {
        Self(0,  vec![deal, deal, deal])
    }

    pub fn run(&mut self) {
		let mut i = 0;
        for _ in 0..100000 {
            for f in self.1.iter() {
				f(&mut i);
			}
        }
		self.0 = i;
    }
}

fn deal(i: &mut usize) {
	*i += 1;
}

pub struct DelayBenchmark(usize, Vec<usize>, fn(&mut usize));
impl DelayBenchmark {
    pub fn new() -> Self {
		let mut arr = Vec::with_capacity(100000);
		for i in 0..100000 {
            arr.push(i);
        }
        Self(0, arr, deal)
    }

    pub fn run(&mut self) {
		// self.1.clear();
		let mut i = 0;
        for i in 0..100000 {
            self.1.push(i);
        }
		let r = self.2;
		for _ in self.1.iter() {
			r(&mut i);
		}
		for _ in self.1.iter() {
			r(&mut i);
		}
		for _ in self.1.iter() {
			r(&mut i);
		}
		self.0 = i;
    }
}

// #[test]
// fn aa () {
// 	let mut r = DelayBenchmark::new();
// 	let mut r1 = ImmediateBenchmark::new();
// 	r.run();
// 	r1.run();
// 	println!("{}, {}", r.0, r1.0)
// }