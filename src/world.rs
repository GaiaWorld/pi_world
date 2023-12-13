/// system上只能看到Query，Sys参数，资源 实体 组件 事件 命令
/// world上包含了全部的单例和实体，及实体原型。 加一个监听管理器，
/// 查询过滤模块会注册监听器来监听新增的原型
/// world上的数据（单例、实体和原型）的线程安全的保护仅在于保护容器，
/// 由调度器生成的执行图，来保证正确的读写。
/// 比如一个原型不可被多线程同时读写，是由执行图时分析依赖，先执行写的sys，再执行读到sys。
/// 由于sys会进行组件的增删，导致实体对于的原型会变化，执行图可能会产生变化，也可以延迟处理，如果延迟一般是在整个执行图执行完毕后，进行整理操作时，进行相应的调整。举例：
/// SysA会对ArcheA的实例增加一个新的组件CompA，第一次会产生ArcheB，SysB会读取ArcheB.
/// 在开始时，SysA和SysB是并行执行的，当
///
///
/// 如果sys上通过Mutate来增删组件，则可以在entity插入时，分析出sys的依赖。除了首次原型创建时，时序不确定，其余的增删，sys会保证先写后读。
/// 如果sys通过CmdQueue来延迟动态增删组件，则sys就不会因此产生依赖，动态增删的结果就只能在下一帧会看到。
///
///
/// world上tick不是每次调度执行时加1，而是每次sys执行时加1，默认从1开始。
/// 每个sys执行时，会处理比tick不为0，并且比本次执行时tick小的数据。也就是说会过滤掉本次执行变化的Entity.

///
use core::fmt::*;
use core::result::Result;
use std::any::{Any, TypeId};
use std::mem::transmute;
use std::ptr::null_mut;
use std::sync::atomic::Ordering;

use crate::archetype::{Archetype, ArchetypeKey, ComponentInfo, ShareArchetype};
use crate::insert::*;
use crate::listener::{EventListKey, ListenerMgr};
use crate::query::QueryError;
use crate::raw::*;
use dashmap::mapref::one::Ref;
use dashmap::DashMap;
use pi_append_vec::AppendVec;
use pi_key_alloter::new_key_type;
use pi_null::Null;
use pi_share::{Share, ShareUsize};
use pi_slot::*;

new_key_type! {
    pub struct Entity;
}

/// A value that tracks when a system ran relative to other systems.
/// This is used to power change detection.
pub type Tick = usize;

#[derive(Clone, Debug)]
pub struct ArchetypeInit<'a>(pub &'a ShareArchetype);
#[derive(Clone, Debug)]
pub struct ArchetypeOk<'a>(pub &'a ShareArchetype);

#[derive(Debug)]
pub struct World {
    pub(crate) single_map: DashMap<TypeId, Box<dyn Any>>,
    pub(crate) entitys: SlotMap<Entity, EntityValue>,
    pub(crate) archetype_map: DashMap<u128, ShareArchetype>,
    pub(crate) archetype_arr: AppendVec<Option<ShareArchetype>>,
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
            single_map: DashMap::default(),
            entitys: SlotMap::default(),
            archetype_map: DashMap::new(),
            archetype_arr: AppendVec::default(),
            empty_archetype: 
                ShareArchetype::new(Archetype::new(vec![])),
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
    pub fn make_insert_state<I: InsertComponents>(&self) -> (ShareArchetype, I::State) {
        let ar = self.find_archtype(I::components());
        self.archtype_ok(&ar);
        let s = I::init_state(self, &ar);
        (ar, s)
    }
    pub fn empty_archetype(&self) -> &ShareArchetype {
        &self.empty_archetype
    }

