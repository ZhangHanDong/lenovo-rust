//! 第 4 课核心逻辑：异步编程与 Tokio。
//!
//! 本 crate 的角色：把第 4 课讲的几条异步要点落到**可编译、可确定性单测**的代码里，
//! 而不是堆一个"能跑就行"的 demo：
//! - **纯逻辑与 IO 分离**：行协议解析 [`handle_line`] 是同步纯函数，可用 `#[test]` 直接测，
//!   不需要任何运行时；网络 IO 单独放在 [`run_server`]，用 `#[tokio::test]` 端到端测。
//! - **Tokio 运行时与任务**：[`run_server`] 用 `TcpListener` 接受连接，每条连接
//!   `tokio::spawn` 一个任务，连接内用 `into_split` 把流拆成异步读 / 写两半。
//! - **共享状态用免锁原子**：连接计数用 `Arc<AtomicUsize>`，避免"跨 `.await` 持有
//!   `std::sync::MutexGuard` 导致 future 非 `Send`"这一经典陷阱（见讲义第 5 段）。
//! - **mpsc 生产者-消费者 + `select!` 优雅退出**：[`spawn_producer`] / [`run_pipeline`]
//!   演示有界通道（背压）与"收到关闭信号即停"的可控收尾。
//!
//! 赏析锚点：tokio 把"`Future` 状态机"驱动起来——reactor(mio) 注册就绪事件、唤醒 waker、
//! work-stealing 调度器把被唤醒的任务重新 poll。本文件的每个 `.await` 都是一个让出点。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};

/// 行协议处理：**同步纯函数**，输入一行文本、输出一行响应。
///
/// 把"协议语义"从"网络 IO"里剥出来，是本课最重要的工程取舍：
/// 纯逻辑没有 `async`、不碰运行时，于是能用普通 `#[test]` 穷举各分支，
/// 测试既快又确定，不依赖端口、不依赖调度时序。
///
/// 支持的命令（大小写不敏感）：
/// - `PING` -> `PONG`
/// - `ECHO <text>` -> `<text>`
/// - `ADD <a> <b>` -> 两个整数之和；操作数非法 -> `ERR ...`
/// - 其它 -> `ERR unknown command: ...`
pub fn handle_line(line: &str) -> String {
    let line = line.trim();
    let mut parts = line.splitn(2, ' ');
    let cmd = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("").trim();

    match cmd.to_ascii_uppercase().as_str() {
        "" => "ERR empty command".to_string(),
        "PING" => "PONG".to_string(),
        "ECHO" => rest.to_string(),
        "ADD" => match parse_add(rest) {
            Some(sum) => sum.to_string(),
            None => format!("ERR invalid ADD operands: {rest:?}"),
        },
        other => format!("ERR unknown command: {other}"),
    }
}

/// 解析 `ADD` 的两个整型操作数并求和。多于两个操作数视为非法。
fn parse_add(rest: &str) -> Option<i64> {
    let mut it = rest.split_whitespace();
    let a: i64 = it.next()?.parse().ok()?;
    let b: i64 = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None; // 多余操作数，拒绝
    }
    a.checked_add(b)
}

/// 运行行协议服务：接受连接，每连接派生一个任务，直到收到关闭信号。
///
/// 设计要点：
/// - `listener` 由**调用方注入**——测试传入绑定到端口 0 的 `TcpListener`，
///   拿到内核分配的真实端口去连，于是端到端测试无需硬编码端口、不会撞端口。
/// - `counter: Arc<AtomicUsize>` 跨任务共享连接计数。用**原子**而非 `Mutex`：
///   计数是 `Send`，可以安全地跨 `.await` 存活，不会把 future 拖成非 `Send`。
/// - `shutdown` 是 `oneshot`：`select!` 在"接受新连接"与"收到关闭"之间多路等待，
///   收到信号即跳出接受循环，实现**优雅退出**（不再接新连接；在途连接任务自然收尾）。
///
/// 注意：任务取消在 Tokio 里就是 **drop 掉对应的 future**——这里跳出循环后，
/// 不再 `accept`，但已 `spawn` 的连接任务是独立的，会读到客户端 EOF 后正常结束。
pub async fn run_server(
    listener: TcpListener,
    counter: Arc<AtomicUsize>,
    mut shutdown: oneshot::Receiver<()>,
) -> std::io::Result<()> {
    loop {
        tokio::select! {
            // 分支一：接受一个新连接，丢给独立任务处理。
            accepted = listener.accept() => {
                let (stream, _peer) = accepted?;
                let counter = Arc::clone(&counter);
                tokio::spawn(async move {
                    if let Err(e) = handle_conn(stream, counter).await {
                        tracing::debug!("连接处理结束：{e}");
                    }
                });
            }
            // 分支二：收到关闭信号，停止接受、优雅退出。
            _ = &mut shutdown => {
                tracing::debug!("收到关闭信号，停止接受新连接");
                break;
            }
        }
    }
    Ok(())
}

/// 处理单条连接：用 `into_split` 拆成异步读 / 写两半，逐行回应。
///
/// `into_split` 返回**有所有权**的读半与写半，于是读、写可分别移动进不同的
/// 任务或在同一任务内独立使用，互不借用冲突。
async fn handle_conn(stream: TcpStream, counter: Arc<AtomicUsize>) -> std::io::Result<()> {
    let (read_half, mut write_half) = stream.into_split();
    let mut lines = BufReader::new(read_half).lines();

    // `next_line().await` 是让出点：没有数据到达时任务挂起，reactor 在套接字
    // 可读时唤醒它——这正是"Future 状态机被运行时驱动"的具体体现。
    while let Some(line) = lines.next_line().await? {
        let response = handle_line(&line);
        counter.fetch_add(1, Ordering::Relaxed); // 纯计数、无需同步其它内存 → Relaxed 最廉价（见第 3 课）
        write_half.write_all(response.as_bytes()).await?;
        write_half.write_all(b"\n").await?;
    }
    Ok(())
}

