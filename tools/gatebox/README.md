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

Owner commands, from the host after independent bundle review (never run the
checkout installer as root):

```bash
set -euo pipefail
read -r -p 'Independently reviewed bundle SHA-256: ' GBX_BUNDLE_SHA
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
```

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

Run qualification from the actual Claude Code process context, with Owner
approval for host/root testing. The suite records the Claude ancestor, invokes
`runuser -u nexus -- /usr/bin/python3 -I -B client.py ...`, retains the accepted
run receipt and the refused non-qualification reply, and authenticates on host:

```bash
sudo /usr/bin/python3 -I -B -m unittest discover \
  -s /usr/libexec/gatebox/tests -p 'test_installed.py' -v
sudo /usr/bin/python3 -I -B /usr/libexec/gatebox/verify.py --latest \
  --repo mother --sha 899e3872c5b61223c87b905f63c66947a816259b \
  --gateset full --require-status PASS_SCOPED
sudo /bin/bash /usr/libexec/gatebox/install.sh enable-qualified
```

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

Manual removal after a failed or interrupted install requires Owner review of
retained receipts and the pre-install accounts/groups. Preflight validates
inputs, identities, unit syntax and sudoers before persistent account/file
changes; its only earlier write is a root-only temporary staging directory.
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
  /etc/sudoers.d/gatebox /etc/polkit-1/rules.d/00-gatebox-deny.rules
sudo /usr/bin/systemctl daemon-reload
sudo /usr/bin/systemctl reset-failed
sudo /usr/bin/gpasswd -d nexus gatesubmit
# Only delete accounts/groups that this installation created:
sudo /usr/sbin/userdel mothergate
sudo /usr/sbin/userdel gaterun
if getent group mothergate >/dev/null; then sudo /usr/sbin/groupdel mothergate; fi
if getent group gaterun >/dev/null; then sudo /usr/sbin/groupdel gaterun; fi
if getent group gatesubmit >/dev/null; then sudo /usr/sbin/groupdel gatesubmit; fi
# Includes retained receipts: copy reviewed evidence elsewhere first.
sudo /usr/bin/rm -rf /usr/libexec/gatebox /etc/gatebox /usr/lib/gatebox/root \
  /var/lib/mothergate /var/lib/gatebox-source /root/gatebox-slice1
sudo /usr/bin/rm -rf /run/mothergate-control
sudo /usr/bin/rm -f /run/mothergate-submit.sock
REMOVE
```

For a partial installation, omit commands for objects that were never created;
retain every pre-existing account/group and its original membership. If nexus
was already a gatesubmit member, retain that membership. `rm -rf` above deletes
only the listed installation trees, including their evidence after review.
