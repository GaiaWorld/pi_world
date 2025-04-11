use std::ops::{Deref, DerefMut};

#[derive(Debug)]
pub struct Ptr<T: Send + Sync>(*mut T);

impl<T: Send + Sync> Ptr<T> {
    // 安全： world不可移动
    pub fn new(world: &mut T) -> Self {
        Self(world as *mut T)
    }
}

impl<T: Send + Sync> Deref for Ptr<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0 }
    }
}

impl<T: Send + Sync> DerefMut for Ptr<T>  {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.0 }
    }
}

unsafe impl<T: Send + Sync> Send for Ptr<T> {
    
}
unsafe impl<T: Send + Sync> Sync for Ptr<T> {
    
}