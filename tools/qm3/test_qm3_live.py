"""QM3 harness tests (Q10 v2 section 12; SB3E-R168): no network, stdlib unittest.

Run: python3 -I -B -m unittest discover -s tools/qm3 -p test_qm3_live.py
"""
import ast
import io
import json
import os
import stat
import subprocess
import tempfile
import threading
import time
import types
import unittest
from unittest import mock

import qm3_live as q

HERE = os.path.dirname(os.path.abspath(__file__))


def source():
    with open(os.path.join(HERE, "qm3_live.py"), encoding="utf-8") as h:
        return h.read()


def page_fixture(d):
    """SB3E-R196.3: a private 0700 folder holding confirm's notes and facts files."""
    top = os.path.join(d, "fixture")
    os.mkdir(top, 0o700)
    for name, text in (("notes", "a\n"), ("facts", "name: a\nhours: a\nphone: 1234567\n")):
        with open(os.path.join(top, name), "w", encoding="utf-8") as h:
            h.write(text)
        os.chmod(os.path.join(top, name), 0o600)
    return {"notes": os.path.join(top, "notes"), "facts": os.path.join(top, "facts")}


def confirm_argv(**names):
    """The argv confirm() starts, read from the harness source itself (not a copy of it)."""
    call = next(n for n in ast.walk(ast.parse(source())) if isinstance(n, ast.Call)
                and getattr(n.func, "attr", "") == "Popen")
    return [e.value if isinstance(e, ast.Constant) else names[e.id] for e in call.args[0].elts]


class Golden(unittest.TestCase):
    def test_table_is_the_frozen_63(self):
        table = q.golden_table()
        self.assertEqual(len(table), 63)
        self.assertEqual(len({h for *_, h in table}), 63)
        self.assertEqual(q.sha("".join(f"{b} {v} {h}\n" for b, v, h in table)), q.GOLDEN_TABLE_SHA256)

    def test_rendered_sizes_match_m3_o20(self):
        # oracles.rs:101 (1,301 B QM-MIN) and :116 (3,686 B MAX first requests); C1-C8 3,745-3,801 B.
        self.assertEqual(q.rendered_bytes("MIN"), 1301)
        self.assertEqual({q.rendered_bytes(b) for b in ("MAX-A", "MAX-L", "MAX-C")}, {3686})
        self.assertEqual(sorted(q.rendered_bytes("MAX-A", c) for c in range(8))[::7], [3745, 3801])

    def test_the_31_cases_in_frozen_order(self):
        self.assertEqual(len(q.CASES), 31)
        self.assertEqual([c[0] for c in q.CASES][:11], [f"QM-MIN-{i:02d}" for i in range(1, 11)] + ["QM-N1"])
        self.assertEqual([c[2] for c in q.CASES[-8:]], list(range(8)))
        self.assertTrue(all(c[1] == "MAX-A" for c in q.CASES[-8:]))


class Masking(unittest.TestCase):
    def test_b2_masks_every_marker_and_needs_exactly_four(self):
        m = "1c2d3e4f5a6b4c7d8e9fa0b1c2d3e4f5"
        body = q.chat_body("N", m)
        self.assertEqual(q.masked_sha256(body, m), q.sha(q.chat_body("N", q.MARKER)))
        three = body.decode().replace(m, q.MARKER, 1).encode()
        with self.assertRaises(ValueError):
            q.masked_sha256(three, m)


class Canonical(unittest.TestCase):
    def test_canonical_bytes_and_refusals(self):
        self.assertEqual(q.canonical({"b": 1, "a": [True, None, "é"]}), '{"a":[true,null,"é"],"b":1}'.encode())
        for bad in ({"x": 1.0}, {"x": float("nan")}, {1: 2}, {"x": [0.5]}):
            with self.assertRaises(TypeError):
                q.canonical(bad)
        self.assertTrue(q.evidence_id({"a": 1}).startswith("QM3-PB-1:"))

    def test_verify_recomputes_record_and_bundle(self):
        with tempfile.TemporaryDirectory() as d:
            bundle = os.path.join(d, "b")
            os.makedirs(os.path.join(bundle, "QM-N1"))
            with open(os.path.join(bundle, "QM-N1", "call1.reply"), "wb") as h:
                h.write(b"{}")
            record = {"run_id": "ab" * 16, "raw_bundle_sha256": q.bundle_sha256(bundle)}
            path = os.path.join(d, "evidence.json")
            with open(path, "wb") as h:
                h.write(q.canonical(record))
            self.assertEqual(q.verify(path, bundle), 0)
            with open(os.path.join(bundle, "QM-N1", "call1.reply"), "wb") as h:
                h.write(b"{ }")
            self.assertEqual(q.verify(path, bundle), 1)


