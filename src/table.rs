/// 在表的entitys上， 每row的Entity e有3种状态:
/// !e.is_null()有值是一种。
/// e.is_null() 又分为2种: 
///     !e.data().version()==0表示为初始化状态。
///     e.data().version().is_null()表示为删除状态。
/// 
/// 为了提升性能，表的entitys和world上entitys的操作都是使用Relaxed方式。
/// 
/// 在world上通过外部参数Entity e获取ar_row(原型和行)时， 如果table上row的e为初始化状态，需要循环等待初始化完毕。
/// 在Record迭代中，新增和修改的Row，要通过table上row的e来判断是否被删除，如果e为初始化状态，也需要循环等待初始化完毕。
/// 
/// 在插入时， table上上分配了row，row的e为初始化状态，先写组件及record，然后再写world上entitys的ar_row，最后改table上的e为正确值的Entity。
/// 删除时， 是先改table上的e为删除状态，然后删除world上的entitys的e，最后销毁组件。
/// mutate移动时， 新table上分配了新row，先写移动相同的组件和新增组件及record，再改world上的entitys的ar_row，然后改旧table上row的e为删除状态，接着销毁旧table上的组件。最后改新table上row的e为正确值的Entity。
/// 
/// mutate所操作的源table， 在执行图中，会被严格保证不会同时有其他system进行操作。


use core::fmt::*;

use crate::archetype::{ColumnIndex, Row};
use crate::world::World;
use crate::archetype::ComponentInfo;
use pi_append_vec::AppendVec;

use crate::{
    column::Column,
    world::Entity,
};

pub struct Table {
    pub(crate) entitys: AppendVec<Entity>, // 记录entity
    //pub(crate) ticks: AppendVec<Tick>, // 记录entity创建和原型变动时的tick，tick为Null表示被移除，tick也用来判断是否在同一个system内不同原型的移动
    pub(crate) columns: Vec<Column>,   // 每个组件
    removes: AppendVec<Row>,      // 整理前被移除的实例
}
impl Table {
    pub fn new(infos: Vec<ComponentInfo>) -> Self {
        Self {
            entitys: AppendVec::default(),
            //ticks: AppendVec::new(),
            columns: infos.into_iter().map(|info| Column::new(info)).collect(),
            removes: AppendVec::default(),
        }
    }
    /// 长度
    #[inline(always)]
    pub fn len(&self) -> Row {
        self.entitys.len() as u32
    }
    #[inline(always)]
    pub fn get(&self, row: Row) -> Entity {
       *self.entitys.load_alloc(row as usize, 1)
    }
    #[inline(always)]
    pub fn set(&self, row: Row, e: Entity) {
        let a = self.entitys.load_alloc(row as usize, 1);
        *a = e;
    }

    #[inline(always)]
    pub(crate) fn get_column_unchecked(&self, index: ColumnIndex) -> &Column {
        unsafe { self.columns.get_unchecked(index as usize) }
    }
    #[inline(always)]
    pub fn alloc(&self) -> Row {
        self.entitys.alloc_index(1) as Row
    }
    pub(crate) fn drop_row(&self, row: Row) {
        // *unsafe { self.entitys
        //     .get_unchecked(row as usize) } = Entity::null();
        for t in self.columns.iter() {
            // if let Some(d) = t.drop_fn {
            //     println!("drop_item:1, ptr:{:?},mem_offset:{}", ptr, t.mem_offset);
            //     d(unsafe { ptr.add(t.mem_offset as usize) });
            // }
        }
    }
    /// 整理方法
    pub(crate) fn collect(&self, _world: &World) {
        for c in self.columns.iter() {
            c.collect()
        }
        // todo!()
    }

}

impl Debug for Table {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("Table")
            .field("entitys", &self.entitys)
            .field("columns", &self.columns)
            .field("removes", &self.removes)
            .finish()
    }
}
