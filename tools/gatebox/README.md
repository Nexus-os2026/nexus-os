# Gate Box Slice 1: offline, scoped Linux qualification

UNVERIFIED until Owner installation and all installed oracles pass. This PR
provides no installed PASS, CI parity, or permission to advance a phase.
Requires the frozen x86-64 Linux host, systemd 259+, Chrome, Python, Xvfb,
xdotool, D-Bus tools, the installed Rust 1.94.0 toolchain and populated registry.
There are no downloads or fallback dependency sources.

The reviewed uncompressed `gatebox-slice1.tar` contains the installer FILES
allowlist under `tools/gatebox/` plus exactly one Q6 data member:
`crates/nexus-verifier-sandbox/tests/phase2_live_sandbox.rs`. That file comes
from the reviewed source tree; it is not duplicated in this PR. Installation
copies its verified bytes to
`/usr/libexec/gatebox/policy/phase2_live_sandbox.rs` and checks blob
`6cf914ba5ae1eeffc1b61376ba6b768be8a0ffdb`. **Any harness edit fails closed
until the pinned policy is re-pinned through a reviewed PR.** The excluded
executable is compiled but never spawned, including for discovery.

Owner order (Q16, each step requires the preceding evidence):

1. Fix M-a and the Q16 pre-install findings; collect offline oracles and plants.
2. Obtain independent delta review of the final full commit and procedures.
3. Complete H1 below, including reboot and both group checks.
4. PIN below: verify objects, build from the reviewed commit and record bundle SHA-256.
5. Install the reviewed bundle using OWNER below, verify checks, then **STOP**.
6. After review lifts STOP, run the protected host plant below; preserve evidence and reboot.
7. Qualify with C2 last, saving all output to the root-owned log below.
8. Reboot after qualification succeeds and evidence is saved.
9. Run enable-qualified below; do not repeat qualification between C2 and enable.

Owner H1 steps, **before installation**, in a host terminal. Close every agent,
background task and sub-agent, including this session. This check must print
nothing; do not proceed while any matching process remains:

```bash
pgrep -u nexus -af 'claude|codex'
# Empty result required. Then, with all agents closed:
sudo gpasswd -d nexus docker
sudo reboot
```

After reboot, run both checks; both outputs must omit `docker`:

```bash
id -nG
id -nG nexus
```

Docker is
subsequently available only through `sudo docker`. Do not restart agents during
the Owner sudo session. In a second host terminal keep `sudo -i` open as lockout
insurance until the post-install checks pass.

Re-pin install-review-2 C1 to the **final independently reviewed full commit SHA
and its worktree**, not d5ef01d7 or mother-gbs2b. After H1 reboot, with agents
closed, build and compare every bundle member to that commit before the OWNER
block. The PIN block refuses replacement refs and corrupt/missing objects before
building. It reads committed blobs, not worktree bytes. Move any previous bundle
aside for review first; exclusive creation refuses to overwrite it. From the
final reviewed repository root, as nexus (not root):

```bash
read -r -p 'Final reviewed full commit SHA: ' GBX_REVIEWED_SHA
python3 -I -B - "$GBX_REVIEWED_SHA" <<'PIN'
import ast, hashlib, pathlib, re, shlex, subprocess, sys, tarfile, tempfile
sha=sys.argv[1]
if not re.fullmatch('[0-9a-f]{40}',sha): raise SystemExit('full reviewed SHA required')
replacements=subprocess.check_output(['git','--no-replace-objects','for-each-ref','--format=%(refname)','refs/replace'])
if replacements.strip(): raise SystemExit('replacement refs present; stop for review')
subprocess.run(['git','--no-replace-objects','fsck','--full','--strict',sha],check=True)
def blob(path): return subprocess.check_output(['git','--no-replace-objects','show',sha+':'+path])
names=subprocess.check_output(['git','--no-replace-objects','ls-tree','-r','--name-only',sha,'--','tools/gatebox']).decode().splitlines()
body=blob('tools/gatebox/install.sh').decode().split("<<'PY'\n",1)[1].rsplit('\nPY',1)[0]
values={n.targets[0].id:ast.literal_eval(n.value) for n in ast.parse(body).body
        if isinstance(n,ast.Assign) and isinstance(n.targets[0],ast.Name)
        and n.targets[0].id in ('FILES','NOT_INSTALLED','HARNESS','WRAPPER','WRAPPER_SHA256')}
files,excluded=values['FILES'],values['NOT_INSTALLED']
if (len(values['FILES'])!=38 or len(excluded)!=12 or len(set(files))!=len(files) or len(set(excluded))!=len(excluded)
        or set(files)&set(excluded) or set(names)!={'tools/gatebox/'+n for n in files+excluded}):
    raise SystemExit('reviewed FILES/NOT_INSTALLED partition mismatch')
names=['tools/gatebox/'+n for n in files]+[values['HARNESS']]
wrapper='tools/gatebox/'+values['WRAPPER']
bundle=pathlib.Path('/home/nexus/NEXUS/gatebox-slice1.tar')
with tempfile.TemporaryDirectory(prefix='gatebox-pin-') as temporary:
    for name in names:
        path=pathlib.Path(temporary)/name; path.parent.mkdir(parents=True,exist_ok=True)
        path.write_bytes(blob(name))
    # SB3E-R18 P12: the wrapper is built here, unprivileged, from the verified blob; root only checks the literal.
    subprocess.run(['/usr/bin/cc','-std=c11','-O2','-Wall','-Wextra','-Werror','-static','-o',wrapper,
                    'tools/gatebox/route/route_rustc.c'],cwd=temporary,env={'PATH':'/usr/bin:/bin','LANG':'C'},check=True)
    built=(pathlib.Path(temporary)/wrapper).read_bytes()
    if hashlib.sha256(built).hexdigest()!=values['WRAPPER_SHA256']: raise SystemExit('wrapper build is not the reviewed literal')
    names+=[wrapper]
    # Keep the original explicit-member, no-recursion tar contract. Its source
    # is this private tree of verified blobs, never the mutable checkout.
    command=shlex.split("tar --exclude='__pycache__' --no-recursion --null --files-from=- -cf -")
    with bundle.open('xb') as output:
        subprocess.run(command,cwd=temporary,input=b''.join(n.encode()+b'\0' for n in names),
                       stdout=output,check=True)
with tarfile.open(bundle,'r:') as archive:
    members=[m for m in archive if not m.isdir()]
    if len(members)!=len(names) or {m.name for m in members}!=set(names):
        raise SystemExit('bundle members differ from final reviewed commit')
    for member in members:
        if not member.isfile() or archive.extractfile(member).read()!=(built if member.name==wrapper else blob(member.name)):
            raise SystemExit('bundle bytes differ: '+member.name)
print('reviewed_sha='+sha+' bundle_sha256='+hashlib.sha256(bundle.read_bytes()).hexdigest())
PIN
```

