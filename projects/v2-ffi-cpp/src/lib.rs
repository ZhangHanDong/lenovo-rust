//! 第 7 课核心逻辑：**为一个 C 库构建对外完全安全的 Rust 封装**。
//!
//! 本 crate 不是"用 Rust 重写一个算法库"，而是把第 7 课讲的 FFI 方法论落到
//! 可编译、可测试的代码里：底层是一个迷你 C 库 `csrc/mathlib.c`（由 `build.rs`
//! 经 `cc` crate 调用系统 clang 编译并静态链接），Rust 侧**手写** `extern "C"`
//! 声明（不依赖 bindgen，因为 libclang 不一定具备），再在外面包一层**没有任何
//! `unsafe`** 的安全 API。
//!
//! 设计要点（对应讲义 §5）：
//!
//! - **`unsafe` 只出现在 FFI 调用点**，且每处都有 `// SAFETY:` 注释说明前提；
//!   对外的 [`add`] / [`c_strlen`] / [`to_upper`] / [`checked_div`] 全部是安全函数。
//! - **`CString` / `CStr` 管理跨界字符串**：Rust 的 `&str` 不以 NUL 结尾，调 C
//!   前必须用 [`CString`] 复制成 NUL 结尾的缓冲区；这一步也天然挡住"内嵌 NUL"
//!   这种非法输入（见 [`FfiError::InteriorNul`]）。
//! - **谁分配谁释放**：所有跨界缓冲区都由 **Rust 侧分配、Rust 侧释放**
//!   （`CString`、`Vec<u8>`），C 函数只读或只写调用方给的内存，从不 `malloc`
//!   返回指针给 Rust——这样就没有"该用 C 的 `free` 还是 Rust 的 dealloc"的歧义。
//! - **错误用 `Result`/`Option` 表达**：C 用返回码 + out 参数表达"可失败"
//!   （`ffi_safe_div` 除零返回 -1），Rust 封装把它翻译成地道的 [`Option`]。
//!
//! 关于 **panic 不可跨 FFI 边界**：本 crate 是"Rust 调 C"方向，C 不会回调 Rust，
//! 因此封装内部不会有 panic 穿过 C 栈的问题。反方向（导出 Rust 给 C 调用）必须
//! 用 `catch_unwind` 在边界拦截 panic——这一点在讲义 §2、§5 详述。
//!
//! 赏析锚点：**cxx**（<https://github.com/dtolnay/cxx>）。本 crate 用"手写
//! extern + CString/CStr + 安全包装"的方式触达 C ABI；cxx 则用一个共享的
//! `#[cxx::bridge]` 模块，让 Rust 与 C++ 双向调用都走自动生成的、带类型检查的
//! 桥接代码，几乎不写裸 `unsafe`。本 crate 是理解 cxx"为什么值得"的微缩对照。
//!
//! # 用法
//!
//! ```
//! use v2_ffi_cpp::{add, c_strlen, to_upper, checked_div};
//!
//! assert_eq!(add(2, 3), 5);
//! assert_eq!(c_strlen("héllo").unwrap(), "héllo".len()); // 按字节数（UTF-8）
//! assert_eq!(to_upper("Rust + C++").unwrap(), "RUST + C++");
//! assert_eq!(checked_div(10, 3), Some(3));
//! assert_eq!(checked_div(1, 0), None); // 除零哨兵 → None
//! ```

use std::ffi::{c_char, c_int, CStr, CString};
use std::fmt;

use libc::size_t;

// ---------------------------------------------------------------------------
// 手写的 `extern "C"` 声明：对应 csrc/mathlib.h 的 ABI 契约。
//
// 这一段就是 bindgen 会替我们自动生成的内容。课程里刻意手写，一来不依赖
// libclang，二来让你看清"FFI 声明 = 把 C 头文件的签名逐字翻译成 Rust 类型"：
//   int           -> c_int
//   const char*   -> *const c_char
//   char*         -> *mut c_char
//   size_t        -> libc::size_t
// `extern "C"` 块里的函数默认就是 `unsafe` 的——调用它们必须在 `unsafe` 块中。
// ---------------------------------------------------------------------------
extern "C" {
    fn ffi_add(a: c_int, b: c_int) -> c_int;
    fn ffi_strlen(s: *const c_char) -> size_t;
    fn ffi_to_upper(src: *const c_char, dst: *mut c_char, dst_cap: size_t) -> size_t;
    fn ffi_safe_div(a: c_int, b: c_int, out: *mut c_int) -> c_int;
}

