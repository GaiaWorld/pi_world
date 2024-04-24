/// 组件变更，增加和删除组件，及删除entity
/// 如果原型上没有要删除的组件，则自动忽略
/// 如果要增加的组件在源原型上存在，则直接替换
/// 和Query一样，可以get和iter，如果该原型可以查到，则会记录在本地源原型列表中
/// 要求entity的原型，必须在本地源原型列表中，这样保证有正确的写依赖
/// 根据删除时entity的原型，动态查找和创建删除后对应的原型
/// 删除组件会在源原型对entity进行标记删除，如果system后续迭代和查找是找不到的，但如果有引用，则引用还是可以继续读写。
/// 目标原型会立即添加新的条目，但该条目的entity是null，表示是看不到的。这时添加的组件，会立即写到目标原型对应的位置，等变更结束时，会将标记删除的entity，誊写到目标原型上，并修改entity。
/// 目标原型有可能和本system的原型重合，但由于alter是延迟的，也不会有引用被改写的问题
///
/// 最新： 计划使用执行图来保证，alter不会操作到正在读写的原型。 这样就不用处理各种多线程数据不一致的情况， 比如 entity 读有值， 但组件没有读到正确数据。
/// 每个system的run会根据依赖是否全部写完毕才开始执行，对应的就是一个ShareU32的wait_count数字减到0。
/// 每个system有自身状态ShareU8的run_state，初值为wait。system的run会先执行before，before会先同步world上原型数组的长度，同步后，修改自身状态ShareU8为running，执行完毕后改为ok。
/// 执行图在执行中， 收到A某system产生的原型创建的事件，此时该原型还为放入world上原型数组， 用该原型去匹配所有的system，返回：无依赖、读、写、目标写原型（alter会根据源原型写到目标原型，该目标原型也需要纳入写）。
/// 如果有依赖，则立即将wait_count加1，如果原wait_count=0表示已经开始执行，那么循环等待该system的run_state为running或ok。 这样，对该system要么看到该原型，要么看不到。
///
use std::any::TypeId;
use std::borrow::Cow;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::mem::{transmute, MaybeUninit};
use std::ops::Range;

use pi_null::Null;
use pi_proc_macros::all_tuples;
use pi_share::Share;

use crate::archetype::*;
use crate::column::Column;
use crate::fetch::FetchComponents;
use crate::filter::FilterComponents;
use crate::insert::InsertComponents;
use crate::param_set::ParamSetElement;
use crate::query::{check, ArchetypeLocalIndex, Query, QueryError, QueryIter, QueryState, Queryer};
use crate::system::SystemMeta;
use crate::system_params::SystemParam;
use crate::world::*;

pub struct Alterer<
    'world,
    Q: FetchComponents + 'static,
    F: FilterComponents + 'static = (),
    A: InsertComponents + 'static = (),
    D: DelComponents + 'static = (),
