/*
 * SB-3E route lane: RUSTC_WORKSPACE_WRAPPER (design 1.3).
 *
 * Adds `--cfg mother_route_seams` when, and only when, the crate name and the
 * source path both match one of the four named units. Adds nothing else.
 *
 * Process state: the compiler gets exactly the state this wrapper received.
 * The wrapper is a compiled, statically linked program so that nothing runs
 * before it can alter that state: an interpreter cannot be the first hop
 * (by measurement, Python's startup ignores SIGPIPE and SIGXFSZ and sets
 * LC_CTYPE under a C locale; dash discards an inherited ignored SIGCHLD and
 * its `exec` rebuilds the environment). This program changes no signal
 * disposition or mask, umask, rlimit or cwd, opens only its close-on-exec log
 * descriptor, and passes `environ` to execve untouched: the environment block
 * as received, in order. Static linking keeps the dynamic loader, and with it
 * LD_PRELOAD, LD_LIBRARY_PATH and LD_AUDIT, out of the wrapper.
 *
 * Built for the lane (CI, and GB1 later) with exactly:
 *   cc -std=c11 -O2 -Wall -Wextra -Werror -static \
 *     -o <out>/route_rustc tools/gatebox/route/route_rustc.c
 * and named by its absolute path in RUSTC_WORKSPACE_WRAPPER.
 *
 * Each invocation is logged to `<CARGO_TARGET_DIR>.route_rustc.log`, a sibling
 * of the target directory, as one JSON line written in one write(2):
 * {"added": BOOL, "crate": NAME-or-null, "source": [...]}, sorted keys, the
 * text Python's json.dumps(..., sort_keys=True) gives for the arguments
 * decoded as UTF-8 with surrogateescape. The target directory is inventoried
 * by A7, and the log's location is derived from a variable the shipped recipe
 * sets too. The log corroborates; it is not the authority (candidate build
 * scripts run beside it).
 */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

extern char **environ;

static const char *const UNITS[][2] = {
    {"nexus_governed_control", "crates/nexus-governed-control/src/lib.rs"},
    {"nexus_mother", "mother/src/lib.rs"},
    {"nexus_mother_host", "mother/host/src/lib.rs"},
    {"route_cells", "mother/host/tests/route_cells/main.rs"},
};

static void refuse(const char *why) {
    fprintf(stderr, "route_rustc: %s\n", why);
    exit(1);
}

struct buf {
    char *data;
    size_t len, cap;
};

static void put(struct buf *b, const char *s, size_t n) {
    if (b->len + n + 1 > b->cap) {
        size_t cap = (b->len + n + 1) * 2;
        char *data = realloc(b->data, cap);
        if (!data)
            refuse("out of memory");
        b->data = data;
        b->cap = cap;
    }
    memcpy(b->data + b->len, s, n);
    b->len += n;
}

static void puts_(struct buf *b, const char *s) { put(b, s, strlen(s)); }

/* `\uXXXX`, lowercase, for a code unit (at most 0xffff). */
static void put_u(struct buf *b, unsigned code) {
    static const char hex[] = "0123456789abcdef";
    char text[6] = {'\\', 'u', hex[code >> 12 & 15], hex[code >> 8 & 15], hex[code >> 4 & 15], hex[code & 15]};
    put(b, text, 6);
}

/* Length of the valid UTF-8 sequence at s (Python's strict decoder), or 0. */
static size_t utf8_len(const unsigned char *s, unsigned *code) {
    unsigned c = s[0];
    size_t n;
    unsigned lo = 0x80, hi = 0xbf;
    if (c >= 0xc2 && c <= 0xdf) {
        n = 2;
        *code = c & 0x1f;
    } else if (c >= 0xe0 && c <= 0xef) {
        n = 3;
        *code = c & 0x0f;
        if (c == 0xe0)
            lo = 0xa0;
        if (c == 0xed)
            hi = 0x9f;
    } else if (c >= 0xf0 && c <= 0xf4) {
        n = 4;
        *code = c & 0x07;
        if (c == 0xf0)
            lo = 0x90;
        if (c == 0xf4)
            hi = 0x8f;
    } else {
        return 0;
    }
    for (size_t i = 1; i < n; i++) {
        unsigned d = s[i];
        if (d < (i == 1 ? lo : 0x80) || d > (i == 1 ? hi : 0xbf))
            return 0;
        *code = (*code << 6) | (d & 0x3f);
    }
    return n;
}

