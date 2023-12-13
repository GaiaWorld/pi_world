use std::any::TypeId;
use std::marker::PhantomData;

use pi_proc_macros::all_tuples;

use crate::archetype::*;
use crate::raw::{ArchetypeData, ArchetypePtr};
use crate::record::ComponentRecord;
use crate::system::SystemMeta;
use crate::system_parms::SystemParam;
use crate::world::*;

pub struct Insert<'world, I: InsertComponents> {
    world: &'world World,
    state: &'world (ShareArchetype, I::State),
    tick: Tick,
    _k: PhantomData<I>,
}

impl<'world, I: InsertComponents> Insert<'world, I> {
    pub fn new(
        world: &'world World,
        state: &'world (ShareArchetype, I::State),
        tick: Tick,
    ) -> Self {
        Insert {
            world,
            state,
            tick,
            _k: PhantomData,
        }
    }
    pub fn insert(&self, components: <I as InsertComponents>::Item) -> Entity {
        let (key, mut data) = self.state.0.alloc();
        I::insert(&self.state.1, components, &mut data);
        let e = self.world.insert(&self.state.0, key, data, self.tick);
        I::set_record(&self.state.1, key, self.state.0.get_records());
        e
    }
}

impl<I: InsertComponents + 'static> SystemParam for Insert<'_, I> {
    type State = (ShareArchetype, I::State);
    type Item<'w> = Insert<'w, I>;

    fn init_state(world: &World, system_meta: &mut SystemMeta) -> Self::State {
        // 如果world上没有找到对应的原型，则创建并放入world中
        let ar = world.find_archtype(I::components());
        world.archtype_ok(&ar);
        let s = I::init_state(world, &ar);
        system_meta.write_archetype_map.insert(*ar.get_id());
        (ar, s)
    }

    #[inline]
    fn get_param<'world>(
        state: &'world mut Self::State,
        _system_meta: &'world SystemMeta,
        world: &'world World,
        change_tick: Tick,
    ) -> Self::Item<'world> {
        Insert::new(world, state, change_tick)
    }
}

#[inline(always)]
fn record(key: ArchetypeKey, records: &Vec<ComponentRecord>, index: ComponentIndex) {
    let records = unsafe { records.get_unchecked(index as usize) };
    if records.addeds.len() > 0 {
        records.added(key);
    }
}
pub trait InsertComponents {

    type Item;

    type State: Send + Sync + Sized;

    fn components() -> Vec<ComponentInfo>;

    fn init_state(world: &World, archetype: &Archetype) -> Self::State;

    fn insert(state: &Self::State, components: Self::Item, data: &mut ArchetypeData);
    fn set_record(state: &Self::State, key: ArchetypeKey, records: &Vec<ComponentRecord>);
}

pub struct TState<T: 'static>(pub MemOffset, pub ComponentIndex, PhantomData<T>);
unsafe impl<T> Sync for TState<T> {}
unsafe impl<T> Send for TState<T> {}
impl<T: 'static> TState<T> {
    pub fn new((offset, index): (MemOffset, ComponentIndex)) -> Self {
        TState(offset, index, PhantomData)
    }
}

macro_rules! impl_tuple_insert {
    ($(($name: ident, $state: ident)),*) => {
        #[allow(non_snake_case)]
        #[allow(clippy::unused_unit)]
        impl<$($name: 'static),*> InsertComponents for ($($name,)*) {
            type Item = ($($name,)*);
            type State = ($(TState<$name>,)*);

            fn components() -> Vec<ComponentInfo> {
                vec![$(ComponentInfo::of::<$name>(),)*]
            }
            fn init_state(_world: &World, archetype: &Archetype) -> Self::State {
                ($(TState::new(archetype.get_mem_offset_ti_index(&TypeId::of::<$name>())),)*)
            }

            fn insert(
                _state: &Self::State,
                _components: Self::Item,
                _data: &mut ArchetypeData,
            ) {
                let ($($name,)*) = _components;
                let ($($state,)*) = _state;
                $(
                    {let r = _data.init_component::<$name>($state.0);
                    r.write($name);}
                )*
            }
            fn set_record(
                _state: &Self::State,
                _key: ArchetypeKey,
                _records: &Vec<ComponentRecord>,
            ){
                let ($($state,)*) = _state;
                $(record(_key, _records, $state.1);)*
            }
        }
    };
}
all_tuples!(impl_tuple_insert, 1, 20, F, S);
