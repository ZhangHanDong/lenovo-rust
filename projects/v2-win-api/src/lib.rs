//! 第 9 课配套工程：用 Rust 调用 Windows API（枚举进程）。
//!
//! 本 crate 刻意切成两层，演示「平台无关核心 + 平台适配层」这条跨平台主线：
//!
//! - **平台无关核心**（本文件顶层）：把「进程列表」抽象成 [`ProcessInfo`]，
//!   并提供纯函数 [`filter_by_name`] / [`sort_by_pid`] / [`format_table`]。
//!   这些逻辑不碰任何系统 API，**在 macOS / Linux / Windows 上行为一致、可单测**。
//! - **Windows 适配层**（[`mod@win`]，`#[cfg(windows)]`）：用 `windows` crate 调用
//!   ToolHelp32Snapshot 真正枚举系统进程，把结果填进 `Vec<ProcessInfo>`。
//! - **非 Windows 桩**（`#[cfg(not(windows))]`）：返回一份示例数据，使 bin 在
//!   macOS 桌面也能 `cargo run` 跑通核心逻辑（过滤 / 排序 / 格式化）。
//!
//! 关键工程取舍：`windows` 依赖写在本 crate 自己 `Cargo.toml` 的
//! `[target.'cfg(windows)'.dependencies]` 下，**交付机（macOS）根本不会编译 windows crate**。
//! 这正是第 9–13 课跨平台条件编译的核心套路：Windows 代码隔离在 `cfg(windows)` 之后，
//! 不拖累其它平台的构建，由客户在 Windows 上验证。
//!
//! 赏析锚点：`windows-rs`——`.winmd` 元数据驱动代码生成，`windows-sys`（零开销原始 FFI）
//! 与 `windows`（COM/WinRT 安全投影）的分层，是「把 unsafe 关进安全抽象」的工业级范本。

/// 一条进程信息。这是**平台无关**的领域类型：Windows 适配层与桩都向它收敛。
///
/// 字段刻意只保留跨平台都有意义的两项，避免把 Win32 的 `PROCESSENTRY32W`
/// 细节泄漏到核心逻辑里——核心逻辑只认 `ProcessInfo`，不认任何系统结构体。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    /// 进程 ID（Win32 的 `th32ProcessID`，跨平台语义上的 pid）。
    pub pid: u32,
    /// 进程映像名（如 `explorer.exe`）。
    pub name: String,
}

impl ProcessInfo {
    /// 便捷构造，主要服务于桩数据与测试。
    #[must_use]
    pub fn new(pid: u32, name: impl Into<String>) -> Self {
        Self {
            pid,
            name: name.into(),
        }
    }
}

/// 按名字子串**大小写不敏感**过滤（借用输入，返回借用，零拷贝）。
///
/// 入参借用 `&[ProcessInfo]`、返回 `Vec<&ProcessInfo>`——不夺走所有权、不复制字符串，
/// 是否物化由调用方决定。这是第 1 课「借用优先」惯用法在本课的延续。
#[must_use]
pub fn filter_by_name<'a>(procs: &'a [ProcessInfo], needle: &str) -> Vec<&'a ProcessInfo> {
    let needle = needle.to_lowercase();
    procs
        .iter()
        .filter(|p| p.name.to_lowercase().contains(&needle))
        .collect()
}

/// 按 pid 升序原地排序。
pub fn sort_by_pid(procs: &mut [ProcessInfo]) {
    procs.sort_by_key(|p| p.pid);
}

/// 把进程列表格式化成对齐的文本表格。纯函数：相同输入恒得相同输出，便于快照式测试。
#[must_use]
pub fn format_table(procs: &[ProcessInfo]) -> String {
    let mut out = String::from("   PID  NAME\n");
    for p in procs {
        out.push_str(&format!("{:>6}  {}\n", p.pid, p.name));
    }
    out
}

/// 列出当前系统的进程。**这是核心与平台层的唯一汇合点**。
///
/// - 在 Windows 上：转发到 [`mod@win`]，真正调用 ToolHelp32Snapshot；失败则返回空表。
/// - 在其它平台上：返回桩示例数据，让桌面也能演示核心逻辑。
#[must_use]
pub fn list_processes() -> Vec<ProcessInfo> {
    #[cfg(windows)]
    {
        // Win32 调用可能因权限等原因失败；这里把错误降级为空列表，
        // 真实工程里可改成返回 `Result` 向上传播（见讲义 5.3）。
        win::enumerate_processes().unwrap_or_default()
    }
    #[cfg(not(windows))]
    {
        stub::sample_processes()
    }
}

/// 非 Windows 平台的桩实现：返回一组示例进程，使 macOS / Linux 桌面可跑通核心逻辑。
#[cfg(not(windows))]
mod stub {
    use super::ProcessInfo;

    pub(super) fn sample_processes() -> Vec<ProcessInfo> {
        vec![
            ProcessInfo::new(4, "System"),
            ProcessInfo::new(1280, "explorer.exe"),
            ProcessInfo::new(2048, "Code.exe"),
            ProcessInfo::new(640, "svchost.exe"),
            ProcessInfo::new(9001, "cargo.exe"),
        ]
    }
}

