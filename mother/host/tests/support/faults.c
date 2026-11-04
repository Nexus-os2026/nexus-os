/* M2d-D test process interposition only. Not linked into any product.
 * Holds actual libc I/O boundaries, with an exact receipt and bounded release.
 * Production validation and broker admission are unchanged. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <poll.h>
#include <stdatomic.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/un.h>
#include <unistd.h>

static atomic_int held;
static atomic_int recording;

static int mode(const char *wanted) {
    const char *value = getenv("M2DD_FAULT");
    const char *arm = getenv("M2DD_ARM");
    return value && !strcmp(value, wanted) && (!arm || access(arm, F_OK) == 0);
}

static int path_of(int fd, char *path, size_t capacity) {
    char source[64];
    snprintf(source, sizeof(source), "/proc/self/fd/%d", fd);
    ssize_t n = readlink(source, path, capacity - 1);
    if (n < 0) return 0;
    path[n] = 0;
    return 1;
}

static void receipt(const char *kind, const char *path, const void *data, size_t n) {
    const char *name = getenv("M2DD_SOCKET");
    if (!name || strlen(name) > 90) _exit(120);
    struct sockaddr_un addr = { .sun_family = AF_UNIX };
    memcpy(addr.sun_path + 1, name, strlen(name));
    int fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
    if (fd < 0 || connect(fd, (struct sockaddr *)&addr,
        offsetof(struct sockaddr_un, sun_path) + 1 + strlen(name))) _exit(121);
    char header[4096];
    int length = snprintf(header, sizeof(header), "%s\n%s\n%zu\n", kind, path, n);
    if (length < 0 || (size_t)length >= sizeof(header)) _exit(123);
    if (write(fd, header, length) != length) _exit(124);
    if (n && write(fd, data, n) != (ssize_t)n) _exit(125);
    struct pollfd ready = { .fd = fd, .events = POLLIN };
    char release = 0;
    if (poll(&ready, 1, 15000) != 1 || read(fd, &release, 1) != 1 || release != 'R') _exit(122);
    close(fd);
}

static void boundary(const char *kind, const char *path, const void *data, size_t n) {
    if (atomic_exchange(&held, 1)) return;
    receipt(kind, path, data, n);
}

int fsync(int fd) {
    int (*real)(int) = dlsym(RTLD_NEXT, "fsync");
    char path[4096];
    if (path_of(fd, path, sizeof(path))) {
        const char *parent = getenv("M2DD_JOURNAL_PARENT");
        if (mode("journal-dir-fail") && parent && !strcmp(parent, path)) {
            errno = EIO; return -1;
        }
        const char *workspace = getenv("M2DD_WORKSPACE");
        if (mode("before-publication") && workspace &&
            !strncmp(path, workspace, strlen(workspace)) && strstr(path, "(deleted)")) {
            int result = real(fd);
            if (!result) boundary("before-publication", path, NULL, 0);
            return result;
        }
    }
    return real(fd);
}

int linkat(int oldfd, const char *old, int newfd, const char *name, int flags) {
    int (*real)(int,const char *,int,const char *,int) = dlsym(RTLD_NEXT, "linkat");
    const char *workspace = getenv("M2DD_WORKSPACE");
    char path[4096];
    if (mode("after-final-check") && workspace && path_of(newfd, path, sizeof(path)) &&
        !strcmp(workspace, path) && !strcmp(name, "hello.txt")) {
        boundary("after-final-check", path, NULL, 0);
    }
    return real(oldfd, old, newfd, name, flags);
}

ssize_t pwrite64(int fd, const void *data, size_t size, off64_t offset) {
    ssize_t (*real)(int,const void *,size_t,off64_t) = dlsym(RTLD_NEXT, "pwrite64");
    char path[4096];
    if (mode("evidence") && path_of(fd, path, sizeof(path)) &&
        strstr(path, "/p3-evidence/evidence.sqlite3")) boundary("evidence", path, NULL, 0);
    /* Execute is held before its final check. Only emergency recording can
     * reach this evidence write until that owned execution is released. */
    if (mode("before-publication") && atomic_load(&held) &&
        path_of(fd, path, sizeof(path)) && strstr(path, "/p3-evidence/evidence.sqlite3") &&
        !atomic_exchange(&recording, 1)) receipt("stop-recording", path, NULL, 0);
    return real(fd, data, size, offset);
}

ssize_t write(int fd, const void *data, size_t size) {
    ssize_t (*real)(int,const void *,size_t) = dlsym(RTLD_NEXT, "write");
    if (fd == STDOUT_FILENO && mode("display")) boundary("display", "stdout", data, size);
    if (fd == STDOUT_FILENO && mode("write-fail")) { errno = EIO; return -1; }
    return real(fd, data, size);
}

ssize_t read(int fd, void *data, size_t size) {
    ssize_t (*real)(int,void *,size_t) = dlsym(RTLD_NEXT, "read");
    if (fd == STDIN_FILENO && mode("read-fail")) { errno = EIO; return -1; }
    return real(fd, data, size);
}

int unlinkat(int fd, const char *name, int flags) {
    int (*real)(int,const char *,int) = dlsym(RTLD_NEXT, "unlinkat");
    char path[4096];
    if (path_of(fd, path, sizeof(path)) && strstr(path, "/coding-runs/") &&
        !strcmp(name, "hello.txt")) {
        if (mode("cleanup")) boundary("cleanup", path, NULL, 0);
        if (mode("cleanup-fail") && !atomic_exchange(&held, 1)) { errno = EIO; return -1; }
    }
    return real(fd, name, flags);
}

int unlink(const char *name) {
    int (*real)(const char *) = dlsym(RTLD_NEXT, "unlink");
    int fd;
    char tail[128], path[4096];
    if (sscanf(name, "/proc/self/fd/%d/%127s", &fd, tail) == 2 &&
        path_of(fd, path, sizeof(path)) && strstr(path, "/coding-runs/") &&
        !strcmp(tail, "hello.txt")) {
        if (mode("cleanup")) boundary("cleanup", path, NULL, 0);
        if (mode("cleanup-fail") && !atomic_exchange(&held, 1)) { errno = EIO; return -1; }
    }
    return real(name);
}
