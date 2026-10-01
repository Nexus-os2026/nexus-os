#!/usr/bin/env python3
"""P2-V1-R1 fixture controls for `phase2_cleanup_check.py` and for the live
step of `.github/workflows/ci-phase2-linux-sandbox.yml` that runs it.

These are not live evidence. No user manager is contacted and nothing is
created outside this test's temporary directories: stand-in queries are
`/bin/sh` scripts, bounded and reaped by the observation itself; runtime
directories are fixture trees in which this test's uid stands in for root;
failures this host's permissions cannot produce deterministically (a refused
open or listing, a failing or foreign stat) are injected. The live step runs
as GitHub runs a bash step, with stand-ins for the live suite and the
observations.

    python3 scripts/ci/test_phase2_cleanup_check.py
"""

import errno
import os
import shlex
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
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


def gone(pid, within=5.0):
    """Whether `pid` is gone (or a zombie awaiting its new parent) within
    the bound."""
    deadline = time.monotonic() + within
    while True:
        try:
            with open(f"/proc/{pid}/stat") as stat_file:
                state = stat_file.read().rsplit(") ", 1)[1][:1]
        except (FileNotFoundError, ProcessLookupError):
            return True
        if state == "Z":
            return True
        if time.monotonic() > deadline:
            return False
        time.sleep(0.02)


def temporary(test, prefix="p2v1r1-"):
    directory = tempfile.mkdtemp(prefix=prefix)
    test.addCleanup(shutil.rmtree, directory, True)
    os.chmod(directory, 0o755)
    return directory


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

    def test_a_query_that_does_not_finish_is_killed_reaped_and_an_error(self):
        started = time.monotonic()
        error = self.fails("timeout", "exec /bin/sleep 30", timeout=0.3)
        self.assertIn(KILLED, str(error))
        self.assertLess(time.monotonic() - started, 8)

    def test_a_query_whose_output_stays_open_is_ended_with_its_process_group(self):
        # The query exits at once, but a process it left behind keeps its
        # output open: no answer is complete, and the bound still holds.
        pid_file = os.path.join(temporary(self), "pid")
        started = time.monotonic()
        self.fails("timeout", f"/bin/sleep 30 & echo $! > {pid_file}; exit 0", timeout=0.5)
        self.assertLess(time.monotonic() - started, 8)
        with open(pid_file) as pid:
            self.assertTrue(gone(int(pid.read())), "the query's process group was killed")

    def test_a_query_that_writes_too_much_is_killed_and_an_error(self):
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

    def step(self, live_status, live_output, observed_status):
        work = temporary(self, "p2v1r1-step-")
        stand_ins = os.path.join(work, "bin")
        os.makedirs(stand_ins)
        os.makedirs(os.path.join(work, "scripts", "ci"))
        marker = os.path.join(work, "observed")
        stand_in_scripts = {
            "cargo": f"printf '%s\\n' {shlex.quote(live_output)}\nexit {live_status}\n",
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
        for needle in ("systemctl", "2>/dev/null", "ls -A", "|| true", "grep -q ."):
            self.assertNotIn(needle, script)
        self.assertIn(f"grep -qx '{PASSED}' phase2-live.log || passed=$?", script)

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
                self.assertIn("::error::the live suite failed (exit 101)", result.stdout)

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