/// Windows 适配层：用 `windows` crate 调用 Win32 ToolHelp API 枚举进程。
///
/// 整个模块在 `#[cfg(windows)]` 之后，macOS / Linux 构建时**不参与编译**，
/// 因此对 `windows` crate 的依赖也只在 Windows 目标被拉取。
#[cfg(windows)]
mod win {
    use super::ProcessInfo;
    use windows::core::Result;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    /// 句柄 RAII 守卫：作用域结束时自动 `CloseHandle`，对照 C++ 里手写的
    /// `HANDLE` + 「别忘了 CloseHandle」纪律。`windows` crate 亦提供
    /// `windows::core::Owned<HANDLE>` 做同样的事，这里手写一份以展示原理。
    struct SnapshotHandle(HANDLE);

    impl Drop for SnapshotHandle {
        fn drop(&mut self) {
            // SAFETY: self.0 来自 CreateToolhelp32Snapshot 成功返回的句柄，
            // 仅在此处关闭一次，关闭后该守卫即被丢弃，不会二次释放。
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    /// 用 ToolHelp32Snapshot 枚举系统进程。
    ///
    /// 错误处理：Win32 函数返回 `windows::core::Result<()>`（内部由 `HRESULT` 映射而来），
    /// 用 `?` 即可把失败的 `HRESULT` 当成 Rust 错误向上传播——对照 C++ 里到处手查
    /// `GetLastError()` / `FAILED(hr)`。
    pub(super) fn enumerate_processes() -> Result<Vec<ProcessInfo>> {
        let mut out = Vec::new();

        // SAFETY: CreateToolhelp32Snapshot 是 FFI 调用；参数为合法的快照标志与 0，
        // 返回的句柄随即交给 SnapshotHandle 管理生命周期，失败时 `?` 直接返回。
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)? };
        let _guard = SnapshotHandle(snapshot);

        // PROCESSENTRY32W 必须先把 dwSize 设成自身大小，否则 Process32FirstW 拒绝工作。
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        // SAFETY: snapshot 为有效快照句柄；entry 已正确初始化 dwSize，
        // 指针在调用期间有效；Process32FirstW 仅写入 entry 指向的栈内存。
        unsafe {
            if Process32FirstW(snapshot, &mut entry).is_err() {
                // 没有进程或失败：返回已收集到的（空）列表。
                return Ok(out);
            }
            loop {
                out.push(ProcessInfo {
                    pid: entry.th32ProcessID,
                    name: exe_name(&entry.szExeFile),
                });
                // Process32NextW 失败（含 ERROR_NO_MORE_FILES）即遍历结束。
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        Ok(out)
    }

    /// 把 Win32 的 UTF-16、以 NUL 结尾的固定数组 `szExeFile` 转成 Rust `String`。
    ///
    /// 字符串跨界是 Win32 的常见坑：Win32 的 `*W` API 用 UTF-16（`u16`），
    /// Rust `String` 是 UTF-8，必须显式转码，不能直接 `as` 强转。
    fn exe_name(buf: &[u16]) -> String {
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<ProcessInfo> {
        vec![
            ProcessInfo::new(4, "System"),
            ProcessInfo::new(1280, "explorer.exe"),
            ProcessInfo::new(2048, "Code.exe"),
            ProcessInfo::new(640, "svchost.exe"),
        ]
    }

    #[test]
    fn filter_is_case_insensitive_substring() {
        let procs = fixture();
        let hits = filter_by_name(&procs, "EXE");
        // explorer.exe / Code.exe / svchost.exe 三个含 "exe"（大小写不敏感）
        assert_eq!(hits.len(), 3);
        assert!(hits.iter().all(|p| p.name.to_lowercase().contains("exe")));
    }

    #[test]
    fn filter_no_match_returns_empty() {
        let procs = fixture();
        assert!(filter_by_name(&procs, "no-such-process").is_empty());
    }

    #[test]
    fn sort_orders_by_pid_ascending() {
        let mut procs = fixture();
        sort_by_pid(&mut procs);
        let pids: Vec<u32> = procs.iter().map(|p| p.pid).collect();
        assert_eq!(pids, vec![4, 640, 1280, 2048]);
    }

    #[test]
    fn format_table_has_header_and_one_row_per_process() {
        let procs = fixture();
        let table = format_table(&procs);
        assert!(table.starts_with("   PID  NAME\n"));
        // 1 行表头 + 4 行数据 = 5 行
        assert_eq!(table.lines().count(), 5);
        assert!(table.contains("explorer.exe"));
    }

    #[test]
    fn list_processes_is_callable_on_every_platform() {
        // macOS 上走桩、Windows 上走真实枚举；两者都应返回一个可格式化的列表。
        let procs = list_processes();
        let _ = format_table(&procs);
        // 桩保证非空；Windows 上系统也至少有若干进程。
        assert!(!procs.is_empty());
    }
}
