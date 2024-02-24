/// system上只能看到Query，Sys参数，资源 实体 组件 事件 命令
/// world上包含了全部的单例和实体，及实体原型。 加一个监听管理器，
/// 查询过滤模块会注册监听器来监听新增的原型
/// world上的数据（单例、实体和原型）的线程安全的保护仅在于保护容器，
/// 由调度器生成的执行图，来保证正确的读写。
/// 比如一个原型不可被多线程同时读写，是由执行图时分析依赖，先执行写的sys，再执行读到sys。
/// 由于sys会进行组件的增删，导致实体对于的原型会变化，执行图可能会产生变化，执行图本身保证对原型的访问是安全的读写。
/// 整理操作时，一般是在整个执行图执行完毕后，进行进行相应的调整。举例：
///
/// 如果sys上通过Alter来增删组件，则可以在entity插入时，分析出sys的依赖。除了首次原型创建时，时序不确定，其余的增删，sys会保证先写后读。
/// 如果sys通过CmdQueue来延迟动态增删组件，则sys就不会因此产生依赖，动态增删的结果就只能在下一帧会看到。
///
///
/// world上tick不是每次调度执行时加1，而是每次sys执行时加1，默认从1开始。
///
use core::fmt::*;
use core::result::Result;
use std::any::{Any, TypeId};
use std::sync::atomic::Ordering;

use crate::archetype::{Archetype, ComponentInfo, Row, ShareArchetype, ArchetypeWorldIndex};
use crate::insert::*;
use crate::listener::{EventListKey, ListenerMgr};
use crate::query::QueryError;
use crate::safe_vec::SafeVec;
use dashmap::iter::Iter;
use dashmap::mapref::{entry::Entry, one::Ref};
use dashmap::DashMap;
use fixedbitset::FixedBitSet;
use pi_key_alloter::new_key_type;
use pi_null::Null;
use pi_share::{Share, ShareUsize};
use pi_slot::SlotMap;

new_key_type! {
    pub struct Entity;
}

/// A value that tracks when a system ran relative to other systems.
/// This is used to power change detection.
pub type Tick = usize;

#[derive(Clone, Debug)]
pub struct ArchetypeInit<'a>(pub &'a ShareArchetype, pub &'a World);
#[derive(Clone, Debug)]
pub struct ArchetypeOk<'a>(pub &'a ShareArchetype, pub ArchetypeWorldIndex, pub &'a World);

#[derive(Debug)]
pub struct World {
    pub(crate) _single_map: DashMap<TypeId, Box<dyn Any>>,
    pub(crate) entitys: SlotMap<Entity, EntityAddr>,
    pub(crate) archetype_map: DashMap<u128, ShareArchetype>,
    pub(crate) archetype_arr: SafeVec<ShareArchetype>,
    pub(crate) empty_archetype: ShareArchetype,
    pub(crate) listener_mgr: ListenerMgr,
    archetype_init_key: EventListKey,
    archetype_ok_key: EventListKey,
    change_tick: ShareUsize,
}
impl World {
    pub fn new() -> Self {
        let listener_mgr = ListenerMgr::default();
        let archetype_init_key = listener_mgr.init_register_event::<ArchetypeInit>();
        let archetype_ok_key = listener_mgr.init_register_event::<ArchetypeOk>();
        Self {
            _single_map: DashMap::default(),
            entitys: SlotMap::default(),
            archetype_map: DashMap::new(),
            archetype_arr: SafeVec::default(),
            empty_archetype: ShareArchetype::new(Archetype::new(vec![])),
            listener_mgr,
            archetype_init_key,
            archetype_ok_key,
            change_tick: ShareUsize::new(1),
        }
    }
    pub fn change_tick(&self) -> Tick {
        self.change_tick.load(Ordering::Relaxed)
    }
    pub fn increment_change_tick(&self) -> Tick {
        self.change_tick.fetch_add(1, Ordering::Relaxed)
    }
    /// 创建一个插入器
    pub fn make_inserter<I: InsertComponents>(&self) -> Inserter<I> {
        let components = I::components();
        let id = ComponentInfo::calc_id(&components);
        let (ar_index, ar) = self.find_archtype(id, components);
        let s = I::init_state(self, &ar);
        Inserter::new(self, (ar_index, ar, s))
    }
    pub fn empty_archetype(&self) -> &ShareArchetype {
        &self.empty_archetype
    }

