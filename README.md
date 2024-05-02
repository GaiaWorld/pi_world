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

# v0.2
## 增加World的tick
## 单例上加tick
## 多例上加tick
## 支持了Insert空原型，及alter从空原型变为有组件原型
## 增加Ticker， 支持查询组件is_changed
## 合并Added和Changed，常用的单组件改变查询的情况下，性能有提升
## 如果一个原型的Component被Ticker查询或有Changed过滤，则会在Column上增加Tick记录, 
## 增加Destroyed的entity，保留其Component，直到所以监听Destroyed的system都执行完毕再删除Entity

ListenType::Remove要修改在原型上加Removes
Query可以优化iter, iter_dirty, iter_dirtys
