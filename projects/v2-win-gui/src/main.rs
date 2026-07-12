//! `appctl`：第 12 课配套小工具，演示「平台无关核心」。
//!
//! 在任意平台（含 macOS 交付机）都能 `cargo run -p v2-win-gui` 跑通：
//! 它构造应用配置与托盘菜单、走一遍服务状态机，并打印渲染出的 HTML。
//! 真正的服务注册 / 注册表 / 事件日志 / GUI 路径在 `#[cfg(windows)]` 适配层，
//! 由客户在 Windows 上验证（见 `lib.rs` 的 service / integration / gui 模块）。

use v2_win_gui::{AppConfig, ServiceCommand, ServiceState, TrayMenu};

fn main() {
    // Windows 服务分发：当 SCM 以 `binPath= "...appctl.exe --service"` 拉起本进程时，
    // 进程必须在 30 秒内调用 StartServiceCtrlDispatcher（即 service::run_as_service，
    // 内部走 service_dispatcher::start），把主线程交给 SCM、由它回调 service_main；
    // 否则 SCM 判定服务无响应（错误 1053）。因此 main 先按命令行参数分发：带 `--service`
    // 就进服务派发路径、在其结束后返回，否则照常走下面的平台无关 CLI 演示。
    // 整段仅在 Windows 目标编译（`service` 模块本身也受 #[cfg(windows)] 约束）。
    #[cfg(windows)]
    {
        if std::env::args().any(|arg| arg == "--service") {
            if let Err(e) = v2_win_gui::service::run_as_service() {
                eprintln!("服务分发失败（未由 SCM 以服务方式拉起？）：{e}");
            }
            return;
        }
    }

    let cfg = AppConfig::new("LenovoAgent");
    println!("应用配置：{cfg:?}");
    match cfg.validate() {
        Ok(()) => println!(
            "配置校验通过；开机自启写入键 = HKCU\\{}",
            v2_win_gui::AUTOSTART_REG_PATH
        ),
        Err(e) => println!("配置非法：{e}"),
    }

    let menu = TrayMenu::new()
        .action("open", "打开主界面")
        .checkbox("autostart", "开机自启", cfg.autostart)
        .separator()
        .action("quit", "退出");
    println!("\n托盘菜单（{} 项）：", menu.len());
    for item in menu.items() {
        println!("  {item:?}");
    }
    println!("\n渲染给 WebView 的 HTML：\n{}", menu.to_html());

    // 在桌面上「干跑」一遍服务生命周期，验证状态机；Windows 上由 SCM 真正驱动。
    println!("\n服务生命周期演练：");
    let mut state = ServiceState::default();
    for cmd in [
        ServiceCommand::Start,
        ServiceCommand::Pause,
        ServiceCommand::Continue,
        ServiceCommand::Stop,
    ] {
        match state.apply(cmd) {
            Ok(next) => {
                println!("  {state:?} --{cmd:?}--> {next:?}");
                state = next;
            }
            Err(e) => println!("  {e}"),
        }
    }

    #[cfg(windows)]
    println!("\n[Windows] 可用：服务注册（service）、注册表/事件日志（integration）；GUI 需 --features gui。");
    #[cfg(not(windows))]
    println!("\n[非 Windows] 服务/注册表/事件日志/GUI 适配层已被 cfg 排除，仅演示平台无关核心。");

    // Windows + `gui` feature 下真正弹出 WebView2 窗口，渲染上面那份托盘菜单。
    // 其余平台/未开 feature 时整段被 cfg 排除，不影响交付机（macOS）构建。
    #[cfg(all(windows, feature = "gui"))]
    {
        println!("\n[Windows+gui] 启动 WebView2 设置窗口……（关闭窗口即退出）");
        if let Err(e) = v2_win_gui::gui::run_window(&menu) {
            eprintln!("GUI 启动失败：{e}");
        }
    }
}
