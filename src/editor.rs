use std::{
    fmt::Debug,
    hash::{DefaultHasher, Hash, Hasher},
    mem::transmute,
};

use pi_map::{hashmap::HashMap, Map};
use pi_null::Null;

use crate::{
    alter::{ArchetypeMapping, State},
    archetype::{ArchetypeWorldIndex, Row},
    insert::Bundle,
    prelude::{Entity, Mut, QueryError, Tick, World},
    query::ArchetypeLocalIndex,
    system::SystemMeta,
    system_params::SystemParam,
    world::ComponentIndex,
};

impl State {
    fn insert_columns(&self, am: &mut ArchetypeMapping, dst_row: Row, e: Entity, tick: Tick) {
        for i in am.add_indexs.clone().into_iter() {
            let (_, dst_i) = unsafe { self.adding.get_unchecked(i) };
            let dst_column = am.dst.get_column_unchecked(*dst_i);
            let dst_data: *mut u8 = dst_column.load(dst_row);
            dst_column.info().default_fn.unwrap()(dst_data);
            dst_column.add_record(e, dst_row, tick)
        }
    }
}
pub struct EntityEditor<'w> {
    world: &'w mut World,
}

impl<'w> EntityEditor<'w> {
    pub fn new(world: &'w mut World) -> Self {
        Self { world }
    }

    fn state(&mut self)-> &mut EditorState{
        &mut self.world.entity_editor_state
    }
    pub fn add_components(
        &mut self,
        e: Entity,
        components: &[ComponentIndex],
    ) -> Result<(), QueryError> {
        self.state().tmp.clear();
        for item in components.iter().rev() {
            self.state().tmp.push((*item, true));
        }
        self.alter_components_impl(e)
    }

    pub fn remove_components(
        &mut self,
        e: Entity,
        components: &[ComponentIndex],
    ) -> Result<(), QueryError> {
        self.state().tmp.clear();
        for item in components.iter().rev() {
            self.state().tmp.push((*item, false));
        }
        self.alter_components_impl(e)
    }

    pub fn alter_components(
        &mut self,
        e: Entity,
        components: &[(ComponentIndex, bool)],
    ) -> Result<(), QueryError> {
        self.state().tmp.clear();
        for item in components.iter().rev() {
            self.state().tmp.push(*item)
        }
        // components.reverse(); // 相同ComponentIndex的多个增删操作，让最后的操作执行
        self.alter_components_impl(e)
    }

    fn alter_components_impl(&mut self, e: Entity) -> Result<(), QueryError> {
        let ptr: *const EditorState = &self.world.entity_editor_state;
        let editor_state = unsafe { &mut *(ptr as *mut EditorState) };
        editor_state.tmp.sort_by(|a, b| a.cmp(b)); // 只比较ComponentIndex，并且保持原始顺序的排序

        let mut hasher = DefaultHasher::new();
        editor_state.tmp.hash(&mut hasher);
        let hash = hasher.finish();

        let addr = match self.world.entities.get(e) {
            Some(v) => v,
            None => return Err(QueryError::NoSuchEntity),
        };

        let ar_index = addr.archetype_index();
        let mut ar = &self.world.empty_archetype;

        if !ar_index.is_null() {
            ar = unsafe { self.world.archetype_arr.get_unchecked(ar_index.index()) };
            let ae = ar.mark_remove(addr.row);
            if e != ae {
                return Err(QueryError::NoMatchEntity(ae));
            }
        }

        let local_index = if let Some(local_index) = editor_state.archetype_map.get(&(ar_index, hash))
        {
            *local_index
        } else {
            editor_state.vec.push(ArchetypeMapping::new(
                ar.clone(),
                self.world.empty_archetype.clone(),
            ));
            let local_index = ArchetypeLocalIndex::from(editor_state.vec.len() - 1);
            editor_state.archetype_map
                .insert((ar_index, hash), local_index);
            local_index
        };

        let state = if let Some(state) = editor_state.alter_map.get_mut(&hash) {
            state
        } else {
            editor_state
                .alter_map
                .insert(hash, State::new(editor_state.tmp.clone()));
            editor_state.alter_map.get_mut(&hash).unwrap()
        };

        let mapping = unsafe { editor_state.vec.get_unchecked_mut(local_index.index()) };
        state.find_mapping(&self.world, mapping, true);

        let dst_row = mapping.dst.alloc();

        let tick = self.world.tick();

        state.insert_columns(mapping, dst_row, e, tick.clone());

        state.alter_row(&self.world, mapping, addr.row, dst_row, e, tick);

        Ok(())
    }

