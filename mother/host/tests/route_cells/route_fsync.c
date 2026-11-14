/* SB-3E external shim (design 4.2, 4.2.1), preloaded only into a route-cells
 * child. It fails exactly one fsync or fdatasync: the first whose calling
 * thread and canonical path equal the control file's "<tid> <path>" line,
 * which the harness renames into place (std::fs, no FFI) while the route is
 * paused at `store.commit`. Firing unlinks the control file. Every call on
 * an evidence store file is logged, so consumption is read back by thread,
 * file and count. Nothing else is changed. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/syscall.h>
#include <unistd.h>

static int fired;

static void logged(const char *line) {
    const char *log = getenv("ROUTE_SHIM_LOG");
    if (!log) _exit(96);
    int fd = open(log, O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC, 0600);
    if (fd < 0) _exit(96);
    size_t n = strlen(line);
    if (write(fd, line, n) != (ssize_t)n) _exit(97);
    if (close(fd) != 0) _exit(97);
}

/* Whether the control file names exactly this thread and path. */
static int armed_for(long tid, const char *path) {
    const char *control = getenv("ROUTE_SHIM_CONTROL");
    if (!control) _exit(98);
    int c = open(control, O_RDONLY | O_CLOEXEC);
    if (c < 0) return 0;
    char line[4200];
    ssize_t got = read(c, line, sizeof line - 1);
    close(c);
    if (got <= 0) return 0;
    line[got] = '\0';
    char *end = NULL;
    long want = strtol(line, &end, 10);
    if (!end || *end != ' ' || want != tid) return 0;
    char *target = end + 1, *newline = strchr(target, '\n');
    if (!newline || newline[1] != '\0') return 0;
    *newline = '\0';
    if (strcmp(target, path) != 0) return 0;
    int expected = 0;
    if (!__atomic_compare_exchange_n(&fired, &expected, 1, 0, __ATOMIC_SEQ_CST, __ATOMIC_SEQ_CST))
        return 0;
    if (unlink(control) != 0) _exit(99);
    return 1;
}

static int fails(const char *fn, int fd) {
    char descriptor[64], path[4096], line[4400];
    snprintf(descriptor, sizeof descriptor, "/proc/self/fd/%d", fd);
    ssize_t size = readlink(descriptor, path, sizeof path - 1);
    if (size < 0) return 0;
    path[size] = '\0';
    if (!strstr(path, "evidence.sqlite3")) return 0;
    long tid = syscall(SYS_gettid);
    int fail = !__atomic_load_n(&fired, __ATOMIC_SEQ_CST) && armed_for(tid, path);
    snprintf(line, sizeof line, "%s %s %ld %s\n", fail ? "FIRED" : "CALL", fn, tid, path);
    logged(line);
    return fail;
}

int fsync(int fd) {
    if (fails("fsync", fd)) {
        errno = EIO;
        return -1;
    }
    int (*real)(int) = (int (*)(int))dlsym(RTLD_NEXT, "fsync");
    if (!real) _exit(95);
    return real(fd);
}

int fdatasync(int fd) {
    if (fails("fdatasync", fd)) {
        errno = EIO;
        return -1;
    }
    int (*real)(int) = (int (*)(int))dlsym(RTLD_NEXT, "fdatasync");
    if (!real) _exit(95);
    return real(fd);
}
