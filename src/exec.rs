/// 执行器
/// 图执行器，可以用边来保存输出，以及条件输出（只有真假2个边，或者真假2个节点，节点可以不输出）
/// 有了条件输出，就可以执行循环及跳出
/// 似乎行为树也可以成为执行器的逻辑
/// 某个节点的依赖输入都就绪，才执行该节点，似乎在行为树的逻辑中不存在
/// 行为树的循环，执行图中也几乎没有


pub trait Runnble: Send + Sync + 'static {
    /// The context.
    type Context;

    /// Runs the system with the given input in the world. Unlike [`System::run`], this function
    /// can be called in parallel with other systems and may break Rust's aliasing rules
    /// if used incorrectly, making it unsafe to call.
    ///
    /// # Safety
    ///
    /// - The caller must ensure that `world` has permission to access any world data
    ///   registered in [`Self::archetype_component_access`]. There must be no conflicting
    ///   simultaneous accesses while the system is running.
    /// - The method [`Self::update_archetype_component_access`] must be called at some
    ///   point before this one, with the same exact [`World`]. If `update_archetype_component_access`
    ///   panics (or otherwise does not return for any reason), this method must not be called.
    fn run(&mut self, context: Self::Context);
}

/// 写时复制的图， 图和边节点内部有ShareUsize来维护引用计数， 子图有生命周期，Drop时会递归释放引用计数。
/// 图执行器，利用子图和一个闭包来创建执行节点，子图被更新时，也对应更新执行节点。执行节点维护自身状态、依赖计算和输出
/// 输入输出概念，体现在执行节点上，优先利用类型进行匹配，自动支持容器匹配（列表和Map），次之用名称匹配。
/// 可以有子图的概念，子图也是一个图节点，有依赖和输入输出，子图的成员就是一个新的图。这样递归。
/// 好像可以将行为树，（也理解成拓扑排序后的单线程执行图，但需要图支持循环。），或者写一个新的行为树执行器？如果在ecs中，似乎只能串行执行（因为会读写很多ECS的数据），有自己的事件队列，等帧推时，处理对应事件，然后执行，似乎不应该使用异步代码？
/// 
/// 默认图是有向的，某些迭代时，忽略方向进行遍历，好像就是无向图了。
/// 
pub struct G;