> {
    query: Queryer<'world, Q, F>,
    state: AlterState<A>,
    _k: PhantomData<D>,
}
impl<
        'world,
        Q: FetchComponents + 'static,
        F: FilterComponents + 'static,
        A: InsertComponents,
        D: DelComponents,
    > Alterer<'world, Q, F, A, D>
{
    // 将新多出来的原型，创建原型空映射
    pub(crate) fn state_align(
        world: &World,
        state: &mut AlterState<A>,
        query_state: &QueryState<Q, F>,
    ) {
        // 将新多出来的原型，创建原型空映射
        for i in state.vec.len()..query_state.vec.len() {
            let ar = unsafe { query_state.vec.get_unchecked(i).0.clone() };
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
    #[inline]
    pub fn contains(&self, entity: Entity) -> bool {
        self.query.contains(entity)
    }
    #[inline]
    pub fn get(
        &'world self,
        e: Entity,
    ) -> Result<<<Q as FetchComponents>::ReadOnly as FetchComponents>::Item<'world>, QueryError>
    {
        self.query.get(e)
    }
    #[inline]
    pub fn get_mut(
        &'world mut self,
        e: Entity,
    ) -> Result<<Q as FetchComponents>::Item<'world>, QueryError> {
        self.query.get_mut(e)
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.query.is_empty()
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.query.len()
    }
    #[inline]
    pub fn iter(&self) -> QueryIter<'_, <Q as FetchComponents>::ReadOnly, F> {
        self.query.iter()
    }
    #[inline]
    pub fn iter_mut(&mut self) -> AlterIter<'_, Q, F, A> {
        AlterIter {
            it: self.query.iter_mut(),
            state: &mut self.state,
        }
    }
    #[inline]
    pub fn delete(&mut self, e: Entity) -> Result<bool, QueryError> {
        delete(
            &self.query.world,
            e,
            &self.state.vec,
            self.query.cache_mapping.get_mut(),
            &self.query.state.map,
            &mut self.state.deletes,
        )
    }
    #[inline]
    pub fn alter(
        &mut self,
        e: Entity,
        components: <A as InsertComponents>::Item,
    ) -> Result<bool, QueryError> {
        let addr = check(
            &self.query.world,
            e,
            self.query.cache_mapping.get_mut(),
            &self.query.state.map,
        )?;
        self.state.alter(
            &self.query.world,
            self.query.state.cache_mapping.1,
            e,
            addr.row,
            components,
        )
    }
}
impl<
        'world,
        Q: FetchComponents + 'static,
        F: FilterComponents + 'static,
        A: InsertComponents + 'static,
        D: DelComponents + 'static,
    > Drop for Alterer<'world, Q, F, A, D>
{
    fn drop(&mut self) {
        clear(
            self.query.world,
            &mut self.state.vec,
            &mut self.state.mapping_dirtys,
            &mut self.state.deletes,
            &self.state.moved_cloumns,
            &self.state.added_cloumns,
            &self.state.del_cloumns,
        );
    }
}
pub struct Alter<
    'world,
    Q: FetchComponents + 'static,
    F: FilterComponents + 'static = (),
    A: InsertComponents + 'static = (),
    D: DelComponents + 'static = (),
> {
    query: Query<'world, Q, F>,
    state: &'world mut AlterState<A>,
    _k: PhantomData<D>,
}

unsafe impl<
        'world,
        Q: FetchComponents + 'static,
        F: FilterComponents + 'static,
        A: InsertComponents,
        D: DelComponents,
    > Send for Alter<'world, Q, F, A, D>
{
}
unsafe impl<
        'world,
        Q: FetchComponents + 'static,
        F: FilterComponents + 'static,
        A: InsertComponents,
        D: DelComponents,
    > Sync for Alter<'world, Q, F, A, D>
{
}

impl<
        'world,
        Q: FetchComponents + 'static,
        F: FilterComponents + 'static,
        A: InsertComponents,
        D: DelComponents,
    > Alter<'world, Q, F, A, D>
{
    #[inline]
    pub(crate) fn new(query: Query<'world, Q, F>, state: &'world mut AlterState<A>) -> Self {
        Alter {
            query,
            state,
            _k: PhantomData,
        }
    }
    #[inline]
    pub fn contains(&self, entity: Entity) -> bool {
        self.query.contains(entity)
    }
    #[inline]
    pub fn get(
        &'world self,
        e: Entity,
    ) -> Result<<<Q as FetchComponents>::ReadOnly as FetchComponents>::Item<'world>, QueryError>
    {
        self.query.get(e)
    }
    #[inline]
    pub fn get_mut(
        &'world mut self,
        e: Entity,
    ) -> Result<<Q as FetchComponents>::Item<'world>, QueryError> {
        self.query.get_mut(e)
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.query.is_empty()
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.query.len()
    }
    #[inline]
    pub fn iter(&self) -> QueryIter<'_, <Q as FetchComponents>::ReadOnly, F> {
        self.query.iter()
    }
    #[inline]
    pub fn iter_mut(&mut self) -> AlterIter<'_, Q, F, A> {
        AlterIter {
            it: self.query.iter_mut(),
            state: &mut self.state,
        }
    }
    #[inline]
    pub fn delete(&mut self, e: Entity) -> Result<bool, QueryError> {
        delete(
            &self.query.world,
            e,
            &self.state.vec,
            self.query.cache_mapping.get_mut(),
            &self.query.state.map,
            &mut self.state.deletes,
        )
    }
    #[inline]
    pub fn alter(
        &mut self,
        e: Entity,
        components: <A as InsertComponents>::Item,
    ) -> Result<bool, QueryError> {
        let addr = check(
            &self.query.world,
            e,
            self.query.cache_mapping.get_mut(),
            &self.query.state.map,
        )?;
        self.state.alter(
            &self.query.world,
            self.query.state.cache_mapping.1,
            e,
            addr.row,
            components,
        )
    }
}