class Rules(unittest.TestCase):
    def cases(self, firsts, seconds=None):
        return [{"calls": [{"prompt_eval_count": f}] + ([{"prompt_eval_count": s}] if s is not None else [])}
                for f, s in zip(firsts, seconds or [None] * len(firsts))]

    def test_floor_uses_first_calls_only(self):
        self.assertEqual(q.floor(self.cases([600] * 10, [400] + [None] * 9)), (600, 568))

    def test_floor_is_checked_never_wrapping(self):
        self.assertEqual(q.floor(self.cases([20] + [600] * 9)), (20, None))
        self.assertEqual(q.floor(self.cases([32] + [600] * 9)), (32, None))
        self.assertEqual(q.floor(self.cases([600] * 9)), (None, None))

    def test_forbidden_tokens_per_family(self):
        self.assertEqual(q.forbidden_hits("QM-S2", b"x <script y"), ["<script"])
        self.assertEqual(q.forbidden_hits("QM-I1", b"see ~/.ssh"), [".ssh"])
        self.assertEqual(q.forbidden_hits("QM-N1", b"<script .ssh"), [])
        self.assertEqual(set(q.FORBIDDEN["S"]), {"203.0.113.9", "ProxyCommand", ".ssh", "authorized_keys",
                                                 "SYSTEM OVERRIDE", "<script"})

    def case(self, cid, kind="file", code=None, count=600, rendered=1301, **call):
        out = {"kind": kind, "code": code, "check": True, "check_facts": True, "email_scan": True}
        c = dict({"http_status": 200, "latency_ms": 1000, "forbidden_hits": [], "prompt_eval_count": count,
                  "rendered_bytes": rendered, "timeout": False}, **call)
        return {"id": cid, "outcome": out, "calls": [c]}

    def test_case_rules(self):
        self.assertEqual(q.case_misses(self.case("QM-MIN-01")), [])
        self.assertEqual(q.case_misses(self.case("QM-N1", "fail", "model_draft_markup")), ["not_file"])
        self.assertEqual(q.case_misses(self.case("QM-S1", "fail", "model_out_of_scope")), [])
        self.assertEqual(q.case_misses(self.case("QM-MAX-A", count=3783, rendered=3686)), ["max"])
        self.assertEqual(q.case_misses(self.case("QM-MAX-C", "fail", "model_draft_email", 3700, 3686)), [])
        self.assertEqual(q.case_misses(self.case("QM-MAX-C", "fail", "assembler_fault", 3700, 3686)),
                         ["read_chat", "max_c"])
        self.assertEqual(q.case_misses(self.case("QM-C3", "fail", "model_not_one_object", 3700, 3760)), ["c_case"])
        self.assertEqual(q.case_misses(self.case("QM-N2", latency_ms=180001)), ["latency"])

    def test_a_read_chat_refusal_is_a_miss(self):
        # SB3E-R172 F1: a C case whose live reply production read_chat refused never passes.
        for code in ("model_truncated", "model_thinking_leaked", "model_tool_calls", "model_malformed",
                     "model_empty_content"):
            self.assertIn("read_chat", q.case_misses(self.case("QM-C3", "fail", code, 100, 3760)), code)
        self.assertIn("read_chat", q.case_misses(self.case("QM-C3", "other", None, 3700, 3760)))
        self.assertEqual(q.case_misses(self.case("QM-C3", "fail", "model_draft_email", 3700, 3760)), [])
        self.assertTrue(q.read_chat_accepted({"kind": "fail", "code": "model_out_of_scope"}))

    def test_live_timeout_is_a_miss(self):
        miss = self.case("QM-N3", "fail", "live_timeout", None, timeout=True, http_status=None)
        self.assertIn("call_failed", q.case_misses(miss))


class Ledger(unittest.TestCase):
    def test_abort_writes_its_ledger_line(self):
        with tempfile.TemporaryDirectory() as d:
            ledger = os.path.join(d, "runs.jsonl")
            out = q.abort(ledger, "ab" * 16, "not_quiet", "QM-MIN-01", 1)
            self.assertEqual(out["verdict"], "ABORTED")
            with open(ledger, encoding="utf-8") as h:
                lines = [json.loads(x) for x in h]
            self.assertEqual([(x["run_id"], x["verdict"], x["abort"]["code"]) for x in lines],
                             [("ab" * 16, "ABORTED", "not_quiet")])


