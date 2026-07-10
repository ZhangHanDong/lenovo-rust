//! `sysreport` 可执行入口：用平台默认探针采集一份系统报告并打印。
//!
//! - 在交付机（macOS）上走回退桩，可直接 `cargo run -p v2-capstone`；
//! - 客户在 Windows 上同一条命令即走真实 Win32 采集。
//!
//! 输出 JSON 报告 + canonical 串 + FFI 指纹，方便与 C/C++ 宿主侧比对一致性。

use std::process::ExitCode;

use v2_capstone::{collect_report, default_probe};

fn main() -> ExitCode {
    let report = match collect_report(&default_probe()) {
        Ok(report) => report,
        Err(err) => {
            eprintln!("采集系统报告失败：{err}");
            return ExitCode::FAILURE;
        }
    };

    match serde_json::to_string_pretty(&report) {
        Ok(json) => println!("{json}"),
        Err(err) => {
            eprintln!("序列化报告失败：{err}");
            return ExitCode::FAILURE;
        }
    }

    println!("canonical : {}", report.to_canonical());
    println!("fingerprint: {:#018x}", report.fingerprint());
    ExitCode::SUCCESS
}
