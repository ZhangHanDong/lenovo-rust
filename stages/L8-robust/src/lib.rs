//! L8 作业：让 v3-core 能上生产——消灭 unwrap、结构化错误、坏数据跳过但计数
//!
//! 三条底线：
//!   1. 库层用 `thiserror` 定义**结构化**错误（不是 `Box<dyn Error>` 一把梭）；
//!   2. 坏数据**跳过但计数**，绝不静默吞掉（`let _ = ...` 是最危险的）；
//!   3. 应用层用 `anyhow` + `.context()` 传播致命错误。
//!
//! criterion 基准（任务卡第 3 件事）不在骨架里——按 SOLUTION.md 自己加 `benches/`，
//! 先测量再优化，并在 PR 里贴 before/after。

use thiserror::Error;

/// 结构化错误：调用方能 `match` 区分处理，而不是只拿到一句字符串。
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("空行")]
    Empty,
    #[error("缺少 ':' 分隔符: {0:?}")]
    NoDelimiter(String),
    #[error("pid 不是数字: {0:?}")]
    BadPid(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub pid: u32,
    pub message: String,
}

/// 解析一行。返回 `Result` —— 强制调用方处理错误，不能假装它不会失败。
///
/// ```
/// # use winmon_l8_robust::parse_line;
/// assert!(parse_line("4:boot").is_ok());
/// assert!(parse_line("  ").is_err());
/// ```
///
/// 下面这段编译失败：`parse_line` 返回 `Result<Event>`，不能当 `Event` 直接用——
/// 错误处理无法被绕过。
///
/// ```compile_fail
/// # use winmon_l8_robust::{parse_line, Event};
/// let _e: Event = parse_line("4:boot");
/// ```
pub fn parse_line(line: &str) -> Result<Event, ParseError> {
    todo!("L8：解析一行，用 ParseError 区分三种失败，绝不 unwrap")
}

/// 一批行的采集结果：成功的事件 + **跳过的坏行数**（不静默丢弃）。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct CollectResult {
    pub events: Vec<Event>,
    pub skipped: usize,
}

/// 采集：坏行跳过但**计数**——绝不 `let _ = parse_line(...)` 把错误吞掉。
pub fn collect_valid(lines: &[&str]) -> CollectResult {
    todo!("L8：解析每行，成功入 events、失败 skipped += 1（不吞错误）")
}

/// 应用层：坏行太多就当致命错误上报——展示 `anyhow` 的用法。
pub fn require_healthy(result: &CollectResult, max_skip: usize) -> anyhow::Result<()> {
    anyhow::ensure!(
        result.skipped <= max_skip,
        "跳过 {} 行，超过阈值 {}",
        result.skipped,
        max_skip
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_line_distinguishes_errors() {
        assert_eq!(parse_line(""), Err(ParseError::Empty));
        assert_eq!(
            parse_line("no delimiter here"),
            Err(ParseError::NoDelimiter("no delimiter here".into()))
        );
        assert_eq!(parse_line("abc:msg"), Err(ParseError::BadPid("abc".into())));
        assert_eq!(
            parse_line("4:boot"),
            Ok(Event {
                pid: 4,
                message: "boot".into()
            })
        );
    }

    #[test]
    fn collect_skips_and_counts() {
        let lines = ["4:boot", "", "bad", "1200:up"];
        let r = collect_valid(&lines);
        assert_eq!(r.events.len(), 2);
        assert_eq!(r.skipped, 2); // 空行 + "bad" 各算一次，没被静默吞掉
    }

    #[test]
    fn require_healthy_flags_too_many_skips() {
        let r = CollectResult {
            events: vec![],
            skipped: 5,
        };
        assert!(require_healthy(&r, 2).is_err());
        assert!(require_healthy(&r, 10).is_ok());
    }
}
