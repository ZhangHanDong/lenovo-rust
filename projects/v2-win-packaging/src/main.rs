//! CLI 适配层：最小可打包的二进制。
//!
//! 这是要被**交叉编译**到 Windows 并**打成 MSI** 的产物（见讲义第 11 课第 5 节）。
//! 它本身完全平台无关：解析参数、调用 `v2_win_packaging` 核心逻辑、打印结果。
//! 没有任何平台分支——交叉编译/打包关心的是"把这一个二进制送到 Windows"，
//! 而不是在源码里塞平台专属逻辑（那属于第 12 课的系统集成）。

use clap::{Parser, Subcommand};

use v2_win_packaging::{banner, greeting};

/// 最小可打包 CLI：演示交叉编译与 MSI 打包流程的载体。
#[derive(Parser, Debug)]
#[command(name = "v2-greeter", version, about)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// 打印一句问候语。
    Greet {
        /// 问候对象（缺省为 world）。
        #[arg(long, default_value = "world")]
        name: String,
    },
    /// 打印版本与编译目标三元组。
    Version,
}

fn main() {
    let cli = Cli::parse();

    match cli.cmd.unwrap_or(Cmd::Greet {
        name: "world".to_string(),
    }) {
        Cmd::Greet { name } => println!("{}", greeting(&name)),
        Cmd::Version => println!("{}", banner()),
    }
}
