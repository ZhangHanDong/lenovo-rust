//! 桌面 GUI 骨架（`#[cfg(all(windows, feature = "gui"))]`）。
//!
//! 这是 Tauri 生态的两块基石：
//!
//! - **`tao`**：跨平台窗口与事件循环（从 winit fork 而来，补齐了菜单、托盘等桌面能力）；
//! - **`wry`**：跨平台 WebView 渲染抽象——在 Windows 上落到 **WebView2（Edge Chromium）**，
//!   在 macOS 落到 WKWebView，在 Linux 落到 WebKitGTK。一套 `WebViewBuilder` API，
//!   各平台用各自的原生 WebView 后端实现，是「统一抽象 + 平台后端」范式的又一例。
//!
//! 与原生 Win32 的关系：`tao` 的事件循环底层就是 Win32 的 `GetMessage`/`DispatchMessage`
//! 消息泵 + 窗口过程（`WndProc`），`wry` 则把 WebView2 的 COM 接口（`ICoreWebView2`，
//! 见 `webview2-com` 绑定）封装进安全 API。你过去在 C++ 里手写消息循环 + 嵌入 WebView2，
//! 这里被收敛成几十行声明式代码。
//!
//! 本骨架把核心的 [`TrayMenu`] 渲染成 HTML（[`TrayMenu::to_html`]）塞进 WebView，
//! 演示「平台无关核心提供数据、GUI 适配层只负责呈现」的分层。
//!
//! > 默认 `gui` feature 关闭以免 CI 拉取大量依赖；客户在 Windows 上
//! > `cargo run -p v2-win-gui --features gui` 验证真实 WebView2 窗口。

use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

use crate::TrayMenu;

/// 打开一个承载 `menu` 的 WebView2 窗口，进入事件循环直到用户关闭窗口。
///
/// # Errors
///
/// 创建窗口或 WebView 失败时返回 `wry::Error`。
pub fn run_window(menu: &TrayMenu) -> wry::Result<()> {
    // 1) 事件循环：底层是 Win32 消息泵。
    let event_loop = EventLoop::new();

    // 2) 原生窗口（tao 负责 CreateWindowEx + WndProc）。
    let window = WindowBuilder::new()
        .with_title("Lenovo Agent — 第 12 课 GUI 骨架")
        .build(&event_loop)
        // tao 的 OsError 不是 wry 的 WindowHandleError（后者 #[from] raw_window_handle::HandleError）；
        // 用 wry::Error::Io 承载，避免类型不匹配。
        .map_err(|e| wry::Error::Io(std::io::Error::other(e.to_string())))?;

    // 3) 把核心渲染出的 HTML 塞进 WebView（Windows 上即 WebView2/Edge Chromium）。
    let html = page(menu);
    let _webview = WebViewBuilder::new(&window).with_html(html).build()?;

    // 4) 跑事件循环：仅处理关闭请求，其余交给默认行为。
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::Exit;
        }
    });
}

/// 包一层最小 HTML 外壳，内容来自平台无关核心。
fn page(menu: &TrayMenu) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <style>body{{font-family:Segoe UI,sans-serif;padding:16px}}\
         .menu{{list-style:none;padding:0}}.sep hr{{border:0;border-top:1px solid #ccc}}\
         .disabled{{color:#aaa}}</style></head><body>\
         <h3>托盘菜单预览</h3>{}</body></html>",
        menu.to_html()
    )
}