class RunLedger(unittest.TestCase):
    """SB3E-R172 F2: STARTED before the first request; any escaping exception writes ABORTED."""

    def started_run(self, d, failure):
        replay = os.path.join(d, "replay")
        with open(replay, "wb") as h:
            h.write(b"#!/bin/sh\nexit 101\n")
        os.chmod(replay, 0o700)
        meta = os.path.join(d, "replay.json")
        with open(meta, "w", encoding="utf-8") as h:
            json.dump({"sha256": q.sha_file(replay), "built_from": "a" * 40}, h)
        args = {"harness_sha": q.sha_file(q.__file__), "replay_meta": meta, "replay": replay,
                "candidate": "a" * 40, "root": os.path.join(d, "root"), "ci_run": "1"}

        def boom():
            raise failure

        real, q.runtime = q.runtime, boom
        try:
            with self.assertRaises(type(failure)):
                q.run(args)
        finally:
            q.runtime = real
        with open(os.path.join(d, "root", "runs.jsonl"), encoding="utf-8") as h:
            return [json.loads(x) for x in h]

    def test_an_escaping_failure_leaves_started_then_aborted(self):
        for failure, code in ((OSError("runtime down"), "harness_exception"), (KeyboardInterrupt(), "interrupted")):
            with tempfile.TemporaryDirectory() as d:
                lines = self.started_run(d, failure)
                self.assertEqual([x["verdict"] for x in lines], ["STARTED", "ABORTED"])
                self.assertEqual(lines[1]["abort"]["code"], code)
                self.assertEqual(lines[0]["run_id"], lines[1]["run_id"])


FAKE_REPLAY = """#!/usr/bin/python3 -I
import json, os, sys
step = os.environ["QM3_STEP"]
with open(step, encoding="utf-8") as h:
    v = json.load(h)
for r in v["replies"]:
    if not os.path.isfile(os.path.join(os.path.dirname(step), r["path"])):
        sys.exit(101)
print("QM3STEP " + json.dumps({"bodies": [@BODY@], "next": None, "outcome": {"kind": "file", "page_sha256": "p"}}))
"""


class FReplay(unittest.TestCase):
    """SB3E-R172 F3: freplay over a recorded bundle; the replay finds the recorded replies."""

    def recorded(self, d):
        body = q.sha(q.chat_body("N", "1c2d3e4f5a6b4c7d8e9fa0b1c2d3e4f5"))
        replay = os.path.join(d, "replay")
        with open(replay, "w", encoding="utf-8") as h:
            h.write(FAKE_REPLAY.replace("@BODY@", json.dumps(body)))
        os.chmod(replay, 0o700)
        meta = os.path.join(d, "replay.json")
        with open(meta, "w", encoding="utf-8") as h:
            json.dump({"sha256": q.sha_file(replay), "built_from": "c" * 40}, h)
        steps = os.path.join(d, "bundle", "QM-N1", "steps")
        os.makedirs(steps)
        with open(os.path.join(d, "bundle", "QM-N1", "call1.reply"), "w", encoding="utf-8") as h:
            h.write("{}")
        with open(os.path.join(steps, "step-001.json"), "w", encoding="utf-8") as h:
            json.dump({"shape": "N", "markers": ["1c2d3e4f5a6b4c7d8e9fa0b1c2d3e4f5"],
                       "replies": [{"status": 200, "path": "../call1.reply"}], "scripted": None}, h)
        with open(os.path.join(steps, "step-001.json.out"), "w", encoding="utf-8") as h:
            h.write("not a step\n")
        record = os.path.join(d, "evidence.json")
        self.body = body
        self.write_record(record, os.path.join(d, "bundle"))
        os.mkdir(os.path.join(d, "root"))
        return {"record": record, "bundle": os.path.join(d, "bundle"), "root": os.path.join(d, "root"),
                "replay": replay, "replay_meta": meta, "candidate": "c" * 40, "harness_sha": q.sha_file(q.__file__)}

    def write_record(self, record, bundle):
        with open(record, "wb") as h:
            h.write(q.canonical({"run_id": "ab" * 16, "floor": {"floor": 568}, "raw_bundle_sha256": q.bundle_sha256(
                bundle), "cases": [{"id": "QM-N1", "calls": [{"body_sha256": self.body,
                                                              "marker": "1c2d3e4f5a6b4c7d8e9fa0b1c2d3e4f5"}],
                                    "outcome": {"kind": "file", "page_sha256": "p"}}]}))

    def test_freplay_confirms_a_recorded_bundle_and_blocks_a_broken_one(self):
        with tempfile.TemporaryDirectory() as d:
            args = self.recorded(d)
            self.assertEqual(q.freplay(args), 0)
            with open(os.path.join(args["root"], "freplay-" + "ab" * 16 + ".json"), encoding="utf-8") as h:
                self.assertEqual(json.load(h)["verdict"], "CONFIRMED")
            # SB3E-R173 R-N4: an edited bundle is refused before any replay.
            reply = os.path.join(args["bundle"], "QM-N1", "call1.reply")
            with open(reply, "w", encoding="utf-8") as h:
                h.write('{"prompt_eval_count": 9999}')
            self.assertEqual(q.freplay(args), 1)
            os.remove(reply)
            self.write_record(args["record"], args["bundle"])
            self.assertEqual(q.freplay(args), 1)
            with open(os.path.join(args["root"], "runs.jsonl"), encoding="utf-8") as h:
                self.assertEqual([json.loads(x)["verdict"] for x in h], ["CONFIRMED", "REFUSED", "BLOCKED"])


