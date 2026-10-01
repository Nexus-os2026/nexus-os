#!/usr/bin/env python3
"""P2-V1-R1 to R3A fixture controls for the live step of
`.github/workflows/ci-phase2-linux-sandbox.yml`: its own script, run as GitHub
runs a bash step.

The cleanup observations themselves are the live harness's observation-only
mode (`--cleanup-observation`, `support/cleanup_observation.rs`), controlled
by `tests/phase2_cleanup_observation.rs`; no runtime observer is written in
Python any more. Here the step's shell logic is proved with stand-ins on its
PATH, bounded subprocesses that are fixture drivers, not observers: a `cargo`
that records every invocation and answers as each test says (the gate's
observation, the live suite, the observation after it), the real `tee`
followed by a given status, and a `systemctl` and a `python3` that only
record that they were called (the step must call neither). Nothing reaches a
user manager or a real suite.

    python3 scripts/ci/test_phase2_cleanup_check.py
"""

import os
import shlex
import shutil
import subprocess
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
WORKFLOW = os.path.join(HERE, "..", "..", ".github", "workflows", "ci-phase2-linux-sandbox.yml")
LIVE_STEP = "      - name: Live Phase Two isolation, escape and cleanup suite (every layer required)"
PASSED = "test result: ok. 31 live sandbox cases passed"
SUITE = [
    "test",
    "-p",
    "nexus-verifier-sandbox",
    "--locked",
    "--features",
    "development-toolchain",
    "--test",
    "phase2_live_sandbox",
]
OBSERVATION = [*SUITE, "--", "--cleanup-observation"]
OBSERVE, LIVE = "observe", "live"


def temporary(test, prefix="p2v1r3a-step-"):
    directory = tempfile.mkdtemp(prefix=prefix)
    test.addCleanup(shutil.rmtree, directory, True)
    return directory


