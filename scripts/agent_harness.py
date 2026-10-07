"""Offline evidence checks. Reading a document never authorizes an action."""
import datetime as dt
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess

PREREQUISITE = {"PASS", "FAIL", "HARNESS_ERROR", "NOT_RUN"}
CONTROL = {"KILLED", "SURVIVED", "INVALID", "HARNESS_ERROR", "NOT_RUN"}
MISSION = {"RUNNING", "BLOCKED", "READY_FOR_REVIEW", "FAILED"}
REVIEW = {"ACCEPTABLE_FOR_NEXT_GATE", "CHANGES_REQUIRED", "BLOCKED", "INCONCLUSIVE"}
PROCEDURES = ("mission", "implement", "verify", "review", "mutation", "handover", "recover")
AGENTS_BUDGET = 12000


class InvalidEvidence(ValueError):
    """A document or measured evidence relationship is inconsistent."""


def require(condition, message):
    if not condition:
        raise InvalidEvidence(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def command_hash(command):
    return digest(json.dumps(command, separators=(",", ":"), ensure_ascii=True).encode())


def git(root, *args):
    return subprocess.check_output(
        ["git", "-C", str(root), *args], text=True,
        env={**os.environ, "GIT_OPTIONAL_LOCKS": "0"},
    ).strip()


def oid(value):
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{40}", value) is not None


def sha256(value):
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def local_file(root, relative):
    require(isinstance(relative, str), "artifact reference must be a relative string")
    p = PurePosixPath(relative)
    require(relative and not p.is_absolute() and ".." not in p.parts
            and "\\" not in relative and ":" not in relative, "unsafe artifact reference")
    target = root
    for part in p.parts:
        target = target / part
        require(not target.is_symlink(), "symlink artifact is not supported")
    require(target.is_file(), f"missing artifact: {relative}")
    return target


def fields(path, expected):
    text = path.read_text(encoding="utf-8")
    chunks = re.split(r"(?m)^## (.+)\n", text)
    result = {}
    for i in range(1, len(chunks), 2):
        key, value = chunks[i], chunks[i + 1].strip()
        require(key not in result, f"{path.name}: duplicate field {key}")
        result[key] = value
    for key in expected:
        require(result.get(key), f"{path.name}: missing/empty field {key}")
    return result


def validate_receipt(r, evidence_root, mission_id):
    required = ("receipt_id", "mission_id", "timestamp_start", "timestamp_end", "cwd",
                "candidate_sha", "candidate_tree", "command", "command_hash", "exit_code",
                "status", "stdout_ref", "stderr_ref", "raw_log_hash", "environment",
                "acceptance_criterion", "control_ids", "notes")
    require(isinstance(r, dict) and all(k in r for k in required), "receipt fields missing")
    require(isinstance(r["receipt_id"], str) and r["receipt_id"], "receipt ID missing")
    require(r["mission_id"] == mission_id, "receipt mission mismatch")
    require(oid(r["candidate_sha"]) and oid(r["candidate_tree"]), "invalid candidate identity")
    require(isinstance(r["cwd"], str) and r["cwd"], "cwd missing")
    require(isinstance(r["command"], list) and r["command"]
            and all(isinstance(a, str) and a for a in r["command"]), "command argv missing")
    require(command_hash(r["command"]) == r["command_hash"], "command hash mismatch")
    require(r["status"] in {"EXITED", "HARNESS_ERROR", "NOT_RUN"}, "invalid receipt status")
    require(isinstance(r["environment"], dict) and r["environment"], "environment identity missing")
    require(isinstance(r["acceptance_criterion"], str) and r["acceptance_criterion"], "criterion missing")
    require(isinstance(r["notes"], str), "receipt notes must be text")
    ids = r["control_ids"]
    require(isinstance(ids, list) and all(isinstance(x, str) and x for x in ids)
            and len(ids) == len(set(ids)), "invalid/duplicate receipt controls")
    if r["status"] == "NOT_RUN":
        require(r["exit_code"] is None and r["timestamp_start"] is None
                and r["timestamp_end"] is None, "NOT_RUN fabricated execution")
        require(r["stdout_ref"] is None and r["stderr_ref"] is None
                and r["raw_log_hash"] == {}, "NOT_RUN fabricated logs")
        return
    try:
        start, end = (dt.datetime.fromisoformat(r[k]) for k in ("timestamp_start", "timestamp_end"))
        require(start.tzinfo is not None and end.tzinfo is not None and start <= end,
                "invalid receipt timestamp ordering/timezone")
    except (TypeError, ValueError) as exc:
        raise InvalidEvidence("invalid receipt timestamps") from exc
    require(type(r["exit_code"]) is int or
            (r["status"] == "HARNESS_ERROR" and r["exit_code"] is None), "exit code missing/invalid")
    for stream in ("stdout", "stderr"):
        path = local_file(evidence_root, r[stream + "_ref"])
        require(r["raw_log_hash"].get(stream) == digest(path.read_bytes()), f"{stream} hash mismatch")


def unique_ids(ids, what):
    require(isinstance(ids, list) and ids and all(isinstance(x, str) and x for x in ids),
            f"{what}: IDs missing")
    require(len(ids) == len(set(ids)), f"{what}: duplicate IDs")
    return set(ids)


def validate_campaign(data, evidence_root):
    """Validate mappings, never stdout markers; do not execute any receipt command."""
    require(oid(data.get("candidate_sha")) and oid(data.get("candidate_tree")), "campaign identity missing")
    require(data.get("mission_status") in MISSION, "invalid mission status")
    require(isinstance(data.get("campaign_id"), str) and data["campaign_id"], "campaign ID missing")
    expected = unique_ids(data.get("required_controls"), "inventory")
    require(isinstance(data.get("executions"), list), "executions missing")
    executions, baseline, mutations, outcomes, cases = {}, set(), set(), {}, set()
    prerequisite_commands = mutation_commands = 0
    for e in data["executions"]:
        require(isinstance(e, dict), "invalid execution")
        eid = e.get("command_execution_id")
        require(isinstance(eid, str) and eid and eid not in executions, "duplicate/missing execution ID")
        require(e.get("campaign_id") == data["campaign_id"], "cannot combine campaign IDs")
        r = e.get("receipt")
        validate_receipt(r, evidence_root, data["mission_id"])
        require(r["receipt_id"] == eid, "execution/receipt ID mismatch")
        require((r["candidate_sha"], r["candidate_tree"]) ==
                (data["candidate_sha"], data["candidate_tree"]), "campaign candidate mismatch")
        ids = unique_ids(r["control_ids"], "execution coverage")
        require(ids <= expected, "unknown control ID")
        kind, result = e.get("kind"), e.get("result")
        if kind == "prerequisite":
            require(result in PREREQUISITE, "invalid prerequisite result")
            require(not ids & baseline, "duplicate prerequisite logical coverage")
            baseline |= ids
            if result == "NOT_RUN":
                require(r["status"] == "NOT_RUN", "NOT_RUN execution mismatch")
            elif result in {"PASS", "FAIL"}:
                require(r["status"] == "EXITED", "prerequisite did not exit")
                require((r["exit_code"] == 0) == (result == "PASS"), "prerequisite exit/result mismatch")
            else:
                require(r["status"] == "HARNESS_ERROR", "harness error receipt mismatch")
            prerequisite_commands += r["status"] != "NOT_RUN"
        elif kind == "mutation":
            require(result in CONTROL, "invalid mutation result (prerequisite PASS is not a kill)")
            require(not ids & mutations, "duplicate mutation logical coverage")
            mutations |= ids
            if result == "NOT_RUN":
                require(r["status"] == "NOT_RUN", "NOT_RUN mutation execution mismatch")
            else:
                require(r["status"] != "NOT_RUN", "mutation was not executed")
                mutation_commands += 1
                require(unique_ids(e.get("mutated_control_ids"), "mutated controls") == ids,
                        "mapped control was not mutated")
                case = e.get("mutation_id")
                require(isinstance(case, str) and case and case not in cases, "duplicate/missing mutation case")
                cases.add(case)
                require(sha256(e.get("mutation_sha256")), "mutation definition hash missing")
                if result in {"KILLED", "SURVIVED"}:
                    b = executions.get(e.get("baseline_execution_id"))
                    require(b is not None and b["kind"] == "prerequisite" and b["result"] == "PASS"
                            and ids <= set(b["receipt"]["control_ids"]), "passing mapped baseline missing")
                    require(r["status"] == "EXITED" and e.get("infrastructure_error") is False,
                            "infrastructure failure cannot be a semantic outcome")
                    require(e.get("restoration") == {"source": True, "execution": True},
                            "source AND execution restoration required")
                    require(e.get("oracle_observed") is True, "semantic oracle evidence missing")
                    require((r["exit_code"] != 0) == (result == "KILLED"), "mutation exit/result mismatch")
                elif result == "HARNESS_ERROR":
                    require(e.get("infrastructure_error") is True, "harness error not identified")
            for control in ids:
                outcomes[control] = result
        else:
            raise InvalidEvidence("unknown execution kind")
        executions[eid] = e
    # Unmapped mutations remain NOT_RUN even if every baseline passed.
    for control in expected:
        outcomes.setdefault(control, "NOT_RUN")
    passed = set().union(*(set(e["receipt"]["control_ids"]) for e in executions.values()
                          if e["kind"] == "prerequisite" and e["result"] == "PASS"))
    counts = {s: sum(v == s for v in outcomes.values()) for s in sorted(CONTROL)}
    complete = passed == expected and counts["KILLED"] == len(expected)
    require(data["mission_status"] != "READY_FOR_REVIEW" or complete,
            "READY_FOR_REVIEW requires complete single-campaign kills and prerequisites")
    return {"prerequisite_commands": prerequisite_commands, "prerequisite_logical_pass": len(passed),
            "mutation_commands": mutation_commands, "control_results": counts,
            "complete_single_campaign": complete}


def tree_hash(path):
    entries = {}
    require(path.is_dir(), "isolation evidence directory missing")
    for p in sorted(path.rglob("*")):
        if p.is_symlink():
            entries[str(p.relative_to(path))] = {"symlink": os.readlink(p)}
        elif p.is_file():
            entries[str(p.relative_to(path))] = {"sha256": digest(p.read_bytes()), "size": p.stat().st_size}
    return {"files": len(entries), "sha256": digest(json.dumps(
        entries, sort_keys=True, separators=(",", ":")).encode()), "entries": entries}


def verify_isolation(root, before, mission_branch):
    refs = dict(line.split() for line in git(root, "for-each-ref", "--format=%(refname) %(objectname)").splitlines()
                if not line.startswith("refs/codex/"))
    refs.pop("refs/heads/" + mission_branch, None)
    require(refs == before["refs"], "pre-existing refs changed or an unauthorized ref appeared")
    for w in before["candidate10_worktrees"]:
        p = Path(w["path"])
        now = {"path": str(p), "head": git(p, "rev-parse", "HEAD"),
               "tree": git(p, "rev-parse", "HEAD^{tree}"),
               "status": git(p, "status", "--porcelain=v1", "--untracked-files=all"),
               "branch": git(p, "branch", "--show-current")}
        require(now == w, f"Candidate-10 worktree changed: {p}")
    old = before["candidate10_evidence"]
    require(tree_hash(Path(old["path"])) == {k: old[k] for k in ("files", "sha256", "entries")},
            "Candidate-10 evidence changed")
    return {"existing_refs_unchanged": len(refs), "candidate10_worktrees_unchanged": len(before["candidate10_worktrees"]),
            "candidate10_evidence_files_unchanged": old["files"]}


def verify_repository(root, mission_path, isolation=False):
    h = root / ".nexus/harness"
    c = json.loads(local_file(root, ".nexus/harness/contract.json").read_text())
    require(c["agents_max_bytes"] == AGENTS_BUDGET, "size budget changed")
    require(c["procedures"] == list(PROCEDURES), "procedure inventory mismatch")
    for key, allowed in [("prerequisite_statuses", PREREQUISITE), ("control_statuses", CONTROL),
                         ("mission_statuses", MISSION), ("review_verdicts", REVIEW)]:
        require(set(c[key]) == allowed, f"status domain mismatch: {key}")
    root_text = local_file(root, "AGENTS.md").read_text()
    require(len(root_text.encode()) < AGENTS_BUDGET, "AGENTS exceeds safe budget")
    require(".nexus/harness/POLICY.md" in root_text, "root startup omits policy")
    require("@AGENTS.md" in local_file(root, "CLAUDE.md").read_text(), "Claude import missing")
    for p in ("README.md", "POLICY.md", "RECEIPTS.md"):
        local_file(h, p)
    receipt_doc = (h / "RECEIPTS.md").read_text()
    for field in ("receipt_id", "mission_id", "timestamp_start", "timestamp_end", "cwd",
                  "candidate_sha", "candidate_tree", "command", "command_hash", "exit_code", "status",
                  "stdout_ref", "stderr_ref", "raw_log_hash", "environment", "acceptance_criterion",
                  "control_ids", "notes", "command_execution_id"):
        require(field in receipt_doc, f"receipt documentation omits {field}")
    local_file(root, ".nexus/missions/README.md")
    for kind in ("mission", "progress", "evidence", "review", "handover"):
        fields(local_file(h, kind.upper() + "_TEMPLATE.md"), c[kind + "_fields"])
        fields(local_file(mission_path, kind.upper() + ".md"), c[kind + "_fields"])
    mission = fields(mission_path / "MISSION.md", c["mission_fields"])
    require(oid(mission["Base SHA"]) and oid(mission["Base tree"]), "mission base identity missing")
    require(git(root, "rev-parse", mission["Base SHA"] + "^{tree}") == mission["Base tree"], "base tree mismatch")
    require(git(root, "branch", "--show-current") == mission["Mission branch"], "mission branch mismatch")
    require(Path(mission["Worktree"]).resolve() == root.resolve(), "mission worktree mismatch")
    subprocess.run(["git", "-C", str(root), "merge-base", "--is-ancestor", mission["Base SHA"], "HEAD"], check=True)
    for n in PROCEDURES:
        fields(local_file(h, "procedures/" + n + ".md"), c["procedure_fields"])
        bodies = []
        for provider in (".agents", ".claude"):
            p = local_file(root, f"{provider}/skills/nexus-{n}/SKILL.md")
            text = p.read_text()
            match = re.fullmatch(r"---\nname: (nexus-[a-z]+)\ndescription: ([^\n]+)\n---\n(.+)", text, re.S)
            require(match is not None and match[1] == "nexus-" + n, "invalid skill frontmatter/name")
            require(match[2].startswith("Use ") and len(text.encode()) < 1600, "skill routing/size invalid")
            links = re.findall(r"\]\(([^)]+)\)", match[3])
            require(links == [f"../../../.nexus/harness/procedures/{n}.md"], "adapter must route to one canonical procedure")
            require((p.parent / links[0]).resolve() == (h / "procedures" / (n + ".md")).resolve(), "broken procedure link")
            require(len(re.findall(r"(?m)^#", match[3])) == 1, "adapter duplicates procedural/policy sections")
            bodies.append(match[3])
        require(bodies[0] == bodies[1], "provider workflows diverged")
    # This preservation audit belongs to the refactor mission, not every future mission.
    if mission["Mission ID"] == "NEXUS-HARNESS-V1":
        # Verbatim original normative text + human preservation map, not keyword-only proof.
        original = git(root, "show", mission["Base SHA"] + ":AGENTS.md")
        policy = (h / "POLICY.md").read_text()
        for b in re.split(r"(?m)^#(?:#)? (?=\d+\. )", original)[1:]:
            n = int(b.split(".")[0])
            if n < 20:
                body = "\n".join(b.splitlines()[1:]).strip().removesuffix("---").strip()
                require(body in policy, f"original policy section {n} lost")
        evidence = (mission_path / "EVIDENCE.md").read_text()
        for n in range(1, 29):
            require(f"#original-{n}`" in evidence and f'id="original-{n}"' in policy,
                    f"preservation map missing section {n}")
    progress = fields(mission_path / "PROGRESS.md", c["progress_fields"])
    handover = fields(mission_path / "HANDOVER.md", c["handover_fields"])
    review = fields(mission_path / "REVIEW.md", c["review_fields"])
    require(progress["Mission"] == mission["Mission ID"] == handover["Mission"]
            == review["Mission ID"], "record mission mismatch")
    require(progress["Branch"] == handover["Exact branch"] == mission["Mission branch"], "record branch mismatch")
    require(progress["Worktree"] == handover["Worktree"] == mission["Worktree"], "record worktree mismatch")
    for record, sk, tk in ((progress, "Recorded candidate SHA", "Recorded candidate tree"),
                           (handover, "Exact SHA", "Exact tree")):
        require(oid(record[sk]) and oid(record[tk]), "checkpoint identity must be measured")
        require(git(root, "rev-parse", record[sk] + "^{tree}") == record[tk], "checkpoint tree mismatch")
        subprocess.run(["git", "-C", str(root), "merge-base", "--is-ancestor", record[sk], "HEAD"], check=True)
    require(handover["Current status"] in MISSION, "invalid handover status")
    require(review["Review verdict"] in REVIEW, "invalid independent review verdict")
    if review["Review verdict"] == "ACCEPTABLE_FOR_NEXT_GATE":
        require(oid(review["Exact candidate SHA"]) and oid(review["Exact candidate tree"]), "accepted review missing identity")
        require(git(root, "rev-parse", review["Exact candidate SHA"] + "^{tree}") == review["Exact candidate tree"],
                "review candidate tree mismatch")
        require(review["Tests independently reproduced"] != "NOT_RUN.", "independent reproduction not recorded")
    base = mission["Base SHA"]
    existing = git(root, "ls-tree", "-r", "--name-only", base, ".claude/skills").splitlines()
    for name in existing:
        require(local_file(root, name).read_bytes() == subprocess.check_output(
            ["git", "-C", str(root), "show", base + ":" + name]), "existing Claude skill changed")
    eroot = mission_path / "evidence"
    local_file(eroot, "logs/README.md")
    manifest = json.loads(local_file(eroot, "manifest.json").read_text())
    require(manifest["mission_id"] == mission["Mission ID"], "manifest mission mismatch")
    require(isinstance(manifest["artifacts"], dict) and manifest["artifacts"], "manifest artifacts empty")
    for name, meta in manifest["artifacts"].items():
        p = local_file(eroot, name)
        require(p.stat().st_size == meta["size"] and digest(p.read_bytes()) == meta["sha256"], f"artifact mismatch: {name}")
    receipt_ids = set()
    for line in local_file(eroot, "receipts.jsonl").read_text().splitlines():
        r = json.loads(line)
        validate_receipt(r, eroot, mission["Mission ID"])
        require(r["receipt_id"] not in receipt_ids, "duplicate receipt ID")
        receipt_ids.add(r["receipt_id"])
        for name in (r["stdout_ref"], r["stderr_ref"]):
            require(name is None or name in manifest["artifacts"], "receipt log absent from manifest")
    require(receipt_ids, "no execution receipts")
    require("receipts.jsonl" in manifest["artifacts"], "receipt stream absent from manifest")
    result = {"structure": "VALID", "skill_adapters": 14, "canonical_procedures": 7,
              "agents_bytes": len(root_text.encode()), "receipts_checked": len(receipt_ids),
              "semantic_policy_review": "REQUIRES_HUMAN_REVIEW", "isolation": "NOT_RUN"}
    if isolation:
        result["isolation"] = verify_isolation(root, json.loads(local_file(eroot, "isolation-before.json").read_text()),
                                               mission["Mission branch"])
    return result
