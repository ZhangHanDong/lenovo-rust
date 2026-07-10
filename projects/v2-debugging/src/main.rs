//! 调试演练 CLI：演示 `tracing` 初始化、`RUST_LOG` 控制级别、`dbg!` 用法。
//!
//! 运行方式（对照讲义第 6 节命令）：
//! ```bash
//! cargo run -p v2-debugging                       # 默认 info 级别
//! RUST_LOG=trace cargo run -p v2-debugging         # 放大到 trace，看到每步状态
//! RUST_LOG=v2_debugging=debug cargo run -p v2-debugging   # 只看本 crate 的 debug
//! ```
//! 缺陷版与修正版的余额会被并排打印——`RUST_LOG=trace` 下能逐行看到
//! 缺陷版在 `Withdraw` 后余额"反而上升"，这正是 tracing 排查状态错误的价值。

use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use v2_debugging::{apply_ops, apply_ops_buggy, checked_average, window_sums, Op};

fn main() {
    // 用 EnvFilter 读取 RUST_LOG；未设置时默认 info（不改代码即可调观测粒度）。
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(true)
        .init();

    let data = vec![1, 2, 3, 4, 5];
    info!(?data, "演练开始");

    // `dbg!` 把表达式的值连同文件:行号打到 stderr，并把值原样返回（不破坏数据流）。
    let windows = dbg!(window_sums(&data, 2));
    info!(?windows, "滑动窗口求和（修正版）");

    let ops = [Op::Deposit(100), Op::Withdraw(30), Op::Deposit(50)];
    let good = apply_ops(&ops);
    let bad = apply_ops_buggy(&ops);
    info!(good, bad, "对照：修正版 vs 缺陷版余额（分）");
    if good != bad {
        warn!(
            diff = bad - good,
            "缺陷版余额与修正版不一致——开 RUST_LOG=trace 可逐步定位是哪一步出错"
        );
    }

    match checked_average(&data) {
        Some(avg) => info!(avg, "平均值"),
        None => warn!("空切片，无平均值可算"),
    }
}
