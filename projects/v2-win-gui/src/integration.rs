//! 系统集成（`#[cfg(windows)]`）：注册表（开机自启）+ Windows 事件日志。
//!
//! 用 `windows` crate 的安全投影调 Win32 注册表与事件日志 API，延续第 9 课的纪律：
//!
//! - **句柄 RAII**：`HKEY` 用 [`RegKey`] 包一层，`Drop` 里 `RegCloseKey`，
//!   任何 `?` 早退都不漏关；
//! - **错误用 `Result` + `?` 传播**：`WIN32_ERROR::ok()` 把返回码转成 `windows::core::Result`；
//! - **字符串跨界**：Rust UTF-8 ↔ Win32 UTF-16，注册表 `REG_SZ` 必须是 **NUL 结尾的 UTF-16**。
//!
//! 开机自启的本质：在 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`（路径取自核心
//! 常量 [`crate::AUTOSTART_REG_PATH`]）下写一个「值名=应用名、值=可执行文件路径」的 `REG_SZ`。
//!
//! 对照 C++：等价于 `RegCreateKeyEx`/`RegSetValueEx` 那套，但 Rust 把句柄释放与错误检查
//! 收进类型系统，免去手写 `if (lResult != ERROR_SUCCESS)` 与 `RegCloseKey` 配平。

use windows::core::{Result, HSTRING, PCWSTR};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
};

use crate::AppConfig;

/// `HKEY` 的 RAII 守卫：作用域结束自动 `RegCloseKey`，杜绝句柄泄漏。
struct RegKey(HKEY);

impl Drop for RegKey {
    fn drop(&mut self) {
        // SAFETY: self.0 来自成功的 RegCreateKeyExW/RegOpenKeyExW，仅在此关闭一次。
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

/// 把 Rust 字符串编码成 Win32 `REG_SZ` 期望的「NUL 结尾 UTF-16」字节序列。
fn reg_sz_bytes(s: &str) -> Vec<u8> {
    let utf16: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
    // 每个 u16 两个字节，按小端（x86/ARM Windows 均小端）展开。
    let mut bytes = Vec::with_capacity(utf16.len() * 2);
    for unit in utf16 {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}

/// 写入/更新当前用户的开机自启项：值名 = `cfg.app_name`，值 = `exe_path`。
///
/// # Errors
///
/// 创建/打开 `Run` 键或写值失败时，返回携带 `WIN32_ERROR` 的 `windows::core::Error`。
pub fn enable_autostart(cfg: &AppConfig, exe_path: &str) -> Result<()> {
    let subkey = HSTRING::from(crate::AUTOSTART_REG_PATH);
    let mut hkey = HKEY::default();

    // SAFETY: 子键路径以 NUL 结尾（HSTRING 保证）；phkResult 指向栈上 hkey；
    // 其余指针参数传 null/None。成功后 hkey 立即交给 RegKey 守卫。
    unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            0,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut hkey,
            None,
        )
        .ok()?;
    }
    let key = RegKey(hkey);

    let value_name = HSTRING::from(cfg.autostart_value_name());
    let data = reg_sz_bytes(exe_path);
    // SAFETY: key.0 有效；value_name 以 NUL 结尾；data 是合法 NUL 结尾 UTF-16 字节，
    // 长度与切片一致；REG_SZ 与数据编码匹配。
    unsafe {
        RegSetValueExW(key.0, PCWSTR(value_name.as_ptr()), 0, REG_SZ, Some(&data)).ok()?;
    }
    Ok(())
}

/// 删除当前用户的开机自启项。键或值不存在时也视具体返回码而定（这里直接传播）。
///
/// # Errors
///
/// 打开 `Run` 键或删除值失败时返回错误。
pub fn disable_autostart(cfg: &AppConfig) -> Result<()> {
    let subkey = HSTRING::from(crate::AUTOSTART_REG_PATH);
    let mut hkey = HKEY::default();

    // SAFETY: 同 enable_autostart；以 KEY_SET_VALUE 打开已存在的 Run 键。
    unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            0, // uloptions：windows 0.58 里是 u32（不是 Option），保留选项位为 0
            KEY_SET_VALUE,
            &mut hkey,
        )
        .ok()?;
    }
    let key = RegKey(hkey);

    let value_name = HSTRING::from(cfg.autostart_value_name());
    // SAFETY: key.0 有效；value_name 以 NUL 结尾。
    unsafe {
        RegDeleteValueW(key.0, PCWSTR(value_name.as_ptr())).ok()?;
    }
    Ok(())
}

// --- 事件日志 -------------------------------------------------------------

use windows::Win32::System::EventLog::{
    DeregisterEventSource, RegisterEventSourceW, ReportEventW, EVENTLOG_ERROR_TYPE,
    EVENTLOG_INFORMATION_TYPE, REPORT_EVENT_TYPE,
};

/// 向 Windows 应用程序事件日志写一条记录。`source` 为日志来源名（通常 = 应用名）。
///
/// 真实部署应先用安装程序在注册表注册「事件来源 + 消息 DLL」，否则事件查看器会提示
/// 「找不到事件 ID 的描述」。此处给出最小可调用骨架，消息以替换字符串形式写入。
///
/// # Errors
///
/// `RegisterEventSourceW` 或 `ReportEventW` 失败时返回错误。
fn report(source: &str, kind: REPORT_EVENT_TYPE, event_id: u32, message: &str) -> Result<()> {
    let src = HSTRING::from(source);
    // SAFETY: 服务器名传 null 表示本机；source 以 NUL 结尾。
    let handle = unsafe { RegisterEventSourceW(PCWSTR::null(), PCWSTR(src.as_ptr()))? };

    let msg = HSTRING::from(message);
    let strings = [PCWSTR(msg.as_ptr())];

    // SAFETY: handle 来自成功的 RegisterEventSourceW；strings 在调用期间存活，
    // 其长度（1）与 wNumStrings 一致；无 SID、无二进制数据。
    let result = unsafe { ReportEventW(handle, kind, 0, event_id, None, 0, Some(&strings), None) };

    // 无论成败都释放事件源句柄（手动配平：此 API 无 RAII 守卫）。
    // SAFETY: handle 有效，仅注销一次。
    unsafe {
        let _ = DeregisterEventSource(handle);
    }
    result
}

/// 写一条「信息」级事件日志。
///
/// # Errors
/// 见 [`report`]。
pub fn log_info(source: &str, message: &str) -> Result<()> {
    report(source, EVENTLOG_INFORMATION_TYPE, 1000, message)
}

/// 写一条「错误」级事件日志（服务异常时使用）。
///
/// # Errors
/// 见 [`report`]。
pub fn log_error(source: &str, message: &str) -> Result<()> {
    report(source, EVENTLOG_ERROR_TYPE, 1001, message)
}
