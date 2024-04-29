# ecs的性能测试 不太满意， 而且schedule 多线程运行时会卡住，测试很奇怪，在run前加了个打印就不卡住了
# 差 ExecGraph的多线程 测试用例的覆盖率
# 差 Query的异步迭代，Insert异步并行插入，提供一个异步的劈分方法？为世界矩阵的层脏做个异步多线程的测试用例
# 差 测试用例的覆盖率
# 差 文档
# 差 代码注释
# 需要构建一个Component的静态依赖图-转成位图集， 如果两个system检查在该图上可以并行，则同一个原型下的两个system也是可以并行的。
# 需要把执行图的图部分拆成单独的crate，这样静态依赖图和渲染图都可以重用这个图，同时也好检查是否循环引用。

# v0.1.11 
## 已经支持异步system
## 新实现Local的SystemParm


增加World的tick， 在Column上增加Option<SafeVec<Tick>>, Ref和Mut上增加Ticker.is_changed, world_tick, last_tick(system), Ticked Changed
增加Deleted的entity，保留其Component，直到所以监听Deleted的system都执行完毕再删除Entity
ListenType::Remove要修改在原型上加Removes
单例上加tick
Query可以优化iter，iter_tick, iter_dirty, iter_dirtys