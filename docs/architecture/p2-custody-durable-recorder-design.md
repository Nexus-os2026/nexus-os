# P2 custody: durable recorder and refusal store — design candidate

| | |
|---|---|
| Mission | P2-V1-R3B-I3-P, a module of Phase Two (not Phase Three) |
| Status | **Design candidate for independent Architect review.** Nothing here is implemented, provisioned, live-validated or approved by being published. |
| Base | `45898e05178a56efaadeb1f8f9ee7a0a521c72c9`, tree `cd197e047cb5a57e458b2af5d32f9c5406371301`, sole parent `0eb2a947e839b5639d9688da4e6b51b5e72a4d13`, on `review/p2-v1-cleanup-observers` |
| Base status | Accepted for component code-and-regression review only. No authoritative integration and no Phase Two completion has occurred. |
| Authorizes | Nothing. Implementation, provisioning of any state root, native execution, service integration, runner changes and live validation each need a separate approved mission. |

Source labels:

| Label | Meaning |
|---|---|
| **[S]** | A fact of the repository at the base, with a `file:line` reference |
| **[U]** | Primary platform documentation, listed in §1.4 |
| **[I]** | A read-only observation of the development host on 2026-10-01. It is not a property of any future runtime host. |
| **[P]** | A proposal of this design |
| **[D]** | A decision that needs Architect (or Owner) authority |
| **[A]** | An assumption that defines the supported profile; software cannot prove it |

Path abbreviations (all under `crates/nexus-verifier-sandbox/tests/`):

| Abbreviation | Path |
|---|---|
| `mod.rs` | `support/custody/mod.rs` |
| `model.rs` | `support/custody/model.rs` |
| `core.rs` | `support/custody/core.rs` |
| `codec.rs` | `support/custody/codec.rs` |
| `core-tests` | `phase2_custody_core.rs` |
| `codec-tests` | `phase2_custody_codec.rs` |

---

## 0. Decisions in brief

| # | Decision | Where |
|---|---|---|
| 1 | One writer, the recorder thread of the custody owner process, makes records durable. The core still decides everything; the recorder only stores bytes and reports durability. | §4, §7 |
| 2 | Each custody generation gets one journal: a pre-provisioned, preallocated, zero-filled pool file with fixed 128-byte slots. Slot *n* holds record sequence *n*'s unchanged version-1 frame. | §3.3, §4.1 |
| 3 | An acknowledgement is sent only after the record's slot is written in full **and** `fdatasync` on the journal returned success, and the journal's identity was rechecked. Nothing else ever becomes an acknowledgement. | §4.3, §4.4 |
| 4 | The first write, sync or identity error poisons the journal for the rest of the process. That record is reported through `record_failed`. There is no retry of a failed sync and no later acknowledgement. | §4.5 |
| 5 | Physical space for every record a generation can ever issue is reserved, and the blocks written, at provisioning time. The core's logical capacity therefore never exceeds the physical slots, by construction rather than by assumption. | §3.8, §4.6 |
| 6 | Only the Owner, as root, provisions the store, through an explicit procedure. Opening never creates, repairs or selects anything. A missing or different component is a refusal. | §3.2–§3.5 |
| 7 | At startup the store classifies every prior journal with a pure function. Anything malformed, or any unsealed generation that recorded native work, becomes a prior incident. The core then refuses the run until a root-owned disposition binds to that incident exactly. | §5.3, §6 |
| 8 | Stored evidence stays data. No record names a PID, unit, cgroup or path, and restart reconstructs no ownership. A disposition means "accepted without cleanup confirmation"; a seal means "closed as recorded". Neither is proof of cleanup. | §6 |
| 9 | Storage never blocks control. The owner thread never waits on storage during a run, and status reads a copy-only recorder status. A stalled sync is reported and never times out into an acknowledgement. | §7 |
| 10 | Precise limit: a process running as the store's uid (including any CI job code) can rewrite journals. Hash chains do not change that. Resisting it needs a trusted anchor, which is an Architect decision. | §5.6 |
| 11 | No core, codec or model change is required for this design. Future API candidates are listed, not assumed. | §2.11 |

---

## 1. Scope, base and sources

### 1.1 What this document is

This is a design for the "Durability" and "Journal and recovery" integration obligations that the accepted core states but does not meet (`mod.rs:144-151`) [S], together with the "Unresolvable states" refusal it leaves to the integration (`mod.rs:168-171`) [S].

It specifies:
- a durable recorder behind the existing `RecordSink` (`core.rs:93`) [S];
- a refusal store that turns prior generations' durable evidence into the `PriorIncident` list that `Custody::new` takes (`core.rs:1292`) [S];
- a disposition reader behind the existing `DispositionValidator` (`core.rs:99`) [S];
- provisioning, crash, verification and implementation boundaries.

It does not design:
- the custody owner process or service;
- its control transport;
- the live harness cases;
- the runner.

Where this design assumes something about them, it says so.

### 1.2 Base and governance

- **Base.** The base was verified as stated in the front matter.
- **Governance.** `AGENTS.md` and `CLAUDE.md` were read at the base. The rules applied are:
  - fail closed, with no `$HOME`, cwd or temp fallback (AGENTS §6.1);
  - authority is explicit, never a string (§6.2);
  - no recreation of trusted roots during lookup (§6.3);
  - bounded cleanup and no PID authority (§6.5);
  - security errors stay observable (§6.6);
  - no dependency churn (§15);
  - no secrets (§16);
  - a second, review-focused pass (§18);
  - the two-attempt rule (§12.2);
  - no lock held across filesystem mutation, wait or audit (CLAUDE.md, "Locks and audit");
  - the trust-boundary list: paths, `HOME`, UUIDs and PIDs are not authority (CLAUDE.md, "Trust boundaries").

### 1.3 Earlier session designs

Two earlier read-only design loops of this Phase 2.2 session produced the custody plan (R3B-P1) and the custody contract (R3B-P2). They are not repository files. This design uses them only for continuity:

- **Kept.** These principles survive:
  - an owner that creates and keeps native ownership;
  - write-ahead admission;
  - a root-initialized state root;
  - root-owned dispositions as the Owner's disposition authority (contract approval item D-2, still pending);
  - "nothing a lower-privileged writer creates lifts a refusal".
- **Superseded.** The contract's record format (one text file per record, with `O_TMPFILE` and `linkat`) is replaced. The version-1 binary frame of I2A (`codec.rs:16-79`) [S] is now the record format and must stay byte-for-byte unchanged.
- **Reconciled.** The two earlier designs disagreed on what blocks after a crash:
  - The plan refused after any non-final record.
  - The contract refused only when work was outstanding.

  This design takes a middle rule, justified in §5.3: refuse after any unsealed generation that recorded native work.

### 1.4 Primary documentation and host observations

Linux man-pages 6.7 (Ubuntu package `manpages` 6.7-2), read locally [U]:

| Page | Used for |
|---|---|
| `fsync(2)` | `fsync` flushes the device cache; a directory entry needs its own `fsync`; `fdatasync` skips metadata not needed for retrieval; EIO semantics since Linux 4.13 |
| `write(2)` | A successful `write` does not guarantee that data is committed; partial writes and EINTR |
| `pread(2)` | BUGS: with `O_APPEND`, `pwrite` appends regardless of offset |
| `fallocate(2)` | Mode 0: "subsequent writes into the range … are guaranteed not to fail because of lack of disk space"; EOPNOTSUPP; `FALLOC_FL_KEEP_SIZE`; `FALLOC_FL_UNSHARE_RANGE` for shared extents |
| `sync_file_range(2)` | Its warning: copy-on-write filesystems cannot overwrite allocated blocks in place, and writes into preallocated space may need block-allocator calls |
| `open(2)` | `O_NOFOLLOW` (final component only), `O_DIRECTORY`, `O_PATH`, `O_CLOEXEC` |
| `openat2(2)` | `RESOLVE_NO_SYMLINKS`, `RESOLVE_NO_MAGICLINKS`, `RESOLVE_BENEATH`, `RESOLVE_NO_XDEV`; since Linux 5.6 |
| `flock(2)` | Locks belong to the open file description, are inherited across `fork` and kept across `execve`, and are advisory |
| `fcntl(2)` | Open-file-description locks (considered; `flock` chosen) |
| `statfs(2)` | `f_type` magic numbers: `EXT4_SUPER_MAGIC 0xef53`, `XFS_SUPER_MAGIC 0x58465342`, `TMPFS_MAGIC`, `BTRFS_SUPER_MAGIC`, `NFS_SUPER_MAGIC`, `OVERLAYFS_SUPER_MAGIC`, `FUSE_SUPER_MAGIC`; `ST_RDONLY` |
| `stat(2)` | `fstat`, and `fstatat` with `AT_SYMLINK_NOFOLLOW` (the link itself, not its target); the owner, mode, link count, inode and device fields |
| `inode(7)` | Inode numbers are unique only within a filesystem; link count |
| `link(2)` | EPERM under `protected_hardlinks`; EPERM for append-only or immutable files |
| `unlink(2)` | EPERM for append-only or immutable files; sticky directories |
| `rename(2)` | Atomic replacement of the target name, used by the Owner's install-by-rename procedures |
| `chattr(1)`, `ioctl_iflags(2)` | `FS_APPEND_FL` and `FS_IMMUTABLE_FL`, both settable only with `CAP_LINUX_IMMUTABLE` |
| `proc_sys_fs(5)` | `protected_hardlinks` (default 0) and `protected_symlinks` semantics |
| `ext4(5)` | Barriers on by default, `barrier=0`/`nobarrier`, `data=ordered` default, `commit` |
| `acl(5)` | The group permission bits equal `ACL_MASK`, which caps every named entry |
| `random(4)` | `boot_id` is a random string generated once, not on each read |

Linux kernel documentation [U]:

- **errseq** (`https://docs.kernel.org/core-api/errseq.html`): "Subsequent calls will return 0, until another error is recorded, at which point it's reported to each of them once."
- **vfs**, "Handling errors during writeback" (`https://docs.kernel.org/filesystems/vfs.html`): "reporting errors to fsync on all file descriptions that were open at the time that the error occurred."

PostgreSQL documentation, `data_sync_retry` (`https://www.postgresql.org/docs/current/runtime-config-error-handling.html`). This is corroboration of the hazard, not Linux authority: "the status of data in the kernel's page cache is unknown after a write-back failure … the second attempt may be reported as successful, when in fact the data has been lost."

Host observations [I], read only, on the development host:
- Linux 6.17.0-35-generic.
- `/`, `/home` and `/var/lib` on ext4, mounted `rw,relatime`. No barrier option is shown, so ext4's default applies (barriers on, `ext4(5)`).
- `/run/user/<uid>` is tmpfs.
- `fs.protected_hardlinks = 1` and `fs.protected_symlinks = 1`.
- A `github-runner` account exists with uid 1001.

None of this selects or initializes a state root.

---

## 2. Actual contract inventory (A)

Implemented at the base [S]. "Proposed" marks this design's adapter behaviour [P].

### 2.1 Evidence records and the recorder interface

| Element | Implemented [S] | Proposed adapter behaviour [P] |
|---|---|---|
| `RecordIntent { id, at, kind, digest }` | `model.rs:457`. The digest is SHA-256 of the frame's header and payload, not of its trailer (`model.rs:451-455`). | The recorder writes `codec::encode_record(intent)` unchanged. |
| `RecordAck { id, digest }`, `RecordAck::of` | `model.rs:466-479` | Sent only after the slot write, `fdatasync` and identity recheck succeed (§4.3). |
| `RecordSink::submit(&mut self, &RecordIntent)` | `core.rs:93-95`. It returns nothing; submitting is not durability (`core.rs:89-92`). | `submit` enqueues on a queue whose capacity is at least `record_capacity`, so it never blocks (§7.1). |
| `Custody::flush_records` | `core.rs:2958-2991`. It submits unsent records in order; a sink panic is contained, the record stays unsent, and it is a `RecorderFault` failure (`core.rs:2974-2985`). | The owner calls it after every custody call that can issue records. |
| `Custody::acknowledge` | `core.rs:2996-3010` → `Ledger::acknowledge` (`core.rs:1042-1069`). Outcomes are in `AckOutcome` (`model.rs:483-502`). | The owner applies the recorder's `Durable` events in FIFO order. Any outcome other than `Acknowledged` or `Duplicate` for its own recorder's event is an integration fault (§4.5). |
| `Custody::record_failed` | `core.rs:3016-3031` → `Ledger::fail` (`core.rs:1072-1090`). Only the next unacknowledged record can fail; after that every later acknowledgement is `LedgerFailed`. It is a fail-stop `RecordFailed` failure. | Reported once, with the id of the first record that is not durable (§4.5). |
| `EvidenceView` in snapshots | `model.rs:837-845` | Status composes it with the recorder's own status (§7.1). |
| `BusyWhat::Record` | `model.rs:880` (only while `submit` runs). | `submit` only enqueues; the recorder's I/O shows in the recorder status instead. |

