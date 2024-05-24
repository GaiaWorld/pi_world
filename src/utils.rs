use pi_null::Null;

pub trait VecExt<T> {
    fn insert_value(&mut self, index: usize, value: T);
}

impl<T: Null> VecExt<T> for Vec<T> {
    default fn insert_value(&mut self, index: usize, value: T) {
        if index >= self.len() {
            self.resize_with(index + 1, || T::null());
        }
        unsafe { *self.get_unchecked_mut(index) = value; };
    }
}
impl<T: Null + Clone> VecExt<T> for Vec<T> {
    fn insert_value(&mut self, index: usize, value: T) {
        let t = T::null();
        if index >= self.len() {
            self.resize(index + 1, t);
        }
        unsafe { *self.get_unchecked_mut(index) = value; };
    }
}
