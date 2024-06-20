# 已知问题
## ecs的性能测试 不太满意， 
## schedule 多线程运行时会卡住
## 差 ExecGraph的多线程 测试用例的覆盖率
## 差 Query的异步迭代，Insert异步并行插入，提供一个异步的劈分方法？为世界矩阵的层脏做个异步多线程的测试用例
## 差 测试用例的覆盖率
## 差 文档
## 差 代码注释
## 需要构建一个Component的静态依赖图-转成位图集， 如果两个system检查在该图上可以并行，则同一个原型下的两个system也是可以并行的。构建2个图，一个普通的完全图，动态修改时加锁，里面的节点包括：System 运行集 原型-组件 单例资源 多例资源。在该图上分析依赖，如果两个system被明确指定了依赖，则在读写分析时发现循环时，遵循指定的依赖。还有一个System的依赖表（小的ComponentIndex加大的ComponentIndex为键，值为强制读写或弱读写），也是加锁操作。还有1个仅system的执行图，这个需要线程安全的添加边。

## 需要把执行图的图部分拆成单独的crate，这样静态依赖图和渲染图都可以重用这个图，同时也好检查是否循环引用。

## 优化原型名字的表达，类似use 模块。
## 可查看出整个world的原型内存状态。

## 测试get性能 去掉Queryer和Alteror Inserter， 去掉SafeVec， 提出pi_listener(将settle trait 放入pi_append_vec)
## ecs world 上提供mem_size
## 检查 world settle ，否则没法解释为啥get 和 iter慢, 也有可能是Mut慢？
## 单例和多例需要提供mem_size？


# v0.1.11 
## 已经支持异步system
## 新实现Local的SystemParm

# v0.2
## 增加World的tick
## 单例上加tick
## 多例上加tick
## 支持了Insert空原型，及alter从空原型变为有组件原型
## 增加Ticker， 支持查询组件is_changed
## 合并Added和Changed，常用的单组件改变查询的情况下，性能有提升
## 如果一个原型的Component被Ticker查询或有Changed过滤，则会在Column上增加Tick记录, 
## 增加Destroyed的entity，保留其Component，直到所以监听Destroyed的system都执行完毕再删除Entity
## 增加Removed<组件>, 原型上加remove_columns的监听
## 增加了ArchetypeInfo的查询对象，显示实体所在的原型及行
## 优化了Query.iter, 根据是否有脏监听过滤器分为三种情况迭代，iter_normal, iter_dirty, iter_dirtys，多个脏监听内部使用bitset去重

# v0.3
## ArcheypeWorldIndex ComponentIndex ColumnIndex Row 改成类型
## 将原型图节点拆成了原型组件节点
## 优化Query的CacheMapping，及alter的Mapping
## 优化SingleRes的init_state, 抽成多个函数，这样相同的T可以共用函数

# v0.4
## 有Changed或Removed的组件，全部原型上对应组件的Column和RemovedColumn都增加Tick记录，这样全局跟踪Tick，并根据该Tick来剔除已经读取过的变化，同时确保系统读取时不会丢失变化。同时，也会根据最旧的System的last_run，来尽量减少脏列表中Row的长度。
## 根据ComponentInfo及其world_index，优化了alter时计算相应原型的过程。

# v0.5
## 调整了alter的代码，将泛型部分分离
## 将alter改为立即执行，这样可以更好的复用，并且在一个system内可连续修改（要求修改后的原型也要被Query捕获）

# v0.6
## alter还是改为延迟，EntityEditor的alter为立即执行
## ComponentRemoved是一个SystemParam，可迭代出上次运行到这次的运行期间被删除的组件的实体

# v0.7
## 调整了Archetype和Column的关系， Column包含多个Archetype的Blob
## 优化了Query的get方法的性能

# v0.8
## SetDefault改为SetFromWorld
## 单例多例都提前downcast
## meta将单例多例资源记录关系，事件支持安全的读写，所以不记录关系
## 增加ComponentAdded和Changed
## 增加OrDefaultRef
