/* SDK-owned kinfo_proc layout and group-list wrapper; never signals a process. */
#define _DARWIN_C_SOURCE
#include <sys/types.h>
#include <sys/sysctl.h>
#include <sys/proc.h>
#include <stdint.h>
#include <string.h>
#include <errno.h>
#include <limits.h>
#include <libproc.h>

struct nexus_darwin_member {
    int32_t pid;
    int32_t pgid;
    int32_t state;
    int64_t start_seconds;
    int64_t start_microseconds;
};

size_t nexus_darwin_proc_record_size(void) {
    return sizeof(struct kinfo_proc);
}

int nexus_darwin_proc_decode(const unsigned char *bytes, size_t length,
                            size_t index, struct nexus_darwin_member *out) {
    if (bytes == NULL || out == NULL || length % sizeof(struct kinfo_proc) != 0 ||
        index >= length / sizeof(struct kinfo_proc)) {
        return EINVAL;
    }
    /* Rust's byte buffer need not have the SDK structure's alignment. */
    struct kinfo_proc record;
    memcpy(&record, bytes + index * sizeof(record), sizeof(record));
    out->pid = record.kp_proc.p_pid;
    out->pgid = record.kp_eproc.e_pgid;
    out->state = record.kp_proc.p_stat;
    out->start_seconds = record.kp_proc.p_starttime.tv_sec;
    out->start_microseconds = record.kp_proc.p_starttime.tv_usec;
    return 0;
}

/* Unlike sysctl's iterator, this group-filtered API includes SIDL members.
 * Its wrapper returns zero on both error and empty success; clear/check errno.
 * A full buffer may be truncated and must never be accepted as complete.
 */
int nexus_darwin_group_pids(int32_t pgid, void *buffer, size_t capacity,
                           size_t *written) {
    if (pgid <= 0 || capacity > INT_MAX || written == NULL) return EINVAL;
    errno = 0;
    int result = proc_listpids(PROC_PGRP_ONLY, (uint32_t)pgid, buffer, (int)capacity);
    if (result == 0 && errno != 0) return errno;
    if (result < 0) return EIO;
    if (buffer != NULL && (size_t)result >= capacity) return ENOMEM;
    *written = (size_t)result;
    return 0;
}
