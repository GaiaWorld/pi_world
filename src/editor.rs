use std::{
    borrow::Cow, fmt::Debug, hash::{DefaultHasher, Hash, Hasher}, mem::transmute
};

use pi_map::{hashmap::HashMap, Map};
use pi_null::Null;

use crate::{
    alter::{AState, ArchetypeMapping}, archetype::{ArchetypeIndex, ArchetypeInfo, Row}, fetch::FetchComponents, filter::FilterComponents, insert::Bundle, prelude::{Entity, Mut, QueryError, Tick, World}, query::{ArchetypeLocalIndex, Queryer}, system::SystemMeta, system_params::SystemParam, world::ComponentIndex
};

impl AState {
    fn insert_columns(&self, am: &mut ArchetypeMapping, dst_row: Row, e: Entity, tick: Tick) {
        for i in am.add_indexs.clone().into_iter() {
            let c = unsafe { self.adding.get_unchecked(i) };
            let dst_column = c.blob_ref_unchecked(am.dst.index());
            // println!("dst_column: {:?}", dst_column.info());
            let dst_data: *mut u8 = unsafe { dst_column.load(dst_row) };
            c.info().default_fn.unwrap()(dst_data);
            dst_column.added_tick(e, dst_row, tick)
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
    fn state(&mut self) -> &mut EditorState {
        &mut self.world.entity_editor_state
    }
    fn _get_entity_prototype(&self, e: Entity) -> Option<(&Cow<'static, str>, ArchetypeIndex)> {
        self.world.get_entity_prototype(e)
    }

    /// 根据组件id列表一次添加或删除多个相应组件(true 为增加， false 为删除)
    pub fn add_components_by_index(
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

    /// 根据组件id列表一次删除多个相应组件
    pub fn remove_components_by_index(
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

    /// 根据组件id列表一次添加或删除多个相应组件(true 为增加， false 为删除)
    pub fn alter_components_by_index(
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

    fn alter_components_impl(& self, e: Entity) -> Result<(), QueryError> {
        let ptr: *const EditorState = &self.world.entity_editor_state;
        let editor_state = unsafe { &mut *(ptr as *mut EditorState) };
        editor_state.tmp.sort_by(|a, b| a.cmp(b)); // 只比较ComponentIndex，并且保持原始顺序的排序

        let mut hasher = DefaultHasher::new();
        editor_state.tmp.hash(&mut hasher);
        let hash = hasher.finish();

        let addr = match self.world.entities.get(e) {
            Some(v) => v,
            None => return Err(QueryError::NoSuchEntity(e)),
        };

        let ar_index = addr.archetype_index();
        let mut ar = &self.world.empty_archetype;

        if !addr.row.is_null() {
            // todo 似乎state.alter_row内判断了，这里可以不用判断和mark_remove
            ar = unsafe { self.world.archetype_arr.get_unchecked(ar_index.index()) };
            let ae = ar.mark_remove(addr.row);
            if e != ae {
                return Err(QueryError::NoMatchEntity(ae));
            }
        }

        let local_index =
            if let Some(local_index) = editor_state.archetype_map.get(&(ar_index, hash)) {
                *local_index
            } else {
                editor_state.vec.push(ArchetypeMapping::new(
                    ar.clone(),
                    self.world.empty_archetype.clone(),
                ));
                let local_index = ArchetypeLocalIndex::from(editor_state.vec.len() - 1);
                editor_state
                    .archetype_map
                    .insert((ar_index, hash), local_index);
                local_index
            };

        let state = if let Some(state) = editor_state.alter_map.get_mut(&hash) {
            state
        } else {
            editor_state
                .alter_map
                .insert(hash, AState::new(editor_state.tmp.clone()));
            editor_state.alter_map.get_mut(&hash).unwrap()
        };

        let mapping = unsafe { editor_state.vec.get_unchecked_mut(local_index.index()) };
        state.find_mapping(&self.world, mapping, true);

        if mapping.dst.id() == mapping.src.id() {
            return Ok(());
        }

        let (_, dst_row) = mapping.dst.alloc();
        // println!("edit: {:?}", (e, addr.row, dst_row, &mapping.dst));

        let tick = self.world.tick();
        // println!("mapping: {}")
        state.insert_columns(mapping, dst_row.into(), e, tick.clone());

        state.alter_row(&self.world, mapping, addr.row, dst_row.into(), e);
        // println!("edit--------: {:?}", (e, addr.row, dst_row, &mapping.dst));
        Ok(())
    }

    /// 根据组件id列表一次插入多个相应组件
    // todo 参数components改为sort_components或&mut自己排序
    pub fn insert_entity_by_index(& self, components: &[ComponentIndex]) -> Result<Entity, QueryError> {
        let components = components
            .iter()
            .map(|index| self.world.get_column(*index).unwrap().clone())
            .collect();
        let info = ArchetypeInfo::sort(components);
        // todo 将Archetype的id改为[ComponentIndex]的hash值，这样尝试获取原型
        let ar = self.world.find_archtype(info);
        let (r, row) = ar.alloc();
        let e = self.world.insert(ar.index(), row.into());
        let tick = self.world.tick();
        // println!("mapping: {}")
        ar.init_row(row.into(), e, tick);
        *r = e;
        Ok(e)
    }
    // todo editer 应该支持Insert的Bundle

    /// 删除实体
    pub fn destroy(&self, e: Entity) -> Result<(), QueryError> {
        let addr = match self.world.entities.get(e) {
            Some(v) => v,
            None => return Err(QueryError::NoSuchEntity(e)),
        };
        if addr.row.is_null() {
            self.world.entities.remove(e).unwrap();
            return Ok(());
        }
        let ar_index = addr.archetype_index();
        let ar = unsafe { self.world.archetype_arr.get_unchecked(ar_index.index()) };

        AState::destroy_row(&self.world, ar, addr.row)?;

        Ok(())
    }

    pub fn alloc_entity(&self) -> Entity {
        self.world.alloc_entity()
    }

    /// 获取组件只读引用
    pub fn get_component<B: Bundle + 'static>(&self, e: Entity) -> Result<&B, QueryError> {
        self.world.get_component::<B>(e)
    }

    /// 获取组件可写引用
    pub fn get_component_mut<B: Bundle + 'static>(
        &mut self,
        e: Entity,
    ) -> Result<Mut<B>, QueryError> {
        self.world.get_component_mut::<B>(e)
    }

    pub fn get_component_unchecked<B: Bundle + 'static>(&self, e: Entity) -> &B {
        self.world.get_component::<B>(e).unwrap()
    }

    pub fn get_component_unchecked_mut<B: Bundle + 'static>(&mut self, e: Entity) -> Mut<B> {
        self.world.get_component_mut::<B>(e).unwrap()
    }

    /// 根据组件id获取组件只读引用（性能相较get_component更好）
    pub fn get_component_by_index<B: Bundle + 'static>(
        &self,
        e: Entity,
        index: ComponentIndex,
    ) -> Result<&B, QueryError> {
        self.world.get_component_by_index::<B>(e, index)
    }

