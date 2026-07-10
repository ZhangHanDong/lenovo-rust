//! FFI 边界：把核心的"报告指纹"能力导出给 C/C++ 宿主调用（第 7 课）。
//!
//! 这里演示的是**导出方向**的互操作：一个 `extern "C"` 函数，C/C++ 端拿到报告的
//! canonical 字符串后可调用它，得到与 Rust 端**逐位一致**的 64 位指纹，用于校验
//! "Rust 采集出的报告"和"宿主看到的报告"是否同一份。
//!
//! 与 C 头文件对应（客户在 Windows/任意平台的 C 工程里这样声明）：
//! ```c
//! #include <stdint.h>
//! uint64_t capstone_report_fingerprint(const char *canonical);
//! ```
//!
//! 设计纪律：unsafe 只出现在解引用裸指针那一步，且被 `# Safety` 契约约束；
//! 真正的算法放在安全的 [`fingerprint`] 里，可在 macOS 上无 unsafe 地单测。

use std::ffi::CStr;
use std::os::raw::c_char;

/// FNV-1a 64 位哈希。纯函数、确定性、无依赖——Rust 与 C/C++ 端实现一致即可互验。
#[must_use]
pub fn fingerprint(s: &str) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    for byte in s.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// C ABI 导出：对一个 NUL 结尾的 C 字符串取 [`fingerprint`]。
///
/// 返回 0 表示输入无效（空指针或非 UTF-8）——`fingerprint("")` 本身不为 0，
/// 故 0 可作为哨兵值供宿主判错。
///
/// # Safety
/// 调用方必须保证 `ptr` 要么为空，要么指向一个有效的、以 NUL 结尾的 C 字符串，
/// 且在本调用期间保持有效、不被其它线程修改。违反此契约是未定义行为。
#[no_mangle]
pub unsafe extern "C" fn capstone_report_fingerprint(ptr: *const c_char) -> u64 {
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: 上面排除了空指针；其余有效性由函数的 `# Safety` 契约转嫁给调用方。
    let cstr = unsafe { CStr::from_ptr(ptr) };
    match cstr.to_str() {
        Ok(s) => fingerprint(s),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn fnv1a_known_vectors() {
        assert_eq!(fingerprint(""), 0xcbf2_9ce4_8422_2325);
        // 不同输入得到不同指纹（碰撞极小概率，足够做一致性校验）。
        assert_ne!(fingerprint("a"), fingerprint("b"));
    }

    #[test]
    fn extern_matches_safe_helper() {
        let canonical = "schema=1;os=windows;host=DESK-01;cpus=16";
        let c = CString::new(canonical).unwrap();
        // SAFETY: c 指向有效的 NUL 结尾字符串，且在调用期间存活。
        let via_ffi = unsafe { capstone_report_fingerprint(c.as_ptr()) };
        assert_eq!(via_ffi, fingerprint(canonical));
    }

    #[test]
    fn null_pointer_returns_zero() {
        // SAFETY: 显式传入空指针，函数契约允许且返回 0。
        let v = unsafe { capstone_report_fingerprint(std::ptr::null()) };
        assert_eq!(v, 0);
    }
}
