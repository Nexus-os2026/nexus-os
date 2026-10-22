#!/usr/bin/env python3
"""P2-V1-R3B-I4 source guards: production/test separation and the scope
module's fixed authorities, checked over the candidate's production sources.

Production files are the sandbox crate's `src/**/*.rs` except `tests`
directories and files whose name ends in `tests.rs` (exactly the desktop
guard's `production_files` rule). Line comments are stripped before
matching (documentation may name what is forbidden), as the desktop guard
does.

Guards:

- SG-I4-BUS: the bus address is derived from the real uid
  (`/run/user/<getuid()>/bus`), must be a socket owned by that uid, and is
  the only address a connection is built from; no environment variable, no
  session or system bus constructor.
- SG-I4-DESTINATION: the destination, object path and interfaces are
  constants; the one `call_method` passes them; callers of the internal
  `call` name only those constants or the object path GetUnit returned; the
  manager interface takes unit names and object paths only.
- SG-I4-NO-SWEEP: no unit enumeration or lookup by process or cgroup, no
  name pattern; the unit-name format appears once (the nonce).
- SG-I4-NO-PID-SIGNAL: the scope and execution sources never signal, wait
  for or open a process by id; processes are ended through the retained
  `Helper` and `cgroup.kill` only.
- SG-I4-TEST-SEAMS: the simulation is compiled only for unit tests; a
  normal build has exactly one manager (zbus) and one kernel view, and
  constructs a controller only from them.
- SG-I4-NO-SERIALIZE: no owner of the scope module or of the execution
  derives or implements Serialize or Deserialize.
- SG-I4-NO-BACKGROUND: no detached cleanup: the scope module spawns no
  thread or task; the execution spawns only its output threads, as before.
- SG-I4-ONE-WAY: a pending operation is constructed only by
  `ScopeManager::prepare`, and a proven scope only by its promotion.
- SG-I4-DESKTOP-REPLICA: the desktop's sandbox-source guards (p2_g_02 pins,
  p2_g_06 gating), replicated over the candidate (the desktop crate itself
  is not built here).

Each guard has a self-test: a violation injected into an in-memory copy of
the sources, which the guard must report.

Usage: source_guards.py <checkout> <output json>
"""
import json
import pathlib
import re
import sys

CRATE = "crates/nexus-verifier-sandbox"
SRC = f"{CRATE}/src"
SCOPE_FILES = ["scope.rs", "scope/manager.rs", "scope/native.rs", "scope/pending.rs"]
HARNESS_ONLY = '#[cfg(any(test, feature = "live-sandbox-harness"))]'


def code(text):
    return "\n".join(line[:line.find("//")] if "//" in line else line
                     for line in text.splitlines())


def production_files(root):
    out = {}
    base = root / SRC
    for path in sorted(base.rglob("*.rs")):
        rel = path.relative_to(base)
        if "tests" in rel.parts[:-1]:
            continue
        if path.name.endswith("tests.rs"):
            continue
        out[str(rel)] = path.read_text()
    return out


def count(files, needle, only=None):
    found = {}
    for name, text in files.items():
        if only is not None and name not in only:
            continue
        n = code(text).count(needle)
        if n:
            found[name] = n
    return found


def guard_bus(files):
    problems = []
    for needle in ["std::env", "env::var", "var_os", "DBUS_SESSION_BUS_ADDRESS",
                   "DBUS_SYSTEM_BUS_ADDRESS", "XDG_RUNTIME_DIR", "Builder::session",
                   "Builder::system", "Connection::session", "Connection::system"]:
        # The scope module (elsewhere, the desktop replica's pins apply).
        hits = count(files, needle, only=set(SCOPE_FILES))
        if hits:
            problems.append(f"{needle}: {hits}")
    scope = code(files["scope.rs"])
    if scope.count('format!("/run/user/{uid}/bus")') != 1 or "let uid = unsafe { libc::getuid() };" not in scope:
        problems.append("the bus path is not derived from the real uid exactly once")
    manager = code(files["scope/manager.rs"])
    for needle in ["!metadata.file_type().is_socket() || metadata.uid() != uid",
                   'let address = format!("unix:path={path}");',
                   "zbus::connection::Builder::address(address.as_str())?"]:
        if manager.count(needle) != 1:
            problems.append(f"manager.rs: not exactly once: {needle}")
    if count(files, "connection::Builder::") != {"scope/manager.rs": 1}:
        problems.append(f"connection builders: {count(files, 'connection::Builder::')}")
    return problems


