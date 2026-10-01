#!/usr/bin/env python3
"""P2-V1-R1/R2 fixture controls for `phase2_cleanup_check.py` and for the
live step of `.github/workflows/ci-phase2-linux-sandbox.yml` that runs it.

These are not live evidence. No user manager is contacted and nothing is
created outside this test's temporary directories: stand-in queries are
`/bin/sh` scripts; runtime directories are fixture trees in which this test's
uid stands in for root; failures this host's permissions cannot produce
deterministically (a refused open or listing, a failing or foreign stat, a
refused signal or reap) are injected. A stand-in that leaves a process behind
reports it over a FIFO and waits until this test holds it by pidfd: the
test's own cleanup ends it, and reaps what is this process's child, whatever
the observer did. The live step runs as GitHub runs a bash step, with
stand-ins for the live suite, the capture of its output and the
observations.

    python3 scripts/ci/test_phase2_cleanup_check.py
"""

import contextlib
import errno
import os
import select
import shlex
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from unittest import mock

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import phase2_cleanup_check as check  # noqa: E402

WORKFLOW = os.path.join(HERE, "..", "..", ".github", "workflows", "ci-phase2-linux-sandbox.yml")
LIVE_STEP = "      - name: Live Phase Two isolation, escape and cleanup suite (every layer required)"
PASSED = "test result: ok. 31 live sandbox cases passed"
# A bus address no stand-in ever connects to.
BUS = "/nonexistent/nexus-p2v1r1/run/user/4242/bus"
KILLED = f"status {-signal.SIGKILL}"


def stand_in(script):
    """The scope query, answered by `script` (run by /bin/sh with the
    query's own arguments after it) instead of systemctl."""
    return check.scope_query(["/bin/sh", "-c", script, "systemctl"], BUS)


def started_with(env):
    """The environment a stand-in started with, from its own
    /proc/<pid>/environ."""
    status, stdout, _ = check.run(["/bin/sh", "-c", "/bin/cat /proc/$$/environ"], env)
    assert status == 0, status
    return dict(entry.split("=", 1) for entry in stdout.decode().split("\0") if entry)


def expected_environment(bus):
    return {
        "DBUS_SESSION_BUS_ADDRESS": f"unix:path={bus}",
        "LC_ALL": "C",
        "SYSTEMD_COLORS": "0",
        "SYSTEMD_URLIFY": "0",
    }


def temporary(test, prefix="p2v1r1-"):
    directory = tempfile.mkdtemp(prefix=prefix)
    test.addCleanup(shutil.rmtree, directory, True)
    os.chmod(directory, 0o755)
    return directory


class Held:
    """A process this test holds by pidfd: its own handle, which the
    observer under test never shares, so the test's cleanup never depends on
    that observer."""

    def __init__(self, pid):
        self.pid = pid
        self.fd = os.pidfd_open(pid)

    def exited_within(self, within):
        """Whether the process has exited within `within` seconds: its pidfd
        is readable once it has. An exit, never a reap."""
        poller = select.poll()
        poller.register(self.fd, select.POLLIN)
        return bool(poller.poll(int(within * 1000)))

    def unreaped_child(self):
        """Whether it is still this process's unreaped child: it was never
        reaped, by anyone."""
        try:
            os.waitid(os.P_PIDFD, self.fd, os.WEXITED | os.WNOHANG | os.WNOWAIT)
        except ChildProcessError:
            return False
        return True

    def kill(self):
        """SIGKILL to exactly this process, through its pidfd."""
        with contextlib.suppress(ProcessLookupError):
            signal.pidfd_send_signal(self.fd, signal.SIGKILL)

    def reap(self):
        """Reap it, if it is still this process's child."""
        with contextlib.suppress(ChildProcessError):
            os.waitid(os.P_PIDFD, self.fd, os.WEXITED)

    def close(self):
        os.close(self.fd)