/// 启动一个生产者任务，把 `items` 逐个送进**有界** mpsc 通道，返回接收端。
///
/// 有界容量带来**背压**：当消费者跟不上、缓冲填满时，`tx.send(..).await` 会挂起，
/// 从而把生产速度反压到消费速度，避免无界堆积撑爆内存。
/// 若消费端已退出（接收端被 drop），`send` 返回 `Err`，生产者随之收尾。
pub fn spawn_producer(items: Vec<String>, capacity: usize) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel(capacity.max(1));
    tokio::spawn(async move {
        for item in items {
            if tx.send(item).await.is_err() {
                break; // 消费者已走，停止生产
            }
        }
        // tx 在此 drop，通道关闭：消费者会读到 None，得知"生产结束"。
    });
    rx
}

/// 消费者流水线：从通道取行、用 [`handle_line`] 处理，直到通道关闭或收到关闭信号。
///
/// `select!` 在两件事上多路等待：
/// - `rx.recv()`：拿到 `Some(line)` 就处理；拿到 `None` 表示生产端全部结束且队列排空，正常收尾；
/// - `shutdown`：收到信号即**优雅退出**，丢弃尚未消费的项。
///
/// 返回已处理项的响应序列（顺序与到达顺序一致，单消费者不会乱序）。
pub async fn run_pipeline(
    mut rx: mpsc::Receiver<String>,
    mut shutdown: oneshot::Receiver<()>,
) -> Vec<String> {
    let mut out = Vec::new();
    loop {
        tokio::select! {
            maybe = rx.recv() => match maybe {
                Some(line) => out.push(handle_line(&line)),
                None => break, // 生产结束 + 队列排空
            },
            _ = &mut shutdown => break, // 优雅退出
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpStream;

    /// AC1（同步纯逻辑）：穷举行协议各分支，无需运行时。
    #[test]
    fn handle_line_covers_protocol() {
        assert_eq!(handle_line("PING"), "PONG");
        assert_eq!(handle_line("ping"), "PONG"); // 大小写不敏感
        assert_eq!(handle_line("ECHO hello world"), "hello world");
        assert_eq!(handle_line("ADD 2 3"), "5");
        assert_eq!(handle_line("ADD -4 9"), "5");
        assert!(handle_line("ADD 1 x").starts_with("ERR"));
        assert!(handle_line("ADD 1 2 3").starts_with("ERR")); // 多余操作数
        assert!(handle_line("").starts_with("ERR"));
        assert!(handle_line("WAT").starts_with("ERR unknown command"));
    }

    /// AC2（端到端）：端口 0 注入 listener，跑通 PING/ECHO/ADD 并校验计数。
    #[tokio::test]
    async fn server_handles_protocol_end_to_end() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let (sd_tx, sd_rx) = oneshot::channel();
        let server = tokio::spawn(run_server(listener, Arc::clone(&counter), sd_rx));

        let stream = TcpStream::connect(addr).await.unwrap();
        let (read_half, mut write_half) = stream.into_split();
        let mut reader = BufReader::new(read_half).lines();

        write_half
            .write_all(b"PING\nECHO hi there\nADD 10 20\n")
            .await
            .unwrap();

        // TCP 有序，响应按发送顺序到达：确定性、无 sleep。
        assert_eq!(reader.next_line().await.unwrap().unwrap(), "PONG");
        assert_eq!(reader.next_line().await.unwrap().unwrap(), "hi there");
        assert_eq!(reader.next_line().await.unwrap().unwrap(), "30");

        // 读到 3 条响应 => 服务端已处理 3 行 => 计数恰为 3。
        assert_eq!(counter.load(Ordering::Relaxed), 3);

        // 触发优雅退出，服务端跳出接受循环正常返回。
        sd_tx.send(()).unwrap();
        server.await.unwrap().unwrap();
    }

    /// AC3（流水线全部处理）：有界通道 + 背压，5 项按序全部处理完。
    #[tokio::test]
    async fn pipeline_processes_all_with_backpressure() {
        let items = vec![
            "PING".to_string(),
            "ECHO a".to_string(),
            "ADD 1 1".to_string(),
            "PING".to_string(),
            "ECHO z".to_string(),
        ];
        let rx = spawn_producer(items, 2); // 容量 2 < 项数，必然触发背压
        let (_keep_alive, sd_rx) = oneshot::channel::<()>(); // 不触发关闭
        let out = run_pipeline(rx, sd_rx).await;
        assert_eq!(out, vec!["PONG", "a", "2", "PONG", "z"]);
    }

    /// AC4（收到关闭即退出）：发送端保活使队列永不关闭，关闭信号先到则立即收尾。
    #[tokio::test]
    async fn pipeline_exits_on_shutdown() {
        // 保持发送端存活 => rx.recv() 永远 Pending；唯有 shutdown 分支可推进。
        let (_tx, rx) = mpsc::channel::<String>(4);
        let (sd_tx, sd_rx) = oneshot::channel();
        sd_tx.send(()).unwrap(); // 关闭信号已就绪
        let out = run_pipeline(rx, sd_rx).await;
        assert!(out.is_empty()); // 未处理任何项即优雅退出
    }
}
