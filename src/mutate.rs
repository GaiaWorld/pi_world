/// 组件变更，增加和删除组件
/// 如果原型上没有要删除的组件，则自动忽略
/// 要求entity的原型，必须在system_meta的写原型中， 这样保证有正确的写依赖
/// 根据删除时entity的原型，动态查找和创建删除后对应的原型，并存储在system_meta上。
/// 删除组件会在源原型对entity进行标记删除，如果system后续迭代和查找是找不到的，但如果有引用，则引用还是可以继续读写。
/// 目标原型会立即添加新的条目，但该条目的entity是0，表示是看不到的。这时添加的组件，会立即写到目标原型对应的位置，等变更结束时，会将标记删除的entity，誊写到目标原型上，并修改entity。
/// 目标原型有可能和本system的原型重合，但由于mutate是延迟的，也不会有引用被改写的问题
/// 为了保证多线程读取的安全性，应该先在旧AppendVec每Entity用Relaxed写为0，然后在新Column上写入组件，然后System的after时，先统一fence(Release)提交组件数据，然后每Entity用Relaxed写为正确的值，再统一fence(Release)提交Entity数据。 其他System读取时，统一fence(Acquire)，似乎可以不用Relaxed读Entity？
/// 
/// 最新： 计划使用执行图来保证，mutate不会操作到正在读写的原型。 这样就不用处理各种多线程数据不一致的情况， 比如 entity 读有值， 但组件没有读到正确数据。
/// 每个system的run会根据依赖是否全部写完毕才开始执行，对应的就是一个ShareU32的wait_count数字减到0。
/// 每个system有自身状态ShareU8的run_state，初值为wait。system的run会先执行before，before会先同步world上原型数组的长度，同步后，修改自身状态ShareU8为running，执行完毕后改为ok。
/// 执行图在执行中， 收到A某system产生的原型创建的事件，此时该原型还为放入world上原型数组， 用该原型去匹配所有的system，返回：无依赖、读、写、目标写原型（mutate会根据源原型写到当前存在的目标原型，该目标原型也需要纳入写）。
/// 如果有依赖，则立即将wait_count加1，如果原wait_count=0表示已经开始执行，那么循环等待该system的run_state为running或ok。 这样，对该system要么看到该原型，要么看不到。
/// 

use std::any::TypeId;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::mem::transmute;
use std::ops::Range;

use pi_null::Null;
use pi_proc_macros::all_tuples;
use pi_share::Share;

use crate::archetype::*;
use crate::column::Column;
use crate::system::SystemMeta;
use crate::system_parms::SystemParam;
use crate::world::*;
use crate::insert::InsertComponents;

#[derive(Debug)]
pub enum MutateError {
    MissingWriteAccess,
    NoSuchEntity,
    NoSuchArchetype,
    AlreadyMutated,
}

pub struct Mutate<'world, A: InsertComponents + 'static, D: DelComponents + 'static = ()> {
    world: &'world World,
    meta: &'world SystemMeta,
    state: &'world mut MutateState<A>,
    tick: Tick,
    _k: PhantomData<D>,
}

