#!/usr/bin/env python3
"""Phase Two cleanup observations for the supported-host workflow (P2-V1-R1).

After the live suite, `.github/workflows/ci-phase2-linux-sandbox.yml` runs
this script, with no arguments, to observe what the suite left behind. It
changes nothing: names it reports are identifiers only, and it never stops,
kills, creates, repairs or removes anything.

* Scopes: the `nexus-verifier-*.scope` units the systemd user manager of this
  process's real uid has loaded. The query is `systemctl --user` on
  `/run/user/<uid>/bus`, checked first as the sandbox checks it (reached from
  `/` without following a symlink; `/run` and `/run/user` root-owned and
  writable by no one else; `/run/user/<uid>` a private tmpfs directory of the
  uid; the bus a socket of the uid). Only `systemctl` itself is found through
  `PATH`: the query's whole environment is that bus address and fixed output
  settings, so an inherited `DBUS_SESSION_BUS_ADDRESS` or `XDG_RUNTIME_DIR`
  can never redirect it. It is bounded in time and output and runs in its own
  process group, which is killed, and the query reaped, at a bound.
* Workspaces: the entries of `/run/user/<uid>/nexus-verifier`, reached the
  same way and checked to be an owner-only directory of the uid on the
  runtime directory's filesystem before it is listed. It may be absent only
  as a genuinely missing final component of the checked runtime directory:
  the sandbox creates it on first use and never removes it.

An observation either answers or fails with its reason. A query that cannot
start, fails, times out, writes too much, reports diagnostics or answers
anything but a list of verifier scopes, and an inspection that is refused,
fails or meets a symlink, a wrong type, owner, mode or filesystem, is never
"nothing left". Both observations always run, neither failure suppressing
the other; the exit status is 0 only if both answered and found nothing.
"""

import contextlib
import ctypes
import os
import re
import selectors
import shutil
import signal
import stat
import subprocess
import sys
import time

QUERY_TIMEOUT = 10.0
QUERY_OUTPUT_LIMIT = 64 * 1024
REAP_TIMEOUT = 5.0
SCOPE_PATTERN = "nexus-verifier-*.scope"
SCOPE_QUERY_ARGS = (
    "--user",
    "--no-pager",
    "--legend=no",
    "--plain",
    "--full",
    "--all",
    "list-units",
    SCOPE_PATTERN,
)
WORKSPACES = "nexus-verifier"
TMPFS_MAGIC = 0x01021994
_DIRECTORY = os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC
_UNIT = re.compile(r"nexus-verifier-[A-Za-z0-9:_.\\@-]+\.scope")
_STATE = re.compile(r"[a-z][a-z-]*")
_PLAIN_PATH = re.compile(r"[A-Za-z0-9_/.-]+")


class ObservationError(Exception):
    """An observation without an answer; `kind` says which failure."""

    def __init__(self, kind, message):
        super().__init__(message)
        self.kind = kind


class Host:
    """Where the runtime directory is checked, and against what. The real
    host is `/`, root and a tmpfs; the fixture controls substitute their own.
    """

    def __init__(self, root="/", uid=None, root_owner=0, fs_magic=TMPFS_MAGIC):
        self.root = root
        self.uid = os.getuid() if uid is None else uid
        self.root_owner = root_owner
        self.fs_magic = fs_magic


def open_dir_at(parent, name):
    """The directory `name` beneath `parent`, never through a symlink."""
    return os.open(name, _DIRECTORY | os.O_NOFOLLOW, dir_fd=parent)


def filesystem_type(fd):
    """`statfs.f_type` of the object `fd` refers to (x86_64 Linux)."""
    libc = ctypes.CDLL(None, use_errno=True)
    buffer = ctypes.create_string_buffer(256)
    if libc.fstatfs(fd, buffer) != 0:
        error = ctypes.get_errno()
        raise OSError(error, os.strerror(error))
    return ctypes.c_long.from_buffer(buffer).value


@contextlib.contextmanager
def _owned(fd):
    try:
        yield fd
    finally:
        os.close(fd)


def _runtime(host, stack, open_at, fstat, fs_type):
    """The checked runtime directory `/run/user/<uid>` beneath `host.root`:
    its descriptor (closed with `stack`) and its stat."""
    runtime = f"/run/user/{host.uid}"
    path = "/"
    try:
        fd = stack.enter_context(_owned(os.open(host.root, _DIRECTORY)))
        for path, name in (("/run", "run"), ("/run/user", "user")):
            fd = stack.enter_context(_owned(open_at(fd, name)))
            st = fstat(fd)
            if st.st_uid != host.root_owner or st.st_mode & 0o022:
                raise ObservationError(
                    "runtime", f"{path} is not root-owned or is writable by others"
                )
        path = runtime
        fd = stack.enter_context(_owned(open_at(fd, str(host.uid))))
        st = fstat(fd)
        if (
            not stat.S_ISDIR(st.st_mode)
            or st.st_uid != host.uid
            or stat.S_IMODE(st.st_mode) != 0o700
        ):
            raise ObservationError(
                "runtime", f"{runtime} is not a private directory of uid {host.uid}"
            )
        if host.fs_magic is not None and fs_type(fd) != host.fs_magic:
            raise ObservationError("runtime", f"{runtime} is not a tmpfs")
        return fd, st
    except OSError as error:
        raise ObservationError("runtime", f"{path} cannot be inspected: {error}") from error


