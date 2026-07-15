//! L6 作业：并行采集——两种方案对比
//!
//! 从 N 个日志源并行采集事件，用两种方式各实现一遍：
//!   - 方案 A：`Arc<Mutex<Vec<Event>>>` 共享缓冲区，各线程往里 push；
//!   - 方案 B：`mpsc::channel`，各线程把事件发给收集端。
//!
//! 方案 A 的关键：**解析在锁外做，只有 push 在锁内**——否则并行退化成串行。

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Event {
    pub pid: u32,
    pub message: String,
}

/// 一个日志源：一批待解析的原始行（格式 `"pid:message"`）。
pub struct Source {
    pub id: u32,
    pub raw: Vec<String>,
}

/// 解析一行原始日志。**这一步是耗时的**，必须在锁外做。
pub fn parse(line: &str) -> Event {
    let (pid, message) = line.split_once(':').unwrap_or(("0", line));
    Event {
        pid: pid.trim().parse().unwrap_or(0),
        message: message.trim().to_owned(),
    }
}

/// 方案 A：共享 `Arc<Mutex<Vec<Event>>>`，各线程解析后 push。
pub fn collect_shared(sources: Vec<Source>) -> Vec<Event> {
    todo!("L6 方案A：Arc<Mutex<Vec>> 共享缓冲，解析在锁外、只有 push 在锁内")
}

/// 方案 B：`mpsc::channel`，各线程把事件发给收集端。不需要 `Mutex`。
pub fn collect_channel(sources: Vec<Source>) -> Vec<Event> {
    todo!("L6 方案B：mpsc channel 汇聚；注意 drop(tx) 否则 rx 永远等")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sources() -> Vec<Source> {
        vec![
            Source {
                id: 1,
                raw: vec!["4:boot".into(), "4:ready".into()],
            },
            Source {
                id: 2,
                raw: vec!["1200:svc up".into()],
            },
        ]
    }

    fn sorted(mut v: Vec<Event>) -> Vec<Event> {
        v.sort();
        v
    }

    #[test]
    fn both_strategies_agree() {
        let a = sorted(collect_shared(sources()));
        let b = sorted(collect_channel(sources()));
        assert_eq!(a, b);
        assert_eq!(a.len(), 3);
    }

    #[test]
    fn parse_splits_pid_and_message() {
        assert_eq!(
            parse("1200:svc up"),
            Event {
                pid: 1200,
                message: "svc up".into()
            }
        );
    }
}
