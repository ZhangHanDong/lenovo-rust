#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

/* 遗留 C 校验库：所有字节之和 mod 256。空数据返回 0。 */
uint8_t wm_checksum(const uint8_t *data, size_t len) {
    uint8_t sum = 0;
    for (size_t i = 0; i < len; i++) {
        sum += data[i];
    }
    return sum;
}

/* 返回指向**库内静态缓冲**的描述字符串。
   契约：内存归库所有，调用方**不得** free，且在下次调用前用完（这里立即拷走）。 */
const char *wm_describe(uint8_t sum) {
    static char buf[32];
    snprintf(buf, sizeof(buf), "checksum=%u", (unsigned)sum);
    return buf;
}
