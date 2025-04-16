use std::{
    borrow::Cow, fmt::Debug, hash::{DefaultHasher, Hash, Hasher}, mem::transmute
};

use pi_map::{hashmap::HashMap, Map};
use pi_null::Null;

use crate::{
    alter::{AState, ArchetypeMapping, QueryAlterState}, archetype::{ArchetypeIndex, ArchetypeInfo, Row}, fetch::FetchComponents, filter::FilterComponents, insert::{Bundle, InsertState}, prelude::{Entity, Mut, QueryError, Tick, World}, query::{LocalIndex, QueryState}, system::SystemMeta, system_params::SystemParam, world::ComponentIndex, world_ptr::Ptr
};

impl AState {
    fn insert_columns(&mut self, world: &mut World, am: &mut ArchetypeMapping, dst_row: Row, e: Entity, tick: Tick) {
        for i in am.add_indexs.clone().into_iter() {
            let c = unsafe { self.adding.get_unchecked(i) };
            let dst_column = c.blob_ref_unchecked(am.dst.index());
            // println!("dst_column: {:?}", dst_column.info());
            let dst_data: *mut u8 = dst_column.load(dst_row, e);
            match c.info().set_fn {
                Some(fun) => fun(world, dst_data),
                None => {
                    log::error!("{:?} is not set_fn!!!", (c, &dst_column));
                    panic!("{:?} is not set_fn!!!", (c, dst_column))
                },
            };
            dst_column.added_tick(e, dst_row, tick)
        }
    }
}

