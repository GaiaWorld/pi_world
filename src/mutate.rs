/// 组件突变，增加和删除组件
/// 如果原型上没有要删除的组件，则自动忽略
/// 要求entity的原型，必须在system_meta的写原型中， 这样保证有正确的写依赖
/// 根据删除时entity的原型，动态查找和创建删除后对应的原型，并存储在system_meta上。
/// 删除组件会在源原型对entity进行标记删除，如果system后续迭代和查找是找不到的，但如果有引用，则引用还是可以继续读写。
/// 目标原型会立即添加新的条目，但该条目的tick是0，表示是看不到的。这时添加的组件，会立即写到目标原型对应的位置，等突变结束时，会将标记删除的entity，誊写到目标原型上，并修改tick。
/// 目标原型有可能和本system的原型重合，但由于mutate是延迟的，也不会有引用被改写的问题
use std::any::TypeId;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::Range;

use pi_null::Null;
use pi_proc_macros::all_tuples;
use pi_share::Share;

use crate::archetype::*;
use crate::raw::{ArchetypeData, ArchetypePtr};
use crate::system::SystemMeta;
use crate::system_parms::SystemParam;
use crate::world::*;
use crate::insert::TState;

#[derive(Debug)]
pub enum MutateError {
    MissingWriteAccess,
    NoSuchEntity,
    NoSuchArchetype,
    AlreadyMutated,
}

pub struct Mutate<'world, A: AddComponents + 'static, D: DelComponents + 'static = ()> {
    world: &'world World,
    meta: &'world SystemMeta,
    state: &'world mut MutateState<A>,
    tick: Tick,
    _k: PhantomData<D>,
}