class Survivor:
    """A stand-in query whose leader starts one descendant in its own
    process group (every standard stream on /dev/null unless `keep_output`,
    so it never holds the query's output), reports both pids over a FIFO,
    waits until this test holds both by pidfd, then exits with `status`
    without writing anything. Whatever the observer does, this test's
    cleanup ends what is left: the descendant through its pidfd, and the
    leader, this process's child, reaped."""

    def __init__(self, test, status=0, keep_output=False):
        directory = temporary(test)
        self.pids = os.path.join(directory, "pids")
        self.go = os.path.join(directory, "go")
        for fifo in (self.pids, self.go):
            os.mkfifo(fifo, 0o600)
        redirect = "" if keep_output else " </dev/null >/dev/null 2>&1"
        self.script = (
            f"/bin/sleep 600{redirect} &\n"
            f'echo "$$ $!" > {self.pids}\n'
            f"read go < {self.go}\n"
            f"exit {status}\n"
        )
        self.held = None
        test.addCleanup(self.cleanup)

    def observe(self, **bounds):
        """The stand-in query, observed while this test takes both of its
        processes by pidfd: a handshake over the FIFOs, never a sleep."""
        failed = []

        def handshake():
            try:
                with open(self.pids) as fifo:
                    leader, descendant = (int(pid) for pid in fifo.read().split())
                # Both are alive here: the leader waits for `go`, the
                # descendant sleeps.
                self.held = (Held(leader), Held(descendant))
                with open(self.go, "w") as fifo:
                    fifo.write("go\n")
            except BaseException as error:  # reported by the test below
                failed.append(error)

        thread = threading.Thread(target=handshake, daemon=True)
        thread.start()
        try:
            return check.observe(*stand_in(self.script), **bounds)
        finally:
            self.unblock()
            thread.join(timeout=30)
            if thread.is_alive() or failed:
                raise AssertionError(f"the handshake failed: {failed!r}")

    def unblock(self):
        """Release a handshake still waiting on a FIFO the stand-in never
        opened."""
        for path, flags in ((self.pids, os.O_WRONLY), (self.go, os.O_RDONLY)):
            with contextlib.suppress(OSError):
                os.close(os.open(path, flags | os.O_NONBLOCK))

    def cleanup(self):
        self.unblock()
        if self.held:
            leader, descendant = self.held
            if not descendant.exited_within(0):
                descendant.kill()
                descendant.exited_within(5)
            if leader.unreaped_child():
                leader.kill()
                leader.reap()
            leader.close()
            descendant.close()


class Fixture:
    """`run/user/<uid>` as a host has it, in a fixture root this test owns:
    `run` and `run/user` 0755, the runtime directory 0700 holding a listening
    bus socket."""

    def __init__(self, test, uid=None):
        self.root = temporary(test)
        self.uid = os.getuid() if uid is None else uid
        self.run = os.path.join(self.root, "run")
        self.user = os.path.join(self.run, "user")
        self.runtime = os.path.join(self.user, str(self.uid))
        os.makedirs(self.runtime)
        for path, mode in ((self.run, 0o755), (self.user, 0o755), (self.runtime, 0o700)):
            os.chmod(path, mode)
        self.bus = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        test.addCleanup(self.bus.close)
        self.bus.bind(os.path.join(self.runtime, "bus"))
        self.bus.listen()
        self.workspaces = os.path.join(self.runtime, check.WORKSPACES)

    def host(self, **changes):
        """This root as a host whose root is this test's uid."""
        values = {"root": self.root, "uid": self.uid, "root_owner": os.getuid(), "fs_magic": None}
        values.update(changes)
        return check.Host(**values)

    def private_workspaces(self, *entries):
        os.mkdir(self.workspaces)
        os.chmod(self.workspaces, 0o700)
        for entry in entries:
            os.mkdir(os.path.join(self.workspaces, entry))


def nth_fstat(n, change):
    """`os.fstat`, except that its `n`th call (1-based) answers `change` of
    the real answer, or raises it."""
    calls = []

    def fstat(fd):
        calls.append(fd)
        st = os.fstat(fd)
        if len(calls) != n:
            return st
        if isinstance(change, BaseException):
            raise change
        return os.stat_result(change(list(st[:10])))

    return fstat


def changed(index, value):
    def change(fields):
        fields[index] = value
        return fields

    return change


