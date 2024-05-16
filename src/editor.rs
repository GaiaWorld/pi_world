use std::mem::transmute;

use crate::{
    archetype::{Archetype, ArchetypeDependResult},
    insert::Bundle,
    prelude::{Entity, QueryError, Tick, World},
    system::SystemMeta,
    system_params::SystemParam,
    world::ComponentIndex,
};

pub struct EntityEditor<'w>(&'w World);

impl<'w> EntityEditor<'w> {
    pub fn new(w: &'w World) -> Self {
        Self(w)
    }

    pub fn add_components(
        &self,
        e: Entity,
        components: &[ComponentIndex],
    ) -> Result<(), QueryError> {
        let mut add_components = Vec::with_capacity(components.len());
        for component in components{
            add_components.push((*component, true));
        }
        self.0.alter_components(e, &add_components)
    }

    pub fn remove_components(
        &self,
        e: Entity,
        components: &[ComponentIndex],
    ) -> Result<(), QueryError> {
        let mut add_components = Vec::with_capacity(components.len());
        for component in components{
            add_components.push((*component, false));
        }
        self.0.alter_components(e, &add_components)
    }

    pub fn alter_components(
        &self,
        e: Entity,
        components: &[(ComponentIndex, bool)],
    ) -> Result<(), QueryError> {
        self.0.alter_components(e, components)
    }

    pub fn insert_components(
        &self,
        e: Entity,
        components: &[ComponentIndex],
    ) -> Result<(), QueryError> {
        // self.0.i.alter_components(e, components)
        todo!()
    }

    pub fn destroy(&self, e: Entity) -> Result<(), QueryError> {
        self.0.destroy_entity2(e)
    }

    pub fn alloc(&self) -> Entity {
        self.0.alloc_entity()
    }

    pub fn get<B: Bundle + 'static>(&self, e: Entity) -> Result<&B, QueryError> {
        self.0.get_component::<B>(e)
    }

    pub fn get_mut<B: Bundle + 'static>(&self, e: Entity) -> Result<&mut B, QueryError> {
        // self.0.get_component_mut::<B>(e)
        todo!()
    }

    pub fn get_unchecked<B: Bundle>(&self, e: Entity) -> &'w B {
        todo!()
    }

    pub fn get_unchecked_mut<B: Bundle>(&self, e: Entity) -> &'w mut B {
        todo!()
    }
}

impl SystemParam for EntityEditor<'_> {
    type State = ();
    type Item<'w> = EntityEditor<'w>;

    fn init_state(world: &mut World, _system_meta: &mut SystemMeta) -> Self::State {
        // 如果world上没有找到对应的原型，则创建并放入world中
        // let components = I::components(Vec::new());
        // let id = ComponentInfo::calc_id(&components);
        // let (ar_index, ar) = world.find_archtype(id, components);
        // let s = I::init_state(world, &ar);
        // (ar_index, ar, s)
        ()
    }
    fn archetype_depend(
        world: &World,
        _system_meta: &SystemMeta,
        _state: &Self::State,
        archetype: &Archetype,
        depend: &mut ArchetypeDependResult,
    ) {
        // let components = I::components(Vec::new());
        // depend.insert(archetype, world, components);
    }

    #[inline]
    fn get_param<'world>(
        world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world> {
        EntityEditor::new(world)
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
