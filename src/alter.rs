/// 组件变更，增加和移除组件，及销毁entity
/// 如果原型上没有要移除的组件，则自动忽略
/// 如果要增加的组件在源原型上存在，则直接替换
/// 和Query一样，可以get和iter，如果该原型可以查到，则会记录在本地源原型列表中
/// 要求entity的原型，必须在本地源原型列表中，这样保证有正确的写依赖
/// 根据移除时entity的原型，动态查找和创建移除后对应的原型
/// 移除组件会在源原型对entity进行标记移除，如果system后续迭代和查找是找不到的，但如果有引用，则引用还是可以继续读写。
/// 目标原型会立即添加新的条目，但该条目的entity是null，表示是看不到的。这时添加的组件，会立即写到目标原型对应的位置，等变更结束时，会将标记移除的entity，誊写到目标原型上，并修改entity。
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
// use std::collections::HashMap;
use std::marker::PhantomData;
use std::mem::{transmute, MaybeUninit};
// use std::ops::Range;

use pi_null::Null;

use crate::archetype::*;
use crate::column::Column;
use crate::fetch::FetchComponents;
use crate::filter::FilterComponents;
use crate::insert::Bundle;
use crate::param_set::ParamSetElement;
use crate::query::{check, ArchetypeLocalIndex, Query, QueryError, QueryIter, QueryState, Queryer};
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
        destroy(
            &self.query.world,
            e,
            &self.state.vec,
            // self.query.cache_mapping.get_mut(),
            &self.query.state.map,
            // &mut self.state.destroys,
        )
    }
    
    pub fn alter(
        &mut self,
        e: Entity,
        components: A,
    ) -> Result<bool, QueryError> {
        let (addr, _world_index, local_index) = check(
            &self.query.world,
            e,
            // self.query.cache_mapping.get_mut(),
            &self.query.state.map,
        )?;
        self.state.alter(
            &self.query.world,
            local_index,
            e,
            addr.row,
            components,
            self.query.tick,
        )
    }
}
impl<
        'world,
        Q: FetchComponents + 'static,
        F: FilterComponents + 'static,
        A: Bundle + 'static,
        D: Bundle + 'static,
    > Drop for Alterer<'world, Q, F, A, D>
{
    fn drop(&mut self) {
        clear(
            self.query.world,
            &mut self.state.vec,
            &mut self.state.mapping_dirtys,
            // &self.state.moved_columns,
            // &self.state.added_columns,
            // &self.state.removed_columns,
            self.query.tick,
        );
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
        destroy(
            &self.query.world,
            e,
            &self.state.vec,
            // self.query.cache_mapping.get_mut(),
            &self.query.state.map,
            // &mut self.state.destroys,
        )
    }
    
    pub fn alter(
        &mut self,
        e: Entity,
        components: A,
    ) -> Result<bool, QueryError> {
        let (addr, _world_index, local_index) = check(
            &self.query.world,
            e,
            // self.query.cache_mapping.get_mut(),
            &self.query.state.map,
        )?;
        self.state.alter(
            &self.query.world,
            local_index,
            e,
            addr.row,
            components,
            self.query.tick,
        )
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
        (q, AlterState::new(A::components(Vec::new()), D::components(Vec::new())))
    }
    fn archetype_depend(
        world: &World,
        _system_meta: &SystemMeta,
        state: &Self::State,
        archetype: &Archetype,
        result: &mut ArchetypeDependResult,
    ) {
        if &state.1.writing_archetype == archetype.id() {
            // println!("archetype_depend: ar:{:?}", archetype.name());
            return result.merge(ArchetypeDepend::Flag(Flags::WRITE));  
        }
        Q::archetype_depend(world, archetype, result);
        // 如果相关， 则添加移除类型，并返回Alter后的原型id
        if result.flag.bits() > 0 && !result.flag.contains(Flags::WITHOUT) {
            result.merge(ArchetypeDepend::Flag(Flags::DELETE));
            let (components, _) = archetype.alter(&state.1.sort_add, &state.1.sort_remove);
            let infos = world.calc_infos(components);
            if archetype.id() != &infos.0 {
                result.merge(ArchetypeDepend::Alter(infos));
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
        Q::init_read_write(world, system_meta);
        F::init_read_write(world, system_meta);
        system_meta.param_set_check();
        let q = QueryState::create(world);
        (q, AlterState::new(A::components(Vec::new()), D::components(Vec::new())))
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
        clear(
            self.query.world,
            &mut self.state.vec,
            &mut self.state.mapping_dirtys,
            // &self.state.moved_columns,
            // &self.state.added_columns,
            // &self.state.removed_columns,
            self.query.tick,
        );
    }
}
#[derive(Debug)]
pub(crate) struct ArchetypeMapping {
    pub(crate)src: ShareArchetype,            // 源原型
    pub(crate) dst: ShareArchetype,            // 映射到的目标原型
    pub(crate)dst_index: ArchetypeWorldIndex, // 目标原型在World原型数组中的位置
    // pub(crate)move_indexs: Range<usize>,      // 源原型和目标原型的组件映射的起始和结束位置
    // pub(crate)add_indexs: Range<usize>,       // 目标原型上新增的组件的起始和结束位置
    // pub(crate)remove_indexs: Range<usize>,    // 源原型上被移除的组件的起始和结束位置
    pub(crate)moves: Vec<(Row, Row, Entity)>, // 本次标记移动的条目
    pub(crate)moving: Vec<ComponentInfo>,
    pub(crate)id: u128,
    pub(crate)add_len: usize,
    pub(crate)remove_len: usize,
}

impl ArchetypeMapping {
    pub fn new(src: ShareArchetype, dst: ShareArchetype) -> Self {
        ArchetypeMapping {
            src,
            dst,
            dst_index: ArchetypeWorldIndex(0),
            // move_indexs: 0..0,
            // add_indexs: 0..0,
            // remove_indexs: 0..0,
            moves: Default::default(),
            moving: Vec::new(),
            id: u128::MAX,
            add_len: 0,
            remove_len: 0,
        }
    }
}
pub struct AlterState<A: Bundle> {
    sort_add: Vec<ComponentInfo>,
    sort_remove: Vec<ComponentInfo>,
    pub(crate) vec: Vec<ArchetypeMapping>, // 记录所有的原型映射
    state_vec: Vec<MaybeUninit<A::State>>, // 记录所有的原型状态，本变更新增组件在目标原型的状态（新增组件的偏移）
    // moved_columns: Vec<(ColumnIndex, ColumnIndex)>, // 源目标原型的组件列位置映射列表
    // added_columns: Vec<ColumnIndex>,       // 目标原型的新增加的组件位置列表，主要是给Bundle用的
    // removed_columns: Vec<(ColumnIndex, ColumnIndex)>, // 源原型的被移除的组件列位置列表及对应目标原型的removed_columns列位置, 如果为Null表示没有对应的监听
    mapping_dirtys: Vec<ArchetypeLocalIndex>,         // 本次变更的原型映射在vec上的索引
    writing_archetype: u128,                                // 正在写入的原型
}
impl<A: Bundle> AlterState<A> {
    pub(crate) fn new(mut add: Vec<ComponentInfo>, mut remove: Vec<ComponentInfo>) -> Self {
        add.sort();
        remove.sort();
        Self {
            sort_add: add,
            sort_remove: remove,
            vec: Default::default(),
            state_vec: Vec::new(),
            // moved_columns: Default::default(),
            // added_columns: Default::default(),
            // removed_columns: Default::default(),
            mapping_dirtys: Vec::new(),
            writing_archetype: 0,
        }
    }
    
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
        components: A,
        tick: Tick,
    ) -> Result<bool, QueryError> {
        let mut mapping = unsafe { self.vec.get_unchecked_mut(ar_index.0 as usize) };
        if mapping.dst.len().0 == 0 {
            // 如果为空映射，则创建components，去world上查找或创建
            mapping_init(
                world,
                &mut mapping,
                // &mut self.moved_columns,
                // &mut self.added_columns,
                // &mut self.removed_columns,
                &self.sort_add,
                &self.sort_remove,
                &mut self.writing_archetype,
            );

            // 因为Bundle的state都是不需要释放的，所以mut替换时，是安全的
            let s = unsafe { self.state_vec.get_unchecked_mut(ar_index.0 as usize) };
            *s = MaybeUninit::new(A::init_state(world, &mapping.dst));
        }
        let dst_row = alter_row(&mut self.mapping_dirtys, &mut mapping, ar_index, row, e)?;
        A::insert(
            unsafe { &self.state_vec.get_unchecked(ar_index.0 as usize).assume_init_ref() },
            components,
            e,
            dst_row,
            tick,
        );
        Ok(true)
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
        destroy_row(
            &self.it.world,
            &self.it.ar,
            // self.it.ar_index,
            self.it.row,
            // &mut self.state.destroys,
        )
    }
    
    pub fn alter(&mut self, components: A) -> Result<bool, QueryError> {
        self.state.alter(
            &self.it.world,
            self.it.ar_index,
            self.it.e,
            self.it.row,
            components,
            self.it.tick,
        )
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

/// 标记销毁实体
fn destroy<'w>(
    world: &'w World,
    entity: Entity,
    vec: &Vec<ArchetypeMapping>,
    // cache_mapping: &mut (ArchetypeWorldIndex, ArchetypeLocalIndex),
    map: &Vec<ArchetypeLocalIndex>,
    // destroys: &mut Vec<(ArchetypeLocalIndex, Row)>,
) -> Result<bool, QueryError> {
    let (addr, _world_index, local_index) = check(world, entity, /* cache_mapping, */ map)?;
    let ar = unsafe { &vec.get_unchecked(local_index.0 as usize).src };
    destroy_row(world, ar, addr.row)
}
/// 标记销毁
fn destroy_row<'w>(
    world: &'w World,
    ar: &'w Archetype,
    // ar_index: ArchetypeLocalIndex,
    row: Row,
    // destroys: &mut Vec<(ArchetypeLocalIndex, Row)>,
) -> Result<bool, QueryError> {
    let e = ar.mark_destroy(row);
    if e.is_null() {
        return Err(QueryError::NoSuchRow);
    }
    world.entities.remove(e).unwrap();
    Ok(true)
}

// 计算源和目标原型，哪些组件是一样，一样就需要获得列位置映射。哪些组件是新增或移除的
pub(crate) fn mapping_init<'a>(
    world: &'a World,
    mapping: &'a mut ArchetypeMapping,
    // moved_columns: &'a mut Vec<(ColumnIndex, ColumnIndex)>,
    // add_columns: &'a mut Vec<ColumnIndex>,
    // removed_columns: &'a mut Vec<(ColumnIndex, ColumnIndex)>,
    sort_add: &Vec<ComponentInfo>,
    sort_remove: &Vec<ComponentInfo>,
    writing_archetype: &mut u128,
) {
    let add_len = sort_add.len();
    let remove_len = sort_remove.len();
    // 如果本地没有找到，则创建components，去world上查找或创建
    let (components, moving) = mapping.src.alter(sort_add, sort_remove);
    let id: u128 = ComponentInfo::calc_id(&components);
    mapping.id = id;
    mapping.moving = moving;
    mapping.add_len = add_len;
    mapping.remove_len = remove_len;
    // 有可能和本system的ar重合，由于alter是延迟的，也不会有引用被改写的问题
    if &id == mapping.src.id() {
        mapping.dst = mapping.src.clone();
        mapping.dst_index = mapping.src.index();
        // 同原型内移动
        // let start = moved_columns.len();
        // for t in moving {
        //     let column_index = mapping.src.get_column_index(t.world_index);
        //     moved_columns.push((column_index, column_index));
        // }
        // mapping.move_indexs = Range {
        //     start,
        //     end: moved_columns.len(),
        // };
        return;
    }
    *writing_archetype = id;
    let (dst_index, dst) = world.find_archtype(id, components);
    mapping.dst = dst;
    mapping.dst_index = dst_index;
    // 两边循环，获得相同组件的列位置映射和移除组件的列位置
    // 计算移动列
    // let start = moved_columns.len();
    // // 获得相同组件的列位置映射
    // for t in moving {
    //     let src_column = mapping.src.get_column_index(t.world_index);
    //     let dst_column = mapping.dst.get_column_index(t.world_index);
    //     moved_columns.push((src_column, dst_column));
    // }
    // mapping.move_indexs = Range {
    //     start,
    //     end: moved_columns.len(),
    // };
    // 计算新增列
    // let start = add_columns.len();
    // if add_len > 0 {
    //     // 新增组件的位置，目标原型组件存在，但源原型上没有该组件
    //     for (i, t) in mapping.dst.get_columns().iter().enumerate() {
    //         let column = mapping.src.get_column_index(t.info().world_index);
    //         if column.is_null() {
    //             add_columns.push(ColumnIndex(i as u16));
    //         }
    //     }
    // }
    // mapping.add_indexs = Range {
    //     start,
    //     end: add_columns.len(),
    // };
    // 计算移除列
    // let start = removed_columns.len();
    // if remove_len > 0 {
    //     // 移除组件的位置，源组件存在，但目标原型上没有该组件
    //     for (i, t) in mapping.src.get_columns().iter().enumerate() {
    //         let column = mapping.dst.get_column_index(t.info().world_index);
    //         if column.is_null() {
    //             // 获取被移除的组件在目标原型的移除列的脏位置，如果没有，则表示该组件无监听
    //             let remove_column_dirty_index =
    //                 mapping.dst.find_remove_column_dirty(t.info().world_index);
    //             removed_columns.push((ColumnIndex(i as u16), remove_column_dirty_index));
    //         }
    //     }
    // }
    // mapping.remove_indexs = Range {
    //     start,
    //     end: removed_columns.len(),
    // };
}