class ScopeObservation(unittest.TestCase):
    def observe(self, script, **bounds):
        return check.observe(*stand_in(script), **bounds)

    def fails(self, kind, script, **bounds):
        with self.assertRaises(check.ObservationError) as raised:
            self.observe(script, **bounds)
        self.assertEqual(raised.exception.kind, kind, raised.exception)
        return raised.exception

    def test_a_successful_empty_answer_is_no_scopes(self):
        self.assertEqual(self.observe("exit 0"), [])

    def test_a_successful_answer_lists_each_loaded_verifier_scope(self):
        answer = (
            "nexus-verifier-0a1b.scope loaded active running Nexus verifier execution\\n\\n"
            "nexus-verifier-ff00.scope loaded inactive dead Nexus verifier execution\\n"
        )
        self.assertEqual(
            self.observe(f"printf '{answer}'"),
            ["nexus-verifier-0a1b.scope", "nexus-verifier-ff00.scope"],
        )

    def test_a_failed_query_with_empty_output_is_an_error_never_no_scopes(self):
        # A silent failure: nothing on either stream.
        self.fails("failed", "exit 4")
        # What systemctl --user does without a reachable user bus.
        error = self.fails("failed", "echo 'Failed to connect to bus: No medium found' >&2; exit 1")
        self.assertIn("No medium found", str(error))

    def test_a_successful_query_that_reports_diagnostics_is_an_error(self):
        error = self.fails("diagnostics", "echo 'warning: something is off' >&2; exit 0")
        self.assertIn("something is off", str(error))

    def test_a_missing_query_program_is_an_error(self):
        argv, env = check.scope_query(["/nonexistent/nexus-p2v1r1/systemctl"], BUS)
        with self.assertRaises(check.ObservationError) as raised:
            check.observe(argv, env)
        self.assertEqual(raised.exception.kind, "spawn")
        for path in ("", "relative/bin", "/nonexistent/nexus-p2v1r1"):
            with self.subTest(path=path), mock.patch.dict(os.environ, {"PATH": path}):
                with self.assertRaises(check.ObservationError) as raised:
                    check.systemctl()
                self.assertEqual(raised.exception.kind, "spawn")

    def test_a_query_that_does_not_finish_is_ended_reaped_and_an_error(self):
        started = time.monotonic()
        error = self.fails("timeout", "exec /bin/sleep 30", timeout=0.3)
        self.assertIn(KILLED, str(error))
        self.assertLess(time.monotonic() - started, 8)

    def test_a_query_whose_output_stays_open_is_ended_with_its_process_group(self):
        # The leader exits, but the descendant it left keeps its output
        # open: no answer is complete, and the bound still holds.
        survivor = Survivor(self, 0, keep_output=True)
        with self.assertRaises(check.ObservationError) as raised:
            survivor.observe(timeout=3)
        self.assertEqual(raised.exception.kind, "timeout")
        self.assertIn("leader status 0", str(raised.exception))
        leader, descendant = survivor.held
        self.assertTrue(descendant.exited_within(5), "the query's process group was ended")
        self.assertFalse(leader.unreaped_child(), "the leader was reaped")

    def test_a_query_that_writes_too_much_is_ended_and_an_error(self):
        line = "nexus-verifier-0a1b.scope loaded active running Nexus verifier execution"
        for stream in ("", " >&2"):
            with self.subTest(stream=stream):
                error = self.fails(
                    "output-limit",
                    f"i=0; while [ $i -lt 64 ]; do echo '{line}'{stream}; i=$((i+1)); done; "
                    "exec /bin/sleep 30",
                    limit=1024,
                )
                self.assertIn(KILLED, str(error))

    def test_a_malformed_answer_is_an_error(self):
        for answer in (
            "garbage\\n",
            "\\342\\227\\217 nexus-verifier-0a1b.scope loaded failed failed Nexus\\n",
            "nexus-verifier-0a1b.scope\\n",
            "nexus-verifier-0a1b.scope loaded active\\n",
            "nexus-verifier-0a1b.scope Loaded active running Nexus\\n",
            "nexus-verifier-0a1b.scope - active running Nexus\\n",
            "other.scope loaded active running Other\\n",
            "nexus-verifier-0a1b.service loaded active running Nexus\\n",
            "nexus-verifier-.scope loaded active running Nexus\\n",
            "nexus-verifier-0a1b.scope loaded active running A\\n"
            "nexus-verifier-0a1b.scope loaded active running A\\n",
            "\\377\\n",
        ):
            with self.subTest(answer=answer):
                self.fails("malformed", f"printf '{answer}'")

    def test_the_query_is_exactly_the_scope_listing_on_the_given_bus(self):
        status, stdout, stderr = check.run(*stand_in('printf "%s\\n" "$@"'))
        self.assertEqual((status, stderr), (0, b""))
        self.assertEqual(
            stdout.decode().splitlines(),
            [
                "--user",
                "--no-pager",
                "--legend=no",
                "--plain",
                "--full",
                "--all",
                "list-units",
                "nexus-verifier-*.scope",
            ],
        )
        self.assertEqual(started_with(stand_in("")[1]), expected_environment(BUS))
        # A bus address is never built from a path that needs escaping.
        for bus in ("/run/user/1000/b us", "/run/user/1000/bus;x", "/run/user/1000/bus,guid=0"):
            with self.subTest(bus=bus), self.assertRaises(check.ObservationError):
                check.scope_query(["/bin/sh"], bus)

    def test_poisoned_or_absent_ambient_bus_and_runtime_never_reach_the_query(self):
        poisoned = {
            "DBUS_SESSION_BUS_ADDRESS": "unix:path=/nonexistent/poisoned/bus",
            "XDG_RUNTIME_DIR": "/nonexistent/poisoned",
        }
        with mock.patch.dict(os.environ, poisoned):
            self.assertEqual(started_with(stand_in("")[1]), expected_environment(BUS))
        with mock.patch.dict(os.environ):
            for key in poisoned:
                os.environ.pop(key, None)
            self.assertEqual(started_with(stand_in("")[1]), expected_environment(BUS))


