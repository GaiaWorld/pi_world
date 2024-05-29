/// 组件变更，增加和移除组件，及销毁entity
/// 如果原型上没有要移除的组件，则自动忽略
/// 如果要增加的组件在源原型上存在，则直接替换
/// 和Query一样，可以get和iter，如果该原型可以查到，则会记录在本地源原型列表中
/// 要求entity的原型，必须在本地源原型列表中，这样保证有正确的写依赖
/// 根据变更时entity的原型，动态查找和创建移除后对应的原型
/// 变更组件会在world上EntityAddr进行标记，不在源原型上标记，这样system后续迭代和查找是可以找到并且操作。
/// 因为跟踪成本过高，不支持在system内多次变更。
/// 目标原型会立即添加新的条目，但该条目的entity是null，表示是看不到的。这时添加的组件，会立即写到目标原型对应的位置，等变更结束时，会将标记的entity，誊写到目标原型上，并修改entity。
/// 目标原型有可能和本system的原型重合，则立地修改。因为使用写引用，并且SystemParam之间有读写冲突检查，所以不会有引用被改写的问题。
///
/// 使用执行图来保证，alter不会操作到正在读写的原型。 这样就不用处理各种多线程数据不一致的情况， 比如 entity 读有值， 但组件没有读到正确数据。
/// 每个system的run会根据依赖是否全部写完毕才开始执行，对应的就是一个ShareU32的wait_count数字减到0。
/// 每个system有自身状态ShareU8的run_state，初值为wait。system的run会先执行before，before会先同步world上原型数组的长度，同步后，修改自身状态ShareU8为running，执行完毕后改为ok。
/// 执行图在执行中， 收到A某system产生的原型创建的事件，此时该原型还为放入world上原型数组， 用该原型去匹配所有的system，返回：无依赖、读、写、目标写原型（alter会根据源原型写到目标原型，该目标原型也需要纳入写）。
/// 如果有依赖，则立即将wait_count加1，如果原wait_count=0表示已经开始执行，那么循环等待该system的run_state为running或ok。 这样，对该system要么看到该原型，要么看不到。
///
use std::any::TypeId;
use std::borrow::Cow;
use std::marker::PhantomData;
use std::mem::{transmute, MaybeUninit};
use std::ops::{Deref, DerefMut, Range};

use pi_null::Null;
use pi_share::Share;

use crate::archetype::*;
use crate::column::Column;
use crate::fetch::FetchComponents;
use crate::filter::FilterComponents;
use crate::insert::Bundle;
use crate::param_set::ParamSetElement;
use crate::query::{check, ArchetypeLocalIndex, Query, QueryError, QueryIter, QueryState, Queryer};
use crate::event::ComponentRemovedRecord;
use crate::system::SystemMeta;
use crate::system_params::SystemParam;
use crate::world::*;

pub struct Alterer<
    'world,
    Q: FetchComponents + 'static,
    F: FilterComponents + 'static = (),
    A: Bundle + 'static = (),
    D: Bundle + 'static = (),
