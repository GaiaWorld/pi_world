use std::mem::transmute;

use crate::{
    insert::Bundle,
    prelude::{Entity, Mut, QueryError, Tick, World},
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
        let components = components
            .iter()
            .filter_map(|v| Some((*v, false)))
            .collect::<Vec<(ComponentIndex, bool)>>();

        self.0.alter_components(e, &components)
    }

    pub fn remove_components(
        &self,
        e: Entity,
        components: &[ComponentIndex],
    ) -> Result<(), QueryError> {
        let components = components
            .iter()
            .filter_map(|v| Some((*v, false)))
            .collect::<Vec<(ComponentIndex, bool)>>();

        self.0.alter_components(e, &components)
    }

    pub fn alter_components(
        &self,
        e: Entity,
        components: &[(ComponentIndex, bool)],
    ) -> Result<(), QueryError> {
        self.0.alter_components(e, components)
    }

    pub fn insert_components(&self, components: &[ComponentIndex]) -> Result<Entity, QueryError> {
        let e = self.0.alloc_entity();
        let components = components
            .iter()
            .filter_map(|v| Some((*v, true)))
            .collect::<Vec<(ComponentIndex, bool)>>();
        // self.0.i.alter_components(e, components)
        self.alter_components(e, &components)?;
        Ok(e)
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

    pub fn get_mut<B: Bundle + 'static>(&self, e: Entity) -> Result<Mut<B>, QueryError> {
        self.0.get_component_mut1::<B>(e)
    }

    pub fn get_unchecked<B: Bundle + 'static>(&self, e: Entity) -> &'w B {
        self.0.get_component::<B>(e).unwrap()
    }

    pub fn get_unchecked_mut<B: Bundle + 'static>(&self, e: Entity) -> Mut<B> {
        self.0.get_component_mut1::<B>(e).unwrap()
    }

    pub fn init_component<B: Bundle + 'static>(&self) -> ComponentIndex {
        self.0.init_component::<B>()
    }
}

impl SystemParam for EntityEditor<'_> {
    type State = ();
    type Item<'w> = EntityEditor<'w>;

    fn init_state(_world: &mut World, _system_meta: &mut SystemMeta) -> Self::State {
        // 如果world上没有找到对应的原型，则创建并放入world中
        ()
    }

    #[inline]
    fn get_param<'world>(
        world: &'world World,
        _system_meta: &'world SystemMeta,
        _state: &'world mut Self::State,
        _tick: Tick,
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
