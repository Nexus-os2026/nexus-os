/* SB-3E U18 read shim (B6 read_failure), preloaded only into a reader child.
 * While the file named by ROUTE_READ_ARM exists, read(2) on fd 0 fails with
 * EIO; every other call is the real one. The harness creates that file
 * (std::fs, no FFI), then writes one byte so the reader's poll wakes. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <stdlib.h>
#include <unistd.h>

ssize_t read(int fd, void *data, size_t size) {
    ssize_t (*real)(int, void *, size_t) = dlsym(RTLD_NEXT, "read");
    const char *arm = getenv("ROUTE_READ_ARM");
    if (fd == STDIN_FILENO && arm && access(arm, F_OK) == 0) {
        errno = EIO;
        return -1;
    }
    return real(fd, data, size);
}
