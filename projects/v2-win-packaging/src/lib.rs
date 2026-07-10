//! 第 11 课核心库：**平台无关、可交叉编译、可打包的最小 CLI 逻辑**。
//!
//! 本 crate 的目的不是"实现某功能"，而是给第 11 课「交叉编译与打包」提供一个
//! **真实可走流程**的最小载体：
//! - `greeting` / `banner` 是**纯平台无关逻辑**，在 macOS 交付机上即可编译、单测、`cargo run`；
//! - 这同一个二进制，用 `cargo xwin build --target x86_64-pc-windows-msvc` 即可在
//!   macOS/Linux 上**交叉编译**到 Windows MSVC 目标，再用 `cargo wix` 打成 MSI（见讲义第 5 节）；
//! - `target_triple` 用 `cfg!` 在编译期选定，让"我现在是哪个目标的产物"在交叉编译演示里一眼可见。
//!
//! 设计原则：**平台无关核心 + 薄适配层**（全程复用，见第 13 课跨平台架构）。核心逻辑可在任意平台测试，
//! 不掺入任何平台专属 API——这正是第 12 课"桌面 GUI 与系统集成"得以把平台边缘
//! （服务、注册表、WebView2）单独下沉的前提。

/// 应用名（也是 MSI 产品名、可执行文件名）。
///
/// 放在平台无关层，供问候语与打包元数据共用，避免多处各写一份字符串导致漂移。
pub const APP_NAME: &str = "v2-greeter";

/// 生成一句问候语。纯函数、无 IO、平台无关——可在 macOS 上直接单测。
///
/// 空白输入回退到 `"world"`，保证输出始终是一句完整问候（避免 "Hello, !"）。
#[must_use]
pub fn greeting(name: &str) -> String {
    let who = name.trim();
    let who = if who.is_empty() { "world" } else { who };
    format!("Hello, {who}! —— from {APP_NAME} v{}", app_version())
}

/// 启动横幅：程序名 + 版本 + 目标三元组（编译期由 Cargo 注入）。
///
/// `TARGET` 不是 Cargo 默认环境变量，这里用 [`target_triple`] 兜底，
/// 让"我现在是哪个目标的产物"在交叉编译演示里一眼可见。
#[must_use]
pub fn banner() -> String {
    format!(
        "{} v{} (target: {})",
        env!("CARGO_PKG_NAME"),
        app_version(),
        target_triple()
    )
}

/// 应用版本号，来自 `Cargo.toml` 的 `package.version`（编译期常量）。
#[must_use]
pub fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// 当前编译目标三元组。用 `cfg!` 在编译期选定，演示"同一份源码、多目标产物"。
///
/// 这是"交叉编译"最小的可观测点：在 macOS 上 `cargo run` 打印 darwin，
/// 用 `cargo xwin run --target x86_64-pc-windows-msvc` 则打印 windows-msvc。
#[must_use]
pub fn target_triple() -> &'static str {
    if cfg!(all(target_os = "windows", target_env = "msvc")) {
        "x86_64/aarch64-pc-windows-msvc"
    } else if cfg!(all(target_os = "windows", target_env = "gnu")) {
        "*-pc-windows-gnu"
    } else if cfg!(target_os = "macos") {
        "*-apple-darwin"
    } else if cfg!(target_os = "linux") {
        "*-unknown-linux-gnu"
    } else {
        "unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greeting_includes_name_and_version() {
        let msg = greeting("Lenovo");
        assert!(msg.contains("Lenovo"), "应包含传入的名字: {msg}");
        assert!(msg.contains(app_version()), "应包含版本号: {msg}");
        assert!(msg.contains(APP_NAME), "应包含应用名: {msg}");
    }

    #[test]
    fn greeting_falls_back_on_blank_input() {
        // 空白输入回退到 world，绝不产生 "Hello, !" 这种半截输出。
        assert!(greeting("   ").contains("world"));
        assert!(greeting("").contains("world"));
    }

    #[test]
    fn banner_carries_version_and_target() {
        let b = banner();
        assert!(b.contains(env!("CARGO_PKG_NAME")));
        assert!(b.contains(app_version()));
        // 目标三元组在任何平台上都应是已知分支之一，不会是 "unknown"。
        assert_ne!(target_triple(), "unknown", "未识别的编译目标: {b}");
    }
}
