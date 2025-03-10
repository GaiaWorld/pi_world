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
use std::marker::PhantomData;
use std::mem::{transmute, MaybeUninit};
use std::ops::{Deref, DerefMut, Range};

use pi_null::Null;
use pi_share::Share;

use crate::archetype::*;
use crate::column::{BlobRef, Column};
use crate::fetch::FetchComponents;
use crate::filter::FilterComponents;
use crate::insert::Bundle;
use crate::query::{LocalIndex, Query, QueryError, QueryIter, QueryState};
use crate::system::SystemMeta;
use crate::system_params::SystemParam;
use crate::utils::VecExt;
use crate::world::*;

// // todo 移除
// pub struct Alterer<
//     'w,
//     Q: FetchComponents + 'static,
//     F: FilterComponents + 'static = (),
//     A: Bundle + 'static = (),
//     D: Bundle + 'static = (),
// > {
//     query: Queryer<'w, Q, F>,
//     state: AlterState<A>,
//     is_delay: bool,
//     _k: PhantomData<D>,
// }
// impl<'w, Q: FetchComponents + 'static, F: FilterComponents + 'static, A: Bundle, D: Bundle>
//     Alterer<'w, Q, F, A, D>
// {
//     pub(crate) fn new(
//         world: &'w World,
//         query_state: QueryState<Q, F>,
//         state: AlterState<A>,
//     ) -> Self {
//         Self {
//             query: Queryer::new(world, query_state),
//             state,
//             is_delay: false,
//             _k: PhantomData,
//         }
//     }

//     pub fn contains(&self, entity: Entity) -> bool {
//         self.query.contains(entity)
//     }

//     pub fn get(
//         &'w self,
//         e: Entity,
//     ) -> Result<<<Q as FetchComponents>::ReadOnly as FetchComponents>::Item<'w>, QueryError> {
//         self.query.get(e)
//     }

//     pub fn get_mut(&mut self, e: Entity) -> Result<<Q as FetchComponents>::Item<'_>, QueryError> {
//         self.query.get_mut(e)
//     }

//     pub fn is_empty(&self) -> bool {
//         self.query.is_empty()
//     }

//     pub fn len(&self) -> usize {
//         self.query.len()
//     }

//     pub fn iter(&self) -> QueryIter<'_, <Q as FetchComponents>::ReadOnly, F> {
//         self.query.iter()
//     }

//     pub fn iter_mut(&mut self) -> AlterIter<'_, Q, F, A> {
//         AlterIter {
//             it: self.query.iter_mut(),
//             state: &mut self.state,
//         }
//     }
//     /// 标记销毁实体

//     pub fn destroy(&mut self, e: Entity) -> Result<bool, QueryError> {
//         // self.state
//         self.state.destroy(&self.query.world, e)
//     }

//     pub fn alter(&mut self, e: Entity, components: A) -> Result<bool, QueryError> {
//         let (addr, local_index) = self.state.check_mark(&self.query.world, e)?;
//         self.state.alter(
//             &self.query.world,
//             local_index,
//             e,
//             addr.row,
//             components,
//             self.query.tick,
//         );
//         self.is_delay = false;
//         self.state.state.clear(
//             self.query.world,
//             &mut self.state.vec,
//             &mut self.state.mapping_dirtys,
//         );
//         Ok(true)
//     }

//     pub fn delay_alter(&mut self, e: Entity, components: A) -> Result<bool, QueryError> {
//         let (addr, local_index) = self.state.check_mark(&self.query.world, e)?;
//         self.state.alter(
//             &self.query.world,
//             local_index,
//             e,
//             addr.row,
//             components,
//             self.query.tick,
//         );
//         self.is_delay = true;
//         Ok(true)
//     }
// }