impl<
        Q: FetchComponents + 'static,
        F: FilterComponents + Send + Sync + 'static,
        A: InsertComponents + 'static,
        D: DelComponents + Send + 'static,
    > SystemParam for Alter<'_, Q, F, A, D>
{
    type State = (QueryState<Q, F>, AlterState<A>);
    type Item<'w> = Alter<'w, Q, F, A, D>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let q = Query::init_state(world, system_meta);
        (q, AlterState::new(A::components(), D::components()))
    }
    fn archetype_depend(
        _world: &World,
        _system_meta: &SystemMeta,
        state: &Self::State,
        archetype: &Archetype,
        result: &mut ArchetypeDependResult,
    ) {
        Q::archetype_depend(archetype, result);
        // 如果相关， 则添加删除类型，并返回Alter后的原型id
        if result.flag.bits() > 0 && !result.flag.contains(Flags::WITHOUT) {
            result.merge(ArchetypeDepend::Flag(Flags::DELETE));
            let (components, _) = archetype.alter(&state.1.sort_add, &state.1.sort_del);
            let id_name = ComponentInfo::calc_id_name(&components);
            result.merge(ArchetypeDepend::Alter(id_name));
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

    #[inline]
    fn align(world: &World, _system_meta: &SystemMeta, state: &mut Self::State) {
        state.0.align(world);
    }

    #[inline]
    fn get_param<'world>(
        world: &'world World,
        _system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
    ) -> Self::Item<'world> {
        // 将新多出来的原型，创建原型空映射
        Alterer::<Q, F, A, D>::state_align(world, &mut state.1, &state.0);
        Alter::new(Query::new(world, &mut state.0), &mut state.1)
    }
    #[inline]
    fn get_self<'world>(
        world: &'world World,
        system_meta: &'world SystemMeta,
        state: &'world mut Self::State,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, system_meta, state)) }
    }
}
impl<
        Q: FetchComponents + 'static,
        F: FilterComponents + Send + Sync,
        A: InsertComponents + 'static,
        D: DelComponents + Send + 'static,
    > ParamSetElement for Alter<'_, Q, F, A, D>
{
    fn init_set_state(world: &World, system_meta: &mut SystemMeta) -> Self::State {
        Q::init_read_write(world, system_meta);
        F::init_read_write(world, system_meta);
        system_meta.param_set_check();
        let q = QueryState::create(world);
        (q, AlterState::new(A::components(), D::components()))
    }
}
impl<
        'world,
        Q: FetchComponents + 'static,
        F: FilterComponents + 'static,
        A: InsertComponents + 'static,
        D: DelComponents + 'static,
    > Drop for Alter<'world, Q, F, A, D>
{
    fn drop(&mut self) {
        clear(
            self.query.world,
            &mut self.state.vec,
            &mut self.state.mapping_dirtys,
            &mut self.state.deletes,
            &self.state.moved_cloumns,
            &self.state.added_cloumns,
            &self.state.del_cloumns,
        );
    }
}
#[derive(Debug)]
pub(crate) struct ArchetypeMapping {
    src: ShareArchetype,            // 源原型
    dst: ShareArchetype,            // 映射到的目标原型
    dst_index: ArchetypeWorldIndex, // 目标原型在World原型数组中的位置
    move_indexs: Range<usize>,      // 源原型和目标原型的组件映射的起始和结束位置
    add_indexs: Range<usize>,       // 目标原型上新增的组件的起始和结束位置
    del_indexs: Range<usize>,       // 源原型上被删除的组件的起始和结束位置
    moves: Vec<(Row, Row, Entity)>, // 本次标记移动的条目
}

