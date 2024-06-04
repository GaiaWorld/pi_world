
use std::marker::PhantomData;
use std::mem::transmute;

use pi_proc_macros::all_tuples;
use pi_slot::SlotMap;
// use pi_world_macros::ParamSetElement;

use crate::archetype::*;
use crate::column::BlobTicks;
use crate::column::BlobRef;
use crate::system::SystemMeta;
use crate::system_params::SystemParam;
use crate::world::*;
pub use pi_world_macros::Bundle;
pub use pi_world_macros::Component;

// 插入器， 一般是给外部的应用通过world上的make_inserter来创建和使用
pub struct Inserter<'world, I: Bundle> {
    world: &'world World,
    state: (ShareArchetype, I::Item),
    tick: Tick,
}

impl<'world, I: Bundle> Inserter<'world, I> {
    #[inline(always)]
    pub fn new(
        world: &'world World,
        state: (ShareArchetype, I::Item),
        tick: Tick,
    ) -> Self {
        Self { world, state, tick }
    }
    #[inline(always)]
    pub fn insert(&self, components: I) -> Entity {
        Insert::<I>::new(self.world, &self.state, self.tick).insert(components)
    }
    #[inline(always)]
    pub fn batch(&self, iter: impl IntoIterator<Item = I>) {
        let iter = iter.into_iter();
        let (lower, upper) = iter.size_hint();
        let length = upper.unwrap_or(lower);
        let ptr: *mut SlotMap<Entity, EntityAddr> = unsafe { transmute(&self.world.entities) };
        unsafe { &mut *ptr }.settle(length);
        // 强行将原型转为可写
        let ptr = ShareArchetype::as_ptr(&self.state.0);
        let ar_mut: &mut Archetype = unsafe { transmute(ptr) };
        ar_mut.reserve(length);
        for item in iter {
            self.insert(item);
        }
    }
}


pub struct Insert<'world, I: Bundle> {
    pub(crate) world: &'world World,
    state: &'world (ShareArchetype, I::Item),
    tick: Tick,
}

impl<'world, I: Bundle> Insert<'world, I> {
    #[inline(always)]
    pub(crate) fn new(
        world: &'world World,
        state: &'world (ShareArchetype, I::Item),
        tick: Tick,
    ) -> Self {
        Insert { world, state, tick }
    }
    #[inline]
    pub fn tick(&self) -> Tick {
        self.tick
    }
    #[inline]
    pub fn insert(&self, components: I) -> Entity {
        let (r, row) = self.state.0.alloc();
        let e = self.world.insert(self.state.0.index(), row.into());
        I::insert(&self.state.1, components, e, row.into(), self.tick);
        *r = e;
        e
    }
}

impl<I: Bundle + 'static> SystemParam for Insert<'_, I> {
    type State = (ShareArchetype, I::Item);
    type Item<'w> = Insert<'w, I>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        // 加meta 如果world上没有找到对应的原型，则创建并放入world中
        let components = I::components(Vec::new());
        let ar = meta.insert(world, components);
        let s = I::init_item(world, &ar);
        (ar, s)
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

// impl<I: Bundle + 'static> ParamSetElement for Insert<'_, I>  {
//     fn init_set_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State{
//         let components = I::components(Vec::new());
//         // todo 移到system_meta，减少泛型代码
//         for component in &components{
//             system_meta.cur_param
//             .writes
//             .insert(component.type_id, component.type_name.clone());
//         }
    
//         let (ar_index, ar) = world.find_ar( components);
//         let s = I::init_item(world, &ar);
//         system_meta.param_set_check();

//         (ar_index, ar, s)
//     }
// }


pub trait Bundle {

    type Item: Send + Sync + Sized;

    fn components(c: Vec<ComponentInfo>) -> Vec<ComponentInfo>;

    fn init_item(world: &World, archetype: &Archetype) -> Self::Item;

    fn insert(item: &Self::Item, components: Self, e: Entity, row: Row, tick: Tick);
}

pub struct TypeItem<T: 'static>(*const BlobTicks, *const ComponentInfo, PhantomData<T>);
unsafe impl<T> Sync for TypeItem<T> {}
unsafe impl<T> Send for TypeItem<T> {}
impl<T: 'static> TypeItem<T> {
    #[inline(always)]
    pub fn new(world: &World, ar: &Archetype) -> Self {
        // println!("TypeItem new:{:?} {:p}", (c.info().type_name), c);
        let  c = world.add_component_info(ComponentInfo::of::<T>(0)).1;
        let c = c.blob_ref(ar.index());
        TypeItem(unsafe { transmute(c.blob) },  unsafe { transmute(c.info) }, PhantomData)
    }
    #[inline(always)]
    pub fn write(&self, val: T, e: Entity, row: Row, tick: Tick) {
        // println!("TypeItem write:{:?} {:p}", (e, row, tick), self.0);
        let c = unsafe { transmute(self.0) };
        let info = unsafe { transmute(self.1) };
        let c = BlobRef::new(c, info);
        c.write(row, val);
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