> {
    query: Queryer<'world, Q, F>,
    state: AlterState<A>,
    _k: PhantomData<D>,
}
impl<'world, Q: FetchComponents + 'static, F: FilterComponents + 'static, A: Bundle, D: Bundle>
    Alterer<'world, Q, F, A, D>
{
    // 将新多出来的原型，创建原型空映射
    pub(crate) fn state_align(
        world: &World,
        state: &mut AlterState<A>,
        query_state: &QueryState<Q, F>,
    ) {
        // 将新多出来的原型，创建原型空映射
        for i in state.vec.len()..query_state.vec.len() {
            let ar = unsafe { query_state.vec.get_unchecked(i).ar.clone() };
            state.push_archetype(ar, world);
        }
    }
    pub(crate) fn new(
        world: &'world World,
        query_state: QueryState<Q, F>,
        state: AlterState<A>,
    ) -> Self {
        Self {
            query: Queryer::new(world, query_state),
            state,
            _k: PhantomData,
        }
    }

    pub fn contains(&self, entity: Entity) -> bool {
        self.query.contains(entity)
    }

    pub fn get(
        &'world self,
        e: Entity,
    ) -> Result<<<Q as FetchComponents>::ReadOnly as FetchComponents>::Item<'world>, QueryError>
    {
        self.query.get(e)
    }

    pub fn get_mut(&mut self, e: Entity) -> Result<<Q as FetchComponents>::Item<'_>, QueryError> {
        self.query.get_mut(e)
    }

    pub fn is_empty(&self) -> bool {
        self.query.is_empty()
    }

    pub fn len(&self) -> usize {
        self.query.len()
    }

    pub fn iter(&self) -> QueryIter<'_, <Q as FetchComponents>::ReadOnly, F> {
        self.query.iter()
    }

    pub fn iter_mut(&mut self) -> AlterIter<'_, Q, F, A> {
        AlterIter {
            it: self.query.iter_mut(),
            state: &mut self.state,
        }
    }
    /// 标记销毁实体

    pub fn destroy(&mut self, e: Entity) -> Result<bool, QueryError> {
        if e.is_null() {
            return Err(QueryError::NullEntity);
        }
        // self.state
        AState::destroy(&self.query.world, &self.state.vec, e, &self.query.state.map)
    }

    pub fn alter(&mut self, e: Entity, components: A) -> Result<bool, QueryError> {
        if e.is_null() {
            return Err(QueryError::NullEntity);
        }
        let (addr, local_index) = check_mark(
            &self.query.world,
            e,
            &self.query.state.map,
        )?;
        self.state.alter(
            &self.query.world,
            local_index,
            e,
            addr.row,
            components,
            self.query.tick,
        );
        Ok(true)
    }
}
pub struct Alter<
    'world,
    Q: FetchComponents + 'static,
    F: FilterComponents + 'static = (),
    A: Bundle + 'static = (),
    D: Bundle + 'static = (),
> {
    query: Query<'world, Q, F>,
    state: &'world mut AlterState<A>,
    _k: PhantomData<D>,
}

unsafe impl<'world, Q: FetchComponents + 'static, F: FilterComponents + 'static, A: Bundle, D: Bundle>
    Send for Alter<'world, Q, F, A, D>
{
}
unsafe impl<'world, Q: FetchComponents + 'static, F: FilterComponents + 'static, A: Bundle, D: Bundle>
    Sync for Alter<'world, Q, F, A, D>
{
}

impl<'world, Q: FetchComponents + 'static, F: FilterComponents + 'static, A: Bundle, D: Bundle>
    Alter<'world, Q, F, A, D>
{
    pub(crate) fn new(query: Query<'world, Q, F>, state: &'world mut AlterState<A>) -> Self {
        Alter {
            query,
            state,
            _k: PhantomData,
        }
    }

    pub fn contains(&self, entity: Entity) -> bool {
        self.query.contains(entity)
    }

    pub fn get<'a>(
        &'a self,
        e: Entity,
    ) -> Result<<<Q as FetchComponents>::ReadOnly as FetchComponents>::Item<'a>, QueryError> {
        self.query.get(e)
    }

    pub fn get_mut<'a>(
        &'a mut self,
        e: Entity,
    ) -> Result<<Q as FetchComponents>::Item<'a>, QueryError> {
        self.query.get_mut(e)
    }

    pub fn is_empty(&self) -> bool {
        self.query.is_empty()
    }

    pub fn len(&self) -> usize {
        self.query.len()
    }

    pub fn iter(&self) -> QueryIter<'_, <Q as FetchComponents>::ReadOnly, F> {
        self.query.iter()
    }

    pub fn iter_mut(&mut self) -> AlterIter<'_, Q, F, A> {
        AlterIter {
            it: self.query.iter_mut(),
            state: &mut self.state,
        }
    }
    /// 标记销毁实体

    pub fn destroy(&mut self, e: Entity) -> Result<bool, QueryError> {
        // self.state
        AState::destroy(&self.query.world, &self.state.vec, e, &self.query.state.map)
    }

    pub fn alter(&mut self, e: Entity, components: A) -> Result<bool, QueryError> {
        let (addr, local_index) = check_mark(&self.query.world, e, &self.query.state.map)?;
        self.state.alter(
            &self.query.world,
            local_index,
            e,
            addr.row,
            components,
            self.query.tick,
        );
        Ok(true)
    }
}