/// 调用本 crate 的字符串相关封装时可能出现的错误。
#[derive(Debug, PartialEq, Eq)]
pub enum FfiError {
    /// 输入的 `&str` 含有内嵌 NUL 字节，无法构造合法的 C 字符串。
    ///
    /// 这是 Rust 字符串（可含任意字节、不以 NUL 结尾）与 C 字符串
    /// （以 NUL 结尾，因此正文不能含 NUL）阻抗不匹配的典型边界。
    InteriorNul,
}

impl fmt::Display for FfiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FfiError::InteriorNul => f.write_str("输入字符串含内嵌 NUL，无法转换为 C 字符串"),
        }
    }
}

impl std::error::Error for FfiError {}

/// 纯值 FFI：两个 `i32` 相加。最简单的形态，无指针、无所有权。
///
/// 溢出语义为**环绕**（如 `add(i32::MAX, 1) == i32::MIN`）：C 侧实现经
/// `uint32_t` 模加法和显式区间映射消除了有符号溢出 UB，安全封装的担保才能覆盖全部输入。
#[must_use]
pub fn add(a: i32, b: i32) -> i32 {
    // SAFETY: ffi_add 是纯函数，只对两个按值传入的整数做加法，不触碰任何指针
    // 或全局状态；c_int 与 i32 在本课程涉及的全部平台（Windows/macOS/Linux/
    // Android/iOS）上同为 32 位，ABI 一致。溢出情形 C 侧已定义为环绕语义
    // （mathlib.c 经 uint32_t 模加法和显式区间映射），因此不存在任何输入能触发
    // C 侧 UB 或实现定义的越界转换——对照 checked_div 拦截 i32::MIN / -1 的做法，
    // 安全担保必须覆盖全域输入。
    unsafe { ffi_add(a, b) }
}

/// 只读指针 FFI：返回 C 侧 `strlen` 测得的字节长度（不含结尾 NUL）。
///
/// 对 ASCII 字符串结果与 `s.len()` 一致；对含多字节 UTF-8 的字符串，
/// 结果是 **UTF-8 字节数**（C 的 `strlen` 按字节计），同样等于 `s.len()`。
///
/// # Errors
///
/// 若 `s` 含内嵌 NUL，返回 [`FfiError::InteriorNul`]。
pub fn c_strlen(s: &str) -> Result<usize, FfiError> {
    // CString::new 复制 s 并追加结尾 NUL；若 s 内含 NUL 则失败——在进入 unsafe
    // 之前就把非法输入挡在门外，这正是"安全封装"的价值。
    let cs = CString::new(s).map_err(|_| FfiError::InteriorNul)?;

    // SAFETY: cs.as_ptr() 指向一段以 NUL 结尾、在本语句期间始终存活的缓冲区
    // （cs 拥有它，直到函数返回才析构）；ffi_strlen 只读不写、不持有该指针，
    // 满足 mathlib.h 约定的"s 必须是合法 NUL 结尾字符串"。返回的 size_t 非负，
    // 转 usize 无损。
    let len = unsafe { ffi_strlen(cs.as_ptr()) };
    Ok(len)
}

/// 输出缓冲区 FFI：把 `s` 转大写。
///
/// 演示"调用方分配输出缓冲区、被调方写入"的经典 C 约定。缓冲区由 **Rust 分配
/// 和释放**（`Vec<u8>`），C 只负责往里写，从不参与内存生命周期。
///
/// 只有 ASCII 字母 `a..=z` 被转为大写，其余字节（含多字节 UTF-8）原样保留，
/// 因此结果对合法 UTF-8 输入仍是合法 UTF-8。
///
/// # Errors
///
/// 若 `s` 含内嵌 NUL，返回 [`FfiError::InteriorNul`]。
pub fn to_upper(s: &str) -> Result<String, FfiError> {
    let cs = CString::new(s).map_err(|_| FfiError::InteriorNul)?;
    let src_len = cs.as_bytes().len(); // 不含结尾 NUL 的字节数

    // Rust 侧分配输出缓冲区：src_len 个字符 + 1 个结尾 NUL。容量精确，
    // 因此 C 侧的"缓冲区不足"哨兵分支在此永远不会触发。
    let cap = src_len + 1;
    let mut dst: Vec<u8> = vec![0u8; cap];

    // SAFETY:
    // - src: cs.as_ptr() 指向存活的、NUL 结尾的只读缓冲区（cs 拥有，函数内有效）。
    // - dst: dst.as_mut_ptr() 指向 cap 个可写字节；我们传给 C 的容量 cap 与实际
    //   分配一致，C 侧实现保证写入不超过 cap（不足则返回哨兵且不写），故无越界。
    // - src 与 dst 是两块不重叠的独立分配，不存在别名问题。
    // - c_char 与 u8 同为 1 字节，指针转型 ABI 兼容。
    let written = unsafe {
        ffi_to_upper(
            cs.as_ptr(),
            dst.as_mut_ptr().cast::<c_char>(),
            cap as size_t,
        )
    };

    // 容量已精确分配，哨兵 (size_t)-1 不可能出现；用 debug_assert 把这条不变量
    // 写成可检查的契约，便于将来改动时及早暴露问题。
    debug_assert_ne!(written, size_t::MAX, "输出缓冲区按精确容量分配，不应溢出");

    // C 写入了 `written` 个字符 + 结尾 NUL；用 CStr 安全地读回这段以 NUL 结尾
    // 的内容，避免手算长度出错。
    // SAFETY: dst 的前 written+1 字节已被 C 写入且以 NUL 结尾（written < cap），
    // 指针指向存活的 dst 缓冲区，满足 CStr::from_ptr 的前提。
    let result = unsafe { CStr::from_ptr(dst.as_ptr().cast::<c_char>()) };

    // to_upper 不改动非 ASCII 字节，故对合法 UTF-8 输入输出仍是合法 UTF-8；
    // 用 to_string_lossy 兜底，绝不 panic。
    Ok(result.to_string_lossy().into_owned())
}