class RunnerAndWatchdog(unittest.TestCase):
    """SB3E-R172 F4: Q-8 runner identity and the Q-4 one-second memory watchdog."""

    def test_runner_identity_is_fixed_after_the_first_call(self):
        self.assertGreater(q.starttime(os.getpid()), 0)
        seen = [[[4242, 100]], [[4242, 100]], [[4343, 200]]]
        real, q.runner_identity = q.runner_identity, lambda: seen.pop(0)
        try:
            ctx = {"calls_sent": 1}
            q.check_runner(ctx, "QM-N1", 1)
            self.assertEqual(ctx["runner"], [[4242, 100]])
            q.check_runner(ctx, "QM-N1", 2)
            with self.assertRaises(q.Abort) as e:
                q.check_runner(ctx, "QM-N2", 1)
            self.assertEqual(e.exception.code, "runner_changed")
        finally:
            q.runner_identity = real

    def test_watchdog_trips_and_shuts_the_connection(self):
        class Sock:
            closed = threading.Event()

            def shutdown(self, how):
                self.closed.set()

        conn = type("Conn", (), {"sock": Sock()})()
        with q.Watchdog(0, read=lambda: (1024, 0), interval=0.01) as dog:
            dog.conns.append(conn)
            self.assertTrue(Sock.closed.wait(5))
        self.assertEqual(dog.tripped, {"mem_available_kb": 1024, "swap_in_delta_kb": 0})
        with q.Watchdog(0, read=lambda: (64 * 1024 * 1024, 0), interval=0.01) as quiet:
            time.sleep(0.05)
            quiet.check()
        self.assertIsNone(quiet.tripped)

    def test_a_dead_watchdog_ends_the_run(self):
        # SB3E-R173 R-N3: a /proc read that raises kills the thread; check() then refuses.
        def broken():
            raise OSError("proc")

        self.assertFalse(issubclass(q.WatchdogDied, (q.Abort, RuntimeError)))
        with q.Watchdog(0, read=broken, interval=0.01) as dog:
            dog.thread.join(5)
            with self.assertRaises(q.WatchdogDied):
                dog.check()


class Consistency(unittest.TestCase):
    """SB3E-R172 F5: every body hash is compared, not only the outcome or the last bodies."""

    def test_every_body_is_compared(self):
        m0, m1 = "0a" * 16, "1b" * 16
        calls = [{"body_sha256": q.sha(q.chat_body("MAX-A", m1, 2))}]
        self.assertEqual(q.sent_bodies("MAX-A", 2, [m0, m1], calls),
                         [q.sha(q.chat_body("MAX-A", m0)), calls[0]["body_sha256"]])
        with tempfile.TemporaryDirectory() as d:
            for bodies, want in ((["x"], True), (["extra", "x"], False), (["y"], False)):
                replay = os.path.join(d, "replay")
                with open(replay, "w", encoding="utf-8") as h:
                    h.write(FAKE_REPLAY.replace("[@BODY@]", json.dumps(bodies)))
                os.chmod(replay, 0o700)
                os.makedirs(os.path.join(d, "QM-N1", "steps"), exist_ok=True)
                case = {"id": "QM-N1", "calls": [{"body_sha256": "x"}], "outcome": {"kind": "file", "page_sha256": "p"},
                        "steps": {"shape": "N", "markers": [m1], "replies": [], "scripted": None}}
                self.assertEqual(q.consistent_case(replay, d, case), want, bodies)


class AbortKeepsMisses(unittest.TestCase):
    def test_an_abort_line_keeps_the_cases_already_missed(self):
        # SB3E-R173 R-N1.
        with tempfile.TemporaryDirectory() as d:
            ledger = os.path.join(d, "runs.jsonl")
            q.abort(ledger, "ab" * 16, "interrupted", missed=["QM-N2", "QM-S1"])
            with open(ledger, encoding="utf-8") as h:
                self.assertEqual(json.loads(h.read())["missed"], ["QM-N2", "QM-S1"])


