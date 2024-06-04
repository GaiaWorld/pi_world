use std::iter::FusedIterator;
use std::mem::transmute;

use crate::archetype::{Archetype, ArchetypeWorldIndex, ShareArchetype};
use crate::insert::{Insert, Bundle};
use crate::prelude::Tick;
use crate::world::{Entity, World};

pub struct InsertBatchIter<'w, I, Ins>
where
    I: Iterator<Item = Ins>,
    Ins: Bundle,
{
    world: &'w mut World,
    inner: I,
    state: (
        ShareArchetype,
        <Ins as Bundle>::Item,
    ),
    tick: Tick,
}

impl<'w, I, Ins> InsertBatchIter<'w, I, Ins>
where
    I: Iterator<Item = Ins>,
    Ins: Bundle,
{
    #[inline]
    pub(crate) fn new(world: &'w mut World, iter: I) -> Self {
        let (lower, upper) = iter.size_hint();
        let length = upper.unwrap_or(lower);
        let components = Ins::components(Vec::new());
        let ar = world.find_ar( components);
        let s = Ins::init_item(world, &ar);

        // world.entitys.reserve(length);
        let ptr = ShareArchetype::as_ptr(&ar);
        let ar_mut: &mut Archetype = unsafe { transmute(ptr) };
        ar_mut.reserve(length);
        let tick = world.tick();
        Self {
            world,
            inner: iter,
            state: (ar, s),
            tick,
        }
    }
}

impl<I, Ins> Drop for InsertBatchIter<'_, I, Ins>
where
    I: Iterator<Item = Ins>,
    Ins: Bundle,
{
    fn drop(&mut self) {
        for _ in self {}
    }
}

impl<I, Ins> Iterator for InsertBatchIter<'_, I, Ins>
where
    I: Iterator<Item = Ins>,
    Ins: Bundle,
{
    type Item = Entity;

    fn next(&mut self) -> Option<Entity> {
        let item = self.inner.next()?;
        let i = Insert::<Ins>::new(&self.world, &mut self.state, self.tick);
        Some(i.insert(item))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<I, Ins> ExactSizeIterator for InsertBatchIter<'_, I, Ins>
where
    I: ExactSizeIterator<Item = Ins>,
    Ins: Bundle,
{
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<I, Ins> FusedIterator for InsertBatchIter<'_, I, Ins>
where
    I: FusedIterator<Item = Ins>,
    Ins: Bundle,
{
}
