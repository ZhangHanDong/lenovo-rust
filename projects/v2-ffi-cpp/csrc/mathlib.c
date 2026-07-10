/* mathlib.c —— 迷你 C 库实现，由 build.rs 经 cc crate（底层调用系统 clang）
 * 编译为静态库并链接进 v2-ffi-cpp。函数语义见 mathlib.h。 */
#include "mathlib.h"

#include <limits.h>
#include <stdint.h>

#if INT_MAX != 2147483647 || INT_MIN != (-2147483647 - 1)
#error "v2-ffi-cpp assumes a 32-bit two's-complement C int"
#endif

int ffi_add(int a, int b) {
    /* C 的有符号溢出是 UB；用 uint32_t 做模 2^32 相加，再用完全定义的分支
     * 映射回 int32_t 取值区间，避免 unsigned -> signed 越界转换的实现定义行为。 */
    uint32_t wrapped = (uint32_t)a + (uint32_t)b;
    if (wrapped <= (uint32_t)INT_MAX) {
        return (int)wrapped;
    }
    return -1 - (int)(UINT32_MAX - wrapped);
}

size_t ffi_strlen(const char *s) {
    size_t n = 0;
    while (s[n] != '\0') {
        n++;
    }
    return n;
}

size_t ffi_to_upper(const char *src, char *dst, size_t dst_cap) {
    size_t n = 0;
    while (src[n] != '\0') {
        n++;
    }
    /* 需要 n 个字符 + 1 个结尾 NUL，缓冲区不够则返回哨兵，绝不越界写。 */
    if (dst_cap < n + 1) {
        return (size_t)-1;
    }
    for (size_t i = 0; i < n; i++) {
        char c = src[i];
        if (c >= 'a' && c <= 'z') {
            c = (char)(c - ('a' - 'A'));
        }
        dst[i] = c;
    }
    dst[n] = '\0';
    return n;
}

int ffi_safe_div(int a, int b, int *out) {
    if (b == 0) {
        return -1; /* 除零哨兵：不写 *out，由调用方据返回码处理。 */
    }
    *out = a / b;
    return 0;
}