DESTINATION_CONSTANTS = [
    'const SYSTEMD: &str = "org.freedesktop.systemd1";',
    'const SYSTEMD_PATH: &str = "/org/freedesktop/systemd1";',
    'const MANAGER: &str = "org.freedesktop.systemd1.Manager";',
    'const SCOPE_INTERFACE: &str = "org.freedesktop.systemd1.Scope";',
    'const PROPERTIES: &str = "org.freedesktop.DBus.Properties";',
]
TRAIT_SIGNATURES = [
    "fn start_scope(&self, request: &ScopeRequest<'_>) -> Started;",
    "fn stop_unit(&self, unit: &str) -> Remote<()>;",
    "fn get_unit(&self, unit: &str) -> Remote<Presence>;",
    "fn runtime_max_usec(&self, unit_path: &str) -> Remote<Option<u64>>;",
    "fn oom_policy(&self, unit_path: &str) -> Remote<Option<String>>;",
]


def guard_destination(files):
    problems = []
    manager = code(files["scope/manager.rs"])
    for constant in DESTINATION_CONSTANTS:
        if manager.count(constant) != 1:
            problems.append(f"constant: {constant}")
    if count(files, "call_method(") != {"scope/manager.rs": 1}:
        problems.append(f"call_method: {count(files, 'call_method(')}")
    if ".call_method(Some(SYSTEMD),path,Some(interface),method,body)" not in re.sub(r"\s+", "", manager):
        problems.append("call_method does not pass the fixed destination")
    if re.search(r"pub(\([^)]*\))?\s+fn call<", manager):
        problems.append("the internal call is not private")
    calls = re.findall(r"self\.call::<[^>]*(?:<[^>]*>)?[^>]*>\(\s*([^,]+),\s*([^,]+),", manager)
    if len(calls) != 5:
        problems.append(f"internal calls: {len(calls)}")
    for path, interface in calls:
        pair = (path.strip(), interface.strip())
        if pair not in {("SYSTEMD_PATH", "MANAGER"), ("unit_path", "PROPERTIES")}:
            problems.append(f"a call to {pair}")
    block = manager[manager.find("pub(crate) trait Manager"):]
    block = block[:block.find("\n}") + 2]
    signatures = sorted(re.sub(r"\s+", " ", s).strip() + ";"
                        for s in re.findall(r"(fn [^;]+);", block))
    if signatures != sorted(TRAIT_SIGNATURES):
        problems.append(f"manager interface: {signatures}")
    return problems


def guard_no_sweep(files):
    problems = []
    for needle in ["ListUnits", "SubscribeUnits", "GetUnitByPID", "GetUnitByControlGroup",
                   "GetUnitByInvocationID", "GetUnitProcesses", "nexus-verifier-*",
                   "nexus-verifier-.*", "read_dir("]:
        hits = count(files, needle, only=set(SCOPE_FILES) | {"execution.rs"})
        if hits:
            problems.append(f"{needle}: {hits}")
    names = {name: len(re.findall(r"nexus-verifier-[^\"\s]*scope", code(text)))
             for name, text in files.items() if name in set(SCOPE_FILES) | {"execution.rs"}}
    names = {name: n for name, n in names.items() if n}
    if names != {"scope.rs": 1}:
        problems.append(f"unit-name strings: {names}")
    if 'format!("nexus-verifier-{}.scope", random_hex()?)' not in code(files["scope.rs"]):
        problems.append("the unit name is not the backend nonce")
    return problems


def guard_no_pid_signal(files):
    problems = []
    for needle in ["libc::kill", "libc::waitpid", "libc::waitid", "pidfd_send_signal",
                   "pidfd_open", "libc::tgkill", "Command::new", "/proc/{}/stat",
                   "/proc/{pid}"]:
        hits = count(files, needle, only=set(SCOPE_FILES) | {"execution.rs"})
        if hits:
            problems.append(f"{needle}: {hits}")
    return problems


def guard_test_seams(files):
    problems = []
    scope = files["scope.rs"]
    if scope.count("#[cfg(test)]\npub(crate) mod tests;") != 1 or code(scope).count("mod tests;") != 1:
        problems.append("the simulation module is not test-only")
    if scope.count("    #[cfg(test)]\n    pub(crate) fn with_controller(") != 1:
        problems.append("with_controller is not test-only and crate-private")
    for name, text in files.items():
        for needle in ["FakeManager", "FakeNative", "FakeDir", "scope::tests", "Script::", "World::"]:
            if needle in code(text):
                problems.append(f"{name}: {needle}")
    impls = {kind: count(files, f"impl {kind} for ") for kind in ("Manager", "Native", "CgroupDir")}
    if impls != {"Manager": {"scope/manager.rs": 1}, "Native": {"scope/native.rs": 1},
                 "CgroupDir": {"scope/native.rs": 1}}:
        problems.append(f"implementations: {impls}")
    for needle in ["impl Manager for ZbusManager", "impl Native for Kernel", "impl CgroupDir for KernelDir"]:
        if needle not in code(files["scope/manager.rs"] + files["scope/native.rs"]):
            problems.append(f"missing: {needle}")
    built = count(files, "Arc::new(Controller {")
    if built != {"scope.rs": 1}:
        problems.append(f"controllers built: {built}")
    connect = code(scope)
    for needle in ["manager: Box::new(ZbusManager::connect_at(path)?),",
                   "native: Box::new(Kernel),", "timing: Timing::PRODUCTION,"]:
        if connect.count(needle) != 1:
            problems.append(f"the production controller: {needle}")
    return problems


