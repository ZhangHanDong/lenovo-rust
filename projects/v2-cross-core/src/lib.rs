//! # v2-cross-core · 平台无关核心 + 平台适配层
//!
//! 本 crate 演示第 13 课的核心架构：**把"平台差异"收敛到一个 trait 后面**，
//! 让业务逻辑只依赖抽象、与具体操作系统解耦。
//!
//! ## 结构
//!
//! ```text
//!            ┌───────────────────────────────┐
//!            │  平台无关核心（core）          │   只依赖 trait，可单测、可注入桩
//!            │  config_file_path / render_*   │
//!            └───────────────┬───────────────┘
//!                            │ 依赖
//!                    ┌───────▼────────┐
//!                    │ trait Platform │   抽象"平台能力"
//!                    │     Info       │
//!                    └───────┬────────┘
//!         ┌──────────────────┼──────────────────┐
//!  #[cfg(windows)]   #[cfg(target_os="macos")]  #[cfg(all(unix, not(macos)))]
//!  WindowsPlatform        MacosPlatform              LinuxPlatform
//! ```
//!
//! - 每个平台一个 `impl PlatformInfo`，用 `#[cfg(...)]` 编译期裁剪——
//!   **当前平台只会编进一份**，其它平台的代码根本不进入这次构建。
//! - [`current_platform`] 用 `#[cfg]` 选出当前平台实现。
//! - [`describe_build`] 演示 `cfg!()` 宏（运行期布尔分支）与 `#[cfg]`
//!   （编译期裁剪）的区别。
//!
//! 全程只用标准库；在 Windows / Linux / macOS 三平台均可编译。

use std::path::PathBuf;

/// 平台适配层的抽象：把"随操作系统而变"的能力收敛到这一组方法后面。
///
/// 业务核心只对本 trait 编程，因此既能在真实平台上跑，也能在测试里
/// 注入一个固定行为的桩（mock）来做确定性断言。
pub trait PlatformInfo {
    /// 当前平台的人类可读名称，如 `"Windows"` / `"macOS"` / `"Linux"`。
    fn os_name(&self) -> &str;

    /// 该平台约定的文本行尾：Windows 为 `"\r\n"`，类 Unix 为 `"\n"`。
    fn line_ending(&self) -> &str;

    /// 该平台放置应用配置的根目录（不保证存在，只给出约定路径）。
    fn config_dir(&self) -> PathBuf;

    /// 该平台的路径分隔符：Windows 为 `'\\'`，类 Unix 为 `'/'`。
    fn path_separator(&self) -> char;
}

// ===========================================================================
// 平台无关核心：只依赖 `dyn PlatformInfo`，不出现任何 #[cfg]
// ===========================================================================

/// 计算某个应用某个配置文件的完整路径。
///
/// 这是"平台无关核心"的代表：它不知道自己跑在哪个系统上，只通过
/// [`PlatformInfo`] 拿到约定目录，再用 [`PathBuf::join`] 拼接——
/// `join` 会自动使用当前平台的分隔符，避免手写 `"/"` 的可移植性陷阱。
pub fn config_file_path(platform: &dyn PlatformInfo, app: &str, file: &str) -> PathBuf {
    platform.config_dir().join(app).join(file)
}

/// 把若干 `key=value` 配置项渲染成该平台行尾约定的文本块。
///
/// 行尾来自注入的 [`PlatformInfo`]，因此同一份核心逻辑在 Windows 上产出
/// `\r\n`、在类 Unix 上产出 `\n`——业务代码里不出现任何 `#[cfg]`。
pub fn render_config(platform: &dyn PlatformInfo, entries: &[(&str, &str)]) -> String {
    let line_ending = platform.line_ending();
    entries
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(line_ending)
}

// ===========================================================================
// 平台适配层：每个平台一份实现，用 #[cfg] 编译期三选一
// ===========================================================================

/// Windows 平台实现（仅在 `#[cfg(windows)]` 下编译）。
#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsPlatform;

#[cfg(windows)]
impl PlatformInfo for WindowsPlatform {
    fn os_name(&self) -> &str {
        "Windows"
    }
    fn line_ending(&self) -> &str {
        "\r\n"
    }
    fn config_dir(&self) -> PathBuf {
        // 优先 %APPDATA%，回退到 C:\，演示"读环境变量 + 平台约定"。
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\"))
    }
    fn path_separator(&self) -> char {
        '\\'
    }
}

/// macOS 平台实现（仅在 `#[cfg(target_os = "macos")]` 下编译）。
#[cfg(target_os = "macos")]
#[derive(Debug, Default, Clone, Copy)]
pub struct MacosPlatform;

#[cfg(target_os = "macos")]
impl PlatformInfo for MacosPlatform {
    fn os_name(&self) -> &str {
        "macOS"
    }
    fn line_ending(&self) -> &str {
        "\n"
    }
    fn config_dir(&self) -> PathBuf {
        // macOS 约定：~/Library/Application Support
        home_dir().join("Library").join("Application Support")
    }
    fn path_separator(&self) -> char {
        '/'
    }
}

/// Linux / 其它类 Unix 平台实现
/// （`#[cfg(all(unix, not(target_os = "macos")))]`，避免与 macOS 重复编译）。
#[cfg(all(unix, not(target_os = "macos")))]
#[derive(Debug, Default, Clone, Copy)]
pub struct LinuxPlatform;

