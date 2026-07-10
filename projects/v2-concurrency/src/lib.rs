//! 第 3 课核心代码：并发与内存模型（进入“并发、异步与性能”模块）。
//!
//! 本 crate 不是为了“实现某功能”，而是把第 3 课的并发主线落到可编译、可测试、
//! **确定性**（不靠 `sleep` 赌时序）的代码里：
//!
//! - [`ThreadPool`]：一个用 **`std::thread` + `mpsc` + `Arc<Mutex<Receiver>>`** 写的线程池/
//!   任务调度器。多个 worker 共享同一个接收端（`Arc<Mutex<_>>`），用 **`Arc<AtomicUsize>`**
//!   统计已完成任务数，靠 **`Drop`** 实现优雅关闭（丢弃发送端 → worker 的 `recv` 返回 `Err` → 退出）。
//! - [`parallel_sum`] / [`parallel_stats`]：用 **`std::thread::scope`** 把切片分块并行聚合，
//!   演示 scoped 线程**直接借用栈上数据**（无需 `Arc`/`'static`）。
//! - [`parallel_sum_rayon`]：赏析锚点——同一件事用 **rayon** 的 `par_iter` 一行写完，
//!   对照手写 scoped 线程版本，体会“安全的并行抽象”。
//!
//! 赏析织入：
//! - **rayon**（<https://github.com/rayon-rs/rayon>）：数据并行（`join` / `par_iter`），
//!   把“工作窃取调度”藏在零成本迭代器抽象之下。
//! - **crossbeam**（<https://github.com/crossbeam-rs/crossbeam>）：无锁数据结构与
//!   scoped 线程的安全封装（`ArrayQueue`/`SegQueue`/`channel`）。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};

/// 一个待执行的任务。`Box<dyn FnOnce>` 把闭包擦除成统一类型，便于经通道传递。
///
/// 约束 `Send + 'static`：任务要能**移动到另一个线程**执行（`Send`），且不借用调用栈
/// 上会更早消失的数据（`'static`）。这正是第 3 课“Send 在编译期消除数据竞争”的体现——
/// 若闭包捕获了 `Rc`，这里会直接编译失败（`E0277: Rc<_> cannot be sent between threads`）。
type Job = Box<dyn FnOnce() + Send + 'static>;

/// 基于标准库的线程池 / 任务调度器。
///
/// 结构（对照第 3 课讲义 5.1）：
/// - `sender: Option<Sender<Job>>`：投递任务的发送端；`Option` 是为了在 [`Drop`] 时
///   能**主动丢弃**它，从而通知所有 worker 收尾。
/// - 每个 worker 持有 `Arc<Mutex<Receiver<Job>>>`：**共享同一个接收端**。`mpsc` 是“多生产者、
///   单消费者”，多个消费者需要用 `Mutex` 串行化对 `recv` 的访问。
/// - `completed: Arc<AtomicUsize>`：跨线程的完成计数器，用 `Atomic` 而非 `Mutex<usize>`——
///   只是一个计数，无需互斥临界区，原子加法即可（见讲义 2.4 内存序）。
pub struct ThreadPool {
    sender: Option<mpsc::Sender<Job>>,
    workers: Vec<JoinHandle<()>>,
    completed: Arc<AtomicUsize>,
}

impl ThreadPool {
    /// 创建一个含 `size` 个 worker 线程的池。
    ///
    /// # Panics
    /// `size == 0` 时 panic：没有 worker 的池无法工作，属调用方的逻辑错误。
    #[must_use]
    pub fn new(size: usize) -> Self {
        assert!(size > 0, "线程池至少需要 1 个 worker");

        let (sender, receiver) = mpsc::channel::<Job>();
        // 接收端被多个 worker 共享：Arc 提供共享所有权，Mutex 串行化 recv。
        let receiver = Arc::new(Mutex::new(receiver));
        let completed = Arc::new(AtomicUsize::new(0));

        let mut workers = Vec::with_capacity(size);
        for _ in 0..size {
            let receiver = Arc::clone(&receiver);
            let completed = Arc::clone(&completed);
            workers.push(thread::spawn(move || {
                loop {
                    // 关键：先锁住接收端、recv 拿到任务后**立即释放锁**，再执行任务，
                    // 这样任务执行期间锁是空闲的，其它 worker 可以并行领取下一个任务。
                    let job = {
                        let guard = receiver.lock().expect("接收端 Mutex 不应中毒");
                        guard.recv()
                    };
                    match job {
                        Ok(job) => {
                            job();
                            // Relaxed 足够：我们只要计数最终正确，不依赖它与其它数据的先后关系。
                            completed.fetch_add(1, Ordering::Relaxed);
                        }
                        // 所有 Sender 被丢弃（即 ThreadPool::drop 把 sender 丢掉）后，
                        // recv 返回 Err——这就是“优雅关闭”的信号，跳出循环、线程自然结束。
                        Err(_) => break,
                    }
                }
            }));
        }

        ThreadPool {
            sender: Some(sender),
            workers,
            completed,
        }
    }

