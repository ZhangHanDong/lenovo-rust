//! L4 作业：事件模型建模——让非法状态无法构造
//!
//! 一条事件可能是：进程启动 / 进程退出 / 资源告警 / 心跳。
//! 你的任务：设计 `Event` 类型，让**每个变体只带它真正需要的数据**，
//! 然后写 `describe(&Event) -> String`，`match` **不写 `_` 兜底**。
//!
//! 对比 `ai_draft`：它用 `kind: String` + 一堆 `Option` 字段，
//! 能构造出大量非法状态（心跳却带 pid、kind 拼错、退出却没 code……）。
//! 好的建模能构造的非法状态数是 **0**。

/// 进程号用 newtype 包起来，**不能**和退出码互相传参。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessId(pub u32);

/// 退出码同样 newtype——和 `ProcessId` 都是整数，但类型不同，编译器帮你挡混用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitCode(pub i32);

/// 资源类型是**闭集**，用 enum 而不是字符串。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resource {
    Cpu,
    Memory,
}

/// 每个变体只带它需要的数据——没有一个字段是"有时有效"的。
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    ProcessStarted {
        pid: ProcessId,
        name: String,
    },
    ProcessExited {
        pid: ProcessId,
        code: ExitCode,
    },
    /// `percent` 用裸 `u8`——101–255 仍可构造，是本课**有意留下的残留非法状态**：
    /// 任务卡的"零非法状态"针对变体结构（心跳带 pid 之类）；
    /// 把 `percent` 收紧成带校验的 newtype（`Percent::new(x) -> Option<Percent>`）是作业加分项。
    ResourceAlert {
        resource: Resource,
        percent: u8,
    },
    Heartbeat,
}

/// 描述一条事件。
///
/// `match` 不写 `_`：将来给 `Event` 加一个新变体时，这里会**编译失败**
/// （E0004 non-exhaustive），逼你处理新情况——把"演化提醒"变成编译期资产。
///
/// 下面的编译失败测试模拟给 `Event` 增加 `Renamed` 变体，却遗漏更新
/// 原有的穷尽式 `match`。`cargo test --doc` 会确认编译器报出 E0004：
///
/// ```compile_fail,E0004
/// enum Event {
///     ProcessStarted,
///     ProcessExited,
///     ResourceAlert,
///     Heartbeat,
///     Renamed,
/// }
///
/// fn describe(event: &Event) -> &'static str {
///     // 这是新增 Renamed 前的 match；没有 `_`，所以演化遗漏会编译失败。
///     match event {
///         Event::ProcessStarted => "进程启动",
///         Event::ProcessExited => "进程退出",
///         Event::ResourceAlert => "资源告警",
///         Event::Heartbeat => "心跳",
///     }
/// }
///
/// fn main() {
///     describe(&Event::Renamed);
/// }
/// ```
pub fn describe(e: &Event) -> String {
    match e {
        Event::ProcessStarted { pid, name } => {
            format!("进程启动：PID {}，名称 {name}", pid.0)
        }
        Event::ProcessExited { pid, code } => {
            format!("进程退出：PID {}，退出码 {}", pid.0, code.0)
        }
        Event::ResourceAlert {
            resource: Resource::Cpu,
            percent,
        } => format!("CPU 资源告警：{percent}%"),
        Event::ResourceAlert {
            resource: Resource::Memory,
            percent,
        } => format!("内存资源告警：{percent}%"),
        Event::Heartbeat => "心跳".to_owned(),
    }
}

// ───────────────── AI 的典型坏建模（对照用，别学它）─────────────────
// 一个 struct 塞下所有字段，用 String 当类型标签、用 Option 表示"这个变体没这字段"。
// 它能构造出无数非法状态——下面 tests 里就造了几个。
pub mod ai_draft {
    #[derive(Debug, Clone, PartialEq)]
    pub struct Event {
        pub kind: String,     // "started" / "exited" / ... —— 拼错编译器不管
        pub pid: Option<u32>, // 心跳时应为 None，但没人强制
        pub name: Option<String>,
        pub code: Option<i32>,
        pub resource: Option<String>,
        pub percent: Option<u8>,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describe_covers_every_variant() {
        assert!(describe(&Event::ProcessStarted {
            pid: ProcessId(1200),
            name: "svc.exe".to_owned(),
        })
        .contains("启动"));
        assert!(describe(&Event::ProcessExited {
            pid: ProcessId(1200),
            code: ExitCode(0),
        })
        .contains("退出"));
        assert!(describe(&Event::ResourceAlert {
            resource: Resource::Cpu,
            percent: 95,
        })
        .contains("CPU"));
        assert_eq!(describe(&Event::Heartbeat), "心跳");
    }

    /// 好建模：每个变体只有合法数据，构造不出"心跳带 pid"这种非法状态。
    #[test]
    fn good_model_has_no_illegal_states() {
        let _ = Event::Heartbeat; // 心跳就是心跳，没有任何多余字段可填
    }

    /// 坏建模：一个 struct 能造出一堆自相矛盾的"事件"。
    #[test]
    fn ai_draft_allows_illegal_states() {
        // 心跳却带着 pid 和退出码 —— 语义上非法，类型上合法。
        let nonsense = ai_draft::Event {
            kind: "heartbaet".to_owned(), // 还拼错了，编译器毫不知情
            pid: Some(4),
            name: None,
            code: Some(-1),
            resource: Some("disk".to_owned()), // "disk" 根本不是合法资源
            percent: None,
        };
        // 它能编译、能构造——这正是问题所在。
        assert_eq!(nonsense.pid, Some(4));
    }
}