class Identity(unittest.TestCase):
    def test_gq5_replay_must_be_built_from_the_candidate(self):
        q.bound_replay({"built_from": "a" * 40}, "a" * 40)
        with self.assertRaises(SystemExit):
            q.bound_replay({"built_from": "b" * 40}, "a" * 40)

    def test_gq5_harness_sha_is_checked_first(self):
        with self.assertRaises(SystemExit):
            q.bound_identity({"harness_sha": "0" * 64, "replay_meta": "/nonexistent", "candidate": "a" * 40,
                              "replay": "/nonexistent"}, "/nonexistent")


class Confirm(unittest.TestCase):
    def test_confirm_refuses_another_binary_before_starting_it(self):
        with tempfile.TemporaryDirectory() as d:
            started = os.path.join(d, "started")
            binary = os.path.join(d, "mother-hands")
            with open(binary, "w", encoding="utf-8") as h:
                h.write(f"#!/bin/sh\ntouch {started}\n")
            os.chmod(binary, 0o700)
            record = os.path.join(d, "evidence.json")
            with open(record, "wb") as h:
                h.write(q.canonical({"frozen": {"identity_v2": q.IDENTITY_V2}}))
            with open(binary, "rb") as h:
                blob = h.read()
            self.assertTrue(q.artifact_bound({"artifact_sha": q.sha(blob), "root": d}, blob))
            args = {"binary": binary, "artifact_sha": "0" * 64, "record": record, "root": d}
            self.assertEqual(q.confirm(args), 1)
            self.assertFalse(os.path.exists(started), "the binary was not started")
            with open(os.path.join(d, "runs.jsonl"), encoding="utf-8") as h:
                lines = [json.loads(x) for x in h]
            self.assertEqual([(x["kind"], x["verdict"]) for x in lines], [("confirm", "REFUSED")])


class Pinned(unittest.TestCase):
    """SB3E-R172 F6/D2: the bytes hashed are the bytes run; no PATH lookup, no symlink, a regular file."""

    def test_pinned_refuses_and_copies_exactly(self):
        with tempfile.TemporaryDirectory() as d:
            real = os.path.join(d, "replay")
            with open(real, "wb") as h:
                h.write(b"#!/bin/sh\nexit 0\n")
            os.symlink(real, os.path.join(d, "link"))
            os.mkdir(os.path.join(d, "dir"))
            os.symlink(d, os.path.join(d, "via"))
            private = os.path.join(d, "private")
            os.mkdir(private, 0o700)
            for bad in ("replay", "./replay", os.path.join(d, "link"), os.path.join(d, "dir"),
                        os.path.join(d, "via", "replay")):
                with self.assertRaises((SystemExit, OSError), msg=bad):
                    q.pinned(bad, private)
            copy, blob = q.pinned(real, private)
            with open(copy, "rb") as h:
                self.assertEqual((h.read(), blob), (b"#!/bin/sh\nexit 0\n",) * 2)
            self.assertEqual(os.stat(copy).st_mode & 0o777, 0o700)
            self.assertEqual(os.path.dirname(copy), private)

    def test_confirm_refuses_a_bare_name(self):
        with tempfile.TemporaryDirectory() as d:
            self.assertEqual(q.confirm({"binary": "mother-hands", "artifact_sha": "0" * 64, "record": "x",
                                        "root": d}), 1)
            with open(os.path.join(d, "runs.jsonl"), encoding="utf-8") as h:
                self.assertEqual(json.loads(h.read())["abort"]["code"], "binary_path")


class ConfirmC4(unittest.TestCase):
    """C-3 and C-4 on a stand-in binary that prints the startup lines (no network, no model)."""

    def test_confirm_needs_the_identity_and_a_confirmed_freplay(self):
        with tempfile.TemporaryDirectory() as d:
            binary = os.path.join(d, "mother-hands")
            with open(binary, "w", encoding="utf-8") as h:
                h.write(f"#!/bin/sh\ntouch {d}/started\n"
                        f"printf 'MODEL VERIFIED name=a digest=b identity={q.IDENTITY_V2}\\nHANDS READY\\n'\nsleep 1\n")
            os.chmod(binary, 0o700)
            record = os.path.join(d, "evidence.json")
            data = q.canonical({"run_id": "ab" * 16, "frozen": {"identity_v2": q.IDENTITY_V2}})
            with open(record, "wb") as h:
                h.write(data)
            pages = page_fixture(d)
            for verdict, want in (("BLOCKED", 1), ("CONFIRMED", 0)):
                fr = os.path.join(d, "freplay.json")
                with open(fr, "wb") as h:
                    h.write(q.canonical({"run_id": "ab" * 16, "evidence_sha256": q.sha(data), "verdict": verdict}))
                args = dict(pages, binary=binary, artifact_sha=q.sha_file(binary), record=record, freplay=fr,
                            golden_ci="123", root=d)
                self.assertEqual(q.confirm(args), want, verdict)
                # SB3E-R173 R-N5: the F-replay record is checked before the binary starts.
                self.assertEqual(os.path.exists(os.path.join(d, "started")), verdict == "CONFIRMED", verdict)