    /// 提交一个任务。任务会被某个空闲 worker 取走执行（提交本身不阻塞）。
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job: Job = Box::new(f);
        if let Some(sender) = &self.sender {
            // 仅当所有 worker 都已退出（接收端全部 drop）时才会失败；正常生命周期内不会发生。
            sender.send(job).expect("worker 线程意外全部退出");
        }
    }

    /// 已完成的任务数（原子读取，随时可调用）。
    #[must_use]
    pub fn completed(&self) -> usize {
        self.completed.load(Ordering::Relaxed)
    }

    /// 主动优雅关闭：消费 `self`，丢弃发送端通知 worker 收尾，`join` 等所有 worker 结束，
    /// 返回最终完成的任务总数。
    ///
    /// 因为返回前已 `join` 所有 worker，所有 `fetch_add` 都已发生，计数是**确定**的——
    /// 测试可据此精确断言，无需 `sleep`。
    #[must_use]
    pub fn join(mut self) -> usize {
        self.shutdown();
        self.completed.load(Ordering::Relaxed)
    }

    /// 丢弃发送端并 `join` 所有 worker。幂等：`Drop` 与 [`join`](Self::join) 都会调用它。
    fn shutdown(&mut self) {
        // 丢弃发送端 → 所有 worker 的 recv 陆续返回 Err → 退出循环。
        drop(self.sender.take());
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

impl Drop for ThreadPool {
    /// 优雅关闭：即便调用方没显式 `join`，离开作用域时也会等所有在途任务跑完。
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// 用 `std::thread::scope` 把切片分块**并行求和**。
///
/// 关键点：scoped 线程可以**直接借用** `data`（栈上数据）——`scope` 保证所有子线程在
/// `scope` 返回前 `join`，因此借用一定不会悬垂。这是 `std::thread::spawn`（要求 `'static`）
/// 做不到的，对照讲义 2.1。
///
/// `threads` 是期望的并行度；为 0 时按 1 处理。空切片返回 0。
#[must_use]
pub fn parallel_sum(data: &[i64], threads: usize) -> i64 {
    if data.is_empty() {
        return 0;
    }
    let threads = threads.max(1);
    let chunk_size = data.len().div_ceil(threads);

    thread::scope(|scope| {
        let handles: Vec<_> = data
            .chunks(chunk_size)
            // move 只移动 chunk（一个 &[i64] 引用），data 仍被安全借用。
            .map(|chunk| scope.spawn(move || chunk.iter().sum::<i64>()))
            .collect();

        handles
            .into_iter()
            .map(|h| h.join().expect("子线程不应 panic"))
            .sum()
    })
}

/// 切片的并行统计结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub sum: i64,
    pub min: i64,
    pub max: i64,
    pub count: usize,
}

/// 用 `std::thread::scope` 分块并行计算 `sum / min / max / count`。
///
/// 演示比单纯求和更复杂的并行聚合：每个分块先各自归约出局部 [`Stats`]，主线程再把局部结果
/// 合并成全局结果（一个典型的 map-reduce）。空切片返回 `None`。
#[must_use]
pub fn parallel_stats(data: &[i64], threads: usize) -> Option<Stats> {
    if data.is_empty() {
        return None;
    }
    let threads = threads.max(1);
    let chunk_size = data.len().div_ceil(threads);

    let merged = thread::scope(|scope| {
        let handles: Vec<_> = data
            .chunks(chunk_size)
            .map(|chunk| scope.spawn(move || chunk_stats(chunk)))
            .collect();

        handles
            .into_iter()
            .map(|h| h.join().expect("子线程不应 panic"))
            // 每个分块非空（chunks 不产生空块），故 reduce 一定有值。
            .reduce(merge_stats)
            .expect("至少有一个分块")
    });
    Some(merged)
}

/// 归约一个**非空**分块为局部 [`Stats`]。
fn chunk_stats(chunk: &[i64]) -> Stats {
    let mut it = chunk.iter().copied();
    let first = it.next().expect("分块非空");
    it.fold(
        Stats {
            sum: first,
            min: first,
            max: first,
            count: 1,
        },
        |mut acc, x| {
            acc.sum += x;
            acc.min = acc.min.min(x);
            acc.max = acc.max.max(x);
            acc.count += 1;
            acc
        },
    )
}

/// 合并两个局部 [`Stats`]（结合律成立，故分块顺序不影响结果）。
fn merge_stats(a: Stats, b: Stats) -> Stats {
    Stats {
        sum: a.sum + b.sum,
        min: a.min.min(b.min),
        max: a.max.max(b.max),
        count: a.count + b.count,
    }
}

/// 赏析锚点：同样是并行求和，用 **rayon** 一行写完。
///
/// `par_iter` 背后是 rayon 的工作窃取线程池与递归二分（`join`）。对照 [`parallel_sum`] 手写的
/// 分块 + scope + join，rayon 把这套调度藏在与串行 `iter` 几乎一致的 API 之下——这就是
/// 第 3 课强调的“安全的并行抽象”：你写得像串行，编译器和库替你保证数据竞争不可能发生。
#[must_use]
pub fn parallel_sum_rayon(data: &[i64]) -> i64 {
    use rayon::prelude::*;
    data.par_iter().sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 线程池能执行所有提交的任务：用结果通道做**确定性**同步——
    /// 收满 N 个结果即证明 N 个任务都跑完了（无需 sleep）。
    #[test]
    fn pool_runs_every_job() {
        let pool = ThreadPool::new(4);
        let (tx, rx) = mpsc::channel();
        let n = 100;
        for i in 0..n {
            let tx = tx.clone();
            pool.execute(move || {
                tx.send(i * 2).expect("结果通道应在");
            });
        }
        drop(tx); // 丢弃主端，使 rx 在收满后能正常结束（这里按计数收取）。

        let sum: i64 = (0..n).map(|_| rx.recv().expect("应能收到结果")).sum();
        assert_eq!(sum, (0..n).map(|i| i * 2).sum());
    }

    /// 完成计数器精确等于任务数：`join` 会 join 所有 worker，
    /// 保证所有 `fetch_add` 都已发生，计数是确定的。
    #[test]
    fn completed_counter_is_exact() {
        let pool = ThreadPool::new(3);
        for _ in 0..250 {
            pool.execute(|| {});
        }
        assert_eq!(pool.join(), 250);
    }

    /// `Arc<Mutex<T>>` 共享可变状态：1000 次自增不丢更新（无数据竞争）。
    /// 用 `join` 等所有任务结束后再读，确定性断言。
    #[test]
    fn arc_mutex_shared_counter_no_lost_update() {
        let counter = Arc::new(Mutex::new(0_u64));
        let pool = ThreadPool::new(8);
        for _ in 0..1000 {
            let counter = Arc::clone(&counter);
            pool.execute(move || {
                let mut guard = counter.lock().expect("不应中毒");
                *guard += 1;
            });
        }
        let total = pool.join(); // 等全部完成
        assert_eq!(total, 1000);
        assert_eq!(*counter.lock().unwrap(), 1000);
    }

    /// Drop 优雅关闭：池离开作用域时会把在途任务跑完，不丢任务。
    #[test]
    fn drop_waits_for_inflight_jobs() {
        let counter = Arc::new(AtomicUsize::new(0));
        {
            let pool = ThreadPool::new(2);
            for _ in 0..500 {
                let counter = Arc::clone(&counter);
                pool.execute(move || {
                    counter.fetch_add(1, Ordering::Relaxed);
                });
            }
            // 不显式 join，靠作用域结束触发 Drop。
        }
        // Drop 已 join 所有 worker，计数确定。
        assert_eq!(counter.load(Ordering::Relaxed), 500);
    }

    /// scoped 并行求和与串行结果一致（多种并行度都成立）。
    #[test]
    fn parallel_sum_matches_sequential() {
        let data: Vec<i64> = (1..=10_000).collect();
        let expected: i64 = data.iter().sum();
        for threads in [1, 2, 3, 7, 16] {
            assert_eq!(parallel_sum(&data, threads), expected);
        }
        assert_eq!(parallel_sum(&[], 4), 0);
    }

    /// scoped 并行统计：sum/min/max/count 与串行一致，且与 rayon 版本求和一致。
    #[test]
    fn parallel_stats_and_rayon_agree() {
        let data: Vec<i64> = (-500..=500).collect();
        let stats = parallel_stats(&data, 4).expect("非空");
        assert_eq!(stats.sum, data.iter().sum::<i64>());
        assert_eq!(stats.min, -500);
        assert_eq!(stats.max, 500);
        assert_eq!(stats.count, data.len());

        assert_eq!(parallel_sum_rayon(&data), data.iter().sum::<i64>());
        assert_eq!(parallel_stats(&[], 4), None);
    }
}
