//! # 综合实战骨架：系统信息收集器（第 15 课共用）
//!
//! 本 crate 是课程的端到端"收官骨架"，把前 14 课的主线压缩进一个**可编译、可测试、
//! 可跨平台交付**的真实小组件。它故意做成三层，对应企业最关心的三件事：
//!
//! ```text
//!            ┌─────────────────────────────────────────────┐
//!  平台无关核心 │  类型建模（newtype + enum）+ 错误处理（thiserror）│   ← 第 1/2 课
//!  (本文件)    │  collect_report() 把"原始事实"组装成有效报告     │
//!            └───────────────┬───────────────┬─────────────┘
//!                            │               │
//!         ┌──────────────────▼──┐        ┌───▼───────────────────────┐
//!  FFI 边界 │ ffi.rs            │  适配层 │ platform.rs               │
//!  (ffi.rs)│ extern "C" 导出   │(platform│  HostProbe trait + 实现：  │
//!          │ 指纹函数供 C/C++  │ .rs)    │  #[cfg(windows)] → windows │ ← 第 7/9 课
//!          │ 宿主调用（第 7 课）│        │  其它平台 → 回退桩（可跑）  │
//!          └──────────────────┘        └────────────────────────────┘
//! ```
//!
//! ## 设计要点（审查时按这几条看）
//! - **核心不碰平台 API**：核心只依赖 [`HostProbe`] 这个 trait，"去哪取主机名/CPU 数"
//!   是适配层的事。核心因此可在 macOS 上用 [`MockProbe`] 完整单测。
//! - **让非法状态不可表达**：[`Hostname`] 非空、[`CpuCount`] 非零，构造失败即
//!   [`CollectError`]，报告一旦建成必然有效（呼应第 1 课）。
//! - **平台代码用 `#[cfg(windows)]` 隔离**：Windows 调用真 Win32（`GetComputerNameExW` /
//!   `GetSystemInfo`），macOS 走回退桩——同一套核心、同一套验收（呼应第 9/13 课）。

mod ffi;
mod platform;

use std::fmt;

use serde::Serialize;
use thiserror::Error;

pub use ffi::{capstone_report_fingerprint, fingerprint};
pub use platform::default_probe;

/// 报告结构版本号。跨进程/跨语言传输时用它做兼容性判断（FFI 指纹也覆盖它）。
pub const SCHEMA_VERSION: u32 = 1;

/// 收集过程中的错误。库 crate 用 `thiserror` 定义**结构化、可匹配**的错误类型，
/// 把"哪里出了问题"编码进类型，而不是抛字符串（呼应第 2 课：库用 thiserror）。
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum CollectError {
    /// 平台探针返回了空主机名——报告无意义，拒绝构造。
    #[error("平台探针返回空主机名")]
    EmptyHostname,
    /// 平台报告 0 个逻辑 CPU——不可能且会让后续除法 panic，拒绝构造。
    #[error("平台报告逻辑 CPU 数为 0")]
    NoCpus,
    /// 平台探针本身失败（如 Win32 调用返回错误）。携带原始描述便于排查。
    #[error("平台探针失败：{0}")]
    Probe(String),
}

/// 主机名 newtype：保证非空、去除首尾空白。误把任意 `String` 当主机名编译不过。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Hostname(String);

impl Hostname {
    /// 构造一个经校验的主机名。空白或空串 → [`CollectError::EmptyHostname`]。
    pub fn new(raw: impl Into<String>) -> Result<Self, CollectError> {
        let trimmed = raw.into().trim().to_owned();
        if trimmed.is_empty() {
            return Err(CollectError::EmptyHostname);
        }
        Ok(Self(trimmed))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Hostname {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 逻辑 CPU 数 newtype：保证 ≥ 1。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct CpuCount(u32);

impl CpuCount {
    /// 0 → [`CollectError::NoCpus`]，否则封装。
    pub fn new(n: u32) -> Result<Self, CollectError> {
        if n == 0 {
            return Err(CollectError::NoCpus);
        }
        Ok(Self(n))
    }

    #[must_use]
    pub fn get(self) -> u32 {
        self.0
    }
}

/// 操作系统家族。和类型让"非法平台"无法表达（对照用裸字符串）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OsFamily {
    Windows,
    MacOs,
    Linux,
    Other,
}

impl OsFamily {
    /// 用于 canonical 字符串与 FFI 指纹的稳定短名。
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            OsFamily::Windows => "windows",
            OsFamily::MacOs => "macos",
            OsFamily::Linux => "linux",
            OsFamily::Other => "other",
        }
    }
}

/// 平台探针契约：核心只认这个 trait，**不认任何具体平台 API**。
///
/// 这是"平台无关核心 + 适配层"的接缝（第 13 课）。Windows 实现走 Win32，
/// 其它平台走回退桩，单测走 [`MockProbe`]——核心代码一行都不用改。
pub trait HostProbe {
    /// 原始主机名（未校验，可能含空白）。
    fn raw_hostname(&self) -> Result<String, CollectError>;
    /// 逻辑 CPU 数。
    fn logical_cpus(&self) -> Result<u32, CollectError>;
    /// 当前操作系统家族。
    fn os_family(&self) -> OsFamily;
}

