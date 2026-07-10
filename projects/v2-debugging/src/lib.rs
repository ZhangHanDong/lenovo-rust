//! 第 14 课核心：一个**可调试演练**工程。
//!
//! 这里不实现"某个真实功能"，而是把第 14 课讲的调试方法落到可编译、可测试的代码：
//! - **典型缺陷函数 + 修正版**：每对函数只差一处 bug（off-by-one / 状态错误），
//!   用 `#[test]` 锁定修正版正确、用 `#[should_panic]` 固定缺陷版的崩溃行为；
//! - **`tracing` 接入**：用 `trace!` / `debug!` / `#[instrument]` 输出结构化日志与 span，
//!   配合 `RUST_LOG` 在不改代码的前提下放大/缩小观测粒度；
//! - **`Option` 表达"可能没有结果"**：用类型把"空切片"这种边界情况显式化，避免 panic。
//!
//! 对照 C++：缺陷版相当于"用 gdb/WinDbg 设断点定位的越界"，修正版相当于"修完用单测固化"。

use tracing::{debug, instrument, trace};

/// 充值 / 扣费指令。`#[derive(Debug)]` 让它能被 `tracing` 以 `?op` 结构化记录。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// 充值（分）。
    Deposit(u64),
    /// 扣费（分）。
    Withdraw(u64),
}

/// 对长度为 `window` 的滑动窗口逐个求和。
///
/// 正确实现：窗口为半开区间 `[start, start + window)`，共 `len - window + 1` 个窗口。
///
/// 边界：`window == 0` 或 `window > len` 时返回空 `Vec`（用类型而非 panic 表达"无结果"）。
#[must_use]
pub fn window_sums(data: &[i32], window: usize) -> Vec<i32> {
    if window == 0 || window > data.len() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(data.len() - window + 1);
    for start in 0..=(data.len() - window) {
        // 半开区间：恰好 window 个元素，不越界。
        let sum: i32 = data[start..start + window].iter().sum();
        trace!(start, window, sum, "window computed");
        out.push(sum);
    }
    out
}

/// 缺陷版：滑动窗口求和，**含一处 off-by-one**。
///
/// BUG：内层用了**闭区间** `start..=start + window`，比目标多读一个元素；
/// 当 `start == len - window` 时下标到达 `start + window == len`，**索引越界 panic**。
///
/// 怎么调：
/// - `RUST_BACKTRACE=1 cargo test` 看 backtrace 定位到本行；
/// - `rust-lldb`/`rust-gdb` 在本函数设断点，打印 `start` / `data.len()`，
///   会发现 `start + window` 越过了切片长度。
///
/// 对照修正版 [`window_sums`]：差别仅在 `..=` 改成 `..` 且补了边界判断。
#[must_use]
pub fn window_sums_buggy(data: &[i32], window: usize) -> Vec<i32> {
    let mut out = Vec::new();
    for start in 0..=(data.len() - window) {
        // BUG: `..=` 是闭区间，末尾多读一个元素 → 最后一个窗口越界 panic。
        let sum: i32 = data[start..=start + window].iter().sum();
        out.push(sum);
    }
    out
}

/// 按指令序列计算最终余额（分）。
///
/// 每步用 `trace!` 记录当前余额——开启 `RUST_LOG=trace` 即可逐步观察状态演化，
/// 这正是排查"状态错误"类 bug 的标准手法。
#[must_use]
pub fn apply_ops(ops: &[Op]) -> i64 {
    let mut balance: i64 = 0;
    for op in ops {
        match op {
            Op::Deposit(amount) => balance += *amount as i64,
            Op::Withdraw(amount) => balance -= *amount as i64,
        }
        trace!(?op, balance, "op applied");
    }
    balance
}

