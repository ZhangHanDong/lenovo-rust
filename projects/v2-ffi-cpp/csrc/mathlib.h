/* mathlib.h —— 第 7 课配套的迷你 C 库接口声明。
 *
 * 这个头文件模拟"现有 C/C++ 团队留下的一个小模块"：几个纯 C 函数，
 * 覆盖 FFI 里最典型的三类参数形态——纯值、只读指针、输出缓冲区。
 * Rust 侧不使用 bindgen（libclang 不一定具备），而是在 src/lib.rs 里
 * 手写对应的 `extern "C"` 声明，再包一层安全 API。真实迁移项目中，
 * 这个头文件正是你要交给 cbindgen / bindgen 或人工对照的"ABI 契约"。
 */
#ifndef V2_FFI_CPP_MATHLIB_H
#define V2_FFI_CPP_MATHLIB_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* 纯值进、纯值出：最简单的 FFI 形态，无指针、无所有权问题。
 * 溢出语义：定义为环绕（wrap-around）——实现内部经 uint32_t 模加法和显式区间映射，
 * 避免 C 有符号溢出 UB 与 unsigned->signed 越界转换，使 Rust 侧能把它包成真正安全的 API。 */
int ffi_add(int a, int b);

/* 只读指针：调用方拥有字符串，被调方只读不持有、不释放。
 * 约定 s 必须是以 NUL 结尾的合法 C 字符串。 */
size_t ffi_strlen(const char *s);

/* 输出缓冲区：调用方分配 dst（容量 dst_cap，含结尾 NUL），
 * 被调方把 src 转大写写入 dst。返回写入的字符数（不含 NUL）；
 * 若缓冲区不足以容纳整串 + NUL，返回 (size_t)-1 作为哨兵，不越界写。 */
size_t ffi_to_upper(const char *src, char *dst, size_t dst_cap);

/* 带哨兵的整数除法：除零时返回 -1（错误），否则返回 0 并把商写入 *out。
 * 演示"用返回码 + out 参数表达可失败操作"的经典 C 约定。 */
int ffi_safe_div(int a, int b, int *out);

#ifdef __cplusplus
}
#endif

#endif /* V2_FFI_CPP_MATHLIB_H */