    /// 获得指定实体的指定组件
    pub fn get_component<T: 'static>(&self, e: Entity) -> Result<&mut T, QueryError> {
        let value = match self.entitys.get(e) {
            Some(v) => v,
            None => return Err(QueryError::NoSuchEntity),
        };
        let ar = value.get_archetype();
        let tid = TypeId::of::<T>();
        let mem_offset = ar.get_mem_offset_ti_index(&tid).0;
        if mem_offset.is_null() {
            return Err(QueryError::MissingComponent);
        }
        Ok(unsafe { transmute(value.value().add(mem_offset as usize)) })
    }

    pub(crate) fn get_archetype(&self, id: u128) -> Option<Ref<u128, ShareArchetype>> {
        self.archetype_map.get(&id)
    }

    // 返回原型及是否新创建
    pub(crate) fn find_archtype(&self, components: Vec<ComponentInfo>) -> ShareArchetype {
        // 如果world上没有找到对应的原型，则创建并放入world中
        let id = ComponentInfo::calc_id(&components);
        let (ar, b) = match self.archetype_map.entry(id) {
            dashmap::mapref::entry::Entry::Occupied(entry) => (entry.get().clone(), false),
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                let ar = Share::new(Archetype::new(components));
                entry.insert(ar.clone());
                (ar, true)
            }
        };
        if b {
            // 通知原型创建，让各查询过滤模块初始化原型的记录列表
            self.listener_mgr
                .notify_event(self.archetype_init_key, ArchetypeInit(&ar));
            // 通知后，让原型就绪， 其他线程也就可以获得该原型
            ar.ready.store(1, Ordering::Relaxed);
        } else {
            // 循环等待原型就绪
            loop {
                if ar.ready.load(Ordering::Relaxed) > 0 {
                    break;
                }
            }
        }
        ar
    }
    // 先事件通知调度器，将原型放入数组，之后其他system可以看到该原型
    pub(crate) fn archtype_ok(&self, ar: &ShareArchetype) {
        match ar
            .ready
            .compare_exchange(1, 2, Ordering::Relaxed, Ordering::Relaxed)
        {
            Ok(_) => (),
            Err(_) => return,
        }
        self.listener_mgr
            .notify_event(self.archetype_ok_key, ArchetypeOk(&ar));
        let e = self.archetype_arr.insert_entry();
        ar.set_index(e.index() as u32);
        *e.value = Some(ar.clone());
    }
    /// 插入一个新的Entity
    pub(crate) fn insert(
        &self,
        a: &Archetype,
        key: ArchetypeKey,
        data: ArchetypeData,
        tick: Tick,
    ) -> Entity {
        data.set_tick(tick);
        let e = self.entitys.insert(EntityValue::new(a, key, data));
        *data.entity() = e;
        e
    }
    /// 替换Entity的原型及数据
    pub(crate) fn replace(
        &self,
        e: Entity,
        a: &Archetype,
        key: ArchetypeKey,
        data: ArchetypeData,
        tick: Tick,
    ) {
        *data.entity() = e;
        data.set_tick(tick);
        let value = unsafe { self.entitys.load_unchecked(e) };
        *value = EntityValue::new(a, key, data);
    }
    /// 整理方法
    pub fn collect(&self) {
        for ar in self.archetype_arr.iter() {
            ar.as_ref().unwrap().collect(self);
        }
    }
}

#[derive(Debug)]
pub(crate) struct EntityValue(
    pub(crate) *mut Archetype,
    pub(crate) ArchetypeKey,
    pub(crate) ArchetypeData,
);
unsafe impl Sync for EntityValue {}
unsafe impl Send for EntityValue {}
impl EntityValue {
    pub fn new(a: &Archetype, key: ArchetypeKey, data: ArchetypeData) -> Self {
        EntityValue(unsafe { transmute(a) }, key, data)
    }
    pub fn get_archetype(&self) -> &Archetype {
        unsafe { &*self.0 }
    }
    pub fn key(&self) -> ArchetypeKey {
        self.1
    }
    pub fn value(&self) -> ArchetypeData {
        self.2
    }
}
impl Default for EntityValue {
    fn default() -> Self {
        Self(null_mut(), ArchetypeKey::default(), null_mut())
    }
}