Independently review the bundle and record its SHA-256. The sheet §10 installer
command needs `--bundle-sha256 "$GBX_BUNDLE_SHA"`, as included below; the sheet
itself is outside this PR's allowed files.

Owner bundle/install commands, after H1 and independent bundle review (never run
the checkout installer as root; never use sudo -E or preserve the environment):

```bash
bash -euo pipefail <<'OWNER'
read -r -p 'Independently reviewed bundle SHA-256: ' GBX_BUNDLE_SHA </dev/tty
[[ "$GBX_BUNDLE_SHA" =~ ^[0-9a-f]{64}$ ]]
sudo /usr/bin/install -d -o root -g root -m 0700 /root/gatebox-slice1
sudo /usr/bin/install -o root -g root -m 0600 \
  /home/nexus/NEXUS/gatebox-slice1.tar /root/gatebox-slice1/bundle.tar
printf '%s  %s\n' "$GBX_BUNDLE_SHA" /root/gatebox-slice1/bundle.tar |
  sudo /usr/bin/sha256sum --check -
sudo /usr/bin/python3 -I -B - "$GBX_BUNDLE_SHA" <<'PY'
import ast, hashlib, io, pathlib, sys, tarfile
root = pathlib.Path('/root/gatebox-slice1'); data = (root/'bundle.tar').read_bytes()
if hashlib.sha256(data).hexdigest()!=sys.argv[1]: raise SystemExit('bundle SHA-256 mismatch')
with tarfile.open(fileobj=io.BytesIO(data),mode='r:') as archive:
    script = archive.extractfile('tools/gatebox/install.sh').read().decode()
    body = script.split("<<'PY'\n",1)[1].rsplit('\nPY',1)[0]
    values = {n.targets[0].id: ast.literal_eval(n.value) for n in ast.parse(body).body
              if isinstance(n,ast.Assign) and isinstance(n.targets[0],ast.Name)
              and n.targets[0].id in ('FILES','HARNESS','WRAPPER')}
    expected = {'tools/gatebox/'+n for n in values['FILES']+[values['WRAPPER']]}|{values['HARNESS']}
    directories = {str(parent) for path in map(pathlib.PurePosixPath,expected)
                   for parent in path.parents if str(parent)!='.'}
    members = archive.getmembers(); seen = set()
    for item in members:
        if item.isdir() and item.name in directories: continue
        if item.name not in expected or item.name in seen or not item.isfile() or item.size>2*1024**2:
            raise SystemExit('unsafe bundle member')
        seen.add(item.name)
    if seen!=expected: raise SystemExit('incomplete bundle')
    archive.extractall(root,members=members,filter='data')
PY
sudo /bin/bash /root/gatebox-slice1/tools/gatebox/install.sh install \
  --bundle-sha256 "$GBX_BUNDLE_SHA" --repo-id mother \
  --source-git /home/nexus/NEXUS/nexus-os/.git --submit-user nexus \
  --fixture-sha 8e0d6ae293df999eb6530e875df1206deac872a8
sudo /usr/bin/systemctl daemon-reload
sudo /usr/bin/systemctl enable --now mothergate.socket
OWNER
```

Then verify the whole sudoers configuration and custody ownership:

```bash
sudo -k; sudo visudo -c
stat -c '%a %U:%G %n' /run/mothergate-control \
  /run/mothergate-control/incoming /run/mothergate-control/sealed
# Expected: 755 root:root, 700 mothergate:mothergate, 700 root:root
```

Close the insurance root shell after these checks. **STOP after installation.**
Qualification and enable-qualified are a separate procedure, requiring this
round's independent review and completed H1 remediation. A failed/refused install
also means STOP and preserve evidence for Architect review.

`enable --now` starts the socket immediately and persists at boot through
`sockets.target`; it is not limited to the current session.

Host changes made by the installer and these Owner commands:

- Locked `mothergate`/`gaterun` accounts and primary groups when absent;
  `gatesubmit` when absent, and nexus's supplementary membership.
- Bootstrap files in `/root/gatebox-slice1`; code in `/usr/libexec/gatebox`;
  configuration in `/etc/gatebox`; payload skeleton in `/usr/lib/gatebox/root`;
  state/receipts in `/var/lib/mothergate`; `/var/lib/gatebox-source` mount point.
- `/etc/systemd/system/mothergate.socket`, `/etc/systemd/system/mothergate.service`,
  `/etc/systemd/system/mothergate-run@.service`,
  `/etc/systemd/system/mothergate-root@.service`, `/etc/sudoers.d/gatebox`,
  `/etc/polkit-1/rules.d/00-gatebox-deny.rules`,
  `/etc/tmpfiles.d/mothergate-control.conf`; temporary root-only `/etc/.gatebox-*` staging.
- Installer systemd reload, socket/service stop and tmpfiles creation; Owner
  socket enable/start, including the boot-enabling link
  `/etc/systemd/system/sockets.target.wants/mothergate.socket`; runtime
  `/run/mothergate-submit.sock` when the socket starts. Tmpfiles creates
  `/run/mothergate-control` root:root 0755, `incoming/` mothergate 0700 and
  `sealed/` root 0700 at installation and boot.
- Q30: each payload run's `mothergate-root@<run>.service` mounts a read-only
  tmpfs copy of the manifest-checked skeleton at `/run/mothergate-root/<run>`
  (base root 0700), so the payload "/" is a whole filesystem; it is unmounted
  when the payload stops, and `/run` is cleared at boot.

The installer verifies units/sudoers and leaves admission `qualifying`, with
`qualification.json` initially exactly `{"identity": null, "passed": []}`.
Only registered UID nexus can submit the fixed SHA/full request in this stage.
Root cannot submit. Install never enables normal admission. Existing outputs
are refused; interrupted installations need review, not an automatic overwrite.
Make the new gatesubmit membership effective in the actual Claude context.

CLIENTS context is a process-ancestry heuristic based on process command lines,
not an attestation of agent identity; it can be spoofed by the registered user.
For qualification only, start the actual foreground Claude context after the
host plant reboot, with no background agents. Run commands via its `!` shell
prefix to retain that ancestry:
`!sudo /usr/bin/python3 -I -B -m unittest discover -s /usr/libexec/gatebox/tests -p 'test_installed.py' -v`.

Host-only endpoint-check plant (step 6), **after the installation STOP is lifted**:
with agents closed, the Owner uses a host terminal. This is the reviewed Q15
script, SHA-256 `d529f1b8be40af8e0222a424092c7c6579103ec59ac5cc32aec6fe7e495f17cd`.
Copy it into root custody before executing; never run root code from the scratch
path. The script changes only the child validator in memory and verifies installed
lifecycle bytes unchanged. There is nothing to restore on disk. It records no
qualification row. A real payload may start and quarantine remains until reboot.

```bash
bash -euo pipefail <<'HOSTPLANT'
sudo /usr/bin/install -d -o root -g root -m 0700 /root/gatebox-slice1/host-plant
sudo /usr/bin/install -o root -g root -m 0600 \
  /tmp/claude-1000/-home-nexus-NEXUS-nexus-os/53f6c814-4d9f-4dfc-bcb5-ab55762fc5cc/scratchpad/gbq15-out/host_plant.py \
  /root/gatebox-slice1/host-plant/host_plant.py
printf '%s  %s\n' d529f1b8be40af8e0222a424092c7c6579103ec59ac5cc32aec6fe7e495f17cd \
  /root/gatebox-slice1/host-plant/host_plant.py | sudo /usr/bin/sha256sum --check -
set +e
sudo /usr/bin/env -u TMPDIR -u TEMP -u TMP /usr/bin/python3 -I -B \
  /root/gatebox-slice1/host-plant/host_plant.py 2>&1 |
  sudo /usr/bin/tee /root/gatebox-slice1/host-plant/output.log
GBX_STATUS=("${PIPESTATUS[@]}")
set -e
[[ "${GBX_STATUS[0]}" == 1 && "${GBX_STATUS[1]}" == 0 ]]
HOSTPLANT
```