// impl<'w, Q: FetchComponents + 'static, F: FilterComponents + 'static, A: Bundle, D: Bundle> Drop
//     for Alterer<'w, Q, F, A, D>
// {
//     fn drop(&mut self) {
//         if self.is_delay {
//             self.state.state.clear(
//                 self.query.world,
//                 &mut self.state.vec,
//                 &mut self.state.mapping_dirtys,
//             );
//         }
//     }
// }

pub struct Alter<
    'w,
    Q: FetchComponents + 'static,
    F: FilterComponents + 'static = (),
    A: Bundle = (),
    D: Bundle = (),
> {
    query: Query<'w, Q, F>,
    state: &'w mut AlterState<A>,
    _k: PhantomData<D>,
}

unsafe impl<'w, Q: FetchComponents + 'static, F: FilterComponents + 'static, A: Bundle, D: Bundle>
    Send for Alter<'w, Q, F, A, D>
{
}
unsafe impl<'w, Q: FetchComponents + 'static, F: FilterComponents + 'static, A: Bundle, D: Bundle>
    Sync for Alter<'w, Q, F, A, D>
{
}

impl<'w, Q: FetchComponents + 'static, F: FilterComponents + 'static, A: Bundle, D: Bundle>
    Alter<'w, Q, F, A, D>
{
    pub(crate) fn new(query: Query<'w, Q, F>, state: &'w mut AlterState<A>) -> Self {
        Alter {
            query,
            state,
            _k: PhantomData,
        }
    }

    pub fn contains(&self, entity: Entity) -> bool {
        self.query.contains(entity)
    }

    pub fn get(
        &self,
        e: Entity,
    ) -> Result<<<Q as FetchComponents>::ReadOnly as FetchComponents>::Item<'_>, QueryError> {
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
        // self.state
        self.state.destroy(&self.query.world, e)
    }

    pub fn alter(&mut self, e: Entity, components: A) -> Result<bool, QueryError> {
        let (addr, local_index) = self.state.check(&self.query.world, e)?;
        // log::error!("Alert: {:?}", (self.state.bundle_vec.capacity(), self.state.adding.capacity(), self.state.sorted_add_removes.capacity(), self.state.mapping_dirtys.capacity(), self.state.moving.capacity(), self.state.vec.capacity()));
        self.state.alter(
            &self.query.world,
            local_index,
            e,
            addr,
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
    type State = QueryAlterState<Q, F, A, D>;
    type Item<'w> = Alter<'w, Q, F, A, D>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        let q = Query::init_state(world, system_meta);
        QueryAlterState(
            q,
            AlterState::make(world, A::components(Vec::with_capacity(256)), D::components(Vec::with_capacity(256))),
            PhantomData,
        )
    }
    fn align(world: &World, _system_meta: &SystemMeta, state: &mut Self::State) {
        state.0.align(world);
    }

    fn get_param<'w>(
        world: &'w World,
        _system_meta: &'w SystemMeta,
        state: &'w mut Self::State,
        tick: Tick,
    ) -> Self::Item<'w> {
        // 将新多出来的原型，创建原型空映射
        state.1.align(world, &state.0.archetypes);
        Alter::new(Query::new(world, &mut state.0, tick), &mut state.1)
    }

    fn get_self<'w>(
        world: &'w World,
        system_meta: &'w SystemMeta,
        state: &'w mut Self::State,
        tick: Tick,
    ) -> Self {
        unsafe { transmute(Self::get_param(world, system_meta, state, tick)) }
    }
}

impl<'w, Q: FetchComponents + 'static, F: FilterComponents + 'static, A: Bundle, D: Bundle> Drop
    for Alter<'w, Q, F, A, D>
{
    fn drop(&mut self) {
        self.state.state.clear(
            self.query.world,
            &mut self.state.vec,
            &mut self.state.mapping_dirtys,
        );
    }
}

pub struct QueryAlterState<
    Q: FetchComponents + 'static,
    F: FilterComponents + 'static,
    A: Bundle,
    D: Bundle,
