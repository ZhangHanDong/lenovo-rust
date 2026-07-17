//! L3 作业：把"到处 clone"的事件过滤器改成借用版
//!
//! `ai_draft` 是"能跑但很脏"的版本——每个函数都拿走所有权、到处 clone。
//! 你的任务：在 `clean` 模块里写出**零拷贝**的等价实现，功能不变、测试全绿。
//!
//! 判断顺序（先自己想，再看 AI）：
//!   1. 这个函数真的需要拿走所有权吗？（多半不需要——它只是"看"事件）
//!   2. 只读函数的参数应该是 `&[Event]` / `&str`，不是 `Vec<Event>` / `String`；
//!   3. 返回筛选结果时，返回**引用** `Vec<&Event>` 而不是 clone 出新的 `Vec<Event>`。

/// 一条事件（沿用 L2 的最简形态）。
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

// ───────────────── AI 的第一版：能跑，但到处 clone ─────────────────
// ⚠️ 每个函数都拿走 Vec 的所有权，还在内部反复 clone。审查它。
pub mod ai_draft {
    use super::Event;

    /// 拿走整个 Vec（调用者之后就没法再用了），还 clone 出每个匹配元素。
    pub fn filter_by_pid(events: Vec<Event>, pid: u32) -> Vec<Event> {
        events.into_iter().filter(|e| e.pid == pid).collect()
    }

    /// 参数是 `String`（强制调用者交出所有权），返回 clone 出的新字符串。
    pub fn messages_containing(events: Vec<Event>, needle: String) -> Vec<String> {
        events
            .into_iter()
            .filter(|e| e.message.contains(&needle))
            .map(|e| e.message)
            .collect()
    }

    /// 只是数个数，却把整个 Vec 吃掉了。
    pub fn count_by_pid(events: Vec<Event>, pid: u32) -> usize {
        events.into_iter().filter(|e| e.pid == pid).count()
    }
}

// ───────────────── 你的借用版：零拷贝 ─────────────────
pub mod clean {
    use super::Event;

    /// 只读筛选：借用输入，返回**指向原事件的引用**——一次堆拷贝都没有。
    ///
    /// 只有 `events` 一个输入引用，返回值的生命周期由 elision 规则自动绑到它，
    /// 不用写 `'a`（对比下面 `messages_containing`）。
    pub fn filter_by_pid(events: &[Event], pid: u32) -> Vec<&Event> {
        events.iter().filter(|e| e.pid == pid).collect()
    }

    /// 返回消息的**字符串切片**引用，不复制字符串内容。
    ///
    /// 这里有 `events` 和 `needle` **两个**输入引用，编译器无法自动判断返回值
    /// 借的是哪个——所以**必须显式**用 `'a` 把返回值绑到 `events`。
    pub fn messages_containing<'a>(events: &'a [Event], needle: &str) -> Vec<&'a str> {
        events
            .iter()
            .filter(|e| e.message.contains(needle))
            .map(|e| e.message.as_str())
            .collect()
    }

    /// 数个数：只读借用就够，不必拿走所有权。
    pub fn count_by_pid(events: &[Event], pid: u32) -> usize {
        events.iter().filter(|e| e.pid == pid).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<Event> {
        vec![
            Event::new(4, "system start"),
            Event::new(1200, "svc up"),
            Event::new(4, "system ready"),
        ]
    }

    #[test]
    fn filter_by_pid_returns_references() {
        let events = sample();
        let hits = clean::filter_by_pid(&events, 4);
        assert_eq!(hits.len(), 2);
        // 关键：返回的是**指向原 Vec 元素的引用**，不是 clone。
        assert!(std::ptr::eq(hits[0], &events[0]));
        assert!(std::ptr::eq(hits[1], &events[2]));
    }

    #[test]
    fn messages_containing_borrows() {
        let events = sample();
        let hits = clean::messages_containing(&events, "system");
        assert_eq!(hits, vec!["system start", "system ready"]);
        // 切片指向原字符串的堆内存。
        assert!(std::ptr::eq(hits[0].as_ptr(), events[0].message.as_ptr()));
    }

    #[test]
    fn count_by_pid_reads_only() {
        let events = sample();
        assert_eq!(clean::count_by_pid(&events, 4), 2);
        // events 还能继续用，证明没被拿走。
        assert_eq!(events.len(), 3);
    }
}
