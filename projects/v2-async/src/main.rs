//! CLI 适配层：用 `#[tokio::main]` 启动行协议 TCP 服务。
//!
//! 角色：薄薄的"装配 + 启动"层。协议语义与服务循环都在 `lib.rs`（可单测、与 IO 解耦），
//! 这里只负责建运行时、绑定监听、接 Ctrl-C 做优雅退出。
//!
//! 试一下：
//! ```text
//! cargo run -p v2-async                 # 默认监听 127.0.0.1:8080
//! # 另开一个终端：
//! printf 'PING\nECHO hi\nADD 2 3\n' | nc 127.0.0.1 8080
//! ```

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::Context;
use tokio::net::TcpListener;
use tokio::signal;
use tokio::sync::oneshot;

use v2_async::run_server;

/// `#[tokio::main]` 把这个 async fn 包成"建多线程运行时 + block_on"的同步入口。
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".into());
    let listener = TcpListener::bind(&addr)
        .await
        .with_context(|| format!("无法绑定监听地址 {addr}"))?;
    tracing::info!("行协议服务已启动：{addr}（Ctrl-C 优雅退出）");

    let counter = Arc::new(AtomicUsize::new(0));

    // 用一个独立任务把 Ctrl-C 转成 oneshot 关闭信号，喂给 run_server 的 select!。
    let (sd_tx, sd_rx) = oneshot::channel();
    tokio::spawn(async move {
        if signal::ctrl_c().await.is_ok() {
            let _ = sd_tx.send(());
        }
    });

    run_server(listener, Arc::clone(&counter), sd_rx).await?;

    tracing::info!("已退出，累计处理 {} 行", counter.load(Ordering::SeqCst));
    Ok(())
}