class WorkflowLiveStep(unittest.TestCase):
    """The workflow's own live-step script, with stand-ins for the gate's
    observation, the live suite, the capture of its output and the
    observation after it."""

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

    def step(self, preflight, live_status, live_output, observed, tee_status=0):
        """Run the step: `preflight` and `observed` are the statuses of the
        observations before and after the suite. Returns the result and
        every cargo invocation, in order (OBSERVE or LIVE)."""
        work = temporary(self)
        stand_ins = os.path.join(work, "bin")
        os.makedirs(stand_ins)
        record = os.path.join(work, "cargo-calls")
        forbidden = os.path.join(work, "forbidden")
        tee = shutil.which("tee")
        self.assertTrue(tee and os.path.isabs(tee), "a tee to stand behind")
        observation = " ".join(OBSERVATION)
        suite = " ".join(SUITE)
        cargo = f"""
printf '%s|%s\\n' "$PWD" "$*" >> {shlex.quote(record)}
case "$*" in
  {shlex.quote(observation)})
    if [ "$(grep -c -- '--cleanup-observation$' {shlex.quote(record)})" = 1 ]; then
      exit {preflight}
    fi
    exit {observed}
    ;;
  {shlex.quote(suite)})
    printf '%s\\n' {shlex.quote(live_output)}
    exit {live_status}
    ;;
esac
echo "unexpected cargo $*" >&2
exit 97
"""
        stand_in_scripts = {
            "cargo": cargo,
            # The capture: the real tee, then the given status (no disk is
            # ever exhausted).
            "tee": f'{shlex.quote(tee)} "$@"\nexit {tee_status}\n',
            # Never a real user manager or a runtime observer in Python.
            "systemctl": f"echo systemctl >> {shlex.quote(forbidden)}\nexit 97\n",
            "python3": f"echo python3 >> {shlex.quote(forbidden)}\nexit 97\n",
        }
        for name, body in stand_in_scripts.items():
            path = os.path.join(stand_ins, name)
            with open(path, "w") as script:
                script.write(f"#!/bin/sh\n{body}")
            os.chmod(path, 0o755)
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
        self.assertFalse(os.path.exists(forbidden), "the step called systemctl or python3")
        calls = []
        if os.path.exists(record):
            with open(record) as recorded:
                for line in recorded.read().splitlines():
                    cwd, argv = line.split("|", 1)
                    self.assertEqual(cwd, work, "cargo ran in the checkout")
                    self.assertIn(argv, (observation, suite), "only the exact two commands")
                    calls.append(OBSERVE if argv == observation else LIVE)
        self.assertNotIn("unexpected cargo", result.stderr)
        return result, calls

    def test_the_step_observes_only_through_the_harness_and_hides_no_error(self):
        script = self.script()
        for needle in (
            "systemctl",
            "python3",
            "phase2_cleanup_check",
            "2>/dev/null",
            "ls -A",
            "|| true",
            "grep -q .",
            "|| live=",
            "continue-on-error",
        ):
            self.assertNotIn(needle, script)
        # One observation command, the exact harness mode of this checkout,
        # used before and after the suite.
        self.assertEqual(script.count(" ".join(["cargo", *OBSERVATION])), 1)
        self.assertEqual(script.count("--cleanup-observation"), 1)
        gate = script.index("observe_cleanup || preflight=$?\n")
        blocked = script.index("  exit 1\nfi\n", gate)
        suite = script.index(f"if cargo {' '.join(SUITE)} 2>&1 | tee phase2-live.log; then\n")
        after = script.index("observe_cleanup || observed=$?\n")
        self.assertTrue(gate < blocked < suite < after, script)
        self.assertEqual(script.count('statuses=("${PIPESTATUS[@]}")'), 2)
        self.assertIn(f"grep -qx '{PASSED}' phase2-live.log || passed=$?", script)

    def test_a_passing_suite_with_nothing_left_passes(self):
        result, calls = self.step(0, 0, PASSED, 0)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(calls, [OBSERVE, LIVE, OBSERVE])

    def test_a_failed_or_finding_gate_blocks_the_live_suite(self):
        for preflight in (1, 2, 101, 127):
            with self.subTest(preflight=preflight):
                result, calls = self.step(preflight, 0, PASSED, 0)
                self.assertEqual(calls, [OBSERVE], "the live suite never ran")
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertIn(
                    "::error::the cleanup observation found something left, or could not "
                    f"answer, before the live suite (cargo exit {preflight}): the live suite "
                    "was not run",
                    result.stdout,
                )

    def test_a_failed_suite_keeps_its_failure_and_is_still_observed(self):
        for observed in (0, 1):
            with self.subTest(observed=observed):
                result, calls = self.step(0, 101, PASSED, observed)
                self.assertEqual(result.returncode, 101, result.stdout + result.stderr)
                self.assertEqual(calls, [OBSERVE, LIVE, OBSERVE], "observed after a failed suite")
                self.assertIn("::error::the live suite failed (cargo exit 101)", result.stdout)

    def test_each_pipeline_command_keeps_its_own_status(self):
        captured = "::error::the live suite's output was not fully captured (tee exit 73)"
        failed = "::error::the live suite failed (cargo exit 101)"
        uncounted = "::error::the live suite did not report 31 passed cases"
        for cargo, tee, observed, output, status, present, absent in (
            (101, 73, 0, PASSED, 101, [failed, captured], [uncounted]),
            (0, 73, 0, PASSED, 1, [captured], ["the live suite failed"]),
            (101, 0, 0, PASSED, 101, [failed], ["not fully captured"]),
            (101, 73, 1, "", 101, [failed, captured, uncounted], []),
            (0, 73, 1, PASSED, 1, [captured], ["the live suite failed"]),
            (0, 0, 1, "", 1, [uncounted], ["the live suite failed", "not fully captured"]),
            (0, 0, 0, "", 1, [uncounted], ["the live suite failed", "not fully captured"]),
        ):
            with self.subTest(cargo=cargo, tee=tee, observed=observed, output=output):
                result, calls = self.step(0, cargo, output, observed, tee_status=tee)
                self.assertEqual(result.returncode, status, result.stdout + result.stderr)
                self.assertEqual(calls, [OBSERVE, LIVE, OBSERVE], "observed after the suite")
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
                result, calls = self.step(0, 0, output, 0)
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertEqual(calls, [OBSERVE, LIVE, OBSERVE])
                self.assertIn("::error::the live suite did not report 31 passed cases", result.stdout)

    def test_a_failed_observation_after_the_suite_fails_a_passing_suite(self):
        for observed in (1, 2, 101, 127):
            with self.subTest(observed=observed):
                result, calls = self.step(0, 0, PASSED, observed)
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertEqual(calls, [OBSERVE, LIVE, OBSERVE])
                self.assertIn(
                    "::error::the cleanup observation found something left, or could not "
                    f"answer, after the live suite (cargo exit {observed})",
                    result.stdout,
                )


if __name__ == "__main__":
    unittest.main(verbosity=2)