def guard_no_serialize(files):
    problems = []
    pattern = re.compile(r"derive\([^)]*\b(Serialize|Deserialize)\b|impl(<[^>]*>)?\s+(serde::)?(ser::)?(de::)?(Serialize|Deserialize)")
    for name in SCOPE_FILES + ["execution.rs"]:
        if pattern.search(code(files[name])):
            problems.append(name)
    return problems


def guard_no_background(files):
    problems = []
    for needle in ["thread::spawn", "thread::Builder", "tokio::spawn", "spawn_blocking",
                   "thread::scope", "task::spawn"]:
        hits = count(files, needle, only=set(SCOPE_FILES))
        if hits:
            problems.append(f"scope: {needle}: {hits}")
    spawned = count(files, "std::thread::Builder::new()", only={"execution.rs"})
    other = {n: count(files, n, only={"execution.rs"})
             for n in ("thread::spawn", "tokio::spawn", "spawn_blocking")}
    if spawned != {"execution.rs": 1} or any(other.values()):
        problems.append(f"execution threads: {spawned} {other}")
    return problems


def guard_one_way(files):
    problems = []
    if count(files, "PendingScope::new(") != {"scope.rs": 1}:
        problems.append(f"PendingScope::new: {count(files, 'PendingScope::new(')}")
    if count(files, "Scope { unit, dir }") != {"scope/pending.rs": 1}:
        problems.append(f"Scope literals: {count(files, 'Scope { unit, dir }')}")
    if count(files, "Self::Proven(Scope { unit, dir })") != {"scope/pending.rs": 1}:
        problems.append("a proven scope is built other than by promotion")
    if re.search(r"pub\s+fn\s+\w+\([^)]*unit:\s*&?str", code(files["scope.rs"] + files["scope/pending.rs"])):
        problems.append("a public function takes a unit name")
    return problems


DESKTOP_PINS = [
    ("Command::new(", {"launcher.rs": 1}),
    ("pre_exec", {}),
    ("CommandExt", {}),
    ("sys::execveat_fd(", {"helper.rs": 1}),
    ("libc::fork(", {"sys.rs": 1}),
    ("sys::unshare(", {"helper.rs": 1}),
    ("libc::socket(", {"sys.rs": 1}),
    ("current_exe()", {"launcher.rs": 1, "toolchain.rs": 1}),
    ("std::env::var", {}),
    ("env::var_os", {}),
    ("XDG_RUNTIME_DIR", {}),
]


def gated(source, name, attribute):
    at = source.find(f"pub fn {name}(")
    if at < 0:
        return False
    before = source[:at]
    start = before.rfind(attribute)
    if start < 0:
        return False
    return all(not line.strip() or line.strip().startswith("///")
               for line in before[start + len(attribute):].splitlines())


def guard_desktop_replica(files):
    problems = []
    if len(files) < 15:
        problems.append(f"only {len(files)} production files")
    by_base = {}
    for name, text in files.items():
        base = pathlib.PurePosixPath(name).name
        by_base.setdefault(base, "")
        by_base[base] += "\n" + text
    for needle, expected in DESKTOP_PINS:
        found = {}
        for name, text in files.items():
            n = code(text).count(needle)
            if n:
                base = pathlib.PurePosixPath(name).name
                found[base] = found.get(base, 0) + n
        if found != expected:
            problems.append(f"pin {needle}: {found} != {expected}")
    launcher, execution = files["launcher.rs"], files["execution.rs"]
    if not gated(launcher, "at", HARNESS_ONLY) or not gated(launcher, "in_extracted_package", HARNESS_ONLY):
        problems.append("a harness helper constructor is not gated")
    if gated(launcher, "installed", HARNESS_ONLY):
        problems.append("installed is gated")
    if code(launcher).count("Self { path") != 2:
        problems.append("helper constructors")
    for function in ["run_with_fault", "holds_scope", "holds_helper"]:
        if not gated(execution, function, HARNESS_ONLY):
            problems.append(f"{function} is not gated")
    if any("HelperProgram { path" in code(text) for text in files.values()):
        problems.append("a helper is built from a path")
    return problems


