use crate::prelude::World;

#[derive(Debug, Clone)]
pub struct ArchetypeDebug {
    pub entitys: Option<usize>,   // 原型的对应的实体数量
    pub columns_info: Vec<Option<ColumnDebug>>,   // 原型的对应的列数量
    pub destroys_listeners: Option<usize>,
    pub removes: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct ColumnDebug {
    pub change_listeners: usize,   // 原型的对应的实体数量
    pub name: Option<&'static str>
}

impl World{
    pub fn assert_archetype_arr(&self, archetypes_info: &[Option<ArchetypeDebug>]){
        if !archetypes_info.is_empty(){
            assert_eq!(archetypes_info.len(), self.archetype_arr.len(), "{:?}", self.archetype_arr);
        }
        
        for i in 0..archetypes_info.len(){
            if let Some(expect) = &archetypes_info[i]{
                let real = &self.archetype_arr[i];

                if let Some(entitys) = expect.entitys{
                    assert_eq!(entitys, real.len().0 as usize, ":{:?}", real);
                }
    
                if !expect.columns_info.is_empty(){
                    assert_eq!(expect.columns_info.len(), real.column_len(), "{:?}", real);
                }

                for j in 0..expect.columns_info.len(){
                    if let Some(expect_column) = &expect.columns_info[j]{
                        let real_column = &real.get_columns()[j];
                        if let Some(name)  = &expect_column.name{
                            assert_eq!(real_column.info().type_name.find(name).is_some(), true, "{:?}", real);
                        }
    
                        assert_eq!(real_column.dirty.listener_len(), expect_column.change_listeners, "[{}]:{:?}", j, real_column.dirty);
                    }
                }

                if let Some(removes) = expect.removes{
                    assert_eq!(real.removes.len(), removes, "{:?}", real);
                } 
            }
        }
    }
}