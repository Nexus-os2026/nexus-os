#!/usr/bin/env python3
"""P2-V1-R3B-I4-R1 source guards: the nine accepted P2-V1-R3B-I4 guards
(adapted only where I4-R1 changed what they pin: the two binding reads, the
closed constructor, the harness owner) and the I4-R1 guards, checked over
the candidate's production sources.

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
  p2_g_06 gating, with I4-R1's additions), replicated over the candidate.
- SG-I4R1-NORMAL-SURFACE: a normal build constructs a manager only by
  `ScopeManager::connect`; `connect_at`, `execution::place`, `ScopedHelper`
  and `PlacementFailed` are compiled only for the crate's tests and live
  harness; the pending operation is crate-private; no public direct start,
  settling or split failure exists.
- SG-I4R1-NO-RESUME-UNWIND: no production source resumes a panic.
- SG-I4R1-BINDING: Pending becomes Proven only after GetUnit, then the
  unit's `Id` and its `ControlGroup`, compared byte for byte (no
  canonicalization, suffix or last-component match), then the properties.
- SG-I4R1-UNCERTAIN: a start is accepted only in the arm of a delivered
  reply; without a candidate, the manager's absence is consulted only for
  an accepted start.
- SG-I4R1-IDENTITY: the helper's identity is allocated by a checked,
  non-wrapping counter, before the child is spawned, and exhaustion refuses
  the spawn.

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
    'const UNIT_INTERFACE: &str = "org.freedesktop.systemd1.Unit";',
    'const SCOPE_INTERFACE: &str = "org.freedesktop.systemd1.Scope";',
    'const PROPERTIES: &str = "org.freedesktop.DBus.Properties";',
]
TRAIT_SIGNATURES = [
    "fn start_scope(&self, request: &ScopeRequest<'_>) -> Started;",
    "fn stop_unit(&self, unit: &str) -> Remote<()>;",
    "fn get_unit(&self, unit: &str) -> Remote<Presence>;",
    "fn runtime_max_usec(&self, unit_path: &str) -> Remote<Option<u64>>;",
    "fn oom_policy(&self, unit_path: &str) -> Remote<Option<String>>;",
    "fn unit_id(&self, unit_path: &str) -> Remote<Option<String>>;",
    "fn control_group(&self, unit_path: &str) -> Remote<Option<String>>;",
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
    if len(calls) != 7:
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
    # I4-R1 (p2_g_06's additions).
    scope, pending = files["scope.rs"], files["scope/pending.rs"]
    if not gated(execution, "place", HARNESS_ONLY):
        problems.append("place is not gated")
    if not gated(scope, "connect_at", HARNESS_ONLY) or gated(scope, "connect", HARNESS_ONLY):
        problems.append("the manager constructors are not as pinned")
    for owner in ["#[derive(Debug)]\npub struct ScopedHelper {",
                  "#[derive(Debug)]\npub struct PlacementFailed {",
                  "impl ScopedHelper {", "impl Drop for ScopedHelper {"]:
        if execution.count(owner) != 1 or f"{HARNESS_ONLY}\n{owner}" not in execution:
            problems.append(f"not gated: {owner[:40]!r}")
    if "pub fn start(" in code(scope) or "pub(crate) struct PendingScope" not in code(pending):
        problems.append("a public direct start or pending operation")
    for text in (scope, pending, execution):
        if "StartFailed" in code(text) or "pub fn settle(" in code(text):
            problems.append("a split failure or public settling")
    return problems


ITEM_KINDS = ("fn ", "struct ", "enum ", "trait ", "use ", "mod ", "const ", "static ", "type ")


def normal_public(source):
    """The `pub` items a build without the test or harness configuration
    declares (outside every item or block gated by them)."""
    names, gated_next, skip_until = [], False, None
    for line in source.splitlines():
        if skip_until is not None:
            if line == skip_until:
                skip_until = None
            continue
        item = line.lstrip()
        indent = line[:len(line) - len(item)]
        if item.startswith("#["):
            gated_next |= ("cfg(test)" in item
                           or 'cfg(any(test, feature = "live-sandbox-harness"))' in item)
            continue
        if item.startswith("//") or not item:
            continue
        if gated_next:
            gated_next = False
            if item.endswith("{"):
                skip_until = indent + "}"
            continue
        if item.startswith("pub "):
            rest = item[4:]
            for kind in ITEM_KINDS:
                if rest.startswith(kind):
                    declared = rest[len(kind):]
                    name = declared.rstrip(";") if kind == "use " else re.match(r"\w*", declared).group(0)
                    names.append(kind + name)
    return names


NORMAL_SCOPE = ["use manager::BUS_CALL_TIMEOUT", "const PLACEMENT_TIMEOUT", "const SETTLE_TIMEOUT",
                "enum ScopeError", "fn expected_limit_files", "struct ScopeManager", "fn connect",
                "struct ScopeEvents", "enum Occupancy", "struct Scope", "fn unit", "fn kill",
                "fn occupancy", "fn wait_empty", "fn events"]
NORMAL_EXECUTION = ["const FINALIZE_TIMEOUT", "struct StreamRecord", "enum EndedBy", "enum NotRun",
                    "enum Cleanup", "fn is_confirmed", "struct RetainedBoundary", "fn retry",
                    "struct ExecutionReport", "enum ExitClass", "fn classify", "fn run"]


def guard_normal_surface(files):
    problems = []
    if normal_public(files["scope.rs"]) != NORMAL_SCOPE:
        problems.append(f"scope.rs: {normal_public(files['scope.rs'])}")
    if normal_public(files["execution.rs"]) != NORMAL_EXECUTION:
        problems.append(f"execution.rs: {normal_public(files['execution.rs'])}")
    for name in ("scope/pending.rs", "scope/native.rs"):
        if normal_public(files[name]):
            problems.append(f"{name}: {normal_public(files[name])}")
    if normal_public(files["scope/manager.rs"]) != ["const BUS_CALL_TIMEOUT"]:
        problems.append(f"scope/manager.rs: {normal_public(files['scope/manager.rs'])}")
    scope = code(files["scope.rs"])
    if "    fn connect_to(path: &str) -> Result<Self, ScopeError> {" not in scope:
        problems.append("the bus constructor is not private")
    if 'Self::connect_to(&format!("/run/user/{uid}/bus"))' not in scope:
        problems.append("connect does not derive the bus from the real uid")
    if code(files["scope.rs"]).count("pub(crate) use pending::{PendingScope, ScopeBoundary};") != 1:
        problems.append("the pending operation is re-exported beyond the crate")
    return problems


def guard_no_resume_unwind(files):
    hits = count(files, "resume_unwind")
    return [f"resume_unwind: {hits}"] if hits else []


def guard_binding(files):
    problems = []
    pending = code(files["scope/pending.rs"])
    prove = pending[pending.find("    fn prove("):]
    prove = prove[:prove.find("\n    }\n") + 6]
    order = ["controller.native.open(&path)?", "controller.manager.get_unit(&self.unit)",
             "controller.manager.unit_id(&unit_path)", "controller.manager.control_group(&unit_path)",
             "controller.manager.runtime_max_usec(&unit_path)", "controller.manager.oom_policy(&unit_path)"]
    at = [prove.find(step) for step in order]
    if -1 in at or at != sorted(at):
        problems.append(f"the proof's order: {dict(zip(order, at))}")
    for needle in ["Remote::Answered(Some(id)) if id == self.unit => {}",
                   "Remote::Answered(Some(group)) if group == path => {}",
                   'Remote::Answered(_) => return Err(ScopeError::Mismatch("unit id")),',
                   'Remote::Answered(_) => return Err(ScopeError::Mismatch("unit control group")),']:
        if prove.count(needle) != 1:
            problems.append(f"not exactly once in the proof: {needle}")
    if prove.count("Remote::Uncertain(reason) => return Err(ScopeError::Bus(reason)),") != 5:
        problems.append("an uncertain answer is not refused in every proof step")
    binding = prove[prove.find("controller.manager.unit_id(&unit_path)"):
                    prove.find("fault::at(fault, FaultPoint::ScopeProperties);")]
    for needle in ["canonicalize", "ends_with", "starts_with", "trim", "to_lowercase",
                   "eq_ignore", "rsplit", "split", "strip_", "contains", "Path::new", "PathBuf"]:
        if needle in binding:
            problems.append(f"the binding uses {needle}")
    return problems


def guard_uncertain(files):
    problems = []
    pending = code(files["scope/pending.rs"])
    if pending.count("self.accepted = true;") != 1 or not re.search(
            r"Started::Accepted => \{\s*self\.accepted = true;", pending):
        problems.append("a start is accepted other than in the delivered reply's arm")
    gone = pending[pending.find("    fn gone("):]
    gone = gone[:gone.find("\n    }\n") + 6]
    if not re.search(r"None if !self\.accepted => false,\s*None => absent\(", gone):
        problems.append("the manager's absence is consulted without an accepted start")
    if pending.count("absent(") != 2:  # its definition and its one use, in gone()
        problems.append(f"absent( occurs {pending.count('absent(')} times")
    if "fault::at(fault, FaultPoint::AfterScopeStart);" not in pending or \
            pending.find("fault::at(fault, FaultPoint::AfterScopeStart);") > pending.find("self.accepted = true;"):
        problems.append("the reply is recorded before the post-dispatch fault point")
    return problems


def guard_identity(files):
    problems = []
    launcher = code(files["launcher.rs"])
    if "fetch_add" in launcher:
        problems.append("a wrapping counter")
    if "serial.checked_add(1).filter(|_| serial != 0)" not in launcher:
        problems.append("the allocation is not checked")
    spawn_with = launcher[launcher.find("pub(crate) fn spawn_with("):]
    allocated = spawn_with.find("let serial = allocate_serial(serials).ok_or(LaunchError::IdentitiesExhausted)?;")
    spawned = spawn_with.find(".spawn()")
    if allocated < 0 or spawned < 0 or allocated > spawned:
        problems.append("the child is spawned before its identity")
    if launcher.count("Self::spawn_with(program, &NEXT_SERIAL)") != 1:
        problems.append("spawn does not use the process counter")
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
    ("SG-I4R1-NORMAL-SURFACE", guard_normal_surface),
    ("SG-I4R1-NO-RESUME-UNWIND", guard_no_resume_unwind),
    ("SG-I4R1-BINDING", guard_binding),
    ("SG-I4R1-UNCERTAIN", guard_uncertain),
    ("SG-I4R1-IDENTITY", guard_identity),
]


def inject(files, name, old, new):
    copy = dict(files)
    if copy[name].count(old) < 1:
        raise SystemExit(f"self-test anchor missing in {name}: {old[:60]!r}")
    copy[name] = copy[name].replace(old, new, 1)
    return copy


# One injected violation per guard (and a few more): each must be reported.
SELF_TESTS = [
    ("SG-I4-BUS", "scope.rs", "        let uid = unsafe { libc::getuid() };\n        Self::connect_to(",
     "        let uid = unsafe { libc::getuid() };\n        let _ = std::env::var(\"DBUS_SESSION_BUS_ADDRESS\");\n        Self::connect_to("),
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
    ("SG-I4-DESKTOP-REPLICA", "execution.rs",
     '#[cfg(any(test, feature = "live-sandbox-harness"))]\npub fn place(', "pub fn place("),
    ("SG-I4R1-NORMAL-SURFACE", "scope.rs",
     '    #[cfg(any(test, feature = "live-sandbox-harness"))]\n    pub fn connect_at(',
     "    pub fn connect_at("),
    ("SG-I4R1-NORMAL-SURFACE", "scope/pending.rs", "pub(crate) struct PendingScope {",
     "pub struct PendingScope {"),
    ("SG-I4R1-NORMAL-SURFACE", "execution.rs",
     '#[cfg(any(test, feature = "live-sandbox-harness"))]\n#[derive(Debug)]\npub struct ScopedHelper {',
     "#[derive(Debug)]\npub struct ScopedHelper {"),
    ("SG-I4R1-NO-RESUME-UNWIND", "execution.rs", "        Err(_) => None,\n    };",
     "        Err(panic) => std::panic::resume_unwind(panic),\n    };"),
    ("SG-I4R1-BINDING", "scope/pending.rs",
     "Remote::Answered(Some(group)) if group == path => {}",
     "Remote::Answered(Some(group)) if group.ends_with(&self.unit) => {}"),
    ("SG-I4R1-BINDING", "scope/pending.rs",
     "Remote::Answered(Some(id)) if id == self.unit => {}",
     "Remote::Answered(Some(_)) => {}"),
    ("SG-I4R1-UNCERTAIN", "scope/pending.rs", "            None if !self.accepted => false,\n", ""),
    ("SG-I4R1-UNCERTAIN", "scope/pending.rs",
     "        self.issued = true;\n", "        self.issued = true;\n        self.accepted = true;\n"),
    ("SG-I4R1-IDENTITY", "launcher.rs", "serial.checked_add(1).filter(|_| serial != 0)",
     "Some(serial.wrapping_add(1))"),
    ("SG-I4R1-IDENTITY", "launcher.rs",
     "        let serial = allocate_serial(serials).ok_or(LaunchError::IdentitiesExhausted)?;\n", ""),
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