// 检查entity是否正确，包括对应的原型是否在本查询内，并判断标记和设置标记
pub(crate) fn check_mark<'w>(
    world: &'w World,
    entity: Entity,
    map: &Vec<ArchetypeLocalIndex>,
) -> Result<(EntityAddr, ArchetypeLocalIndex), QueryError> {
    // assert!(!entity.is_null());
    match world.entities.load(entity) {
        Some(addr) => match map.get(addr.archetype_index().index()) {
            Some(local_index) => {
                if local_index.is_null() {
                    Err(QueryError::NoMatchArchetype)
                } else {
                    if addr.is_mark() {
                        Err(QueryError::RepeatAlter)
                    } else {
                        addr.mark();
                        Ok((*addr, *local_index))
                    }
                }
            }
            None => Err(QueryError::NoMatchArchetype),
        },
        None => Err(QueryError::NoSuchEntity),
    }
}

impl<
        Q: FetchComponents + 'static,
        F: FilterComponents + Send + Sync + 'static,
        A: Bundle + 'static,
        D: Bundle + Send + 'static,
    > SystemParam for Alter<'_, Q, F, A, D>
{
    type State = (QueryState<Q, F>, AlterState<A>);
    type Item<'w> = Alter<'w, Q, F, A, D>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let q = Query::init_state(world, system_meta);
        (
            q,
            AlterState::make(world, A::components(Vec::new()), D::components(Vec::new())),
        )
    }
    fn archetype_depend(
        world: &World,
        _system_meta: &SystemMeta,
        state: &Self::State,
        archetype: &Archetype,
        result: &mut ArchetypeDependResult,
    ) {
        Q::archetype_depend(world, archetype, result);
        // 如果相关， 则添加移除类型，并返回Alter后的原型id
        if result.flag.bits() > 0 && !result.flag.contains(Flags::WITHOUT) {
            result.merge(ArchetypeDepend::Flag(Flags::WRITE));
            let info = archetype.alter1(
                world,
                &state.1.sorted_add_removes,
                &mut Vec::new(),
                &mut Vec::new(),
                &mut Vec::new(),
                false,
            );
            if archetype.id() != &info.id {
                result.merge(ArchetypeDepend::Alter((
                    info.id,
                    info.name(),
                    info.sorted_components,
                )));
            }
        }
    }
    fn res_depend(
        _world: &World,
        _system_meta: &SystemMeta,
        _state: &Self::State,
        res_tid: &TypeId,
        res_name: &Cow<'static, str>,
        single: bool,
        result: &mut Flags,
    ) {
        Q::res_depend(res_tid, res_name, single, result);
    }

    fn align(world: &World, _system_meta: &SystemMeta, state: &mut Self::State) {
        state.0.align(world);
    }

    fn get_param<'world>(
        world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self::Item<'world> {
        // 将新多出来的原型，创建原型空映射
        Alterer::<Q, F, A, D>::state_align(world, &mut state.1, &state.0);
        Alter::new(Query::new(world, &mut state.0, tick), &mut state.1)
    }

    fn get_self<'world>(
        world: &'world World,
        system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
        tick: Tick,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, system_meta, state, tick)) }
    }
}
impl<
        Q: FetchComponents + 'static,
        F: FilterComponents + Send + Sync,
        A: Bundle + 'static,
        D: Bundle + Send + 'static,
    > ParamSetElement for Alter<'_, Q, F, A, D>
{
    fn init_set_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        system_meta.param_set_check();
        let q = QueryState::create(world);
        (
            q,
            AlterState::make(world, A::components(Vec::new()), D::components(Vec::new())),
        )
    }
}
impl<
        'world,
        Q: FetchComponents + 'static,
        F: FilterComponents + 'static,
        A: Bundle + 'static,
        D: Bundle + 'static,
    > Drop for Alter<'world, Q, F, A, D>
{
    fn drop(&mut self) {
        self.state.state.clear(
            self.query.world,
            &mut self.state.vec,
            &mut self.state.mapping_dirtys,
        );
    }
}
#[derive(Debug)]
pub struct ArchetypeMapping {
    pub(crate) src: ShareArchetype,               // 源原型
    pub(crate) dst: ShareArchetype,               // 映射到的目标原型
    pub(crate) dst_index: ArchetypeWorldIndex,    // 目标原型在World原型数组中的位置
    pub(crate) add_indexs: Range<usize>,          // 目标原型上新增的组件的起始和结束位置
    pub(crate) move_indexs: Range<usize>,         // 源原型和目标原型的组件映射的起始和结束位置
    pub(crate) removed_indexs: Range<usize>,      // 源原型上被移除的组件的起始和结束位置
    // pub(crate) move_removed_indexs: Range<usize>, // 源原型上被移除的组件的起始和结束位置
    pub(crate) moves: Vec<(Row, Row, Entity)>,    // 本次标记移动的条目
}

