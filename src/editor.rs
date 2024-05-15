use std::mem::transmute;

use crate::{
    archetype::{Archetype, ArchetypeDependResult},
    insert::Bundle,
    prelude::{Entity, QueryError, Tick, World},
    system::SystemMeta,
    system_params::SystemParam,
    world::ComponentIndex,
};

pub struct Editor<'w>(&'w World);

impl<'w> Editor<'w> {
    pub fn new(w: &'w World) -> Self {
        Self(w)
    }

    pub fn add_components(&self, e: Entity, components: &[ComponentIndex]) -> Result<(), QueryError> {
        Ok(())
    }

    pub fn remove_components(&self, e: Entity, components: &[ComponentIndex]) -> Result<(), QueryError> {
        Ok(())
    }

    pub fn insert_components(&self, e: Entity, components: &[ComponentIndex]) -> Result<(), QueryError> {
        Ok(())
    }

    pub fn delete_entity(&self, e: Entity) -> Result<(), QueryError> {
        Ok(())
    }

    pub fn get< B: Bundle>(&self, e: Entity) -> &'w B {
        todo!()
    }

    pub fn get_mut< B: Bundle>(&self, e: Entity) -> &'w mut B {
        todo!()
    }
}

impl SystemParam for Editor<'_> {
    type State = ();
    type Item<'w> = Editor<'w>;

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
        Editor::new(world)
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