def user_bus(host, open_at=open_dir_at, fstat=os.fstat, fs_type=filesystem_type):
    """The checked user bus of `host.uid`: `/run/user/<uid>/bus` beneath
    `host.root`."""
    bus = f"/run/user/{host.uid}/bus"
    with contextlib.ExitStack() as stack:
        runtime, _ = _runtime(host, stack, open_at, fstat, fs_type)
        try:
            st = os.stat("bus", dir_fd=runtime, follow_symlinks=False)
        except OSError as error:
            raise ObservationError("runtime", f"{bus} cannot be inspected: {error}") from error
        if not stat.S_ISSOCK(st.st_mode) or st.st_uid != host.uid:
            raise ObservationError("runtime", f"{bus} is not a socket of uid {host.uid}")
    return os.path.join(host.root, "run", "user", str(host.uid), "bus")


def systemctl():
    """`systemctl` from this process's `PATH` (absolute entries only)."""
    entries = os.environ.get("PATH", "").split(os.pathsep)
    path = os.pathsep.join(entry for entry in entries if os.path.isabs(entry))
    found = shutil.which("systemctl", path=path) if path else None
    if not found:
        raise ObservationError("spawn", "no systemctl on PATH")
    return found


def scope_query(program, bus):
    """The argv and whole environment of the query of the loaded verifier
    scopes on the user bus at `bus`; `program` is the argv before the query's
    own arguments (a stand-in script's, in the fixture controls)."""
    if not _PLAIN_PATH.fullmatch(bus):
        raise ObservationError("runtime", f"{bus!r} is not a plain bus path")
    env = {
        "DBUS_SESSION_BUS_ADDRESS": f"unix:path={bus}",
        "LC_ALL": "C",
        "SYSTEMD_COLORS": "0",
        "SYSTEMD_URLIFY": "0",
    }
    return [*program, *SCOPE_QUERY_ARGS], env


class _Stop(Exception):
    def __init__(self, kind, message):
        super().__init__(message)
        self.kind = kind


def _collect(process, deadline, limit):
    """Both output streams until both close, within `deadline` and `limit`."""
    output = {process.stdout: bytearray(), process.stderr: bytearray()}
    written = 0
    with selectors.DefaultSelector() as selector:
        for pipe in output:
            os.set_blocking(pipe.fileno(), False)
            selector.register(pipe, selectors.EVENT_READ)
        while selector.get_map():
            left = deadline - time.monotonic()
            if left <= 0:
                raise _Stop("timeout", "did not finish in time")
            for key, _ in selector.select(left):
                try:
                    chunk = os.read(key.fileobj.fileno(), 65536)
                except BlockingIOError:
                    continue
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                written += len(chunk)
                if written > limit:
                    raise _Stop("output-limit", f"wrote more than {limit} bytes")
                output[key.fileobj] += chunk
    return bytes(output[process.stdout]), bytes(output[process.stderr])


def _end(process):
    """Kill the query's process group, then reap the query within a bound.
    The query leads that group and is not yet reaped, so the group is still
    its own."""
    with contextlib.suppress(ProcessLookupError):
        os.killpg(process.pid, signal.SIGKILL)
    try:
        return f"reaped, status {process.wait(timeout=REAP_TIMEOUT)}"
    except subprocess.TimeoutExpired:
        return "not reaped"


def run(argv, env, timeout=QUERY_TIMEOUT, limit=QUERY_OUTPUT_LIMIT):
    """Run the query `argv` with exactly `env`, to completion within its
    bounds: its status, stdout and stderr."""
    try:
        process = subprocess.Popen(
            argv,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
            close_fds=True,
            start_new_session=True,
        )
    except OSError as error:
        raise ObservationError("spawn", f"the query could not be started: {error}") from error
    deadline = time.monotonic() + timeout
    # A stopped query is ended by the kill below while its output is still
    # open, never by a pipe closed under it.
    try:
        stdout, stderr = _collect(process, deadline, limit)
        try:
            status = process.wait(timeout=max(deadline - time.monotonic(), 0))
        except subprocess.TimeoutExpired:
            raise _Stop("timeout", "did not finish in time") from None
        return status, stdout, stderr
    except _Stop as stop:
        reaped = _end(process)
        raise ObservationError(stop.kind, f"the query {stop} and was killed ({reaped})") from None
    except OSError as error:
        reaped = _end(process)
        raise ObservationError(
            "io", f"the query could not be read or awaited ({error}) and was killed ({reaped})"
        ) from error
    except BaseException:
        _end(process)
        raise
    finally:
        process.stdout.close()
        process.stderr.close()


