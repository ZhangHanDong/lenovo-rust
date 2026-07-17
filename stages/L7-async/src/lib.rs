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

/// 周期采样服务（任务卡第二步的核心骨架）：
/// 每个 `interval` 采一轮（N 个源**并发**采），事件发进 channel 汇聚给主循环；
/// 收到 shutdown 信号后停止采集并退出（返回完成的轮数）。
///
/// 真实 main 里 shutdown 接 Ctrl-C（`tokio::signal::ctrl_c()`）；
/// 测试里用 `watch::channel` 显式发信号——**优雅关闭必须是可测试的**。
pub async fn run_sampler(
    ids: Vec<u32>,
    interval: Duration,
    tx: tokio::sync::mpsc::Sender<Event>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> usize {
    let mut rounds = 0;
    let mut ticker = tokio::time::interval(interval);
    loop {
        // watch 只在"值变化"时唤醒——如果启动时就已经是 true,changed() 永远等不到
        if *shutdown.borrow() {
            return rounds;
        }
        todo!("L7 用 select! 在 采样tick 和 shutdown 之间二选一，实现优雅关闭")
    }
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
    async fn sampler_shuts_down_gracefully() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(64);
        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        let h = tokio::spawn(run_sampler(
            vec![1, 2, 3],
            Duration::from_millis(20),
            tx,
            stop_rx,
        ));
        // 至少收到一轮事件（证明周期采样在跑）
        let first = rx.recv().await.expect("应收到事件");
        assert!(first.source_id >= 1);
        // 发关闭信号 → 任务应在有限时间内退出（挂起 = 优雅关闭没做对）
        stop_tx.send(true).unwrap();
        let rounds = tokio::time::timeout(Duration::from_secs(2), h)
            .await
            .expect("shutdown 后 2s 内必须退出")
            .unwrap();
        assert!(rounds >= 1);
    }

    #[tokio::test]
    async fn sampler_respects_preexisting_shutdown() {
        // 启动时 shutdown 已经是 true——不该采任何一轮
        let (tx, _rx) = tokio::sync::mpsc::channel(4);
        let (_stop_tx, stop_rx) = tokio::sync::watch::channel(true);
        let rounds = run_sampler(vec![1], Duration::from_millis(10), tx, stop_rx).await;
        assert_eq!(rounds, 0, "启动前已要求关闭,不应采样");
    }

    #[tokio::test]
    async fn ai_draft_is_serial() {
        let start = Instant::now();
        let _ = ai_draft::sample_all(&[1, 2, 3]).await;
        // 串行版必然 ≥300ms —— 用它反衬"编译通过 ≠ 并发"。
        assert!(start.elapsed() >= Duration::from_millis(300));
    }
}