### 2.2 Ledger ordering and capacity reservation

Implemented [S]:

- **Order.** `Ledger::push` numbers records from 1 per generation and computes `codec::record_digest` (`core.rs:1013-1027`, digest at `core.rs:1018`).
- **Acknowledgement.** Records are acknowledged strictly in order and bound to the exact digest. The outcomes are Conflict, Duplicate, OutOfOrder, NotIssued, Foreign and LedgerFailed (`core.rs:1042-1069`).
- **Two capacities.** The general capacity is `record_capacity - control_reserve`; the control capacity is `control_reserve` (`core.rs:967-968`).
  - `reserve(count)` is all-or-nothing against general capacity (`core.rs:980-989`).
  - `issue_reserved` consumes a reservation (`core.rs:996-1000`).
  - `issue_control` draws only from the control reserve (`core.rs:1004-1011`).
- **Who reserves what.**

| Operation | Reservation | Reference |
|---|---|---|
| Run start | Recovery budget + 3 (RunStarted issued at once; RunEnded, RecoveryRequired, one per attempt) | `core.rs:1473` |
| Case | 2 (CaseStarted issued; CaseEnded) | `core.rs:1545` |
| Action | 3 (ActionStarted issued; ActionFailed; ActionSettled) | `core.rs:1599` |
| Late incident | 2, plus a RecoveryRequired while a completion candidate | `core.rs:1939` |
| Control facts | At most one AdmissionClosed and one ShutdownRefused | `core.rs:2374-2387`, `core.rs:2856-2864` |

- **Bounds.** `Config::LIVE`: `record_capacity` 512, `control_reserve` 2 (`model.rs:763-775`). `Custody::new` refuses `control_reserve < 2` or `> record_capacity` (`core.rs:1298-1304`).
- **Test.** `h14_capacity_reservation_prevents_unrecordable_admission` (`core-tests:3785`) proves that an admitted action can always record its worst case within capacity.

**Consequence used by this design [P].** A generation issues at most `record_capacity` records, each a version-1 record frame of 73 to 123 bytes. The record payload is 33 to 83 bytes (`codec.rs:75`, `LARGEST_PAYLOAD` at `codec.rs:115`) plus 40 bytes of header and trailer.

### 2.3 Write-ahead gates in the core

Implemented [S]:
- **Admission.** It requires the action's start record to be acknowledged (`core.rs:1658-1664`) before the gate's linearization point (`core.rs:1665-1675`).
- **New case.** It requires every earlier record to be acknowledged (`core.rs:1542-1544`).
- **Terminal commitment.** It requires every earlier record acknowledged, recording intact and nothing unresolved (`core.rs:2662-2669`). The terminal record is issued in the same step (`core.rs:2678-2686`).
- **Finality.** A run is final only once the terminal record is acknowledged (`core.rs:2690-2697`). A pass is published only then (`core.rs:2710-2718`).
- **Tests.** `h13_acknowledgements_bind_exact_records_in_order` (`core-tests:3708`) and `h24_finalization_boundary_candidate_terminal_and_final` (`core-tests:6031`).

**Consequence [P].** Provided acknowledgement implies durability, every action that may have started natively has a durable `ActionStarted`, and every durable record has a durable predecessor. §4.3 makes acknowledgement imply durability.

### 2.4 Request sequencing and retained receipts

Implemented [S]:
- `Control::submit` queues one request (`core.rs:616-623`).
- `Custody::serve` → `sequence` → `execute` (`core.rs:2722-2785`).
- Receipts live in the in-memory `RequestGate` and are bounded by `receipt_limit` (`core.rs:1148-1171`).
- Another generation's request is `Foreign` (`core.rs:2745-2747`).
- Receipt digests are canonical. This is verified by `c17_core_retains_the_canonical_request_digest_in_its_receipts` (`codec-tests:2252`) and `h19_replayed_evicted_conflicting_and_foreign_requests_execute_nothing` (`core-tests:5427`).

Proposed [P]: receipts are not made durable.
- A request is bound to one generation.
- A restart creates a new generation, so no old request can execute against it.
- Within a live process the in-memory gate is authoritative.

Peer authentication stays outside the core (`mod.rs:152-154`) and outside this design.

### 2.5 Prior incidents and disposition validation

Implemented [S]:
- `PriorIncident { binding: IncidentBinding, outcome: PriorOutcome }` (`model.rs:683-702`). Its outcomes are `Resolved`, `Unresolved { outstanding }` and `Malformed`.
- `Custody::new` takes the list and refuses more than `incident_limit` entries with `Refusal::Capacity` (`core.rs:1292-1304`). `Config::LIVE.incident_limit` is 16.
- `Prior::blocks`: an outcome other than `Resolved` with no disposition (`core.rs:1207-1209`).
- `apply_disposition`, only while NotStarted (`core.rs:1410-1447`):
  - it asks the validator about exactly one binding;
  - a panicking or misbound answer is refused.
- `start_run` refuses with `PriorUnresolved` while any prior blocks (`core.rs:1463-1465`). `RunStarted { dispositioned }` records only the count (`core.rs:1476-1482`).
- `ValidatedDisposition::new` exists for validators only. The disposition reasons are OwnerDestroyed, HostRebooted, RecordsMalformed, CompletionNotRecorded and Other (`model.rs:705-730`).
- Test: `h20_prior_incidents_need_a_bound_external_disposition` (`core-tests:5537`).

Proposed [P]:
- The store computes the bindings (§6.3).
- The store implements `DispositionValidator` over root-owned files (§6.4).
- The store decides which priors are still current, using the applied-history rule (§6.4).

### 2.6 Terminal commitment, final acknowledgement and closure refusal

Implemented [S]:
- `Shared::commit` makes the commitment under one gate guard (`core.rs:473-482`).
- `try_finalize` and `note_finalized` (`core.rs:2662-2697`).
- `shutdown_decision` and `unresolved_now` (`core.rs:2872-2924`) refuse closure for:
  - an active or recovering run;
  - a completion candidate;
  - a pending terminal record;
  - held owners;
  - late incidents;
  - a pending or unknown operation;
  - lost authority;
  - pending evidence;
  - failed evidence.
- `close` returns `Closed { terminal, verdict, resolved_by, records, … }` only when nothing is unresolved (`core.rs:2929-2954`, `Closed` at `core.rs:317-335`).
- Version 1 has no record kind for closure (`model.rs:395-449`). Clean closure is therefore not in the record stream.
- Tests: `r03_failed_evidence_keeps_the_custody_open` (`core-tests:889`), `h15_physical_cleanup_and_recording_failure_stay_distinct` (`core-tests:3899`), `h04_shutdown_is_refused_while_native_or_evidence_state_is_unresolved` (`core-tests:2595`).

Proposed [P]: after `close` succeeds, the recorder publishes a **closure seal** outside the version-1 frames (§4.8). That seal is the only durable trace of clean closure.

### 2.7 Codec facts the store relies on

Implemented [S]:
- The frame is magic `NXCD`, domain `52` for records, version `01`, a u16 payload length `L`, the payload, and a SHA-256 trailer over bytes `0 .. 8 + L` (`codec.rs:22-31`).
- `MAX_FRAME` 4096 (`codec.rs:110`).
- `encode_record` refuses an intent whose digest is not its fields' (`codec.rs:230-236`).
- `decode_record` accepts exactly one complete, consistent frame, checks lengths before reading, allocates nothing and returns plain data (`codec.rs:238-324`, envelope at `codec.rs:496-544`).
- The codec checks syntax, never lifecycle (`codec.rs:75-79`).
- A matching digest is integrity, not authentication (`codec.rs:7-14`).
- Tests: `c11_core_issued_records_encode_decode_and_check` (`codec-tests:1786`) and `c12_core_refuses_acknowledgements_for_altered_bytes` (`codec-tests:1815`).

### 2.8 Threads and locks

Implemented [S]:
- `Custody` belongs to one execution-owner thread. `Control` is the only shared part, and each of its locks is taken alone and held for a copy or an assignment (`core.rs:1-11`, `Shared` at `core.rs:431-442`).
- Adapter callbacks are contained (`contained`, `core.rs:377-385`).
- With a stalled recorder `submit`, status, renewal and submission stay responsive: `h08_stalled_adapters_do_not_block_the_control_side` (`core-tests:3037`, recorder part at `core-tests:3113-3158`).
- A cancellation accepted while the recorder is paused precedes the commitment: `r10_cancellation_while_the_recorder_is_paused_precedes_the_commitment` (`core-tests:1541`).

### 2.9 Fixtures at the base

The test stand-ins are `Journal` (an in-memory sink), `StallingSink`, `PanickingSink` and `Validator` (`core-tests:256-312`). None of them stores anything. Each test acknowledges records itself (`core-tests:253-254`).

### 2.10 Not implemented at the base

The following do not exist at the base:
- storage;
- a journal;
- a startup scan;
- a disposition writer or reader;
- provisioning;
- an owner process;
- a transport.

`mod.rs` says so explicitly in its "Integration obligations" (`mod.rs:137-171`).

### 2.11 API change candidates (not made, not assumed approved)

The baseline needs **no** change to `core.rs`, `model.rs` or `codec.rs`. Every adapter point it uses is public:

- `RecordSink`;
- `acknowledge` and `record_failed`;
- `RecordAck::of`;
- `codec::encode_record` and `codec::decode_record`;
- `PriorIncident`, `IncidentBinding::new` and `ValidatedDisposition::new`;
- `DispositionValidator`;
- `Custody::new` and `Generation::new`.

Candidates for a later, separately approved mission:

| Id | Candidate | Why it might be wanted | Baseline without it |
|---|---|---|---|
| API-1 | A record kind for clean closure and for failure detail, in a new record version | Durable failure detail and closure in-band | The closure seal (§4.8). Failure detail beyond the version-1 kinds is not durable (§7.3). |
| API-2 | A way to construct a custody whose priors are already dispositioned, or a higher `incident_limit` | Liveness with more than 16 current incidents | The applied-history rule and Owner archival (§6.4). `Refusal::Capacity` otherwise, which fails closed. |
| API-3 | `RecordSink::submit` returning a result | An explicit refusal from a poisoned recorder | The sink accepts; the failure arrives through `record_failed` (§4.5). |
| API-4 | The next unacknowledged `RecordId` in `EvidenceView` | Convenience | The sink keeps the ids it submitted. |

---

## 3. Storage and authority model (B)

### 3.1 Actors and authority

