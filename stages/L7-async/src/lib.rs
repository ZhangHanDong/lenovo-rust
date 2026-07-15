//! L7 作业：异步采集——抓出 AI 的伪并发
//!
//! AI 写异步代码时最常见的坑：循环里 `.await`，看起来并发，其实**串行**。
//! `ai_draft::sample_all` 就是这样——3 个源、每个 100ms，它要跑 300ms。
//! 你的 `sample_all` 要让它们**同时飞**，3 个源总耗时接近 100ms。
//!
//! 口诀：**每次看到循环里的 `.await`，停下来问一句"这里本该并发吗"。**

use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub source_id: u32,
    pub value: u32,
}

/// 模拟从一个源采样一次——耗时 100ms。
pub async fn sample_one(source_id: u32) -> Event {
    tokio::time::sleep(Duration::from_millis(100)).await;
    Event {
        source_id,
        value: source_id * 10,
    }
}

// ───────────────── AI 的第一版：串行（看起来并发）─────────────────
pub mod ai_draft {
    use super::{sample_one, Event};

    /// ⚠️ 循环里 `.await`——3 个源排队采，总耗时 300ms。能编译、结果对、就是不并发。
    pub async fn sample_all(ids: &[u32]) -> Vec<Event> {
        let mut out = Vec::new();
        for &id in ids {
            out.push(sample_one(id).await);
        }
        out
    }
}

/// 并发采样：N 个源**同时**发出去，总耗时 ≈ 单个源的耗时。
pub async fn sample_all(ids: &[u32]) -> Vec<Event> {
    todo!("L7：让 N 个源并发采样（提示 futures::future::join_all）")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn sample_all_is_concurrent() {
        let start = Instant::now();
        let out = sample_all(&[1, 2, 3]).await;
        let elapsed = start.elapsed();
        assert_eq!(out.len(), 3);
        // 并发：3 个源各 100ms，总耗时应接近 100ms（不是 300ms）。
        // 阈值放宽到 250ms 以容忍 CI 抖动，但仍能挡住串行版（300ms）。
        assert!(
            elapsed < Duration::from_millis(250),
            "应并发（≈100ms），实际 {elapsed:?} —— 像是串行了"
        );
    }

    #[tokio::test]
    async fn ai_draft_is_serial() {
        let start = Instant::now();
        let _ = ai_draft::sample_all(&[1, 2, 3]).await;
        // 串行版必然 ≥300ms —— 用它反衬"编译通过 ≠ 并发"。
        assert!(start.elapsed() >= Duration::from_millis(300));
    }
}