    /// 获得指定实体的指定组件，为了安全，必须保证不在ECS执行中调用
    pub fn get_component<T: 'static>(&self, e: Entity) -> Result<&mut T, QueryError> {
        let addr = match self.entitys.get(e) {
            Some(v) => v,
            None => return Err(QueryError::NoSuchEntity),
        };
        let ar = unsafe { self.archetype_arr.get_unchecked(addr.ar_index()) };
        let tid = TypeId::of::<T>();
        if let Some(c) = ar.get_column(&tid) {
            Ok(c.get_mut(addr.row))
        } else {
            Err(QueryError::MissingComponent)
        }
    }

    pub fn get_archetype(&self, id: u128) -> Option<Ref<u128, ShareArchetype>> {
        self.archetype_map.get(&id)
    }
    pub fn archetype_list<'a>(&'a self) -> Iter<'a, u128, ShareArchetype> {
        self.archetype_map.iter()
    }

    // 返回原型及是否新创建
    pub(crate) fn find_archtype(
        &self,
        id: u128,
        components: Vec<ComponentInfo>,
    ) -> (ArchetypeWorldIndex, ShareArchetype) {
        // 如果world上没有找到对应的原型，则创建并放入world中
        let (ar, b) = match self.archetype_map.entry(id) {
            Entry::Occupied(entry) => (entry.get().clone(), false),
            Entry::Vacant(entry) => {
                let ar = Share::new(Archetype::new(components));
                entry.insert(ar.clone());
                (ar, true)
            }
        };
        if b {
            // 通知原型创建，让各查询过滤模块初始化原型的记录列表
            self.listener_mgr
                .notify_event(self.archetype_init_key, ArchetypeInit(&ar, &self));
            // 通知后，让原型就绪， 其他线程也就可以获得该原型
            let ar_index = self.archtype_ok(&ar);
            self.listener_mgr
            .notify_event(self.archetype_ok_key, ArchetypeOk(&ar, ar_index, &self));
            (ar_index, ar)
        } else {
            // 循环等待原型就绪
            loop {
                let index = ar.index();
                if index.is_null() {
                    std::hint::spin_loop();
                }
                return (index, ar);
            }
        }
    }
    // 先事件通知调度器，将原型放入数组，之后其他system可以看到该原型
    pub(crate) fn archtype_ok(&self, ar: &ShareArchetype) -> ArchetypeWorldIndex {
        let entry = self.archetype_arr.alloc_entry();
        let index = entry.index() as u32;
        ar.index.store(index, Ordering::Relaxed);
        entry.insert(ar.clone()); // 确保其他线程一定可以看见
        index
    }
    /// 插入一个新的Entity
    #[inline(always)]
    pub(crate) fn insert(&self, ar_index: ArchetypeWorldIndex, row: Row) -> Entity {
        self.entitys.insert(EntityAddr::new(ar_index, row))
    }
    /// 替换Entity的原型及行
    #[inline(always)]
    pub(crate) fn replace(&self, e: Entity, ar_index: ArchetypeWorldIndex, row: Row) {
        let addr = unsafe { self.entitys.load_unchecked(e) };
        addr.ar_index = ar_index;
        addr.row = row;
    }
    /// 替换Entity的原型及行
    #[inline(always)]
    pub(crate) fn replace_row(&self, e: Entity, row: Row) {
        let addr = unsafe { self.entitys.load_unchecked(e) };
        addr.row = row;
    }

    /// 只有主调度完毕后，才能调用的整理方法，必须保证调用时没有其他线程读写world
    pub unsafe fn collect(&self, action: &mut Vec<(Row, Row)>,
    set: &mut FixedBitSet) {
        for ar in self.archetype_arr.iter() {
            let archetype = unsafe { Share::get_mut_unchecked(ar) };
            archetype.collect(self, action, set)
        }
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct EntityAddr {
    pub(crate) ar_index: ArchetypeWorldIndex,
    pub(crate) row: Row,
}
unsafe impl Sync for EntityAddr {}
unsafe impl Send for EntityAddr {}

impl EntityAddr {
    #[inline(always)]
    pub fn new(ar_index: ArchetypeWorldIndex, row: Row) -> Self {
        EntityAddr { ar_index, row }
    }
    #[inline(always)]
    pub fn ar_index(&self) -> usize {
        self.ar_index as usize
    }
}