impl ArchetypeMapping {
    pub fn new(src: ShareArchetype, dst: ShareArchetype) -> Self {
        ArchetypeMapping {
            src,
            dst,
            dst_index: ArchetypeWorldIndex::null(),
            move_indexs: 0..0,
            add_indexs: 0..0,
            removed_indexs: 0..0,
            // move_removed_indexs: 0..0,
            moves: Default::default(),
        }
    }
    pub(crate) fn push(
        &mut self,
        src_row: Row,
        dst_row: Row,
        e: Entity,
        ar_index: ArchetypeLocalIndex,
        mapping_dirtys: &mut Vec<ArchetypeLocalIndex>,
    ) {
        self.moves.push((src_row, dst_row, e));
        // 目标原型和源原型不同，需要移动数据
        if self.moves.len() == 1 {
            // 如果该映射首次移动数据，则需要记录到映射脏上
            mapping_dirtys.push(ar_index);
        }
    }
    pub(crate) fn move_columns(
        &self,
        src_row: Row,
        dst_row: Row,
        e: Entity,
        moving: &Vec<(ComponentIndex, ColumnIndex, ColumnIndex)>,
    ) {
        for index in self.move_indexs.clone() {
            let (_, src_index, dst_index) = unsafe { moving.get_unchecked(index) };
            let src_column = self.src.get_column_unchecked(*src_index);
            let dst_column = self.dst.get_column_unchecked(*dst_index);
            self.move_column(src_row, dst_row, e, src_column, dst_column);
        }
    }
    // 将源组件移动到新位置上
    pub(crate) fn move_column(
        &self,
        src_row: Row,
        dst_row: Row,
        e: Entity,
        src_column: &Column,
        dst_column: &Column,
    ) {
        let src_data: *mut u8 = src_column.get_row(src_row);
        dst_column.write_row(dst_row, src_data);
        if src_column.info().is_tick() {
            let tick = src_column.get_tick_unchecked(src_row);
            dst_column.add_record_unchecked(e, dst_row, tick);
        }
    }
    pub(crate) fn remove_columns(
        &self,
        src_row: Row,
        e: Entity,
        removing: &Vec<(ComponentIndex, ColumnIndex)>,
        removed_columns: &Vec<Option<Share<ComponentRemovedRecord>>>,
    ) {
        for i in self.removed_indexs.clone().into_iter() {
            let (_, column_index) = unsafe { removing.get_unchecked(i) };
            let column = self.src.get_column_unchecked(*column_index);
            if column.needs_drop() {
                column.drop_row_unchecked(src_row);
            }
            // if !dst.is_null() {
            //     // 如果目标原型的移除列上有对应监听，则记录移除行的tick
            //     let d = self.dst.get_remove_column(*dst);
            //     *d.ticks.load_alloc(dst_row.index()) = tick;
            //     // 在脏列表上记录移除行
            //     d.dirty.record(e, dst_row, tick);
            // }
            // 如果移除列上有对应监听，则记录移除实体
            if let Some(record) = unsafe { removed_columns.get_unchecked(i)} {
                record.record(e);
            }
        }
    }
    // pub(crate) fn move_remove_columns(
    //     &self,
    //     src_row: Row,
    //     dst_row: Row,
    //     e: Entity,
    //     move_removed_columns: &Vec<(ColumnIndex, ColumnIndex)>,
    // ) {
    //     for i in self.move_removed_indexs.clone().into_iter() {
    //         let column_index = unsafe { move_removed_columns.get_unchecked(i) };
    //         let column = self.src.get_remove_column(column_index.0);
    //         // 如果目标原型的移除列上有对应监听，则记录移除行
    //         let d = self.dst.get_remove_column(column_index.1);
    //         // 在脏列表上记录移除行
    //         let tick = column
    //             .ticks
    //             .get_i(src_row.index())
    //             .map_or(Tick::null(), |r| *r);
    //         *d.ticks.load_alloc(dst_row.index()) = tick;
    //         d.dirty.record_unchecked(e, dst_row);
    //     }
    // }
}