>(
    pub(crate) QueryState<Q, F>,
    pub(crate) AlterState<A>,
    pub(crate) PhantomData<D>,
);
unsafe impl<Q: FetchComponents + 'static, F: FilterComponents + 'static, A: Bundle, D: Bundle> Send
    for QueryAlterState<Q, F, A, D>
{
}
unsafe impl<Q: FetchComponents + 'static, F: FilterComponents + 'static, A: Bundle, D: Bundle> Sync
    for QueryAlterState<Q, F, A, D>
{
}

impl<Q: FetchComponents + 'static, F: FilterComponents + 'static, A: Bundle, D: Bundle>
    QueryAlterState<Q, F, A, D>
{
    pub fn get_param<'w>(&'w mut self, world: &'w World) -> Alter<'_, Q, F, A, D> {
        Alter::new(Query::new(world, &mut self.0, world.tick()), &mut self.1)
    }
}

pub struct AlterState<A: Bundle> {
    bundle_vec: Vec<MaybeUninit<A::Item>>, // 记录所有的原型状态，本变更新增组件在目标原型的状态（新增组件的偏移）
    pub(crate) vec: Vec<ArchetypeMapping>, // 记录所有的原型映射
    mapping_dirtys: Vec<LocalIndex>,       // 本次变更的原型映射在vec上的索引
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
            bundle_vec: Vec::with_capacity(256),
            vec: Vec::with_capacity(256),
            mapping_dirtys: Vec::with_capacity(256),
            state,
        }
    }
    // 将新多出来的原型，创建原型空映射
    pub(crate) fn align(&mut self, world: &World, archetypes: &Vec<ShareArchetype>) {
        // 将新多出来的原型，创建原型空映射
        for i in self.vec.len()..archetypes.len() {
            let ar = unsafe { archetypes.get_unchecked(i).clone() };
            self.push_archetype(world, ar);
        }
    }

    pub(crate) fn push_archetype(&mut self, world: &World, ar: ShareArchetype) {
        self.state.push_map(ar.index(), self.vec.len());
        self.vec
            .push(ArchetypeMapping::new(ar, world.empty_archetype().clone()));
        self.bundle_vec.push(MaybeUninit::uninit());
    }

    pub(crate) fn alter<'w>(
        &mut self,
        world: &'w World,
        ar_index: LocalIndex,
        e: Entity,
        addr: &mut EntityAddr,
        components: A,
        tick: Tick,
    ) -> Result<bool, QueryError> {
        let mapping = unsafe { self.vec.get_unchecked_mut(ar_index.index()) };
        // println!("alter: {:?}", (e, src_row, ar_index));
        let (is_new, _new_ar) = self.state.find_mapping(world, mapping, false);
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
            A::insert(item, components, e, addr.row, tick);
            return Ok(false);
        }
        // 判断地址是否已经标记移动了，不允许一个system内修改一个entity原型2次
        // mark标记在Alter Drop时，被clear方法replace entity的地址时被清除
        if addr.is_mark() {
            return Err(QueryError::RepeatAlter);
        } else {
            addr.mark();
        }
        let (_, dst_row) = mapping.dst.alloc();
        // println!("alter: {:?}", (e, src_row, dst_row, &mapping.dst));
        let item = unsafe {
            self.bundle_vec
                .get_unchecked(ar_index.index())
                .assume_init_ref()
        };
        A::insert(item, components, e, dst_row.into(), tick);
        // 记录移除行
        mapping.push(
            addr.row,
            dst_row.into(),
            e,
            ar_index,
            &mut self.mapping_dirtys,
        );
        // if new_ar {
        //     Some((&mapping.dst, mapping.dst_index))
        // } else {
        //     None
        // }
        Ok(true)
    }
}

