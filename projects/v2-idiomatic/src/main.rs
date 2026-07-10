//! CLI 适配层：演示惯用的"核心库 + 薄适配层"分层。
//!
//! 核心逻辑全在 `lib.rs`（可单测、平台无关）；这里只做参数解析与装配。

use std::fs;

use anyhow::{Context, Result};
use clap::Parser;

use v2_idiomatic::{aggregate, parse_events, Kind, Query, UserId};

/// 事件查询工具：从 JSON Lines 文件按条件聚合事件。
#[derive(Parser, Debug)]
#[command(name = "events", version, about)]
struct Cli {
    /// 事件文件（每行一条 JSON）。
    file: std::path::PathBuf,

    /// 只统计某类别：login / logout / purchase / error。
    #[arg(long)]
    kind: Option<String>,

    /// 只统计某用户 id。
    #[arg(long)]
    user: Option<u64>,

    /// 只统计时间戳 >= 该值的事件。
    #[arg(long)]
    since: Option<u64>,
}

fn parse_kind(s: &str) -> Result<Kind> {
    Ok(match s {
        "login" => Kind::Login,
        "logout" => Kind::Logout,
        "purchase" => Kind::Purchase,
        "error" => Kind::Error,
        other => anyhow::bail!("未知类别: {other}"),
    })
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let text = fs::read_to_string(&cli.file)
        .with_context(|| format!("无法读取事件文件: {}", cli.file.display()))?;
    let (events, skipped) = parse_events(&text);

    // 用 builder 惯用法组装查询：每个可选项一行，意图清晰。
    let mut query = Query::new();
    if let Some(k) = &cli.kind {
        query = query.kind(parse_kind(k)?);
    }
    if let Some(u) = cli.user {
        query = query.user(UserId(u));
    }
    if let Some(s) = cli.since {
        query = query.since(s);
    }

    let stats = aggregate(query.filter(&events));

    println!("匹配事件: {}", stats.count);
    println!(
        "营收合计: {}.{:02} 元",
        stats.revenue_cents / 100,
        stats.revenue_cents % 100
    );
    if skipped > 0 {
        eprintln!("（{skipped} 行解析失败已跳过）");
    }
    Ok(())
}
