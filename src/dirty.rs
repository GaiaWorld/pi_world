//! 脏
//! 为每个Query::Filter的Added和Changed都维护一个单独ShareUsize的AddedIndex和ChangedIndex
//! 并且将AddedIndex和ChangedIndex注册到关联的原型列上
//! AddedIndex和ChangedIndex仅记录本system query读取到的最后位置
//! 每Column用AppendVec脏所有Added和Changed的Row，也不做去重
//! 查询时，每Query::Filter内维护自己的fixedbitset，用来做去重（可去重多次修改相同组件和或修改相同组件）
//! 整理时，如果该Column所关联的所有AddedIndex都和该AppendVec的长度一样，则所有AddedIndex和AppendVec就可以清空，否则继续保留，等下一帧再检查是否清空。 因为必须保证Row不被错误重用，只有清空的情况下，才可以做原型的Removed整理。
//!
use core::fmt::*;
use std::sync::atomic::Ordering;

use pi_append_vec::AppendVec;
use pi_arr::Iter;
use pi_null::Null;
use pi_share::ShareUsize;

use crate::archetype::{ColumnIndex, Row};
use crate::world::{Entity, Tick};

#[derive(Default, Debug, Clone, Copy)]
pub enum DirtyType {
    #[default]
    Destroyed,
    Changed(ColumnIndex), // table.columns组件列的位置
    // Removed(ColumnIndex), // table.remove_columns组件列的位置
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct DirtyIndex {
    pub(crate) dtype: DirtyType,
    pub(crate) listener_index: u32, // 在Dirty的Vec中的位置
}

pub(crate) struct DirtyIter<'a> {
    pub(crate) it: Iter<'a, EntityRow>,
    pub(crate) ticks: Option<&'a AppendVec<Tick>>, // 是否对entity进行校验
}
impl<'a> DirtyIter<'a> {
    pub(crate) fn empty() -> Self {
        Self {
            it: Iter::empty(),
            ticks: None,
        }
    }
    pub(crate) fn new(it: Iter<'a, EntityRow>, ticks: Option<&'a AppendVec<Tick>>) -> Self {
        Self {
            it,
            ticks,
        }
    }
}
#[derive(Debug, Default)]
pub(crate) struct EntityRow {
    pub(crate) e: Entity,
    pub(crate) row: Row,
}
impl Null for EntityRow {
    fn is_null(&self) -> bool {
        self.e.is_null()
    }

    fn null() -> Self {
        EntityRow {
            e: Entity::null(),
            row: Row::null(),
        }
    }
}

// 监听器信息
#[derive(Debug, Default)]
pub struct ListenerInfo {
    owner: Tick, // 监听器id，也是QueryState.id, 由world上分配的唯一tick
    read_len: ShareUsize, // 已读取的长度
    tick: ShareUsize, // 读取时的tick
}
impl ListenerInfo {
    pub fn new(owner: Tick) -> Self {
        Self {
            owner,
            read_len: ShareUsize::new(0),
            tick: ShareUsize::new(0),
        }
    }
}
#[derive(Debug, Default)]
pub struct Dirty {
    listeners: Vec<ListenerInfo>, // 每个监听器的唯一Id和当前读取的长度
    pub(crate) vec: AppendVec<EntityRow>,            // 记录的脏Row，不重复
    pub(crate) min_tick: Tick,            // 所有监听器的最小tick，用来快速剔除记录
}
unsafe impl Sync for Dirty {}
unsafe impl Send for Dirty {}
impl Dirty {
    /// 插入一个监听者的类型id
    pub(crate) fn insert_listener(&mut self, owner: Tick) {
        // println!("insert_listener!!! self: {:p}", self);
		self.listeners.push(ListenerInfo::new(owner));
        self.min_tick = 0usize.into();
    }
    /// 插入一个监听者的类型id
    pub(crate) fn listener_list(&self) -> &Vec<ListenerInfo> {
        &self.listeners
    }
    // 返回监听器的位置
    pub fn find_listener_index(&self, owner: Tick) -> u32 {
        self.listener_list()
            .iter()
            .enumerate()
            .find(|r| r.1.owner == owner)
            .map_or(u32::null(), |r| r.0 as u32)
    }
    #[inline(always)]
    pub(crate) fn listener_len(&self) -> usize {
        self.listener_list().len()
    }
    #[inline(always)]
    pub(crate) fn record_unchecked(&self, e: Entity, row: Row) {
        self.vec.insert(EntityRow { e, row });
    }
    #[inline(always)]
    pub(crate) fn record(&self, e: Entity, row: Row, tick: Tick) {
        // println!("record!!! self: {:p}, listener_index: {:?}", self, (e, row, self.listener_list().len()));
        if tick > self.min_tick {
            self.vec.insert(EntityRow { e, row });
        }
    }
    #[inline(always)]
    pub fn reserve(&mut self, additional: usize) {
        if self.listener_len() > 0 {
            self.vec.reserve(additional);
        }
    }
    pub(crate) fn get_iter<'a>(&'a self, listener_index: u32, tick: Tick) -> Iter<'a, EntityRow> {
        // println!("get_iter!!! self: {:p}, listener_index: {:?}", self, (listener_index, self.listener_list().len(), self.vec.len()));
        let end = self.vec.len();
        // 从上次读取到的位置开始读取
        let info = unsafe {
            &self
                .listener_list()
                .get_unchecked(listener_index as usize)
        };
        let start = info.read_len.swap(end, Ordering::Relaxed);
        if !tick.is_null() {
            info.tick.store(tick.index(), Ordering::Relaxed);
        }
        self.vec.slice(start..end)
    }
    /// 判断是否能够清空脏列表
    pub(crate) fn can_clear(&mut self) -> Option<usize> {
        let len = self.vec.len();
        if len == 0 {
            return Some(0);
        }
        let mut min_tick = Tick::max();
        let mut can = true;
        for info in self.listeners.iter_mut() {
            min_tick = min_tick.min(info.tick.load(Ordering::Relaxed).into());
            if *info.read_len.get_mut() < len {
                can = false;
            }
        }
        self.min_tick = min_tick;
        // 只有所有的监听器都读取了全部的脏列表，才可以清空脏列表
        if can {
            Some(len)
        }else{
            None
        }
    }
    /// 清理方法
    pub(crate) fn clear(&mut self, len: usize) {
        self.vec.clear();
        // 以前用到了arr，所以扩容
        if self.vec.vec_capacity() < len {
            unsafe { self.vec.vec_reserve(len - self.vec.vec_capacity()) };
        }
        for info in self.listeners.iter_mut() {
            *info.read_len.get_mut() = 0;
        }
    }
    // 整理方法， 返回是否已经将脏列表清空，只有所有的监听器都读取了全部的脏列表，才可以清空脏列表
    pub(crate) fn collect(&mut self) -> bool {
        if self.listeners.is_empty() {
            return true;
        }
        match self.can_clear() {
            Some(len) => {
                if len > 0 {
                    self.clear(len);
                }
                true
            }
            _ => false,
        }
    }
}
#[inline(always)]
pub(crate) const fn encode(low: u32, high: u32) -> u64 {
    (low as u64) | ((high as u64) << 32)
}
#[inline(always)]
pub(crate) const fn decode(value: u64) -> (u32, u32) {
    let low = value & 0xffff_ffff;
    let high = value >> 32;
    (low as u32, high as u32)
}