| Actor | Identity | May write | Authority it holds |
|---|---|---|---|
| **Owner** (a human, as root) | uid 0 | `PROVISION`, the state-root directories, the pool files' creation, dispositions, the archive | Provisioning, disposition, archival, retirement of history. Every such act is an explicit, root-owned artifact. |
| **Recorder** (one thread in the custody owner process) | the store uid (for example the runner's) | Only the bytes of the pool file it claimed, and the store `LOCK` (by locking it, never writing it) | None. It produces evidence and reports durability. |
| **Startup scan, disposition reader, read-only status** (in the custody owner process) | the store uid | Nothing | None. They compute data. |
| **Any other process of the store uid**, including job code | the store uid | Whatever that uid can write (§5.6) | None that the store recognizes |
| **Core** (`Custody`) | in process | Nothing on disk | Native ownership, admission and verdicts, in memory (`mod.rs:1-21`) [S] |

### 3.2 Provisioning versus ordinary opening

**Provisioning** [P][D] is an explicit, later act of the Owner as root. It is **not performed in this mission**, and no runtime path ever runs it. Its required effects:

1. **Choose the root.** Pick `<STATE_ROOT>` on a local ext4 or XFS filesystem (§3.8). Every ancestor of it must be root-owned and not group- or other-writable.
2. **Create the root directory.** `<STATE_ROOT>` is `root:root 0755`.
3. **Create the subdirectories.** `journals/`, `dispositions/` and `archive/` are each `root:root 0755`.
4. **Create the lock.** `LOCK` is `<uid>:<gid> 0600` and empty.
5. **Create the pool.** For each `i` in `0 .. N`, create `journals/j<iiiii>.journal` as `<uid>:<gid> 0600`, then:
   - allocate `4096 + C × 128` bytes with `fallocate` mode 0 (EOPNOTSUPP or ENOSPC fails provisioning);
   - write zeros over the whole file;
   - `fsync` it.
   `N` and `C` are in §3.8.
6. **Sync the directories.** `fsync` each created directory and its parent, so their entries are durable (`fsync(2)`) [U].
7. **Generate the root id.** Draw a random 16-byte root id.
8. **Write `PROVISION`.** Write it (§3.3) as `root:root 0444` at `<PROVISION_PATH>`:
   - write it to a temporary name in the same directory;
   - rename it into place;
   - `fsync` the directory.
9. **Verify.** As the store uid, run the read-only verifier (§8.7). Keep its output.

**Opening** [P] is what every custody owner process does. It validates (§3.4). It **never** creates, chmods, chowns, renames, truncates or deletes any store component. It never falls back to another location. It never "repairs".

Anything missing or different is a refusal with a reason (§3.5). So is an extra entry in `<STATE_ROOT>`, `journals/` or `archive/` (§3.4). In `dispositions/` only names of the form `<64 hex>.disposition` are ever read, so other names, such as the Owner's temporary files, are ignored (§6.4). The trust-root rule of AGENTS §6.3 applies literally.

`<PROVISION_PATH>` and `<STATE_ROOT>` are placeholders [D]:
- The proposed form of `<PROVISION_PATH>` is a fixed, compiled-in path such as `/etc/nexus-os/phase2-custody/uid-<uid>.provision`.
- `<STATE_ROOT>` is the Owner's choice, recorded in `PROVISION`. A form such as `/var/lib/nexus-os/phase2-custody/uid-<uid>` would satisfy the ancestor rule.

**This mission selects neither.**

### 3.3 Layout

```
<PROVISION_PATH>            root:root 0444  identity and parameters (text, exact format)
<STATE_ROOT>/               root:root 0755
  LOCK                      uid:gid   0600  the store writer lock (flock), never written
  journals/                 root:root 0755  names fixed by root; contents owned by the uid
    j00000.journal          uid:gid   0600  pool file 0, preallocated and zero-filled
    …
    j<N-1>.journal          uid:gid   0600
  dispositions/             root:root 0755
    <64 hex>.disposition    root:root 0444  one per dispositioned incident (text)
  archive/                  root:root 0755  journals the Owner retired from the pool [D]
    <claim>-<32 hex>.journal  root:root 0444
```

Why the directories are root-owned:
- The store uid cannot create, delete or rename any entry in `<STATE_ROOT>`, `journals/`, `dispositions/` or `archive/`. Removing or renaming an entry needs write permission on its directory.
- So an accidental `rm` of a journal by the store uid fails.
- The uid can still overwrite or truncate the **contents** of its own pool files. Truncation is detected through the exact size check (§5.5). Rewriting is the same-uid limit (§5.6).

`PROVISION` is ASCII text: LF line endings, the exact field order below, every field exactly once, a final newline, at most 64 KiB. Every value is printable ASCII (`0x20`–`0x7e`) within its bound; any other byte, a missing, repeated, reordered or unknown field, or trailing data invalidates the file. Nothing is unescaped or normalized. It contains:

```
nexus-phase2-custody-provision 1
uid=<dec>
root-id=<32 hex>
state-root=<absolute path, printable ASCII, no spaces>
root-inode=<dec>
lock-inode=<dec>
journals-inode=<dec>
dispositions-inode=<dec>
archive-inode=<dec>
pool=<N>
pool-capacity=<C>
pool-00000-inode=<dec>
…  (one line per pool file, in index order)
retired-through=<claim dec>
predecessor=<none | 32 hex>
predecessor-statement=<printable ASCII, at most 512 characters>
operator=<printable ASCII, at most 64 characters>
created=<UTC, RFC 3339>
digest=<64 hex: SHA-256 of every preceding byte>
```

**Its digest is integrity only.** `PROVISION`'s authority comes from root ownership at a root-controlled path, never from its content or its existence.

### 3.4 Opening and validation procedure

All descriptors are opened `O_CLOEXEC` and kept for the life of the process. Every later operation is relative to a kept descriptor; no path is resolved twice (§3.6).

1. **`PROVISION`.**
   - Resolve `<PROVISION_PATH>` from `/` with `openat2(RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS)`.
   - Every ancestor must be a root-owned directory that is not group- or other-writable.
   - The file itself must be:
     - a regular file with `st_nlink == 1`;
     - owned by uid 0;
     - mode `0444` or `0644`;
     - at most 64 KiB, in the exact format, with its digest valid;
     - `uid` must equal `getuid()`.
2. **`<STATE_ROOT>`.**
   - Resolve the same way. The ancestor rule applies.
   - The directory itself must be owned by uid 0, mode exactly `0755`, with `st_ino == root-inode`.
   - `fstatfs`:
     - `f_type` must be `EXT4_SUPER_MAGIC` or `XFS_SUPER_MAGIC`;
     - `ST_RDONLY` must be clear;
     - anything else is "unsupported filesystem" (`statfs(2)`) [U].
   - Read `/proc/sys/fs/protected_hardlinks` and `/proc/sys/fs/protected_symlinks` as data. A value other than 1 is a refusal [D].
   - Read the mount's options from `/proc/self/mountinfo` as data. `nobarrier` or `barrier=0` is a refusal [D].
3. **Children.** Open each relative to the root descriptor with `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS | RESOLVE_NO_XDEV)` and `O_NOFOLLOW`. Then check type, owner, exact mode, `st_nlink == 1` for regular files, inode against `PROVISION`, and `st_dev` equal to the root's:

   | Child | Type | Owner | Mode | Other checks |
   |---|---|---|---|---|
   | `LOCK` | regular | the uid | `0600` | |
   | `journals/` | directory | root | `0755` | |
   | `dispositions/` | directory | root | `0755` | |
   | `archive/` | directory | root | `0755` | |
   | each pool file | regular | the uid | `0600` | size exactly `4096 + C × 128` |

   In `<STATE_ROOT>`, any entry other than `LOCK`, `journals/`, `dispositions/` and `archive/` is a refusal. In `journals/`, an entry the provisioning did not create is a refusal. Only root could have added either.

   In `archive/`, every entry must be a regular file named `<claim>-<32 hex>.journal`, owned by root, mode `0444`, `st_nlink == 1`, on the root's device. Its header's claim and generation must equal its name. Anything else is a refusal, because only root writes there.
4. **Lock.** `flock(LOCK, LOCK_EX | LOCK_NB)`. `EWOULDBLOCK` means Busy, a refusal. There is one attempt; any retry belongs to the client.
5. **Scan.** Run the startup scan (§5.2).

### 3.5 Missing root versus lost or corrupted prior state

| Condition | Store state | New execution | Resolution (authority) |
|---|---|---|---|
| `<PROVISION_PATH>` absent | **Unprovisioned** | refused | Owner provisioning (§3.2). Never automatic. |
| `PROVISION` present but invalid (owner, mode, links, format, digest, uid) | **Invalid** | refused | Owner repair, recorded in a new `PROVISION` |
| `<STATE_ROOT>`, a subdirectory, `LOCK` or a pool file missing, or its inode differs from `PROVISION` | **Lost or replaced** | refused | The Owner decides. Re-provisioning creates a new root id with `predecessor=<old id>` and a root-owned statement that the prior state was lost. That statement is the only way history is ever "forgotten". |
| A restored backup (inode numbers differ) | **Lost or replaced** (rollback evidence) | refused | As above |
| Unsupported filesystem, read-only mount, `nobarrier`, link protections off | **Unsupported** | refused | Owner fixes the host or re-provisions |
| Valid store, a pool file's content invalid | the store is valid; that file is a **Malformed** incident (§5.3) | refused until dispositioned | Root-owned disposition (§6.4) |
| Valid store, a claim number missing between two used claims | a **claim-gap** incident (§5.5) | refused until dispositioned | Root-owned disposition |
| Valid store, all pool files unused | **Fresh** | permitted | |

A **missing** store is never treated as fresh. Fresh means a provisioned, verified store whose pool files are all unused.

### 3.6 Identity: symlinks, hard links, replacement, ownership, permissions

- **Symlinks.** Absolute resolution uses `RESOLVE_NO_SYMLINKS` for every component. Opens under kept descriptors add `O_NOFOLLOW` and `RESOLVE_BENEATH`. Any symbolic link is a refusal (`openat2(2)`, `open(2)`) [U].
- **Hard links.** Every regular store file must have `st_nlink == 1`. With `protected_hardlinks = 1`, the uid cannot link root-owned files it cannot write (`proc_sys_fs(5)`) [U]. The check is required because the kernel default is 0.
- **Replacement.**
  - Identity is bound by inode number in root-owned `PROVISION`.
  - Entries live in root-owned directories, so the uid cannot replace them.
  - Inode numbers are unique only within one filesystem (`inode(7)`) [U], so `st_dev` must equal the root's at runtime. Device numbers are not stored, because they can change across boots.
  - Before every acknowledgement, the recorder rechecks:
    - its claimed journal's `fstat`: `st_nlink == 1` and the size unchanged;
    - `fstatat(journals_fd, name, AT_SYMLINK_NOFOLLOW)`: the same inode.

    A mismatch poisons the journal (§4.5).
- **Ownership and permissions.** These are exact checks, not "at least". POSIX ACLs cannot widen access past the mode check: the group bits equal `ACL_MASK`, which caps every named entry (`acl(5)`) [U]. So "no group or other write bit" bounds ACL grants too.
- **Retained descriptors.** Store paths are resolved once at opening. Pool files, `LOCK`, `dispositions/` and `archive/` are then used only through kept descriptors and `*at` calls.

### 3.7 Concurrent writers and stale instances

- **Single writer per store.** An exclusive `flock` on `LOCK` is held for the life of the owner process, through a descriptor opened `O_CLOEXEC`, so a helper never inherits it across `execve` (`flock(2)`) [U]. A second owner gets `EWOULDBLOCK` and refuses (Busy).
- **Single writer per journal (defense in depth).** The recorder holds `LOCK_EX` on the pool file it claimed. The startup scan takes `LOCK_SH | LOCK_NB` on every used pool file. Failure means another writer is live, and startup refuses. This covers a writer that lost its store lock through a bug.
- **Stale instance.** A live old owner (hung, recovering or holding natives) keeps both locks, so every new owner is refused. **Nothing ever steals, breaks or times out a lock**, and nothing uses a PID file.

  A hung owner ends only by administrative destruction, which is authority loss (§6.6). A process blocked in uninterruptible I/O may not exit until that I/O completes; until then its lock stays held and the store stays Busy (fail closed).
- **Advisory scope.** `flock` is advisory (`flock(2)`) [U]. It coordinates custody owners; it does not stop a hostile same-uid process (§5.6).

### 3.8 Capacity: memory, disk and reservation

| Bound | Value | Status |
|---|---|---|
| Pool files `N` | 256 | [D] |
| Pool capacity `C` (slots per file) | 512, equal to `Config::LIVE.record_capacity`. A claim requires `config.record_capacity ≤ C`. | [D] |
| Slot | 128 bytes, which is at least the largest version-1 record frame (123 bytes) | [P] |
| Pool file | `4096 + 512 × 128` = 69,632 bytes; the pool totals 17,825,792 bytes | [P] |
| Applied dispositions per journal header | at most 32 (`Config::LIVE.incident_limit` is 16) | [P] |
| Disposition file | at most 4096 bytes; statement at most 512 characters, operator at most 64 | [P] |
| `PROVISION` | at most 64 KiB | [P] |
| Archive files scanned | at most 4096; beyond that the Owner raises `retired-through` | [D] |
| Current (non-history) incidents | at most `incident_limit`, else `Custody::new` refuses with `Refusal::Capacity` (`core.rs:1298`) [S] | [S][P] |
| Recorder queue and event channel | `record_capacity + 2` entries each | [P] |
| Scan memory | One journal at a time (at most 69,632 bytes), plus per-journal summaries with lists capped at 16 entries and counts | [P] |

**Supported filesystem profile [A].**
- `<STATE_ROOT>` is on a local ext4 or XFS filesystem: not tmpfs, overlayfs, NFS, FUSE, btrfs or another copy-on-write filesystem.
- It is mounted read-write with write barriers (the ext4 default, `ext4(5)`) [U].
- The device honours cache-flush requests.

Two points are documented [U]:
- `fallocate` mode 0 guarantees that later writes into the range do not fail for lack of space, and a filesystem without the operation returns EOPNOTSUPP (`fallocate(2)`). The page does not list which filesystems support mode 0, so provisioning relies on the call succeeding on the chosen filesystem and fails otherwise.
- Copy-on-write filesystems cannot overwrite in place (`sync_file_range(2)` warning).

The other filesystems are refused because this design has not established their allocation and flush semantics. Device honesty cannot be checked by software.

### 3.9 What is never authority

- A path that exists.
- `$HOME`, cwd or any temporary directory.
- The root id, a generation, a claim number, a boot id or any UUID as such.
- A decoded frame, a seal, a header field or an "applied" list.
- A PID, unit name, cgroup path or socket name. None appears in any record.
- A disposition owned by anyone but root.
- A lock held by someone. A lock's absence proves only that no cooperating writer holds it.

---

## 4. Durable acknowledgement protocol (C)

### 4.1 Journal geometry and formats

All three structures are fixed-width and big-endian, using the conventions of the codec (`codec.rs:33-38`) [S]. Version-1 record frames are stored **unchanged**: the slot holds exactly the bytes `codec::encode_record` returned, followed by zero padding.

**Journal header**, at offset 0 (the first 4096 bytes are the header block):

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | magic `NXCJ` |
| 4 | 1 | container version `01` |
| 5 | 1 | record frame domain `52` |
| 6 | 1 | record frame version `01` |
| 7 | 1 | `00` |
| 8 | 16 | root id (equal to `PROVISION`) |
| 24 | 16 | generation |
| 40 | 8 | claim number (from 1, strictly increasing per store) |
| 48 | 4 | slot size, 128 |
| 52 | 4 | capacity (this generation's `record_capacity`, at most `C`) |
| 56 | 8 | created at (Unix milliseconds, `CLOCK_REALTIME`; data) |
| 64 | 16 | boot id (`random(4)`; data) |
| 80 | 1 | applied dispositions `n`, at most 32 |
| 81 | 33 × n | `n` × (binding 32 bytes, reason tag 1 byte) |
| 81 + 33n | 32 | SHA-256 of bytes `0 .. 81 + 33n` |

Bytes after the header digest up to 2048 must be zero. The disposition reason tags are `01` OwnerDestroyed, `02` HostRebooted, `03` RecordsMalformed, `04` CompletionNotRecorded and `05` Other (`model.rs:705-711`) [S].

**Closure seal**, at offset 2048 (written only after a successful `close`, §4.8):

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | magic `NXCS` |
| 4 | 1 | version `01` |
| 5 | 3 | `00 00 00` |
| 8 | 16 | generation |
| 24 | 8 | records (`Closed.records`, which equals the last sequence) |
| 32 | 32 | digest of the last record (zeros if none) |
| 64 | 8 | terminal record sequence (0 if none) |
| 72 | 1 | verdict tag, as in the codec (`01` Pending, `02` Passed, `03` Failed) |
| 73 | 1 | `resolved_by` present (`00` or `01`) |
| 74 | 4 | `resolved_by` (zero if absent) |
| 78 | 8 | closed at (Unix milliseconds; data) |
| 86 | 32 | SHA-256 of seal bytes `0 .. 86` |

Bytes 2176 to 4096 must be zero.

**Slot `n`** (record sequence `n`, from 1) lies at offset `4096 + (n − 1) × 128`. It holds the record's version-1 frame (`8 + L + 32` bytes, with `L ≤ 88` so that it fits) followed by zeros. An all-zero slot is unwritten. Slots beyond the header's capacity must stay zero.

### 4.2 Claim (generation initialization)

The owner process performs these steps after opening and scanning (§5.2), and after `Custody::new` and every `apply_disposition`. It claims only if no prior still blocks (checked from the core's own snapshot, `PriorView`, `model.rs:860-865`) [S]. A refused startup therefore leaves no trace in the pool.

1. **Choose.** The recorder thread picks the lowest-index pool file the scan classified Unused. If none is, the pool is exhausted and the claim is refused (§5.4).
2. **Lock and recheck.** Take `flock(LOCK_EX | LOCK_NB)` on that file through the recorder's own descriptor, then re-read it. It must still be all zero; otherwise the claim is refused.
3. **Write the header.**
   - Claim number = 1 + the largest claim among valid headers in the pool and archive (at least `retired-through + 1`).
   - Generation: the one the owner drew with `getrandom` before `Custody::new`. The owner checks it against every header the scan saw and draws again on a collision, so it is unique among headers (§5.5).
   - Capacity, created-at, boot id, and the bindings and reasons of the dispositions this custody applied. More than 32 applied dispositions refuses the claim.
   - The header is written with `pwrite` (loop on short writes).
4. **Sync.** `fdatasync`. The blocks were allocated and zero-filled at provisioning, so this is an overwrite with no size change (`fsync(2)`) [U].
5. **Report.** On success, report `Claimed`. Only then does the owner call `start_run`. On any error, report `ClaimFailed`; the owner does not start the run, closes the custody (NotStarted, nothing held) and exits.

### 4.3 Append: from `RecordIntent` to acknowledgement

| Step | Thread | Action | Effect if it fails |
|---|---|---|---|
| A1 | owner | `flush_records(&mut sink, now)` (`core.rs:2958`) [S]. `submit` pushes the intent onto the recorder queue. | Nothing durable; the record is still unacknowledged |
| A2 | recorder | Check the intent: generation equals the journal's; sequence equals the next slot; sequence ≤ capacity. Then `encode_record(intent)` must succeed and be at most 128 bytes. | Poison (§4.5) |
| A3 | recorder | Publish status "writing *n* since *t*" (copy-only lock, §7.1) | — |
| A4 | recorder | `pwrite` frame plus zero padding to slot *n*, looping on short counts and EINTR until all 128 bytes are written | Poison |
| A5 | recorder | `fdatasync(journal)`. EINTR is retried a bounded number of times; any other result is final. | Poison. **No retry after an error** (§4.5). |
| A6 | recorder | Identity recheck (§3.6) | Poison |
| A7 | recorder | Mark *n* durable: keep its digest, advance the next slot, publish status "durable through *n*" | — |
| A8 | recorder | Send `Durable(RecordAck::of(intent))` on the event channel | — |
| A9 | owner | At its next safe point, `acknowledge(&ack, now)` (`core.rs:2996`) [S]. `Acknowledged` (or `Duplicate`) is expected. | Integration fault (§4.5) |

**File and directory durability [P].**
- **File.** A record, the claim header and the seal are each made durable by `fdatasync` on the journal. No runtime write changes a file's size or allocation: every byte was allocated and written at provisioning. So no metadata beyond what data retrieval needs is involved (`fsync(2)`) [U].
- **Directory.** No runtime operation creates, renames or removes a directory entry, so no runtime directory sync is needed. Provisioning and every Owner procedure must `fsync` each directory they change, because a file's `fsync` does not make its directory entry durable (`fsync(2)`) [U].

**Ordering invariant [P].** Record *n* is written only after record *n − 1*'s sync returned success. A durable record therefore always has a durable predecessor (crash-only, honest storage), and the durable journal is always a prefix.

With §2.3 this gives write-ahead:
- an action admitted natively has a durable `ActionStarted`;
- a durable record never depends on a later one.

### 4.4 Where acknowledgement is prohibited

No `Durable` event is ever sent:

- before A5 returns success for that record's slot;
- after any error in A2 to A6 for that record or an earlier one in this process;
- for a sequence other than the next slot;
- for another generation's record, or one this recorder did not write in this process;
- for a record read back from storage, including at startup. A restart never acknowledges anything: its custody is a new generation, and the old generation's ledger died with its process;
- because a timeout expired, a status read happened, or a later sync returned success;
- for a record whose frame does not fit a slot or whose encoding failed.

### 4.5 Errors, short writes, interruption, capacity, uncertainty, duplicates

**Poisoning.** The first failure in A2 to A6 poisons the journal for the life of the process. Poisoning means:

- **Report.** The recorder sends `Failed(id of the first record not known durable)`. That is exactly the core's next unacknowledged record, because events are FIFO and every earlier record was acknowledged before it.
- **Apply.** The owner calls `record_failed(id, now)` (`core.rs:3016`) [S] and expects `FailureRecorded`. The core:
  - records a fail-stop `RecordFailed` failure;
  - refuses every later acknowledgement (`LedgerFailed`);
  - never issues a terminal record;
  - keeps closure refused for good (`core.rs:3012-3015`).
- **Stop.** The recorder writes nothing more. Later submissions are drained and ignored, and the status shows the poisoned sequence and the error class.

The individual cases:

| Case | Handling |
|---|---|
| **Short write** | `pwrite` continues at the remaining offset. Partial writes are possible (`write(2)`) [U]. Only a complete 128-byte slot counts as written. |
| **Interrupted operation** | EINTR before any byte is written is retried a bounded number of times (`write(2)`) [U]. The owner process installs no signal handlers, so EINTR is not expected [P]. |
| **Capacity exhaustion** | The core refuses before issuing (`EvidenceCapacity`, §2.2). A sequence beyond the header's capacity poisons. ENOSPC or EDQUOT on a slot write or sync cannot come from the space reservation (§4.6); if it occurs anyway, it is an error and poisons. |
| **I/O error** | EIO from `pwrite` or `fdatasync` poisons. |
| **Uncertain persistence** | After a sync error the page cache's state is unknown. A later sync may return 0 although the data never reached storage. The kernel documentation (errseq, vfs) [U] reports a writeback error once per file description and later calls return 0; PostgreSQL's `data_sync_retry` documentation describes the same hazard. So the recorder never retries a failed sync and never acknowledges after one. The record may or may not be on disk. Startup treats whatever is there as evidence, which over-approximates (§5.4). |
| **Duplicate delivery** | A sink panic leaves the record unsent, and the core resubmits it on the next flush (`core.rs:2956-2985`) [S]. A submission with a sequence below the next slot and the same digest as that slot is re-acknowledged; the core answers `Duplicate`, which is harmless. A different digest, a sequence above the next slot, or another generation poisons. |
| **Recorder thread death** | A panic or exit of the recorder thread disconnects the queue. The sink records "recorder lost" instead of panicking. The owner reports `record_failed` for its next unacknowledged record, whose id it holds from the intents it submitted. The core's panic containment at `submit` stays as defense in depth (`core.rs:2974-2985`) [S]. |
| **Integration fault** | `acknowledge` or `record_failed` returning anything unexpected for the owner's own recorder event indicates a bug or in-process tampering. The owner stops flushing, keeps the custody (natives stay held, closure stays refused because records are pending), and reports a custody fault. |

### 4.6 Logical versus physical reservation

**The core reserves logically.** Its reservation is a count of records: all-or-nothing reservations against `record_capacity` (§2.2), made before anything native exists. **It says nothing about bytes on a disk.**

**The mechanism that makes every logical reservation physically satisfiable** [P]:

1. **Space reserved at provisioning.** Each pool file is allocated with `fallocate` mode 0, after which "subsequent writes into the range … are guaranteed not to fail because of lack of disk space" (`fallocate(2)`) [U]. It is then **zero-filled and synced**. Every later slot or header write is therefore a pure overwrite of written blocks, needing no block-allocator call; `sync_file_range(2)` warns that writes into merely preallocated space may need one [U].
2. **Capacity fits.** A claim requires `config.record_capacity ≤ C`, and the header records this generation's capacity.
3. **One slot per record.** The core issues at most `record_capacity` records per generation (§2.2). Record sequence *n* maps to the one slot *n*. The slot size of 128 bytes is at least the largest version-1 record frame (123 bytes, §2.2). So every record the core can ever issue has a slot that already exists, allocated and written, before the generation is claimed.
4. **Defense in depth.** If the core issued more anyway (a bug), sequence *n* > capacity poisons the journal: fail closed.

Therefore the core's logical capacity is at most the physical slots. This holds by the mechanism above, not by equating the two.

**Residual.** The guarantee covers space only. Device errors, a lost device, or an unsupported filesystem that broke the overwrite assumption still end in `record_failed` or a refusal, never in an acknowledgement. Copy-on-write filesystems are refused precisely because they break assumption 1.

### 4.7 Terminal record

`RunEnded` is an ordinary append:
- The core issues it at the commitment and makes the run final only on its acknowledgement (§2.6).
- A failed `RunEnded` write is `record_failed`. The run never becomes final, closure stays refused, and the owner process keeps holding what it holds (`mod.rs:139-143`) [S].
- After a crash, a durable `RunEnded` is reported as the recorded verdict. It does not by itself make the generation resolved (§5.3).

### 4.8 Closure seal (checkpoint publication)

1. **Close.** The owner calls `close(now)` (`core.rs:2929`) [S]. Only `Ok(Closed)` proceeds; it requires every record acknowledged and nothing held.
2. **Request the seal.** The owner sends `Seal` to the recorder, carrying `Closed`'s terminal, verdict, `resolved_by` and records. The recorder knows the last record's digest.
3. **Write.** The recorder writes the seal at offset 2048 (§4.1), then `fdatasync`. On error it reports `SealFailed`. Nothing is retried, and nothing is held natively any more.
4. **Exit.** The owner exits after `Sealed`. It waits with no deadline, because it holds nothing native. The status shows "sealing since *t*".

On `SealFailed` the owner reports the failure and exits; it holds nothing native. At the next startup that generation is unsealed. If it recorded native work it blocks until a disposition (for example `completion-not-recorded`), exactly as for a crash at S1 (§5.4).

A seal is published nowhere else in the baseline. An external anchor (§5.6) would publish the seal's content.

---

## 5. Startup, crash and restart (D)

### 5.1 Fault models

| Id | Model | What survives |
|---|---|---|
| **F1** | **Process termination**: SIGKILL, abort, OOM kill, administrative destruction | The kernel page cache survives. Every completed `pwrite` is visible to the next process and will normally reach storage, unless F2 follows first. A process in uninterruptible I/O may not terminate until that I/O completes. |
| **F2** | **Machine or power loss** | Only data a successful `fsync` or `fdatasync` covered (`fsync(2)`) [U], under the supported profile [A]. A write in flight may be absent, complete or, in principle, torn. |
| **F3** | **Storage corruption**: bit flips, misdirected or lost writes, a device that lies about flushing | Detection only, through digests and structure. A lost flushed write is undetectable without an anchor. |
| **F4** | **Adversarial rewriting or rollback** by any process with the store uid's write authority, including job code run as that uid | Root-owned files cannot be forged by that uid. Every uid-owned byte can be rewritten consistently, because there is no key (§5.6). |

### 5.2 Startup procedure

The owner thread runs this before `Custody::new`. It holds no native owner.

1. **Open and lock.** §3.4 steps 1 to 4. Any failure refuses, with its category.
2. **Read each file durably.** For each pool file and each archive entry:
   - `fsync` it first, failing closed on error;
   - for pool files, take `LOCK_SH | LOCK_NB` (failure means Live: refuse);
   - read its bytes;
   - classify (§5.3);
   - release the shared lock and close the descriptor.

   The store `LOCK`, held exclusively, keeps every other owner out from here to the claim, so the scan's shared locks are not needed afterwards. Releasing them lets the recorder take `LOCK_EX` on the file it claims.

   Syncing before reading means a decision is never made on page-cache state that a later power loss could undo (F1 followed by F2). The cost is small, and syncing a read-only descriptor flushes the file's dirty pages.
3. **Pool-level checks** (§5.5): claim contiguity above `retired-through`, duplicate generations, foreign roots.
4. **Partition the incidents.** Using the applied-history rule (§6.4), split incidents into **history** (not passed to the core) and **current**. More than `incident_limit` current incidents refuses (`Refusal::Capacity` is what `Custody::new` would return, `core.rs:1298`) [S].
5. **Report.** Produce the bounded report (§6.2) and the disposition reader (§6.4).
6. **Create the custody.** `Custody::new(config, generation, current incidents, now)`, then `apply_disposition` for each current incident. If any still blocks, do not claim: close the NotStarted custody and exit, reporting the refusal.
7. **Run.** Claim (§4.2), then `start_run`.

### 5.3 Classification (a pure function)

**Input:** a pool or archive file's bytes `B`, the root id, and the pool capacity `C`. **Output:** one of the classes below, plus a summary.

**Structure** (any failure is **Malformed**, with a code and an offset):

1. **Size.** `len(B) == 4096 + C × 128` for a pool file. An archive file keeps the geometry of the pool it came from: its size must be `4096 + m × 128` with `capacity ≤ m ≤ 4096`.
2. **Unused.** All zero means **Unused**.
3. **Header region** (bytes 0 to 2047; the seal lies at 2048).
   - All zero, but anything else non-zero: Malformed (`no-header`).
   - Non-zero and invalid, with every slot and the seal zero: **AbandonedClaim**. This is a torn or partial claim; it never had records, and the file is spent.
   - Non-zero and invalid, with anything else non-zero: Malformed (`header`).
4. **Header fields.**
   - Root id must equal `PROVISION` (`foreign-root`).
   - Slot size must be 128.
   - Capacity must be at most `C`.
   - `n` must be at most 32.
   - Padding must be zero.
   - Slots beyond capacity must be zero.
5. **Slots** `i = 1 ..= capacity`, in order:
   - The first all-zero slot ends the log. Every later slot must be zero (`hole`).
   - A non-zero slot must satisfy all of these:
     - `L` from the frame header satisfies `8 + L + 32 ≤ 128` (`slot-length`);
     - the padding after the frame is zero (`slot-padding`);
     - `decode_record(frame)` succeeds (`frame: CodecError`);
     - the frame's generation equals the header's (`foreign-generation`);
     - its sequence equals `i` (`slot-order`);
     - its `at` is at least the previous record's (`time-order`).
6. **Grammar.** The decoded prefix must satisfy the record grammar below (`grammar: rule, sequence`).
7. **Seal.**
   - Region zero: **unsealed**.
   - Non-zero and invalid: **seal unreadable**, treated as unsealed and reported.
   - Valid: it must match the prefix, or the file is Malformed (`seal-mismatch`):
     - same generation;
     - records equal the prefix length;
     - same last digest;
     - terminal sequence, verdict and `resolved_by` equal the `RunEnded` record, or "none" if there is none;
     - nothing outstanding.

**Record grammar** [P], derived from the core. Each rule names the code that guarantees it [S]:

| Rule | From |
|---|---|
| `RunStarted` at most once. Every `CaseStarted`, `ActionStarted`, `RecoveryRequired`, `RecoveryAttempt` and `RunEnded` follows it. Control facts may precede it. | `core.rs:1459-1488`, `core.rs:2374-2387` |
| `RunStarted.dispositioned` equals the header's `n` | `core.rs:1476-1482`; §4.2 |
| `CaseStarted` only with no case open, and never after `RecoveryRequired`, `RunEnded` or `Control{AdmissionClosed}`. `CaseEnded` closes the open case, with the same id. | `core.rs:1519-1566`, `core.rs:2157-2212` |
| `ActionStarted` only while a case is open, never after `Control{AdmissionClosed}`. Action numbers are this generation's, contiguous from 1. | `core.rs:1579-1620` |
| `ActionFailed` and `ActionSettled` refer to a started action, at most once each; `ActionFailed` comes before `ActionSettled` | `core.rs:1783-1808`, `core.rs:1989-2016`, `core.rs:2570-2607` |
| `IncidentOpened` numbers are this generation's, contiguous from 1, and name a started action. `IncidentSettled` refers to an opened incident, at most once, with `how` ∈ {Confirmed, OutputLost}. | `core.rs:1931-1987`, `core.rs:2570-2607` |
| `Control{AdmissionClosed}` at most once; `Control{ShutdownRefused}` at most once | `core.rs:2374-2387`, `core.rs:2856-2864` |
| `RecoveryRequired` after `RunStarted`, never with a case open, never after `RunEnded` | `core.rs:2624-2644`, `core.rs:1979-1982` |
| `RecoveryAttempt` numbers are contiguous from 1 | `core.rs:2801-2837` |
| `RunEnded` at most once, after `RunStarted`, with no case open. Every action started and every incident opened before it is settled before it. Its verdict is Passed or Failed. `resolved_by`, if present, names an earlier `RecoveryAttempt` with `resolved` true. | `core.rs:2662-2706`, `core.rs:2617-2619` |
| After `RunEnded` only `IncidentOpened`, `IncidentSettled`, `RecoveryAttempt` and `Control{ShutdownRefused}` occur | `core.rs:2801-2837`, `core.rs:1931-1987`, `core.rs:2846-2866` |

The grammar is a proposal checked against the code. The implementation must prove it by conformance (§8.3, invariant I9): every journal the real core produces in the existing deterministic runs must classify as non-Malformed.

**Outstanding set:** actions started and not settled (including failed and lost ones), plus incidents opened and not settled.

**Classes and the refusal rule** [P]:

| Class | Condition | Passed to the core as |
|---|---|---|
| Unused, AbandonedClaim | as above | not an incident (reported) |
| **Resolved (closed)** | valid seal matching the prefix | not passed |
| **Resolved (no native work)** | unsealed, and the prefix has no `ActionStarted` | not passed |
| **Unresolved** | unsealed, and the prefix has at least one `ActionStarted` | `PriorOutcome::Unresolved { outstanding }`, the outstanding-set size, possibly 0 |
| **Malformed** | any structure or grammar failure | `PriorOutcome::Malformed` |

**Why unsealed generations with native work block even with nothing outstanding [P][D].** The P2 contract proposed refusing only when work was outstanding. That rule misses three things durable records cannot rule out:

1. **A late owner.** A late owner exists before its `IncidentOpened` can be written (`core.rs:1924-1930`) [S]. A crash in that window leaves only a settled action on disk.
2. **A recording failure.** After one, closure is refused for good (`core.rs:3012-3015`) [S], so the durable prefix is incomplete by definition.
3. **Native effects after the last durable record**, which only the live owner could have known about.

A generation that recorded no `ActionStarted` admitted nothing natively (§2.3). A late owner also needs an admitted action. So the rule refuses exactly when native work was possible and clean closure was not proven.

The cost is a root disposition after every abnormal end of a run that did native work. **Alternative [D]:** the contract's outstanding-only rule.

### 5.4 Failure-point table

Notation:
- "≥1 AS": the prefix holds at least one `ActionStarted`.
- **Blocks**: new execution is refused until a root-owned disposition binds to that generation's incident (§6.4).
- **Permitted**: that generation does not block.

| Point | Possible durable state after F1 (process termination) | after F2 (power loss) | Startup concludes | New execution | Resolution |
|---|---|---|---|---|---|
| P1 provisioning in progress | partial root; no `PROVISION`, or partial | the same, or less | Unprovisioned or Invalid | refused | Owner completes provisioning and verifies |
| O1 open, scan, `Custody::new`, dispositions (no writes) | unchanged | unchanged | as before | as before | — |
| C1 pool file locked, header not written | unused | unused | Unused | permitted | — |
| C2 header written, not yet synced | header visible, synced by the next startup | absent (Unused), torn (AbandonedClaim) or complete | Unused, AbandonedClaim or Resolved (no native work) | permitted | — |
| C3 header durable, no record yet | header | header | Resolved (no native work) | permitted | — |
| A1 record *n* written, sync pending (not acknowledged) | *n* visible, synced by the next startup | *n* absent, complete or torn | prefix with or without *n*; torn *n* is Malformed | blocks if ≥1 AS or torn; otherwise permitted | disposition (`completion-not-recorded` or `host-rebooted`) |
| A2 *n* durable, acknowledgement not yet applied | *n* | *n* | *n* counted although the core never acted on it (for example, an `ActionStarted` never admitted): over-approximation | blocks if ≥1 AS | disposition; the report says admission was not proven |
| A3 *n* acknowledged; the core may have acted | *n* | *n* | write-ahead holds (§4.3) | blocks if ≥1 AS | disposition |
| A4 an append error, then `record_failed` (process alive) | — | — | live: the store is Busy | refused (Busy) | wait, or administrative destruction (authority loss) |
| A5 after A4, the process destroyed | prefix up to *n − 1*, possibly *n* | the same or shorter (all synced records survive) | unsealed | blocks if ≥1 AS | disposition |
| T1 `RunEnded` written or durable, not acknowledged | as A1/A2 | as A1/A2 | unsealed; the verdict is reported if present | blocks if ≥1 AS | disposition |
| T2 `RunEnded` acknowledged (Finalized), not closed | prefix with `RunEnded` | the same | unsealed; the recorded verdict is reported | blocks if ≥1 AS | disposition |
| S1 `close` succeeded, seal not written | full prefix, no seal | the same | unsealed: a documented false-positive window | blocks if ≥1 AS | disposition (`completion-not-recorded`) |
| S2 seal written, not synced | seal visible, synced by the next startup | absent, torn (unreadable) or complete | Resolved (closed), or unsealed | permitted, or blocks if ≥1 AS | disposition if it blocks |
| S3 seal durable | sealed | sealed | Resolved (closed) | permitted | — |
| D1–D4 Owner disposition (temporary file, `fsync`, rename to `<binding>.disposition`, `fsync` of the directory) | before the rename: no disposition (temporary names are ignored) | the same | the incident is current and undispositioned | blocks | Owner repeats |
| D5 a startup applied a disposition and claimed (header lists the binding) | header lists it | header lists it if synced | history (§6.4) while the disposition stays valid | permitted | — |
| R1–R4 Owner archival or recycling [D] (move, chown and chmod, recreate the pool file, rewrite `PROVISION`) | an interrupted procedure leaves an inode mismatch or a stray entry | the same | Invalid or Lost | refused | Owner completes the procedure |

**F3 and F4** do not fit the table. They are covered in §5.5 (detection) and §5.6 (limits).

### 5.5 Detection

| Manipulation or fault | Detected by | Result |
|---|---|---|
| **Incomplete frame** (torn, truncated, bit-flipped) | Frame length, zero padding, `decode_record` (digest, envelope) | Malformed |
| **Reordered records** | Slot index must equal the frame's sequence | Malformed |
| **Repeated record** | Sequence and slot mismatch; duplicate action or incident numbers in the grammar | Malformed |
| **Foreign generation** | Frame generation ≠ header generation; header generation duplicated across files | Malformed, for each file involved |
| **Foreign root** | Header root id ≠ `PROVISION` | Malformed |
| **Hole inside a journal** | A non-zero slot after a zero slot | Malformed |
| **Truncated or extended file** | Exact size check | Malformed |
| **Missing history: a whole journal deleted** | Impossible for the store uid (root-owned `journals/`). A missing or replaced pool file breaks the inode binding. | Store Lost or Replaced: refused |
| **Missing history: a journal zeroed by the uid while a later claim exists** | Claims must be contiguous over the pool and archive, above `retired-through` | claim-gap incident (Malformed): blocks |
| **Deleted suffix inside a journal** (trailing slots zeroed) | **Not detectable** without an anchor. A seal, if present and intact, exposes it for closed generations only (`seal-mismatch`). | §5.6 |
| **Deleted suffix at pool level** (the last claimed journal zeroed) | **Not detectable** without an anchor | §5.6 |
| **Rolled-back store** (restored backup) | Inode binding in `PROVISION` | Lost or Replaced: refused |
| **Lost flushed write** (lying device, F3) | Not detectable in general. It shows only if it breaks structure. | Assumption [A] |

No hash chain is added. Version-1 frames already carry their own sequence, and each frame has its own digest. A chain would add nothing against F4, because the attacker recomputes it, and it **cannot show that a suffix was not deleted**: a truncated chain is a valid chain.

### 5.6 Trusted anchor: what is required, and the options

**What the baseline (A0, no anchor) guarantees:**
- Under F1 and F2 with honest storage: acknowledged implies durable; write-ahead holds; startup never under-reports native work it could know of; every malformation it can see blocks.
- Under F3: corruption that breaks structure or digests is detected.
- Under F4: no lower-privileged process can forge a disposition, `PROVISION`, the archive, or root-owned directory entries.

**What A0 does not guarantee.** Any process able to write as the store uid can rewrite a journal's bytes consistently, so that an unresolved generation classifies as closed. This includes CI job code: on a self-hosted runner, job steps execute as the runner uid, which is also the custody owner's uid in the earlier contract's architecture. Such a process can also zero the last claimed journal, or delete a suffix of records. **Refusal is then lifted without a disposition.** The earlier contract's sentence "nothing a uid-1001 process writes can lift a refusal" holds here only for the disposition path, not for the evidence the classification reads. This design says so instead of claiming more.

**What would be required.** Detecting or preventing this needs an anchor: a monotonic record of what the store must contain, which the store uid cannot write. Options [D]:

| Option | Mechanism | Limits |
|---|---|---|
| **A1** root-written anchors | A privileged helper writes, at claim and at seal, a root-owned anchor record: claim, generation, header digest, then records and last digest. | A new privileged runtime component with its own attack surface. Records written between claim and seal are anchored only if anchored one by one, at a cost per record. |
| **A2** privileged append-only journals | A helper with `CAP_LINUX_IMMUTABLE` sets `FS_APPEND_FL` on each claimed journal (`ioctl_iflags(2)`). The file can then be opened only with `O_APPEND`, and `unlink(2)` fails with EPERM. | Needs an append-model journal: `pwrite` appends regardless of offset with `O_APPEND` (`pread(2)` BUGS), and preallocation with `FALLOC_FL_KEEP_SIZE`. A privileged helper per generation. The uid can still append garbage, which is detectable. Root can always rewrite. |
| **A3** external witness | Claim and seal checkpoints published to an external append-only service | A **cloud or network dependency**: availability, latency, the witness's own integrity, confidentiality. Detects rollback only relative to what was witnessed. |
| **A4** hardware monotonic counter (TPM NV) | The anchor high-water mark held in hardware | A privileged and hardware dependency; provisioning; device-specific behaviour |
| **A5** privilege separation | The store, and so the owner, run under a dedicated uid that differs from the uid executing job code | Account provisioning, cross-uid IPC, and a re-examination of native ownership: which user manager owns the scopes. An architecture change. |

None of A1 to A5 is introduced by this design. **Recommendation:**
- Implement A0, with its non-claim stated in every report, if the Architect accepts that the store is crash and accident evidence and not tamper evidence.
- If tamper resistance against job code is required, choose A5 or A2 before implementation. That changes §3 and §4.

---

## 6. Native custody and administrative disposition (E)

### 6.1 Evidence stays data

- **Nothing to reconstruct from.** Version-1 records carry identities, instants, kinds and outcomes only (`codec.rs:40-73`) [S]. They name no PID, unit, cgroup, scope, workspace path, socket or file. The container formats add only the root id, generation, claim, boot id, times and bindings (§4.1).
- **Restart never reconstructs ownership.** It never reopens, adopts, kills, removes or "cleans" anything named by a record. The core cannot either: decoding is plain data (`codec.rs:89-93`; `mod.rs:147-151`) [S].
- **The boot id is data.** A changed boot id may be **reported** ("host rebooted since this generation was claimed"). It never dispositions an incident automatically.

### 6.2 What the store reports

For each current incident, and for each history entry on request, the store reports the following. All fields are bounded, every byte outside printable ASCII is escaped (as the core's `bounded` escapes failure details, `core.rs:3171-3183`) [S], and lists are capped at 16 with counts.

- **Identity:** claim, generation, file index or archive name, class, and Malformed code with offset.
- **Header data:** created-at, boot id, and whether the boot id differs from the current one.
- **Prefix:** its length, and the last record's sequence, kind and instant.
- **Outstanding actions:** sequence, declared slot kind from `ActionStarted`, and whether `ActionFailed` is recorded.
- **Outstanding incidents:** sequence, retired action and declared kind.
- **Run summary:** started or not; cases started and ended; recovery required and attempts; the recorded verdict if `RunEnded` is durable; control facts present.
- **Seal state:** sealed, unsealed or unreadable.
- **Binding and disposition:** the binding (§6.3) and the disposition state (none, valid with reason, or invalid with why).

**It never reports** that anything is "clean", "gone", "safe" or "confirmed now".

### 6.3 Incident binding

`IncidentBinding` is SHA-256 over these fixed-width, big-endian bytes [P]:

1. ASCII `nexus-phase2-custody-incident`;
2. `01`, the binding version;
3. the kind: `01` journal, `02` claim gap, `03` pool file without a valid header but with content, `04` duplicate generation;
4. the root id (16 bytes);
5. the claim (u64, 0 if none);
6. the file index (u32, `ffffffff` for the archive or none);
7. the SHA-256 of the file's complete bytes (zeros if none);
8. the class: `01` unresolved, `02` malformed;
9. the outstanding count (u32);
10. the Malformed code (u16, 0 if none).

Because the whole file's digest is included, any later change to the incident's bytes produces a new binding. A disposition for the old binding then no longer validates, and the incident blocks again. A claim-gap binding covers only the missing claim number and the root id, so it stays stable as later generations are claimed.

### 6.4 Disposition

**Authority [D].** Only the Owner, as root, dispositions. The Owner decides out of band: for example after a reboot, after inspecting each listed resource, or by accepting the loss. The store **does not verify** the Owner's statement.

**Format** [P]. The file is `dispositions/<binding hex>.disposition`, ASCII, with exact lines, at most 4096 bytes. It follows the same strict text rules as `PROVISION` (§3.3):

```
nexus-phase2-custody-disposition 1
root=<32 hex>
binding=<64 hex>
claim=<dec>
generation=<32 hex | none>
file=<64 hex | none>
class=<unresolved | malformed>
outstanding=<dec>
reason=<owner-destroyed | host-rebooted | records-malformed | completion-not-recorded | other>
statement=<printable ASCII, at most 512 characters>
operator=<printable ASCII, at most 64 characters>
at=<UTC, RFC 3339>
```

**Validation** (inside `DispositionValidator::validate`, which runs on the owner thread within `apply_disposition`, `core.rs:1410-1447` [S]). The disposition is accepted only if every check holds:

- the file was opened by name from the kept `dispositions/` descriptor with `O_NOFOLLOW`;
- it is a regular file owned by uid 0, mode `0444` or `0644`, with `st_nlink == 1`;
- it has the exact format, and its name equals its binding;
- its root, binding, claim, generation, file digest, class and outstanding count equal the store's **current** computation for exactly the binding asked about.

The validator then returns `ValidatedDisposition::new(binding, reason)`. Anything else returns `None` and the incident keeps blocking. Only names matching `<64 hex>.disposition` are read; temporary installation names are ignored.

**Effect.** These are the core's semantics (`model.rs:713-718`) [S]:
- the incident no longer blocks this start;
- `RunStarted.dispositioned` counts it;
- the new journal's header lists its binding and reason (§4.2).

**It never turns into confirmed cleanup.** The report keeps showing "dispositioned: accepted without cleanup confirmation".

**Applied-history rule** [P]. A current incident becomes **history**, and is no longer passed to `Custody::new`, when both hold:

- a later valid journal header lists its binding (the custody that applied it recorded that before `start_run`);
- a valid root-owned disposition for that binding still exists.

Deleting the disposition (Owner revocation) makes the incident current and blocking again. Forging an "applied" list as the store uid gains nothing, because the root-owned disposition must still exist. This keeps the number of current incidents bounded (`incident_limit`, §3.8) without any core change.

**Archival and retirement [D].** These are Owner procedures, never automatic:
- **Archival.** The Owner may move a sealed or applied-history journal to `archive/`, owned by root with mode `0444`, recreate a fresh pool file, and rewrite `PROVISION`. Archived files are classified like pool files (§5.3); the archive only changes who can write them.
- **Retirement.** The Owner may raise `retired-through` to stop scanning old claims. That is a root-owned statement that the history at or below it no longer gates anything.

### 6.5 What a disposition, a seal or a settlement record never means

- A **valid stored cleanup claim**, such as `ActionSettled { how: Confirmed }` or `IncidentSettled`, is a past observation by a process that no longer exists. It is **not fresh proof** of native cleanup.
- A **seal** means the custody closed as recorded, at that time.
- A **disposition** means the Owner accepted the incident without cleanup confirmation.

None of them authorizes reopening, adopting or deleting anything, and none of them is reported as "confirmed".

### 6.6 No force-close

There is no API, flag, file or procedure that does any of the following:
- closes a custody while anything is unresolved;
- drops the last owner;
- marks a failure as cleared;
- removes a record;
- converts an unresolved generation into a resolved one.

Two consequences follow:
- **The only exit for a live custody** that cannot close is administrative destruction of its process (authority loss, F1). Its journal then blocks until a disposition.
- **The only way past a blocking incident** is a root-owned disposition. The Owner may also retire whole histories, but only through a root-owned statement that stays on record.

---

## 7. Responsiveness and failure retention (F)

### 7.1 Threads, queues and locks

| Thread | Owns | Never |
|---|---|---|
| **Owner** (execution owner) | `Custody`; the native owners; the submitted-intent ids | Waits on the recorder during a run. Takes the recorder status lock while doing I/O. |
| **Control** | `Control` (gate, lease, queue, snapshot); the control transport (out of scope) | Touches store files or `Custody` |
| **Recorder** | The `LOCK` and claimed-journal descriptors; the next slot; the written digests; the poison state | Touches `Custody`, `Control` or native resources. Holds any lock across I/O. |

| Path | Type | Bound | When full |
|---|---|---|---|
| owner → recorder | `Claim`, `Record(intent)`, `Seal` | `record_capacity + 2` | It cannot fill: the core issues at most `record_capacity` records (§2.2) |
| recorder → owner | `Claimed`/`ClaimFailed`, `Durable(ack)`, `Failed(id, class)`, `Sealed`/`SealFailed` | `record_capacity + 2` | It cannot fill for the same reason. The recorder, not the owner, would wait. |
| recorder → status | `Mutex<RecorderStatus>`: claim, durable-through, pending (sequence, since), poisoned (sequence, class, at), sealing | one value | Written before and after each blocking call; held only for a copy |

**Status** is `Control::status()` (`core.rs:522-548`) [S] plus a copy of `RecorderStatus`. No status, cancellation, lease or queue path ever waits on storage. This is the property `h08` proves for the core with a stalled `submit` (§2.8) [S]. §8.4 control NC-S18 proves it for the recorder.

### 7.2 Blocked or uncertain I/O

A `pwrite` or `fdatasync` may block without bound. Timing out a waiting thread **does not cancel** the kernel operation, and a thread in uninterruptible I/O cannot be stopped. While the recorder is blocked:

| Concern | What happens |
|---|---|
| **No new execution** | Admission needs the start record acknowledged; new cases and the terminal commitment need every record acknowledged (§2.3). Unacknowledged records stop all three, with no help from the store. |
| **No lost custody** | The owner thread continues. It is not blocked, because `submit` only enqueues. In-flight operations complete, cleanup attempts run, and owners stay held. `close` is refused (`EvidencePending`). The owner process does not exit. |
| **No claimed persistence** | Pending records stay pending. The status shows "record *n* pending since *t*". No timeout, status read or later event becomes an acknowledgement (§4.4). |
| **Cancellation and lease** | Handled on the control thread through the gate, which the recorder never touches. A cancellation closes admission at once. Its control-fact record waits for the recorder like any other. |
| **Stall policy [D]** | The baseline reports the stall and nothing else. An alternative treats a stall beyond a threshold as `record_failed` (fail-stop). That is safe (it over-approximates if the write later completes) but turns slow storage into a failed run. It must never become an acknowledgement, and it must never release anything. |
| **Destruction while blocked** | The process may stay until the I/O completes or the host reboots. Its locks stay held, so the store stays Busy (§3.7). |

### 7.3 Failure retention

- **Poisoning is permanent.** A poisoned journal stays poisoned for the life of the process. The core's `RecordFailed` failure, its closure refusal and its `EvidenceFailed { record, at }` stay in memory (`core.rs:2914-2916`) [S].
- **Durable records are never altered or removed** by any runtime path (§6.6).
- **Failure detail is not durable** in version 1, beyond what the record kinds carry:
  - the first closure's reason, in `Control{AdmissionClosed}`;
  - case outcomes, recovery attempts and the verdict.

  The in-memory failure log (`core.rs:1105-1137`) dies with the process. This is limit API-1 (§2.11). Nothing in this design claims more.

### 7.4 Proposals outside this mission

- The owner process structure, the transport and status messages that carry `RecorderStatus`, and the unit and service properties belong to the custody owner's own design. The earlier contract's §3.1–§3.11 are not adopted or changed here.
- **No core, model or codec change is needed** (§2.11).
- Anchors A1 to A5, the stall policy, and provisioning automation are Architect decisions (§8.9).
- **The owner's filesystem view.** The owner-process mission must state its unit's filesystem view. A unit that mounts `<STATE_ROOT>` read-only (for example under a strict system protection setting without a write exception), or runs in a mount namespace without it, makes opening refuse (`ST_RDONLY`, or a missing root). That is fail-closed, never a fallback.

---

## 8. Verification plan and implementation envelope (G)

### 8.1 Test architecture

The store has three layers:

1. **Pure functions:** formats, classification, bindings and disposition parsing. They are tested on byte images.
2. **A storage interface** (`StoreIo`) with two implementations:
   - the Linux one, through `libc`;
   - `SimStorage`, compiled for tests only: an in-memory filesystem with explicit **durability state**.
3. **The recorder:** thread wiring, queues and status. It can also be driven synchronously, on the caller's thread, so that crash points are deterministic.

`SimStorage` keeps, per file:
- the **current** bytes, as reads see them;
- the **durable** bytes;
- the ordered **pending** writes.

It also simulates metadata (type, owner uid, mode, link count, inode, filesystem magic), so root ownership and every refusal can be tested without privilege.

| Fault | Mechanism |
|---|---|
| Injected error | Before operation *k*: return EIO, ENOSPC, EDQUOT, EINTR or a short count |
| Lying sync | Return 0 and leave pending writes volatile. Used only in tests that document which invariant (I3) cannot hold under a lying device. Never counted as coverage. |
| F1 crash | `durable := current` |
| F2 crash | `durable` kept, pending dropped |
| F2 crash with tear | The last pending write applied up to a chosen byte |

### 8.2 Deterministic storage-fault and crash-point matrix

Every scenario uses the **real core** (`support/custody`) with the existing in-process stand-in style (`core-tests:80-326`) [S] and the store over `SimStorage`. Each scenario first records its sequence of storage operations. Then, for each operation index *k* and each crash mode, it reruns to *k*, crashes, runs the startup scan and checks the invariants (§8.3).

Crash modes: F1, F2, and F2 with a tear at bytes 1, 8, 64 and 127 of the slot.

Scenarios:

| # | Scenario |
|---|---|
| X1 | Clean pass: one case, one created and confirmed process owner, finish, terminal record, close, seal |
| X2 | Failed cleanup, then recovery, resolved by an explicit attempt; final verdict Failed |
| X3 | Unknown outcome at case end (the run stops with the operation open) |
| X4 | Authority lost (`Ended` without facts) |
| X5 | A late incident after finalization, recovered |
| X6 | Cancellation before admission (`NotAdmitted` settlement) |
| X7 | `record_failed` injected at each record of X1, as an error in A4 or A5, or a recheck failure in A6 |
| X8 | A sink-side duplicate submission after a contained sink panic |
| X9 | Prior incidents with valid, missing, misbound, uid-owned and stale dispositions; the claim header's applied list; the applied-history rule on the following startup |
| X10 | Pool exhaustion; an abandoned claim; a claim gap; duplicate generations; a foreign root |
| X11 | A stalled `fdatasync` (threaded wiring; channel-ordered, no sleeps): status, cancellation and lease stay responsive, no acknowledgement occurs, and the owner keeps its owners |

### 8.3 Invariants checked at every crash point

| Id | Invariant |
|---|---|
| I1 | **Write-ahead.** Every action for which the core issued an `OpTicket` before the crash has an `ActionStarted` in the post-crash durable image (F1, F2, F2 with tear). |
| I2 | **No false resolution.** If the core held any owner, had an open or unknown operation, a lost action or an unsettled late incident at the crash point, that generation is not Resolved after the crash. |
| I3 | **Acknowledged implies durable.** Every record the core had acknowledged lies within the durable valid prefix (F2, honest sync). |
| I4 | **No acknowledgement after an error.** After an injected error at record *n*: the core's acknowledged count stays below *n*; the snapshot shows `EvidenceFailed` for *n*; `close` is refused; no later `Durable` event is sent. |
| I5 | **Deterministic.** Classifying the same image twice gives identical classes and bindings. A read-only scan interrupted by F1 changes nothing. |
| I6 | **Binding sensitivity.** Changing any single sampled byte of a non-resolved journal changes its binding, and the old disposition stops validating. |
| I7 | **Seal soundness.** A Resolved (closed) classification implies that the core returned `Closed` with every record acknowledged. |
| I8 | **Bounds.** No read beyond the geometry; memory within §3.8. |
| I9 | **Conformance.** Every journal produced by every existing deterministic core run classifies as non-Malformed. The runs include `Run::every_kind` (`codec-tests`) and the h18 adversarial sequences (`core-tests:5237`), recorded through a store-backed sink. |
| I10 | **Outstanding superset.** Under F1 and F2 with honest storage, the reported outstanding set of the crashed generation contains every action and incident the core still held, had open, had lost or had unsettled at the crash point. Equality does not hold, because over-approximation is allowed (§5.4 A2). |

### 8.4 Negative controls

Each control mutates the future implementation at one point. It must compile, fail exactly its intended test with the marker inside the failing assertion, and be restored, with SHA-256 evidence of restoration. A failure caught only by a source guard does not count.

| Control | Mutation | Intended failing assertion |
|---|---|---|
| NC-S1 | Send `Durable` before `fdatasync` returns | I3 `[ack-durable]` (X1, F2 at the sync) |
| NC-S2 | After an `fdatasync` error, retry it and acknowledge on success | I4 `[no-ack-after-error]` (X7) |
| NC-S3 | Count only failed actions as outstanding | I10 `[outstanding-superset]` (X1, crash while the created owner is held). The refusal rule alone would still block there, so I2 cannot catch this mutation. |
| NC-S4 | Outstanding-only rule (unsealed with zero outstanding is Resolved) | `[unsealed-refusal]` (X5 crash before `IncidentOpened`) |
| NC-S5 | Skip the frame-generation check | `[foreign-generation]` (X10) |
| NC-S6 | Skip the sequence-equals-slot check | `[slot-order]` |
| NC-S7 | Accept non-zero slot padding | `[slot-padding]` |
| NC-S8 | Accept a valid slot after an empty one | `[log-hole]` |
| NC-S9 | Disposition reader accepts a file not owned by root | `[root-disposition]` (X9) |
| NC-S10 | Disposition reader trusts the file name and not the recomputed binding | `[disposition-binding]` (X9 stale) |
| NC-S11 | Opening follows a symbolic link in a path component | `[no-symlink]` |
| NC-S12 | Opening accepts `st_nlink > 1` | `[no-hardlink]` |
| NC-S13 | Opening creates a missing `journals/` | `[no-create]` |
| NC-S14 | Opening accepts tmpfs | `[fs-type]` |
| NC-S15 | Startup takes the store lock without `LOCK_NB`, or not at all | `[single-writer]` (two owners on one store) |
| NC-S16 | Recorder writes a submission out of order | `[submission-order]` |
| NC-S17 | Scan ignores claim gaps | `[claim-gap]` |
| NC-S18 | Status reads `RecorderStatus` under the recorder's I/O lock | `[status-responsive]` (X11) |
| NC-S19 | A stall past a deadline becomes `Durable` | `[no-timeout-ack]` (X11) |
| NC-S20 | Claim header omits the applied dispositions | `[dispositions-recorded]` (grammar cross-check) |
| NC-S21 | Binding omits the file digest | I6 `[binding-exact]` |
| NC-S22 | Seal accepted without matching the prefix | `[seal-tail]` |
| NC-S23 | More than `incident_limit` current incidents truncated to the limit | `[no-truncated-priors]` |
| NC-S24 | Scan reads before `fsync` (F1 then F2 sequence) | `[durable-read]` |

### 8.5 Filesystem fixtures and supported profile

- **Ordinary tests** (`cargo test --locked -p nexus-verifier-sandbox --test phase2_custody_store`) use `SimStorage` and byte images only. They need no real runner, user bus, provisioned state root, live sandbox, root privilege or network.
- **Unprivileged real-filesystem tests**, in the same target, use a test-owned directory under `CARGO_TARGET_TMPDIR`. They check only what an unprivileged process can establish:
  - `RESOLVE_NO_SYMLINKS` and `O_NOFOLLOW` refusals;
  - hard-link detection;
  - `flock` exclusivity between two descriptions;
  - `pwrite` and `fdatasync` wiring on a preallocated file;
  - filesystem-type classification of the fixture's own filesystem, through the same check function the opening uses (the full opening fails earlier, on the root-owned `PROVISION`). On ext4 or XFS the check accepts; on tmpfs or any other type it refuses. Both branches assert.

  Root-owned components cannot be created without privilege, so those paths are tested through `SimStorage` metadata.
- **Privileged tests** need root-owned fixtures and provisioning verification on a host. They are a separate, later, explicitly authorized step [D]. **No experiment on the runner, the user bus or any real state root is authorized now.**

### 8.6 Simulated crash versus power-loss proof

**What the simulated tests prove:** the protocol's properties (I1 to I9) under the durability model of §8.1, at every enumerated point, deterministically.

**What they do not prove:**
- that a real device honours flushes;
- that ext4 or XFS behave as documented on a given host;
- that a real power cut leaves only states the model allows.

**A power-loss proof** would need a power-cut rig on the target hardware, repeated many times, with independent verification. It is not proposed for authorization here [D]. Device honesty stays an assumption [A], and every report must keep the two apart.

### 8.7 Implementation files and dependencies

The proposal is for a later implementation mission; nothing is created now.

| Path (under `crates/nexus-verifier-sandbox/tests/`) | Content |
|---|---|
| `support/custody/store/mod.rs` | Module documentation (claims and non-claims) and the public API: `StoreRoot`, `StartupReport`, `Recorder`, `RecorderSink`, `RecorderStatus` |
| `support/custody/store/format.rs` | Journal header, seal and slot geometry (binary); `PROVISION` and disposition text grammars; canonical binding bytes |
| `support/custody/store/classify.rs` | The pure classifier: structure, grammar, outstanding set, seal, classes, pool-level checks, applied-history rule |
| `support/custody/store/open.rs` | Opening and validation (§3.4) over `StoreIo` |
| `support/custody/store/io.rs` | The `StoreIo` trait and its Linux implementation (`openat2` through `SYS_openat2` and `open_how`, `fstatat`, `fstatfs`, `flock`, `pwrite`, `fdatasync`, `getrandom`) |
| `support/custody/store/recorder.rs` | The recorder: claim, append, poison, seal, status; threaded and synchronous drivers |
| `support/custody/store/disposition.rs` | The `DispositionValidator` implementation |
| `support/custody/store/sim.rs` | `SimStorage`, `#[cfg(test)]`: the durability model, fault and crash injection, simulated metadata |
| `support/custody/mod.rs` | `pub mod store;` and updated integration obligations (documentation) |
| `phase2_custody_store.rs` | The new test target: X1–X11, I1–I9, format and opening refusals, unprivileged real-filesystem checks |

**Unchanged:**
- `core.rs`, `model.rs` and `codec.rs` (no API change, §2.11);
- the version-1 wire format;
- production `src/`;
- the live harness;
- workflows, `Cargo.toml` and `Cargo.lock`.

The **read-only verifier** for provisioning (§3.2 step 9) and the store status view belong to the later owner-process mission.

**Dependencies: none new.** The crate already depends on:
- `sha2 = "0.10"` and `hex = "0.4"`;
- `libc = "0.2"` on Linux, locked at 0.2.183. That version provides `syscall` with `SYS_openat2` and `open_how`, `RESOLVE_*`, `fstatat`, `fstatfs`, `EXT4_SUPER_MAGIC`, `XFS_SUPER_MAGIC`, `TMPFS_MAGIC`, `flock` with `LOCK_*`, `pwrite`, `fdatasync` and `getrandom` [S]. No runtime path calls `fallocate`; only provisioning allocates.

Integration tests may use the package's normal dependencies, as the custody codec already does with `sha2`.

**Unsafe surface.** The standard library covers `pwrite` (`FileExt::write_at`), `fdatasync` (`File::sync_data`), `fstat` and advisory locking. It does not cover `openat2`, `fstatat` relative to a kept directory descriptor, or `fstatfs`. Those need a small `unsafe` FFI surface in `io.rs`: each call is wrapped once, with its preconditions documented, and gets a dedicated review. The custody core itself contains no `unsafe`.

**Validation for that mission:**
- `cargo test --locked -p nexus-verifier-sandbox` with `--test phase2_custody_store`, `--test phase2_custody_core` and `--test phase2_custody_codec`;
- `cargo clippy --locked -p nexus-verifier-sandbox` on those targets;
- `cargo fmt --all -- --check`;
- the live harness built only;
- the negative controls of §8.4.

### 8.8 Non-goals

This design does not do or include any of the following:
- initialize, select or provision any state root, `PROVISION`, pool or disposition;
- touch the runner, the user bus, systemd or the host;
- run any live case;
- design the custody owner process, the service unit, the transport or the client;
- provide tamper resistance against processes of the store uid (§5.6);
- implement any anchor;
- prove power-loss behaviour;
- make failure detail durable beyond version 1;
- support Windows or macOS (the Phase Two Linux support profile, AGENTS §24);
- support filesystems other than ext4 and XFS;
- delete, compact or prune automatically;
- offer an emergency force-close.

### 8.9 Architect decisions and future authorizations

| Id | Decision | Recommendation |
|---|---|---|
| D-1 | The pool-based, root-provisioned layout (§3.3), which depends on the earlier contract's item D-2 (root-initialized state root, root-owned dispositions) | Approve |
| D-2 | `<PROVISION_PATH>` form, `<STATE_ROOT>` constraints, `N` = 256, `C` = 512, archive scan bound | Approve the forms; the Owner chooses the path at provisioning |
| D-3 | Refusal rule: unsealed with native work blocks (§5.3), versus outstanding-only | Unsealed with native work blocks |
| D-4 | Trusted anchor: A0 with its non-claim, or A1 to A5 (§5.6) | A0 now if the store is accepted as crash and accident evidence; otherwise A5 or A2 first |
| D-5 | Stall policy: report only, or fail-stop after a threshold (§7.2) | Report only |
| D-6 | Refuse when `protected_hardlinks` or `protected_symlinks` is not 1, or the mount has `nobarrier` (§3.4) | Refuse |
| D-7 | Owner archival, recycling and `retired-through` procedures (§6.4) | Approve as Owner-only procedures; specify exactly in the implementation mission |
| D-8 | A provisioning tool (privileged code), or a documented manual procedure plus a read-only verifier | Manual procedure plus a read-only verifier |
| D-9 | Whether a later record version should carry closure and failure detail (API-1) | Defer |
| D-10 | Whether a power-loss rig is ever required | Defer; keep the non-claim |

**Future authorizations required:**
1. **Implementation.** An implementation mission for §8.7.
2. **Provisioning.** An Owner provisioning act on a named host, which this mission does not authorize.
3. **Integration.** The owner-process and service integration mission.
4. **Live validation.** Any live validation.
5. **Anchors.** Any privileged or external anchor (D-4).

---

## 9. Review record

These challenges were raised against the draft and resolved within scope. Each is stated with its resolution.

| # | Challenge | Resolution |
|---|---|---|
| 1 | A registry of generations (uid-owned) was drafted to detect deleted journals. A same-uid deletion of a journal together with its registry entry stays undetected, and creating names at runtime brings directory-sync crash points. | **Second and final attempt:** a pool of root-provisioned files in root-owned `journals/`. Deletion becomes impossible for the uid instead of merely detectable. There are no runtime names, and physical reservation happens at provisioning. Two attempts; no third revision was needed. |
| 2 | Physical reservation equated with logical capacity | Replaced by the explicit mechanism of §4.6 (allocation, zero-fill, capacity at most `C`, one slot per sequence, slot of at least 123 bytes), with CoW filesystems excluded. |
| 3 | Retrying `fdatasync` after EIO | Prohibited, on kernel errseq and vfs semantics (§4.5). |
| 4 | F1 followed by F2: the startup scan reads page-cache state, then power is lost | Sync before reading (§5.2, NC-S24). |
| 5 | Outstanding-only refusal misses a late owner recorded after the fact (`core.rs:1924-1930`) and the incomplete prefix after a recording failure | The unsealed-with-native-work rule (§5.3, D-3). |
| 6 | `incident_limit` deadlock: dispositioned incidents stay `Unresolved` forever, and more than 16 make `Custody::new` refuse before any disposition can apply | The applied-history rule (§6.4) plus Owner archival. API-2 remains only an option. |
| 7 | "Nothing a uid process writes lifts a refusal" overstated | Restated precisely (§5.6): true for dispositions, false for the evidence the classification reads, without an anchor. |
| 8 | Device numbers in `PROVISION` change across boots | Inode numbers only, with `st_dev` compared at runtime (§3.6). |
| 9 | `flock` inherited by helper children | `O_CLOEXEC` on every store descriptor (§3.7). |
| 10 | `RecordSink::submit` cannot fail | Non-blocking enqueue; failure through `record_failed` (§4.5). No core change. |
| 11 | Clean closure is not in version 1 | The closure seal outside the frames (§4.8), with its crash window (S1, S2) documented. |
| 12 | ACLs bypassing mode checks | The `acl(5)` mask rule bounds them (§3.6). |

---

## 10. Non-claims

- **Not implemented.** Nothing in this document is implemented. No file, directory, pool, `PROVISION`, lock or disposition has been created anywhere.
- **Not executed.** No Cargo build, test binary, live observer, systemd or bus operation, host provisioning or workflow ran for this design.
- **Not tamper-proof.** The store is not tamper evidence against processes of its own uid unless an anchor is chosen (§5.6).
- **Not power-loss proven.** No power-loss behaviour is proven (§8.6).
- **No proof of cleanup.** Nothing here confirms native cleanup, authenticates a peer, or establishes journal completeness beyond the stated detection.
- **No live acceptance.** Nothing here establishes live acceptance, integration or Phase Two completion.
- **Not approved.** Publication of this document is not approval.