#[cfg(all(unix, not(target_os = "macos")))]
impl PlatformInfo for LinuxPlatform {
    fn os_name(&self) -> &str {
        "Linux"
    }
    fn line_ending(&self) -> &str {
        "\n"
    }
    fn config_dir(&self) -> PathBuf {
        // XDG 约定：$XDG_CONFIG_HOME，回退 ~/.config
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir().join(".config"))
    }
    fn path_separator(&self) -> char {
        '/'
    }
}

/// 读取家目录（仅类 Unix 用得到，故只在 unix 下编译，避免 Windows 上 dead_code 告警）。
#[cfg(unix)]
fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// 返回**当前编译目标**对应的平台实现。
///
/// 返回 `impl PlatformInfo`（静态分发、零成本）。三个分支由 `#[cfg]`
/// 编译期裁剪，最终只有一个会进入二进制——这正是"一套代码、三平台编译"
/// 的关键：源代码里写齐三种，构建时只编一种。
#[cfg(windows)]
pub fn current_platform() -> impl PlatformInfo {
    WindowsPlatform
}

/// 见上。
#[cfg(target_os = "macos")]
pub fn current_platform() -> impl PlatformInfo {
    MacosPlatform
}

/// 见上。
#[cfg(all(unix, not(target_os = "macos")))]
pub fn current_platform() -> impl PlatformInfo {
    LinuxPlatform
}

/// 用 `cfg!()` 宏在**运行期**描述本次构建的目标平台。
///
/// 与 `#[cfg]` 的关键区别：
/// - `#[cfg(...)]` 是**编译期裁剪**——不满足条件的代码根本不参与编译；
/// - `cfg!(...)` 是一个**编译期求值为布尔常量**的表达式，所有分支都要
///   能通过编译（类型必须一致），只是其中一支在当前目标恒为 `true`。
///
/// 因此涉及"某平台特有的类型/函数"时只能用 `#[cfg]`，`cfg!()` 适合在
/// 各平台都能编译的纯逻辑里做轻量分支。
pub fn describe_build() -> String {
    let family = if cfg!(windows) {
        "windows"
    } else if cfg!(unix) {
        "unix"
    } else {
        "other"
    };
    // std::env::consts 提供编译期常量，可移植地拿到目标信息。
    format!(
        "family={family}, os={}, arch={}",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试桩：固定行为，不依赖真实运行平台，让核心逻辑断言确定。
    struct MockPlatform {
        os: &'static str,
        line_ending: &'static str,
        root: PathBuf,
        sep: char,
    }

    impl PlatformInfo for MockPlatform {
        fn os_name(&self) -> &str {
            self.os
        }
        fn line_ending(&self) -> &str {
            self.line_ending
        }
        fn config_dir(&self) -> PathBuf {
            self.root.clone()
        }
        fn path_separator(&self) -> char {
            self.sep
        }
    }

    fn unix_mock() -> MockPlatform {
        MockPlatform {
            os: "MockUnix",
            line_ending: "\n",
            root: PathBuf::from("/home/u/.config"),
            sep: '/',
        }
    }

    fn windows_mock() -> MockPlatform {
        MockPlatform {
            os: "MockWin",
            line_ending: "\r\n",
            root: PathBuf::from(r"C:\Users\u\AppData\Roaming"),
            sep: '\\',
        }
    }

    /// 核心逻辑对 trait 编程：注入 mock 即可在任意宿主上确定性地断言。
    #[test]
    fn core_builds_path_via_injected_platform() {
        let p = unix_mock();
        let path = config_file_path(&p, "myapp", "settings.toml");
        assert!(path.ends_with("myapp/settings.toml"));
        assert!(path.starts_with("/home/u/.config"));
    }

    /// 行尾来自注入平台：同一核心逻辑，Windows 桩产出 \r\n，Unix 桩产出 \n。
    #[test]
    fn core_render_respects_platform_line_ending() {
        let entries = [("name", "demo"), ("level", "3")];

        let unix = render_config(&unix_mock(), &entries);
        assert_eq!(unix, "name=demo\nlevel=3");

        let win = render_config(&windows_mock(), &entries);
        assert_eq!(win, "name=demo\r\nlevel=3");
        assert!(win.contains("\r\n"));
    }

    /// mock 的其余能力也按注入值返回，证明核心完全与真实平台解耦。
    #[test]
    fn mock_reports_injected_metadata() {
        let p = windows_mock();
        assert_eq!(p.os_name(), "MockWin");
        assert_eq!(p.path_separator(), '\\');
    }

    /// 当前平台实现应返回合理值：名称非空、行尾是已知两种之一、目录非空。
    #[test]
    fn current_platform_returns_sane_values() {
        let p = current_platform();
        assert!(!p.os_name().is_empty());
        assert!(matches!(p.line_ending(), "\n" | "\r\n"));
        assert!(!p.config_dir().as_os_str().is_empty());
        assert!(matches!(p.path_separator(), '/' | '\\'));
    }

    /// `cfg!()` 运行期分支应与标准库的编译期常量自洽。
    #[test]
    fn describe_build_matches_std_consts() {
        let s = describe_build();
        assert!(s.contains(std::env::consts::OS));
        assert!(s.contains(std::env::consts::ARCH));
        // 当前 CI 三平台都属于 windows 或 unix 家族。
        assert!(s.contains("family=windows") || s.contains("family=unix"));
    }
}
