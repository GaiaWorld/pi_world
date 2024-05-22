use core::hash;
use std::{hash::{DefaultHasher, Hash, Hasher}, mem::{transmute, ManuallyDrop}};

use pi_map::{hashmap::HashMap, Map};
use pi_null::Null;

use crate::{
    alter::{ArchetypeMapping, State}, insert::Bundle, prelude::{Entity, Mut, QueryError, Tick, World}, query::ArchetypeLocalIndex, system::SystemMeta, system_params::SystemParam, world::ComponentIndex
};

pub struct EntityEditor<'w> {
    alter_map: HashMap<u64, State>, // sorted_add_removes的hash值
    archetype_map: HashMap<(u32, u64), ArchetypeLocalIndex>, // (原型id和sorted_add_removes的hash值)为键, 值为State.vec的索引
    world: ManuallyDrop<&'w mut World>,
    vec: Vec<ArchetypeMapping>,
    tmp: Vec<(ComponentIndex, bool)>,
}

impl<'w> EntityEditor<'w> {
    pub fn new(world: ManuallyDrop<&'w mut World>) -> Self {
        Self{
            alter_map: HashMap::default(),
            archetype_map: HashMap::default(),
            world,
            vec: Vec::new(),
            tmp: Vec::new(),
        }
    }

    pub fn add_components(
        &mut self,
        e: Entity,
        components: &[ComponentIndex],
    ) -> Result<(), QueryError> {
        let mut components = components
            .iter()
            .filter_map(|v| Some((*v, true)))
            .collect::<Vec<(ComponentIndex, bool)>>();

        self.world.alter_components(e, &mut components)
    }

    pub fn remove_components(
        &mut self,
        e: Entity,
        components: &[ComponentIndex],
    ) -> Result<(), QueryError> {
        let mut components = components
            .iter()
            .filter_map(|v| Some((*v, false)))
            .collect::<Vec<(ComponentIndex, bool)>>();

        self.world.alter_components(e, &mut components)
    }

    pub fn alter_components(
        &mut self,
        e: Entity,
        components: &[(ComponentIndex, bool)],
    ) -> Result<(), QueryError> {
        self.tmp.clear();
        for item in components.iter().rev(){
            self.tmp.push(*item)
        }
        // components.reverse(); // 相同ComponentIndex的多个增删操作，让最后的操作执行
        self.tmp.sort_by(|a, b| a.cmp(b)); // 只比较ComponentIndex，并且保持原始顺序的排序
        
        let mut hasher = DefaultHasher::new();
        self.tmp.hash(&mut hasher);
        let hash = hasher.finish() ;
        
        let addr = match self.world.entities.get(e) {
            Some(v) => v,
            None => return Err(QueryError::NoSuchEntity),
        };
        
        let ar_index = addr.archetype_index();
        let mut ar = self.world.empty_archetype();

        if !addr.index.is_null() {
            ar = unsafe { self.world.archetype_arr.get_unchecked(ar_index as usize)};
            let ae = ar.mark_remove(addr.row);
            if e != ae {
                return Err(QueryError::NoMatchEntity(ae));
            }
        }

        let local_index = if let Some(local_index ) = self.archetype_map.get(&(ar_index, hash)){
            *local_index
        }else{
            self.vec.push(ArchetypeMapping::new(ar.clone(), self.world.empty_archetype().clone()));
            let local_index = ArchetypeLocalIndex::from(self.vec.len() - 1);
            self.archetype_map.insert((ar_index, hash), local_index);
            local_index
        };

        let state = if let Some(state) = self.alter_map.get_mut(&hash){
            state
        }else{
            self.alter_map.insert(hash, State::new(self.tmp.clone()));
            self.alter_map.get_mut(&hash).unwrap()
        };

        let mapping = unsafe { self.vec.get_unchecked_mut(local_index.index()) };
        state.find_mapping(&self.world, mapping, true);

        let dst_row = mapping.dst.alloc();
        state.alter_row(&self.world, mapping, addr.row, dst_row, e, self.world.tick());

        Ok(())
    }

    pub fn insert_components(&mut self, components: &[ComponentIndex]) -> Result<Entity, QueryError> {
        let e = self.world.alloc_entity();
        let mut components = components
            .iter()
            .filter_map(|v| Some((*v, true)))
            .collect::<Vec<(ComponentIndex, bool)>>();
        // self.0.i.alter_components(e, components)
        self.alter_components(e, &mut components)?;
        Ok(e)
    }

    pub fn destroy(&self, e: Entity) -> Result<(), QueryError> {
        self.world.destroy_entity2(e)
    }

    pub fn alloc(&self) -> Entity {
        self.world.alloc_entity()
    }

    pub fn get<B: Bundle + 'static>(&self, e: Entity) -> Result<&B, QueryError> {
        self.world.get_component::<B>(e)
    }

    pub fn get_mut<B: Bundle + 'static>(&mut self, e: Entity) -> Result<Mut<B>, QueryError> {
        self.world.get_component_mut1::<B>(e)
    }

    pub fn get_unchecked<B: Bundle + 'static>(&'w self, e: Entity) -> &'w B {
        self.world.get_component::<B>(e).unwrap()
    }

    pub fn get_unchecked_mut<B: Bundle + 'static>(&mut self, e: Entity) -> Mut<B> {
        self.world.get_component_mut1::<B>(e).unwrap()
    }

    pub fn init_component<B: Bundle + 'static>(&self) -> ComponentIndex {
        self.world.init_component::<B>()
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
        EntityEditor::new(world.unsafe_world())
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