// pub type EntityEditor<'w> = &'w mut EntityEditor<'w>;
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
        println!("add_components_by_index: {:?}", e);
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
        println!("remove_components_by_index: {:?}", e);
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

    fn alter_components_impl(&mut self, e: Entity) -> Result<(), QueryError> {
        let ptr: *const EditorState = &self.world.entity_editor_state;
        let editor_state = unsafe { &mut *(ptr as *mut EditorState) };
        editor_state.tmp.sort_by(|a, b| a.cmp(b)); // 只比较ComponentIndex，并且保持原始顺序的排序

        let mut hasher = DefaultHasher::new();
        editor_state.tmp.hash(&mut hasher);
        let hash = hasher.finish();

        let addr = match self.world.entities.get(e) {
            Some(v) => *v,
            None => return Err(QueryError::NoSuchEntity(e)),
        };

        let ar_index = addr.archetype_index();
        let ar = unsafe { self.world.archetype_arr.get_unchecked(ar_index.index()) };

        let local_index =
            if let Some(local_index) = editor_state.archetype_map.get(&(ar_index, hash)) {
                *local_index
            } else {
                editor_state.vec.push(ArchetypeMapping::new(
                    ar.clone(),
                    self.world.empty_archetype.clone(),
                ));
                let local_index = LocalIndex::from(editor_state.vec.len() - 1);
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
        // println!("edit1: {:?}", (e, addr, &mapping.src.index, &mapping.dst_index));
        if mapping.dst.id() == mapping.src.id() {
            return Ok(());
        }

        let (_, dst_row) = mapping.dst.alloc();
        // println!("edit2: {:?}", (e, addr.row, dst_row, &mapping.dst_index));

        let tick = self.world.tick();
        // println!("mapping: {}")
        state.insert_columns(self.world, mapping, dst_row.into(), e, tick.clone());

        state.alter_row(&self.world, mapping, addr.row, dst_row.into(), e);
        // println!("edit--------: {:?}", (e, addr.row, dst_row, &mapping.dst));
        Ok(())
    }

    /// 根据组件id列表一次插入多个相应组件
    // todo 参数components改为sort_components或&mut自己排序
    pub fn insert_entity_by_index(&mut self, components: &[ComponentIndex]) -> Result<Entity, QueryError> {
        let components = components
            .iter()
            .map(|index| self.world.get_column(*index).unwrap().clone())
            .collect();
        let info = ArchetypeInfo::sort(components);
        // todo 将Archetype的id改为[ComponentIndex]的hash值，这样尝试获取原型
        let ar = self.world.find_archtype(info);
        let (r, row) = ar.alloc();
        let e = self.world.insert_addr(ar.index(), row.into());
        let tick = self.world.tick();
        // println!("mapping: {}")
        ar.init_row(self.world, row.into(), e, tick);
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
        self.world.spawn_empty()
    }

    /// 获取组件只读引用
    pub fn get_component<T: 'static>(&self, e: Entity) -> Result<&T, QueryError> {
        self.world.get_component::<T>(e)
    }

    /// 获取组件可写引用
    pub fn get_component_mut<T: 'static>(
        &mut self,
        e: Entity,
    ) -> Result<Mut<T>, QueryError> {
        self.world.get_component_mut::<T>(e)
    }

    pub fn get_component_unchecked<T: 'static>(&self, e: Entity) -> &T {
        self.world.get_component::<T>(e).unwrap()
    }

    pub fn get_component_unchecked_mut<T: 'static>(&mut self, e: Entity) -> Mut<T> {
        self.world.get_component_mut::<T>(e).unwrap()
    }

    /// 根据组件id获取组件只读引用（性能相较get_component更好）
    pub fn get_component_by_index<T: 'static>(
        &self,
        e: Entity,
        index: ComponentIndex,
    ) -> Result<&T, QueryError> {
        self.world.get_component_by_index::<T>(e, index)
    }

    /// 根据组件id获取组件可写引用（性能相较get_component_mut更好）
    pub fn get_component_mut_by_index<T: 'static>(
        &mut self,
        e: Entity,
        index: ComponentIndex,
    ) -> Result<Mut<T>, QueryError> {
        self.world.get_component_mut_by_index(e, index)
    }

    pub fn get_component_unchecked_by_index<T: 'static>(
        &self,
        e: Entity,
        index: ComponentIndex,
    ) -> &T {
        self.world.get_component_by_index::<T>(e, index).unwrap()
    }

    pub fn get_component_unchecked_mut_by_index<T: 'static>(
        &mut self,
        e: Entity,
        index: ComponentIndex,
    ) -> Mut<T> {
        self.world.get_component_mut_by_index(e, index).unwrap()
    }

    /// 获取组件id
    pub fn init_component<T: 'static>(&mut self) -> ComponentIndex {
        self.world.init_component::<T>()
    }

    /// 是否包含实体
    pub fn contains_entity(&self, e: Entity) -> bool {
        self.world.contains_entity(e)
    }

    /// 添加多个组件 todo 改成add_bundle
    pub fn add_components<B: Bundle + 'static>(
        &mut self,
        e: Entity,
        components: B,
    ) -> Result<(), QueryError> {
        self.world.make_alter::<(), (), B, ()>().get_param(self.world).alter(e, components)?;
        Ok(())
    }

    /// 插入多个组件，返回对应的实体
    pub fn insert_entity<B: Bundle + 'static>(
        &mut self,
        components: B,
    ) -> Entity {
        self.world.make_insert().insert(self.world, components)
    }

    /// 创建一个插入器
    pub fn make_insert<B: Bundle + 'static>(
        &mut self,
    ) -> InsertState<B> {
        self.world.make_insert::<B>()
    }

     /// 创建一个查询器
    pub fn make_query<Q: FetchComponents + 'static, F: FilterComponents + 'static = ()>(
        &mut self,
    )-> QueryState<Q, F> {
         self.world.make_query::<Q, F>()
    }
    /// 创建一个改变器
    pub fn make_alter<
        Q: FetchComponents + 'static,
        F: FilterComponents + 'static,
        A: Bundle + 'static,
        D: Bundle + 'static,
    >(
        &mut self,
    ) -> QueryAlterState<Q, F, A, D> {
        self.world.make_alter::<Q, F, A, D>()
    }    

}

#[derive(Default)]
pub(crate) struct EditorState {
    alter_map: HashMap<u64, AState>, // sorted_add_removes的hash值
    archetype_map: HashMap<(ArchetypeIndex, u64), LocalIndex>, // (原型id和sorted_add_removes的hash值)为键, 值为State.vec的索引
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
    type State = Ptr<World>;
    type Item<'w> = EntityEditor<'w>;

    fn init_state(world: &mut World, meta: &mut SystemMeta) -> Self::State {
        meta.relate(crate::system::Relation::WriteAll);
        meta.related_ok();
        Ptr::new(world)
    }

    #[inline]
    fn get_param<'world>(
        // world: &'world World,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        state.make_entity_editor()
    }
    #[inline]
    fn get_self<'world>(
        // world: &'world World,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(state)) }
    }
}
