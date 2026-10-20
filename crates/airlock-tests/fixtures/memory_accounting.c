/* Expose the upstream reporter's C arithmetic to Rust-owned regression scenarios. */
#define _GNU_SOURCE
#include "memory-accounting.h"

int read_pcp(const char *text, long page_size, long long *result) {
    FILE *stream = fmemopen((void *)text, strlen(text), "r");
    if (!stream) return 0;
    int ok = pcp_free_kib(stream, page_size, result);
    fclose(stream);
    return ok;
}

int consistent(long long before, long long after, int threshold) {
    return pcp_sample_consistent(before, after, threshold);
}

int used(long long total, long long free_kib, long long pcp,
         long long buffers, long long cached, long long swap_total,
         long long swap_free, long long *result) {
    return memory_used_kib(total, free_kib, pcp, buffers, cached,
                           swap_total, swap_free, result);
}

#ifdef AIRLOCK_MEMORY_SELFTEST
/* Package checks verify the actual patched header compiles and runs. */
int main(void) {
    long long result = -1;
    if (!used(1048576, 600000, 0, 0, 65536, 0, 0, &result)) return 1;
    if (result != 383040 || !consistent(100000, 115000, 30000)) return 1;
    return 0;
}
#endif
