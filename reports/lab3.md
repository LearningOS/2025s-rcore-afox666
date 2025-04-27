### 总结实现的功能
1. 由于获取当前进程的接口在本章发生了改变，因此修改了ch4中的mmap相关代码。
2. 完成了`sys_spawn`系统调用的实现，通过调用`TaskControlBlock::new`方法直接新建一个进程，然后将新进程的`parent`设置为当前进程，向当前进程的`child`列表增加新进程的指针，最后通过`add_task()`方法将新进程加入到进程调度的队列中。
3. 实现stride调度算法：
    - 在`TaskControlBlockInner`结构中加入新的成员`stride`和`priority`，新增了两个方法`set_priority()`（设置优先级），`update_stride()`(自动更新stride变量的值)，在`suspend_current_and_run_next()`方法将当前任务状态更改时，调用`update_stride()`方法更新进程的stride值即可实现stride调度算法。
    - 修改`TaskManager.ready_queue`的类型为`Vec`，对应的`fetch()`方法将遍历`ready_queue`找到stride值最小的进程返回，同时将该进程从`ready_queue`中移除

### 问答题
#### stride 算法深入
stride 算法原理非常简单，但是有一个比较大的问题。例如两个 pass = 10 的进程，使用 8bit 无符号整形储存 stride， p1.stride = 255, p2.stride = 250，在 p2 执行一个时间片后，理论上下一次应该 p1 执行。  

**实际情况是轮到 p1 执行吗？为什么？**  
*不是，由于使用 8bit 无符号整形储存 stride，因此p2在执行完一个时间片后，p2.stride = 250 + 10 = 260，此时会产生溢出，最后p2.stride = 4, 此时p2.stride < p1.stride,因此实际情况是p2会继续执行（不产生溢出异常的情况下）*

我们之前要求进程优先级 >= 2 其实就是为了解决这个问题。可以证明， 在不考虑溢出的情况下 , 在进程优先级全部 >= 2 的情况下，如果严格按照算法执行，那么 STRIDE_MAX – STRIDE_MIN <= BigStride / 2。

为什么？尝试简单说明（不要求严格证明）。  
已知 `P.pass = BigStride / P.prio`,且 `P.prio >= 2`的情况下，  
可得`P.pass <= (BigStride / 2)`，  
为了让算法中程序运行的次数大致与其优先级呈正比，那么
`STRIDE_MAX – STRIDE_MIN <= MAX(P.pass)`，也就是`STRIDE_MAX – STRIDE_MIN <= BigStride / 2`


已知以上结论，考虑溢出的情况下，可以为 Stride 设计特别的比较器，让 BinaryHeap<Stride> 的 pop 方法能返回真正最小的 Stride。补全下列代码中的 partial_cmp 函数，假设两个 Stride 永远不会相等。
``` rust
use core::cmp::Ordering;

struct Stride(u64);

impl PartialOrd for Stride {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let self_value = self.0 as i8;
        let other_value = other.0 as i8;
        let pass_max = BigStride / 2;
        if self_value > 0 && other_value < 0 && self_value - other_value > pass_max {
            Some(Ordering::Less)
        } else if self_value < 0 && other_value > 0 && other_value - self_value > pass_max  {
            Some(Ordering::Grater)
        } else {
            Some(self_value.cmp(&other_value))
        }
    }
}

impl PartialEq for Stride {
    fn eq(&self, other: &Self) -> bool {
        false
    }
}

```


### 荣誉准则
1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

    向claude3.7老师请教了许多问题，感谢它的帮助

2. 此外，我也参考了 以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

    参考了 https://cheats.rs/ 手册的一些语法细节。

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。