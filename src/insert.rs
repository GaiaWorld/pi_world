use std::any::TypeId;
use std::marker::PhantomData;

use pi_proc_macros::all_tuples;

use crate::archetype::*;
use crate::record::ComponentRecord;
use crate::raw::{ArchetypePtr, ArchetypeData};
use crate::system::SystemMeta;
use crate::system_parms::SystemParam;
use crate::world::*;

pub struct Insert<'world, Q: InsertComponents> {
    world: &'world World,
    state: &'world (ShareArchetype, Q::State),
    tick: Tick,
    _k: PhantomData<Q>,
}

impl<'world, Q: InsertComponents> Insert<'world, Q> {
    pub fn new(
        world: &'world World,
        state: &'world (ShareArchetype, Q::State),
        tick: Tick,
    ) -> Self {
        Insert {
            world,
            state,
            tick,
            _k: PhantomData,
        }
    }
    pub fn insert(&mut self, components: <Q as InsertComponents>::Item) -> Entity {
        let (key, mut data) = self.state.0.alloc();
        println!("insert!, key:{}, ar:{:?}", key, self.state.0.get_id());
        Q::insert(
            &self.state.1,
            components,
            &mut data,
        );
        let e = self.world.insert(&self.state.0, key, data, self.tick);
        Q::set_record(&self.state.1, key, self.state.0.get_records());
        e
    }
}

// SAFETY: Relevant Insert ComponentId and ArchetypeComponentId access is applied to SystemMeta. If
// this Insert conflicts with any prior access, a panic will occur.
impl<Q: InsertComponents + 'static> SystemParam for Insert<'_, Q> {
    type State = (ShareArchetype, Q::State);
    type Item<'w> = Insert<'w, Q>;

    fn init_state(world: &World, system_meta: &mut SystemMeta) -> Self::State {
        // 如果world上没有找到对应的原型，则创建并放入world中
        let ar = world.find_archtype(Q::components());
        world.archtype_ok(ar.clone());
        let s = Q::init_state(world, &ar);
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
        // SAFETY: We have registered all of the Insert's world accesses,
        // so the caller ensures that `world` has permission to access any
        // world data that the Insert needs.
        Insert::new(world, state, change_tick)
    }

}
fn set_record(key: ArchetypeKey, records: &Vec<ComponentRecord>, index: ComponentIndex) {
    let records = unsafe { records.get_unchecked(index as usize) };
    records.added(key);
}
pub trait InsertComponents {
    /// The item returned by this [`WorldInsert`]
    type Item;

    /// State used to construct a [`Self::Fetch`](crate::Insert::WorldInsert::Fetch). This will be cached inside [`InsertState`](crate::Insert::InsertState),
    /// so it is best to move as much data / computation here as possible to reduce the cost of
    /// constructing [`Self::Fetch`](crate::Insert::WorldInsert::Fetch).
    type State: Send + Sync + Sized;

    fn components() -> Vec<ComponentInfo>;

    /// Creates and initializes a [`State`](WorldInsert::State) for this [`WorldInsert`] type.
    fn init_state(world: &World, archetype: &Archetype) -> Self::State;

    fn insert(
        state: &Self::State,
        components: Self::Item,
        data: &mut ArchetypeData,
    );
    fn set_record(
        state: &Self::State,
        key: ArchetypeKey,
        records: &Vec<ComponentRecord>,
    );
}

impl<T0: 'static> InsertComponents for (T0,) {
    type Item = (T0,);
    type State = ((MemOffset, ComponentIndex),);

    fn components() -> Vec<ComponentInfo> {
        vec![ComponentInfo::of::<T0>()]
    }
    fn init_state(_world: &World, archetype: &Archetype) -> Self::State {
        (archetype.get_mem_offset_ti_index(&TypeId::of::<T0>()),)
    }

    fn insert(
        state: &Self::State,
        components: Self::Item,
        data: &mut ArchetypeData,
    ) {
        let r = data.init_component::<T0>(state.0.0);
        r.write(components.0);
    }
    fn set_record(
        state: &Self::State,
        key: ArchetypeKey,
        records: &Vec<ComponentRecord>,
    ){
        set_record(key, records, state.0.1);
    }

}
impl<T0: 'static, T1: 'static> InsertComponents for (T0,T1,) {
    type Item = (T0,T1,);
    type State = ((MemOffset, ComponentIndex),(MemOffset, ComponentIndex),);

    fn components() -> Vec<ComponentInfo> {
        vec![ComponentInfo::of::<T0>(),ComponentInfo::of::<T1>()]
    }
    fn init_state(_world: &World, archetype: &Archetype) -> Self::State {
        (archetype.get_mem_offset_ti_index(&TypeId::of::<T0>()),archetype.get_mem_offset_ti_index(&TypeId::of::<T1>()))
    }

    fn insert(
        state: &Self::State,
        components: Self::Item,
        data: &mut ArchetypeData,
    ) {
        let r = data.init_component::<T0>(state.0.0);
        r.write(components.0);
        let r = data.init_component::<T1>(state.1.0);
        r.write(components.1);
    }
    fn set_record(
        state: &Self::State,
        key: ArchetypeKey,
        records: &Vec<ComponentRecord>,
    ){
        set_record(key, records, state.0.1);
        set_record(key, records, state.1.1);
    }

}


macro_rules! impl_tuple_insert {
    ($(($name: ident, $state: ident)),*) => {
        #[allow(non_snake_case)]
        #[allow(clippy::unused_unit)]
        impl<$($name: 'static),*> InsertComponents for ($($name,)*) {
            type Item = ($($name,)*);
            type State = ($($state,)*);

            fn components() -> Vec<TypeInfo> {
                vec![$(TypeInfo::of::<$name>(),)*]
            }
            fn init_state(_world: &World, archetype: &Archetype) -> Self::State {
                ($(archetype.get_mem_offset_ti_index(&TypeId::of::<$name>()),)*)
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
                )*;
            }
            fn set_record(
                _state: &Self::State,
                _key: ArchetypeKey,
                _records: &Vec<RecordList>,
            ){
                let ($($state,)*) = _state;
                ($(set_record(_key, _records, $state.1),)*);
            }
        }

    };
}
// all_tuples!(impl_tuple_insert, 1, 15, F, S);