impl<'world, A: InsertComponents, D: DelComponents> Mutate<'world, A, D> {
    pub fn new(
        world: &'world World,
        meta: &'world SystemMeta,
        state: &'world mut MutateState<A>,
        tick: Tick,
    ) -> Self {
        Mutate {
            world,
            meta,
            state,
            tick,
            _k: PhantomData,
        }
    }
    pub fn mutate(
        &mut self,
        e: Entity,
        components: <A as InsertComponents>::Item,
    ) -> Result<bool, MutateError> {
        let (row, aid) = MutateState::<A>::check(&self.meta, &self.world, e)?;
        if self.state.last.0 != aid {
            // 和上次的原型不一样，更新该原型对应的目标原型，及组件映射
            let vec_index = if let Some(vec_index) = self.state.map.get(&aid) {
                *vec_index
            } else {
                // 如果本地没有找到，则创建components，去world上查找或创建
                let (vec_index, dst) = MutateState::<A>::mapping(
                    &mut self.state.map,
                    &mut self.state.vec,
                    &mut self.state.move_mem_offsets,
                    &mut self.state.add_indexs,
                    &mut self.state.del_mem_offsets,
                    &self.world,
                    aid,
                    A::components(),
                    D::components(),
                );
                self.state
                    .state_vec
                    .push(A::init_state(self.world, &dst));
                vec_index
            };
            self.state.last.0 = aid;
            self.state.last.1 = vec_index;
        }
        let dst_row = MutateState::<A>::mutate(
            self.world,
            e,
            &mut self.state.vec,
            &mut self.state.removes,
            self.state.last.1,
            row,
            self.tick,
        );
        A::insert(
            &self.state.state_vec[self.state.last.1 as usize],
            components,
            dst_row,
        );
        Ok(true)
    }
}
// SAFETY: Relevant Insert ComponentId and ArchetypeComponentId access is applied to SystemMeta. If
// this Insert conflicts with any prior access, a panic will occur.
impl<A: InsertComponents + 'static, D: DelComponents + 'static> SystemParam for Mutate<'_, A, D> {
    type State = MutateState<A>;
    type Item<'w> = Mutate<'w, A, D>;

    fn init_state(_world: &World, _system_meta: &mut SystemMeta) -> Self::State {
        MutateState::new()
    }
    #[inline]
    fn get_param<'world>(
        state: &'world mut Self::State,
        system_meta: &'world SystemMeta,
        world: &'world World,
        change_tick: Tick,
    ) -> Self::Item<'world> {
        // SAFETY: We have registered all of the Insert's world accesses,
        // so the caller ensures that `world` has permission to access any
        // world data that the Insert needs.
        Mutate::new(world, system_meta, state, change_tick)
    }
    fn after(
        state: &mut Self::State,
        system_meta: &mut SystemMeta,
        world: &World,
        change_tick: Tick,
    ) {
        MutateState::<A>::clear(
            world,
            &mut state.vec,
            &mut state.removes,
            &mut state.archetype_len,
            &state.move_mem_offsets,
            &state.add_indexs,
            &state.del_mem_offsets,
            system_meta,
            change_tick,
        );
    }
}

