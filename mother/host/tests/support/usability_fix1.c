/* PR #98 review-1 schedules only. The frozen death harness stays untouched. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <fcntl.h>
#include <poll.h>
#include <pthread.h>
#include <stdatomic.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>

static atomic_int delivered;
static atomic_int held;

static int mode(const char *wanted) {
    const char *value = getenv("M2DD_FAULT");
    const char *arm = getenv("M2DD_ARM");
    return value && !strcmp(value, wanted) && arm && access(arm, F_OK) == 0;
}

static void boundary(const char *kind, const char *path) {
    if (atomic_exchange(&held, 1)) return;
    const char *name = getenv("M2DD_SOCKET");
    if (!name || strlen(name) > 90) _exit(120);
    struct sockaddr_un addr = { .sun_family = AF_UNIX };
    memcpy(addr.sun_path + 1, name, strlen(name));
    int fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
    if (fd < 0 || connect(fd, (struct sockaddr *)&addr,
        offsetof(struct sockaddr_un, sun_path) + 1 + strlen(name))) _exit(121);
    char header[4096];
    int n = snprintf(header, sizeof(header), "%s\n%s\n0\n", kind, path);
    if (n < 0 || (size_t)n >= sizeof(header) || write(fd, header, n) != n) _exit(122);
    struct pollfd ready = { .fd = fd, .events = POLLIN };
    char release = 0;
    if (poll(&ready, 1, 15000) != 1 || read(fd, &release, 1) != 1 || release != 'R') _exit(123);
    close(fd);
}

int fsync(int fd) {
    int (*real)(int) = dlsym(RTLD_NEXT, "fsync");
    int result = real(fd);
    char source[64], path[4096];
    snprintf(source, sizeof(source), "/proc/self/fd/%d", fd);
    ssize_t n = readlink(source, path, sizeof(path) - 1);
    if (!result && n > 0 && mode("schema-before-session")) {
        path[n] = 0;
        const char *suffix = "/broker/p3-evidence";
        if ((size_t)n >= strlen(suffix) && !strcmp(path + n - strlen(suffix), suffix)) {
            /* SQLite can sync its parent while merely entering WAL mode.
             * Require the checkpointed application identity in the main
             * file before selecting the post-schema directory sync. */
            int database = openat(fd, "evidence.sqlite3", O_RDONLY | O_NOFOLLOW | O_CLOEXEC);
            unsigned char header[100];
            ssize_t bytes = database < 0 ? -1 : pread(database, header, sizeof(header), 0);
            if (database >= 0) close(database);
            if (bytes == sizeof(header) && !memcmp(header, "SQLite format 3\0", 16) &&
                !memcmp(header + 68, "P3EV", 4))
                boundary("schema-before-session", path);
        }
    }
    return result;
}

ssize_t write(int fd, const void *data, size_t size) {
    ssize_t (*real)(int,const void *,size_t) = dlsym(RTLD_NEXT, "write");
    ssize_t result = real(fd, data, size);
    if (fd == STDOUT_FILENO && result == (ssize_t)size &&
        memmem(data, size, "SESSION CLOSED recorded", 23))
        atomic_store(&delivered, 1);
    return result;
}

int pthread_join(pthread_t thread, void **value) {
    int (*real)(pthread_t,void **) = dlsym(RTLD_NEXT, "pthread_join");
    int result = real(thread, value);
    /* Delivery has succeeded, and the renderer has actually joined. Hold
     * finalization's return, so its original deadline check must fail. */
    if (!result && atomic_load(&delivered) && mode("finalize-after-delivery"))
        boundary("finalize-after-delivery", "joined-after-delivery");
    return result;
}
