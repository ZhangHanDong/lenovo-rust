//! L11 作业：再封装一个 Win32 API——句柄用 RAII，错误用 `?`
//!
//! 采集器要加"内存信息"。真正的 `GlobalMemoryStatusEx` 调用在 `#[cfg(windows)]` 里
//! （见 SOLUTION.md，需在 Windows 机器上跑）。本骨架里**平台无关、可测**的两块是：
//!   1. `format_bytes`：把字节数格式化成人类可读（GB/MB）；
//!   2. `HandleGuard`：演示"句柄用 RAII 守卫自动关闭，代码里零个手写 CloseHandle"。
//!
//! 四项检查（审查 AI 的 Win32 封装用）：句柄 RAII · 错误用 `?` 不 unwrap ·
//! `dwSize` 填了 · 有错误路径测试。

/// 平台无关的内存信息（Windows 侧从 `MEMORYSTATUSEX` 填充）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub avail_bytes: u64,
}

impl MemoryInfo {
    /// 已用百分比（0–100）。
    pub fn used_percent(&self) -> u8 {
        if self.total_bytes == 0 {
            return 0;
        }
        let used = self.total_bytes - self.avail_bytes;
        ((used as u128 * 100) / self.total_bytes as u128) as u8
    }
}

/// 把字节数格式化成人类可读：`1536 MiB` 之类。
pub fn format_bytes(bytes: u64) -> String {
    todo!("L11：按 GiB/MiB/KiB/B 选最大合适单位，保留一位小数")
}

/// RAII 句柄守卫：持有一个"需要显式关闭的资源"，离开作用域时自动关闭。
///
/// 这正是 Win32 的正确姿势——**代码里不出现手写的 `CloseHandle`**，
/// 靠 `Drop` 保证每条失败路径上句柄都被关。这里用一个可观测的
/// `closed` 标志代替真实的 `CloseHandle`，好让测试验证它确实被调用了。
pub struct HandleGuard {
    raw: usize, // 假装是 HANDLE
    closed: std::rc::Rc<std::cell::Cell<bool>>,
}

impl HandleGuard {
    pub fn new(raw: usize, closed: std::rc::Rc<std::cell::Cell<bool>>) -> Self {
        HandleGuard { raw, closed }
    }
    pub fn raw(&self) -> usize {
        self.raw
    }
}

impl Drop for HandleGuard {
    fn drop(&mut self) {
        todo!("L11：这里相当于 CloseHandle(self.raw)——标记已关闭")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn format_bytes_picks_unit() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KiB");
        assert_eq!(format_bytes(1024 * 1024 * 3 / 2), "1.5 MiB");
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 2), "2.0 GiB");
    }

    #[test]
    fn used_percent() {
        let m = MemoryInfo {
            total_bytes: 100,
            avail_bytes: 25,
        };
        assert_eq!(m.used_percent(), 75);
    }

    #[test]
    fn guard_closes_on_drop() {
        let closed = Rc::new(Cell::new(false));
        {
            let g = HandleGuard::new(0x1234, Rc::clone(&closed));
            assert_eq!(g.raw(), 0x1234);
            assert!(!closed.get()); // 还没关
        } // 离开作用域 → Drop → 自动"关闭"
        assert!(closed.get(), "句柄应在离开作用域时自动关闭");
    }
}