The expected exit 1 alone is insufficient: review `HOST PLANT REACHED`, the C2
assertion failure (not a preflight error/skip), and the unchanged installed hash
in `/root/gatebox-slice1/host-plant/output.log`. On any different result, STOP.
Preserve the printed run, canary, manager/cgroup and fixture evidence outside
`/run`; follow exact-name recovery below if persistent leftovers remain. Once
reviewed and persistent staging/pending are empty, perform the required reboot:

```bash
sudo reboot
```

Separate qualification procedure (only after the installation STOP is lifted):
run from the actual Claude Code process context, with Owner approval for host/root
testing and no background agents. The suite records the Claude ancestor, invokes
`runuser -u nexus -- /usr/bin/python3 -I -B client.py ...`, retains the accepted
run receipt and the refused non-qualification reply, and authenticates on host:

```bash
bash -euo pipefail <<'QUALIFY'
sudo /usr/bin/python3 -I -B -m unittest discover \
  -s /usr/libexec/gatebox/tests -p 'test_installed.py' -v 2>&1 |
  sudo /usr/bin/tee -a /root/gatebox-slice1/qualification.log
sudo -k
sudo /usr/bin/python3 -I -B /usr/libexec/gatebox/verify.py --latest \
  --repo mother --sha 8e0d6ae293df999eb6530e875df1206deac872a8 \
  --gateset full --require-status PASS_SCOPED 2>&1 |
  sudo /usr/bin/tee -a /root/gatebox-slice1/qualification.log
QUALIFY
```

Output, including stderr, is saved in the root-owned
`/root/gatebox-slice1/qualification.log`; pipefail preserves suite/verifier errors.
C2 runs LAST and intentionally leaves root custody held. Preserve the log and
`/etc/gatebox/qualification.json` for review, then step 8 requires reboot:

```bash
sudo reboot
```

After the reviewed reboot, in a host terminal, with no pending submissions:

```bash
sudo /bin/bash /usr/libexec/gatebox/install.sh enable-qualified
```

Do not re-run qualification between C2 and enable-qualified. C2 evidence includes
its pre-reboot kernel boot_id, proof run_id and row; enable-qualified requires a
different current boot_id and empty staging, pending and incoming directories,
with no sealed run or pending custody marker. Root used-ID markers are allowed.
A missing boot identity, stale proof or any leftover fails closed.

C1-SEALED, C2-CANARY and C3-RESTART run through test_installed.py and are
mandatory qualification rows. C1 tests only unlink/rename/symlink denial on the
two endpoint files. C1/C3 use real root/mothergate DAC, the installed template,
disposable exact-instance runtime dependencies and a FIFO barrier with a
25-second deadline. They call lifecycle.py directly as root; they do not test
the privilege-elevation command or its policy. The collector runs on the host
as mothergate, outside the supervisor service sandbox. C2 substitutes incoming
leaves before handoff with symlinks to a root canary and a missing root target;
it requires rejection, no invocation/job, unchanged canary metadata/bytes and
no created target. Its quarantined symlinks stay in custody until reboot.
C3 tests collector death and an actual supervisor restart. BindsTo may cancel the queued
job on restart; custody and the admission blocker must remain until an explicit
settled stop proves cleanup. No payload restriction changes. Cleanup removes only
verified known leaves; ambiguity fails and retains custody for review. Direct
test_custody.py discovery runs proofs without recording rows; use test_installed.py
to qualify. Unavailable/skipped proofs FAIL qualification and cannot enable admission.

B2 proves denial of the tested direct manager calls. It also checks that the
installed polkit rule matches its installed-files hash. Whether this specific
rule caused denial, or wins over earlier conflicting rules, is UNVERIFIED.

Enable verifies every acceptance row, protected provenance and current installed
hashes. A host-only qualification invocation fails CLIENTS. Failed attempts are
retained. Offline discovery skips installed tests with a reason; it never emits
installed PASS lines. For later submissions, as nexus after group activation:

```bash
/usr/bin/python3 -I -B /usr/libexec/gatebox/client.py --repo mother \
  --sha FULL_40_LOWERCASE_SHA --gateset full --wait
```

Codex direct submission is unsupported (runner denies AF_UNIX); the Architect
submits on Codex's behalf. Copied/remapped receipts are UNAUTHENTICATED. Success
is only `PASS_SCOPED`: included gates, 39 exact exclusions, authenticated host
receipt. Chrome's recorded AppArmor label may be unconfined; it is not evidence
of enforcing AppArmor confinement. Desktop's registered harness owns its private
display and bus; the frozen child environment is preserved.

Deferred limitations (not qualified or protected by Slice 1):

- Root D-Bus controller/input custody: compromised supervisor can manipulate
  staging and receipts; basic checks do not close the broker-compromise race.
- Separate importer identity: Git parsing shares receipt-writer authority.
- Immutable Rust/vendor custody: host maintainers remain trusted; before/after
  inventories do not defeat change-and-restore attacks.
- A10 crash recovery: ambiguous/pending runs block admission; no crash proof.
- A9 exhaustion corpus: only fixed limits and the legitimate fixture's observed
  headroom. Scratch peaks are sampled every 50ms and can miss shorter spikes.
