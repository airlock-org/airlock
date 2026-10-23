/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef QUBES_MEMORY_ACCOUNTING_H
#define QUBES_MEMORY_ACCOUNTING_H

#include <ctype.h>
#include <errno.h>
#include <limits.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Linux 6.12's NR_FREE_PAGES excludes free pages on the per-CPU lists.
 * Read the counts, not the high-water limits, across every zone and CPU.
 * Failure must never yield a partially accumulated correction. */
static inline bool pcp_free_kib(FILE *stream, long page_size,
                               long long *result)
{
    char *line = NULL;
    size_t capacity = 0;
    unsigned long long pages = 0;
    bool pagesets = false, want_count = false, saw_count = false;
    bool valid = page_size >= 1024 && page_size % 1024 == 0;

    if (!valid)
        return false;
    while (getline(&line, &capacity, stream) >= 0) {
        char *p = line, *end;
        unsigned long long count;

        while (isspace((unsigned char)*p))
            p++;
        if (!*p)
            continue;
        if (!strchr(p, '\n')) {
            valid = false;
            break;
        }
        if (!strncmp(p, "Node ", 5)) {
            if (want_count) {
                valid = false;
                break;
            }
            pagesets = false;
        } else if (!strcmp(p, "pagesets\n")) {
            pagesets = true;
        } else if (pagesets && !strncmp(p, "cpu:", 4)) {
            if (want_count) {
                valid = false;
                break;
            }
            want_count = true;
        } else if (pagesets && !strncmp(p, "count:", 6)) {
            p += 6;
            while (isspace((unsigned char)*p))
                p++;
            if (!want_count || !isdigit((unsigned char)*p)) {
                valid = false;
                break;
            }
            errno = 0;
            count = strtoull(p, &end, 10);
            while (isspace((unsigned char)*end))
                end++;
            if (errno || *end || count > ULLONG_MAX - pages) {
                valid = false;
                break;
            }
            pages += count;
            want_count = false;
            saw_count = true;
        } else if (want_count) {
            valid = false;
            break;
        }
    }
    valid = valid && feof(stream) && !ferror(stream) && !want_count && saw_count &&
        pages <= (unsigned long long)LLONG_MAX / (page_size / 1024);
    free(line);
    if (valid)
        *result = pages * (page_size / 1024);
    return valid;
}

/* Bracket meminfo with PCP snapshots. A large transfer between the buddy
 * allocator and a PCP cache otherwise looks like a change in demand.
 * Retry in the caller rather than publishing that inconsistent sample. */
static inline bool pcp_sample_consistent(long long before, long long after,
                                         int threshold)
{
    if (before < 0 || after < 0 || threshold <= 0)
        return false;
    return (before > after ? before - after : after - before) <= threshold / 2;
}

static inline bool memory_used_kib(long long total, long long free_kib,
                                   long long pcp_kib, long long buffers,
                                   long long cached, long long swap_total,
                                   long long swap_free, long long *used)
{
    long long remaining;
    const long long reclaimable[] = {free_kib, pcp_kib, buffers, cached};

    if (total < 0 || swap_total < 0 || swap_free < 0 || swap_free > swap_total ||
        total > LLONG_MAX - (swap_total - swap_free))
        return false;
    remaining = total + (swap_total - swap_free);
    for (size_t i = 0; i < sizeof(reclaimable) / sizeof(reclaimable[0]); i++) {
        if (reclaimable[i] < 0 || reclaimable[i] > remaining)
            return false;
        remaining -= reclaimable[i];
    }
    *used = remaining;
    return true;
}

#endif
