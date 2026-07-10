//! 平台适配层：把 [`HostProbe`] 落到具体操作系统上。
//!
//! 这是整个 crate 唯一带 `#[cfg(windows)]` 的地方——**平台差异被关在这一层**，
//! 核心（`lib.rs`）与 FFI（`ffi.rs`）完全平台无关。这正是第 13 课"平台无关核心 +
//! 薄适配层"的落地：换平台只换这一个文件，核心与测试一行不动。
//!
//! - **Windows**：调用真正的 Win32（`GetComputerNameExW` / `GetSystemInfo`），
//!   由客户在 Windows 机器上验证（第 9 课）。
//! - **其它平台（macOS/Linux）**：回退桩，用 `std` 能拿到的信息，保证交付机可
//!   `build`/`run`/`test`。

use crate::{CollectError, HostProbe, OsFamily};

/// 返回当前平台的默认探针。`main` 用它跑真实采集；测试用 `MockProbe` 不走这里。
///
/// 返回 `impl HostProbe`：调用方不关心具体是 Windows 探针还是回退桩——
/// 平台选择在编译期由 `cfg` 决定，零运行时开销。
#[must_use]
pub fn default_probe() -> impl HostProbe {
    PlatformProbe
}

/// 单元结构体探针：所有平台共用同一个名字，方法体由 `cfg` 分流。
struct PlatformProbe;

// ───────────────────────── Windows 实现（客户验证） ─────────────────────────
#[cfg(windows)]
impl HostProbe for PlatformProbe {
    fn raw_hostname(&self) -> Result<String, CollectError> {
        windows_imp::computer_name()
    }

    fn logical_cpus(&self) -> Result<u32, CollectError> {
        windows_imp::logical_cpus()
    }

    fn os_family(&self) -> OsFamily {
        OsFamily::Windows
    }
}

#[cfg(windows)]
mod windows_imp {
    use crate::CollectError;
    use windows::Win32::System::SystemInformation::{
        ComputerNamePhysicalDnsHostname, GetComputerNameExW, GetSystemInfo, SYSTEM_INFO,
    };

    /// 用 `GetComputerNameExW` 取物理 DNS 主机名。
    ///
    /// 经典的 Win32 "两次调用"模式：第一次传 null 缓冲拿所需长度，第二次真正写入。
    /// 这里把 unsafe 严格关进函数内部，对外只暴露安全的 `Result<String, _>`（第 6 课）。
    pub(super) fn computer_name() -> Result<String, CollectError> {
        use windows::core::PWSTR;

        let mut size: u32 = 0;
        // 第一次：lpbuffer = null，仅查询长度。预期返回错误（ERROR_MORE_DATA），
        // 但 size 被写入所需的 wchar 数（含结尾 NUL）。
        // SAFETY: 传入合法的 size 指针；buffer 为 null 是该 API 查询长度的约定用法。
        let _ = unsafe {
            GetComputerNameExW(ComputerNamePhysicalDnsHostname, PWSTR::null(), &mut size)
        };
        if size == 0 {
            return Err(CollectError::Probe(
                "GetComputerNameExW 返回长度 0".to_owned(),
            ));
        }

        let mut buf = vec![0u16; size as usize];
        // 第二次：真正写入。SAFETY: buf 已按上一步返回的 size 分配，size 指针有效。
        unsafe {
            GetComputerNameExW(
                ComputerNamePhysicalDnsHostname,
                PWSTR(buf.as_mut_ptr()),
                &mut size,
            )
        }
        .map_err(|e| CollectError::Probe(format!("GetComputerNameExW 失败：{e}")))?;

        // 第二次成功后 size 是不含结尾 NUL 的实际长度。
        let name = String::from_utf16_lossy(&buf[..size as usize]);
        Ok(name)
    }

    /// 用 `GetSystemInfo` 取逻辑处理器数。
    pub(super) fn logical_cpus() -> Result<u32, CollectError> {
        let mut info = SYSTEM_INFO::default();
        // SAFETY: 传入一个有效、可写的 SYSTEM_INFO 指针；该调用仅填充结构体。
        unsafe { GetSystemInfo(&mut info) };
        Ok(info.dwNumberOfProcessors)
    }
}

// ───────────────────── 非 Windows 回退实现（交付机可跑） ─────────────────────
#[cfg(not(windows))]
impl HostProbe for PlatformProbe {
    fn raw_hostname(&self) -> Result<String, CollectError> {
        // 无第三方依赖的回退：优先读常见环境变量，否则给一个确定的占位名。
        // 真实主机名采集在 Windows 路径里走 Win32；此处仅保证交付机可编译/运行。
        let name = std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| "localhost".to_owned());
        Ok(name)
    }

    fn logical_cpus(&self) -> Result<u32, CollectError> {
        // std 的可移植入口：拿不到时退化为 1（而非 0），保证下游 CpuCount 校验通过。
        let n = std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(1);
        Ok(u32::try_from(n).unwrap_or(u32::MAX))
    }

    fn os_family(&self) -> OsFamily {
        if cfg!(target_os = "macos") {
            OsFamily::MacOs
        } else if cfg!(target_os = "linux") {
            OsFamily::Linux
        } else {
            OsFamily::Other
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collect_report;

    #[test]
    fn default_probe_collects_on_this_host() {
        // 在交付机（macOS）上，默认探针必须能产出一份有效报告。
        let report = collect_report(&default_probe()).expect("default probe works on host");
        assert!(report.logical_cpus.get() >= 1);
        assert!(!report.hostname.as_str().is_empty());
    }
}