- Dedicated bounded receipt/staging filesystems: no total host storage quota.
- A4 broad hostile import/repack/bound-interaction corpus remains unqualified.
- A11 comprehensive tamper races, replay/rollback and compromised-supervisor
  resistance remain unqualified.
- Pruning: none; receipts are retained indefinitely.
- Nested verifier: all 39 D-E2-10 cases stay GitHub-CI-only.
- Authoritative CI/advisory audits remain required; offline gates do not replace them.
- tmpfs rename/fsync tests show syscall behavior, not persistent crash durability.

Live host canaries cover qualification only. Normal receipts reuse the recorded
host namespace baseline; they do not claim a new independent host-canary run.

## GB1 route-cells and the reviewed upgrade (SB3E-R18, R19)

`route-cells` requests carry `inventory_sha256`; success is `PASS_ROUTE_CELLS`, a
receipt with a `lane` group, validated by `receipt.validate_route` against
`policy/route-cells.json`. `rc_` credit needs a host-authenticated receipt: the
submitter runs `printf '%s\n' "$RUN_ID" | sudo -n /usr/libexec/gatebox/verify.py --authenticate-stdin`
(one exact, read-only sudoers rule with no wildcard; the ID is one lowercase 32-hex line
on stdin and any other argument is refused, SB3E-R21.6) and requires
`"authentication": "AUTHENTICATED"` and `"identity": "current"`. Public policy hashes
and copied summaries never authenticate.

Every receipt is validated against its own recorded gate policy: the current one,
or a retained set under `/etc/gatebox/history/<gates_sha256>/`, which keeps every
earlier installation's policy (SB3E-R21.4). Upgrade and `enable-qualified` hold the
lifecycle custody lock and the qualification lock from their first check through their
final re-check, so no run starts or stops and no qualification evidence is written
meanwhile (SB3E-R21.2). An existing install
is upgraded only by the separate action, never by `install` (which still refuses
every existing output). After PIN and independent bundle review, with the new
bundle extracted to the installer's `UPGRADE_BOOT` (`/root/gatebox-gb3` since GB3;
GB1's `/root/gatebox-gb1` and GB2's `/root/gatebox-gb2` are left in place) exactly as `/root/gatebox-slice1` above:

```bash
sudo /bin/bash /root/gatebox-gb3/tools/gatebox/install.sh upgrade \
  --bundle-sha256 "$GBX_BUNDLE_SHA" --old-bundle-sha256 "$GBX_OLD_BUNDLE_SHA"
```

It refuses unless the installed bytes are exactly the recorded old bundle and
identity, both services are stopped with no queued job and no run/root unit active,
pending, staging, incoming and sealed custody hold nothing but `.used-` markers, the
gate accounts and state ownership are unchanged and every retained receipt
authenticates against its own policy. It then retains the old policy and the old
identity, qualification and admission records under `/etc/gatebox/history`, never
reusing them, and leaves admission `qualifying` for the new identity: full
requalification (H1, C2, two reboots), then `enable-qualified`.

GB2 (SB3E-R26): the registry pins the mother `.git` by its boot-stable identity, the
ext2/3/4 filesystem's UUID-derived `f_fsid` plus the inode, never by `st_dev` (which
follows the NVMe enumeration order and differs between boots). Other filesystem types
and an unavailable identity are refused. An upgrade from a GB1 install computes the
identity live, requires the installed row's inode and frozen fields, never compares the
old `device`, and moves the old `registry.json` into the same history archive. Leftover
`acceptance-*.json` attempt records do not block requalification (nothing reads them);
the upgrade archives them there too.

### The route-cells payload (GB1b: SB3E-R18 P1, R19.1, R20)

For a `route-cells` request `init.py` runs no probe. It checks the `/opt/rust`
manifest against the policy pin, then: quiescence (no process but init; any survivor
is killed by its own PID and the run fails), A7 `run` (`/work/src`, the policy
environment, `HOME=/work/home`), quiescence again, and seals the A7 verdict and the
exact evidence set into the private stream, read without following links. Only on a
`pass` verdict does it take the harness named by A7's lane inventory: no-follow open,
regular file, bytes equal to the inventory hash, then `--list --format terse` from
`/work/src/mother/host`; the listed names must hash to the request's
`inventory_sha256`; quiescence again; re-open, re-hash, and only then the run by
pathname. A7 refuses before any build unless the resolved graph's local (path source)
packages are exactly its reviewed list and every local build script is pinned
(R19.1). The supervisor publishes the `lane` group from the sealed logs.

### GB3: route A1 in the bundle (SB3E-R48, R54, R57; route policy version 2)

A `route-cells` request also carries `route_pins_sha256`, the sha256 of the commit's
`tools/route_pins.json` (lock digest, graph hashes, declared dependencies). After the
first quiescence (`before-route-a1`), init opens that file without following links,
requires a bounded regular file whose sha256 equals the request, and records the result;
a refusal (`PINS: …`) ends the run before any Cargo process. Route A1 then runs from
`/gate/bin/route/route_a1.py` with `--pins /work/src/tools/route_pins.json`. It refuses
a bad pins file, an inherited toolchain variable or any snapshot Cargo configuration
finding before any Cargo process, then checks the lock, both resolved graphs and the
route packages. Its whole output must be one JSON verdict `pass`, or the run fails
(`route A1: …`) and A7 never starts. Quiescence `after-route-a1` precedes A7, which
reads its lock digest from the same file. The pins file is chosen by the submitter: the
box refuses drift from it, not from the reviewed base; only credit compares its digest
with the independently reviewed one. Every command's log ends with an empty packet, so
the supervisor knows a complete log; published bytes are unchanged.