/* A JSON string, as json.dumps(ensure_ascii=True) writes the decoded text. */
static void put_json(struct buf *b, const char *text) {
    const unsigned char *s = (const unsigned char *)text;
    puts_(b, "\"");
    while (*s) {
        unsigned c = *s, code;
        size_t n;
        if (c == '"' || c == '\\') {
            char esc[2] = {'\\', (char)c};
            put(b, esc, 2);
            s++;
        } else if (c == '\n' || c == '\r' || c == '\t' || c == '\b' || c == '\f') {
            puts_(b, c == '\n' ? "\\n" : c == '\r' ? "\\r" : c == '\t' ? "\\t" : c == '\b' ? "\\b" : "\\f");
            s++;
        } else if (c >= 0x20 && c < 0x7f) {
            put(b, (const char *)s, 1);
            s++;
        } else if (c < 0x80) {
            put_u(b, c);
            s++;
        } else if ((n = utf8_len(s, &code)) != 0) {
            if (code > 0xffff) {
                code -= 0x10000;
                put_u(b, 0xd800 | (code >> 10));
                put_u(b, 0xdc00 | (code & 0x3ff));
            } else {
                put_u(b, code);
            }
            s += n;
        } else {
            put_u(b, 0xdc00 | c); /* surrogateescape */
            s++;
        }
    }
    puts_(b, "\"");
}

static int ends_with_rs(const char *arg) {
    size_t n = strlen(arg);
    return n >= 3 && strcmp(arg + n - 3, ".rs") == 0;
}

int main(int argc, char **argv) {
    if (argc < 2)
        refuse("no compiler given");
    const char *target = getenv("CARGO_TARGET_DIR");
    if (!target || target[0] != '/')
        refuse("CARGO_TARGET_DIR must be an absolute path");
    char **args = argv + 2;
    int nargs = argc - 2;

    const char *name = NULL;
    int names = 0;
    for (int i = 0; i + 1 < nargs; i++)
        if (strcmp(args[i], "--crate-name") == 0) {
            names++;
            name = args[i + 1];
        }
    if (names != 1)
        name = NULL;
    int sources = 0;
    const char *source = NULL;
    for (int i = 0; i < nargs; i++)
        if (ends_with_rs(args[i]) && args[i][0] != '-') {
            sources++;
            source = args[i];
        }
    int added = 0;
    if (sources == 1 && name)
        for (size_t u = 0; u < sizeof UNITS / sizeof UNITS[0]; u++)
            if (strcmp(name, UNITS[u][0]) == 0 && strcmp(source, UNITS[u][1]) == 0)
                added = 1;

    struct buf line = {0};
    puts_(&line, added ? "{\"added\": true, \"crate\": " : "{\"added\": false, \"crate\": ");
    if (name)
        put_json(&line, name);
    else
        puts_(&line, "null");
    puts_(&line, ", \"source\": [");
    int first = 1;
    for (int i = 0; i < nargs; i++)
        if (ends_with_rs(args[i]) && args[i][0] != '-') {
            if (!first)
                puts_(&line, ", ");
            first = 0;
            put_json(&line, args[i]);
        }
    puts_(&line, "]}\n");

    struct buf path = {0};
    puts_(&path, target);
    puts_(&path, ".route_rustc.log");
    put(&path, "", 1);
    int fd = open(path.data, O_WRONLY | O_APPEND | O_CREAT | O_CLOEXEC, 0644);
    if (fd < 0) {
        perror("route_rustc: log");
        return 1;
    }
    if (write(fd, line.data, line.len) != (ssize_t)line.len) {
        perror("route_rustc: log write");
        return 1;
    }
    if (close(fd) != 0) {
        perror("route_rustc: log close");
        return 1;
    }

    char **out = calloc((size_t)argc + 3, sizeof *out);
    if (!out)
        refuse("out of memory");
    int k = 0;
    out[k++] = argv[1];
    for (int i = 0; i < nargs; i++)
        out[k++] = args[i];
    if (added) {
        out[k++] = "--cfg";
        out[k++] = "mother_route_seams";
    }
    out[k] = NULL;
    execve(argv[1], out, environ);
    perror("route_rustc: exec");
    return 1;
}
