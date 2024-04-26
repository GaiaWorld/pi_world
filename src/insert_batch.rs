use std::iter::FusedIterator;
use std::mem::transmute;

use crate::archetype::{Archetype, ArchetypeWorldIndex, ComponentInfo, ShareArchetype};
use crate::insert::{Insert, Bundle};
use crate::world::{Entity, World};

pub struct InsertBatchIter<'w, I, Ins>
where
    I: Iterator<Item = <Ins as Bundle>::Item>,
    Ins: Bundle,
{
    world: &'w mut World,
    inner: I,
    state: (
        ArchetypeWorldIndex,
        ShareArchetype,
        <Ins as Bundle>::State,
    ),
}

impl<'w, I, Ins> InsertBatchIter<'w, I, Ins>
where
    I: Iterator<Item = <Ins as Bundle>::Item>,
    Ins: Bundle,
{
    #[inline]
    pub(crate) fn new(world: &'w mut World, iter: I) -> Self {
        let (lower, upper) = iter.size_hint();
        let length = upper.unwrap_or(lower);
        let components = Ins::components();
        let id = ComponentInfo::calc_id(&components);
        let (ar_index, ar) = world.find_archtype(id, components);
        let s = Ins::init_state(world, &ar);

        // world.entitys.reserve(length);
        let ptr = ShareArchetype::as_ptr(&ar);
        let ar_mut: &mut Archetype = unsafe { transmute(ptr) };
        ar_mut.table.reserve(length);

        Self {
            world,
            inner: iter,
            state: (ar_index, ar, s),
        }
    }
}

impl<I, Ins> Drop for InsertBatchIter<'_, I, Ins>
where
    I: Iterator<Item = <Ins as Bundle>::Item>,
    Ins: Bundle,
{
    fn drop(&mut self) {
        for _ in self {}
    }
}

impl<I, Ins> Iterator for InsertBatchIter<'_, I, Ins>
where
    I: Iterator<Item = <Ins as Bundle>::Item>,
    Ins: Bundle,
{
    type Item = Entity;

    fn next(&mut self) -> Option<Entity> {
        let item = self.inner.next()?;
        let i = Insert::<Ins>::new(&self.world, &mut self.state);
        Some(i.insert(item))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<I, Ins> ExactSizeIterator for InsertBatchIter<'_, I, Ins>
where
    I: ExactSizeIterator<Item = <Ins as Bundle>::Item>,
    Ins: Bundle,
{
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<I, Ins> FusedIterator for InsertBatchIter<'_, I, Ins>
where
    I: FusedIterator<Item = <Ins as Bundle>::Item>,
    Ins: Bundle,
{
}