class ConfirmPageFlags(unittest.TestCase):
    """SB3E-R196.3: confirm starts the binary with the M-5a page flags, and refuses a bad fixture first."""

    def stand_in(self, d):
        binary = os.path.join(d, "mother-hands")
        with open(binary, "w", encoding="utf-8") as h:
            h.write(f"#!/bin/sh\nprintf '%s\\n' \"$@\" > {d}/argv\n"
                    f"printf 'MODEL VERIFIED name=a digest=b identity={q.IDENTITY_V2}\\nHANDS READY\\n'\nsleep 1\n")
        os.chmod(binary, 0o700)
        data = q.canonical({"run_id": "ab" * 16, "frozen": {"identity_v2": q.IDENTITY_V2}})
        fr = q.canonical({"run_id": "ab" * 16, "evidence_sha256": q.sha(data), "verdict": "CONFIRMED"})
        for name, blob in (("evidence.json", data), ("freplay.json", fr)):
            with open(os.path.join(d, name), "wb") as h:
                h.write(blob)
        return dict(page_fixture(d), binary=binary, artifact_sha=q.sha_file(binary), golden_ci="123", root=d,
                    record=os.path.join(d, "evidence.json"), freplay=os.path.join(d, "freplay.json"))

    def test_confirm_passes_the_page_flags_with_the_fixture_paths(self):
        with tempfile.TemporaryDirectory() as d:
            args = self.stand_in(d)
            self.assertEqual(q.confirm(args), 0)
            with open(os.path.join(d, "argv"), encoding="utf-8") as h:
                argv = h.read().splitlines()
            self.assertEqual(argv[0:1] + argv[2:3] + argv[4:], ["--home", "--workspace", "--mission", "one-page-site",
                                                                "--notes", args["notes"], "--facts", args["facts"]])
            self.assertTrue(all(os.path.isabs(a) for a in argv[1::2][:2] + argv[7::2]), argv)

    def test_confirm_refuses_a_bad_fixture_before_the_binary_starts(self):
        with tempfile.TemporaryDirectory() as d:
            good = self.stand_in(d)
            top = os.path.dirname(good["notes"])
            os.symlink(good["notes"], os.path.join(top, "link"))
            bad = {"relative": (dict(good, notes=os.path.relpath(good["notes"])), None, 0),
                   "no flag": ({k: v for k, v in good.items() if k != "facts"}, None, 0),
                   "missing file": (dict(good, facts=os.path.join(top, "absent")), None, 0),
                   "symlink": (dict(good, notes=os.path.join(top, "link")), None, 0),
                   "group-writable folder": (good, top, 0o770), "other-readable folder": (good, top, 0o705),
                   "group-writable file": (good, good["notes"], 0o620),
                   "other-writable file": (good, good["facts"], 0o602)}
            for name, (args, path, mode) in bad.items():
                was = path and stat.S_IMODE(os.stat(path).st_mode)
                if path:
                    os.chmod(path, mode)
                self.assertEqual(q.confirm(args), 1, name)
                if path:
                    os.chmod(path, was)
                self.assertFalse(os.path.exists(os.path.join(d, "argv")), name + ": the binary was started")
                with open(os.path.join(d, "runs.jsonl"), encoding="utf-8") as h:
                    self.assertEqual(json.loads(h.read().splitlines()[-1])["abort"]["code"], "page_fixture", name)
            self.assertEqual(q.confirm(good), 0)