impl ArchetypeMapping {
    pub fn new(src: ShareArchetype, dst: ShareArchetype) -> Self {
        ArchetypeMapping {
            src,
            dst,
            dst_index: 0,
            move_indexs: 0..0,
            add_indexs: 0..0,
            del_indexs: 0..0,
            moves: Default::default(),
        }
    }
}
pub struct AlterState<A: InsertComponents> {
    sort_add: Vec<ComponentInfo>,
    sort_del: Vec<TypeId>,
    pub(crate) vec: Vec<ArchetypeMapping>, // 记录所有的原型映射
    state_vec: Vec<MaybeUninit<A::State>>, // 记录所有的原型状态，本变更新增组件在目标原型的状态（新增组件的偏移）
    moved_cloumns: Vec<(ColumnIndex, ColumnIndex)>, // 源目标原型的组件列位置映射列表
    added_cloumns: Vec<ColumnIndex>, // 目标原型的新增加的组件位置列表，主要是给InsertComponents用的
    del_cloumns: Vec<ColumnIndex>,   // 源原型的被删除的组件列位置列表
    mapping_dirtys: Vec<ArchetypeLocalIndex>, // 本次变更的原型映射在vec上的索引
    deletes: Vec<(ArchetypeLocalIndex, Row)>, // 本次删除的本地原型位置及条目
}
impl<A: InsertComponents> AlterState<A> {
    pub(crate) fn new(mut add: Vec<ComponentInfo>, mut del: Vec<TypeId>) -> Self {
        add.sort();
        del.sort();
        Self {
            sort_add: add,
            sort_del: del,
            vec: Default::default(),
            state_vec: Vec::new(),
            moved_cloumns: Default::default(),
            added_cloumns: Default::default(),
            del_cloumns: Default::default(),
            mapping_dirtys: Vec::new(),
            deletes: Vec::new(),
        }
    }
    #[inline]
    pub(crate) fn push_archetype(&mut self, ar: ShareArchetype, world: &World) {
        self.vec
            .push(ArchetypeMapping::new(ar, world.empty_archetype().clone()));
        self.state_vec.push(MaybeUninit::uninit());
    }

    pub(crate) fn alter<'w>(
        &mut self,
        world: &'w World,
        ar_index: ArchetypeLocalIndex,
        e: Entity,
        row: Row,
        components: A::Item,
    ) -> Result<bool, QueryError> {
        let mut mapping = unsafe { self.vec.get_unchecked_mut(ar_index) };
        if mapping.dst.table.columns.len() == 0 {
            // 如果为空映射，则创建components，去world上查找或创建
            mapping_init(
                world,
                &mut mapping,
                &mut self.moved_cloumns,
                &mut self.added_cloumns,
                &mut self.del_cloumns,
                &self.sort_add,
                &self.sort_del,
            );

            // 因为InsertComponents的state都是不需要释放的，所以mut替换时，是安全的
            let s = unsafe { self.state_vec.get_unchecked_mut(ar_index) };
            *s = MaybeUninit::new(A::init_state(world, &mapping.dst));
        }
        let dst_row = alter_row(&mut self.mapping_dirtys, &mut mapping, ar_index, row)?;
        A::insert(
            unsafe { &self.state_vec.get_unchecked(ar_index).assume_init_ref() },
            components,
            e,
            dst_row,
        );
        Ok(true)
    }
}

pub struct AlterIter<
    'w,
    Q: FetchComponents + 'static,
    F: FilterComponents + 'static,
    A: InsertComponents,
> {
    it: QueryIter<'w, Q, F>,
    state: &'w mut AlterState<A>,
}
impl<'w, Q: FetchComponents, F: FilterComponents, A: InsertComponents> AlterIter<'w, Q, F, A> {
    #[inline(always)]
    pub fn entity(&self) -> Entity {
        self.it.entity()
    }
    #[inline(always)]
    pub fn delete(&mut self) -> Result<bool, QueryError> {
        delete_row(
            &self.it.world,
            &self.it.ar,
            self.it.ar_index,
            self.it.row,
            &mut self.state.deletes,
        )
    }
    #[inline(always)]
    pub fn alter(&mut self, components: <A as InsertComponents>::Item) -> Result<bool, QueryError> {
        self.state.alter(
            &self.it.world,
            self.it.ar_index,
            self.it.e,
            self.it.row,
            components,
        )
    }
}
impl<'w, Q: FetchComponents, F: FilterComponents, A: InsertComponents> Iterator
    for AlterIter<'w, Q, F, A>
{
    type Item = Q::Item<'w>;
    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.it.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
}