The lane adds `route_a1`, `route_a1_sha256` (route A1's log) and `route_pins_sha256`. A
failed run keeps route A1's own result (`NOT_RUN`, `PINS_UNRECEIVED`, `PINS_REFUSED`,
`NO_START_RECORD`, `START_WITHOUT_END`, `LOG_INCOMPLETE`, `FAIL`, `PASS`) from the pins
record and route A1's start, end and log only, and invents no exit code or digest. A
success receipt is validated by the schema its own route policy's `schema_version`
selects, so GB1/GB2 route receipts (version 1) still authenticate, as `historical`.
`client.py` and `verify.py --latest` take `--route-pins-sha256`. The PIN step requires
`FILES` and `NOT_INSTALLED` to partition the tracked `tools/gatebox` files exactly and
packages only `FILES`.

## Maintenance: retained custody and reboot-only reinstallation

The installer never deletes or changes ownership of an existing
`/run/mothergate-control` entry. After validating inputs, it checks absence
read-only before staging any units and rechecks before publication. It publishes
the validated units, reloads systemd, stops both `mothergate.socket`
and `mothergate.service`, and checks the control name without following links.
Any entry (including an empty directory, a file or a dangling symlink) refuses
installation with **reboot required after review of retained custody**. Stop,
inspection or tmpfiles errors abort installation. It rechecks absence immediately
before the fixed `systemd-tmpfiles --create` call; tmpfiles is not a migration
or repair tool for an old supervisor-owned directory. A pre-existing control
entry leaves no published unit files. State appearing after the precheck can
still leave partial outputs; follow reviewed removal before retry.

Retained `sealed/RUN_ID`, `.pending-RUN_ID` and `.used-RUN_ID` state is evidence,
not disposable scratch. A start timeout does not prove job cancellation. A
partial relocation after timeout or interruption can leave only one leaf in
`sealed/RUN_ID` and another in `incoming/`. Stop cannot validate the required
pair, so cleanup refuses and pending custody continues blocking admission.
Do not repair this by moving leaves, deleting markers, recursively deleting the
control tree or rerunning tmpfiles over it.

Owner maintenance path:

1. Disable socket activation and stop the collector as in Manual removal below.
   Preserve receipts, manager job/state and exact cgroup evidence, and inspect
   retained custody without following any links. A stopped collector alone does
   not prove a payload stopped.
2. For an intact endpoint pair, the installed fixed lifecycle stop for the exact
   reviewed run ID may clean up only after an empty `Job=`, inactive/failed unit
   and unpopulated/absent exact cgroup are proved. Used-ID markers remain.
3. Missing/partial pairs, unexpected entries or ambiguous manager/cgroup state
   require Owner/Architect review. Preserve reviewed evidence outside `/run`
   before reboot, because `/run` is volatile. No automatic crash recovery or
   generic recursive deletion is provided.
4. After review, remove the installed tmpfiles policy and disable the socket
   before reboot so an old layout is not recreated. Complete reviewed persistent
   output removal below (retaining pre-existing accounts and evidence). Reboot,
   then retry the independently reviewed bundle only with the control name
   absent. If anything recreates it, investigate and refuse again.

### Failed custody proof: exact-name recovery

Qualification fails if cleanup reports any unexpected leftover. Cleanup is
registered before setUp's first write, attempts each installed unit stop even
after another fails, then proves settlement before removing persistent names.
It prints `run_id`, every retained path and SHA-256 of each runtime unit fixture.
Never guess these values. Preserve that output, qualification evidence, canary
and manager/cgroup state outside `/run` before the required reviewed reboot.

For the **recorded** 32-lowercase-hex RUN only, use a host terminal:

```bash
read -r -p 'Recorded custody RUN: ' GBX_RUN
[[ "$GBX_RUN" =~ ^[0-9a-f]{32}$ ]]
sudo /usr/bin/systemctl stop mothergate.socket mothergate.service
sudo /usr/bin/python3 -I -B /usr/libexec/gatebox/lifecycle.py stop "$GBX_RUN"
```

Require an empty Job, inactive/failed state and the production exact-cgroup
settlement proof. For C2's intentional symlink quarantine this command stops
the unit but refuses endpoint deletion; the read-only `Custody.settled(RUN)`
must prove settlement independently. Any settlement failure means STOP for review.

Only after settlement, as mothergate, run the installed proof's exact remover
(it uses no-follow directory descriptors and never recurses):

