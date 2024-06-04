
use std::marker::PhantomData;
use std::mem::transmute;

use pi_proc_macros::all_tuples;
use pi_slot::SlotMap;
// use pi_world_macros::ParamSetElement;

use crate::archetype::*;
use crate::column::Column;
use crate::editor::EntityEditor;
use crate::param_set::ParamSetElement;
use crate::prelude::QueryError;
use crate::system::SystemMeta;
use crate::system_params::SystemParam;
use crate::world::*;
pub use pi_world_macros::Bundle;
pub use pi_world_macros::Component;

// 插入器， 一般是给外部的应用通过world上的make_inserter来创建和使用
pub struct Inserter<'world, I: Bundle> {
    world: &'world World,
    state: (ArchetypeWorldIndex, ShareArchetype, I::Item),
    tick: Tick,
}

impl<'world, I: Bundle> Inserter<'world, I> {
    #[inline(always)]
    pub fn new(
        world: &'world World,
        state: (ArchetypeWorldIndex, ShareArchetype, I::Item),
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
        unsafe { &mut *ptr }.reserve(length);
        // 强行将原型转为可写
        let ptr = ShareArchetype::as_ptr(&self.state.1);
        let ar_mut: &mut Archetype = unsafe { transmute(ptr) };
        ar_mut.reserve(length);
        for item in iter {
            self.insert(item);
        }
    }
}


pub struct Insert<'world, I: Bundle> {
    pub(crate) world: &'world World,
    state: &'world (ArchetypeWorldIndex, ShareArchetype, I::Item),
    tick: Tick,
}

impl<'world, I: Bundle> Insert<'world, I> {
    #[inline(always)]
    pub(crate) fn new(
        world: &'world World,
        state: &'world (ArchetypeWorldIndex, ShareArchetype, I::Item),
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
        let row = self.state.1.alloc();
        let e = self.world.insert(self.state.0, row);
        I::insert(&self.state.2, components, e, row, self.tick);
        self.state.1.set(row, e);
        e
    }
}

impl<I: Bundle + 'static> SystemParam for Insert<'_, I> {
    type State = (ArchetypeWorldIndex, ShareArchetype, I::Item);
    type Item<'w> = Insert<'w, I>;

    fn init_state(world: &mut World, _system_meta: &mut SystemMeta) -> Self::State {
        // 如果world上没有找到对应的原型，则创建并放入world中
        let components = I::components(Vec::new());
        let (ar_index, ar) = world.find_ar(components);
        let s = I::init_item(world, &ar);
        (ar_index, ar, s)
    }
    fn archetype_depend(
        world: &World,
        _system_meta: &SystemMeta,
        _state: &Self::State,
        archetype: &Archetype,
        depend: &mut ArchetypeDependResult,
    ) {
        // todo 似乎可以使用state上的ShareArchetype
        let components = I::components(Vec::new());
        depend.insert(archetype, world, components);
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

impl<I: Bundle + 'static> ParamSetElement for Insert<'_, I>  {
    fn init_set_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State{
        let components = I::components(Vec::new());
        // todo 移到system_meta，减少泛型代码
        for component in &components{
            system_meta.cur_param
            .writes
            .insert(component.type_id, component.type_name.clone());
        }
    
        let (ar_index, ar) = world.find_ar( components);
        let s = I::init_item(world, &ar);
        system_meta.param_set_check();

        (ar_index, ar, s)
    }
}


pub trait Bundle {
    // type Item;

    type Item: Send + Sync + Sized;

    fn components(c: Vec<ComponentInfo>) -> Vec<ComponentInfo>;
    // todo 改名成init_item，每次get_parm时调用，TState放ColumnIndex
    fn init_item(world: &World, archetype: &Archetype) -> Self::Item;

    fn insert(item: &Self::Item, components: Self, e: Entity, row: Row, tick: Tick);
}

pub trait BundleExt: Bundle {
    fn add_components(editor: &mut EntityEditor, e: Entity, components: Self) -> Result<(), QueryError>;
    fn add_components2(editor: &mut EntityEditor, e: Entity, components: Self) -> Result<(), QueryError>;
}

pub struct TypeItem<T: 'static>(pub *const Column, PhantomData<T>);
unsafe impl<T> Sync for TypeItem<T> {}
unsafe impl<T> Send for TypeItem<T> {}
impl<T: 'static> TypeItem<T> {
    #[inline(always)]
    pub fn new(c: &Column) -> Self {
        // println!("TypeItem new:{:?} {:p}", (c.info().type_name), c);
        TypeItem(unsafe { transmute(c) }, PhantomData)
    }
    #[inline(always)]
    pub fn write(&self, e: Entity, row: Row, val: T, tick: Tick) {
        // println!("TypeItem write:{:?} {:p}", (e, row, tick), self.0);
        let c: &mut Column = unsafe { transmute(self.0) };
        c.write(row, val);
        c.add_record(e, row, tick);
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

macro_rules! impl_tuple_add {
    ($(($name: ident, $item:ident, $name1:ident)),*) => {
        #[allow(non_snake_case)]
        #[allow(clippy::unused_unit)]

        impl<$($name: 'static + BundleExt),*> BundleExt for ($($name,)*) {
            fn add_components(editor: &mut EntityEditor, e: Entity,  components: Self) -> Result<(), crate::prelude::QueryError> {
                let components_index = [
                    $(
                        (editor.init_component::<$name>(), true),
                    )*
                ];
            
                editor.alter_components_by_index(e, &components_index)?;

                let ($($item,)*) = components;
                let [$($name1,)*] = components_index;
 
                $(
                    *editor.get_component_unchecked_mut_by_id(e, $name1.0) = $item;
                )*
               
                Ok(())
            }

            fn add_components2(_editor: &mut EntityEditor, _e: Entity,  components: Self) -> Result<(), crate::prelude::QueryError> {
                let ($($item,)*) = components;
    
                $(
                    <$name as BundleExt>::add_components2(_editor, _e, $item)?;
                )*
               
                Ok(())
            }
        }
    };
}
all_tuples!(impl_tuple_add, 0, 32, F, S, n);