pub(crate) fn alter_row<'w, 'a>(
    mapping_dirtys: &mut Vec<ArchetypeLocalIndex>,
    mapping: &mut ArchetypeMapping,
    ar_index: ArchetypeLocalIndex,
    src_row: Row,
    e: Entity,
) -> Result<Row, QueryError> {
    let e = if !ar_index.0.is_null() {
        let e = mapping.src.mark_remove(src_row);
        if e.is_null() {
            return Err(QueryError::NoSuchRow);
        }
        e
    }else{
        e
    };
    let dst_row = mapping.dst.alloc();
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
    // moved_columns: &Vec<(ColumnIndex, ColumnIndex)>,
    // added_columns: &Vec<ColumnIndex>,
    // removed_columns: &Vec<(ColumnIndex, ColumnIndex)>,
    tick: Tick,
) {
    // 处理标记移除的条目， 将要移除的组件释放，将相同的组件拷贝
    for ar_index in mapping_dirtys.iter() {
        let am = unsafe { vec.get_unchecked_mut((*ar_index).0 as usize) };
        move_columns(am);
        remove_columns(am);
        add_columns(am, tick);
        update_table_world(world, am);
        am.moves.clear();
    }
    mapping_dirtys.clear();
}
// 将需要移动的全部源组件移动到新位置上
pub(crate) fn move_columns( am: &mut ArchetypeMapping) {
    for t in &am.moving {
        let src_index = am.src.get_column_index(t.world_index);
        let src_column = am.src.get_column_unchecked(src_index);
        let dst_column = if *am.src.id() == am.id {
            src_column
        } else {
            let dst_index = am.dst.get_column_index(t.world_index);
             am.dst.get_column_unchecked(dst_index)
        };
        move_column(src_column, dst_column, &am.moves);
    }
    // for i in am.move_indexs.clone().into_iter() {
    //     let (src_i, dst_i) = unsafe { moved_columns.get_unchecked(i) };
    //     let src_column = am.src.get_column_unchecked(*src_i);
    //     let dst_column = am.dst.get_column_unchecked(*dst_i);
    //     move_column(src_column, dst_column, &am.moves);
    // }
}
// 将源组件移动到新位置上
pub(crate)fn move_column(src_column: &Column, dst_column: &Column, moves: &Vec<(Row, Row, Entity)>) {
    for (src_row, dst_row, _) in moves.iter() {
        let src_data: *mut u8 = src_column.get_row(*src_row);
        dst_column.write_row(*dst_row, src_data);
    }
}
// 将需要移除的全部源组件移除，如果目标原型的移除列上有对应监听，则记录移除行
pub(crate)fn remove_columns(am: &mut ArchetypeMapping) {

    // for i in am.remove_indexs.clone().into_iter() {
    //     let column_index = unsafe { removed_columns.get_unchecked(i) };
    //     let column = am.src.get_column_unchecked(column_index.0);
    //     if !column_index.1.is_null() {
    //         // 如果目标原型的移除列上有对应监听，则记录移除行
    //         let d = am.dst.get_remove_column_dirty(column_index.1);
    //         if column.needs_drop() {
    //             for (src_row, dst_row, e) in am.moves.iter() {
    //                 column.drop_row_unchecked(*src_row);
    //                 // 在脏列表上记录移除行
    //                 d.record_unchecked(*e, *dst_row);
    //             }
    //         } else {
    //             for (_src_row, dst_row, e) in am.moves.iter() {
    //                 // 在脏列表上记录移除行
    //                 d.record_unchecked(*e, *dst_row);
    //             }
    //         }
    //     } else if column.needs_drop() {
    //         for (src_row, _dst_row, _e) in am.moves.iter() {
    //             column.drop_row_unchecked(*src_row)
    //         }
    //     }
    // }
    if am.remove_len == 0{
        return;
    }
    for (i, t) in am.src.get_columns().iter().enumerate() {
        let column = am.dst.get_column_index(t.info().world_index);
        if column.is_null() {
            // 获取被移除的组件在目标原型的移除列的脏位置，如果没有，则表示该组件无监听
            let remove_column_dirty_index =
            am.dst.find_remove_column_dirty(t.info().world_index);
            
            let column = am.src.get_column_unchecked(ColumnIndex(i as u16));
            if !remove_column_dirty_index.is_null() {
                // 如果目标原型的移除列上有对应监听，则记录移除行
                let d = am.dst.get_remove_column_dirty(remove_column_dirty_index);
                if column.needs_drop() {
                    for (src_row, dst_row, e) in am.moves.iter() {
                        column.drop_row_unchecked(*src_row);
                        // 在脏列表上记录移除行
                        d.record_unchecked(*e, *dst_row);
                    }
                } else {
                    for (_src_row, dst_row, e) in am.moves.iter() {
                        // 在脏列表上记录移除行
                        d.record_unchecked(*e, *dst_row);
                    }
                }
            } else if column.needs_drop() {
                for (src_row, _dst_row, _e) in am.moves.iter() {
                    column.drop_row_unchecked(*src_row)
                }
            }
        }
    }
}
// 通知新增的源组件
pub(crate) fn add_columns(am: &mut ArchetypeMapping, tick: Tick) {
    // for i in am.add_indexs.clone().into_iter() {
    //     let column_index = unsafe { add_columns.get_unchecked(i) };
    //     let column = am.dst.get_column_unchecked(*column_index);
    //     if column.dirty.listener_len() == 0 {
    //         continue;
    //     }
    //     for (_, dst_row, e) in am.moves.iter() {
    //         column.add_record_unchecked(*e, *dst_row, tick);
    //     }
    // }
    if am.add_len == 0{
        return;
    }
    for (i, t) in am.dst.get_columns().iter().enumerate() {
        let column_index = am.src.get_column_index(t.info().world_index);
        if column_index.is_null() {
            // let column_index = unsafe { add_columns.get_unchecked(i) };
            let column = am.dst.get_column_unchecked(ColumnIndex(i as u16));
            if column.dirty.listener_len() == 0 {
                continue;
            }
            for (_, dst_row, e) in am.moves.iter() {
                column.add_record_unchecked(*e, *dst_row, tick);
            }
        }
    }
}
// 修改entity上的EntityAddr， table上的entitys也对应记录Entity
pub(crate) fn update_table_world(world: &World, am: &mut ArchetypeMapping) {
    for (_, dst_row, e) in am.moves.iter() {
        am.dst.set(*dst_row, *e);
        world.replace(*e, am.dst_index, *dst_row);
    }
}