def refused(*_):
    raise PermissionError(errno.EPERM, "Operation not permitted")


class Counted:
    """A process operation that counts its calls before it runs `then`."""

    def __init__(self, then):
        self.calls = 0
        self.then = then

    def __call__(self, *args):
        self.calls += 1
        return self.then(*args)


class QueryOwnership(unittest.TestCase):
    """A query's process group is ended before any answer; one that cannot
    be confirmed ended stays owned, for an explicit retry."""

    def unfinalized(self, survivor, **ops):
        with self.assertRaises(check.ObservationError) as raised:
            survivor.observe(ops=check.PROCESS.replace(**ops))
        self.assertEqual(raised.exception.kind, "unfinalized", raised.exception)
        return raised.exception

    def test_a_descendant_left_by_a_successful_query_is_ended_before_the_answer(self):
        # The leader exits 0 without output; its descendant, in its group,
        # holds none of the query's output. Neither the end of the output nor
        # the leader's exit ends the query.
        survivor = Survivor(self, 0)
        self.assertEqual(survivor.observe(), [])
        leader, descendant = survivor.held
        self.assertTrue(descendant.exited_within(5), "the query's process group was ended")
        self.assertFalse(leader.unreaped_child(), "the leader was reaped")

    def test_a_descendant_left_by_a_failed_query_is_ended_with_it(self):
        survivor = Survivor(self, 3)
        with self.assertRaises(check.ObservationError) as raised:
            survivor.observe()
        self.assertEqual(raised.exception.kind, "failed")
        self.assertIn("status 3", str(raised.exception))
        leader, descendant = survivor.held
        self.assertTrue(descendant.exited_within(5))
        self.assertFalse(leader.unreaped_child())

    def test_an_unreadable_query_is_ended_and_an_error(self):
        survivor = Survivor(self, 0)

        def unreadable(_):
            raise OSError(errno.EIO, "Input/output error")

        with self.assertRaises(check.ObservationError) as raised:
            survivor.observe(ops=check.PROCESS.replace(exited=unreadable))
        self.assertEqual(raised.exception.kind, "io")
        self.assertIn("Input/output error", str(raised.exception))
        leader, descendant = survivor.held
        self.assertTrue(descendant.exited_within(5))
        self.assertFalse(leader.unreaped_child())

    def test_an_unconfirmed_group_end_keeps_the_query_owned_for_an_explicit_retry(self):
        survivor = Survivor(self, 0)
        error = self.unfinalized(survivor, signal_group=refused)
        self.assertEqual(error.first[0], 0, "the answer came first")
        self.assertIsInstance(error.failure, PermissionError)
        leader, descendant = survivor.held
        self.assertEqual(error.query.process.pid, leader.pid)
        self.assertTrue(leader.unreaped_child(), "still owned: the leader unreaped")
        self.assertFalse(descendant.exited_within(0), "nothing ended the group")
        # A later explicit retry, without the injected failure, ends it.
        error.query.ops = check.PROCESS
        self.assertEqual(error.query.finalize(), 0)
        self.assertTrue(descendant.exited_within(5))
        self.assertFalse(leader.unreaped_child())

    def test_an_unreaped_leader_keeps_the_query_owned_and_anchored(self):
        survivor = Survivor(self, 0)
        error = self.unfinalized(survivor, reap=lambda process, deadline: None)
        self.assertIsInstance(error.failure, TimeoutError)
        leader, descendant = survivor.held
        self.assertTrue(descendant.exited_within(5), "the group was signalled, anchored")
        self.assertTrue(leader.unreaped_child(), "the anchor is kept")
        error.query.ops = check.PROCESS
        self.assertEqual(error.query.finalize(), 0)
        self.assertFalse(leader.unreaped_child())

    def test_a_leader_in_an_unknown_state_is_never_signalled_again(self):
        survivor = Survivor(self, 0)
        signalled = Counted(check.kill_group)

        def unknown(process, deadline):
            raise ChildProcessError(errno.ECHILD, "No child processes")

        error = self.unfinalized(survivor, signal_group=signalled, reap=unknown)
        self.assertEqual(signalled.calls, 1, "signalled once, anchored")
        leader, descendant = survivor.held
        self.assertTrue(descendant.exited_within(5))
        # Its reap failed, so the leader's state is unknown: neither a retry
        # nor the release signals that group id again.
        with self.assertRaises(OSError) as raised:
            error.query.finalize()
        self.assertIn("never signalled again", str(raised.exception))
        error.query.release()
        self.assertEqual(signalled.calls, 1)
        # The leader is in fact still this process's child: reaped here,
        # explicitly.
        self.assertTrue(leader.unreaped_child())
        self.assertEqual(error.query.process.wait(timeout=5), 0)
        self.assertFalse(leader.unreaped_child())

    def test_a_reported_unconfirmed_query_is_retried_then_released(self):
        survivor = Survivor(self, 0)
        refusing = Counted(refused)
        error = self.unfinalized(survivor, signal_group=refusing)
        report = check.reported(error)
        self.assertIn("still not confirmed ended after 3 explicit attempts", report)
        self.assertIn("released to its backstop, never a confirmation", report)
        # The run's attempt, each explicit attempt, then the release's
        # backstop, which still held the group's anchor.
        self.assertEqual(refusing.calls, 1 + check.EXPLICIT_ATTEMPTS + 1)
        leader, descendant = survivor.held
        self.assertFalse(leader.unreaped_child(), "the release reaped the leader")
        self.assertFalse(descendant.exited_within(0), "nothing could end the group")

    def test_a_reported_query_is_finalized_again_explicitly(self):
        survivor = Survivor(self, 0)
        error = self.unfinalized(survivor, signal_group=refused)
        error.query.ops = check.PROCESS
        report = check.reported(error)
        self.assertIn("confirmed ended only on explicit attempt 1", report)
        leader, descendant = survivor.held
        self.assertTrue(descendant.exited_within(5))
        self.assertFalse(leader.unreaped_child())


