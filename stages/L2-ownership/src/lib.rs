//! L2 作业：事件缓冲区的所有权设计
//!
//! WinMon 的采集器把事件收进一个 `EventBuffer`，然后交给下游处理。
//! 需求三个方法：
//!   - `push(&mut self, event)`  收一个事件
//!   - `len(&self) -> usize`     看有多少个（**不能拿走所有权**）
//!   - `take_all(&mut self)`     把所有事件交给下游，缓冲区清空
//!
//! 验收：**可执行代码中无 `.clone()` 调用**（注释与 derive 不计）。
//! 如果你认为某处必须 clone，在 PR 里论证它——但这道题不需要任何 clone。

/// 一个事件。这里先用最简单的形态（L4 会把它建模成 enum）。
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub pid: u32,
    pub message: String,
}

impl Event {
    pub fn new(pid: u32, message: &str) -> Self {
        Event {
            pid,
            message: message.to_owned(),
        }
    }
}

#[derive(Default)]
pub struct EventBuffer {
    events: Vec<Event>,
}

impl EventBuffer {
    pub fn new() -> Self {
        EventBuffer::default()
    }

    /// 收一个事件：所有权移进来。
    pub fn push(&mut self, event: Event) {
        todo!("L2 push：把 event 移入内部 Vec")
    }

    /// 看有多少个——**只读借用**，不拿走所有权。
    ///
    /// 审查点：签名必须是 `&self` 不是 `self`。
    /// 如果是 `self`，调用一次 `len()` 就把整个 buffer 吃掉了。
    pub fn len(&self) -> usize {
        todo!("L2 len：返回事件个数（注意签名是 &self）")
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// 把所有事件交给下游，缓冲区清空。
    ///
    /// 关键：**零拷贝**。用 `std::mem::take` 把内部 Vec 整个交出去，
    /// 原地留一个空 Vec —— 一次堆拷贝都不发生。
    ///
    /// ⚠️ AI 的典型错误：写成 `self.events.clone()`（复制整个 Vec！）
    ///    然后 buffer 里还留着一份，既浪费又不符合"交出去"的语义。
    pub fn take_all(&mut self) -> Vec<Event> {
        todo!("L2 take_all：零拷贝地把内部 Vec 交出去（提示 std::mem::take）")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_then_take_all_transfers_ownership() {
        let mut buf = EventBuffer::new();
        buf.push(Event::new(4, "system start"));
        buf.push(Event::new(1200, "svc up"));
        assert_eq!(buf.len(), 2);

        let taken = buf.take_all();
        assert_eq!(taken.len(), 2);
        // take_all 之后，缓冲区应该空了
        assert_eq!(buf.len(), 0);
        assert!(buf.is_empty());
    }

    #[test]
    fn len_does_not_consume() {
        let mut buf = EventBuffer::new();
        buf.push(Event::new(4, "x"));
        // 多次调用 len 都不该出问题（证明它是 &self）
        assert_eq!(buf.len(), 1);
        assert_eq!(buf.len(), 1);
        buf.push(Event::new(5, "y"));
        assert_eq!(buf.len(), 2);
    }

    /// take_all 是零拷贝的：交出去的 Vec 用的还是原来那块堆内存。
    #[test]
    fn take_all_is_zero_copy() {
        let mut buf = EventBuffer::new();
        buf.push(Event::new(4, "x"));
        let ptr_before = buf.events.as_ptr();
        let taken = buf.take_all();
        // 交出去的 Vec 的堆指针 == 原来那块（没有重新分配）
        assert_eq!(
            taken.as_ptr(),
            ptr_before,
            "take_all 应该零拷贝，不能重新分配"
        );
    }
}
