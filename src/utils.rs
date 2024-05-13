use pi_null::Null;

pub trait VecExt {
    type Item;
    fn insert_value(&mut self, index: usize, value: Self::Item);
}

impl<T: Null> VecExt for Vec<T> {
    type Item = T;

    fn insert_value(&mut self, index: usize, value: Self::Item) {
        if index < self.len() {
            self[index] = value
        } else {
            for _ in self.len()..index {
                self.push(<Self::Item as Null>::null())
            }
            self.push(value);
        }
    }
}
