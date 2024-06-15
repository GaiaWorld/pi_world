use std::any::TypeId;
use std::iter::FusedIterator;
use std::marker::PhantomData;
use std::mem::transmute;

use pi_proc_macros::all_tuples;
use pi_share::Share;
use pi_slot::SlotMap;

use crate::archetype::*;
use crate::column::Column;
use crate::system::SystemMeta;
use crate::system_params::SystemParam;
use crate::world::*;
pub use pi_world_macros::Bundle;
pub use pi_world_macros::Component;


pub struct Insert<'world, B: Bundle> {
    pub(crate) world: &'world World,
    state: &'world InsertState<B>,
    tick: Tick,
}

impl<'world, B: Bundle> Insert<'world, B> {
    #[inline(always)]
    pub(crate) fn new(world: &'world World, state: &'world InsertState<B>, tick: Tick) -> Self {
        Insert { world, state, tick }
    }
    #[inline]
    pub fn tick(&self) -> Tick {
        self.tick
    }
    #[inline]
    pub fn insert(&self, components: B) -> Entity {
        self.state
            .insert_with_tick(&self.world, components, self.tick)
    }
}

impl<B: Bundle + 'static> SystemParam for Insert<'_, B> {
    type State = InsertState<B>;
    type Item<'w> = Insert<'w, B>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        // 加meta 如果world上没有找到对应的原型，则创建并放入world中
        let components = B::components(Vec::new());
        let ar = meta.insert(world, components);
        let s = B::init_item(world, &ar);
        InsertState::new(ar, s)
    }
    #[inline]
    fn get_param<'world>(
        world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world> {
        Insert::new(world, state, tick)
    }
    #[inline]
    fn get_self<'world>(
        world: &'world World,
        system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, system_meta, state, tick)) }
    }
}

pub struct InsertState<B: Bundle> {
    pub(crate) archetype: ShareArchetype,
    pub(crate) item: B::Item,
}

impl<B: Bundle> InsertState<B> {
    #[inline(always)]
    pub fn new(archetype: ShareArchetype, item: B::Item) -> Self {
        Self { archetype, item }
    }
    #[inline(always)]
    pub fn insert(&self, world: &World, components: B) -> Entity {
        self.insert_with_tick(world, components, world.tick())
    }
    #[inline(always)]
    pub fn insert_with_tick(&self, world: &World, components: B, tick: Tick) -> Entity {
        let (r, row) = self.archetype.alloc();
        let e = world.insert_addr(self.archetype.index(), row.into());
        B::insert(&self.item, components, e, row.into(), tick);
        *r = e;
        e
    }
    #[inline(always)]
    pub fn batch<'w, I: IntoIterator<Item = B>>(
        &'w self,
        world: &'w World,
        iter: I,
    ) -> InsertBatchIter<'_, <I as IntoIterator>::IntoIter, B> {
        let tick = world.tick();
        let iter = iter.into_iter();
        let (lower, upper) = iter.size_hint();
        let length = upper.unwrap_or(lower);
        let ptr: *mut SlotMap<Entity, EntityAddr> = unsafe { transmute(&world.entities) };
        unsafe { &mut *ptr }.settle(length);
        // 强行将原型转为可写
        let ptr = ShareArchetype::as_ptr(&self.archetype);
        let ar_mut: &mut Archetype = unsafe { transmute(ptr) };
        ar_mut.reserve(length);
        InsertBatchIter {
            world,
            inner: iter,
            state: self,
            tick,
        }
    }
}
pub struct InsertBatchIter<'w, I, B>
where
    I: Iterator<Item = B>,
    B: Bundle,
{
    world: &'w World,
    inner: I,
    state: &'w InsertState<B>,
    tick: Tick,
}

impl<I, B> Drop for InsertBatchIter<'_, I, B>
where
    I: Iterator<Item = B>,
    B: Bundle,
{
    fn drop(&mut self) {
        for _ in self {}
    }
}
impl<I, B> Iterator for InsertBatchIter<'_, I, B>
where
    I: Iterator<Item = B>,
    B: Bundle,
{
    type Item = Entity;

    fn next(&mut self) -> Option<Entity> {
        let item = self.inner.next()?;
        let i = Insert::<B>::new(&self.world, &mut self.state, self.tick);
        Some(i.insert(item))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}
impl<I, B> ExactSizeIterator for InsertBatchIter<'_, I, B>
where
    I: ExactSizeIterator<Item = B>,
    B: Bundle,
{
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<I, B> FusedIterator for InsertBatchIter<'_, I, B>
where
    I: FusedIterator<Item = B>,
    B: Bundle,
{
}

pub trait Bundle {
    type Item: Send + Sync + Sized;

    fn components(c: Vec<ComponentInfo>) -> Vec<ComponentInfo>;

    fn init_item(world: &World, archetype: &Archetype) -> Self::Item;

    fn insert(item: &Self::Item, components: Self, e: Entity, row: Row, tick: Tick);
}

pub struct TypeItem<T: 'static>(Share<Column>, ArchetypeIndex, PhantomData<T>);
unsafe impl<T> Sync for TypeItem<T> {}
unsafe impl<T> Send for TypeItem<T> {}
impl<T: 'static> TypeItem<T> {
    #[inline(always)]
    pub fn new(world: &World, ar: &Archetype) -> Self {
        let c = world.get_column_by_id(&TypeId::of::<T>()).unwrap().clone();
        TypeItem(c, ar.index(), PhantomData)
    }
    #[inline(always)]
    pub fn write(&self, val: T, e: Entity, row: Row, tick: Tick) {
        let c = self.0.blob_ref_unchecked(self.1);
        c.write(row, e, val);
        c.added_tick(e, row, tick);
    }
}

macro_rules! impl_tuple_insert {
    ($(($name: ident, $item: ident)),*) => {
        #[allow(non_snake_case)]
        #[allow(clippy::unused_unit)]
        impl<$($name: 'static + Bundle),*> Bundle for ($($name,)*) {
            type Item = ($(<$name as Bundle>::Item,)*);

            fn components(c: Vec<ComponentInfo>) -> Vec<ComponentInfo> {
                $(let c = $name::components(c);)*
                c
            }
            fn init_item(_world: &World, _archetype: &Archetype) -> Self::Item {
                ($(<$name as Bundle>::init_item(_world, _archetype),)*)
            }

            fn insert(
                _item: &Self::Item,
                _components: Self,
                _e: Entity,
                _row: Row,
                _tick: Tick,
            ) {
                let ($($item,)*) = _item;
                let ($($name,)*) = _components;
                $(
                    <$name as Bundle>::insert($item, $name, _e, _row, _tick);
                )*
            }
        }
    };
}
all_tuples!(impl_tuple_insert, 0, 32, F, S);