/// 一份经校验的系统报告。字段全是受约束的类型——能构造出来就一定有效。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SystemReport {
    pub schema_version: u32,
    pub os_family: OsFamily,
    pub hostname: Hostname,
    pub logical_cpus: CpuCount,
}

impl SystemReport {
    /// 稳定的规范化字符串。供日志、跨语言传输与 FFI 指纹使用——
    /// 字段顺序与格式固定，便于客户在 Windows 端比对一致性。
    #[must_use]
    pub fn to_canonical(&self) -> String {
        format!(
            "schema={};os={};host={};cpus={}",
            self.schema_version,
            self.os_family.as_str(),
            self.hostname.as_str(),
            self.logical_cpus.get(),
        )
    }

    /// 对规范化字符串取 FFI 指纹（FNV-1a 64 位）。Rust 端与 C/C++ 端应得到同值。
    #[must_use]
    pub fn fingerprint(&self) -> u64 {
        fingerprint(&self.to_canonical())
    }
}

/// 核心组装逻辑：从任意 [`HostProbe`] 读取原始事实，校验后产出 [`SystemReport`]。
///
/// 全程平台无关：把"取数据"委托给探针，自己只负责**建模与校验**。
pub fn collect_report<P: HostProbe + ?Sized>(probe: &P) -> Result<SystemReport, CollectError> {
    let hostname = Hostname::new(probe.raw_hostname()?)?;
    let logical_cpus = CpuCount::new(probe.logical_cpus()?)?;
    Ok(SystemReport {
        schema_version: SCHEMA_VERSION,
        os_family: probe.os_family(),
        hostname,
        logical_cpus,
    })
}

/// 测试/演示用的可控探针：让核心逻辑在 macOS 上也能确定性单测。
#[derive(Debug, Clone)]
pub struct MockProbe {
    pub hostname: Result<String, CollectError>,
    pub cpus: Result<u32, CollectError>,
    pub os: OsFamily,
}

impl HostProbe for MockProbe {
    fn raw_hostname(&self) -> Result<String, CollectError> {
        self.hostname.clone()
    }
    fn logical_cpus(&self) -> Result<u32, CollectError> {
        self.cpus.clone()
    }
    fn os_family(&self) -> OsFamily {
        self.os
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_probe() -> MockProbe {
        MockProbe {
            hostname: Ok("  build-box  ".to_owned()),
            cpus: Ok(8),
            os: OsFamily::MacOs,
        }
    }

    #[test]
    fn collects_and_trims_a_valid_report() {
        let report = collect_report(&ok_probe()).expect("valid probe");
        assert_eq!(report.hostname.as_str(), "build-box"); // 首尾空白被去掉
        assert_eq!(report.logical_cpus.get(), 8);
        assert_eq!(report.os_family, OsFamily::MacOs);
        assert_eq!(report.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn empty_hostname_is_rejected() {
        let probe = MockProbe {
            hostname: Ok("   ".to_owned()),
            ..ok_probe()
        };
        assert_eq!(collect_report(&probe), Err(CollectError::EmptyHostname));
    }

    #[test]
    fn zero_cpus_is_rejected() {
        let probe = MockProbe {
            cpus: Ok(0),
            ..ok_probe()
        };
        assert_eq!(collect_report(&probe), Err(CollectError::NoCpus));
    }

    #[test]
    fn probe_failure_propagates() {
        let probe = MockProbe {
            cpus: Err(CollectError::Probe("Win32 GetSystemInfo failed".to_owned())),
            ..ok_probe()
        };
        assert!(matches!(
            collect_report(&probe),
            Err(CollectError::Probe(_))
        ));
    }

    #[test]
    fn canonical_string_is_stable() {
        let report = collect_report(&ok_probe()).unwrap();
        assert_eq!(
            report.to_canonical(),
            "schema=1;os=macos;host=build-box;cpus=8"
        );
    }

    #[test]
    fn fingerprint_matches_ffi_helper() {
        let report = collect_report(&ok_probe()).unwrap();
        // 报告指纹 == 直接对其 canonical 串取指纹（C/C++ 宿主走同一路径）。
        assert_eq!(report.fingerprint(), fingerprint(&report.to_canonical()));
        // FNV-1a 是确定性的：固定输入必得固定值，便于跨语言比对。
        assert_eq!(fingerprint(""), 0xcbf2_9ce4_8422_2325);
    }

    #[test]
    fn report_serializes_to_json() {
        let report = collect_report(&ok_probe()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(
            json.contains("\"host\":\"build-box\"") || json.contains("\"hostname\":\"build-box\"")
        );
    }
}