#[derive(Debug)]
pub struct AState {
    sorted_add_removes: Vec<(ComponentIndex, bool)>,
    pub(crate) adding: Vec<(ComponentIndex, ColumnIndex)>, // ColumnIndex是组件在目标原型vec中的位置
    moving: Vec<(ComponentIndex, ColumnIndex, ColumnIndex)>, // 两个ColumnIndex分别是源原型vec中的位置及目标原型vec中的位置
    removing: Vec<(ComponentIndex, ColumnIndex)>,            // ColumnIndex是组件在源原型vec中的位置
    removed_columns: Vec<Option<Share<ComponentRemovedRecord>>>, // 源原型的被移除的组件的移除记录
    // removed_columns: Vec<(ColumnIndex, ColumnIndex)>, // 源原型的被移除的组件列位置列表及对应目标原型的removed_columns列位置, 如果为Null表示没有Tick及对应的监听
    // move_removed_columns: Vec<(ColumnIndex, ColumnIndex)>, // 源原型的removed_column的组件列位置列表及对应目标原型的removed_columns列位置, 如果为Null表示没有Tick及对应的监听
}
impl AState {
    pub(crate) fn make(
        world: &mut World,
        add: Vec<ComponentInfo>,
        remove: Vec<ComponentInfo>,
    ) -> Self {
        let mut sorted_add_removes = Vec::new();
        world.add_component_indexs(add, &mut sorted_add_removes, true);
        world.add_component_indexs(remove, &mut sorted_add_removes, false);
        sorted_add_removes.sort_unstable();
        Self::new(sorted_add_removes)
    }

