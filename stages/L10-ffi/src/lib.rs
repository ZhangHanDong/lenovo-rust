//! L10 作业：接入遗留 C 校验库——FFI 的信任边界
//!
//! `csrc/checksum.c` 是一个"遗留的" C 校验库，事件入库前要走它校验。
//! 你的任务：声明 `extern "C"` 绑定，写**安全封装层**，让业务代码零 unsafe。
//!
//! 跨边界三条规则：
//!   1. 所有跨界 struct 标 `#[repr(C)]`（本课用基本类型，不涉及自定义 struct）；
//!   2. 每个 `unsafe` 块写 `// SAFETY:`，说清依赖的 C 侧契约；
//!   3. C 字符串**谁分配谁释放**——`wm_describe` 返回库内静态缓冲，我们**立即拷成 String**，不持有指针。

use std::ffi::CStr;
use std::os::raw::{c_char, c_uchar};

// C 侧原始声明——只有封装层能碰它们。
extern "C" {
    fn wm_checksum(data: *const c_uchar, len: usize) -> c_uchar;
    fn wm_describe(sum: c_uchar) -> *const c_char;
}

/// 安全封装：算一段数据的校验和。空切片合法（返回 0）。
pub fn checksum(data: &[u8]) -> u8 {
    todo!("L10：调用 wm_checksum；空切片时 as_ptr 仍有效、len 为 0，C 侧不解引用")
}

/// 安全封装：把校验和描述成字符串。**立即拷贝**，不持有 C 指针。
///
/// C 侧的 `wm_describe` 用的是**非线程安全的静态缓冲**——两个线程并发调用会互相
/// 覆写（数据竞争 UB）。安全封装必须替调用方扛下这个契约：用一把锁把调用串行化，
/// 这样这个 safe fn 才配得上"怎么用都不会 UB"（L9 的标准）。
pub fn describe(sum: u8) -> String {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = LOCK.lock().unwrap();
    todo!("L10：调用 wm_describe，用 CStr 立即拷成 String（内存归库，不 free）")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_basic() {
        assert_eq!(checksum(&[1, 2, 3]), 6);
        assert_eq!(checksum(&[100, 200]), 44); // 300 mod 256
    }

    #[test]
    fn checksum_empty_slice() {
        // 边界：空切片不能让 C 侧解引用空指针。
        assert_eq!(checksum(&[]), 0);
    }

    #[test]
    fn checksum_long_data() {
        // 边界：超长数据（一圈溢出）也不崩。
        let data = vec![1u8; 1000];
        assert_eq!(checksum(&data), (1000 % 256) as u8);
    }

    #[test]
    fn describe_copies_c_string() {
        let s = describe(42);
        assert_eq!(s, "checksum=42");
        // s 是拥有所有权的 String，C 缓冲之后被覆盖也不影响它。
        let _ = describe(7);
        assert_eq!(s, "checksum=42");
    }
}