GUARDS = [
    ("SG-I4-BUS", guard_bus),
    ("SG-I4-DESTINATION", guard_destination),
    ("SG-I4-NO-SWEEP", guard_no_sweep),
    ("SG-I4-NO-PID-SIGNAL", guard_no_pid_signal),
    ("SG-I4-TEST-SEAMS", guard_test_seams),
    ("SG-I4-NO-SERIALIZE", guard_no_serialize),
    ("SG-I4-NO-BACKGROUND", guard_no_background),
    ("SG-I4-ONE-WAY", guard_one_way),
    ("SG-I4-DESKTOP-REPLICA", guard_desktop_replica),
]


def inject(files, name, old, new):
    copy = dict(files)
    if copy[name].count(old) < 1:
        raise SystemExit(f"self-test anchor missing in {name}: {old[:60]!r}")
    copy[name] = copy[name].replace(old, new, 1)
    return copy


# One injected violation per guard (and a few more): each must be reported.
SELF_TESTS = [
    ("SG-I4-BUS", "scope.rs", "        let uid = unsafe { libc::getuid() };\n        Self::connect_at(",
     "        let uid = unsafe { libc::getuid() };\n        let _ = std::env::var(\"DBUS_SESSION_BUS_ADDRESS\");\n        Self::connect_at("),
    ("SG-I4-BUS", "scope/manager.rs", "!metadata.file_type().is_socket() || metadata.uid() != uid",
     "!metadata.file_type().is_socket()"),
    ("SG-I4-DESTINATION", "scope/manager.rs", 'const SYSTEMD: &str = "org.freedesktop.systemd1";',
     'const SYSTEMD: &str = "org.example.other";'),
    ("SG-I4-DESTINATION", "scope/manager.rs", "    fn stop_unit(&self, unit: &str) -> Remote<()>;",
     "    fn stop_unit(&self, destination: &str, unit: &str) -> Remote<()>;"),
    ("SG-I4-NO-SWEEP", "scope/pending.rs", "fn names_unit(",
     "fn sweep() { let _ = \"ListUnitsByPatterns\"; }\nfn names_unit("),
    ("SG-I4-NO-SWEEP", "scope/pending.rs", "fn names_unit(",
     "const PATTERN: &str = \"nexus-verifier-*.scope\";\nfn names_unit("),
    ("SG-I4-NO-PID-SIGNAL", "execution.rs", "fn end_now(",
     "fn signal(pid: i32) { unsafe { libc::kill(pid, libc::SIGKILL) }; }\nfn end_now("),
    ("SG-I4-TEST-SEAMS", "scope.rs", "    #[cfg(test)]\n    pub(crate) fn with_controller(",
     "    pub(crate) fn with_controller("),
    ("SG-I4-TEST-SEAMS", "scope.rs", "#[cfg(test)]\npub(crate) mod tests;", "pub(crate) mod tests;"),
    ("SG-I4-NO-SERIALIZE", "scope/pending.rs", "/// What tests observe about a pending scope operation.",
     "#[derive(serde::Serialize)]\nstruct Saved;\n/// What tests observe about a pending scope operation."),
    ("SG-I4-NO-BACKGROUND", "scope/pending.rs", "fn names_unit(",
     "fn background() { std::thread::spawn(|| {}); }\nfn names_unit("),
    ("SG-I4-ONE-WAY", "scope/pending.rs", "fn names_unit(",
     "pub fn from_name(unit: &str) {}\nfn names_unit("),
    ("SG-I4-DESKTOP-REPLICA", "execution.rs",
     '    #[cfg(any(test, feature = "live-sandbox-harness"))]\n    pub fn holds_scope(',
     "    pub fn holds_scope("),
]


def main():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    root = pathlib.Path(sys.argv[1]).resolve()
    files = production_files(root)
    results = {gid: guard(files) for gid, guard in GUARDS}
    self_tests = []
    for gid, name, old, new in SELF_TESTS:
        guard = dict(GUARDS)[gid]
        reported = guard(inject(files, name, old, new))
        self_tests.append(dict(guard=gid, file=name, injected=new[:120],
                               reported=reported, detected=bool(reported)))
    summary = dict(
        production_files=sorted(files),
        guards={gid: dict(ok=not problems, problems=problems) for gid, problems in results.items()},
        all_guards_pass=all(not p for p in results.values()),
        self_tests=self_tests,
        all_self_tests_detected=all(t["detected"] for t in self_tests),
        guards_with_self_test=sorted({t["guard"] for t in self_tests}),
    )
    pathlib.Path(sys.argv[2]).write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(dict(all_guards_pass=summary["all_guards_pass"],
                          all_self_tests_detected=summary["all_self_tests_detected"],
                          failing=[g for g, r in summary["guards"].items() if not r["ok"]],
                          undetected=[t["guard"] for t in self_tests if not t["detected"]])))
    every_guard_tested = set(summary["guards_with_self_test"]) == {g for g, _ in GUARDS}
    return 0 if summary["all_guards_pass"] and summary["all_self_tests_detected"] and every_guard_tested else 1


if __name__ == "__main__":
    sys.exit(main())