    pub(crate) fn new(sorted_add_removes: Vec<(ComponentIndex, bool)>) -> Self {
        Self {
            sorted_add_removes,
            adding: Default::default(),
            moving: Default::default(),
            removing: Default::default(),
            removed_columns: Default::default(),
            // move_removed_columns: Default::default(),
        }
    }
    // 计算源和目标原型，哪些组件是一样，一样就需要获得列位置映射。哪些组件是新增或移除的
    pub(crate) fn clear<'a>(
        &self,
        world: &'a World,
        vec: &mut Vec<ArchetypeMapping>, // 记录所有的原型映射
        mapping_dirtys: &mut Vec<ArchetypeLocalIndex>,
    ) {
        // 处理标记移除的条目， 将要移除的组件释放，将相同的组件拷贝
        for ar_index in mapping_dirtys.drain(..) {
            let am = unsafe { vec.get_unchecked_mut(ar_index.index()) };
            // 检查是否有destroy
            for i in (0..am.moves.len()).rev() {
                let (src_row, dst_row, _e) = unsafe { am.moves.get_unchecked(i) };
                let old = am.src.mark_remove(*src_row);
                if old.is_null() { // 已经被destroy
                    // 目标原型上移除该行
                    am.dst.removes.insert(*dst_row);
                    // 删除move条目
                    am.moves.swap_remove(i);
                }
            }
            self.move_columns(am);
            self.remove_columns(am);
            // 设置目标原型的entity及entity上的EntityAddr
            for (_, dst_row, e) in am.moves.iter() {
                am.dst.set(*dst_row, *e);
                world.replace(*e, am.dst_index, *dst_row);
            }
            am.moves.clear();
        }
    }

    // 将需要移动的全部源组件移动到新位置上
    pub(crate) fn move_columns(&self, am: &mut ArchetypeMapping) {
        for index in am.move_indexs.clone() {
            let (_, src_index, dst_index) = unsafe { self.moving.get_unchecked(index) };
            let src_column = am.src.get_column_unchecked(*src_index);
            let dst_column = am.dst.get_column_unchecked(*dst_index);
            Self::move_column(src_column, dst_column, &am.moves);
        }
    }
    // 将源组件移动到新位置上
    pub(crate) fn move_column(
        src_column: &Column,
        dst_column: &Column,
        moves: &Vec<(Row, Row, Entity)>,
    ) {
        for (src_row, dst_row, _) in moves.iter() {
            let src_data: *mut u8 = src_column.get_row(*src_row);
            // println!("move_column dst_column: {:?}, src_column: {:?}, src_row: {:?}, dst_row: {:?}", (dst_column.info().world_index, &dst_column.info().type_name), (src_column.info().world_index, &src_column.info().type_name), src_row, dst_row);
            dst_column.write_row(*dst_row, src_data);
        }
        if src_column.info().is_tick() {
            for (src_row, dst_row, e) in moves.iter() {
                let tick = src_column.get_tick_unchecked(*src_row);
                dst_column.add_record_unchecked(*e, *dst_row, tick);
            }
        }
    }
    // 将需要移除的全部源组件移除，如果目标原型的移除列上有对应监听，则记录移除行
    pub(crate) fn remove_columns(&self, am: &mut ArchetypeMapping) {
        for i in am.removed_indexs.clone().into_iter() {
            let (_, column_index) = unsafe { self.removing.get_unchecked(i) };
            let column = am.src.get_column_unchecked(*column_index);
            if column.needs_drop() {
                for (src_row, _dst_row, _e) in am.moves.iter() {
                    column.drop_row_unchecked(*src_row)
                }
            }
            // 如果移除列上有对应监听，则记录移除行
            if let Some(record) = unsafe { self.removed_columns.get_unchecked(i)} {
                for (_src_row, _dst_row, e) in am.moves.iter() {
                    // 记录移除实体
                    record.record(*e);
                }
            }
        }
    }
    // // 移动移除组件的tick
    // pub(crate) fn move_remove_columns(&self, am: &mut ArchetypeMapping) {
    //     for i in am.move_removed_indexs.clone().into_iter() {
    //         let column_index = unsafe { self.move_removed_columns.get_unchecked(i) };
    //         let src_removed_column = am.src.get_remove_column(column_index.0);
    //         // 如果目标原型的移除列上有对应监听，则记录移除行
    //         let dst_removed_column = am.dst.get_remove_column(column_index.1);
    //         for (src_row, dst_row, e) in am.moves.iter() {
    //             // 在脏列表上记录移除行
    //             let tick = src_removed_column
    //                 .ticks
    //                 .get_i(src_row.index())
    //                 .map_or(Tick::null(), |r| *r);
    //             *dst_removed_column.ticks.load_alloc(dst_row.index()) = tick;
    //             dst_removed_column.dirty.record(*e, *dst_row, tick);
    //         }
    //     }
    // }

    // 计算源和目标原型，哪些组件是一样，一样就需要获得列位置映射。哪些组件是新增或移除的
    pub(crate) fn find_mapping<'a>(
        &mut self,
        world: &'a World,
        mapping: &mut ArchetypeMapping,
        existed_adding_is_move: bool,
    ) -> (bool, bool) {
        // let mapping = unsafe { self.vec.get_unchecked_mut(ar_index.index()) };
        // println!("find_mapping: {:?}", (ar_index, mapping.src.index(), mapping.dst.index(), mapping.dst_index));
        if !mapping.dst_index.is_null() {
            return (false, false);
        }
        let add_start: usize = self.adding.len();
        let move_start = self.moving.len();
        let removing_start = self.removing.len();
        // 如果本地没有找到，则创建components，去world上查找或创建
        let info = mapping.src.alter1(
            world,
            &mut self.sorted_add_removes,
            &mut self.adding,
            &mut self.moving,
            &mut self.removing,
            existed_adding_is_move,
        );
        mapping.add_indexs = add_start..self.adding.len();
        mapping.move_indexs = move_start..self.moving.len();
        mapping.removed_indexs = removing_start..self.removing.len();
        // 有可能和本system的ar重合，由于alter是延迟的，也不会有引用被改写的问题
        if &info.id == mapping.src.id() {
            // 同原型内移动，由于bundle_vec的对应位置还未初始化，所以is_new应为true
            mapping.dst = mapping.src.clone();
            mapping.dst_index = mapping.src.index();
            return (true, false);
        }
        let (dst_index, dst) = world.find_archtype(info);
        mapping.dst = dst;
        mapping.dst_index = dst_index;
        // 计算移除列及对应移除记录
        for i in mapping.removed_indexs.clone() {
            let (_, index) = unsafe { self.removing.get_unchecked(i) };
            let c = mapping.src.get_column_unchecked(*index);
            let crr = world.get_event_record(&c.info().type_id).map(|r| {
                // todo 首次alter时，就state上记录crr。不需要removes_columns上记录很多次
                Share::downcast::<ComponentRemovedRecord>(r.into_any()).unwrap()
            });
            self.removed_columns.push(crr);
        }
        // let move_removed_start = self.move_removed_columns.len();
        // // 计算源原型的RemovedColumns，在目标原型上RemovedColumns对应的位置
        // for (i, r) in mapping.src.get_remove_columns().iter().enumerate() {
        //     // 获取被移除的组件在目标原型的移除列的位置
        //     let remove_column_index = mapping.dst.add_remove_column_index(r.index);
        //     self.move_removed_columns
        //         .push((i.into(), remove_column_index));
        // }
        // mapping.move_removed_indexs = move_removed_start..self.move_removed_columns.len();
        (true, true)
    }
    pub(crate) fn alter_row(
        &self,
        world: &World,
        mapping: &ArchetypeMapping, // 原型映射
        src_row: Row,
        dst_row: Row,
        e: Entity,
    ) {
        // println!("alter_row: {:?}", (&mapping.dst_index, ar_index, src_row, dst_row, e, tick));
        mapping.src.mark_remove(src_row);
        mapping.move_columns(src_row, dst_row, e, &self.moving);
        mapping.remove_columns(src_row,  e, &self.removing, &self.removed_columns);
        // // // 移动所有的RemoveColumn
        // mapping.move_remove_columns(src_row, dst_row, e, &self.move_removed_columns);
        // 写目标行的Entity
        mapping.dst.set(dst_row, e);
        // 更改entity上存的EntityAddr
        world.replace(e, mapping.dst_index, dst_row);
    }

    /// 销毁实体
    fn destroy(
        // &self,
        world: &World,
        vec: &Vec<ArchetypeMapping>, // 记录所有的原型映射
        entity: Entity,
        map: &Vec<ArchetypeLocalIndex>,
    ) -> Result<bool, QueryError> {
        let (addr, local_index) = check(world, entity, /* cache_mapping, */ map)?;
        let ar = unsafe { &vec.get_unchecked(local_index.index()).src };
        Self::destroy_row(world, ar, addr.row)
    }
    /// 销毁
    pub(crate) fn destroy_row(world: &World, ar: &Archetype, row: Row) -> Result<bool, QueryError> {
        let e = ar.destroy(row);
        if e.is_null() {
            return Err(QueryError::NoSuchRow);
        }
        world.entities.remove(e).unwrap();
        Ok(true)
    }
}