def symlinked(path, real):
    os.rename(path, real)
    os.symlink(real, path)


def bound_socket(path):
    with socket.socket(socket.AF_UNIX) as sock:
        sock.bind(path)


class UserBus(unittest.TestCase):
    def test_the_bus_is_taken_only_from_a_checked_private_runtime_directory(self):
        fixture = Fixture(self)
        self.assertEqual(check.user_bus(fixture.host()), os.path.join(fixture.runtime, "bus"))
        fd = os.open(fixture.runtime, os.O_RDONLY | os.O_DIRECTORY)
        try:
            magic = check.filesystem_type(fd)
        finally:
            os.close(fd)
        self.assertEqual(
            check.user_bus(fixture.host(fs_magic=magic)), os.path.join(fixture.runtime, "bus")
        )

    def test_an_unusable_runtime_directory_or_bus_is_an_error(self):
        bus = lambda f: os.path.join(f.runtime, "bus")  # noqa: E731
        changes = {
            "no runtime directory": lambda f: shutil.rmtree(f.runtime),
            "a symlinked runtime directory": lambda f: symlinked(
                f.runtime, os.path.join(f.root, "elsewhere")
            ),
            "a shared runtime directory": lambda f: os.chmod(f.runtime, 0o755),
            "no bus": lambda f: os.remove(bus(f)),
            "a bus that is a file": lambda f: (os.remove(bus(f)), open(bus(f), "w").close()),
            "a symlinked bus": lambda f: symlinked(bus(f), os.path.join(f.root, "real-bus")),
            "a writable /run": lambda f: os.chmod(f.run, 0o775),
            "a writable /run/user": lambda f: os.chmod(f.user, 0o777),
            "a symlinked /run": lambda f: symlinked(f.run, os.path.join(f.root, "real-run")),
            "a symlinked /run/user": lambda f: symlinked(f.user, os.path.join(f.root, "real-user")),
        }
        for what, change in changes.items():
            with self.subTest(what):
                fixture = Fixture(self)
                change(fixture)
                with self.assertRaises(check.ObservationError) as raised:
                    check.user_bus(fixture.host())
                self.assertEqual(raised.exception.kind, "runtime")
        # Another uid's runtime directory (owned here by this test), a /run
        # not owned by root's stand-in, and the wrong filesystem.
        other = Fixture(self, uid=os.getuid() + 1)
        fixture = Fixture(self)
        for host in (
            other.host(),
            fixture.host(root_owner=os.getuid() + 1),
            fixture.host(fs_magic=0x1234),
        ):
            with self.subTest(uid=host.uid, root=host.root_owner, magic=host.fs_magic):
                with self.assertRaises(check.ObservationError) as raised:
                    check.user_bus(host)
                self.assertEqual(raised.exception.kind, "runtime")


