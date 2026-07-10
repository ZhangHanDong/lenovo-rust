//! Windows 服务骨架（`#[cfg(windows)]`）。
//!
//! 用 Mullvad 维护的 `windows-service` 把本程序注册进 **SCM（服务控制管理器）**，
//! 并实现 start/stop 生命周期与状态汇报。核心要点：
//!
//! - 服务的「主人」是 SCM，不是 `main`。`service_dispatcher::start` 把当前线程交给 SCM，
//!   由它通过 `define_windows_service!` 生成的 FFI 入口回调 [`service_main`]；
//! - 收到 `ServiceControl::Stop` 必须**尽快**把状态切到 `Stopped` 并 `set_service_status`，
//!   否则 SCM 会判定服务无响应；
//! - 这里用核心的 [`ServiceState`] 状态机作为「对外汇报状态」的唯一真相源，
//!   保证向 SCM 汇报的每一步都是一次合法迁移。
//!
//! 对照：与 Linux `systemd` 单元、Android `WorkManager`/`Service` 同构——
//! 「生命周期由系统托管、进程只负责响应回调」是三平台共通的服务模型。
//!
//! > 客户在 Windows 上验证：以 SYSTEM 账户安装服务后，`sc start` / `sc stop`
//! > 应能驱动本模块的状态迁移；详见本课（第 12 课）的 `sc.exe` / MSI `ServiceInstall`。

use std::ffi::OsString;
use std::sync::mpsc;
use std::time::Duration;

use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState as ScmState, ServiceStatus,
    ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::{define_windows_service, service_dispatcher};

use crate::{ServiceCommand, ServiceState};

/// 注册到 SCM 的服务名（需与安装时 `sc create <name>` 一致）。
const SERVICE_NAME: &str = "v2_win_gui_demo";
/// 独占进程型服务。
const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

/// 进程入口（在 `main` 的 Windows 分支）调用它，把控制权交给 SCM。
///
/// # Errors
///
/// 当进程不是由 SCM 以服务方式拉起（如直接双击运行）时，`service_dispatcher::start`
/// 会返回错误——这是预期行为，调用方可据此回退到普通 GUI/CLI 模式。
pub fn run_as_service() -> windows_service::Result<()> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)
}

// 生成名为 `ffi_service_main` 的 `extern "system"` FFI 入口，转调安全的 `service_main`。
define_windows_service!(ffi_service_main, service_main);

/// SCM 回调的服务主函数。任何错误都应落到事件日志（见 `integration::log_error`）。
fn service_main(_arguments: Vec<OsString>) {
    if let Err(_e) = run_service() {
        // 真实部署中应写入 Windows 事件日志：
        // let _ = crate::integration::log_error(SERVICE_NAME, &_e.to_string());
    }
}

fn run_service() -> windows_service::Result<()> {
    // 用核心状态机驱动对外汇报：初始 Stopped。
    let mut state = ServiceState::default();

    // Stop 控制事件通过 channel 通知主循环退出。
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            // Interrogate：SCM 探询当前状态，回 NoError 即可。
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            // Stop：通知主循环收尾。
            ServiceControl::Stop => {
                let _ = shutdown_tx.send(());
                ServiceControlHandlerResult::NoError
            }
            // 其余控制暂不支持。
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    // 注册控制处理器，拿到用于汇报状态的句柄。
    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

    // Stopped --Start--> Running，并向 SCM 汇报 Running。
    state = state.apply(ServiceCommand::Start).unwrap_or(state);
    debug_assert_eq!(state, ServiceState::Running);
    status_handle.set_service_status(status(ScmState::Running, ServiceControlAccept::STOP))?;

    // 主工作循环：周期性做事，直到收到 Stop。
    loop {
        match shutdown_rx.recv_timeout(Duration::from_secs(1)) {
            // 收到 Stop 或发送端断开：退出循环。
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            // 超时：此处放置周期性后台工作。
            Err(mpsc::RecvTimeoutError::Timeout) => { /* periodic work */ }
        }
    }

    // Running --Stop--> Stopped，并向 SCM 汇报 Stopped（不再接受任何控制）。
    state = state.apply(ServiceCommand::Stop).unwrap_or(state);
    debug_assert_eq!(state, ServiceState::Stopped);
    status_handle.set_service_status(status(ScmState::Stopped, ServiceControlAccept::empty()))?;
    Ok(())
}

/// 构造一份 SCM 状态结构。把样板集中在一处，避免每次汇报都重复一长串字段。
fn status(current_state: ScmState, controls_accepted: ServiceControlAccept) -> ServiceStatus {
    ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state,
        controls_accepted,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    }
}