pub struct AlterState<A: Bundle> {
    bundle_vec: Vec<MaybeUninit<A::Item>>, // 记录所有的原型状态，本变更新增组件在目标原型的状态（新增组件的偏移）
    pub(crate) vec: Vec<ArchetypeMapping>, // 记录所有的原型映射
    mapping_dirtys: Vec<ArchetypeLocalIndex>, // 本次变更的原型映射在vec上的索引
    state: AState,
}
impl<A: Bundle> Deref for AlterState<A> {
    type Target = AState;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}
impl<A: Bundle> DerefMut for AlterState<A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}
impl<A: Bundle> AlterState<A> {
    pub(crate) fn make(
        world: &mut World,
        add: Vec<ComponentInfo>,
        remove: Vec<ComponentInfo>,
    ) -> Self {
        let state = AState::make(world, add, remove);
        Self {
            bundle_vec: Vec::new(),
            vec: Vec::new(),
            mapping_dirtys: Vec::new(),
            state,
        }
    }

    pub(crate) fn push_archetype(&mut self, ar: ShareArchetype, world: &World) {
        self.vec
            .push(ArchetypeMapping::new(ar, world.empty_archetype().clone()));
        self.bundle_vec.push(MaybeUninit::uninit());
    }

    pub(crate) fn alter<'w>(
        &mut self,
        world: &'w World,
        ar_index: ArchetypeLocalIndex,
        e: Entity,
        src_row: Row,
        components: A,
        tick: Tick,
    ) -> Option<(&ShareArchetype, ArchetypeWorldIndex)> {
        let mapping = unsafe { self.vec.get_unchecked_mut(ar_index.index()) };
        // println!("alter: {:?}", (e, src_row, ar_index));
        let (is_new, new_ar) = self.state.find_mapping(world, mapping, false);
        if is_new {
            // 首次映射
            // 因为Bundle的state都是不需要释放的，所以mut替换时，是安全的
            let s = unsafe { self.bundle_vec.get_unchecked_mut(ar_index.index()) };
            *s = MaybeUninit::new(A::init_item(world, &mapping.dst));
        }
        if mapping.dst.id() == mapping.src.id() {
            let item = unsafe {
                self.bundle_vec
                    .get_unchecked(ar_index.index())
                    .assume_init_ref()
            };
            // 目标原型和源原型相同，直接写入
            A::insert(item, components, e, src_row, tick);
            return None;
        }
        let dst_row = mapping.dst.alloc();
        let item = unsafe {
            self.bundle_vec
                .get_unchecked(ar_index.index())
                .assume_init_ref()
        };
        A::insert(item, components, e, dst_row, tick);
        // 记录移除行
        mapping.push(src_row, dst_row, e, ar_index, &mut self.mapping_dirtys);
        // self.state.alter_row(world, mapping, src_row, dst_row, e, tick);
        if new_ar {
            Some((&mapping.dst, mapping.dst_index))
        } else {
            None
        }
    }
}

pub struct AlterIter<'w, Q: FetchComponents + 'static, F: FilterComponents + 'static, A: Bundle> {
    it: QueryIter<'w, Q, F>,
    state: &'w mut AlterState<A>,
}
impl<'w, Q: FetchComponents, F: FilterComponents, A: Bundle> AlterIter<'w, Q, F, A> {
    pub fn entity(&self) -> Entity {
        self.it.entity()
    }
    /// 标记销毁当前迭代的实体

    pub fn destroy(&mut self) -> Result<bool, QueryError> {
        AState::destroy_row(&self.it.world, &self.it.ar, self.it.row)
    }

    pub fn alter(&mut self, components: A) -> Result<bool, QueryError> {
        check_mark(
            &self.it.world,
            self.it.e,
            &self.it.state.map,
        )?;
        self.state.alter(
            &self.it.world,
            self.it.ar_index,
            self.it.e,
            self.it.row,
            components,
            self.it.tick,
        );
        Ok(true)
    }
}
impl<'w, Q: FetchComponents, F: FilterComponents, A: Bundle> Iterator for AlterIter<'w, Q, F, A> {
    type Item = Q::Item<'w>;

    fn next(&mut self) -> Option<Self::Item> {
        self.it.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
}
