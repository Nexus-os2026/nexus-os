#!/usr/bin/env python3
"""Phase Two cleanup observations for the supported-host workflow (P2-V1-R1, R2).

After the live suite, `.github/workflows/ci-phase2-linux-sandbox.yml` runs
this script, with no arguments, to observe what the suite left behind. It
changes nothing it observes: names it reports are identifiers only, and it
never stops, kills, creates, repairs or removes anything but its own query.

* Scopes: the `nexus-verifier-*.scope` units the systemd user manager of this
  process's real uid has loaded. The query is `systemctl --user` on
  `/run/user/<uid>/bus`, checked first as the sandbox checks it (reached from
  `/` without following a symlink; `/run` and `/run/user` root-owned and
  writable by no one else; `/run/user/<uid>` a private tmpfs directory of the
  uid; the bus a socket of the uid). Only `systemctl` itself is found through
  `PATH`: the query's whole environment is that bus address and fixed output
  settings, so an inherited `DBUS_SESSION_BUS_ADDRESS` or `XDG_RUNTIME_DIR`
  can never redirect it. It is bounded in time and output. Its processes
  stay owned until they are ended: the query leads its own process group,
  and until its leader is reaped the leader's pid anchors the group id.
  Whatever the query did, the group is ended with SIGKILL while the leader is
  unreaped and the output still open, and only then is the leader reaped; an
  answer comes only with its group ended, and a query whose group cannot be
  confirmed ended stays owned in its error, is finalized again explicitly
  and, still unconfirmed, released only once reported. This owns the query's
  processes; it confines nothing: a process that leaves the group is not
  reached.
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
import errno
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
EXPLICIT_ATTEMPTS = 3
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
    """An observation without an answer; `kind` says which failure. An
    `unfinalized` one still owns its query (`query`, an OwnedQuery), with what
    came first (`first`: the answer, or why it was stopped) and why its group
    is not confirmed ended (`failure`)."""

    def __init__(self, kind, message, query=None, first=None, failure=None):
        super().__init__(message)
        self.kind = kind
        self.query = query
        self.first = first
        self.failure = failure


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


class Ops:
    """The process operations a query's finalization uses: PROCESS, or a
    fixture's injected failures."""

    def __init__(self, exited, signal_group, reap):
        self.exited = exited
        self.signal_group = signal_group
        self.reap = reap

    def replace(self, **changes):
        values = {"exited": self.exited, "signal_group": self.signal_group, "reap": self.reap}
        values.update(changes)
        return Ops(**values)