class WorkspaceObservation(unittest.TestCase):
    def fails(self, kind, fixture, **injected):
        with self.assertRaises(check.ObservationError) as raised:
            check.observe_workspaces(fixture.host(), **injected)
        self.assertEqual(raised.exception.kind, kind, raised.exception)
        return raised.exception

    def test_an_empty_workspaces_directory_is_nothing_left(self):
        fixture = Fixture(self)
        fixture.private_workspaces()
        self.assertEqual(check.observe_workspaces(fixture.host()), [])

    def test_whatever_the_workspaces_directory_holds_is_left_behind(self):
        fixture = Fixture(self)
        fixture.private_workspaces("ws-0a1b")
        with open(os.path.join(fixture.workspaces, ".hidden"), "w"):
            pass
        self.assertEqual(check.observe_workspaces(fixture.host()), [".hidden", "ws-0a1b"])

    def test_absence_is_only_a_missing_final_component_of_a_checked_runtime_directory(self):
        fixture = Fixture(self)
        self.assertIsNone(check.observe_workspaces(fixture.host()))
        # A missing, symlinked or unchecked runtime directory is not absence.
        shutil.rmtree(fixture.runtime)
        self.fails("runtime", fixture)
        fixture = Fixture(self)
        symlinked(fixture.runtime, os.path.join(fixture.root, "elsewhere"))
        self.fails("runtime", fixture)
        fixture = Fixture(self)
        os.chmod(fixture.run, 0o777)
        self.fails("runtime", fixture)
        # Nor is a runtime directory removed while it is inspected.
        fixture = Fixture(self)
        self.fails("inspection", fixture, fstat=nth_fstat(4, changed(3, 0)))

    def test_a_symlink_or_another_type_is_never_empty(self):
        def symlink_to_private(f):
            real = os.path.join(f.runtime, "real")
            os.mkdir(real)
            os.chmod(real, 0o700)
            os.symlink(real, f.workspaces)

        changes = {
            "a symlink to a private directory": symlink_to_private,
            "a dangling symlink": lambda f: os.symlink("/nonexistent/nexus-p2v1r1", f.workspaces),
            "a file": lambda f: open(f.workspaces, "w").close(),
            "a FIFO": lambda f: os.mkfifo(f.workspaces, 0o600),
            "a socket": lambda f: bound_socket(f.workspaces),
        }
        for what, change in changes.items():
            with self.subTest(what):
                fixture = Fixture(self)
                change(fixture)
                self.fails("inspection", fixture)

    def test_a_wrong_owner_mode_or_filesystem_is_never_empty(self):
        fixture = Fixture(self)
        fixture.private_workspaces()
        os.chmod(fixture.workspaces, 0o755)
        self.fails("inspection", fixture)
        # Owners and filesystems this test cannot create are injected into
        # the workspaces directory's own stat (the fourth).
        for what, change in (
            ("another owner", changed(4, os.getuid() + 1)),
            ("another filesystem", lambda fields: changed(2, fields[2] + 1)(fields)),
        ):
            with self.subTest(what):
                fixture = Fixture(self)
                fixture.private_workspaces()
                self.fails("inspection", fixture, fstat=nth_fstat(4, change))
        other = Fixture(self, uid=os.getuid() + 1)
        other.private_workspaces()
        with self.assertRaises(check.ObservationError) as raised:
            check.observe_workspaces(other.host())
        self.assertEqual(raised.exception.kind, "runtime")
        fixture = Fixture(self)
        fixture.private_workspaces()
        with self.assertRaises(check.ObservationError) as raised:
            check.observe_workspaces(fixture.host(fs_magic=0x1234))
        self.assertEqual(raised.exception.kind, "runtime")

    def test_a_refused_or_failing_inspection_is_never_empty(self):
        refused = PermissionError(errno.EACCES, "Permission denied")
        failing = OSError(errno.EIO, "Input/output error")

        def open_refused(name):
            def open_at(parent, entry):
                if entry == name:
                    raise refused
                return check.open_dir_at(parent, entry)

            return open_at

        def listdir_raising(error):
            def listdir(fd):
                raise error

            return listdir

        cases = (
            ("an open refused", "inspection", {"open_at": open_refused(check.WORKSPACES)}),
            ("a listing refused", "inspection", {"listdir": listdir_raising(refused)}),
            ("a listing failing", "inspection", {"listdir": listdir_raising(failing)}),
            ("a stat failing", "inspection", {"fstat": nth_fstat(4, failing)}),
            ("/run/user refused", "runtime", {"open_at": open_refused("user")}),
            ("a runtime stat failing", "runtime", {"fstat": nth_fstat(3, failing)}),
        )
        for what, kind, injected in cases:
            with self.subTest(what):
                fixture = Fixture(self)
                fixture.private_workspaces()
                self.fails(kind, fixture, **injected)