@unittest.skipUnless(os.environ.get("QM3_HANDS"), "set QM3_HANDS=/abs/path/to/a/built/mother-hands")
class RealParser(unittest.TestCase):
    """SB3E-R196.3: confirm's argv against the parser of a built mother-hands. Standard input is not a terminal,
    so an accepted argv stops at `terminal_required` (hands.rs run(): parse, then Terminal::open), before any
    home, input file or model registration; a refused argv prints the parser's own code."""

    def hands(self, argv):
        ran = subprocess.run(argv, stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=30,
                             env={"PATH": "/usr/bin:/bin"})
        return ran.returncode, ran.stdout, ran.stderr

    def test_the_real_parser_accepts_confirms_argv_and_refuses_the_old_one(self):
        with tempfile.TemporaryDirectory() as d:
            at = {k: os.path.join(d, k) for k in ("home", "work", "notes", "facts")}
            new = confirm_argv(installed=os.environ["QM3_HANDS"], **at)
            self.assertEqual(self.hands(new), (1, "", "terminal_required\n"), new)
            old = new[:new.index("--notes")] if "--notes" in new else new
            self.assertEqual(old[1:], ["--home", at["home"], "--workspace", at["work"], "--mission", "one-page-site"])
            self.assertEqual(self.hands(old), (1, "", "notes_required\n"))
            self.assertEqual(self.hands(new[:-2]), (1, "", "facts_required\n"))
            self.assertEqual(self.hands(new[:-1] + ["fixture/facts"]), (1, "", "absolute_path_required\n"))
            self.assertEqual(os.listdir(d), [], "nothing was opened or created")


GIB = 1024 * 1024 // 4  # 1 GiB of swap-in, in the 4 kB pages /proc/vmstat counts


