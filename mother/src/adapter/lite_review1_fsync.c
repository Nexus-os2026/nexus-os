/* Test-only, loaded in the isolated native publication oracle child. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

int fsync(int fd) {
    static int fired;
    const char *workspace = getenv("NEXUS_LITE_SYNC_FAILURE");
    if (!fired && workspace) {
        char descriptor[64], path[4096];
        snprintf(descriptor, sizeof descriptor, "/proc/self/fd/%d", fd);
        ssize_t size = readlink(descriptor, path, sizeof path - 1);
        if (size >= 0) {
            path[size] = '\0';
            if (strcmp(path, workspace) == 0) {
                const char *receipt = getenv("NEXUS_LITE_SYNC_RECEIPT");
                if (!receipt) _exit(91);
                int proof = open(receipt, O_WRONLY | O_CREAT | O_EXCL, 0600);
                if (proof < 0) _exit(92);
                const char mark[] = "workspace-fsync:EIO\n";
                if (write(proof, mark, sizeof mark - 1) != sizeof mark - 1) _exit(93);
                if (close(proof) != 0) _exit(94);
                fired = 1;
                errno = EIO;
                return -1;
            }
        }
    }
    int (*real_sync)(int) = (int (*)(int))dlsym(RTLD_NEXT, "fsync");
    if (!real_sync) _exit(95);
    return real_sync(fd);
}