class BothObservations(unittest.TestCase):
    def outcome(self, scopes, workspaces):
        calls, lines = [], []

        def observed(name, result):
            def observe():
                calls.append(name)
                if isinstance(result, BaseException):
                    raise result
                return result

            return observe

        clean = check.check(observed("scopes", scopes), observed("workspaces", workspaces), lines.append)
        self.assertEqual(calls, ["scopes", "workspaces"], "both observations always run")
        return clean, "\n".join(lines)

    def test_only_two_answers_of_nothing_left_are_clean(self):
        failure = check.ObservationError("failed", "the query failed")
        self.assertTrue(self.outcome([], [])[0])
        self.assertTrue(self.outcome([], None)[0])
        for scopes, workspaces, expected in (
            (failure, [], "the verifier scope observation failed: the query failed"),
            ([], failure, "the verification workspace observation failed: the query failed"),
            (["nexus-verifier-0a1b.scope"], [], "a verifier scope was left behind"),
            ([], ["ws-0a1b"], "a verification workspace was left behind"),
            (RuntimeError("bug"), [], "the verifier scope observation failed: bug"),
        ):
            with self.subTest(scopes=scopes, workspaces=workspaces):
                clean, report = self.outcome(scopes, workspaces)
                self.assertFalse(clean)
                self.assertIn(f"::error::{expected}", report)
        # Neither failure hides the other.
        clean, report = self.outcome(failure, OSError(errno.EIO, "gone"))
        self.assertFalse(clean)
        self.assertEqual(report.count("::error::"), 2, report)

    def test_a_query_still_owned_by_a_failure_is_finalized_before_it_is_reported(self):
        class Owned:
            """A query's ownership standing in: it ends after `failures`
            refused attempts; every finalization and release is counted."""

            def __init__(self, failures):
                self.failures, self.finalized, self.released = failures, 0, 0

            def finalize(self):
                self.finalized += 1
                if self.failures:
                    self.failures -= 1
                    raise PermissionError(errno.EPERM, "refused")
                return 0

            def release(self):
                self.released += 1

        def unfinalized(owned):
            return check.ObservationError(
                "unfinalized",
                "the query answered; its process group is not confirmed ended",
                query=owned,
                first=(0, b"", b""),
                failure=PermissionError(errno.EPERM, "refused"),
            )

        recovered = Owned(1)
        clean, report = self.outcome(unfinalized(recovered), [])
        self.assertFalse(clean, "a confirmation later is still a failed observation")
        self.assertIn("confirmed ended only on explicit attempt 2", report)
        self.assertEqual((recovered.finalized, recovered.released), (2, 0))
        stuck = Owned(10)
        clean, report = self.outcome(unfinalized(stuck), [])
        self.assertFalse(clean)
        self.assertIn("still not confirmed ended after 3 explicit attempts", report)
        self.assertEqual((stuck.finalized, stuck.released), (3, 1))

    def test_the_command_takes_no_arguments(self):
        self.assertEqual(check.main(["--root", "/tmp"]), 2)


