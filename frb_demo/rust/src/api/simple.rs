//! 第 8 课配套 frb 工程的 Rust API。
//!
//! 这些普通 Rust 函数经 `flutter_rust_bridge_codegen generate` 自动生成等价的 Dart 绑定，
//! Flutter 侧像调用普通 Dart 函数一样调用它们。演示三种跨界形态：
//! - **同步** `#[frb(sync)]`：零开销直返（适合纯计算）；
//! - **异步**（默认）：在 Rust 侧 worker 执行，Dart 侧 `await`，不阻塞 UI 线程；
//! - **`Result`**：`Err` 在 Dart 侧抛成异常。

/// 同步问候。`#[frb(sync)]` 让 Dart 侧得到一个同步函数（无需 await）。
#[flutter_rust_bridge::frb(sync)]
pub fn greet(name: String) -> String {
    format!("Hello, {name}!")
}

/// 异步加法：默认（不加 `sync`）即异步——Dart 侧 `await add(a: 2, b: 40)`，
/// Rust 在独立线程池执行，不卡 Flutter UI 线程。演示 frb 的 async 桥接。
pub async fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// 返回 `Result`：除零返回 `Err`，在 Dart 侧表现为抛出的异常。
/// 演示「Rust 错误 → Dart 异常」的跨界映射（对照 UniFFI 的受检异常）。
pub fn divide(a: f64, b: f64) -> Result<f64, String> {
    if b == 0.0 {
        Err("除数不能为零".to_string())
    } else {
        Ok(a / b)
    }
}

#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    // Default utilities - feel free to customize
    flutter_rust_bridge::setup_default_user_utils();
}