    /// 根据组件id获取组件可写引用（性能相较get_component_mut更好）
    pub fn get_component_mut_by_index<B: Bundle + 'static>(
        &mut self,
        e: Entity,
        index: ComponentIndex,
    ) -> Result<Mut<B>, QueryError> {
        self.world.get_component_mut_by_index(e, index)
    }

    pub fn get_component_unchecked_by_index<B: Bundle + 'static>(
        &self,
        e: Entity,
        index: ComponentIndex,
    ) -> &B {
        self.world.get_component_by_index::<B>(e, index).unwrap()
    }

    pub fn get_component_unchecked_mut_by_index<B: Bundle + 'static>(
        &mut self,
        e: Entity,
        index: ComponentIndex,
    ) -> Mut<B> {
        self.world.get_component_mut_by_index(e, index).unwrap()
    }

    /// 获取组件id
    pub fn init_component<B: Bundle + 'static>(&mut self) -> ComponentIndex {
        self.world.init_component::<B>()
    }

    /// 是否包含实体
    pub fn contains_entity(&self, e: Entity) -> bool {
        self.world.contains_entity(e)
    }

    /// 添加多个组件
    pub fn add_components<B: Bundle + 'static>(
        &mut self,
        e: Entity,
        components: B,
    ) -> Result<(), QueryError> {
        // B::add_components(self, e, components)
        self.world.make_alterer::<(), (), B, ()>().alter(e, components)?;
        Ok(())
    }

    /// 插入多个组件，返回对应的实体
    pub fn insert_entity<B: Bundle + 'static>(
        &mut self,
        components: B,
    ) -> Entity {
        self.world.make_inserter().insert(components)
        // B::insert_components(self,  components)
    }

     /// 创建一个查询器
    pub fn make_queryer<Q: FetchComponents + 'static, F: FilterComponents + 'static = ()>(
        &mut self,
    )-> Queryer<Q, F> {
         self.world.make_queryer::<Q, F>()
    }
}

#[derive(Default)]
pub(crate) struct EditorState {
    alter_map: HashMap<u64, AState>, // sorted_add_removes的hash值
    archetype_map: HashMap<(ArchetypeIndex, u64), ArchetypeLocalIndex>, // (原型id和sorted_add_removes的hash值)为键, 值为State.vec的索引
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