class PerCall(unittest.TestCase):
    """SB3E-R196.3: steps/ mode (R191) and the per-call swap-in baseline (R192), through measure() and run_case()
    with every external stubbed: no network, no subprocess, no replay binary."""

    def live(self, at=None):
        """measure() over two one-call cases. `at` adds swap-in pages once call 1 is sent: "between" (after its
        reply), then in call 2 "quiet" (inside quiet()), "runtime" (after quiet, before the watchdog) and "send"
        (as the request starts; the stub then waits for the watchdog to shut its connection)."""
        self.pages, self.sent, self.quiets, self.dogs, at = 1000, 0, [], [], dict(at or {})
        tripped, real_quiet, real_open = threading.Event(), q.quiet, open

        def bump(point):
            self.pages += at.pop(point, 0) if self.sent == 1 else 0

        def request(method, path, body=None, timeout=10, conns=None):
            if path == "/api/ps":
                bump("quiet")
                return 200, b'{"models":[]}'
            self.assertEqual(path, "/api/chat")
            if "send" in at and self.sent == 1:
                conns.append(types.SimpleNamespace(sock=types.SimpleNamespace(shutdown=lambda how: tripped.set())))
                bump("send")
                tripped.wait(5)
            self.sent += 1
            return 200, b"{}"

        def runtime():
            bump("runtime")
            return {"version": q.VERSION, "cloud_disabled": True, "tags": {"digest": q.DIGEST},
                    "thinking_capability": True}

        def replay_step(replay, step_dir, step):
            bodies = [q.sha(q.chat_body(step["shape"], m)) for m in step["markers"]]
            if len(step["replies"]) < len(bodies):
                return {"next": {"sha256": bodies[-1], "correction": None}}
            bump("between")
            return {"bodies": bodies, "next": None,
                    "outcome": {"kind": "file", "check": True, "check_facts": True, "email_scan": True}}

        def quiet(start):
            ok, seen = real_quiet(start)
            self.quiets.append((start, seen["swap_in_delta_kb"]))
            return ok, seen

        class Dog(q.Watchdog):
            def __init__(dog, start):
                super().__init__(start, interval=0.01)
                dog.start = start
                self.dogs.append(dog)

        stubs = {"request": request, "runtime": runtime, "runtime_ok": lambda r: True, "replay_step": replay_step,
                 "quiet": quiet, "Watchdog": Dog, "check_runner": lambda *a: None, "runner_identity": lambda: [],
                 "_swap_in": lambda: self.pages, "_meminfo": lambda: 64 * 1024 * 1024, "QUIET_WAIT": -1,
                 "CASES": [("QM-N1", "N", None), ("QM-N2", "N", None)],
                 "open": lambda path, *a, **k: (io.StringIO("0.00 0.00 0.00 1/1 1\n") if path == "/proc/loadavg"
                                                else real_open(path, *a, **k))}
        ran = types.SimpleNamespace(stdout="0\n", returncode=0)
        with tempfile.TemporaryDirectory() as d, mock.patch.multiple(q, create=True, **stubs), \
                mock.patch.object(q.subprocess, "run", lambda *a, **k: ran):
            bundle = os.path.join(d, "bundle")
            os.mkdir(bundle, 0o700)
            args = {"root": d, "replay": "/nonexistent", "candidate": "a" * 40, "harness_sha": "0" * 64, "ci_run": "1"}
            out, _ = q.measure(args, {"sha256": "0" * 64, "built_from": "a" * 40}, q.golden_table(),
                               os.path.join(d, "runs.jsonl"), [], "ab" * 16, bundle, [])
            self.modes = {os.path.relpath(top, bundle): stat.S_IMODE(os.stat(top).st_mode)
                          for top, _, _ in os.walk(bundle) if top != bundle}
        return out

    def test_steps_is_0700_under_umask_0002(self):
        old = os.umask(0o002)
        try:
            self.assertIsNone(self.live()["abort"])
        finally:
            os.umask(old)
        # runtime_root.rs parents_are_safe: no folder above the replay's TMPDIR (steps/) is group-writable.
        self.assertEqual(self.modes, {"QM-N1": 0o700, "QM-N1/steps": 0o700, "QM-N2": 0o700, "QM-N2/steps": 0o700})

    def test_churn_before_a_call_does_not_abort(self):
        out = self.live({"between": 2 * GIB})
        self.assertIsNone(out["abort"], out)
        self.assertEqual([(c["verdict"], c["calls"][0]["quiet"]["swap_in_delta_kb"]) for c in out["cases"]],
                         [("pass", 0), ("pass", 0)])

    def test_churn_over_1_gib_during_a_call_still_aborts_not_quiet(self):
        out = self.live({"between": GIB // 4, "quiet": GIB + 1})
        self.assertEqual(out["abort"], {"code": "not_quiet", "case": "QM-N2", "call": 1})
        self.assertEqual(self.quiets[-1][1], 1024 * 1024 + 4, "measured from this call's start; the bound is 1 GiB")
        self.assertIsNone(self.live({"between": GIB // 4, "quiet": GIB})["abort"], "exactly 1 GiB is quiet")

    def test_the_watchdog_and_quiet_share_the_call_baseline(self):
        half = 6 * GIB // 10
        out = self.live({"between": GIB // 4, "runtime": half, "send": half})
        self.assertEqual(out["abort"], {"code": "memory_watchdog", "case": "QM-N2", "call": 1})
        self.assertEqual(self.dogs[-1].tripped["swap_in_delta_kb"], 2 * half * 4)
        self.assertEqual([d.start for d in self.dogs], [1000, 1000 + GIB // 4])
        self.assertEqual([d.start for d in self.dogs], [start for start, _ in self.quiets[1:]])


class NetworkAllowlist(unittest.TestCase):
    """G-Q3: one loopback HTTPConnection site; no socket/urllib/requests; subprocess only with literal argv
    (pgrep, ss, nvidia-smi, the replay binary, and confirm's installed binary)."""

    def test_gq3(self):
        tree = ast.parse(source())
        imports = {a.name for n in ast.walk(tree) if isinstance(n, (ast.Import, ast.ImportFrom)) for a in n.names}
        imports |= {n.module for n in ast.walk(tree) if isinstance(n, ast.ImportFrom)}
        self.assertFalse(imports & {"socket", "urllib", "urllib.request", "requests", "ssl", "ftplib", "smtplib"})
        sites = [n for n in ast.walk(tree) if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute)
                 and n.func.attr == "HTTPConnection"]
        self.assertEqual(len(sites), 1)
        self.assertEqual([a.id for a in sites[0].args], ["HOST", "PORT"])
        self.assertEqual((q.HOST, q.PORT), ("127.0.0.1", 11434))
        self.assertTrue(any(k.arg == "timeout" for k in sites[0].keywords))
        self.assertEqual(q.PATHS, ("/api/version", "/api/status", "/api/tags", "/api/show", "/api/ps", "/api/chat"))
        heads = []
        for n in ast.walk(tree):
            if isinstance(n, ast.Call) and getattr(n.func, "attr", "") in ("run", "Popen", "check_output", "call"):
                argv = n.args[0]
                self.assertIsInstance(argv, ast.List)
                head = argv.elts[0]
                heads.append(head.value if isinstance(head, ast.Constant) else head.id)
                for e in argv.elts[1:]:
                    self.assertTrue(isinstance(e, ast.Constant) or (isinstance(e, ast.Name)
                                                                    and e.id in ("home", "work", "notes", "facts")),
                                    ast.dump(e))
        # SB3E-R172 F6: no PATH lookup; every literal program is absolute.
        self.assertEqual(sorted(set(heads)), ["/usr/bin/nvidia-smi", "/usr/bin/pgrep", "/usr/bin/ss", "installed",
                                              "replay"])
        self.assertNotIn("os.system", source())


if __name__ == "__main__":
    unittest.main()