```bash
read -r -p 'Recorded custody RUN: ' GBX_RUN
[[ "$GBX_RUN" =~ ^[0-9a-f]{32}$ ]]
sudo /usr/bin/python3 -I -B - "$GBX_RUN" <<'RECOVER'
import pwd, re, sys
run=sys.argv[1]
if not re.fullmatch('[0-9a-f]{32}',run): raise SystemExit('invalid run ID')
sys.path.insert(0,'/usr/libexec/gatebox/tests')
import test_custody
owner=pwd.getpwnam('mothergate')
bridge=test_custody.lifecycle.Custody(owner=owner.pw_uid,owner_gid=owner.pw_gid)
bridge.settled(run)  # never treat a failed stop as permission
case=test_custody.InstalledCustodyTests('test_systemd_output_never_writes_disposable_root_canary')
case.run=run; case.owner=owner; case.remove_persistent()
RECOVER
```

This removes only `/var/lib/mothergate/staging/RUN/request.json`, the empty
`staging/RUN/snapshot`, `staging/RUN`, and `/var/lib/mothergate/pending/.RUN`.
Unexpected contents or symlinks at directory names are refused. No root write
occurs below those mothergate-owned parents. Manual runtime-fixture removal is
unnecessary: the reviewed reboot clears the exact drop-in, barrier and canary
fixtures under `/run` along with custody. Preserve `/run/gatebox-custody-RUN`
for review (exact known leaves: `release`, `canary`; `missing` must be absent).
Do not delete or repair sealed quarantine/markers. After reviewed evidence
preservation, reboot clears `/run`; the installed tmpfiles policy recreates the
current layout. Persistent staging/pending must already be empty. Verify clean
custody before enable-qualified; a failed qualification requires reviewed
requalification, not manual insertion of passed rows. This proof recovery does
not require uninstalling the reviewed bundle.

### Reviewed qualification reset after refusal

After C2 is recorded every qualify refuses. If enable-qualified refuses, STOP
for review; never insert or edit passed rows. After preserving evidence and the
exact-name recovery above, reboot first. If the same installed identity is still
in the **qualifying** stage and custody is clean, the Owner may reset the entire
qualification, retaining a root-owned copy of the previous evidence. Changed
installed bytes/identity require independent review and reinstallation instead.
This reset does not enable admission or convert any failure into a pass.

```bash
sudo /usr/bin/systemctl stop mothergate.socket mothergate.service
sudo /usr/bin/python3 -I -B - <<'RESET'
import fcntl, os, sys, uuid
sys.path.insert(0,'/usr/libexec/gatebox')
import probe
probe.require_installed()
fd=os.open(probe.CONFIG/'qualification.lock',os.O_RDWR|os.O_CREAT|os.O_NOFOLLOW,0o600)
try:
    probe.protected(probe.CONFIG/'qualification.lock')
    fcntl.flock(fd,fcntl.LOCK_EX|fcntl.LOCK_NB)  # refuse a concurrent qualification
    probe.require(probe.config('admission.json')==dict(identity=probe.installed_identity(),stage='qualifying'),
                  'reset requires qualifying stage')
    probe.require(probe.clean_custody(),'custody leftovers; review required')
    previous=probe.config('qualification.json')
    backup='qualification-reset-'+uuid.uuid4().hex+'.json'
    probe.atomic_config(backup,previous)
    probe.atomic_config('qualification.json',{'identity':None,'passed':[]})
    print('previous evidence retained at '+str(probe.CONFIG/backup))
finally: os.close(fd)
RESET
sudo /usr/bin/systemctl start mothergate.socket
```

Resume the full QUALIFY procedure in the actual Claude context, C2 last, save
the log, reboot, then enable-qualified. Prior receipts and attempt records remain.

Manual removal after a failed or interrupted install requires Owner review of
retained receipts and the pre-install accounts/groups. Preflight validates
inputs, identities, unit syntax and sudoers before persistent account/file
changes; its only earlier write is a root-only temporary staging directory.
After the read-only precheck, units are published and reloaded before stopping
runtime services; accounts and remaining outputs follow the second absence check.
`/etc/gatebox/created-accounts.json` records exactly the users/groups created by a
successful account step; pre-existing entries are excluded. It is hashed in
installed-files.json. A failure during account creation may precede the record:
missing records require actual-state review, never guessed deletion authority.
I/O or command failure can still leave partial outputs; there is no automatic
rollback. Preserve evidence before running these host commands in a
`bash -euo pipefail` subshell:

```bash
bash -euo pipefail <<'REMOVE'
if [[ -e /etc/systemd/system/mothergate.socket ]]; then
  sudo /usr/bin/systemctl disable --now mothergate.socket
fi
if [[ -e /etc/systemd/system/mothergate.service ]]; then
  sudo /usr/bin/systemctl stop mothergate.service
fi
sudo /usr/bin/systemctl stop 'mothergate-run@*.service'
sudo /usr/bin/systemctl stop 'mothergate-root@*.service'
sudo /usr/bin/rm -f /etc/systemd/system/mothergate.socket \
  /etc/systemd/system/mothergate.service /etc/systemd/system/mothergate-run@.service \
  /etc/systemd/system/mothergate-root@.service \
  /etc/sudoers.d/gatebox /etc/polkit-1/rules.d/00-gatebox-deny.rules \
  /etc/tmpfiles.d/mothergate-control.conf
sudo /usr/bin/systemctl daemon-reload
sudo /usr/bin/systemctl reset-failed
# Read the protected creation record BEFORE removing configuration. If missing,
# stop for review rather than guessing account provenance.
sudo /usr/bin/python3 -I -B - <<'PY'
import grp, json, os, pathlib, pwd, stat, subprocess
record = pathlib.Path('/etc/gatebox/created-accounts.json')
if os.path.lexists(record):
    for path in [*reversed(record.parents),record]:
        info = path.lstat()
        if info.st_uid or info.st_mode & 0o022 or stat.S_ISLNK(info.st_mode):
            raise SystemExit('unprotected creation record')
        if any(n.startswith('system.posix_acl_') for n in os.listxattr(path)):
            raise SystemExit('ACL on creation record')
    created = json.loads(record.read_bytes())
else:
    # Even with no output directories, a crash during apply_accounts may have
    # left accounts. Inspect actual identities; never silently assume absence.
    present=[]
    for name in ('mothergate','gaterun'):
        try: pwd.getpwnam(name); present.append('user:'+name)
        except KeyError: pass
    for name in ('mothergate','gaterun','gatesubmit'):
        try: grp.getgrnam(name); present.append('group:'+name)
        except KeyError: pass
    if present or os.path.lexists(record.parent) or os.path.lexists('/usr/libexec/gatebox'):
        raise SystemExit('missing creation record; actual-state review required: '+repr(present))
    created = dict(users=[],groups=[])
if set(created) != {'users','groups'}: raise SystemExit('unknown record schema')
for key,allowed in [('users',{'mothergate','gaterun'}),
                    ('groups',{'mothergate','gaterun','gatesubmit'})]:
    names = created[key]
    if type(names) is not list or any(type(n) is not str for n in names):
        raise SystemExit('bad record list')
    if len(names)!=len(set(names)) or not set(names)<=allowed:
        raise SystemExit('unknown/duplicate recorded identity')
try: group=grp.getgrnam('gatesubmit')
except KeyError: group=None
# Prior nexus membership is not recorded. Preserve it here; changing it needs
# separate evidence of its pre-install state, never a manual code edit.
if group is not None and 'nexus' in group.gr_mem:
    if 'gatesubmit' in created['groups']:
        subprocess.run(['/usr/bin/gpasswd','-d','nexus','gatesubmit'],check=True)
    else: print('preserving nexus membership in pre-existing gatesubmit')
for name in created['users']:
    try: pwd.getpwnam(name)
    except KeyError: continue
    subprocess.run(['/usr/sbin/userdel',name],check=True)
# Some userdel configurations also remove an empty private group.
for name in created['groups']:
    try: grp.getgrnam(name)
    except KeyError: continue
    subprocess.run(['/usr/sbin/groupdel',name],check=True)
PY
# Includes retained receipts: copy reviewed evidence elsewhere first.
# Review each root-owned tree and any crash-left staging directory first.
sudo /usr/bin/python3 -I -B - <<'PY'
import os, pathlib, re, stat, subprocess
paths = list(map(pathlib.Path,('/usr/libexec/gatebox','/etc/gatebox',
    '/usr/lib/gatebox/root','/var/lib/mothergate','/var/lib/gatebox-source',
    '/root/gatebox-slice1')))
for path in pathlib.Path('/etc').glob('.gatebox-*'):
    if not re.fullmatch(r'\.gatebox-[a-z0-9_]{8}',path.name):
        raise SystemExit('unexpected staging name; review required')
    paths.append(path)
for path in paths:
    for parent in [*reversed(path.parents),path]:
        try: info = parent.lstat()
        except FileNotFoundError: break
        if not stat.S_ISDIR(info.st_mode) or info.st_uid or info.st_mode & 0o022:
            raise SystemExit('unsafe removal root/ancestor: '+str(parent))
        if any(n.startswith('system.posix_acl_') for n in os.listxattr(parent)):
            raise SystemExit('ACL on removal root/ancestor')
subprocess.run(['/usr/bin/rm','-rf','--one-file-system','--',*map(str,paths)],check=True)
PY
# Remove /usr/lib/gatebox only if empty; preserve unrelated contents.
if [[ -e /usr/lib/gatebox ]]; then
  sudo /usr/bin/rmdir --ignore-fail-on-non-empty /usr/lib/gatebox
fi
# Do not recursively delete runtime custody; use the reviewed reboot path above.
sudo /usr/bin/rm -f /run/mothergate-submit.sock
REMOVE
```

For a partial installation, absent outputs are skipped; retain every pre-existing
account/group and its original membership. A pre-existing gatesubmit group keeps
nexus membership because its prior state is unrecorded. Removing a newly created
gatesubmit group also removes its membership. A missing record with any relevant
identity present blocks removal for actual-state review, including a crash before
output directories were created. `rm -rf` above deletes
only the listed installation trees, including their evidence after review.