/// 标记删除
#[inline(always)]
fn delete<'w>(
    world: &'w World,
    entity: Entity,
    vec: &Vec<ArchetypeMapping>,
    cache_mapping: &mut (ArchetypeWorldIndex, ArchetypeLocalIndex),
    map: &HashMap<ArchetypeWorldIndex, ArchetypeLocalIndex>,
    deletes: &mut Vec<(ArchetypeLocalIndex, Row)>,
) -> Result<bool, QueryError> {
    let addr = check(world, entity, cache_mapping, map)?;
    let ar = unsafe { &vec.get_unchecked(cache_mapping.1).src };
    delete_row(world, ar, cache_mapping.1, addr.row, deletes)
}
/// 标记删除
#[inline(always)]
fn delete_row<'w>(
    world: &'w World,
    ar: &'w Archetype,
    ar_index: ArchetypeLocalIndex,
    row: Row,
    deletes: &mut Vec<(ArchetypeLocalIndex, Row)>,
) -> Result<bool, QueryError> {
    let e = ar.table.mark_remove(row);
    if e.is_null() {
        return Err(QueryError::NoSuchRow);
    }
    world.entities.remove(e).unwrap();
    deletes.push((ar_index, row));
    Ok(true)
}

// 计算源和目标原型，哪些组件是一样，一样就需要获得列位置映射。哪些组件是新增或删除的
pub(crate) fn mapping_init<'a>(
    world: &'a World,
    mapping: &'a mut ArchetypeMapping,
    move_cloumns: &'a mut Vec<(ColumnIndex, ColumnIndex)>,
    add_cloumns: &'a mut Vec<ColumnIndex>,
    del_cloumns: &'a mut Vec<ColumnIndex>,
    sort_add: &Vec<ComponentInfo>,
    sort_del: &Vec<TypeId>,
) {
    let add_len = sort_add.len();
    let del_len = sort_del.len();
    // 如果本地没有找到，则创建components，去world上查找或创建
    let (components, moving) = mapping.src.alter(sort_add, sort_del);
    let id = ComponentInfo::calc_id(&components);
    // 有可能和本system的ar重合，由于alter是延迟的，也不会有引用被改写的问题
    let (dst_index, dst) = world.find_archtype(id, components);
    mapping.dst = dst;
    mapping.dst_index = dst_index;
    // 两边循环，获得相同组件的列位置映射和删除组件的列位置
    // 计算移动列
    let start = move_cloumns.len();
    if !Share::ptr_eq(&mapping.src, &mapping.dst) {
        // 获得相同组件的列位置映射
        for t in moving {
            let src_column = mapping.src.get_column_index(&t);
            let dst_column = mapping.dst.get_column_index(&t);
            move_cloumns.push((src_column, dst_column));
        }
    } else {
        // 同原型内移动
        for t in moving {
            let column_index = mapping.src.get_column_index(&t);
            move_cloumns.push((column_index, column_index));
        }
    }
    mapping.move_indexs = Range {
        start,
        end: move_cloumns.len(),
    };
    // 计算新增列
    let start = add_cloumns.len();
    if add_len > 0 && !Share::ptr_eq(&mapping.src, &mapping.dst) {
        // 新增组件的位置，目标原型组件存在，但源原型上没有该组件
        for (i, t) in mapping.dst.get_columns().iter().enumerate() {
            let column = mapping.src.get_column_index(&t.info().type_id);
            if column.is_null() {
                add_cloumns.push(i as ColumnIndex);
            }
        }
    }
    mapping.add_indexs = Range {
        start,
        end: add_cloumns.len(),
    };
    // 计算删除列
    let start = del_cloumns.len();
    if del_len > 0 && !Share::ptr_eq(&mapping.src, &mapping.dst) {
        // 删除组件的位置，源组件存在，但目标原型上没有该组件
        for (i, t) in mapping.src.get_columns().iter().enumerate() {
            let column = mapping.dst.get_column_index(&t.info().type_id);
            if column.is_null() {
                del_cloumns.push(i as ColumnIndex);
            }
        }
    }
    mapping.del_indexs = Range {
        start,
        end: del_cloumns.len(),
    };
}

pub(crate) fn alter_row<'w, 'a>(
    mapping_dirtys: &mut Vec<ArchetypeLocalIndex>,
    mapping: &mut ArchetypeMapping,
    ar_index: ArchetypeLocalIndex,
    src_row: Row,
) -> Result<Row, QueryError> {
    let e = mapping.src.table.mark_remove(src_row);
    if e.is_null() {
        return Err(QueryError::NoSuchRow);
    }
    let dst_row = mapping.dst.table.alloc();
    // 记录移动条目的源位置和目标位置
    mapping.moves.push((src_row, dst_row, e));
    if mapping.moves.len() == 1 {
        // 如果该映射是首次记录，则记脏该映射
        mapping_dirtys.push(ar_index);
    }
    Ok(dst_row)
}