    pub fn insert_components(
        &mut self,
        components: &[ComponentIndex],
    ) -> Result<Entity, QueryError> {
        let e = self.world.alloc_entity();
        // todo 应该优化一下，遍历时算id，然后在world上查找原型，没有找到再用world.find_ar来创建原型，不应该调用alter_components_impl, alter_components_impl内的ar_index判断也可以去掉
        self.state().tmp.clear();
        for item in components.iter().rev() {
            self.state().tmp.push((*item, true))
        }

        self.alter_components_impl(e)?;
        Ok(e)
    }

    pub fn destroy(&self, e: Entity) -> Result<(), QueryError> {
        let addr = match self.world.entities.get(e) {
            Some(v) => v,
            None => return Err(QueryError::NoSuchEntity),
        };

        let ar_index = addr.archetype_index();
        let ar = unsafe { self.world.archetype_arr.get_unchecked(ar_index.index()) };

        State::destroy_row(&self.world, ar, addr.row)?;

        Ok(())
    }

    pub fn alloc(&self) -> Entity {
        self.world.alloc_entity()
    }

    pub fn get_component<B: Bundle + 'static>(&self, e: Entity) -> Result<&B, QueryError> {
        self.world.get_component::<B>(e)
    }

    pub fn get_component_mut<B: Bundle + 'static>(&mut self, e: Entity) -> Result<Mut<B>, QueryError> {
        self.world.get_component_mut1::<B>(e)
    }

    pub fn get_component_unchecked<B: Bundle + 'static>(&self, e: Entity) -> &B {
        self.world.get_component::<B>(e).unwrap()
    }

    pub fn get_component_unchecked_mut<B: Bundle + 'static>(&mut self, e: Entity) -> Mut<B> {
        self.world.get_component_mut1::<B>(e).unwrap()
    }

    pub fn get_component_by_index<B: Bundle + 'static>(&self, e: Entity, index: ComponentIndex) -> Result<&B, QueryError> {
        self.world.get_component_by_index::<B>(e, index)
    }

    pub fn get_component_mut_by_id<B: Bundle + 'static>(&mut self, e: Entity, index: ComponentIndex) -> Result<Mut<B>, QueryError> {
        self.world.get_component_by_index_mut(e, index)
    }

    pub fn get_component_unchecked_by_index<B: Bundle + 'static>(&self, e: Entity, index: ComponentIndex) ->&B { 
        self.world.get_component_by_index::<B>(e, index).unwrap()
    }

    pub fn get_component_unchecked_mut_by_id<B: Bundle + 'static>(&mut self, e: Entity, index: ComponentIndex) -> Mut<B> {
        self.world.get_component_by_index_mut(e, index).unwrap()
    }

    pub fn init_component<B: Bundle + 'static>(&self) -> ComponentIndex {
        self.world.init_component::<B>()
    }
}

#[derive(Default)]
pub struct EditorState {
    alter_map: HashMap<u64, State>, // sorted_add_removes的hash值
    archetype_map: HashMap<(ArchetypeWorldIndex, u64), ArchetypeLocalIndex>, // (原型id和sorted_add_removes的hash值)为键, 值为State.vec的索引
    vec: Vec<ArchetypeMapping>,
    tmp: Vec<(ComponentIndex, bool)>,
}

impl Debug for EditorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorState")
            .field("vec", &self.vec)
            .field("tmp", &self.tmp)
            .finish()
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
        let ptr: *const World = world;
        let world = unsafe { &mut *(ptr as *mut World) }; 
        world.make_entity_editor()
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
