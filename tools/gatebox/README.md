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

Build the uncompressed bundle from the reviewed repository root, without root:

```bash
bash -euo pipefail <<'BUILD'
python3 -I -B - <<'PY' | tar --exclude='__pycache__' --no-recursion --null --files-from=- -cf /home/nexus/NEXUS/gatebox-slice1.tar
import ast, pathlib, stat, sys
text = pathlib.Path('tools/gatebox/install.sh').read_text().split("<<'PY'\n",1)[1].rsplit('\nPY',1)[0]
values = {n.targets[0].id: ast.literal_eval(n.value) for n in ast.parse(text).body
          if isinstance(n,ast.Assign) and isinstance(n.targets[0],ast.Name)
          and n.targets[0].id in ('FILES','HARNESS')}
for name in ['tools/gatebox/'+n for n in values['FILES']]+[values['HARNESS']]:
    if not stat.S_ISREG(pathlib.Path(name).lstat().st_mode): raise SystemExit('not a regular file')
    sys.stdout.buffer.write(name.encode()+b'\0')
PY
sha256sum /home/nexus/NEXUS/gatebox-slice1.tar
BUILD
```

Independently review the bundle and record its SHA-256. The sheet §10 installer
command needs `--bundle-sha256 "$GBX_BUNDLE_SHA"`, as included below; the sheet
itself is outside this PR's allowed files.

Owner H1 steps, **before installation**, in a host terminal. Close every agent,
background task and sub-agent, including this session. This check must print
nothing; do not proceed while any matching process remains:

```bash
pgrep -u nexus -af 'claude|codex'
# Empty result required. Then, with all agents closed:
sudo gpasswd -d nexus docker
sudo reboot
```

After reboot, confirm both `id -nG` and `id -nG nexus` omit `docker`. Docker is
subsequently available only through `sudo docker`. Do not restart agents during
the Owner sudo session. In a second host terminal keep `sudo -i` open as lockout
insurance until the post-install checks pass.

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
              and n.targets[0].id in ('FILES','HARNESS')}
    expected = {'tools/gatebox/'+n for n in values['FILES']}|{values['HARNESS']}
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
  --fixture-sha 899e3872c5b61223c87b905f63c66947a816259b
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
  `/etc/systemd/system/mothergate-run@.service`, `/etc/sudoers.d/gatebox`,
  `/etc/polkit-1/rules.d/00-gatebox-deny.rules`,
  `/etc/tmpfiles.d/mothergate-control.conf`; temporary root-only `/etc/.gatebox-*` staging.
- Installer systemd reload, socket/service stop and tmpfiles creation; Owner
  socket enable/start, including the boot-enabling link
  `/etc/systemd/system/sockets.target.wants/mothergate.socket`; runtime
  `/run/mothergate-submit.sock` when the socket starts. Tmpfiles creates
  `/run/mothergate-control` root:root 0755, `incoming/` mothergate 0700 and
  `sealed/` root 0700 at installation and boot.

The installer verifies units/sudoers and leaves admission `qualifying`, with
`qualification.json` initially exactly `{"identity": null, "passed": []}`.
Only registered UID nexus can submit the fixed SHA/full request in this stage.
Root cannot submit. Install never enables normal admission. Existing outputs
are refused; interrupted installations need review, not an automatic overwrite.
Make the new gatesubmit membership effective in the actual Claude context.

CLIENTS context is a process-ancestry heuristic based on process command lines,
not an attestation of agent identity; it can be spoofed by the registered user.
Run sudo commands via Claude Code's `!` shell prefix to retain that ancestry:
`!sudo /usr/bin/python3 -I -B -m unittest discover -s /usr/libexec/gatebox/tests -p 'test_installed.py' -v`.

Separate qualification procedure (only after the installation STOP is lifted):
run from the actual Claude Code process context, with Owner approval for host/root
testing and no background agents. The suite records the Claude ancestor, invokes
`runuser -u nexus -- /usr/bin/python3 -I -B client.py ...`, retains the accepted
run receipt and the refused non-qualification reply, and authenticates on host:

```bash
bash -euo pipefail <<'QUALIFY'
sudo /usr/bin/python3 -I -B -m unittest discover \
  -s /usr/libexec/gatebox/tests -p 'test_installed.py' -v
sudo -k
sudo /usr/bin/python3 -I -B /usr/libexec/gatebox/verify.py --latest \
  --repo mother --sha 899e3872c5b61223c87b905f63c66947a816259b \
  --gateset full --require-status PASS_SCOPED
sudo /bin/bash /usr/libexec/gatebox/install.sh enable-qualified
QUALIFY
```

C1-SEALED, C2-CANARY and C3-RESTART run through test_installed.py and are
mandatory qualification rows. They use real root/mothergate DAC, the production
shim/template and collector, disposable exact-instance runtime dependencies,
a FIFO barrier with a 25-second deadline, a root canary, a pending payload job,
collector death and an actual supervisor restart. BindsTo may cancel the queued
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
sudo /usr/bin/systemctl disable --now mothergate.socket
sudo /usr/bin/systemctl stop mothergate.service
sudo /usr/bin/systemctl stop 'mothergate-run@*.service'
sudo /usr/bin/rm -f /etc/systemd/system/mothergate.socket \
  /etc/systemd/system/mothergate.service /etc/systemd/system/mothergate-run@.service \
  /etc/sudoers.d/gatebox /etc/polkit-1/rules.d/00-gatebox-deny.rules \
  /etc/tmpfiles.d/mothergate-control.conf
sudo /usr/bin/systemctl daemon-reload
sudo /usr/bin/systemctl reset-failed
sudo /usr/bin/gpasswd -d nexus gatesubmit
# Read the protected creation record BEFORE removing configuration. If missing,
# stop for review rather than guessing account provenance.
sudo /usr/bin/python3 -I -B - <<'PY'
import grp, json, os, pathlib, stat, subprocess
record = pathlib.Path('/etc/gatebox/created-accounts.json')
for path in [*reversed(record.parents),record]:
    info = path.lstat()
    if info.st_uid or info.st_mode & 0o022 or stat.S_ISLNK(info.st_mode):
        raise SystemExit('unprotected creation record')
    if any(n.startswith('system.posix_acl_') for n in os.listxattr(path)):
        raise SystemExit('ACL on creation record')
created = json.loads(record.read_bytes())
if set(created) != {'users','groups'}: raise SystemExit('unknown record schema')
for key,allowed in [('users',{'mothergate','gaterun'}),
                    ('groups',{'mothergate','gaterun','gatesubmit'})]:
    names = created[key]
    if type(names) is not list or any(type(n) is not str for n in names):
        raise SystemExit('bad record list')
    if len(names)!=len(set(names)) or not set(names)<=allowed:
        raise SystemExit('unknown/duplicate recorded identity')
for name in created['users']:
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
sudo /usr/bin/rmdir --ignore-fail-on-non-empty /usr/lib/gatebox
# Do not recursively delete runtime custody; use the reviewed reboot path above.
sudo /usr/bin/rm -f /run/mothergate-submit.sock
REMOVE
```

For a partial installation, omit commands for objects that were never created;
retain every pre-existing account/group and its original membership. If nexus
was already a gatesubmit member, retain that membership. `rm -rf` above deletes
only the listed installation trees, including their evidence after review.