def _excerpt(data):
    return data.decode("utf-8", "replace").strip()[:512]


def parse_scopes(stdout):
    """The scopes a successful query listed: one line per loaded verifier
    scope (`UNIT LOAD ACTIVE SUB DESCRIPTION`; blank lines aside), each once."""
    try:
        text = stdout.decode("utf-8")
    except UnicodeDecodeError:
        raise ObservationError("malformed", "the answer is not UTF-8") from None
    scopes = set()
    for line in text.split("\n"):
        line = line.removesuffix("\r")
        if not line.strip():
            continue
        fields = line.split()
        if (
            len(fields) < 4
            or not _UNIT.fullmatch(fields[0])
            or not all(_STATE.fullmatch(state) for state in fields[1:4])
        ):
            raise ObservationError("malformed", f"unexpected line {line!r}")
        if fields[0] in scopes:
            raise ObservationError("malformed", f"{fields[0]} is listed twice")
        scopes.add(fields[0])
    return sorted(scopes)


def observe(argv, env, timeout=QUERY_TIMEOUT, limit=QUERY_OUTPUT_LIMIT):
    """Run the query and read its answer: the loaded verifier scopes. Only a
    successful query that wrote nothing to its error output answers."""
    status, stdout, stderr = run(argv, env, timeout, limit)
    if status != 0:
        raise ObservationError("failed", f"the query failed (status {status}): {_excerpt(stderr)}")
    if stderr:
        raise ObservationError("diagnostics", f"the query reported: {_excerpt(stderr)}")
    return parse_scopes(stdout)


def observe_scopes():
    """The verifier scopes the user manager of this process's real uid has
    loaded."""
    bus = user_bus(Host())
    return observe(*scope_query([systemctl()], bus))


def observe_workspaces(
    host=None, open_at=open_dir_at, fstat=os.fstat, listdir=os.listdir, fs_type=filesystem_type
):
    """The entries of `/run/user/<uid>/nexus-verifier` beneath `host.root`,
    sorted; `None` if it genuinely does not exist."""
    host = host or Host()
    path = f"/run/user/{host.uid}/{WORKSPACES}"
    with contextlib.ExitStack() as stack:
        runtime, runtime_st = _runtime(host, stack, open_at, fstat, fs_type)
        try:
            fd = stack.enter_context(_owned(open_at(runtime, WORKSPACES)))
        except FileNotFoundError:
            # The permitted absence: nothing by that name in the checked
            # runtime directory, which is still there.
            try:
                linked = fstat(runtime).st_nlink > 0
            except OSError as error:
                raise ObservationError("inspection", f"{path} cannot be inspected: {error}") from error
            if not linked:
                raise ObservationError("inspection", f"/run/user/{host.uid} was removed") from None
            return None
        except OSError as error:
            raise ObservationError("inspection", f"{path} cannot be opened: {error}") from error
        try:
            st = fstat(fd)
            if (
                not stat.S_ISDIR(st.st_mode)
                or st.st_uid != host.uid
                or stat.S_IMODE(st.st_mode) != 0o700
                or st.st_dev != runtime_st.st_dev
            ):
                raise ObservationError(
                    "inspection", f"{path} is not an owner-only directory of uid {host.uid}"
                )
            return sorted(listdir(fd))
        except OSError as error:
            raise ObservationError("inspection", f"{path} cannot be listed: {error}") from error


def _line(error):
    """One annotation-safe line."""
    return " ".join(str(error).split()) or type(error).__name__


def check(scopes=observe_scopes, workspaces=observe_workspaces, out=print):
    """Both observations, reported; True only if both answered and found
    nothing. A failure of either never suppresses the other."""
    clean = True
    try:
        found = scopes()
    except Exception as error:  # any failure is a failed observation
        out(f"::error::the verifier scope observation failed: {_line(error)}")
        clean = False
    else:
        if found:
            out(f"::error::a verifier scope was left behind: {', '.join(found)}")
            clean = False
        else:
            out("verifier scopes loaded by the user manager: none")
    try:
        found = workspaces()
    except Exception as error:  # any failure is a failed observation
        out(f"::error::the verification workspace observation failed: {_line(error)}")
        clean = False
    else:
        if found:
            out(f"::error::a verification workspace was left behind: {', '.join(map(repr, found))}")
            clean = False
        elif found is None:
            out(f"verification workspaces: none ({WORKSPACES} was never created here)")
        else:
            out("verification workspaces: none")
    return clean


def main(argv):
    if argv:
        print("usage: phase2_cleanup_check.py (no arguments)", file=sys.stderr)
        return 2
    return 0 if check() else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