// 系统结束后，将变更的条目移动
pub(crate) fn clear(
    world: &World,
    vec: &mut Vec<ArchetypeMapping>,
    mapping_dirtys: &mut Vec<ArchetypeLocalIndex>,
    deletes: &mut Vec<(ArchetypeLocalIndex, Row)>,
    moved_columns: &Vec<(ColumnIndex, ColumnIndex)>,
    added_columns: &Vec<ColumnIndex>,
    del_columns: &Vec<ColumnIndex>,
) {
    // 处理标记移除的条目， 将要删除的组件释放，将相同的组件拷贝
    for ar_index in mapping_dirtys.iter() {
        let am = unsafe { vec.get_unchecked_mut(*ar_index) };
        move_columns(am, moved_columns);
        delete_columns(am, del_columns);
        add_columns(am, added_columns);
        update_table_world(world, am);
        am.moves.clear();
    }
    mapping_dirtys.clear();
    // 处理deletes
    if deletes.len() == 0 {
        return;
    }
    // 处理标记移除的条目
    for &(ar_index, row) in deletes.iter() {
        let am = unsafe { vec.get_unchecked(ar_index) };
        am.src.table.drop_row(row);
    }
    deletes.clear();
}
// 将需要移动的全部源组件移动到新位置上
fn move_columns(am: &mut ArchetypeMapping, move_columns: &Vec<(ColumnIndex, ColumnIndex)>) {
    for i in am.move_indexs.clone().into_iter() {
        let (src_i, dst_i) = unsafe { move_columns.get_unchecked(i) };
        let src_column = am.src.table.get_column_unchecked(*src_i);
        let dst_column = am.dst.table.get_column_unchecked(*dst_i);
        move_column(src_column, dst_column, &am.moves);
    }
}
// 将源组件移动到新位置上
fn move_column(src_column: &Column, dst_column: &Column, moves: &Vec<(Row, Row, Entity)>) {
    for (src_row, dst_row, _) in moves.iter() {
        let src_data: *mut u8 = src_column.get_row(*src_row);
        dst_column.write_row(*dst_row, src_data);
    }
}
// 将需要删除的全部源组件删除
fn delete_columns(am: &mut ArchetypeMapping, del_columns: &Vec<ColumnIndex>) {
    for i in am.del_indexs.clone().into_iter() {
        let column_index = unsafe { del_columns.get_unchecked(i) };
        let column = am.src.table.get_column_unchecked(*column_index);
        if column.removed.listener_len() > 0 {
            if column.needs_drop() {
                for (src_row, _dst_row, e) in am.moves.iter() {
                    column.drop_row_unchecked(*src_row);
                    column.removed.record_unchecked(*e, *src_row);
                }
            } else {
                for (src_row, _dst_row, e) in am.moves.iter() {
                    column.removed.record_unchecked(*e, *src_row);
                }
            }
        } else if column.needs_drop() {
            for (src_row, _dst_row, _) in am.moves.iter() {
                column.drop_row_unchecked(*src_row)
            }
        }
    }
}
// 通知新增的源组件
fn add_columns(am: &mut ArchetypeMapping, add_columns: &Vec<ColumnIndex>) {
    for i in am.add_indexs.clone().into_iter() {
        let column_index = unsafe { add_columns.get_unchecked(i) };
        let column = am.dst.table.get_column_unchecked(*column_index);
        if column.added.listener_len() == 0 {
            continue;
        }
        for (_, dst_row, e) in am.moves.iter() {
            column.added.record_unchecked(*e, *dst_row);
        }
    }
}
// 修改entity上的EntityAddr， table上的entitys也对应记录Entity
fn update_table_world(world: &World, am: &mut ArchetypeMapping) {
    for (_, dst_row, e) in am.moves.iter() {
        am.dst.table.set(*dst_row, *e);
        world.replace(*e, am.dst_index, *dst_row);
    }
}
pub trait DelComponents {
    fn components() -> Vec<TypeId>;
}

macro_rules! impl_tuple_del_components {
    ($($name: ident),*) => {
        #[allow(non_snake_case)]
        #[allow(clippy::unused_unit)]
        impl<$($name: 'static),*> DelComponents for ($($name,)*) {

            fn components() -> Vec<TypeId> {
                vec![$(TypeId::of::<$name>(),)*]
            }
        }
    };
}
all_tuples!(impl_tuple_del_components, 0, 16, F);