impl<'world, A: AddComponents, D: DelComponents> Mutate<'world, A, D> {
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
        components: <A as AddComponents>::Item,
    ) -> Result<bool, MutateError> {
        let (key, aid) = MutateState::<A>::check(&self.meta, &self.world, e)?;
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
        let mut data = MutateState::<A>::mutate(
            self.world,
            e,
            &mut self.state.vec,
            &mut self.state.removes,
            self.state.last.1,
            key,
            self.tick,
        );
        A::add(
            &self.state.state_vec[self.state.last.1 as usize],
            components,
            &mut data,
        );
        Ok(true)
    }
}
// SAFETY: Relevant Insert ComponentId and ArchetypeComponentId access is applied to SystemMeta. If
// this Insert conflicts with any prior access, a panic will occur.
impl<A: AddComponents + 'static, D: DelComponents + 'static> SystemParam for Mutate<'_, A, D> {
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
    move_indexs: Range<usize>,                  // 源原型和目标原型的组件映射的起始和结束位置
    add_indexs: Range<usize>,                   // 目标原型上新增的组件的起始和结束位置
    del_indexs: Range<usize>,                   // 源原型上被删除的组件的起始和结束位置
    removes: Vec<(ArchetypeKey, ArchetypeKey)>, // 本次标记移除的条目
}
impl ArchetypeMapping {
    pub fn new(
        src: ShareArchetype,
        dst: ShareArchetype,
        move_indexs: Range<usize>,
        add_indexs: Range<usize>,
        del_indexs: Range<usize>,
    ) -> Self {
        ArchetypeMapping {
            src,
            dst,
            move_indexs,
            add_indexs,
            del_indexs,
            removes: Default::default(),
        }
    }
}
pub struct MutateState<A: AddComponents> {
    map: HashMap<u128, u32>,                // 记录源原型对应的本地映射的位置
    vec: Vec<ArchetypeMapping>,             // 记录所有的原型映射
    state_vec: Vec<A::State>, // 记录所有的原型状态，本突变新增组件在目标原型的状态（新增组件的偏移）
    move_mem_offsets: Vec<(MemOffset, MemOffset, u32)>, // 源目标原型的组件内存位置映射及内存长度列表
    add_indexs: Vec<ComponentIndex>,        // 目标原型的新增加的组件位置列表，主要是给AddFilter用的
    del_mem_offsets: Vec<ComponentIndex>,        // 源原型的被删除的组件内存位置列表
    last: (u128, u32), // 最近一次的源原型对应的vec的原型映射的索引 // todo!() 改到Mutate上来支持并发
    removes: Vec<u32>, // 本次突变的原型映射在vec上的索引// todo!() 改成AppendVec来支持并发
    archetype_len: usize, // 记录的本次最新的原型，如果有更新的，则更新到SystemMeta上
}
impl<A: AddComponents> MutateState<A> {
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
    // 计算源和目标原型，哪些组件是一样，一样就需要获得内存位置映射。哪些组件是新增或删除的
    pub(crate) fn mapping<'a>(
        map: &'a mut HashMap<u128, u32>,
        vec: &'a mut Vec<ArchetypeMapping>,
        move_mem_offsets: &'a mut Vec<(MemOffset, MemOffset, u32)>,
        add_indexs: &'a mut Vec<ComponentIndex>,
        del_mem_offsets: &'a mut Vec<MemOffset>,
        world: &'a World,
        src_aid: u128,
        add: Vec<ComponentInfo>,
        del: Vec<TypeId>,
    ) -> (u32, ShareArchetype) {
        let add_len = add.len();
        let del_len = del.len();
        let src = world.get_archetype(src_aid).unwrap().clone();
        // 如果本地没有找到，则创建components，去world上查找或创建
        let (components, moving) = src.mutate(add, del);
        // 有可能和本system的ar重合，由于mutate是延迟的，也不会有引用被改写的问题
        let dst = world.find_archtype(components);
        let vec_index = vec.len() as u32;
        map.insert(*dst.get_id(), vec_index);
        // 两边循环，获得相同组件的内存位置映射和删除组件的内存位置
        let move_indexs = {
            let start = move_mem_offsets.len();
            if !Share::ptr_eq(&src, &dst) {
                // 获得相同组件的内存位置映射
                for t in moving {
                    let info = src.get_type_info(&t).unwrap();
                    let mem_offset = dst.get_mem_offset_ti_index(&t).0;
                    move_mem_offsets.push((info.mem_offset, mem_offset, info.mem_size));
                }
            } else {
                // 同原型内移动
                for t in moving {
                    let info = src.get_type_info(&t).unwrap();
                    move_mem_offsets.push((info.mem_offset, info.mem_offset, info.mem_size));
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
                for (i, t) in dst.get_type_infos().iter().enumerate() {
                    let mem_offset = src.get_mem_offset_ti_index(&t.type_id).0;
                    if mem_offset.is_null() {
                        add_indexs.push(i as u32);
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
                for t in src.get_type_infos().iter() {
                    let mem_offset = dst.get_mem_offset_ti_index(&t.type_id).0;
                    if mem_offset.is_null() {
                        del_mem_offsets.push(t.mem_offset);
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
            move_indexs,
            add_indexs,
            del_indexs,
        ));
        (vec_index, dst)
    }
    pub(crate) fn mutate<'w>(
        world: &'w World,
        e: Entity,
        vec: &mut Vec<ArchetypeMapping>,
        removes: &mut Vec<u32>,
        index: u32,
        src_key: ArchetypeKey,
        tick: Tick,
    ) -> ArchetypeData {
        let am = &mut vec[index as usize];
        let (dst_key, data) = am.dst.alloc();
        // 记录移动条目的源位置和目标位置
        am.removes.push((src_key, dst_key));
        if am.removes.len() == 1 {
            // 如果该映射是首次记录，则记录该映射
            removes.push(index);
        }
        world.replace(e, &am.dst, dst_key, data, tick);
        data
    }
    pub(crate) fn check<'w>(
        meta: &'w SystemMeta,
        world: &'w World,
        e: Entity,
    ) -> Result<(ArchetypeKey, u128), MutateError> {
        let value = match world.entitys.get(e) {
            Some(v) => v,
            None => return Err(MutateError::NoSuchEntity),
        };
        let ar = value.get_archetype();
        // 检查本地system_meta，如果没有写依赖，则不允许突变
        if !meta.write_archetype_map.contains(ar.get_id()) {
            return Err(MutateError::MissingWriteAccess);
        }
        // 在源原型上标记移除，如果已经标记，则不允许再次标记
        if !ar.remove(value.key()) {
            return Err(MutateError::AlreadyMutated);
        }
        Ok((value.key(), *ar.get_id()))
    }
    // 系统结束后，将突变的条目移动
    pub(crate) fn clear(
        world: &World,
        vec: &mut Vec<ArchetypeMapping>,
        removes: &mut Vec<u32>,
        archetype_len: &mut usize,
        move_mem_offsets: &Vec<(MemOffset, MemOffset, u32)>,
        add_indexs: &Vec<ComponentIndex>,
        del_mem_offsets: &Vec<MemOffset>,
        system_meta: &mut SystemMeta,
        change_tick: Tick,
    ) {
        // 处理标记移除的条目， 将要删除的组件释放，将相同的组件拷贝
        while let Some(map_index) = removes.pop() {
            let am = &mut vec[map_index as usize];
            if Share::ptr_eq(&am.src, &am.dst) {
                // 源原型和目标原型是一个
                while let Some((src_key, dst_key)) = am.removes.pop() {
                    let src_data = am.src.get(src_key);
                    let dst_data = am.src.get(dst_key);
                    Self::move_components(
                        move_mem_offsets,
                        src_data,
                        dst_data,
                        am.move_indexs.clone(),
                        change_tick,
                    );
                }
                continue;
            }
            for (src_key, dst_key) in am.removes.iter() {
                let src_data = am.src.get(*src_key);
                // 在源原型上将删除的组件释放掉
                for i in am.del_indexs.clone().into_iter() {
                    am.src.drop_component(src_data, del_mem_offsets[i]);
                }
                let dst_data = am.dst.get(*dst_key);
                // 将源原型上的组件移动到目标原型上
                Self::move_components(
                    move_mem_offsets,
                    src_data,
                    dst_data,
                    am.move_indexs.clone(),
                    change_tick,
                );
            }
            // 通知新增组件
            for i in am.add_indexs.clone().into_iter() {
                let record = unsafe {
                    let index = *add_indexs.get_unchecked(i);
                    am.dst.get_component_record(index)
                };
                if record.addeds.len() > 0 {
                    record.added_iter(am.removes.iter().map(|(_, dst_key)| *dst_key));
                }
            }
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
    fn move_components(
        move_mem_offsets: &Vec<(MemOffset, MemOffset, u32)>,
        src_data: ArchetypeData,
        dst_data: ArchetypeData,
        range: Range<usize>,
        change_tick: Tick,
    ) {
        // 将源组件移动到新位置上
        for i in range.into_iter() {
            let (src_i, dst_i, len) = move_mem_offsets[i];
            unsafe {
                src_data
                    .add(src_i as usize)
                    .copy_to_nonoverlapping(dst_data.add(dst_i as usize), len as usize)
            };
        }
        // 设置条目的tick
        dst_data.set_tick(change_tick);
    }
}

pub trait AddComponents {

    type Item;

    type State: Send + Sync + Sized;

    fn components() -> Vec<ComponentInfo>;

    fn init_state(world: &World, archetype: &Archetype) -> Self::State;
    fn add(state: &Self::State, components: Self::Item, data: &mut ArchetypeData);
}

pub trait DelComponents {
    fn components() -> Vec<TypeId>;
}

macro_rules! impl_tuple_add_components {
    ($(($name: ident, $state: ident)),*) => {
        #[allow(non_snake_case)]
        #[allow(clippy::unused_unit)]
        impl<$($name: 'static),*> AddComponents for ($($name,)*) {

            type Item = ($($name,)*);
            type State = ($(TState<$name>,)*);

            fn components() -> Vec<ComponentInfo> {
                vec![$(ComponentInfo::of::<$name>(),)*]
            }
            fn init_state(_world: &World, _archetype: &Archetype) -> Self::State {
                ($(TState::new(_archetype.get_mem_offset_ti_index(&TypeId::of::<$name>())),)*)
            }
            fn add(
                _state: &Self::State,
                _components: Self::Item,
                _data: &mut ArchetypeData,
            ) {
                let ($($name,)*) = _components;
                let ($($state,)*) = _state;
                $(
                    {let r = _data.init_component::<$name>($state.0);
                    r.write($name);}
                )*
            }

        }
    };
}
all_tuples!(impl_tuple_add_components, 0, 16, F, S);


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