#[derive(Debug)]
pub struct AState {
    map: Vec<LocalIndex>, // 本地mapping映射的索引
    map_start: usize,
    sorted_add_removes: Vec<(ComponentIndex, bool)>,
    pub(crate) adding: Vec<Share<Column>>, // 所有映射添加的列
    moving: Vec<Share<Column>>,            // 所有映射移动的列
    removing: Vec<Share<Column>>,          // 所有映射移除的列
}
impl AState {
    pub(crate) fn make(
        world: &mut World,
        add: Vec<ComponentInfo>,
        remove: Vec<ComponentInfo>,
    ) -> Self {
        let mut sorted_add_removes = Vec::with_capacity(256);
        world.add_component_indexs(add, &mut sorted_add_removes, true);
        world.add_component_indexs(remove, &mut sorted_add_removes, false);
        sorted_add_removes.sort_unstable();
        Self::new(sorted_add_removes)
    }

    pub(crate) fn new(sorted_add_removes: Vec<(ComponentIndex, bool)>) -> Self {
        Self {
            map: Default::default(),
            map_start: 0,
            sorted_add_removes,
            adding: Default::default(),
            moving: Default::default(),
            removing: Default::default(),
            // removed_columns: Default::default(),
        }
    }
    // 放入本地映射
    pub(crate) fn push_map(&mut self, index: ArchetypeIndex, len: usize) {
        if len == 0 {
            self.map_start = index.index();
        }
        // println!("push_map: {:?}", (index, len, self.map_start));
        let index = index.index() - self.map_start;
        self.map.insert_value(index, len.into());
    }
    // 计算源和目标原型，哪些组件是一样，一样就需要获得列位置映射。哪些组件是新增或移除的
    pub(crate) fn clear<'a>(
        &self,
        world: &'a World,
        vec: &mut Vec<ArchetypeMapping>, // 记录所有的原型映射
        mapping_dirtys: &mut Vec<LocalIndex>,
    ) {
        // 处理标记移除的条目， 将要移除的组件释放，将相同的组件拷贝
        for ar_index in mapping_dirtys.drain(..) {
            let am = unsafe { vec.get_unchecked_mut(ar_index.index()) };
            // 检查是否有destroy
            for i in (0..am.moves.len()).rev() {
                let (src_row, dst_row, e) = unsafe { am.moves.get_unchecked(i) };
                if src_row.is_null() {
                    continue;
                }
                let old = am.src.mark_remove(*src_row);
                if old.is_null() {
                    // 已经被destroy
                    // 目标原型上移除该行
                    self.destroy_add_columns(am, *dst_row, *e);
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
    /// 目标原型上移除该行， 并且销毁add的列
    pub(crate) fn destroy_add_columns(&self, am: &ArchetypeMapping, dst_row: Row, e: Entity) {
        for index in am.add_indexs.clone() {
            let c = unsafe { self.adding.get_unchecked(index) };
            if c.info().drop_fn.is_some() {
                let column = c.blob_ref_unchecked(am.dst_index);
                column.drop_row_unchecked(dst_row, e);
            }
        }
        am.dst.removes.insert(dst_row);
    }

    // 将需要移动的全部源组件移动到新位置上
    pub(crate) fn move_columns(&self, am: &mut ArchetypeMapping) {
        for index in am.move_indexs.clone() {
            let c = unsafe { self.moving.get_unchecked(index) };
            let src_column = c.blob_ref_unchecked(am.src.index());
            let dst_column = c.blob_ref_unchecked(am.dst.index());
            Self::move_column(src_column, dst_column, &am.moves, c.info().is_tick());
        }
    }
    // 将源组件移动到新位置上
    pub(crate) fn move_column<'a>(
        src_column: BlobRef<'a>,
        dst_column: BlobRef<'a>,
        moves: &Vec<(Row, Row, Entity)>,
        is_tick: bool,
    ) {
        for (src_row, dst_row, e) in moves.iter() {
            let src_data: *mut u8 = src_column.get_row(*src_row, *e);
            dst_column.write_row(*dst_row, *e, src_data);
        }
        if is_tick {
            for (src_row, dst_row, _e) in moves.iter() {
                let tick = src_column.get_tick_unchecked(*src_row);
                dst_column.set_tick_unchecked(*dst_row, tick);
            }
        }
    }
    // 将需要移除的全部源组件移除，如果目标原型的移除列上有对应监听，则记录移除行
    pub(crate) fn remove_columns(&self, am: &mut ArchetypeMapping) {
        for i in am.removed_indexs.clone().into_iter() {
            let c = unsafe { self.removing.get_unchecked(i) };
            if c.info().drop_fn.is_some() {
                let column = c.blob_ref_unchecked(am.src.index());
                for (src_row, _dst_row, e) in am.moves.iter() {
                    // println!("drop_row_unchecked====={:?}", (c.info.type_name(), i, am.src.index(), _e,  src_row));
                    column.drop_row_unchecked(*src_row, *e)
                }
            }
            // 如果移除列上有对应监听，则记录移除行
            if let Some(record) = &c.info.removed {
                for (_src_row, _dst_row, e) in am.moves.iter() {
                    // 记录移除实体
                    record.record(*e);
                }
            }
        }
    }
    // 计算源和目标原型，哪些组件是一样，一样就需要获得列位置映射。哪些组件是新增或移除的
    pub(crate) fn find_mapping<'a>(
        &mut self,
        world: &'a World,
        mapping: &mut ArchetypeMapping,
        existed_adding_is_move: bool,
    ) -> (bool, bool) {
        // println!("find_mapping: {:?}", (ar_index, mapping.src.index(), mapping.dst.index(), mapping.dst_index));
        if !mapping.dst_index.is_null() {
            return (false, false);
        }
        let add_start: usize = self.adding.len();
        let move_start = self.moving.len();
        let removing_start = self.removing.len();
        // 如果本地没有找到，则创建components，去world上查找或创建
        let info = mapping.src.alter(
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
        // 有可能和本system的ar重合，转成立地修改，由于alter是有可写引用的，也不会有引用被改写的问题
        if info.id == mapping.src.id() {
            // 同原型内移动，由于bundle_vec的对应位置还未初始化，所以is_new应为true
            mapping.dst = mapping.src.clone();
            mapping.dst_index = mapping.src.index();
            return (true, false);
        }
        let dst = world.find_archtype(info);
        mapping.dst_index = dst.index();
        mapping.dst = dst;
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
        // println!("alter_row: {:?}", (&mapping.dst_index, mapping.src.index, src_row, dst_row, e));
        if !src_row.is_null() {
            mapping.src.mark_remove(src_row);
            mapping.move_columns(src_row, dst_row, e, &self.moving);
            mapping.remove_columns(src_row, e, &self.removing);
        }
        // 写目标行的Entity
        mapping.dst.set(dst_row, e);
        // 更改entity上存的EntityAddr
        world.replace(e, mapping.dst_index, dst_row);
    }
    /// 销毁实体
    fn destroy(&self, world: &World, e: Entity) -> Result<bool, QueryError> {
        let (addr, _local_index) = self.check(world, e)?;
        if addr.row.is_null() {
            world.entities.remove(e).unwrap();
            return Ok(true);
        }
        let ar = unsafe { world.get_archetype_unchecked(addr.archetype_index()) };
        Self::destroy_row(world, ar, addr.row)
    }
    /// 销毁
    pub(crate) fn destroy_row(world: &World, ar: &Archetype, row: Row) -> Result<bool, QueryError> {
        let e = ar.destroy(row);
        if e.is_null() {
            return Err(QueryError::NoSuchRow(row));
        }
        world.entities.remove(e).unwrap();
        Ok(true)
    }
    // // 检查entity是否正确，包括对应的原型是否在本查询内，并将查询到的原型本地位置记到cache_mapping上
    // pub(crate) fn check_mark<'w>(
    //     &self,
    //     world: &'w World,
    //     entity: Entity,
    // ) -> Result<(&'w mut EntityAddr, LocalIndex), QueryError> {
    //     let (addr, local_index) = self.check(world, entity)?;
    //     if addr.is_mark() {
    //         return Err(QueryError::RepeatAlter);
    //     } else {
    //         addr.mark();
    //     }
    //     Ok((addr, local_index))
    // }
    // 检查entity是否正确，包括对应的原型是否在本查询内，并将查询到的原型本地位置记到cache_mapping上
    pub(crate) fn check<'w>(
        &self,
        world: &'w World,
        entity: Entity,
    ) -> Result<(&'w mut EntityAddr, LocalIndex), QueryError> {
        // assert!(!entity.is_null());
        let addr = match world.entities.load(entity) {
            Some(v) => v,
            None => return Err(QueryError::NoSuchEntity(entity)),
        };
        // println!("addr======{:?}", (entity, addr));
        let index = addr.archetype_index().index().wrapping_sub(self.map_start);
        match self.map.get(index) {
            Some(v) if !v.is_null() => Ok((addr, *v)),
            _ => Err(QueryError::NoMatchArchetype),
        }
    }
}

#[derive(Debug)]
pub struct ArchetypeMapping {
    pub(crate) src: ShareArchetype,            // 源原型
    pub(crate) dst: ShareArchetype,            // 映射到的目标原型
    pub(crate) dst_index: ArchetypeIndex,      // 目标原型在World原型数组中的位置
    pub(crate) add_indexs: Range<usize>,       // 目标原型上新增的组件的起始和结束位置
    pub(crate) move_indexs: Range<usize>,      // 源原型和目标原型的组件映射的起始和结束位置
    pub(crate) removed_indexs: Range<usize>,   // 源原型上被移除的组件的起始和结束位置
    pub(crate) moves: Vec<(Row, Row, Entity)>, // 本次标记移动的条目
}

impl ArchetypeMapping {
    pub fn new(src: ShareArchetype, dst: ShareArchetype) -> Self {
        ArchetypeMapping {
            src,
            dst,
            dst_index: ArchetypeIndex::null(),
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
        ar_index: LocalIndex,
        mapping_dirtys: &mut Vec<LocalIndex>,
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
        moving: &Vec<Share<Column>>,
    ) {
        for index in self.move_indexs.clone() {
            let c = unsafe { moving.get_unchecked(index) };
            let src_column = c.blob_ref_unchecked(self.src.index());
            let dst_column = c.blob_ref_unchecked(self.dst.index());
            self.move_column(
                src_row,
                dst_row,
                e,
                src_column,
                dst_column,
                c.info().is_tick(),
            );
        }
    }
    // 将源组件移动到新位置上
    pub(crate) fn move_column<'a>(
        &self,
        src_row: Row,
        dst_row: Row,
        e: Entity,
        src_column: BlobRef<'a>,
        dst_column: BlobRef<'a>,
        is_tick: bool,
    ) {
        let src_data: *mut u8 = src_column.get_row(src_row, e);
        dst_column.write_row(dst_row, e, src_data);
        if is_tick {
            let tick = src_column.get_tick_unchecked(src_row);
            dst_column.set_tick_unchecked(dst_row, tick);
        }
    }
    pub(crate) fn remove_columns(&self, src_row: Row, e: Entity, removing: &Vec<Share<Column>>) {
        for i in self.removed_indexs.clone().into_iter() {
            let c = unsafe { removing.get_unchecked(i) };
            if c.info().drop_fn.is_some() {
                let src_column = c.blob_ref_unchecked(self.src.index());
                src_column.drop_row_unchecked(src_row, e);
            }
            // 如果移除列上有对应监听，则记录移除实体
            if let Some(record) = &c.info.removed {
                record.record(e);
            }
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
        let addr = self.it.world.entities.load(self.it.e).unwrap();
        // let (addr, _) =self.state.check(&self.it.world, self.it.e)?;
        self.state.alter(
            &self.it.world,
            self.it.ar_index,
            self.it.e,
            addr,
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