#[derive(Debug)]
pub(crate) struct ArchetypeMapping {
    src: ShareArchetype,                        // 源原型
    dst: ShareArchetype,                        // 映射到的目标原型
    dst_index: WorldArchetypeIndex,             // 目标原型在World原型数组中的位置
    move_indexs: Range<usize>,                  // 源原型和目标原型的组件映射的起始和结束位置
    add_indexs: Range<usize>,                   // 目标原型上新增的组件的起始和结束位置
    del_indexs: Range<usize>,                   // 源原型上被删除的组件的起始和结束位置
    removes: Vec<(Row, Row)>, // 本次标记移除的条目
}
pub type ArchetypeMappingIndex = usize;
impl ArchetypeMapping {
    pub fn new(
        src: ShareArchetype,
        dst: ShareArchetype,
        dst_index: WorldArchetypeIndex,
        move_indexs: Range<usize>,
        add_indexs: Range<usize>,
        del_indexs: Range<usize>,
    ) -> Self {
        ArchetypeMapping {
            src,
            dst,
            dst_index,
            move_indexs,
            add_indexs,
            del_indexs,
            removes: Default::default(),
        }
    }
}
pub struct MutateState<A: InsertComponents> {
    map: HashMap<u128, ArchetypeMappingIndex>,                // 记录源原型对应的本地映射的位置
    vec: Vec<ArchetypeMapping>,             // 记录所有的原型映射
    state_vec: Vec<A::State>, // 记录所有的原型状态，本变更新增组件在目标原型的状态（新增组件的偏移）
    move_mem_offsets: Vec<(ColumnIndex, ColumnIndex)>, // 源目标原型的组件列位置映射列表
    add_indexs: Vec<ColumnIndex>,        // 目标原型的新增加的组件位置列表，主要是给AddFilter用的
    del_mem_offsets: Vec<ColumnIndex>,        // 源原型的被删除的组件列位置列表
    last: (u128, ArchetypeMappingIndex), // 最近一次的源原型对应的vec的原型映射的索引 // todo!() 改到Mutate上来支持并发
    removes: Vec<ArchetypeMappingIndex>, // 本次变更的原型映射在vec上的索引// todo!() 改成AppendVec来支持并发
    archetype_len: usize, // 记录的本次最新的原型，如果有更新的，则更新到SystemMeta上
}
impl<A: InsertComponents> MutateState<A> {
    fn new() -> Self {
        Self {
            map: Default::default(),
            vec: Default::default(),
            state_vec: Vec::new(),
            move_mem_offsets: Default::default(),
            add_indexs: Default::default(),
            del_mem_offsets: Default::default(),
            last: (u128::null(), Default::default()),
            removes: Vec::new(),
            archetype_len: 0,
        }
    }
    // 计算源和目标原型，哪些组件是一样，一样就需要获得列位置映射。哪些组件是新增或删除的
    pub(crate) fn mapping<'a>(
        map: &'a mut HashMap<u128, ArchetypeMappingIndex>,
        vec: &'a mut Vec<ArchetypeMapping>,
        move_mem_offsets: &'a mut Vec<(ColumnIndex, ColumnIndex)>,
        add_indexs: &'a mut Vec<ColumnIndex>,
        del_mem_offsets: &'a mut Vec<ColumnIndex>,
        world: &'a World,
        src_aid: u128,
        add: Vec<ComponentInfo>,
        del: Vec<TypeId>,
    ) -> (ArchetypeMappingIndex, ShareArchetype) {
        let add_len = add.len();
        let del_len = del.len();
        let src = world.get_archetype(src_aid).unwrap().clone();
        // todo! 利用异或的次序无关性， 快速计算新原型的aid，去world上查
        // 如果本地没有找到，则创建components，去world上查找或创建
        let (components, moving) = src.mutate(add, del);
        // 有可能和本system的ar重合，由于mutate是延迟的，也不会有引用被改写的问题
        let (dst_index, dst) = world.find_archtype(components);
        let mapping_index = vec.len();
        map.insert(*dst.get_id(), vec.len());
        // 两边循环，获得相同组件的列位置映射和删除组件的列位置
        let move_indexs = {
            let start = move_mem_offsets.len();
            if !Share::ptr_eq(&src, &dst) {
                // 获得相同组件的列位置映射
                for t in moving {
                    let src_column = src.get_column_index(&t);
                    let dst_column = dst.get_column_index(&t);
                    move_mem_offsets.push((src_column, dst_column));
                }
            } else {
                // 同原型内移动
                for t in moving {
                    let column_index = src.get_column_index(&t);
                    move_mem_offsets.push((column_index, column_index));
                }
            }
            Range {
                start,
                end: move_mem_offsets.len(),
            }
        };
        let add_indexs = {
            let start = add_indexs.len();
            if add_len > 0 && !Share::ptr_eq(&src, &dst) {
                // 新增组件的位置，目标原型组件存在，但源原型上没有该组件
                for (i, t) in dst.get_columns().iter().enumerate() {
                    let column = src.get_column_index(&t.info.type_id);
                    if column.is_null() {
                        add_indexs.push(i as ColumnIndex);
                    }
                }
            }
            Range {
                start,
                end: add_indexs.len(),
            }
        };
        let del_indexs = {
            let start = del_mem_offsets.len();
            if del_len > 0 && !Share::ptr_eq(&src, &dst) {
                // 删除组件的位置，源组件存在，但目标原型上没有该组件
                for (i, t) in src.get_columns().iter().enumerate() {
                    let column = dst.get_column_index(&t.info.type_id);
                    if column.is_null() {
                        del_mem_offsets.push(i as ColumnIndex);
                    }
                }
            }
            Range {
                start,
                end: del_mem_offsets.len(),
            }
        };
        vec.push(ArchetypeMapping::new(
            src,
            dst.clone(),
            dst_index,
            move_indexs,
            add_indexs,
            del_indexs,
        ));
        (mapping_index, dst)
    }
    pub(crate) fn mutate<'w, 'a>(
        world: &'w World,
        e: Entity,
        vec: &'a mut Vec<ArchetypeMapping>,
        removes: &mut Vec<ArchetypeMappingIndex>,
        index: ArchetypeMappingIndex,
        src_row: Row,
        _tick: Tick,
    ) -> Row {
        let am = &mut vec[index as usize];
        let dst_row = am.dst.table.alloc();
        // todo! 设置e
        // 记录移动条目的源位置和目标位置
        am.removes.push((src_row, dst_row));
        if am.removes.len() == 1 {
            // 如果该映射是首次记录，则记录该映射
            removes.push(index);
        }
        world.replace(e, am.dst_index, dst_row);
        dst_row
    }
    pub(crate) fn check<'w>(
        meta: &'w SystemMeta,
        world: &'w World,
        e: Entity,
    ) -> Result<(Row, u128), MutateError> {
        todo!()
        // let value = match world.entitys.get(e) {
        //     Some(v) => v,
        //     None => return Err(MutateError::NoSuchEntity),
        // };
        // let ar = value.get_archetype();
        // // 检查本地system_meta，如果没有写依赖，则不允许变更
        // if !meta.write_archetype_map.contains(ar.get_id()) {
        //     return Err(MutateError::MissingWriteAccess);
        // }
        // // 在源原型上标记移除，如果已经标记，则不允许再次标记
        // if !ar.remove(value.row()) {
        //     return Err(MutateError::AlreadyMutated);
        // }
        // Ok((value.row(), *ar.get_id()))
    }
    // 系统结束后，将变更的条目移动
    pub(crate) fn clear(
        world: &World,
        vec: &mut Vec<ArchetypeMapping>,
        removes: &mut Vec<ArchetypeMappingIndex>,
        archetype_len: &mut usize,
        move_columns: &Vec<(ColumnIndex, ColumnIndex)>,
        add_columns: &Vec<ColumnIndex>,
        del_columns: &Vec<ColumnIndex>,
        system_meta: &mut SystemMeta,
        _change_tick: Tick,
    ) {
        // 处理标记移除的条目， 将要删除的组件释放，将相同的组件拷贝
        while let Some(map_index) = removes.pop() {
            let am = &mut vec[map_index];
            Self::move_columns(am, move_columns);
            Self::delete_columns(am, del_columns);
            Self::add_columns(am, add_columns);
            am.removes.clear();
        }
        if *archetype_len < vec.len() {
            // 将新增的被插入的原型，放入到system_meta上，等后面进行关联分析
            for i in *archetype_len..vec.len() {
                let ar = &vec[i].dst;
                system_meta.write_archetype_map.insert(*ar.get_id());
                // 如果该原型还没有被加入到世界的原型数组中，则事件通知并加入
                world.archtype_ok(ar);
            }
            *archetype_len = vec.len();
        }
    }
    // 将需要移动的全部源组件移动到新位置上
    fn move_columns(
        am: &mut ArchetypeMapping,
        move_columns: &Vec<(ColumnIndex, ColumnIndex)>,
    ) {
        for i in am.move_indexs.clone().into_iter() {
            let (src_i, dst_i) = unsafe { 
            move_columns.get_unchecked(i)};
            let src_column = am.src.table.get_column_unchecked(*src_i);
            let dst_column = am.dst.table.get_column_unchecked(*dst_i);
            Self::move_column(src_column, dst_column, &am.removes);
        }
    }
    // 将源组件移动到新位置上
    fn move_column(
        src_column: &Column,
        dst_column: &Column,
        moves: &Vec<(u32, u32)>,
    ) {
        for (src_row, dst_row) in moves.iter() {
            unsafe {
                let src_data: *mut u8 = transmute(src_column.get_row(*src_row));
                let dst_data: *mut u8 = transmute(dst_column.get_row(*dst_row));
                src_data.copy_to_nonoverlapping(dst_data, src_column.info.mem_size as usize)
            }
        }
    }
    // 将需要删除的全部源组件删除
    fn delete_columns(
        am: &mut ArchetypeMapping,
        del_columns: &Vec<ColumnIndex>,
    ) {
        for i in am.del_indexs.clone().into_iter() {
            let column_index = unsafe { 
                del_columns.get_unchecked(i)};
            let column = am.src.table.get_column_unchecked(*column_index);
            for (src_row, _dst_row) in am.removes.iter() {
                column.remove(*src_row)
            }
        }
    }
    // 通知新增的源组件
    fn add_columns(
        am: &mut ArchetypeMapping,
        add_columns: &Vec<ColumnIndex>,
    ) {
        for i in am.add_indexs.clone().into_iter() {
            let column_index = unsafe { 
                add_columns.get_unchecked(i)};
            let column = am.dst.table.get_column_unchecked(*column_index);
            if column.record.addeds.len() > 0 {
                column.record.added_iter(am.removes.iter().map(|(_, dst_row)| *dst_row));
            }
        }
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