/// 带哨兵的整数除法：把 C 的"返回码 + out 参数"翻译成地道的 [`Option`]。
///
/// 除数为 0 时 C 侧返回 -1（错误码），本封装将其映射为 [`None`]；否则返回
/// `Some(商)`。这是把"C 风格可失败 API"包成"Rust 风格可失败 API"的范例。
#[must_use]
pub fn checked_div(a: i32, b: i32) -> Option<i32> {
    // i32::MIN / -1 的商 2^31 超出 i32 范围，C 侧的 `a / b` 对此是有符号溢出（UB）。
    // 作为「对外完全安全」的封装，这里先拦掉这个唯一的溢出情形（除零仍交给 C 演示错误码翻译）。
    if a == i32::MIN && b == -1 {
        return None;
    }
    let mut out: c_int = 0;
    // SAFETY: &mut out 指向当前栈帧上一个有效、对齐的 c_int；ffi_safe_div 仅在
    // 返回 0（成功）时写入 *out，失败时不写。我们据返回码决定是否读取 out，
    // 因此即使未写入也不会读到未定义内容（out 已初始化为 0）。
    let rc = unsafe { ffi_safe_div(a, b, &mut out) };
    if rc == 0 {
        Some(out)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_pure_value() {
        assert_eq!(add(2, 3), 5);
        assert_eq!(add(-4, 4), 0);
        assert_eq!(add(i32::MAX - 1, 1), i32::MAX);
        // 溢出为环绕语义（C 侧经 uint32_t 模加法，无 UB）——安全 API 全域可调。
        assert_eq!(add(i32::MAX, 1), i32::MIN);
        assert_eq!(add(i32::MIN, -1), i32::MAX);
    }

    #[test]
    fn strlen_matches_rust_byte_len() {
        assert_eq!(c_strlen("").unwrap(), 0);
        assert_eq!(c_strlen("hello").unwrap(), 5);
        // 多字节 UTF-8：C 的 strlen 按字节计，等于 Rust 的 .len()
        let s = "héllo";
        assert_eq!(c_strlen(s).unwrap(), s.len());
        assert!(s.len() > s.chars().count()); // 确认确有多字节字符
    }

    #[test]
    fn to_upper_ascii_and_preserves_unicode() {
        assert_eq!(to_upper("rust + c++").unwrap(), "RUST + C++");
        assert_eq!(to_upper("").unwrap(), "");
        // 非 ASCII 字节原样保留，只有 ASCII 字母被大写
        assert_eq!(to_upper("café").unwrap(), "CAFé");
    }

    #[test]
    fn checked_div_normal_and_zero_sentinel() {
        assert_eq!(checked_div(10, 3), Some(3));
        assert_eq!(checked_div(-9, 3), Some(-3));
        // 边界：除零哨兵 → None（C 侧返回 -1，封装翻译为 Option）
        assert_eq!(checked_div(1, 0), None);
        assert_eq!(checked_div(0, 0), None);
    }

    #[test]
    fn interior_nul_is_rejected() {
        // 边界：内嵌 NUL 的 Rust 字符串无法构造 C 字符串，安全封装返回错误而非 UB
        assert_eq!(c_strlen("a\0b"), Err(FfiError::InteriorNul));
        assert_eq!(to_upper("x\0y"), Err(FfiError::InteriorNul));
    }
}