/// 缺陷版：计算余额，**含一处状态错误**（不会 panic，结果却不对——最难查的那类）。
///
/// BUG：`Withdraw` 分支错写成 `+=`，扣费被当成充值，余额只增不减。
///
/// 怎么调：这类 bug 不崩溃、不报错，靠 `tracing` 排查最有效——
/// 开 `RUST_LOG=trace` 跑一遍，会看到某次 `Withdraw` 后 `balance` 反而上升，
/// 一眼定位到出问题的那一步（对照 C++ 里 print 大法 / 条件断点观察变量）。
#[must_use]
pub fn apply_ops_buggy(ops: &[Op]) -> i64 {
    let mut balance: i64 = 0;
    for op in ops {
        match op {
            Op::Deposit(amount) => balance += *amount as i64,
            // BUG: 应为 `-=`。扣费被错误地累加，导致余额状态错误。
            Op::Withdraw(amount) => balance += *amount as i64,
        }
        trace!(?op, balance, "op applied (buggy)");
    }
    balance
}

/// 整数平均值。空切片返回 `None`，避免"除以零 panic"——用类型表达边界。
#[must_use]
pub fn checked_average(data: &[i32]) -> Option<i64> {
    if data.is_empty() {
        return None;
    }
    let sum: i64 = data.iter().map(|&x| i64::from(x)).sum();
    Some(sum / data.len() as i64)
}

/// 异步演示：被 `#[instrument]` 包裹会自动生成一个带参数的 span，
/// 配合 `tokio-console` / `RUST_LOG=debug` 可观察 async 任务的进入/退出。
///
/// 故意拆成两个 `await` 点，演示"span 跨 await 仍然有效"。无 `sleep`，测试确定性。
#[instrument(level = "debug")]
pub async fn fetch_and_double(values: &[i32]) -> i32 {
    let sum = compute_sum(values).await;
    debug!(sum, "sum computed inside span");
    sum.saturating_mul(2)
}

#[instrument(level = "trace")]
async fn compute_sum(values: &[i32]) -> i32 {
    values.iter().sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    // AC1：修正版滑动窗口求和正确（半开区间，len-window+1 个窗口）。
    #[test]
    fn window_sums_is_correct() {
        assert_eq!(window_sums(&[1, 2, 3, 4], 2), vec![3, 5, 7]);
        assert_eq!(window_sums(&[1, 2, 3, 4], 4), vec![10]);
    }

    // AC2：修正版的边界——window 非法时返回空，不 panic。
    #[test]
    fn window_sums_handles_edges() {
        assert_eq!(window_sums(&[1, 2, 3], 0), Vec::<i32>::new());
        assert_eq!(window_sums(&[1, 2, 3], 5), Vec::<i32>::new());
        assert_eq!(window_sums(&[], 2), Vec::<i32>::new());
    }

    // AC3：固定缺陷版的崩溃行为——off-by-one 导致越界 panic。
    #[test]
    #[should_panic(expected = "out of range")]
    fn buggy_window_panics_on_overflow() {
        // 合法输入下修正版本应得 [3, 5, 7]；缺陷版却越界 panic。
        let _ = window_sums_buggy(&[1, 2, 3, 4], 2);
    }

    // AC4：修正版状态机余额正确（100 充 - 30 扣 + 50 充 = 120）。
    #[test]
    fn apply_ops_is_correct() {
        let ops = [Op::Deposit(100), Op::Withdraw(30), Op::Deposit(50)];
        assert_eq!(apply_ops(&ops), 120);
    }

    // AC5：固定缺陷版的错误行为——扣费被当充值累加（100+30+50=180），
    // 用断言把"已知错误"钉住，证明它确实与修正版不同（回归保护）。
    #[test]
    fn buggy_apply_ops_has_wrong_state() {
        let ops = [Op::Deposit(100), Op::Withdraw(30), Op::Deposit(50)];
        assert_eq!(apply_ops_buggy(&ops), 180);
        assert_ne!(apply_ops_buggy(&ops), apply_ops(&ops));
    }

    // AC6：checked_average 用 Option 表达空切片。
    #[test]
    fn average_handles_empty() {
        assert_eq!(checked_average(&[2, 4, 6]), Some(4));
        assert_eq!(checked_average(&[]), None);
    }

    // AC7：异步函数（确定性，无 sleep）。
    #[tokio::test]
    async fn async_fetch_and_double_works() {
        assert_eq!(fetch_and_double(&[1, 2, 3]).await, 12);
    }
}