def leader_exited(process):
    """The leader's status once it has exited, read without reaping it
    (WNOWAIT): it stays this process's unreaped child, still anchoring its
    group. A negative status is the signal that ended it, as Popen has it."""
    result = os.waitid(os.P_PID, process.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
    if result is None or result.si_pid == 0:
        return None
    if result.si_code == os.CLD_EXITED:
        return result.si_status
    if result.si_code in (os.CLD_KILLED, os.CLD_DUMPED):
        return -result.si_status
    raise OSError(errno.EINVAL, f"unexpected child state {result.si_code}")


def kill_group(group):
    """SIGKILL to every member of process group `group`."""
    os.killpg(group, signal.SIGKILL)


def reap(process, deadline):
    """Reap the leader, waiting until `deadline` at most; None if it was not
    reaped in time."""
    try:
        return process.wait(timeout=max(deadline - time.monotonic(), 0))
    except subprocess.TimeoutExpired:
        return None


PROCESS = Ops(leader_exited, kill_group, reap)


class OwnedQuery:
    """A started query's processes, owned through its leader: the child this
    observer spawned, which leads the query's process group. Until the leader
    is reaped, its pid anchors the group's id, so signalling the group reaches
    only the query's own members. Finalizing ends the group, then reaps the
    leader."""

    def __init__(self, process, ops):
        self.process = process
        self.group = process.pid
        # The unreaped leader still anchors the group id. Cleared once the
        # leader is reaped, or when reaping it failed and its state is
        # unknown: the group is never signalled again.
        self.anchored = True
        self.ops = ops

    def finalize(self):
        """End the query: SIGKILL to every member of its group while the
        unreaped leader anchors the group id, then reap the leader within the
        bound. Returns the leader's status; raises OSError, the query still
        owned, otherwise."""
        if not self.anchored:
            raise OSError(
                errno.ECHILD, "the leader's state is unknown: its group is never signalled again"
            )
        self.ops.signal_group(self.group)
        try:
            status = self.ops.reap(self.process, time.monotonic() + REAP_TIMEOUT)
        except BaseException:
            self.anchored = False
            raise
        if status is None:
            raise TimeoutError("the leader was not reaped in time")
        self.anchored = False
        return status

    def release(self):
        """Defense in depth for a query released without a confirmed
        finalization: while the unreaped leader still anchors the group id,
        SIGKILL the group and try once to reap the leader. Never a
        confirmation."""
        if self.anchored:
            with contextlib.suppress(OSError):
                self.ops.signal_group(self.group)
            with contextlib.suppress(Exception):
                if self.process.poll() is not None:
                    self.anchored = False


def _exit_by(query, deadline):
    """Wait, without reaping, until the leader exits or `deadline` passes."""
    while True:
        status = query.ops.exited(query.process)
        if status is not None:
            return status
        if time.monotonic() >= deadline:
            return None
        time.sleep(0.005)


def _attempt(query, deadline, limit):
    """The query's answer (its leader's status, stdout and stderr), its
    leader exited but not reaped; or why it was stopped (a _Stop)."""
    try:
        stdout, stderr = _collect(query.process, deadline, limit)
        status = _exit_by(query, deadline)
    except _Stop as stop:
        return stop
    except OSError as error:
        return _Stop("io", f"could not be read or awaited ({error})")
    if status is None:
        return _Stop("timeout", "did not finish in time")
    return status, stdout, stderr


def _came_first(first):
    if isinstance(first, _Stop):
        return f"the query {first}"
    return f"the query answered (leader status {first[0]})"


def run(argv, env, timeout=QUERY_TIMEOUT, limit=QUERY_OUTPUT_LIMIT, ops=None):
    """Run the query `argv` with exactly `env`, to completion within its
    bounds: its leader's status, stdout and stderr. Whatever happens, the
    query's process group is ended while its leader is still unreaped and its
    output still open, and only then is the leader reaped: an answer comes
    only with its group ended, and a query whose group cannot be confirmed
    ended stays owned in the error (kind `unfinalized`)."""
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
    query = OwnedQuery(process, ops or PROCESS)
    try:
        first = _attempt(query, time.monotonic() + timeout, limit)
        try:
            status = query.finalize()
        except OSError as failure:
            raise ObservationError(
                "unfinalized",
                f"{_came_first(first)}; its process group is not confirmed ended ({failure})",
                query=query,
                first=first,
                failure=failure,
            ) from failure
    except ObservationError:
        raise
    except BaseException:
        # Interrupted: what this query owns is still ended before it goes on.
        query.release()
        raise
    finally:
        process.stdout.close()
        process.stderr.close()
    if isinstance(first, _Stop):
        raise ObservationError(
            first.kind,
            f"{_came_first(first)}; its process group was ended (leader status {status})",
        )
    _, stdout, stderr = first
    return status, stdout, stderr


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


def observe(argv, env, timeout=QUERY_TIMEOUT, limit=QUERY_OUTPUT_LIMIT, ops=None):
    """Run the query and read its answer: the loaded verifier scopes. Only a
    successful query that wrote nothing to its error output answers."""
    status, stdout, stderr = run(argv, env, timeout, limit, ops)
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


def reported(error):
    """An observation failure, made reportable without losing what it owns:
    a query whose process group is not confirmed ended is finalized again
    explicitly (at most EXPLICIT_ATTEMPTS times) and, if still unconfirmed,
    released to its backstop only once that is described. The failure itself
    always remains one."""
    query = getattr(error, "query", None)
    if query is None:
        return _line(error)
    failures = [error.failure]
    for attempt in range(1, EXPLICIT_ATTEMPTS + 1):
        try:
            query.finalize()
        except OSError as failure:
            failures.append(failure)
        else:
            return _line(
                f"{_came_first(error.first)}; its process group was confirmed ended only on "
                f"explicit attempt {attempt} ({'; '.join(map(str, failures))})"
            )
    report = _line(
        f"{_came_first(error.first)}; its process group is still not confirmed ended after "
        f"{EXPLICIT_ATTEMPTS} explicit attempts ({'; '.join(map(str, failures))})"
    )
    query.release()
    return f"{report}; released to its backstop, never a confirmation"


def _reportable(error):
    try:
        return reported(error)
    except Exception as nested:  # describing must not hide the other observation
        return f"{_line(error)} (its query could not be finalized: {_line(nested)})"


def check(scopes=observe_scopes, workspaces=observe_workspaces, out=print):
    """Both observations, reported; True only if both answered and found
    nothing. A failure of either never suppresses the other, and a query
    still owned by a failure is finalized explicitly before it is reported."""
    clean = True
    try:
        found = scopes()
    except Exception as error:  # any failure is a failed observation
        out(f"::error::the verifier scope observation failed: {_reportable(error)}")
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
        out(f"::error::the verification workspace observation failed: {_reportable(error)}")
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