class WorkflowLiveStep(unittest.TestCase):
    """The workflow's own live-step script, run as GitHub runs a bash step,
    with stand-ins for the live suite and for the observations."""

    def script(self):
        with open(WORKFLOW) as workflow:
            lines = workflow.read().split("\n")
        start = lines.index(LIVE_STEP)
        run = lines.index("        run: |", start)
        body = []
        for line in lines[run + 1 :]:
            if line.strip() and not line.startswith(" " * 10):
                break
            body.append(line[10:])
        return "\n".join(body).rstrip() + "\n"

    def step(self, live_status, live_output, observed_status, tee_status=0):
        work = temporary(self, "p2v1r1-step-")
        stand_ins = os.path.join(work, "bin")
        os.makedirs(stand_ins)
        os.makedirs(os.path.join(work, "scripts", "ci"))
        marker = os.path.join(work, "observed")
        tee = shutil.which("tee")
        self.assertTrue(tee and os.path.isabs(tee), "a tee to stand behind")
        stand_in_scripts = {
            "cargo": f"printf '%s\\n' {shlex.quote(live_output)}\nexit {live_status}\n",
            # The capture: the real tee, then the given status (no disk is
            # ever exhausted).
            "tee": f'{shlex.quote(tee)} "$@"\nexit {tee_status}\n',
            # Never a real user manager, whatever the step runs.
            "systemctl": f"echo systemctl >> {shlex.quote(marker)}\nexit 97\n",
        }
        for name, body in stand_in_scripts.items():
            with open(os.path.join(stand_ins, name), "w") as script:
                script.write(f"#!/bin/sh\n{body}")
            os.chmod(os.path.join(stand_ins, name), 0o755)
        with open(os.path.join(work, "scripts", "ci", "phase2_cleanup_check.py"), "w") as script:
            script.write(
                f"import sys\nopen({marker!r}, 'a').write('observed\\n')\nsys.exit({observed_status})\n"
            )
        step = os.path.join(work, "step.sh")
        with open(step, "w") as script:
            script.write(self.script())
        env = {"PATH": stand_ins + os.pathsep + os.environ.get("PATH", "/usr/bin:/bin")}
        result = subprocess.run(
            ["bash", "--noprofile", "--norc", "-eo", "pipefail", step],
            cwd=work,
            env=env,
            capture_output=True,
            text=True,
            timeout=60,
        )
        record = ""
        if os.path.exists(marker):
            with open(marker) as recorded:
                record = recorded.read()
        self.assertNotIn("systemctl", record, "the step queried a manager itself")
        return result, record.count("observed")

    def test_the_step_queries_nothing_itself_and_hides_no_error(self):
        script = self.script()
        for needle in ("systemctl", "2>/dev/null", "ls -A", "|| true", "grep -q .", "|| live="):
            self.assertNotIn(needle, script)
        self.assertIn(f"grep -qx '{PASSED}' phase2-live.log || passed=$?", script)
        self.assertEqual(script.count('statuses=("${PIPESTATUS[@]}")'), 2)

    def test_a_passing_suite_with_nothing_left_passes(self):
        result, observed = self.step(0, PASSED, 0)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(observed, 1)

    def test_a_failed_suite_keeps_its_failure_and_is_still_observed(self):
        for observed_status in (0, 1):
            with self.subTest(observed_status=observed_status):
                result, observed = self.step(101, PASSED, observed_status)
                self.assertEqual(result.returncode, 101, result.stdout + result.stderr)
                self.assertEqual(observed, 1, "the observations run after a failed suite")
                self.assertIn("::error::the live suite failed (cargo exit 101)", result.stdout)

    def test_each_pipeline_command_keeps_its_own_status(self):
        captured = "::error::the live suite's output was not fully captured (tee exit 73)"
        failed = "::error::the live suite failed (cargo exit 101)"
        uncounted = "::error::the live suite did not report 31 passed cases"
        for cargo, tee, observed_status, output, status, present, absent in (
            (101, 73, 0, PASSED, 101, [failed, captured], [uncounted]),
            (0, 73, 0, PASSED, 1, [captured], ["the live suite failed"]),
            (101, 0, 0, PASSED, 101, [failed], ["not fully captured"]),
            (101, 73, 1, "", 101, [failed, captured, uncounted], []),
            (0, 73, 1, PASSED, 1, [captured], ["the live suite failed"]),
            (0, 0, 1, "", 1, [uncounted], ["the live suite failed", "not fully captured"]),
            (0, 0, 0, "", 1, [uncounted], ["the live suite failed", "not fully captured"]),
        ):
            with self.subTest(cargo=cargo, tee=tee, observed=observed_status, output=output):
                result, observed = self.step(cargo, output, observed_status, tee_status=tee)
                self.assertEqual(result.returncode, status, result.stdout + result.stderr)
                self.assertEqual(observed, 1, "both observations run after the attempted suite")
                for message in present:
                    self.assertIn(message, result.stdout)
                for message in absent:
                    self.assertNotIn(message, result.stdout)

    def test_a_missing_passed_count_fails_and_is_still_observed(self):
        for output in (
            "test result: ok. 30 live sandbox cases passed",
            f"{PASSED} (and more)",
            "",
        ):
            with self.subTest(output=output):
                result, observed = self.step(0, output, 0)
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertEqual(observed, 1)
                self.assertIn("::error::the live suite did not report 31 passed cases", result.stdout)

    def test_a_failed_observation_fails_a_passing_suite(self):
        for observed_status in (1, 2, 127):
            with self.subTest(observed_status=observed_status):
                result, observed = self.step(0, PASSED, observed_status)
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertEqual(observed, 1)


if __name__ == "__main__":
    unittest.main(verbosity=2)
