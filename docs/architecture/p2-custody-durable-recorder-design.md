# P2 custody: durable recorder and refusal store — design (revision R4)

| | |
|---|---|
| Missions | P2-V1-R3B-I3-P (first candidate, `b23ae1ac`), revised by P2-V1-R3B-I3-P-R1 (`34cb35d8`), P2-V1-R3B-I3-P-R2 (`8f888252`), P2-V1-R3B-I3-P-R3 (`b0f84376`) and P2-V1-R3B-I3-P-R4. A module of Phase Two, not Phase Three. |
| Status | **Design candidate for independent Architect review.** Not implemented, not provisioned, not qualified on any host, not live-validated, and not approved by being published. |
| Design base | `b0f8437636d0bc7d38818e75df71d1bf516cff8e` (tree `08a0e04f…`, parent `8f888252…`), the R3 candidate. It is not accepted for implementation. |
| Source baseline | `45898e05178a56efaadeb1f8f9ee7a0a521c72c9`. Every `file:line` reference to a Rust file below is to this commit; every reference to a Linux file is to the v6.17 tag (§1.4). |
| Validation | `docs/evidence/p2-v1-r3b-i3-p-r4/` holds the R4 standard-library Python **design model**, its checks, its model-level negative controls, re-runs of the Architect's counterexamples on the unchanged R1 and R2 models and of the R4 findings on the unchanged R3 model, and a reference check. The R1, R2 and R3 evidence in `docs/evidence/p2-v1-r3b-i3-p-r1/`, `-r2/` and `-r3/` is kept unchanged. The model validates this specification's protocols against the model's own semantics. It is not the store, an owner service, a kernel, or a substitute for the later Rust tests (§16). |
| Authorizes | Nothing beyond this document. Implementation, provisioning, qualification, service integration and live validation each need a separate Architect authorization (§18). |

Source labels:

| Label | Meaning |
|---|---|
| **[S]** | A fact of the source baseline, with a `file:line` reference |
| **[U]** | Primary documentation, listed in §1.4 |
| **[I]** | A read-only observation of the development host on 2026-10-01. It is not a property of any future runtime host. |
| **[P]** | A proposal of this design |
| **[D]** | An Architect disposition (§2.1) |
| **[A]** | An assumption of the supported profile. Software cannot prove it. |
| **[M]** | Exercised by the design model (§16.1), against the model's semantics, not against the Rust code, the kernel or a device |

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

## 0. Summary

### 0.1 What the module is

The module meets two of the core's unmet integration obligations: durability, and the journal-and-recovery obligation (`mod.rs:144-151`) [S]. It also provides the refusal those obligations leave to the integration (`mod.rs:168-171`) [S]. It has three parts:

- **Recorder.** It makes each record the core issues durable before that record is acknowledged.
- **Refusal store.** It turns every earlier generation's evidence into the prior incidents that block the next run (`core.rs:1289-1297`, `core.rs:1463-1465`) [S].
- **Disposition reader.** It accepts only root-owned dispositions that bind exactly to one incident.

### 0.2 Principal decisions

| # | Decision | Section |
|---|---|---|
| 1 | One journal per custody generation. Each journal is a root-provisioned, preallocated, zero-filled **pool file** of 4096-byte blocks: block 0 is the header, block 1 the closure seal, and block 1+*n* holds record *n*'s unchanged version-1 frame. Each block is written at most once per generation. | §6.3, §7 |
| 2 | An acknowledgement exists only as the recorder's published *durable-through* count. The count advances only after the record's block is fully written, synced on the writer's own long-open descriptor, and the journal's identity rechecked. No message, timeout or later event creates one. | §9 |
| 3 | The crash model keeps separate: process-local state, kernel-visible bytes, durable bytes, pending writeback, writeback-error observation, and directory entries. Process death makes nothing durable. A sync on a newly opened descriptor certifies nothing about earlier writebacks. | §4 |
| 4 | Physical containment is an explicit assumption [A]: a failed write of one aligned 4096-byte block alters no other block. The Owner attests it and it is qualified empirically later. The store refuses where the verifiable prerequisites do not hold. | §5 |
| 5 | Lock-bearing descriptors (the store lock and the claimed journal's lock) belong to the owner process's `StoreGuard`, beside `Custody`. The recorder thread has its own lock-free I/O descriptor. A recorder panic, disconnect or worker-side unlock cannot release exclusion. | §8 |
| 6 | Submission and delivery use a bounded, idempotent, state-based **exchange**, not message queues. Every fatal condition (I/O failure, recorder loss, an invalid or conflicting submission, an unexpected acknowledgement outcome, a poisoned mutex) is one latch that records its first cause and the durable position P at that instant. After it nothing is published, admitted or acknowledged beyond P. The core is told only at a record it really issued. | §9 |
| 6a | The integration calls `Custody::admit` only through the **store admission gate**: one critical section of the exchange mutex covers the health check and the call. A fatal latch therefore stops new admission at once, without waiting for a record. | §9.8 |
| 7 | Startup takes every lock non-blocking **before** syncing or reading any journal. It type-checks entries before opening them, opens with non-blocking, no-follow flags, and changes no application state. Its persistence effects are disclosed: the evidence-preservation sync, and (R3) the activation's directory syncs and the timestamp probe on `LOCK`. | §10 |
| 8 | Refusal stays conservative [D-3]. An unsealed generation that recorded any action start is refused, whatever its recorded-unsettled count. The restart report keeps separate the recorded unsettled entries, evidence completeness, whether native work was possible, and the refusal. It never invents an incident identity. | §11 |
| 9 | Every container and text byte is assigned and validated, with full consumption and checked arithmetic. The NXCD version-1 record frame stays byte-identical [D-9]. | §7 |
| 10 | Only ext4 is supported initially [D-6]. It is identified from the retained descriptor's mount ID in `mountinfo`, never from the shared `0xef53` magic alone. XFS is deferred. **R4 narrows the profile**: an internal journal, `data=ordered`, barriers, no fast or asynchronous commits, normal journal loading, and the kernel the Owner qualified. Each is read where the kernel shows its effective state, never inferred from a string absent from `mountinfo`; what the store uid cannot read is a qualification fact the Owner keeps. | §5.3, §6.4, §13.2 |
| 11 | Maintenance is offline, root-performed, and either fully specified or unsupported [D-7]. Every procedure runs inside one **maintenance session** that holds the store lock exclusively for its whole lifetime and verifies through that retained authority, never through a new lock request. Journal bindings are content-addressed, so archival keeps them. Any byte change creates a new binding, explicitly. | §12.3, §13 |
| 11a | A `PROVISION` read is only a candidate until the reader holds the store lock and has **revalidated** it by fresh lookups: the same `PROVISION` inode and bytes, the same state root, and the locked `LOCK` inode. A failed revalidation re-selects, at most three times. A successor is published only while its maintenance session holds both stores' locks. | §10.5, §13.6 |
| 11b | Crash outcomes of administrative procedures are computed under explicit metadata-persistence schedules: entries may become durable before any directory sync. Every model operation is a written protocol step, and the crash matrix is recomputed. | §4.8, §15 |
| 11c | **Durable activation (R3; its proof restated in R4).** No owner claims a journal, and no maintenance session decides, on a selection that is merely visible. Under the store lock, each first syncs every directory its selection and decision depend on, then probes its own `LOCK`, and revalidates. Recovery never provisions a fresh root over a root that may hold history. R4 states exactly what this establishes: every transaction holding a dependency has committed, shown by the syncs' own abort tests and by the probe's handle start. It does not show that every earlier filesystem transaction succeeded (§10.7). | §10.6, §10.7, §13.2, §13.12 |
| 12 | Store A0 is limited crash evidence, **not** tamper-resistant admission authority [D-4]. Production or runner use stays behind the unresolved gate **G-AUTH**. | §3.3, §18 |
| 13 | No change to `core.rs`, `model.rs` or `codec.rs` is required. One narrow core API (API-5) is proposed for a later mission; the design does not depend on it. | §2.3 |

### 0.3 Changes from the first candidate

| Finding | Was | Now | Section |
|---|---|---|---|
| R1, crash model | Process death modelled as `durable := current`; the startup sync treated as establishing durability | Separate state components. Process death leaves durability unchanged. Writeback-error (errseq) semantics. Reopen-after-error is explicit. | §4 |
| R2, geometry | 128-byte slots sharing pages; header and seal in one block | One record per block, the seal in its own block, write-once blocks. Containment is an assumption with an attestation and later qualification. | §5, §6.3 |
| R3, lock lifetime | The recorder thread owned the lock descriptors | `StoreGuard` owns them; separate open file descriptions | §8 |
| R4, startup order | Sync before lock; contradictory descriptor lifetimes | Lock before sync or read; descriptor classes; safe open; claim handoff | §10 |
| R5, queues | Bounded queues sized from unique records; failure sent as a message | Idempotent exchange plus latches; re-acknowledgement removed | §9 |
| R6, restart knowledge | An impossible "outstanding superset" claim | A four-part honest report; invariants with domains; the grammar re-audited against the core | §11, §12 |
| R7, grammars | Unassigned bytes; lenient abandoned-claim rule | Every byte assigned; a checksum-valid but invalid header is Malformed | §7 |
| R8, filesystem | Magic `0xef53` taken as ext4; XFS included | Mount-ID identification; ext4 only; allocation facts kept distinct | §6.4, §6.5 |
| R9, administration | A binding that included the pool location; maintenance underspecified | Content-addressed binding; every retained procedure specified with its crash points | §12.3, §13 |
| R10, reconciliation | — | The whole document reconciled | all |

### 0.4 Changes in R2

Each finding was first reproduced on the unchanged R1 model (§19.3).

| Finding | R1 | R2 | Section |
|---|---|---|---|
| F1, fatal failure and admission | Failure, loss and conflict were three latches. They reached the core only through `record_failed` at one exact position, so a loss with no record pending, a conflict on an acknowledged record and an invalid submission were never delivered, and admission continued meanwhile. | One fatal latch with its first cause and P. Publication stops at the latch. The store admission gate refuses admission at once. The failure is delivered at the core's next unacknowledged record once it exists. Eight concerns are kept apart, and INV-4 is restated. API-5 is proposed, not required. | §9, §12.1, §2.3 |
| F2, maintenance verification | Procedures ran under `flock -n LOCK` and called the standalone verifier, which takes a shared lock and so reports Busy inside every procedure | A maintenance session holds the exclusive lock for its lifetime and verifies through it. The standalone verifier stays a separate, outside-maintenance contract. | §13.1, §13.11 |
| F3, `PROVISION` selection | A startup could read `PROVISION` A, pause, and lock A's store after a successor B was published, while another startup locked B | Post-lock revalidation by fresh lookups, a `revision` field, bounded re-selection, and successor publication while the session holds both locks | §10.5, §13.6 |
| F4, administrative persistence | Entries became durable only through a directory sync; the model synced the state root at `LOCK` creation without the text saying so; clean pages could not be reclaimed while a descriptor was open | Ordered and per-directory metadata schedules; the hidden step written down and every step checked for parity; page reclaim separate from inode eviction; the crash matrix recomputed | §4, §4.8, §13, §15 |
| Section 8, whole module | Every disposition and revoked entry was read with no aggregate bound; reports were unbounded | Entry counts checked before any entry is examined; a bounded report that says when it is partial; authorization only from the complete set | §6.6, §7.7, §11.2 |

### 0.5 Changes in R3

The Architect's composed counterexample was first reproduced on the unchanged R2 model (§19.4).

| Finding | R2 | R3 | Section |
|---|---|---|---|
| Durable activation | A successor's `PROVISION` could be visible but not durable when its session died. A new owner then selected it, claimed and recorded acknowledged work. A power loss could restore the predecessor, whose history omits that work, and a fresh owner then ran. | An owner and a session activate before they decide: one filesystem verified, every dependency directory synced, a certification probe, and revalidation. Nothing is claimed before that. | §10.6 |
| A sync through a new descriptor | — | At Linux v6.17 a directory fsync returns 0, having committed nothing, when an earlier commit already failed silently. The probe exposes such a failure before the activation is trusted. | §5.1 (A-M2), §10.6 |
| Initial provisioning | R2's recovery after Unprovisioned removed "the incomplete root, which holds no history" and provisioned again. In the composed case the root held an acknowledged, unsealed generation. | Unprovisioned is never permission for a fresh root: a root with any non-zero pool byte is re-published. Activation makes such a root impossible in domain H. | §13.2 |
| Dependency audit | Each procedure's crash points were checked in isolation | Every procedure is audited and composed with dependent work and a second crash | §13.12, §15.4 |
| Profile | `<PROVISION_PATH>` and `<STATE_ROOT>` could lie on different filesystems | One filesystem, verified at every opening, so that the qualified profile and the probe cover the `PROVISION` directory | §6.2, §6.4 |

### 0.6 Changes in R4

The Architect's findings F1–F4 were traced through the complete Linux v6.17 paths (§1.4). The R4 findings were first re-run on the unchanged R3 model (§19.5).

| Finding | R3 | R4 | Section |
|---|---|---|---|
| F1, two discarded flushes in `commit.c` | R3 cited the commit path's I/O errors as aborting the journal | Confirmed: the external journal's filesystem-device flush and the asynchronous commit's final flush discard their status. Neither is reachable on the narrowed profile: the journal is internal, and v6.17 refuses to mount `journal_async_commit` with `data=ordered`. | §5.1, §6.4 |
| F2, the checkpoint's discarded flush | Not considered | Confirmed and reachable: before the log tail moves, the flush status is discarded. If the device fails that flush, committed metadata, an activated selection's included, can be lost at a later power loss, and no process sees an error. No layer reliably catches it. Failed home writes are caught. A persistent failure fails the superblock's write where FUA is emulated by a flush. A transient failure, or native FUA, passes. **A-S5 (new)**: the device does not fail cache flushes. | §5.1, §10.7, §12.2 |
| F3, the completed-transaction return | "An `fsync` of that file then waits for that handle's transaction" | Confirmed. A regular file's `fsync` waits only for a running or committing transaction; for a completed one it returns 0 without testing the abort flag. The narrower conclusion is that the activation certificate rests on two things: each sync's own abort test, and the probe's handle-start test, which the probe's `fsync` reports as the emergency state. An abort the completed branch can hide comes after that test and cannot uncommit a dependency. | §5.1, §10.6, §10.7 |
| F4, R3's model | `fsync_file` returned EIO for every aborted, unnoticed journal | Corrected. No R3 check took that branch, so no R3 result changes. | §16.1 |
| Profile establishment | Pinned `mountinfo` strings; excluded options recognised only by their presence | The effective option listing, the journal's jbd2 name and the kernel identity are read at every opening and again after activation's syncs. Unknown, contradictory or unqualified information refuses. `PROVISION` gains the `kernel` field. | §5.3, §6.4, §7.5, §13.2 |

---

## 1. Scope, base and sources

### 1.1 In scope

The module is the recorder, refusal store and disposition reader for the live harness's custody owner. The custody core is test infrastructure in `tests/support/custody/` (`mod.rs:1-21`) [S]. This design covers:

- the storage contract and its formats;
- locks and descriptor lifetimes;
- the recorder and startup protocols;
- restart semantics;
- administrative procedures;
- the verification plan and the implementation envelope.

### 1.2 Out of scope

- the custody owner process or service, its control transport and the client;
- the live cases;
- runner and host changes;
- any privileged helper, account, capability, TPM or cloud component [D-4];
- any Rust implementation.

### 1.3 Governance and base

- **Governance read.** `AGENTS.md` and `CLAUDE.md` were read at `45898e05`. They are byte-identical at `b23ae1ac`, `34cb35d8`, `8f888252` and the R4 design base `b0f84376`, verified by blob id. No descendant `AGENTS.md` or `CLAUDE.md` exists.
- **Rules applied:**
  - fail closed, with no `$HOME`, cwd or temporary fallback;
  - explicit authority, and no recreation of trusted roots;
  - bounded cleanup, and no PID authority;
  - observable security errors;
  - no dependency churn and no secrets;
  - the two-attempt rule;
  - no lock held across filesystem mutation or waiting;
  - paths, `HOME`, UUIDs and PIDs are not authority.
- **Earlier session designs.** The custody plan (R3B-P1) and contract (R3B-P2) were session documents, not repository files. Nothing here relies on them as evidence. Where this design departs from them, it says so.

### 1.4 Primary documentation

**Linux man-pages** 6.7 (Ubuntu package `manpages` 6.7-2), read locally [U]:

| Page | Used for |
|---|---|
| `fsync(2)` | `fsync` flushes the device cache. A new directory entry needs an `fsync` of the directory. `fdatasync` skips metadata not needed to retrieve data. EIO reporting. |
| `write(2)` | Partial writes and EINTR. A successful write is not a durability guarantee; errors can be reported later. |
| `open(2)` | `O_NOFOLLOW` covers the final component only. `O_NONBLOCK` has no effect for regular files and block devices. `O_PATH`, `O_DIRECTORY`, `O_CLOEXEC`, `O_NOCTTY`. |
| `fifo(7)` | Opening a FIFO normally blocks until the other end opens. In non-blocking mode a read-only open succeeds. |
| `openat2(2)` | `RESOLVE_BENEATH`, `RESOLVE_NO_SYMLINKS`, `RESOLVE_NO_MAGICLINKS`, `RESOLVE_NO_XDEV`; since Linux 5.6 |
| `stat(2)` | `fstat`; `fstatat` with `AT_SYMLINK_NOFOLLOW` |
| `statx(2)` | `stx_mnt_id` corresponds to field 1 of a `/proc/<pid>/mountinfo` record |
| `proc_pid_mountinfo(5)` | Field (1) mount ID; (3) major:minor, the value of `st_dev`; (6) per-mount options; (9) filesystem type; (11) per-superblock options. Parsers ignore unrecognized optional fields. |
| `statfs(2)` | `EXT2_SUPER_MAGIC`, `EXT3_SUPER_MAGIC` and `EXT4_SUPER_MAGIC` are all `0xef53` |
| `statvfs(3)` | `f_bsize`, `f_frsize` |
| `sysconf(3)` | `_SC_PAGESIZE` |
| `fallocate(2)` | Mode 0 guarantees that later writes into the range do not fail for lack of space; EOPNOTSUPP; shared extents |
| `sync_file_range(2)` | Copy-on-write filesystems cannot overwrite in place. Writes into preallocated space may need the allocator. It is not a durability call. |
| `flock(2)` | Locks belong to the open file description. Duplicates share a lock, and `LOCK_UN` on any duplicate releases it. Released when all duplicates close. Kept across `execve`. Advisory. |
| `flock(1)` | `-n` fails rather than waits; `-x` is the default; `-o` closes the lock descriptor before the command and must not be used |
| `link(2)` | EEXIST when the new path exists (no-replace publication). EPERM under `protected_hardlinks`. |
| `rename(2)` | Atomic replacement of the new path |
| `unlink(2)` | Sticky directories; EPERM cases |
| `inode(7)` | Inode numbers are unique only within one filesystem |
| `proc_sys_fs(5)` | `protected_hardlinks` and `protected_symlinks` (kernel default 0) |
| `random(4)` | `boot_id` is generated once per boot |
| `fsync(2)`, again (R3) | Linux needs no write access: a read-only descriptor can be synced. EINTR is a documented error. |
| `open(2)` `O_PATH` (R3) | Operations other than those listed fail with EBADF, and `fsync` is not listed. `O_RDONLY` needs read permission on the object. |
| `utimensat(2)` (R3) | Setting both timestamps to the current time (`times` NULL) is permitted to the file's owner |
| `uname(2)` (R4) | `struct utsname` fields `release` and `version`: the running kernel's identity, compared with the qualified one (§6.4 step 11) |
| `readlink(2)` (R4) | Reads a symbolic link's target; used for `/sys/dev/block/<major>:<minor>` (§6.4 step 9) |

**Linux kernel documentation and source** [U]:
- **errseq** (`https://docs.kernel.org/core-api/errseq.html`), on `errseq_sample`: "If the error has been "seen", new callers will not see an old error. If there is an unseen error in eseq, the caller of this function will see it the next time it checks for an error."
- **vfs**, "Handling errors during writeback" (`https://docs.kernel.org/filesystems/vfs.html`): errors are reported "to fsync on all file descriptions that were open at the time that the error occurred."
- **Linux v6.17 `fs/open.c`**, `do_dentry_open`: `f->f_wb_err = filemap_sample_wb_err(f->f_mapping);`. A new open file description samples the error sequence when it is opened.
- **FIEMAP** (`https://docs.kernel.org/filesystems/fiemap.html`): `FIEMAP_EXTENT_UNWRITTEN` (allocated but not initialized), `_SHARED`, `_DELALLOC`, `_UNKNOWN`, `_ENCODED`, `_DATA_INLINE`, `_NOT_ALIGNED`; `FIEMAP_FLAG_SYNC`.
- **sysfs-block ABI** (Linux v6.17 `Documentation/ABI/stable/sysfs-block`):
  - `logical_block_size` is "the smallest unit the storage device can address";
  - `physical_block_size` is "the smallest unit a physical storage device can write atomically";
  - `write_cache` reports the cache mode.

- **Linux v6.17 sources** (`raw.githubusercontent.com/torvalds/linux/v6.17/…`; `checkpoint.c`, `commit.c` and `jbd2.h` were also compared byte for byte with `git.kernel.org`'s v6.17 tree; every file's SHA-256 is in the R4 evidence). Read in full along the paths below for R4; R3 read the first twelve files for single branches. Citations use short names: `journal.c`, `commit.c`, `checkpoint.c`, `recovery.c` and `transaction.c` are in `fs/jbd2/`; `fsync.c`, `super.c`, `inode.c`, `ext4_jbd2.c`, `ext4_jbd2.h`, `ext4.h`, `fast_commit.c` and `sysfs.c` in `fs/ext4/`; `fs-writeback.c`, `utimes.c`, `sync.c` and `buffer.c` in `fs/`; `blk-flush.c` and `blk-core.c` in `block/`; `jbd2.h` is `include/linux/jbd2.h` and `journal.rst` is `Documentation/filesystems/ext4/journal.rst`.
  - **Syncs.** `ext4_sync_file` (`fsync.c:129-177`):
    - returns an emergency state first (`fsync.c:135-137`);
    - returns having committed nothing when the superblock is read-only (`fsync.c:143-144`);
    - otherwise writes and waits for the file's pages, calls `ext4_fsync_journal` (`fsync.c:97-116`), and issues its own cache flush when needed, checking the status (`fsync.c:166-170`).

    `ext4_fsync_journal` handles the two cases:
    - a directory forces a full commit (`fsync.c:108-109`), through `__jbd2_journal_force_commit` (`journal.c:499-527`), which waits for the running transaction, else the committing one, and with neither returns "Nothing to commit" without testing the abort flag (`journal.c:513-517`);
    - a regular file asks whether its transaction will send the barrier (`fsync.c:111-113`, `journal.c:603-645`; a committed one needs ext4's own flush, `journal.c:611-613`), then calls `ext4_fc_commit` (`fsync.c:115`). Without fast commits that is `jbd2_complete_transaction` (`fast_commit.c:1207-1208`, `journal.c:787-808`): it commits and waits for a running transaction (`journal.c:792-798`), waits for a committing one, and returns 0 for any other without testing the abort flag (`journal.c:800-805`).

    `jbd2_log_wait_commit` (`journal.c:652-692`) tests the flag only after its wait loop (`journal.c:678-686`, `journal.c:689-690`). `kjournald2` commits a running transaction on its timer (`journal.c:197-204`, `journal.c:241-245`), so a transaction can complete before an `fsync` examines it. An inode's sync tids are set at load to the running or committing transaction (`inode.c:5400-5416`), and updated only by a handle that has not aborted (`ext4_jbd2.h:354-365`).
  - **Aborts.**
    - `jbd2_journal_abort` (`journal.c:2549-2597`) sets the flag under the state lock (`journal.c:2565-2589`). It "cannot be undone without closing and reopening the journal" (`journal.c:2515-2516`).
    - Every abort site of `jbd2_journal_commit_transaction` (`commit.c:548`, `commit.c:614`, `commit.c:642`, `commit.c:785`, `commit.c:866`, `commit.c:878`, `commit.c:889`) precedes the transaction's completion (`commit.c:1102-1105`).
    - On an aborted journal no commit record is written (`commit.c:126-127`), and buffers are un-journaled rather than written home (`commit.c:582-600`, `commit.c:1014-1018`).
    - `ext4_journal_check_start` (`ext4_jbd2.c:65-91`) returns an emergency state first (`ext4_jbd2.c:72-74`). It turns a journal "aborted behind our backs (eg. EIO in the commit thread)" into `ext4_abort` (`ext4_jbd2.c:81-89`), called from `__ext4_journal_start_sb` (`ext4_jbd2.c:93-117`).
    - `start_this_handle` (`transaction.c:312`) refuses an aborted journal with EROFS and sets nothing (`transaction.c:366-371`).
    - `ext4_abort` forces read-only (`ext4.h:3197-3198`). `ext4_handle_error` (`super.c:680-733`) sets `EXT4_FLAGS_EMERGENCY_RO` (`super.c:732`), not `SB_RDONLY` (`super.c:727`), or panics under `errors=panic` (`super.c:717-720`). `ext4_emergency_state` returns EIO or EROFS (`ext4.h:2267-2274`).
  - **The probe.** An owner's timestamp update starts a journal handle:
    - `vfs_utimes` (`utimes.c:20-66`) calls `ext4_setattr` (`inode.c:5845`), which returns an emergency state first (`inode.c:5854-5856`) and then marks the inode dirty (`inode.c:6052-6056`);
    - `__mark_inode_dirty` (`fs-writeback.c:2495-2526`) calls `ext4_dirty_inode` (`inode.c:6531-6540`), which starts a handle and swallows its error.
  - **Flushes and the log tail.**
    - The commit record goes out with PREFLUSH and FUA unless commits are asynchronous (`commit.c:114-159`, `commit.c:152-154`). Its completion is checked (`commit.c:165-178`, `commit.c:874-881`, `commit.c:888-889`).
    - Two flushes discard their status: the external journal's filesystem-device flush (`commit.c:775-778`), and the asynchronous commit's final flush (`commit.c:781-786`, `commit.c:883-886`).
    - Ordered-data wait errors are printed and discarded (`commit.c:738-744`); they stay in the file's errseq (`commit.c:240-247`).
    - At commit, the tail moves only after the commit record's checked flush (`commit.c:746-765`, `commit.c:899-900`).
    - At a checkpoint, `jbd2_cleanup_journal_tail` (`checkpoint.c:318-342`) discards the status of its flush (`checkpoint.c:338-339`). It then calls `__jbd2_update_log_tail` (`journal.c:1056-1091`) and `jbd2_journal_update_sb_log_tail` (`journal.c:1852-1885`), which:
      - tests the abort flag (`journal.c:1859-1860`);
      - tests for a failed home write (`journal.c:1861-1864`, `jbd2.h:1701-1707`), recorded through `buffer.c:165-176` and `buffer.c:1214-1222`;
      - writes the superblock with FUA (`journal.c:1069`); a write error aborts (`journal.c:1827-1837`).
    - That path is reached at an ordinary handle start when log space runs low (`transaction.c:272-279`, `checkpoint.c:49-124`, `checkpoint.c:154-298`), and by `jbd2_journal_flush` (`journal.c:2402-2470`) and `jbd2_journal_destroy` (`journal.c:2116-2193`).
    - Recovery replays from the superblock's tail (`recovery.c:282-345`, `recovery.c:611`), with its own flush checked (`recovery.c:339-343`).
    - `blkdev_issue_flush` returns the flush's status and records it nowhere (`blk-flush.c:468-474`). On a device without a write-back cache the block layer completes a flush without sending it (`blk-core.c:809-820`).
  - **The profile.**
    - An internal journal is opened by `jbd2_journal_init_inode` (`journal.c:1667-1697`) on the filesystem's own device (`journal.c:1684`) and named "<device>-<inode>" (`journal.c:1691-1693`). An external one is opened by `jbd2_journal_init_dev` (`journal.c:1641-1657`) and named after its own device (`journal.c:1651-1653`). `/proc/fs/jbd2/<name>/info` is world-readable (`journal.c:1222-1229`).
    - `ext4_load_journal` (`super.c:5984`) chooses by the superblock's journal inode and journal device (`super.c:5998-6020`) and refuses both at once (`super.c:6006-6010`).
    - `noload`, printed `norecovery`, runs with no journal at all (`super.c:5415-5444`).
    - `journal_async_commit` with `data=ordered` refuses to mount (`super.c:4963-4968`). The jbd2 async-commit feature follows that option alone (`super.c:4058-4095`).
    - `JBD2_BARRIER` follows `barrier` at every mount and remount (`super.c:5768-5788`).
    - The per-superblock defaults come from the superblock's default mount options and stored option string (`super.c:4330-4386`, `super.c:5298-5303`). Fast commits come from the superblock feature (`super.c:4349-4350`) or the debug-only `fc_debug_force` (`super.c:1893-1896`).
    - `_ext4_show_options` (`super.c:2918-3042`) skips an option equal to the per-superblock default unless asked for every option (`super.c:2949-2951`, the data mode `super.c:2985-2993`). It names `noload` by its first name (`super.c:2903-2911`, `super.c:1726-1727`), and shows `emergency_ro` and `shutdown` (`super.c:3034-3038`). The table entries are `super.c:1854-1860`.
    - `/proc/fs/ext4/<device>/options` prints every option in its effective form (`super.c:3049-3058`) and is world-readable (`sysfs.c:571-583`).
  - `do_fsync` (`sync.c:205-213`): an `O_PATH` descriptor is rejected.
  - `journal.rst:31-40`: fast commits log minimal per-inode deltas outside the full-commit order.

**PostgreSQL** `data_sync_retry` (`https://www.postgresql.org/docs/current/runtime-config-error-handling.html`) is corroboration only: "the second attempt may be reported as successful, when in fact the data has been lost."

**Dependencies.** `libc` 0.2.183 is the locked version for this crate's Linux target. Its source declares `SYS_openat2`, `open_how`, `RESOLVE_*`, `statx`, `STATX_MNT_ID`, `AT_EMPTY_PATH`, `fstatvfs`, `_SC_PAGESIZE`, `flock` and `fdatasync`. The crate also depends on `sha2` 0.10.9 and `hex` 0.4.3 [S, `Cargo.lock`].

**Host observations** [I], read only:
- Linux 6.17.0-35-generic.
- `/`, `/home` and `/var/lib` on ext4, mounted `rw,relatime`.
- `fs.protected_hardlinks = 1` and `fs.protected_symlinks = 1`.
- A `github-runner` account with uid 1001.

None of this is host qualification, and none of it selects a state root.

---

## 2. Architect dispositions and contract inventory

### 2.1 Final dispositions D-1 to D-10

| Id | Disposition, as applied here |
|---|---|
| **D-1** | Root-controlled provisioning and administrative disposition are kept. Ordinary opening never creates, repairs or selects anything. Its activation (R3, §10.6) only makes the visible, selected namespace durable. The layout is corrected (§6). |
| **D-2** | Paths are chosen by the Owner, never here. Capacities are finite. The initial targets are N=256 pool files and C=512 records per journal; with the safe geometry the pool costs 538,968,064 bytes (§6.6). The Owner may provision a smaller N. Every bound is checked before allocation or traversal (§7.7). |
| **D-3** | Refusal is conservative. An unsealed generation that could have admitted native work is refused. A recorded-unsettled count of zero is never proof (§11). |
| **D-4** | A0 is specified only as limited crash evidence, never as tamper-resistant admission authority. Gate **G-AUTH** stays open for production or runner use. No A1–A5 helper, account, capability, TPM or cloud dependency is introduced. The earlier requirement that data the store uid writes must not lift a refusal is **kept as an open requirement**, not dropped (§3.3). |
| **D-5** | A stall is reported, never acknowledged and never failed by timeout. Admission, failure and late completion are defined exactly. Recorder loss and I/O failure are latched (§9.6). |
| **D-6** | Initial support is ext4 only. It is identified through the descriptor's mount, with the supported options pinned at provisioning. XFS is deferred. Ambiguity refuses (§6.4). Nothing is qualified now. R4 narrows the profile to an internal journal, `data=ordered`, barriers, no fast or asynchronous commits and normal journal loading, each read in its effective state (§6.4). |
| **D-7** | Maintenance is offline and root-authorized, and nothing is automatic. Each retained procedure is fully specified; anything else is unsupported (§13). |
| **D-8** | No provisioning executable. A manual procedure and a read-only verifier contract are specified, including how `PROVISION` itself is made durable (§13.2, §13.11). |
| **D-9** | The NXCD version-1 frame stays byte-identical. The container, header and seal are new. No store has been deployed, so no migration is required. |
| **D-10** | Hardware power-loss tests are deferred. They would be empirical qualification, not exhaustive proof. The physical assumptions stay explicit (§5). |

### 2.2 Contract inventory at the source baseline

| Element | Implemented [S] | Adapter behaviour [P] |
|---|---|---|
| `RecordIntent` and its digest | `model.rs:451-455`, `model.rs:457` | The recorder writes `codec::encode_record(intent)` unchanged (§7.4) |
| `RecordAck` | `model.rs:466-479` | Built by the owner from its retained intent once `durable_through` covers it (§9.4) |
| `RecordSink::submit`, which returns nothing | `core.rs:89-95` | An idempotent, non-blocking insertion into the exchange (§9.3) |
| `flush_records`: unsent records in order; a sink panic is contained, recorded as a failure, and the record stays unsent | `core.rs:2958-2991` | Re-submission after a panic is idempotent (§9.3) |
| `acknowledge` → `Ledger::acknowledge`: in order and exact; `NotIssued`, `Duplicate`, `Conflict`, `LedgerFailed`, `OutOfOrder` and `Foreign` otherwise | `core.rs:2996-3010`, `core.rs:1042-1069` | Applied exactly once per sequence, never beyond P. Any outcome other than `Acknowledged` latches the fatal cause `UnexpectedAck` and stops application (§9.4). |
| `record_failed` → `Ledger::fail`: only the next unacknowledged record, and only an issued one; `NotIssued`, `Conflict`, `OutOfOrder`, `Foreign` otherwise, and `LedgerFailed` afterwards | `core.rs:3016-3031`, `core.rs:1072-1090` | Called once, for the core's next unacknowledged record, and only once that record has been issued. Nothing is reported while no such record exists (§9.5). |
| A failure recorded by any entry point closes admission with `ClosureReason::Failed`; `Control::cancel` closes only with `ClosureReason::Cancelled`, and an actual failure is never recorded as a cancellation | `core.rs:3046-3069`, `core.rs:554-556`, `model.rs:377-384` | `Control::cancel` is never used to report a recorder failure. The store admission gate refuses admission instead (§9.8); API-5 is the narrow future alternative (§2.3). |
| The core's `lock()` helper recovers a poisoned mutex | `core.rs:367-371` | The exchange does not: a poisoned exchange mutex is a fatal cause (§9.5) |
| Capacity: general and control reserve, all-or-nothing reservations; a control reserve of at least 2 | `core.rs:967-968`, `core.rs:980-989`, `core.rs:1004-1011`, `core.rs:1298-1304` | Every issuable record has a pre-written zero block (§6.6) |
| Admission needs an acknowledged start record. Case begin and the commitment need everything acknowledged. | `core.rs:1658-1675`, `core.rs:1542-1544`, `core.rs:2663-2669` | Write-ahead (INV-2). `Custody::admit` is called only through the store admission gate (§9.8). |
| `Custody::admit` decides in its own gate critical section and performs no I/O, callback or wait: clock check, closure observation, reservation and acknowledgement checks, the gate, then the snapshot | `core.rs:1626-1697`, `core.rs:3033-3039`, `core.rs:3072-3077` | This is what lets the store hold the exchange mutex across the call (§8.2 rule 4) |
| Terminal record; finality; published verdict | `core.rs:2678-2686`, `core.rs:2690-2697`, `core.rs:2710-2718` | — |
| Closure refusal; `close`; no closure record kind in version 1 | `core.rs:2872-2924`, `core.rs:2929-2954`, `model.rs:395-449` | A closure seal outside the frames (§7.3, §9.7) |
| Requests: sequencing, in-memory receipts, another generation's request `Foreign` | `core.rs:2722-2785`, `core.rs:2745-2747` | Not persisted: a restart is a new generation |
| Prior incidents: `IncidentBinding`, `PriorOutcome` and `PriorIncident`, at most `incident_limit` | `model.rs:683-702`, `core.rs:1298-1304` | Computed by the store (§11, §12.3) |
| `apply_disposition` (NotStarted; exactly one binding; validator contained); `Prior::blocks` | `core.rs:1410-1447`, `core.rs:1207-1209` | Root-owned dispositions (§13.4) |
| `RunStarted { dispositioned }`, a count only | `core.rs:1476-1482` | The header lists the applied bindings (§7.2) |
| `ValidatedDisposition`; disposition reasons | `model.rs:705-711`, `model.rs:713-718` | — |
| Shared control-side state: one lock per field, each held only for a copy or an assignment | `core.rs:431-442` | The recorder's state follows the same rule (§8.2) |
| Codec version 1: frame layout, `MAX_FRAME`, largest record payload 83 | `codec.rs:22-31`, `codec.rs:110`, `codec.rs:115` | Frames of 73 to 123 bytes per record (§7.4) |

### 2.3 API candidates, not made and not assumed approved

The design needs no change to `core.rs`, `model.rs` or `codec.rs`. Candidates for a later approved mission:

| Id | Candidate | Without it |
|---|---|---|
| API-1 | Closure and failure-detail record kinds in a new record version | A closure seal (§7.3). Failure detail beyond the version-1 records is not durable (§14.3). |
| API-2 | Constructing a custody with already-dispositioned priors, or a larger `incident_limit` | Explicit refusal plus Owner archival (§13.10) |
| API-3 | `RecordSink::submit` returning a result | The exchange latches, and the owner reports `record_failed` (§9.5) |
| API-4 | The next unacknowledged `RecordId` in `EvidenceView` | The owner keeps its own copies of submitted intents (§9.4) |
| API-5 | A control-side failure closure: `Control::recorder_failed(now)`, closing the core's gate with `ClosureReason::Failed(FailureClass::RecorderFault)` exactly as an owner-side failure does (`core.rs:448-465`, `core.rs:3046-3069`), never as a cancellation | The store admission gate refuses every admission after the latch (§9.8), and the core learns of the failure at its next issued record (§9.5). The gap that remains: until such a record exists, the core's own state and a terminal commitment made in that window do not show the failure. Such a commitment is never final or published (§9.9). |

**API-5, rationale and validation obligations.** Not implemented and not approved.
- **Why.** It closes the core's own gate at the instant the store latches, whether or not a record exists. The closure linearizes in the gate's critical section against admission and against the terminal commitment (`core.rs:1665-1675`, `core.rs:473-482`), so a run whose recorder failed before its commitment always commits `Failed`.
- **Shape.** It reuses the existing `FailureClass::RecorderFault`, whose documentation would widen from "the adapter panicked while a record was submitted" (`model.rs:310-311`) to "the recorder could not continue". The NXCD version-1 bytes stay unchanged, since the class already has a code. It changes no acknowledgement, never retracts one, and is accepted once; a later call returns the first closure, as `Shared::close` does.
- **Validation obligations** for the mission that would make it:
  - a core test that it closes admission before an admission and before a commitment, and is late after the commitment;
  - a test that the closure is recorded as `Failed(RecorderFault)`, never `Cancelled`;
  - a test that a later `record_failed` at a real record is still applied;
  - negative controls: the closure recorded as a cancellation; a closure after the commitment changing the verdict;
  - the existing `core-tests` unchanged and passing.

---

## 3. Threat and fault domains

### 3.1 Domains

Every invariant (§12.1) names the domains in which it holds.

| Domain | Meaning |
|---|---|
| **H**, honest reachable execution | The owner and recorder follow this protocol, and the core behaves as its tests establish. Storage meets the supported profile (§5). The faults are process termination (F1) and machine or power loss (F2), in any order and number, including F1, then restart, then F2. An error a system call of the store returns (EIO, EROFS, ENOSPC, EINTR) is handled fail-closed in this domain as in every other. R4: a cache flush that the device fails is not part of H (A-S5, §5.1), because the kernel discards one such status. |
| **M**, malformed bytes | Arbitrary content in any store file. Required: totality, bounded resources, determinism, and refusal of anything structurally invalid. **Not** required: semantic truth. Arbitrary bytes can form a well-formed but misleading journal. |
| **S**, storage faults (F3) | Bit flips, torn writes outside the containment unit, misdirected or lost writes, a device that lies about flushing, and (R4) a cache flush that the device fails. Required: refusal of every detectable fault. Undetectable faults are outside every guarantee; §12.2 names the one R4 found. |
| **A**, adversarial rewriting (F4) | Any process with the store uid's write authority, including CI job code running as that uid. It can rewrite every uid-owned byte consistently, because nothing is keyed. It cannot forge root-owned files, given the account assumptions below. |

### 3.2 Account assumptions [A]

- The store uid has no root and no `sudo`. It also has none of `CAP_CHOWN`, `CAP_FOWNER`, `CAP_DAC_OVERRIDE`, `CAP_DAC_READ_SEARCH`, `CAP_LINUX_IMMUTABLE`, `CAP_SYS_ADMIN` or `CAP_MKNOD`.
- Root, the Owner, is trusted.
- `fs.protected_hardlinks = 1` and `fs.protected_symlinks = 1`. Opening refuses otherwise (§10.2).

### 3.3 What A0 is and is not

- **A0 is crash evidence.** In domain H it gives write-ahead, acknowledgement-implies-durable and no false resolution (§12.1). In domains M and S it gives detection. Dispositions are root-only.
- **A0 is not admission authority against domain A.** A process running as the store uid can rewrite a journal so that an unresolved generation classifies as sealed, or as having no native work. It can zero the last claimed journal. The seal is unkeyed, so in domain A it proves nothing about `close()`.
- **G-AUTH.** Root-owned dispositions do **not** protect the uid-writable inputs to classification. The requirement that nothing a store-uid process writes can lift a refusal therefore remains **unmet and open**. Before any production or runner use, the Architect must approve an end-to-end authority model (an anchor, privilege separation, or something else). No such mechanism is introduced here.

---

## 4. Storage state model (R1)

### 4.1 State components

| Component | Meaning | After F1 (process death) | After F2 (power loss) |
|---|---|---|---|
| Process-local state | The owner's and recorder's memory: `Custody`, the exchange, cursors, latches | Lost | Lost |
| Kernel-visible bytes **K** | What `read` returns now, from the page cache or the medium | Unchanged | Become **D** |
| Durable bytes **D** | Bytes on stable media | Unchanged | Unchanged (A-S1) |
| Pending writeback | Dirty pages whose **K** has not yet reached **D** | Still pending: written back later, or failing later | Lost |
| Error sequence **E** | The inode mapping's writeback-error sequence: a counter and a "seen" flag (errseq) [U] | Unchanged while the inode stays cached | Lost. Also lost when the inode is evicted, since it is in-memory state. |
| Error cursor | Per open file description: the error sequence sampled at open and advanced when an error is reported through it [U] | Destroyed with the description | Lost |
| Directory entries | Visible names **K_dir**; a log of metadata operations not yet guaranteed durable; the guaranteed durable names (§4.8) | Unchanged; the log is still pending | The names of one **permitted schedule** (§4.8): at least everything a completed directory `fsync` guaranteed, at most everything visible |
| Advisory locks | Attached to open file descriptions [U] | Released when the last duplicate closes, which process death causes | Gone |

### 4.2 Events

| Event | Effect |
|---|---|
| `pwrite` | Updates **K** for the bytes written, which may be fewer than requested, or none. Marks those pages pending. |
| Background writeback, success | The page's **D** becomes its **K**; the page is clean |
| Background writeback, failure | **E** records an unseen error, and the page is clean but **D** is unchanged. **K** may keep the new bytes until the page is evicted. |
| `fdatasync(d)` | Writes back every pending page of the file and flushes the device. Returns EIO if **E** has advanced past *d*'s cursor (an error recorded after *d* was opened, or after the last error reported through *d*), then advances the cursor and marks the error seen. Otherwise returns 0. |
| Open | The new description's cursor is sampled: before the newest error if no description has seen it yet (the new description will report it), otherwise at it (the new description never reports it) [U] |
| Page reclaim | Any **clean** page may be dropped at any time, **including while descriptors on the file are open**, so that page's **K** reverts to **D**. A page whose writeback failed is clean, so reclaim can remove bytes that only **K** held. An open descriptor keeps the inode, not its clean pages. |
| Inode eviction | Only when no descriptor is open and nothing else holds the inode. It drops **E** and every page. |
| Metadata operation | `create`, `link`, `unlink` and `rename` change **K_dir** at once and append to the pending metadata log (§4.8). A directory `fsync` that returns 0 guarantees its operations, and under A-M1 every earlier one. |
| F1 | Process-local state is lost. Descriptors close and their locks are released. **K**, **D**, pending writeback, **E** and the pending metadata log are unchanged. |
| F2 | **K** becomes **D**. Pending writeback, **E** and all cursors are lost. The directory entries become those of one permitted schedule (§4.8); pending file data whose sync had not returned ends old or new. A block being written may end old, new or indeterminate, but only within its containment unit (§5). |

### 4.3 What a sync certifies

| Who syncs | In domain H, `fdatasync` returning 0 certifies | It does not certify |
|---|---|---|
| **The writer**: the recorder's I/O description, opened at the claim before any of its writes and kept open | Every page the writer dirtied before the call reached **D**, and the device was flushed (A-S1). A writeback failure of those pages, before or during the call, is reported to this still-open description [U, vfs]. R4, on the supported profile, how the flush happens: a record block is a pure overwrite of a written, preallocated extent (§6.5), so the inode's datasync transaction has completed and ext4 issues and checks its own cache flush (`fsync.c:111-113`, `fsync.c:166-170`). The exception is an inode loaded while a transaction ran (`inode.c:5400-5416`). The first sync after that load waits for that transaction instead. If the transaction completes between ext4's two tests, the sync returns 0 without the flush and without the abort test (`journal.c:800-805`). The startup's preservation sync is normally that first sync. If the inode was reloaded, it is the claim's header sync, and the first record's sync flushes the device again before any acknowledgement (§9.7). | — |
| **A newly opened description**: startup, the verifier, any other process | Only that the pages pending at the time of the call were written successfully | That earlier writebacks succeeded. An error already reported through another description, now closed, is never reported to it ("new callers will not see an old error" [U, errseq]). An error held by an evicted inode is gone. Pages whose writeback failed are clean; they may still show new bytes in **K** while **D** lacks them. |
| **A newly opened description of a directory** (R3; R4 precise): activation | On the supported profile, when the sync waited for a running or committing transaction: that the operations issued before the call are in transactions that committed without an abort by the time of its abort test (`journal.c:499-527`, `journal.c:689-690`). Committed means the commit record went out with a cache flush and forced unit access, and its completion was checked (`commit.c:152-154`, `commit.c:888-889`). | When it waited for nothing: anything. It returns 0 having committed and tested nothing (`journal.c:513-517`), and an earlier commit may have failed silently; the probe's handle start detects that (§10.6, §10.7). In no case: that those transactions stay durable through a later checkpoint whose flush the device fails (A-S5). |

Neither this design nor any report it specifies may present a sync on a newly opened description as evidence that no earlier writeback failed [M: C03].

### 4.4 What startup may infer from the bytes it reads

- **A visible valid record (domain H).** The recorder wrote it and the core issued it. It is a true statement of what the core issued at that instant. It may not be durable: it may exist only in **K**, and a later F2, page reclaim or inode eviction may remove it.
- **A missing record proves nothing.** It may never have been issued, may have been lost by F2, may have failed writeback, or may never have been written.
- **A visible valid seal (domain H).** `close()` returned `Ok` at that time. The seal may not be durable.
- **A visible valid header with no records (domain H).** Under write-ahead, no action of that generation was admitted (INV-2).

### 4.5 Historical uncertainty that cannot be removed

Startup cannot know:
- whether the visible bytes equal **D**;
- whether records beyond the visible end were issued;
- whether an earlier writeback failed;
- whether native effects occurred that no durable record describes. Examples:
  - a late owner delivered before its `IncidentOpened` became durable (`core.rs:1924-1930`, `core.rs:1931-1987`) [S];
  - an owner surfacing after `close()`, which is outside custody (§14.4).

### 4.6 Consequences

- **Decisions use the visible bytes.** The refusal decision reads **K** and never needs durability. In domain H visible records are true, and missing records are handled conservatively by the refusal rule (§11.2).
- **Less evidence never hides admitted work.** F2, page reclaim or inode eviction may later remove visible-only bytes. Re-classification then sees less, and the class may change: a torn visible block, for example, can become a shorter valid prefix. But an action whose start record never became durable was never admitted (INV-2), so no later view hides admitted native work [M: C16].
- **Evidence-preservation sync.** Startup syncs each prior journal after locking it (§10.2 step 5).
  - This **changes persistence state**: every page still dirty at the call becomes durable.
  - It does **not** make durable a page whose earlier writeback failed. That page is clean, so the sync does not write it, and the error was already reported to another description, so the sync returns 0 (§4.3). Such bytes stay visible-only until reclaim or F2 removes them.
  - The report therefore says "preservation sync returned 0", never "evidence durable" or "evidence preserved".
  - It changes no application state and certifies no history.
  - An error from it refuses the store as "evidence unreliable" [M: C02, C03].
- **Failed evidence is not cleanup.** A record that is visible but failed is still only a statement of what the core issued. Native cleanup success is established only by the core's own facts at the time. A stored `ActionSettled` is a past statement (§14.2).

### 4.7 Sequences the model exercises [M]

| Sequence | Outcome |
|---|---|
| The writer `pwrite`s record *n*, then F1 | **D** is unchanged and **K** shows *n*. Process death made nothing durable [C01]. |
| F1, restart, F2 before any sync | *n* is lost [C02] |
| F1, restart, a startup sync that returns 0, then F2 | *n* is kept [C02] |
| *n*'s writeback fails, the writer's sync reports EIO, then F1 and restart | A new description's sync returns 0, yet **D** lacks *n* while **K** shows it. After inode eviction **K** loses *n* too. The report must not claim *n* is durable [C03]. |
| *n*'s writeback fails while the writer's descriptor stays open; the page is reclaimed | **K** loses *n* although the descriptor is open. The inode and **E** stay. The generation is still refused (unsealed) [C22]. |
| The writer dies before observing *n*'s writeback failure; restart | The error is unseen, so the new description's sync returns EIO and startup refuses [C03] |
| Short write, zero-progress write, EINTR | A block counts as written only once all 4096 bytes are written. A bounded number of zero-progress results or EINTR poisons the journal [C15]. |

### 4.8 Directory metadata: what may be durable before a directory sync

R1 treated a directory entry as durable only after an `fsync` of its directory, and as absent after F2 otherwise. That is "not guaranteed durable", restated wrongly as "guaranteed absent". On ext4, a `link`, `unlink` or `rename` can reach the journal at any later commit, before any explicit directory sync. R2 keeps five states apart:

| State | Meaning |
|---|---|
| Visible | In **K_dir**: what a lookup returns now |
| Possibly durable | In the outcome of at least one permitted schedule below |
| Guaranteed durable | Covered by a directory `fsync` that returned 0 (and, under A-M1, every earlier metadata operation). R4: a sync that waited for no transaction certifies nothing about an earlier commit that failed silently (§4.3). No later decision depends on such an operation before an activation, which detects that failure (§10.7). |
| Sync-certified | Reported durable by a sync the procedure itself issued and checked. For file data, only the writer's own sync certifies (§4.3). |
| Startup-observable | What an opening after F1 or F2 can see: the visible state after F1, one permitted schedule after F2 |

**Pending metadata log.** Every `mkdir`, `create`, `link`, `unlink` and `rename` is one operation. A rename has two halves: the source name removed and the target name added.

**A-M1 [A], ordered metadata.** On the supported ext4 profile (§6.4, §13.2: an internal journal, `data=ordered`, barriers, no fast or asynchronous commits, normal journal loading), the jbd2 journal commits metadata operations in transactions, in issue order, each transaction atomically. A power loss therefore leaves a namespace equal to the visible namespace after some **prefix** of the operation log, and that prefix contains every guaranteed operation. A rename is never split. This is an assumption of the profile, attested with A-S1 to A-S4 and qualified later (D-10); software cannot prove it.

| Schedule family | Outcomes after F2 | Status |
|---|---|---|
| **Ordered** | Every prefix of the log from the last guaranteed operation to the last operation issued | A superset of ext4 outcomes under A-M1: ext4 commits whole transactions, which are prefixes too |
| **Per-directory** | Each directory independently keeps a prefix of its own operations and rename halves, at least up to its own last completed `fsync`. A rename can therefore be split, within one directory or, for a rename between two directories, with either half persisting without the other. | A deliberate **over-approximation**, not ext4 behaviour. It shows which conclusions do not depend on A-M1. |
| Guaranteed only | Exactly the guaranteed operations | R1's model. Not a permitted schedule of ext4: it omits outcomes that occur. |

**Data.** A file synced before it is published is durable before its name can be. Pending data whose sync had not returned ends old or new, and the model runs both.

**Result** [M: C12, C22]. Under both families, no crash point of a single procedure accepts a partial file, honours a temporary entry, loses an incident silently or retires unresolved history. The only outcome the per-directory over-approximation adds is a revocation interrupted between its two directory syncs leaving both names (Invalid, a refusal, §13.4); under A-M1 a rename is atomic and that outcome does not occur. Every crash outcome, per family, is in §15.2.

**Composed (R3)** [M: C26]. Chains of procedures, and procedures followed by an owner's work, were checked only in R3.
- An archival interrupted after its link, then a recycling and a retirement in later sessions, can drop the retired claim's archive copy under the per-directory over-approximation, unless the later sessions first make the link durable. R2 depended on A-M1 there.
- A publication still only visible can be rolled back after an owner's work depends on it (§10.6).

Activation, by owners and by sessions, removes both dependences.

---

## 5. Physical storage assumptions and containment (R2)

### 5.1 Assumptions [A]

- **A-S1, flush honesty.** When `fdatasync` returns 0, the data is on stable media: the device honours cache flushes. sysfs `write_cache` may report a write-back cache [U]; flushes must still be honoured.
- **A-S2, containment.** A failed or interrupted write of one aligned 4096-byte block, issued by the filesystem for one page, can leave only that block indeterminate. It never alters any other block's durable content.
- **A-S3, no silent loss.** After a successful flush the device neither loses nor misdirects writes. This is the boundary of domain S.
- **A-S4, read stability.** Reading durable data returns those bytes or an error.
- **A-S5, flush success (R4, new).** The device completes every cache flush the kernel issues to it. A flush the device fails is a storage fault outside domain H (§3.1).
  - **Why it is needed.** At a checkpoint, v6.17 jbd2 discards the status of the flush it issues before moving the log tail (`checkpoint.c:338-339`).
    - Suppose that flush fails while the superblock update succeeds.
    - Transactions then leave the log, but their home blocks may not be durable.
    - A power loss before a later successful flush can revert committed metadata, an activated selection's dependencies included, and no process receives an error.
  - **Every other flush on the profile's paths is checked:**
    - the commit record's flush (`commit.c:152-154`, `commit.c:888-889`);
    - `fsync`'s own flush (`fsync.c:166-170`);
    - recovery's flush (`recovery.c:339-343`).

    A failed home write is caught before the tail moves (`journal.c:1861-1864`).
  - **Why it is the minimum.** The store cannot observe the event (§10.7), and nothing within this design's means removes it without a kernel change. A device without a write-back cache meets A-S5 by construction: the block layer sends it no flush (`blk-core.c:809-820`), and A-S1 covers what the device reports about its cache.
  - **Status.** It is new: R1 to R3 did not state it, and R3's A-M2 implied that every journal I/O failure aborts the journal [M: C29 shows a failed checkpoint flush losing a certified dependency].
- **A-M1, ordered metadata** (§4.8). Metadata operations become durable in issue order, in atomic transactions. The safety results of §15.2 also hold under the per-directory over-approximation, which does not assume it; A-M1 only removes one extra refusal.
- **A-M2, the journal behaviour activation relies on (R3; corrected in R4).** On the supported profile, as the Linux v6.17 sources of §1.4 show:
  - **A directory `fsync`** waits for the running transaction, else the committing one, then returns EIO if the journal has aborted (`journal.c:499-527`, `journal.c:689-690`).
    - With neither, it returns 0 having committed and tested nothing (`journal.c:513-517`).
    - On a read-only superblock it returns 0 having committed nothing (`fsync.c:143-144`).
  - **A regular file's `fsync`** waits for its inode's sync transaction only while that transaction runs or commits, then tests the abort flag. For a completed transaction it returns 0 without that test, after ext4's own cache flush, whose status it checks (`fsync.c:111-115`, `journal.c:787-808`, `fsync.c:166-170`).
  - **Every `fsync`** returns an emergency state first (`fsync.c:135-137`).
  - **A journal abort** is permanent until the journal is closed (`journal.c:2515-2516`). A failing commit sets it before that transaction completes (`commit.c:1102-1105`).
  - **A handle start** turns an aborted journal into emergency read-only (`ext4_jbd2.c:81-89`, `super.c:732`). The exception is an abort landing after ext4's test and before jbd2's (`transaction.c:366-371`): that sets nothing.
  - **A timestamp update** by a file's owner starts a journal handle and joins the running transaction (`inode.c:6052-6056`, `inode.c:6531-6540`).
  - **A committed transaction's record** went out with a cache flush and forced unit access, and both were checked (`commit.c:152-154`, `commit.c:888-889`). The tail then moves at commit only after that flush (`commit.c:899-900`). At a checkpoint it moves after a flush whose status is discarded (`checkpoint.c:338-339`), hence A-S5.

  **The correction (F3).** R3 said that an `fsync` of the probed file "then waits for that handle's transaction". That holds only while the transaction runs or commits, and §10.7 restates what the probe establishes.

  These are read from the sources, not tested. The running kernel's build must be qualified (G-HOST).

### 5.2 How alignment and the layout relate to the assumptions

- **Geometry.** The header, the seal and every record each occupy one whole, aligned 4096-byte block. Each block is written at most once per generation (§6.3). In domain H the recorder has at most one record block written but not yet synced at any time (§9.4).
- **What alignment buys.** The supported profile requires:
  - filesystem block size 4096;
  - page size 4096;
  - device logical and physical block sizes that divide 4096.

  Given these, a record block is whole device sectors shared with no other block, and the page writeback of one record touches only its own block.
- **What alignment does not buy.** Alignment is necessary for A-S2 to mean anything, but it does not prove A-S2. A device can have larger internal units: flash pages, RAID stripes, read-modify-write cycles. sysfs `physical_block_size` is the device's own claim about its atomic unit [U], not a guarantee about other blocks.
- **The earlier geometry violated containment even under A-S2** [M: C04]. 32 records shared each page, and the header and seal shared block 0. One interrupted page write could therefore alter earlier durable records, or the header, while writing a later slot or the seal.
- **What a violation of A-S2 or A-S3 can do** [M: C04]. If an interrupted write damages another block, the damage is usually detected: a checksum fails or the prefix gains a hole, so the journal is Malformed and refused. **Not always:** if the damage turns the last durable record block back to zeros while the block being written also stays zero, the prefix silently loses an acknowledged record. That is why these remain explicit assumptions.

### 5.3 Verified later versus assumed

| Property | Status |
|---|---|
| ext4 through the descriptor's mount; mount options; filesystem block size 4096; page size 4096 | Verified at every opening (§6.4) |
| The effective mode (R4): `data=ordered`, barriers, no asynchronous commits, normal journal loading, neither emergency read-only nor shut down | Read at every opening, and again after activation's syncs, from the listing that prints every option in its effective form (§6.4 steps 9 and 10) |
| An internal journal (R4) | Read at every opening from the journal's jbd2 name (§6.4 step 10). Its superblock facts are established at qualification (below). |
| The kernel (R4) | Its behaviour (A-M2) is qualified by the Owner for the exact build, from the build's sources (G-HOST). Its identity is pinned in `PROVISION` and compared at every opening (§6.4 step 11). A matching identity binds the running kernel to that qualification. It is not itself qualification. |
| Device logical and physical block sizes (sysfs, for the mount's major:minor); pool extents written, not shared, not delayed (FIEMAP); `fallocate` support | Verified at provisioning as part of the Owner's qualification, then pinned in `PROVISION` (§13.2). Not re-read at runtime. |
| A-S1 to A-S5, A-M1 | Assumed. The Owner attests them in `PROVISION` (`storage-attestation`). They can be qualified empirically only (D-10), never proven. An attestation states them; it makes nothing true. |
| The ext4 journal at inode 8 with no external journal device; no `fast_commit` feature | Established by the Owner at qualification, as root, from the superblock (§13.2). The store uid cannot read the superblock, so these are not re-read at runtime. The Owner re-qualifies after any change, and a change made outside that is unsupported, like any root action outside a procedure (§13.1). |

### 5.4 When the assumptions cannot be established

- **A verifiable property does not match.** Opening refuses as **Unsupported**.
- **The effective profile cannot be read, is contradictory, or the kernel is not the qualified one (R4).** Opening refuses as **Unsupported** until the Owner re-qualifies (§13.3). Unknown information is never read as the supported value.
- **The Owner cannot attest A-S1 to A-S5 for the device.** The store must not be provisioned there; it stays Unprovisioned, and every run is refused.
- **A torn block in domain H** (non-zero and invalid) is never read as benign. The journal is Malformed and refused (§11.1). A torn unacknowledged write and corruption cannot be told apart.

---

## 6. Authority, layout and filesystem profile

### 6.1 Actors

| Actor | Identity | May write | Authority |
|---|---|---|---|
| **Owner** | root | `PROVISION`; root-owned directories; creation and replacement of pool files; dispositions; the archive | Provisioning, disposition, revocation, archival, recycling, retirement and succession. Each is an explicit root-owned artifact made through a §13 procedure, inside a maintenance session (§13.1). |
| **Custody owner process** | store uid | Through its recorder, the bytes of the one pool file it claimed. Locks on `LOCK` and that file. `LOCK`'s timestamps, through the activation probe (R3, §10.6). | None. It produces evidence; the core decides. |
| **Read-only verifier** | store uid or root | Nothing | None |
| **Other store-uid processes**, including job code | store uid | Whatever that uid can write (domain A) | None recognized |

### 6.2 Layout

```
<PROVISION_PATH>               root:root 0444  identity, parameters, attestation (text, §7.5)
<PROVISION_PATH>.predecessor-<old root id>  root:root 0444  kept by a successor (§13.6)
<STATE_ROOT>/                  root:root 0755
  LOCK                         uid:gid   0600  store lock (flock); never written (activation updates its timestamps, §10.6)
  journals/                    root:root 0755  names fixed by root
    j00000.journal … j<N-1>    uid:gid   0600  pool files: preallocated, zero-filled
  dispositions/                root:root 0755
    <64 hex>.disposition       root:root 0444
    revoked/                   root:root 0755
      <64 hex>-<YYYYMMDDTHHMMSSZ>.disposition  root:root 0444
  archive/                     root:root 0755
    j-<claim>-<32 hex>-<64 hex>-<s|n|u|m>.journal  root:root 0444
    p-<5 digits>-<64 hex>.journal                  root:root 0444
```

`<PROVISION_PATH>` and `<STATE_ROOT>` are Owner choices [D-2].
- The proposed forms are `/var/lib/nexus-os/phase2-custody/provision/uid-<uid>.provision` and `/var/lib/nexus-os/phase2-custody/roots/uid-<uid>-<root id>` (R3: R1 and R2 proposed `/etc` for `PROVISION`, which can be another filesystem).
- Every ancestor of both must be root-owned and neither group- nor other-writable.
- **One filesystem (R3).** The directory holding `<PROVISION_PATH>`, the parent of `<STATE_ROOT>` and `<STATE_ROOT>` itself must be on one filesystem, and `<STATE_ROOT>` must not be a mount point. Activation (§10.6) syncs and certifies the `PROVISION` directory through the journal of the filesystem that holds the store's `LOCK`, and the qualified profile (§5) then covers it too. This is verified at every opening (§6.4 step 8).
- Nothing in this design selects or creates either.

Because the directories are root-owned, the store uid cannot create, delete or rename any entry. Whole-journal deletion by that uid is therefore impossible. It can still overwrite its pool files' **contents** (domain A), or change their size, which is caught by the size check (§7.7).

### 6.3 Journal geometry

All offsets are in bytes, with B = 4096.

| Block | Offset | Content | Written |
|---|---|---|---|
| 0 | 0 | Header (§7.2) | Once, at the claim |
| 1 | 4096 | Closure seal (§7.3) | Once, after `close()` returns `Ok` |
| 1 + *n*, for *n* = 1 … capacity | 4096 × (*n* + 1) | Record *n*: its version-1 frame, then zeros (§7.4) | Once, by the append protocol |
| 2 + capacity … 1 + C_pool | — | Zeros | Never |

- **File size.** (C_pool + 2) × 4096.
- **Write-once rule.** No block that holds data is rewritten during its generation. Provisioning's zero-fill comes before the claim.

### 6.4 Filesystem profile: ext4 only [D-6]; narrowed in R4

`statfs` magic `0xef53` is shared by ext2, ext3 and ext4 [U]. It is never used to identify ext4.

**The supported profile (R4).** Each property, why the design needs it, and how it is established. "Runtime" means observed at every opening and again after activation's syncs (§10.6 A4); "qualification" means a root-controlled fact the Owner establishes and keeps (§13.2).

| Property | Needed because (Linux v6.17, §1.4) | Established by | Refuses |
|---|---|---|---|
| ext4, through the mount | Only ext4's behaviour was read | Runtime, bound to the root descriptor: steps 1–4 | Another type, or an ambiguous record |
| One filesystem | The probe and the qualified profile must cover the `PROVISION` directory | Runtime: step 8 | Different `st_dev` |
| An internal journal | An external journal flushes the filesystem device with the status discarded (`commit.c:775-778`), and `fsync` relies on that flush (`journal.c:633-636`) | Qualification: the superblock's journal is inode 8, with no journal device. Runtime: step 10. | No `<name>-8` jbd2 entry |
| `data=ordered` | A-M1. v6.17 refuses it together with asynchronous commits (`super.c:4963-4968`). | Runtime: step 9 | Another mode, or none |
| Barriers | The commit record's flush and forced unit access (`commit.c:152-154`); `fsync`'s own flush (`fsync.c:111-113`) | Runtime: step 9. The flag follows every remount (`super.c:5768-5788`). | `nobarrier`, or no `barrier` |
| No asynchronous commits | The asynchronous commit's final flush discards its status (`commit.c:883-886`) | Runtime: step 9. Also excluded by `data=ordered` at v6.17. | `journal_async_commit` |
| No fast commits | A file's `fsync` would take the fast-commit path, which §10.7 does not cover. Fast commits also log outside the full-commit order (A-M1, `journal.rst:31-40`). | Qualification: the superblock lacks the feature, the only enabler with the debug option (`super.c:4349-4350`, `super.c:1893-1896`). Runtime: step 9 refuses `fc_debug_force`. **The store uid cannot observe the feature itself**; the Owner keeps it (§13.2). | At qualification |
| Normal journal loading and recovery | `noload` runs with no journal at all (`super.c:5415-5444`) | Runtime: steps 9 and 10 | `norecovery`, or no jbd2 entry |
| Neither emergency read-only nor shut down | Every sync then fails | Runtime: step 9 (`super.c:3034-3038`), and the system calls themselves | Listed |
| The qualified kernel | A-M2 and §10.7 are claims about v6.17's code | Qualification of the exact build (G-HOST). Runtime: step 11 binds the running kernel to it. | Another identity |
| A-S1 to A-S5, A-M1 | §5.1 | Assumptions, attested by the Owner | Not provisioned |

**Kept apart.**
- **Filesystem identity versus journal location.** Filesystem identity (steps 1–4 and 8) is not the journal device's identity (step 10).
- **An absent string versus an effective state.** The absence of a string in `mountinfo` is not an effective state (step 6 versus step 9).
- **A version string versus qualification.** A kernel version string is not behavioural qualification (step 11).
- **An attestation versus a mechanism.** An attestation records what the Owner asserts; it makes nothing true.

Identification is bound to the retained root descriptor:

1. `statx(root_fd, "", AT_EMPTY_PATH, STATX_MNT_ID)` yields `stx_mnt_id` [U].
2. Read `/proc/<pid>/mountinfo`: this process's own pid, not the `self` symlink, opened with `RESOLVE_NO_SYMLINKS`. Exactly one record must have field 1 equal to `stx_mnt_id`. Zero or several matches, or an unparsable line, refuse.
3. That record's field 3 (major:minor) must equal `fstat(root_fd).st_dev` [U]. While the root descriptor is held open, the mount cannot be unmounted normally. A lazily detached mount disappears from `mountinfo`, which refuses at step 2.
4. Field 9 (filesystem type) must be exactly `ext4`.
5. Field 6 (per-mount options) and field 11 (per-superblock options) must equal `mount-options` and `super-options` in `PROVISION`, byte for byte. Changed options refuse until the Owner re-qualifies (§13.3).
6. The pinned option strings must not contain `ro`, `nobarrier`, `barrier=0` or `data=writeback`. This is a provisioning rule, re-checked at every opening.
   - **R4: it is not sufficient.** Field 11 omits any option equal to the superblock's default (`super.c:2949-2951`, `super.c:2985-2993`). The superblock's default mount options and its stored option string can make `nobarrier` or `data=writeback` that default (`super.c:4330-4386`, `super.c:5298-5303`).
   - The absence of a string there proves nothing; step 9 reads the effective state.
7. `fstatvfs(root_fd)` must report `f_bsize` and `f_frsize` of 4096, and `sysconf(_SC_PAGESIZE)` must be 4096.
8. **One filesystem (R3).** `fstat` must show the same `st_dev` for the `PROVISION` directory (reached by the component walk of §10.1), the state root's parent and the root descriptor. Otherwise the opening refuses as Unsupported. Activation checks this again after its syncs (§10.6).
9. **The effective options (R4).**
   - **Resolve the device name.** Resolve the root descriptor's `st_dev` to the device's name: the last component of `readlink("/sys/dev/block/<major>:<minor>")`.
   - **Read the listing.** Read `/proc/fs/ext4/<name>/options` with a bound of 4096 bytes. It lists every option of ext4's table in its effective form, one per line, defaults included (`super.c:3049-3058`), and is world-readable (`sysfs.c:571-583`).
   - **What the listing must hold:**
     - its first line is `rw`;
     - it contains `data=ordered` and `barrier`;
     - it contains none of `nobarrier`, `data=journal`, `data=writeback`, `journal_async_commit`, `norecovery`, `noload`, `fc_debug_force`, `emergency_ro` or `shutdown`;
     - every one of those options that field 11 shows is also in it. Otherwise the two describe different superblocks, and the opening refuses as contradictory.
   - **What refuses as Unsupported:** an unresolvable name, an unreadable or oversized listing, or any mismatch.
10. **The journal's location (R4).** `/proc/fs/jbd2/<name>-8` must exist. jbd2 names an internal journal "<device>-<inode>" and an external one after its own device (`journal.c:1691-1693`, `journal.c:1651-1653`), so its absence means an external journal or none, and refuses. Inode 8 is the qualified journal inode (§13.2).
11. **The kernel (R4).** `uname(2)`'s `release` and `version`, joined by one space, must equal `kernel` in `PROVISION` (§7.5).
    - Another kernel refuses until the Owner qualifies it and re-qualifies the store (§13.3).
    - A match only binds the running kernel to the build the Owner qualified. The qualification is the Owner's review of that build's sources against §5.1 (G-HOST), never the string.

Steps 9 to 11 run at every opening of an owner, the verifier or a session, and again at activation's A4 [M: C27]. Re-qualification mode (§13.3) skips step 11's comparison, because the pinned identity is being replaced. It never skips the rules of steps 9 and 10.

XFS and every other filesystem are outside the initial profile.

### 6.5 Allocation facts, kept distinct (R8)

| Notion | Meaning here |
|---|---|
| Reserved logical record count | The core's all-or-nothing reservations against `record_capacity` (`core.rs:980-989`) [S]. A count, not bytes. |
| Allocated physical range | Each pool file's (C_pool + 2) × 4096 bytes, allocated at provisioning with `fallocate` mode 0 [U] |
| Written versus unwritten extents | `fallocate` leaves extents unwritten [U]. Provisioning zero-fills and syncs, and qualification must see no `UNWRITTEN` extent. Runtime record writes are then pure overwrites that need no allocator call (`sync_file_range(2)` warning) [U]. |
| Copy-on-write and shared extents | Qualification must see no extent marked `SHARED`, `ENCODED`, `DELALLOC`, `UNKNOWN`, `DATA_INLINE` or `NOT_ALIGNED` [U] |
| Quotas | Charged at provisioning. Runtime overwrites need no new blocks. |
| Metadata and I/O failure | Possible regardless. At runtime EIO, or any uncertainty, poisons the journal (§9.5). |
| Flush honesty and overwrite containment | A-S1 and A-S2: assumptions (§5) |

**ENOSPC, EDQUOT, EIO, EROFS and any uncertainty are never success**, at provisioning or at runtime.

### 6.6 Capacity and resource bounds (recomputed)

| Bound | Value |
|---|---|
| Block | 4096 bytes |
| C_pool, record blocks per pool file | Target 512; allowed 1 to 4096 |
| `record_capacity` of a claiming configuration | 1 to C_pool |
| `incident_limit` of a claiming configuration | At most 32, since the header lists at most 32 bindings (`Config::LIVE` uses 16, `model.rs:763-775`) [S] |
| N, pool files | Target 256; allowed 1 to 1024 |
| Pool file | (C_pool + 2) × 4096: 2,105,344 bytes at C_pool = 512, at most 16,785,408 bytes |
| Pool total | N × file: 538,968,064 bytes at the targets, at most 17,188,257,792 bytes |
| Directory entries (R2) | `archive/`: at most 4096. `dispositions/`: at most 4097, the disposition files plus `revoked`. `dispositions/revoked/`: at most 4096. Each count is taken from the names alone, **before any entry is type-checked, opened or read**; more refuses as Invalid ("enumeration bound"). A per-file size limit is not an aggregate limit, so both exist. |
| Startup read volume | Every pool file in full, plus blocks 0 and 1 of each archive entry, plus at most 4097 bytes of each disposition: 538,968,064 + 33,554,432 + 16,781,312 bytes at the targets and limits. Revoked entries are type-checked, never read. |
| Scan memory | One 4096-byte block buffer, a SHA-256 state, and per-journal grammar state proportional to `capacity`. One bounded summary per file, and one parsed record per disposition (at most 4096). |
| Incidents held while scanning | At most N pool-file and journal incidents, one history binding per `u`, `m` or `p-` archive entry, and the enumerated claim gaps, which §7.7 bounds before enumerating. Authorization (§10.2 step 8, §13.9) always uses this complete set. |
| Activation (R3) | Seven directory syncs, one sync of `PROVISION` and one probe per owner startup or session beginning, each sync retried at most 3 times on EINTR (§10.6) |
| Profile reads (R4) | Per opening and again at A4: one `readlink` of the device's sysfs entry, one option listing of at most 4096 bytes, one jbd2 entry looked up, one `uname` (§6.4 steps 9–11) |
| Report detail (R2) | At most 64 incidents listed in full, in a fixed order (§11.2), with exact totals of all incidents and of current ones and an explicit **partial** flag when more exist (§11.2). The detail limit never truncates the set used for any decision. |
| Exchange memory | `capacity` slots, each holding one fixed-size `RecordIntent` (`model.rs:457`) [S]. Allocated once at the claim and never grown. |
| Header | 124 + 33 × n bytes, with n ≤ 32: at most 1180 |
| `PROVISION` | At most 65,536 bytes |
| Disposition file | At most 4096 bytes |
| Prior incidents passed to the core | At most `incident_limit` (`core.rs:1298-1304`) [S]; more refuses (§13.10) |

---

## 7. Container and text grammars (R7)

### 7.1 Conventions

- Integers are unsigned, big-endian and fixed width, as in the codec (`codec.rs:33-38`) [S]. "Zero" means every byte is `00`.
- **Full consumption.** A structure is accepted only if every assigned byte is valid and every unassigned byte is zero. No byte goes unchecked [M: C10].

### 7.2 Header (block 0)

| Offset | Size | Field | Rule |
|---|---|---|---|
| 0 | 4 | magic | `NXCJ` |
| 4 | 1 | container version | `01` |
| 5 | 1 | frame domain | `52` |
| 6 | 1 | frame version | `01` |
| 7 | 1 | log2 of the block size | `0c` |
| 8 | 16 | root id | Equals `PROVISION` |
| 24 | 16 | generation | Not all zero |
| 40 | 8 | claim | 1 ≤ claim ≤ 2^63 |
| 48 | 4 | pool index | Equals the file's index in a pool file; below 1024 in an archive entry |
| 52 | 4 | C_pool | 1 to 4096. Equals `PROVISION` in a pool file. |
| 56 | 4 | capacity | 1 to C_pool |
| 60 | 4 | reserved | Zero |
| 64 | 8 | created at (Unix ms; data only) | Any |
| 72 | 16 | boot id (data only) | Any |
| 88 | 1 | n, the applied dispositions | 0 to 32 |
| 89 | 3 | reserved | Zero |
| 92 | 33 × n | n × (binding: 32 bytes; reason tag: 1 byte) | Tags: `01` OwnerDestroyed, `02` HostRebooted, `03` RecordsMalformed, `04` CompletionNotRecorded, `05` Other (`model.rs:705-711`) [S]. Bindings **strictly increasing**, so no duplicates and one canonical order. |
| 92 + 33n | 32 | header digest | SHA-256 of bytes `0 .. 92 + 33n` |
| 124 + 33n | rest | — | Zero up to 4096 |

A header is **checksum-valid** when all three hold:
- bytes 0–3 are `NXCJ`;
- n ≤ 32;
- the digest equals SHA-256 of bytes `0 .. 92 + 33n`.

It is **valid** when it is checksum-valid and every other rule holds. A header that is checksum-valid but not valid is **Malformed**, never an abandoned claim (§11.1) [M: C10].

### 7.3 Closure seal (block 1)

| Offset | Size | Field | Rule |
|---|---|---|---|
| 0 | 4 | magic | `NXCS` |
| 4 | 1 | version | `01` |
| 5 | 3 | reserved | Zero |
| 8 | 16 | generation | Equals the header's |
| 24 | 8 | claim | Equals the header's |
| 32 | 32 | header digest | Equals the header's digest field |
| 64 | 8 | records | Equals the length of the valid record prefix, and at most capacity |
| 72 | 32 | last record digest | Zero if records is 0, otherwise the digest of record `records` |
| 104 | 8 | terminal sequence | 0 if the prefix holds no `RunEnded`, otherwise that record's sequence |
| 112 | 1 | verdict | `00` if the terminal sequence is 0; otherwise the `RunEnded` verdict, `02` Passed or `03` Failed |
| 113 | 1 | `resolved_by` present | `00` or `01`. `00` if the terminal sequence is 0, otherwise as in the `RunEnded`. |
| 114 | 4 | `resolved_by` | 0 if absent |
| 118 | 8 | closed at (Unix ms; data only) | Any |
| 126 | 2 | reserved | Zero |
| 128 | 32 | seal digest | SHA-256 of bytes `0 .. 128` |
| 160 | 3936 | — | Zero |

The seal is also consistent with what `close()` requires (`core.rs:2881-2924`) [S]. A seal violating any of the following is Malformed:
- the prefix contains a `RunEnded` if it contains a `RunStarted`;
- no recorded-unsettled entry remains.

A seal is **checksum-valid** when bytes 0–3 are `NXCS` and its digest matches. The seal block is classified as follows:

| Seal block | Classification |
|---|---|
| All zero | Unsealed |
| Not checksum-valid | **Seal unreadable**. In domain H this is a torn seal write. Treated as unsealed, the more conservative state, and reported as such. |
| Checksum-valid but breaking any rule above | **Malformed** |
| Valid | Sealed |

### 7.4 Record block (block 1 + *n*)

| Bytes | Rule |
|---|---|
| `0 .. 8 + L + 32` | A version-1 record frame, unchanged. `L`, from bytes 6–7, must be 33 to 83, the version-1 record payload range (`codec.rs:75`) [S]. `codec::decode_record` must accept exactly these bytes (`codec.rs:238-324`) [S]. |
| The rest of the block | Zero |

For record *n* the store also checks:
- the frame's generation equals the header's;
- the frame's sequence equals *n*;
- the frame's instant is not earlier than record *n − 1*'s.

The codec checks syntax, never lifecycle (`codec.rs:75-79`) [S]. The lifecycle checks belong to the grammar (§11.4).

### 7.5 Text grammars

These rules cover `PROVISION`, dispositions and directory entry names:

- **Encoding.** ASCII, LF line endings, no CR, NUL or byte-order mark, and a final newline.
- **Fields.** Exact keys in exact order, each exactly once. No blank lines, comments or trailing data.
- **Canonical values:**

  | Value | Form |
  |---|---|
  | Decimal | `0` or `[1-9][0-9]*`, at most 20 digits, within the field's range |
  | Hex | Lowercase, of the exact length |
  | Timestamp | `YYYY-MM-DDTHH:MM:SSZ`, a valid Gregorian date and time from 1970 to 9999 |
  | Path | `/`-separated components of `[A-Za-z0-9._-]`, 1 to 64 bytes each, neither `.` nor `..`; at most 255 bytes in all |
  | Option string | Printable ASCII without spaces, at most 1024 bytes |
  | Statement | Printable ASCII `0x20`–`0x7e`, 1 to 512 bytes, no leading or trailing space |
  | Operator | The same as a statement, 1 to 64 bytes |

`PROVISION`:

```
nexus-phase2-custody-provision 1
uid=<dec>
gid=<dec>
root-id=<32 hex>
state-root=<path>
root-inode=<dec>
lock-inode=<dec>
journals-inode=<dec>
dispositions-inode=<dec>
revoked-inode=<dec>
archive-inode=<dec>
mount-fstype=ext4
mount-options=<option string>
super-options=<option string>
fs-block-size=4096
page-size=4096
device-logical-block-size=<dec, divides 4096>
device-physical-block-size=<dec, divides 4096>
kernel=<statement: uname(2) release, one space, version, as qualified (R4)>
storage-attestation=<statement>
pool=<N, 1..1024>
pool-capacity=<C_pool, 1..4096>
pool-00000-inode=<dec>
...                       (exactly N lines, in index order, five-digit index)
retired-through=<dec>
predecessor=<none | 32 hex>
predecessor-statement=<none | statement>
operator=<operator>
created=<timestamp>
revision=<dec, 1 or more>
digest=<64 hex: SHA-256 of every preceding byte>
```

- `predecessor-statement` is `none` if and only if `predecessor` is `none`.
- The digest is integrity only. Authority comes from root ownership at a root-controlled path.
- `revision` (R2) is 1 at provisioning and one more than the replaced `PROVISION`'s at every rewrite and at succession. It is the maintenance epoch: every publication has different bytes, and reports name the revision they read (§10.5).
- `kernel` (R4) is the identity of the kernel build the Owner qualified (§13.2). Every opening compares it with the running kernel (§6.4 step 11), and a re-qualification after a kernel update rewrites it (§13.3).

A disposition is stored in `dispositions/<binding>.disposition`:

```
nexus-phase2-custody-disposition 1
root=<32 hex>
binding=<64 hex>
kind=<journal | claim-gap | pool-file>
claim=<dec | none>
generation=<32 hex | none>
pool-index=<dec | none>
content=<64 hex | none>
class=<unresolved | malformed>
recorded-unsettled=<dec>
reason=<owner-destroyed | host-rebooted | records-malformed | completion-not-recorded | other>
statement=<statement>
operator=<operator>
at=<timestamp>
```

Field presence depends on `kind`:

| kind | claim | generation | pool-index | content | class | recorded-unsettled |
|---|---|---|---|---|---|---|
| `journal` | dec | hex | `none` | hex | `unresolved` or `malformed` | the store's count |
| `claim-gap` | dec | `none` | `none` | `none` | `malformed` | `0` |
| `pool-file` | `none` | `none` | dec | hex | `malformed` | `0` |

The binding recomputed from the fields (§12.3) must equal both the `binding` field and the file name.

Entry names:

| Directory | Accepted names |
|---|---|
| `journals/` | `j<5-digit index>.journal`, for the indices below N |
| `dispositions/` | `<64 hex>.disposition`, plus the directory `revoked` |
| `dispositions/revoked/` | `<64 hex>-<YYYYMMDDTHHMMSSZ>.disposition` |
| `archive/` | `j-<claim>-<32 hex>-<64 hex>-<class letter>.journal` (claim as canonical decimal; class `s` sealed, `n` no native work, `u` unresolved, `m` malformed), and `p-<5-digit index>-<64 hex>.journal` |
| `<STATE_ROOT>/` | `LOCK`, `journals`, `dispositions`, `archive` |

Any other name in a store directory refuses. That includes a leftover `.tmp-` entry from an interrupted procedure, which refuses as **MaintenanceIncomplete** (§13.1).

In the directory of `<PROVISION_PATH>`, which may hold other files, these names belong to the store: `<PROVISION_PATH>` itself; each kept predecessor `<PROVISION_PATH>.predecessor-<old root id>` (§13.6); and the temporaries `<PROVISION_PATH>.tmp` and `<PROVISION_PATH>.predecessor.tmp`. An opening reads only `<PROVISION_PATH>`. A leftover temporary is never read, but the next rewrite's exclusive create fails with EEXIST until the Owner removes it in a session (§13.1).

### 7.6 Archive entries

| Form | Content | Validated at startup (§11.3) |
|---|---|---|
| `j-…` | A byte-identical copy of a pool journal with a valid header | Header block valid and matching the name's claim and generation. Class `s`: seal block checksum-valid and matching the header. Class `u` or `m`: a valid disposition for the binding (§12.3). |
| `p-…` | A byte-identical copy of a pool file without a valid header (a `pool-file` incident) | A valid disposition for the binding of kind `03` with that index and content |

### 7.7 Arithmetic and traversal bounds

Before any allocation, seek or read, the reader checks:
- N ≤ 1024 and C_pool ≤ 4096;
- a pool file's size equals (C_pool + 2) × 4096, with checked multiplication in `u64`, before any block is read; an archive `j-` file's size equals (its header's C_pool + 2) × 4096, with that C_pool ≤ 4096; an archive `p-` file's size equals the pool file size for the current `PROVISION`;
- each block offset (*k* × 4096) is below the file size;
- the header's n ≤ 32 before 33 × n is used;
- L is between 33 and 83 before 8 + L + 32 is used;
- the size of `PROVISION` and of each disposition is within its bound, read with a limit of bound + 1 so that oversize is detected;
- the archive holds at most 4096 entries, `dispositions/` at most 4097 and `dispositions/revoked/` at most 4096, counted from the names before any entry is examined (§6.6) [M: C24];
- claim gaps are counted arithmetically, as (highest claim − `retired-through`) minus the claims present. They are enumerated only when that count is at most `incident_limit` plus the number of bindings listed in valid headers, which is the most that can be history. A larger count refuses as **Capacity** without enumerating;
- the current incident count is at most `incident_limit` before `Custody::new`, and `incident_limit` is at most 32.

Claims:
- **Claim exhaustion.** A new claim is max + 1 only if max + 1 ≤ 2^63; otherwise the claim refuses as **ClaimExhausted**. The counter never wraps [M: C17].
- **Generation.** 16 bytes from `getrandom`. It must not be all zero, and it is redrawn if it equals any generation in a scanned header. After 8 draws, refuse.

---

## 8. Exclusion locks, descriptors and lifetimes (R3)

### 8.1 Object and lifetime table (custody owner process)

| Object | Kind | Retained by | Released | Notes |
|---|---|---|---|---|
| Store lock description on `LOCK` | Long-lived advisory exclusion: `LOCK_EX`, taken non-blocking | `StoreGuard`, in the owner's main frame beside `Custody` | Only at process exit. No `LOCK_UN` call exists. | Opened `O_RDONLY \| O_NONBLOCK \| O_NOCTTY \| O_NOFOLLOW \| O_CLOEXEC` |
| Journal lock description on the claimed pool file | Long-lived advisory exclusion: `LOCK_EX`, non-blocking | `StoreGuard` | Only at process exit | A **separate** open, never shared with the recorder |
| Journal I/O description (`O_RDWR`) | Data descriptor; holds **no** lock | Recorder worker | When the worker exits or unwinds | The worker never calls `flock`. A `LOCK_UN` on this description, through a bug, releases nothing, because no lock is attached to it. |
| Root and journals directory descriptors (`O_PATH \| O_DIRECTORY`) | — | `StoreGuard` | Process exit | Used for `*at` calls and the identity recheck after each record |
| Scan descriptors, one per pool file | Short advisory lock: `LOCK_SH`, non-blocking | Startup scanner | Closed once that file is classified (§10.2) | — |
| Activation descriptors (R3): one `O_RDONLY \| O_DIRECTORY` per synced directory, one `O_RDONLY` on `PROVISION` | No lock | The activation (§10.6) | Closed as soon as each is synced | The store-lock description, already held, carries the probe |
| Archive directory and entries, `PROVISION`, dispositions directories and files, `mountinfo`; (R4) the option listing, the jbd2 entry and the sysfs link of §6.4 steps 9–10 | Read-only | Scanner, disposition reader, mount and profile check | Closed when startup ends | — |
| Exchange (§9.2) | **Short mutex**, held for a copy or an assignment, and by the admission gate across one `Custody::admit` call (§9.8) | `Arc`, shared by owner and worker | Process exit | Never held across I/O, a callback or a wait |
| Recorder status (§14.1) | Short mutex | `Arc` | Process exit | Copy only |
| Fatal latch (R2) | Inside the exchange: the first cause and P, later causes counted; set once, never cleared | `Arc` | — | Set by the worker, its drop guard, the sink, the owner's apply step, or the first acquisition that finds the mutex poisoned (§9.5) |
| Worker join handle | — | Owner | Process exit | `is_finished()` without a latched normal exit latches `WorkerVanished` (§9.2) |
| `Custody` and native owners | — | Owner's main frame | `close()` returning `Ok`, or destruction of the process | Never dropped while it holds anything (`mod.rs:139-143`) [S] |

### 8.2 Rules

1. **Separate descriptions.** Lock-bearing descriptions live only in `StoreGuard` and are never `dup`ed into worker code. A duplicate shares the description, and `LOCK_UN` on any duplicate releases the lock (`flock(2)`) [U] [M: C05].
2. **Release only at process exit.** `StoreGuard` releases its locks only when the process exits. Closing a lock-bearing descriptor earlier is a defect.
3. **Close-on-exec.** Every descriptor is `O_CLOEXEC`. A child between `fork` and `execve` shares the descriptions, which only keeps the locks held longer. The descriptors close at `execve`.
4. **No storage under a mutex.** No status, control or exchange mutex is held across `pwrite`, `fdatasync`, `fstat`, an open, a read, a callback or a wait. The core's own control-side locks follow the same rule (`core.rs:431-442`) [S].
   - **The one call under the exchange mutex** is `Custody::admit`, in the admission gate (§9.8). It performs no I/O, callback or wait (§2.2).
   - **Lock order:** the exchange mutex, then the core's internal locks. No thread requests the exchange mutex while it holds a core lock. The worker takes only the exchange mutex; the owner thread calls the core only outside the exchange mutex, except through the gate; the control thread copies the core's state and the recorder status in separate steps.
5. **Advisory scope.** The locks exclude cooperating owners and maintenance (§13.1), not domain-A processes.
6. **Poisoning (R2).** A panic while the exchange mutex is held poisons it. Unlike the core's `lock()` helper (`core.rs:367-371`), the store never treats a poisoned exchange as usable: every acquisition checks, and the first that finds it poisoned latches the fatal cause `Poisoned` (§9.5). The state read afterwards serves only to report.

### 8.3 Behaviour by case [M: C05]

| Case | Locks | Custody | Notes |
|---|---|---|---|
| Normal closure | Held until exit | `close()` returns `Ok`; the seal is written; the process exits | Released by exit |
| Recorder panic or worker disconnect | Held by `StoreGuard` | Kept. Loss is latched by the drop guard, or by the owner's join-handle check. Admission is refused at once (§9.8). The failure is reported at the core's next unacknowledged record once one is issued (§9.5); from then on closure is refused for good (`core.rs:3012-3015`) [S]. With every issued record acknowledged and nothing held, `close()` may still return `Ok`, but the seal is withheld (§9.9). | The process stays until administrative destruction or exit, and the store is Busy for everyone else meanwhile |
| Failed startup or refused activation (R3), before the claim | Held during activation and the scan | NotStarted and untouched; it closes | Released by exit. No claim exists, so a later crash can omit nothing [M: C25]. |
| Failed claim, including recorder loss during the claim | Held | NotStarted and untouched (no record was issued, §10.2). The latch turns `Requested` into `Failed` [M: C18]; it closes. | The file stays an abandoned claim or a header-only journal (§11.1). Released by exit. |
| Failed append | Held | Kept. The failure is latched; admission refused at once; reported at the next unacknowledged record (§9.5) | As for recorder panic |
| Failed seal | Held until exit | Already closed; nothing native | The seal may stay kernel-visible, and is then still true: `close()` returned `Ok`. Once page reclaim, inode eviction or power loss removes it, the generation is unsealed, and a startup refuses it if it recorded an action start [M: C05]. |
| I/O blocked in the kernel | Held | Kept; the owner stays responsive (§14.1) | A process cannot finish exiting while a thread is in uninterruptible I/O, so the locks stay held until that I/O ends |

### 8.4 Maintenance session objects (R2)

A maintenance session (§13.1) is one root process. It is a specified future interface; no executable exists or is authorized here.

| Object | Kind | Retained by | Released | Notes |
|---|---|---|---|---|
| Store lock description, per state root the session holds | `LOCK_EX`, non-blocking, opened safely (§10.1) | The session object | Only at the session's end. Never converted to `LOCK_SH`, never unlocked early, never duplicated into another process. | Its existence, on the inode the `LOCK` entry currently names, is the session's authority (INV-15) |
| Journal lock descriptions | `LOCK_EX`, non-blocking, on each pool file the procedure reads or replaces | The session object | At the session's end | In-session verification reads those files through them |
| Selection (§10.5) | The revalidated `PROVISION` selection | The session object | Replaced only by re-selection under the session's own lock, after the session itself published `PROVISION` | — |
| Verification scan descriptors | `LOCK_EX`, non-blocking, on pool files the session does not already hold | The in-session scan | Closed once that file is classified | Busy means a domain-A process holds it: Live |

---

## 9. Recorder protocol (R5, D-5; R2: total fatal-failure and admission contract)

### 9.1 Roles

- **The owner thread** owns `Custody`. It calls `flush_records` (`core.rs:2958-2991`) with the exchange-backed sink, applies durability to the core at safe points, and admits actions only through the store admission gate (§9.8).
- **The worker** owns only the journal I/O description and its own position.

### 9.2 The exchange

```
Exchange {                                   // one Mutex: copies, assignments, and the admission gate
  generation, capacity,                      // fixed at the claim
  slots: [Option<RecordIntent>; capacity],   // index = sequence − 1, allocated once
  submitted_through: u64,                    // slots 1 ..= submitted_through are filled, contiguously
  claim: ClaimState,                         // None | Requested(header) | Claimed | Failed
  seal:  SealState,                          // None | Requested(seal) | Sealed | Failed
  durable_through: u64,                      // set only by W6, and only while nothing is latched
  fatal: Option<(Cause, u64)>,               // the first fatal cause, and P = durable_through then
  later_causes: u64,                         // causes after the first, counted only
  pending: Option<(u64, Tick)>,              // the record in flight, for status only
}
```

The exchange holds **no message queue**, so nothing in it can fill up, overflow or be dropped.

**The fatal latch.** R1 kept three latches (`failed`, `lost`, `conflict`) and delivered each only at one exact position. R2 has one latch for every fatal cause:

| Cause | Latched by | When |
|---|---|---|
| `ClaimWrite`, `ClaimSync` | Worker | The claim's write, sync or identity recheck failed (§9.7) |
| `Encode(n)`, `Write(n)`, `Sync(n, error)`, `Identity(n)` | Worker | W2, W3, W4 or W5 failed for record *n* (§9.4) |
| `SealWrite`, `SealSync` | Worker | The seal's write or sync failed (§9.7) |
| `WorkerLost` | The worker's drop guard | The worker unwinds |
| `WorkerVanished` | Owner | The join handle has finished without a latched normal exit |
| `InvalidSubmission(n, why)` | Sink | Before `Claimed`; another generation; sequence 0 or beyond capacity; a future sequence (§9.3) |
| `Conflict(n)` | Sink | A submission for an already submitted sequence with a different digest, acknowledged or not |
| `UnexpectedAck(n, outcome)` | Owner's apply step | `acknowledge` returned anything but `Acknowledged` (§9.4) |
| `Poisoned` | The first acquisition that finds the mutex poisoned | A panic while the mutex was held (§8.2 rule 6) |

Latching is one critical section. If nothing is latched, it records the cause and **P = `durable_through`** at that instant, and a `Requested` claim or seal becomes `Failed`. Otherwise it only counts the cause. The latch is never cleared.

### 9.3 Submission: `RecordSink::submit`

Submission is non-blocking apart from the short critical section, idempotent and contiguous. The conditions are checked in this order:

| Condition | Effect |
|---|---|
| A cause is latched, or the mutex is poisoned (then `Poisoned` is latched first) | No effect. The record stays issued and never becomes durable; §9.5 reports the failure. |
| Claim not `Claimed` | Latch `InvalidSubmission` (before the claim) |
| Another generation | Latch `InvalidSubmission` (foreign) |
| Sequence 0, or beyond capacity | Latch `InvalidSubmission` (out of range) |
| Sequence ≤ `submitted_through`, same digest | **No effect.** This is a duplicate, such as a re-submission after a sink panic (`core.rs:2974-2985`) [S]. |
| Sequence ≤ `submitted_through`, different digest | Latch `Conflict(n)`. This includes a conflict on an acknowledged record, whose acknowledgement stays true. |
| Sequence = `submitted_through` + 1 | Store the intent; `submitted_through` advances |
| Sequence > `submitted_through` + 1 | Latch `InvalidSubmission` (future). The core submits in order (`core.rs:2958-2991`) [S], so a gap is an integration defect. It is never stored silently. |

After storing, the sink wakes the worker with a condition-variable hint. A missed hint is harmless, because the worker also rechecks the slots on each loop. Two cases need no special handling:
- **A panic after the store, before `submit` returns.** The core keeps the record unsent and records a failure. Its next flush resubmits the same intent, which is a no-op.
- **Repeated submissions.** They change nothing and cost one comparison each [M: C07].

### 9.4 Delivery: state, not messages

The worker's loop works on `next = durable_through + 1`:

| Step | Action |
|---|---|
| W1 | Under the mutex: if nothing is latched and `slots[next]` is present, copy it and set `pending` |
| W2 | `codec::encode_record` must succeed (it refuses a digest mismatch, `codec.rs:230-236`) [S] and give 73 to 123 bytes |
| W3 | `pwrite` the 4096-byte block (frame, then zeros) at offset 4096 × (next + 1). Loop on short counts. A zero-progress result or EINTR is retried at most 3 times, then poisons. |
| W4 | `fdatasync` on the I/O description. EINTR is retried at most 3 times; any other result is final. |
| W5 | Identity recheck: `fstat` of the I/O description shows `st_nlink == 1` and an unchanged size, and `fstatat(journals_fd, name, AT_SYMLINK_NOFOLLOW)` shows the same inode |
| W6 | Under the mutex: **only if nothing is latched and the mutex is not poisoned**, set `durable_through = next`, clear `pending` and wake the owner. Otherwise publish nothing and stop: the block may be durable, but it is never acknowledged. |

A failure in W2 to W5 latches its cause and stops all writing. The W6 check and the assignment share one critical section with every latch, so each publication either precedes the latch, and P includes it, or never happens. A latch that arrives while the worker is anywhere between W1 and W6 therefore stops that record's publication [M: C18, C19].

The owner's apply step runs at safe points and never blocks:
1. If the join handle has finished without a latched normal exit, latch `WorkerVanished`.
2. Under the mutex, copy `durable_through` and `fatal`. After a latch the copied position is at most P, because no publication follows a latch. The copied `durable_through` is the delivered position.
3. For each sequence from `applied_through + 1` up to the delivered position, call `acknowledge(RecordAck { id, digest })`, built from the owner's own retained intent. On `Acknowledged`, `applied_through` advances. Any other outcome (`NotIssued`, `Duplicate`, `Conflict`, `LedgerFailed`, `OutOfOrder`, `Foreign`, or `ClockRegression`) latches `UnexpectedAck(n, outcome)` and stops application for good. It is never retried and never counted as delivered.
4. If a cause is latched, run failure delivery (§9.5).

The owner applies after every `flush_records` call, at every safe point, and on each wake-up hint.

The protocol keeps five positions apart:

| Position | Holder | Meaning |
|---|---|---|
| Last durably written | Worker: `durable_through` | Set at W6; never beyond P |
| Last delivered | Owner: its latest copy of `durable_through` | A read of state; it cannot be lost or duplicated |
| Last applied by the core | Owner: `applied_through`, equal to `EvidenceView.acknowledged` (`model.rs:837-845`) [S] | At most P after a latch |
| P | The latch | `durable_through` when the first cause latched. No sequence beyond P is ever published or acknowledged (INV-4). |
| First not established durable | The core's next unacknowledged record, `applied_through + 1` | Where the failure is delivered (§9.5) |

**Duplicates.** A duplicate submission is a no-op. Each sequence is acknowledged to the core exactly once, from state. Re-acknowledgement does not exist. An `acknowledge` result of `Duplicate` would mean an owner defect, and latches `UnexpectedAck` [M: C07, C18].

### 9.5 Failure delivery

Failure delivery runs at the end of every apply step while a cause is latched and no failure has been reported:
1. If the core's ledger has already failed, nothing more is reported.
2. Let the **target** be the core's next unacknowledged record: `EvidenceView.acknowledged + 1`. If the core has not issued it yet, report nothing now. No record exists that could truthfully be failed, and `Ledger::fail` would return `NotIssued` (`core.rs:1072-1090`) [S]. Delivery runs again at the next apply step.
3. Otherwise call `record_failed(id_target)` once and expect `FailureRecorded`. The core fail-stops: it closes admission with `ClosureReason::Failed(RecordFailed)`, accepts no later acknowledgement, issues no terminal record after it and never closes (`core.rs:3012-3031`, `core.rs:3046-3069`) [S]. Any other outcome is recorded as an integration fault, never retried, and never counted as delivery.

What this keeps, whatever the cause:
- **True acknowledgements stay.** Everything published before the latch is applied, including after a conflict on an acknowledged record. Nothing is retracted.
- **No record is invented.** The target is a record the core really issued. `NotIssued`, `Conflict`, `Foreign` and `OutOfOrder` never count as success.
- **The failure lands at the first record not established durable.** For a conflict at record 6 with P = 3, the target is record 4: records 4 to 6 were never published.

**Until the target exists** the core's own state does not show the failure. Admission is nevertheless refused at once, by the store gate (§9.8), and the owner's status shows the latched cause (§14.1). API-5 would close that gap (§2.3).

A failed sync is never retried, and nothing is acknowledged after one. errseq reports an error once per description, so a retry can return 0 for data that was lost ([U]; corroborated by PostgreSQL `data_sync_retry`) [M: C15]. The record may or may not be durable; startup treats whatever is visible as evidence (§4.4).

**No undelivered signal is ever treated as delivered, and no failure is treated as durable** [M: C08, C18].

### 9.6 Stall semantics [D-5]

| Concern | Behaviour |
|---|---|
| Admission while a record is pending | Refused by the core. A start record must be acknowledged, and a new case or the terminal commitment needs everything acknowledged (`core.rs:1658-1675`, `core.rs:1542-1544`, `core.rs:2663-2669`) [S]. |
| Failure on stall | **None.** A stall is neither a failure nor latched as one. Status shows "record *n* pending since *t*" (§14.1). |
| A stall versus a fatal cause (R2) | A stall has a pending record, nothing latched, and may resume. A fatal cause is latched, refuses admission at once (§9.8) and never resumes. No fatal cause is reported or treated as a stall, and no stall as a failure. |
| Late completion: the stalled write and sync succeed | `durable_through` advances, if nothing has latched meanwhile, and the acknowledgement is applied late; progress resumes |
| Late completion: they fail | Latched; then §9.5 |
| Recorder loss | Latched; then §9.5 |
| Timeouts | Never become an acknowledgement or a failure. Timing out a waiting thread does not cancel a kernel operation. |
| Operator action | Administrative destruction of the process only. That is authority loss, and refusal follows (§11.2). |

### 9.7 Claim and seal

**Claim.** The owner requests it only after activation (§10.6), and the worker performs it:
1. Write the header block (§7.2).
2. `fdatasync`.
3. Recheck identity (W5).
4. Under the mutex: publish `Claimed` if nothing is latched. A failure latches `ClaimWrite` or `ClaimSync`, and a latch for any other cause, including recorder loss during the claim, turns `Requested` into `Failed` [M: C18].

`start_run` is called only after `Claimed`.

**Seal.** Requested only after `close()` returns `Ok`, and only while nothing is latched:
1. Write the seal block (§7.3), with records equal to `durable_through`.
2. `fdatasync`.
3. Under the mutex: publish `Sealed` if nothing is latched, otherwise `Failed`.

A cause latched before the request **withholds the seal**. Nothing native is held by then, so the owner exits either way, and a restart sees an unsealed generation (§9.9).

### 9.8 The store admission gate (R2)

The integration calls `Custody::admit` only through the gate:

```
fn admit(gate, custody, reservation, now) -> Result<OpTicket, GateRefused> {
    let exchange = gate.exchange.lock();       // poisoned: latch Poisoned, then refuse
    if exchange.fatal.is_some() {
        return Err(RecorderFatal);             // at once, without waiting for any record
    }
    custody.admit(reservation, now)            // still under the same guard
}
```

- **Linearization.** The health check and the admission are one critical section of the exchange mutex, the mutex every latch takes. An admission therefore completes before the latch, or starts after it and is refused. **A check followed by an unprotected admission call is not sufficient**: a latch between the two would admit after a fatal condition [M: C19, NC19].
- **Why holding the mutex is safe.** `Custody::admit` performs no I/O, callback or wait (§2.2), and the lock order of §8.2 rule 4 holds.
- **In flight versus new.** An action admitted before the latch is in flight. It runs to its end, its owner is held, and cleanup proceeds; its later records are issued but never acknowledged. Every admission requested after the latch is refused with `RecorderFatal`.
- **The reservation.** A refused reservation stays `Reserved` in the core. The owner ends the case: `end_case` settles the reservation as `NotAdmitted`, runs cleanup and issues `CaseEnded` (`core.rs:2157-2212`, `core.rs:1700-1704`) [S]. Those issued records give failure delivery its target (§9.5) [M: C18].
- **No native start without a ticket.** The integration starts a native operation only with the `OpTicket` that the gate returned.
- **Owner policy after a latch.** The owner refuses admissions, ends an open case, runs cleanup, keeps every owner held, keeps serving status and late owners, and calls `close()` only when the core permits it. It calls nothing that begins work (`start_run`, `begin_case`). It does not call `finish_run`: a commitment then would be decided before the core learns of the failure (§9.9).
- **Without native ownership.** Publishing the failure never requires dropping the last owner. A failure with nothing held is still latched, refuses admission, withholds the seal, and makes restart refuse the generation if it recorded an action start [M: C18].

### 9.9 Eight concerns kept apart (R2)

| # | Concern | Holder | Rule |
|---|---|---|---|
| 1 | Recorder health | The exchange's fatal latch | Set once, with its first cause and P; never cleared; later causes counted |
| 2 | Admission permission | The store admission gate, then the core's gate | Refused at once from the latch on (§9.8) |
| 3 | Durability publication | W6 | Only while nothing is latched, so never beyond P |
| 4 | Acknowledgement delivery and application | The owner's apply step | Exactly once per sequence, up to P. An unexpected outcome latches and stops. |
| 5 | Evidence failure at a real record | Failure delivery (§9.5) | Only at the core's next unacknowledged record, only once that record is issued, and only once |
| 6 | Committed core outcome | The core (`core.rs:2662-2687`) [S] | Never rewritten. A commitment whose terminal record was published before the latch becomes final, and its verdict is true. Any other commitment is never acknowledged, so the run is never final and the published verdict is never `Passed` (`core.rs:2710-2718`) [S]. |
| 7 | Owner and store outcome; seal eligibility | The owner | The store outcome is `RecorderFatal` from the latch on. A seal is written only after `close()` returned `Ok` with nothing latched; otherwise it is withheld. |
| 8 | Retained native custody and exclusion | `Custody` and `StoreGuard` | No fatal cause changes them: owners stay held, cleanup runs, and the locks stay until process exit (§8) |

### 9.10 The F1 cases

Each was reproduced on the R1 model first (§19.3).

| Case | R1 | R2 [M: C18, C19] |
|---|---|---|
| A: worker loss with three records acknowledged and none issued since | Never delivered, since record 4 did not exist; admission still permitted | `WorkerLost` with P = 3. Admission is refused at once. The failure is delivered at record 4 once ending the case issues it. |
| B: a conflicting duplicate of acknowledged record 1 | Latched and never delivered. R1's INV-4 ("nothing at or after 1 acknowledged") contradicted three applied acknowledgements. | `Conflict(1)` with P = 3. Acknowledgements 1 to 3 stay. The failure lands at record 4. |
| C: sequence zero, out of range, foreign or future; a conflict beyond the next unapplied record | Latched and never delivered; the future sequence was stored silently; admission still permitted | `InvalidSubmission` or `Conflict`. The failure lands at the core's next unacknowledged record. |
| D: loss between a durable reservation and its admission | Admission permitted | The gate refuses `RecorderFatal`. `end_case` settles the reservation `NotAdmitted`, and the failure lands at record 4. |
| E: an append in flight when another cause latches | The record in flight was published and acknowledged after the latch | W6 refuses. P stays, and the record in flight is the one failed. |
| F: loss during the claim or the seal; a failure with no native ownership left | Claim and seal stayed `None`: neither failed nor completed | `Requested` becomes `Failed`. With nothing held, `close()` may return `Ok`, the seal is withheld, and restart refuses an unsealed generation with an action start. |
| An unexpected acknowledgement outcome | An unspecified integration fault | `UnexpectedAck`. Application stops, and the failure lands at that record. |
| A poisoned mutex; the worker unwinding | Not specified | `Poisoned`; `WorkerLost` (or `WorkerVanished` if no drop guard ran) |

---

## 10. Startup, scan and claim handoff (R4)

### 10.1 Safe open

For each entry, before it is opened:

1. **Type check.** `fstatat(dirfd, name, AT_SYMLINK_NOFOLLOW)` must show the expected type (regular file or directory), the expected owner and exact mode, `st_nlink == 1` for regular files, and the inode recorded in `PROVISION` where one is recorded. A special file (FIFO, socket, device) or a symbolic link refuses **without being opened** [M: C14].
2. **Open.** `openat2(dirfd, name, flags, RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS | RESOLVE_NO_XDEV)`:
   - regular files: `O_RDONLY | O_NONBLOCK | O_NOCTTY | O_NOFOLLOW | O_CLOEXEC`;
   - directories: `O_PATH | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC`.

   Even if a FIFO replaced the entry between check and open, `O_NONBLOCK` makes a read-only FIFO open return instead of blocking (`fifo(7)`). The flag has no effect on regular files (`open(2)`) [U].
3. **Identity recheck.** `fstat(fd)` must show the device, inode and type of step 1; otherwise the entry refuses as replaced.

`<PROVISION_PATH>` and `<STATE_ROOT>` are walked one component at a time from `/`:
- each component is opened `O_PATH | O_DIRECTORY` with `RESOLVE_NO_SYMLINKS`;
- its owner and mode are checked: root-owned, neither group- nor other-writable.

### 10.2 Order

| Step | Action | Refuses with |
|---|---|---|
| 0 | Configuration: `record_capacity` ≤ C_pool, `incident_limit` ≤ 32 | Unsupported |
| 1 | Select `PROVISION` (§10.5): open it safely, read it with a limit and parse it (§7.5). The result is only a candidate selection. | Unprovisioned, Invalid |
| 2 | Open the candidate's `<STATE_ROOT>`; identify ext4 through the mount (§6.4); check block size, page size, the link sysctls and the ancestors; (R4) read the effective profile, the journal's location and the kernel (§6.4 steps 9–11) | Lost, Unsupported |
| 3 | Open `LOCK` safely and take `flock(LOCK_EX \| LOCK_NB)`. Then revalidate the selection (§10.5). On a mismatch, close every descriptor of this attempt, which releases the lock, and return to step 1, at most three selections in all. | **Busy**, before any journal content is touched [M: C06]; **SelectionChanged** after three failed revalidations [M: C21] |
| 3a | **Activate** the selection (§10.6, R3; §10.7, R4): one filesystem, every dependency directory synced, the probe, revalidation with the profile checked again | Unsupported, Invalid, Lost, Unreliable, SelectionChanged [M: C25, C27, C28] |
| 4 | Enumerate the store directories, entry names only. Every name must be accepted (§7.5); each entry is type-checked as in §10.1 step 1. | Invalid, MaintenanceIncomplete, Lost |
| 5 | For each pool file, in index order: <ul><li>open it safely;</li><li>take `flock(LOCK_SH \| LOCK_NB)`; failure means **Live**, and the file is neither synced nor read;</li><li>`fdatasync` to preserve evidence; EIO refuses as **Unreliable**;</li><li>read the file and classify it (§11.1);</li><li>close it, which releases the shared lock.</li></ul> | Live, Unreliable |
| 6 | Archive: safely open each entry and read blocks 0 and 1 (§11.3) | Invalid |
| 7 | Dispositions: read and validate every file (§13.4) | Invalid |
| 8 | Pool-level checks (§11.3); then split the incidents into history and current (§13.5). More current incidents than `incident_limit` refuse (§13.10). | Invalid, Capacity |
| 9 | Draw the generation (§7.7). Call `Custody::new(config, generation, current, now)`, then `apply_disposition` for each current incident; the disposition reader holds the `dispositions/` descriptor. If any incident still blocks (`snapshot().prior`, `model.rs:860-865`) [S], close the NotStarted custody and exit. | PriorUnresolved |
| 10 | Claim handoff (§10.3), then `start_run`. No Unused pool file means PoolExhausted. | PoolExhausted, ClaimExhausted, ClaimFailed |

- **No record before the claim.** Until the claim is published, the owner serves no control request and calls no core method that could issue a record. A refused or failed claim therefore leaves an untouched NotStarted custody. Its `close()` is permitted, since nothing was issued (`core.rs:2881-2924`) [S], and the process exits.
- **No writes before step 10.** Startup writes no store byte before then. Activation (step 3a) syncs directories and updates `LOCK`'s timestamps, and writes no byte.
- **Disclosed persistence effect.** The step 5 sync makes the pages still dirty durable, and nothing more (§4.6).
- **Every refusal is reported with the selection's revision** (§10.5), so a report names the `PROVISION` it was made against.

### 10.3 Scan-to-claim handoff

1. Choose the lowest-index pool file classified **Unused**.
2. Open a **new** lock description on it (safe open) and take `LOCK_EX | LOCK_NB`. The scan's shared lock was already released in step 5, and the store lock excludes cooperating owners. If a domain-A process holds a lock anyway, the claim is refused.
3. Open the worker's I/O description with `O_RDWR | O_NONBLOCK | O_NOCTTY | O_NOFOLLOW | O_CLOEXEC` and the same resolve flags. Its `fstat` identity must equal the lock description's.
4. Re-read the whole file through the I/O description; it must still be all zero.
5. Move the I/O description into the worker. The lock description stays in `StoreGuard`.
6. Request the claim (§9.7); activation (§10.6) completed before the scan. The claim number is the highest of `retired-through` and every claim seen, plus 1 (§7.7). The generation is redrawn if it equals the generation in any scanned header, of a pool file or of an archive entry (§7.7). The header lists the bindings of all current incidents, each with the reason from its disposition.

### 10.4 Descriptor classes after startup

| Kept for the life of the process | Closed by the end of startup |
|---|---|
| Root directory, journals directory, store lock description, journal lock description (all in `StoreGuard`) | Scan descriptors; activation descriptors; archive directory and entries; dispositions directories and files; `PROVISION`; `mountinfo`; (R4) the profile files |
| The worker's I/O description (until the worker ends) | |

### 10.5 `PROVISION` selection and revalidation (R2)

**The selection.** A selection is a tuple: the `PROVISION` inode and device; the SHA-256 of the complete bytes read; the `revision` and root id they carry; and the state-root and `LOCK` inodes they record. Reading `PROVISION` yields a **candidate**. A selection becomes **authoritative** only for a process that holds the store lock on that selection's `LOCK` and has then revalidated it. A path, a cached configuration, an old descriptor, a PID, or a revision number on its own confers nothing.

**Revalidation**, after the lock is held [M: C21]:
1. A fresh `fstatat` of `<PROVISION_PATH>`, reached by the component walk of §10.1, shows the selected inode. This is never checked through the descriptor kept from the read: that descriptor still names the original inode after a replacement [NC21b].
2. A fresh safe open and read gives the selected SHA-256.
3. A fresh lookup of `<STATE_ROOT>` shows the selected root inode.
4. The `LOCK` entry in that root names the selected `LOCK` inode, and `fstat` of the locked description shows the same inode.

On any mismatch the process closes every descriptor of the attempt, which releases the lock, and selects again. After three selections it refuses as **SelectionChanged**. It never waits and never retries without bound.

**Why this suffices among cooperating processes.** `PROVISION` names one state root at any instant. A rewrite or a succession happens only inside a maintenance session that holds the selected root's lock (§13), and every publication changes the bytes, because `revision` increases. Hence:
- a reader that selected before a rewrite and locked after it fails step 1 or 2 and re-selects;
- a session cannot begin while an owner holds the lock, so nothing is rewritten under a live owner;
- the session locks a successor's root before publishing its `PROVISION` (§13.6). No owner can start on the successor until the session ends. Once the publication is **durable** (by the session's final directory sync, or by the first owner's activation, §10.6), no process can revalidate the predecessor. While it is only visible, a power loss can restore the predecessor, and no work depends on the successor then, because no owner claims before activating (R3).

Two cooperating owners therefore never both hold an authoritative selection, whether of the same root or of a predecessor and its successor [M: C21].

**Stale readers and open descriptors.** A process that holds an old `PROVISION` descriptor, an old root descriptor or an unlinked pool inode gains nothing, because authority comes only from a revalidated selection. Domain A is outside this argument: the locks are advisory (§8.2 rule 5), gate G-AUTH stays open, and nothing here protects against root or kernel compromise.

**Where it applies.** Ordinary opening (§10.2), the standalone verifier (§13.11), the beginning of every maintenance session (§13.1), and a session's re-selection after its own publication (§13.3). The model exercises ordinary opening, re-qualification, recycling, retirement, successor publication and the bounded retry [M: C21].

### 10.6 Durable activation (R3; what it establishes is restated in R4)

**Why.** A selection that is only visible is not enough to depend on. Suppose a session dies after renaming `PROVISION` B over A but before syncing the directory: P-SUCCESSOR operation 28, or, for a first provisioning, P-PROV operation 22.
- B is visible, and an owner can select, lock and revalidate it (§10.5), claim, and record acknowledged work.
- A power loss before the rename is durable can restore A, or no `PROVISION` at all.
- A's history omits B's acknowledged, unsealed generation, and a fresh owner on A runs as if it never happened. This reproduced on the R2 model in 10 of 14 power-loss outcomes (§19.4).

A valid digest, a current inode, a higher revision and a held lock each prove only visibility.

**Five states of a selection:**

| State | Established by | Grants |
|---|---|---|
| Candidate | Reading `PROVISION` (§10.5) | Nothing |
| Revalidated | The store lock held, then the fresh lookups of §10.5 | Exclusion and identity, and reading the visible store. Not a claim. |
| Activated | Steps A1–A5 below, under the same store lock | A claim, and everything that follows from it |
| Refused or uncertain | Any failure of A1–A5 | Nothing. An owner closes its NotStarted custody and exits; a session ends. |
| Administrative change | A maintenance session, which activates at its beginning and whose own procedure syncs and checks its changes through its own descriptors (§13.1) | Retirement, succession and the other procedures |

**Before what.** Activation completes before the claim is requested (§10.3 step 6). Everything else follows from the claim:
- `start_run` follows `Claimed` (§9.7);
- every record follows `start_run`;
- every acknowledgement follows its record's durability (§9.4);
- every admission follows an acknowledged start record and passes the store gate (§9.8).

No claim, record, acknowledgement or native admission can therefore exist on a selection that is not activated (INV-18).

**Steps**, by the owner process, which has held the store lock continuously since §10.2 step 3:

| Step | Action | Refuses with |
|---|---|---|
| A1 | One filesystem: the `PROVISION` directory, the state root's parent and the root show the same `st_dev` (§6.4 step 8) | Unsupported |
| A2 | For each directory, in this order: `journals/`, `dispositions/revoked/`, `dispositions/`, `archive/`, `<STATE_ROOT>`, the state root's parent, the `PROVISION` directory.<ul><li>Resolve it afresh by the component walk of §10.1.</li><li>Check its inode against `PROVISION` (the store's directories) or the selection (the parent and the `PROVISION` directory).</li><li>Open it `O_RDONLY \| O_DIRECTORY \| O_NOFOLLOW \| O_CLOEXEC` (an `O_PATH` descriptor cannot be synced), `fsync` it, and close it.</li></ul>EINTR is retried at most 3 times; any other error is final. Then `fdatasync` `<PROVISION_PATH>` itself through a new `O_RDONLY` descriptor. | Invalid or Lost (identity); Unreliable (any error) |
| A3 | Probe, after the last A2 sync: `futimens(NULL)` on the store-lock description, then `fsync` of that description. Both must return 0. `LOCK` is the store uid's own file, so no new permission is needed. | Unreliable |
| A4 | Revalidate after the syncs: resolve every A2 directory again and check its inode; repeat §10.5 steps 1–4; repeat the mount and profile checks of §6.4, steps 8 to 11 included | SelectionChanged, Unsupported |
| A5 | **Linearization point**: A4 has succeeded with the store lock still held. The selection is activated, and the startup report records its revision and root id. | — |

**What each step establishes**, on the supported profile (A-M2; R4 states it precisely in §10.7):
- **A2, the directories.** Each directory `fsync` returns only after every transaction holding that directory's earlier operations has completed.
  - A sync that waited tests the abort flag itself.
  - A sync that found nothing to commit returns 0 having tested nothing. It does **not** certify an earlier commit that already failed.
- **A3.** The probe's handle start tests the abort flag after every A2 sync has returned, and turns an abort into emergency read-only, which the probe's `fsync` returns first.
  - A successful probe therefore shows that no transaction holding a dependency failed (§10.7).
  - It does **not** show that the journal is healthy afterwards. The probe's `fsync` waits only for a running or committing transaction (F3), so it can pass after a later abort, which cannot uncommit a dependency (§10.7).
  - R3 claimed more: that the journal "had not aborted by the time of the probe". R4 withdraws that claim.
- **A2, `PROVISION` itself.** Defence in depth. It may report an unseen writeback error, but it never certifies the content: a sync on a new descriptor cannot (§4.3). The procedure that published `PROVISION` certified the content with its own descriptor before the rename (§13.1 rule 5).
- **A4.** It binds the now-durable entries to this selection. A replacement racing the syncs, a directory swapped for another, a remount read-only, or a profile changed during activation refuses. A read-only superblock also makes A2 return 0 having committed nothing, and A3 then fails with EROFS.

**Failures, never success.** No timeout or retry turns a failure into activation:
- EINTR is retried at most 3 times, then refuses;
- EIO, EROFS, ENOSPC and every other error refuse;
- a failed identity check refuses;
- a sync that blocks leaves the owner waiting before its claim, serving no request and holding no custody, until the sync returns or the process is destroyed;
- process death or power loss during activation leaves no claim, so nothing can be omitted [M: C25].

The refusing owner closes its NotStarted custody and exits; a later startup activates again from the beginning.

**Persistence effects of opening.** An owner's startup commits the pending metadata of that filesystem (A2) and updates `LOCK`'s timestamps (A3), besides the evidence-preservation sync (§4.6). It changes no byte of the store. Its profile reads (R4, §6.4 steps 9–11) write nothing.

**Who activates:**
- every owner, before it scans and decides (§10.2 step 3a), so that the decision reads a durable namespace;
- every maintenance session, at its beginning, before verify-before (§13.1).

**Who does not:** the standalone verifier. It reads and reports the visible state, never declares it durable, and authorizes nothing (§13.11).

**Permissions.** Read and search on the root-owned 0755 directories and read on the 0444 `PROVISION`, which the store uid already has; ownership of `LOCK` for the probe, which the uid already owns. R4's profile reads use world-readable kernel files (`sysfs.c:571-583`, `journal.c:1222-1229`), `/sys/dev/block` and `uname(2)`. Nothing is expanded.

**Why the store's directories too.** A recycled pool file's name, a disposition's name, an archive entry or a successor root's entry could otherwise roll back after a decision used it. With activation, every name the decision read is durable before anything is authorized [M: C25, C26].

**Ancestors.** The directories above the `PROVISION` directory and above the state root's parent are created, and made durable, when the host is set up (§13.2 step 1). No procedure changes them, so activation does not sync them.

**G-AUTH is unchanged.** Activation is an honest-crash obligation of A0 in domain H. A process in domain A can still rewrite the store uid's files (§3.3).

**Activation operations** [M: C25; checked against the model by the reference check]:

| Step | Operation | Object |
|---|---|---|
| 10.6/A2 | fsync_dir | `journals` |
| 10.6/A2 | fsync_dir | `dispositions/revoked` |
| 10.6/A2 | fsync_dir | `dispositions` |
| 10.6/A2 | fsync_dir | `archive` |
| 10.6/A2 | fsync_dir | `root` |
| 10.6/A2 | fsync_dir | `parent` |
| 10.6/A2 | fsync_dir | `provdir` |
| 10.6/A2 | fdatasync | `PROVISION` |
| 10.6/A3 | futimens | `LOCK` |
| 10.6/A3 | fsync | `LOCK` |

Object names: `root` is `<STATE_ROOT>`, `parent` its parent, and `provdir` the directory of `<PROVISION_PATH>`.

### 10.7 Activation proof (R4)

**The property.** Before an owner or a maintenance session decides anything that depends on the selected store, the selection's dependencies are established durable under the declared supported model (§5.1, §6.4), or activation refuses. The dependencies are:
- every entry the selection and the decision read in the seven A2 directories: the store's directories, the state root's entry in its parent, and `PROVISION`'s entry;
- the content of `PROVISION`, which its publishing procedure synced through its own descriptor before the rename (§13.1 rule 5).

It is **not** a certificate that every earlier filesystem transaction succeeded, that the filesystem stays healthy, or that no later error occurs. Activation runs one probe and never repeats it to argue for continuing health.

**What activation establishes** (A-M2; Linux v6.17):
1. **Each dependency is in a transaction that completed before its directory's A2 sync returned.**
   - A sync that finds a running or committing transaction waits until every transaction up to it has completed (`journal.c:499-527`, `journal.c:678-686`).
   - A sync that finds neither returns because all have completed (`journal.c:513-517`).
   - Either way, the dependency's operation was issued before the sync, so its transaction is among them.
2. **None of those transactions failed.** A failing commit sets the abort flag before the transaction completes (`commit.c:1102-1105`), and the flag stays (`journal.c:2515-2516`).
   - A sync that waited tests the flag after its wait (`journal.c:689-690`).
   - For a sync that found nothing to commit, the probe's handle start tests the flag (`ext4_jbd2.c:81-89`) after the last A2 sync has returned. It turns an abort into emergency read-only, which the probe's `fsync` returns before anything else (`fsync.c:135-137`), although `futimens` itself returns 0 (`inode.c:6531-6540`). Under `errors=panic` it panics instead (`super.c:717-720`), and nothing is claimed either.
3. **A completed transaction that did not fail is committed.** Its commit record went out with a cache flush and forced unit access, and the completion was checked (`commit.c:152-154`, `commit.c:874-881`, `commit.c:888-889`). Under A-S1 the record and the log blocks before it are durable, and recovery replays the transaction from the log (`recovery.c:611`).
4. **It stays durable.** jbd2 drops a committed transaction from the log only after its home blocks were written with no recorded error (`journal.c:1861-1864`) and a cache flush followed:
   - at a commit, the commit record's flush, which is checked (`commit.c:746-765`, `commit.c:899-900`);
   - at a checkpoint, a flush whose status is discarded (`checkpoint.c:338-339`). A-S5 makes that flush succeed.

   An aborted journal never moves the tail (`journal.c:1859-1860`).

**What the probe's `fsync` shows, and what it does not.** A successful probe shows (2) only: the journal had not aborted when the probe's handle start tested it.
- **The `fsync` does not always wait.** It waits for the probe's transaction only while that transaction runs or commits (`journal.c:792-807`). For a completed one it returns 0 without the abort test (`journal.c:800-805`, F3), after ext4's own checked cache flush (`fsync.c:111-113`, `fsync.c:166-170`).
- **So it can pass with the journal aborted.** That happens when the abort lands:
  - after ext4's test and before jbd2's (`transaction.c:366-371`), which sets nothing;
  - after the handle started, when the probe's transaction completes before its `fsync`.
- **Neither window can hide a dependency that was not established.** By (1) and (2), every dependency's transaction completed without an abort before the probe's test. Such an abort is an error after the dependencies were established, so activation need not refuse it and does not relabel it as lost history.
  - **After a handle start.** The next handle start, by any process, turns the abort into emergency read-only. From then on every sync fails, the recorder's included (§9.5).
  - **Until then.** The recorder's syncs of its pure overwrites still issue and check their own cache flush (§4.3), so what they acknowledge is durable.

**Timing windows** [M: C28, C29]:

| Window | What v6.17 does | Activation | The dependencies |
|---|---|---|---|
| A dependency's transaction fails while its directory's sync waits | The sync tests the flag after the wait: EIO (`journal.c:689-690`) | Refuses (Unreliable) | Not established; nothing is claimed |
| It fails and completes before a sync that then finds nothing to commit | That sync returns 0 untested (`journal.c:513-517`); the probe's handle start forces emergency read-only; its `fsync` returns EROFS (`fsync.c:135-137`) | Refuses | Not established. The earlier failure is not hidden by the later success. |
| Any abort before the probe's handle test | EROFS, as above | Refuses; conservatively when the dependencies had committed | — |
| An abort between ext4's and jbd2's handle tests | `start_this_handle` returns EROFS and sets nothing (`transaction.c:366-371`); the inode keeps an older, completed sync tid (`ext4_jbd2.h:354-365`); `fsync` returns 0 (`journal.c:800-805`) | Certifies | Established before the abort, a later error |
| The probe's transaction still running at its `fsync` | Committed and waited for, then tested: EIO if the journal aborted meanwhile | Refuses on EIO, conservatively | Established before |
| The probe's transaction committing at its `fsync` | Waited for, then tested | As above | Established before |
| The probe's transaction completed before its `fsync` | 0 untested, after ext4's own checked flush | Certifies, even if a later commit has aborted the journal | Established before; not mislabelled as lost |
| An error after the dependencies were established | Not reported to the activation, or refused conservatively | Either | Unaffected: committed transactions are replayed (`recovery.c:611`) |
| Emergency read-only | Every `fsync` and `futimens` returns EROFS first (`fsync.c:135-137`, `inode.c:5854-5856`); the listing shows `emergency_ro` (`super.c:3034-3038`) | Refuses | — |
| An ordinary read-only superblock | A directory `fsync` returns 0 having committed nothing (`fsync.c:143-144`); `futimens` returns EROFS; the listing begins `ro` | Refuses (A3, A4) | — |
| After activation, a checkpoint whose flush the device fails | Status discarded (`checkpoint.c:338-339`); the tail can move; a power loss can then revert committed metadata, and no process sees an error | Already certified | **Outside domain H** (A-S5); the store cannot detect it (§12.2) |

**Enumeration** [M: C28; counts computed by the model and compared by the reference check]. The model's transaction state and its four windows through the whole store model are described in §16.1.

| Quantity | Count |
|---|---|
| Initial journal states: the two dependencies running, committing, committed or failed, with unrelated work | 50 |
| Activation runs: any background event or none before each step, an abort inside the probe's handle start or not, forced commits succeeding or failing | 259200 |
| Certified | 51820 |
| Refused | 207380 |
| Refused although the dependencies had committed (conservative) | 48404 |
| Certified with the journal aborted after the probe's test, each through the completed-transaction return | 32460 |
| Later continuations of certified runs, each followed by a power loss, every certified dependency kept | 310920 |

The enumeration is bounded, over a model that is not a kernel. It shows that the argument's steps hold in every enumerated interleaving, not that no other interleaving exists.

**Proof obligations.** Each claim; the source path or assumption it rests on; the modelled transition; the check that exercises it; and what remains to be established outside this design.

| # | Claim | Source path or assumption | Modelled transition | Check | Still required |
|---|---|---|---|---|---|
| P1 | A directory sync waits for every transaction up to the running or committing one and then tests the abort flag; with neither, it returns 0 untested | `journal.c:499-527`, `journal.c:678-690`, `journal.c:513-517` | `JournalSim.dir_fsync`; `SimFS.sync_dir` | C28, C29 | G-HOST: the running build's code |
| P2 | A failing commit sets the abort flag before the transaction completes; the flag is permanent | `commit.c:548`, `commit.c:866`, `commit.c:889`, `commit.c:1102-1105`, `journal.c:2515-2516` | `JournalSim.finish_commit`, `abort` | C28, C29 | G-HOST |
| P3 | The probe's handle start turns an earlier abort into emergency read-only, which its `fsync` returns first | `ext4_jbd2.c:81-89`, `super.c:722-732`, `inode.c:6531-6540`, `fsync.c:135-137` | `probe_touch`, `probe_fsync`; `SimFS.touch`, `fsync_file` | C25, C28, C29 | G-HOST |
| P4 | A regular file's `fsync` returns 0 untested for a completed transaction; an abort after ext4's handle test sets nothing | `journal.c:800-805`, `transaction.c:366-371`, `ext4_jbd2.h:354-365` | The completed branch; `abort_in_probe_handle` | C29 (conformance), C28 (safety) | G-HOST |
| P5 | A committed transaction's record went out with a checked cache flush and forced unit access | `commit.c:152-154`, `commit.c:874-881`, `commit.c:888-889` | `finish_commit`: the log durable | C28 | A-S1; G-PWR if ever required |
| P6 | The tail moves past a transaction only after its home blocks were written with no recorded error and a flush followed; at a checkpoint that flush's status is discarded | `journal.c:1861-1864`, `commit.c:899-900`, `checkpoint.c:338-339` | `checkpoint`, `_tail_update` | C28 (with A-S5), C29 (A-S5 necessary) | **A-S5 (new)**, attested; G-PWR |
| P7 | Recovery replays the committed transactions from the tail | `recovery.c:282-345`, `recovery.c:611` | `JournalSim.survives` | C28 | G-HOST |
| P8 | The probe follows the last A2 sync | §10.6 A3 | `journal_activation` order | C28 (NC-PROOF-ORDER) | G-IMPL: the Rust order |
| P9 | Every dependency directory is synced, on one filesystem, and revalidated | §10.6 A1, A2, A4 | `activate` | C25, C26 | G-IMPL |
| P10 | The profile excludes the two discarded `commit.c` flushes and fast commits, from effective state | §6.4; `commit.c:775-778`, `commit.c:883-886`, `super.c:4963-4968` | `check_profile`; the profile sweep | C27, C29 | The Owner's qualification facts (§13.2) |
| P11 | The running kernel is the qualified build | §6.4 step 11 | `check_profile`, kernel comparison | C27 | G-HOST: each build qualified |
| P12 | Nothing depends on the store before activation | §10.6 "Before what"; INV-18 | `startup`, `MaintenanceSession.begin` | C25, C26 | G-IMPL |

---

## 11. Classification and restart knowledge (R6)

### 11.1 Per-file classification (a pure function)

**Input:** the file's bytes, the root id, C_pool, and the file's pool index. Steps, in order:

1. **Size.** It must equal (C_pool + 2) × 4096. A wrong size makes the store **Invalid**, since in domain H sizes never change (§7.7).
2. **All zero.** A pool file that is all zero is **Unused**.
3. **Header.**
   - Valid: continue.
   - Checksum-valid but not valid: **Malformed (pool-file)**.
   - Not checksum-valid, every other block zero: **AbandonedClaim**. In domain H this is an interrupted claim. No record was written, because records are written only after the header is synced, so the generation admitted nothing (INV-2). The file is spent. The report states that this does not prove nothing happened in domains S or A.
   - Not checksum-valid, anything else non-zero: **Malformed (pool-file)**.
4. **Records** (§7.4), for *n* = 1 to capacity:
   - the first all-zero block ends the prefix, and every later record block must also be zero; otherwise the file is **Malformed (journal)**, because it has a hole;
   - blocks beyond capacity must be zero;
   - any non-zero invalid block makes the file **Malformed (journal)**. A torn last block is never read as benign (§5.4) [M: C04].
5. **Grammar** over the prefix (§11.4). A violation makes the file **Malformed (journal)**.
6. **Seal** (§7.3): unsealed, unreadable (treated as unsealed), valid, or Malformed.
7. **Class:**

| Class | Condition |
|---|---|
| Sealed | Valid seal that matches the prefix |
| Unsealed, no action start | The prefix holds no `ActionStarted` |
| Unsealed, action start | The prefix holds at least one `ActionStarted` |
| Malformed (journal) | A valid header, but any later failure above |
| Malformed (pool-file) | No valid header, other than AbandonedClaim |

### 11.2 Restart report and refusal

For each generation the store reports four separate things. It **never names an identity that no visible record holds**, and never uses the absence of a record as proof.

| Report field | Values |
|---|---|
| Recorded unsettled | The action and incident identities started or opened, and not settled, in the visible valid prefix. Exact for the prefix. |
| Evidence completeness | Complete only if sealed (domain H). Unsealed means records may have been issued and lost, failed or never written. Malformed means unknown. |
| Native work possible | False only for a structurally valid file whose prefix holds no `ActionStarted`, and for Unused and AbandonedClaim files. True for any action start, any Malformed file and any claim gap. |
| Refused | True for a Malformed file, a claim gap, and an unsealed generation where native work is possible, **including one with a recorded-unsettled count of zero**. Each needs a valid disposition, or archival as history (§13.5). |

Mapping to the core (`model.rs:683-702`) [S]:
- Malformed files and claim gaps map to `PriorOutcome::Malformed`.
- An unsealed generation with an action start maps to `PriorOutcome::Unresolved { outstanding }`, with `outstanding` equal to the recorded-unsettled count. **That count describes the records only; it is not a bound on native state.**
- `Prior::blocks` treats every outcome except `Resolved` as blocking until dispositioned, including `Unresolved { outstanding: 0 }` (`core.rs:1207-1209`) [S].
- Sealed and no-native-work generations are not passed to the core.

An unsealed generation where native work is possible carries two uncertainty notes:
- "late owners or other native effects may exist with no durable record (`core.rs:1924-1930`)";
- "an owner surfacing after `close()` is outside custody (§14.4)".

[M: C09, C16]

**Bounded report (R2).** A report lists at most 64 incidents in full: those without a claim first, then in claim order, then by pool index and binding. It always carries the exact total of incidents, the number that are current, and a **partial** flag, set whenever detail was omitted. Every decision uses the complete incident set, never the listed detail: the refusal, the priors passed to `Custody::new`, the applied list of the claim header, the Capacity refusal, and the preconditions of every §13 procedure. A partial report is never presented as complete, and a truncated set never authorizes execution or retirement [M: C24].

### 11.3 Pool-level and archive checks

| Check | Rule | Result |
|---|---|---|
| Claims | The claims of valid headers in pool files and `j-` archive entries above `retired-through` must form exactly `retired-through + 1 ..= max` | Each missing claim is a **claim-gap** incident |
| Pending recycle | A pool file and a `j-` entry with the same claim, generation and content digest, or a pool file and a `p-` entry with the same index and content digest | The pool file is archived, pending recycling (§13.8). This is not a duplicate. |
| Duplicates | Any other repeated claim or generation | **Invalid** (domains S, A or Owner error; impossible in domain H) |
| Retirement | A pool file whose claim is at or below `retired-through` | **Invalid** (§13.9) |
| `j-` entry | The header is valid and matches the claim and generation in the name. Class `s`: the seal block is checksum-valid and its generation, claim and header digest match the header. Classes `u` and `m`: a valid disposition exists for the binding (§12.3). | Otherwise **Invalid** ("archive inconsistent") |
| `p-` entry | A valid disposition exists for the kind `03` binding of its index and content | Otherwise **Invalid** |
| Retired `j-` entries (claim ≤ `retired-through`) | Name and type are checked; nothing else | — |

The rest of an archive entry is not re-read at startup. It was verified by the root procedure that created it, which archives only history (§13.7). It is root-owned and not writable by the store uid.

### 11.4 Record grammar, audited against the core

The grammar is **prefix-closed**: each rule refers only to earlier records and the current one, so a log that stops anywhere is valid. "Before X" means earlier in the journal. Embedded identities (actions, incidents) must carry the record's generation (rule G-id), because the codec checks only syntax (`codec.rs:75-79`) [S].

| Rule | Statement | Source |
|---|---|---|
| G1 | Instants are non-decreasing | `core.rs:3033-3039`, `core.rs:1716-1726` |
| G2 | At most one `RunStarted`, never after `Control{AdmissionClosed}`. Its `dispositioned` equals the header's n. | `core.rs:1459-1488` (closure observed before the start: `core.rs:1469-1471`) |
| G3 | Before `RunStarted`, only control facts appear. A `ShutdownRefused` there needs an earlier `AdmissionClosed`: before the start, a refusal needs pending or failed evidence, and the only possible earlier record is that closure. | `core.rs:2881-2924`, `core.rs:2722-2737`, `core.rs:2846-2866` |
| G4 | `CaseStarted` needs a `RunStarted` before it and no open case. It never follows `AdmissionClosed`, `RecoveryRequired` or `RunEnded`. Every earlier action and incident must be settled. | `core.rs:1519-1566` (halted check at `core.rs:1525-1529`; open case at `core.rs:1533-1535`; unresolved state at `core.rs:2617-2619`) |
| G5 | `CaseEnded` closes the open case, with its id. If it says `passed`, then every action started in the case is settled, and no `ActionFailed` and no `AdmissionClosed{Failed}` occurred while the case was open. | `core.rs:2157-2212`, `core.rs:3046-3069` |
| G6 | `ActionStarted` needs an open case and is never after `AdmissionClosed`. Action numbers are contiguous from 1. | `core.rs:1579-1620` (closure observed first: `core.rs:1580-1582`; numbering: `core.rs:1602-1606`) |
| G7 | `ActionFailed` names a started action, at most once, before that action's settlement and before `RunEnded` | `core.rs:1813-1849`, `core.rs:1989-2016`, `core.rs:2216-2233`, `core.rs:2238-2255` |
| G8 | `ActionSettled` names a started action, at most once, before `RunEnded`. `NotAdmitted` never follows that action's `ActionFailed`. | `core.rs:1700-1704`, `core.rs:1783-1808`, `core.rs:2570-2607` |
| G9 | At most one `Control{AdmissionClosed}`. Its `after` is at most the number of action starts before it. | `core.rs:2374-2387`, `core.rs:448-465`, `core.rs:1665-1675` |
| G10 | At most one `Control{ShutdownRefused}` | `core.rs:2856-2864` |
| G11 | `RecoveryRequired` needs a `RunStarted` before it and no open case, and never follows `RunEnded` | `core.rs:2624-2644`, `core.rs:1979-1982` |
| G12 | `RecoveryAttempt` numbers are contiguous from 1. Before `RunEnded`, an attempt needs more `RecoveryRequired` records than earlier attempts with resolved true. After `RunEnded`, it needs an `IncidentOpened` after `RunEnded`. | `core.rs:2801-2837`, `core.rs:2648-2652` |
| G13 | `IncidentOpened` numbers are contiguous from 1, and each names a started action. Tickets are never constructed outside the core. | `core.rs:1931-1987` (numbering: `core.rs:1952-1956`), `core.rs:117-126`, `core.rs:1744-1752` |
| G14 | `IncidentSettled` names an opened incident, at most once, with Confirmed or OutputLost | `core.rs:2570-2607`, `core.rs:741-747` |
| G15 | At most one `RunEnded`. It needs a `RunStarted` before it, no open case, and every started action and opened incident already settled. Its verdict is Passed or Failed. `resolved_by` is absent if no `RecoveryRequired` precedes it; otherwise it is the number of the last earlier attempt with resolved true. | `core.rs:2662-2687`, `core.rs:2700-2706`, `core.rs:2648-2652` |
| G16 | `RunEnded` is Failed **if and only if** an `AdmissionClosed` precedes it | `core.rs:3046-3069` (every failure closes admission and records the first closure at once); `core.rs:2395-2415` (a cancellation before the commitment fails the run); `core.rs:2700-2706`; `core.rs:1298-1304` (the control reserve of at least 2 always fits both control facts) |
| G17 | After `RunEnded`, only `IncidentOpened`, `IncidentSettled`, `RecoveryAttempt` and `Control{ShutdownRefused}` occur | `core.rs:2801-2837`, `core.rs:1931-1987`, `core.rs:2846-2866`, `core.rs:448-465` (no closure after the commitment) |

Each path the audit had to cover:

| Path | Covered by |
|---|---|
| Control records before the start | G2, G3 |
| Failed evidence | The prefix stops; prefix closure |
| Cancellation | G6, G9, G16 |
| Late incidents before the terminal record | G13, plus the closure and recovery (G9, G11) |
| Late incidents after the terminal record | G13, G17 |
| Recovery after the terminal commitment | G12, G14, G17 |

### 11.5 Source-derived design fixtures

These record sequences were **derived by reading the cited paths at `45898e05`. They are not output of the Rust core.** Bracketed items are native events: `+a1` means an admitted operation or held owner for action 1 exists, `−a1` that its end was confirmed or proven. They are not records. Abbreviations:

| Abbreviation | Record |
|---|---|
| RS | `RunStarted` |
| CS | `CaseStarted` |
| AS | `ActionStarted` |
| AF | `ActionFailed` |
| ASet | `ActionSettled` |
| CE | `CaseEnded` |
| AC | `Control{AdmissionClosed}` |
| SR | `Control{ShutdownRefused}` |
| RR | `RecoveryRequired` |
| RA | `RecoveryAttempt` |
| RE | `RunEnded` |
| IO | `IncidentOpened` |
| IS | `IncidentSettled` |

| Id | Scenario | Sequence | Derived from |
|---|---|---|---|
| T1 | Clean pass | RS, CS(Clean), AS(a1), [+a1], [−a1], ASet(Confirmed), CE(true), RE(Passed) | `core.rs:1459-1488`, `core.rs:2157-2212`, `core.rs:2570-2607`, `core.rs:2662-2687` |
| T2 | Cancelled before the start, shutdown refused | AC(Cancelled(Requested), 0), SR | `core.rs:2346-2353`, `core.rs:2846-2866` |
| T3 | Cancelled after admission | RS, CS, AS(a1), [+a1], AC(Cancelled(Requested), 1), [−a1], ASet(Confirmed), CE(true), RE(Failed) | `core.rs:2157-2212`, `core.rs:2395-2415` |
| T4 | Failed cleanup, then recovery | RS, CS, AS(a1), [+a1], AF, AC(Failed(UnexpectedCleanup), 1), CE(false), RR, [−a1], ASet(Confirmed), RA(1, true), RE(Failed, 1) | `core.rs:2238-2255`, `core.rs:2624-2644`, `core.rs:2801-2837` |
| T5 | Unknown outcome, then proof of no effect | RS, CS, AS(a1), [+a1], AF, AC(Failed(UnknownOutcome), 1), CE(false), RR, [−a1], ASet(NothingCreated), RA(1, true), RE(Failed, 1) | `core.rs:2216-2233`, `core.rs:1783-1808` |
| T6 | Authority lost | RS, CS, AS(a1), [+a1], AF, AC(Failed(AuthorityLost), 1), CE(false), RR (never resolves) | `core.rs:1813-1849` |
| T7 | Late owner after the terminal record | T1's records, then [+i1], IO(i1, a1, Process), [−i1], IS(i1, Confirmed), RA(1, true) | `core.rs:1931-1987`, `core.rs:2801-2837` |
| T8 | T7 crashed after [+i1], with the IO record not durable | Visible: T1's records only | `core.rs:1924-1930` |
| T9 | Retained boundary, met | RS, CS(RetainedBoundary), AS(a1), [+a1], [−a1], ASet(Confirmed), CE(true), RE(Passed) | `core.rs:1855-1922`, `core.rs:2487-2565` |
| T10 | Declared output detachment | RS, CS(OutputDetached), AS(a1), [+a1], [−a1], ASet(OutputLost), CE(true), RE(Passed) | `core.rs:1813-1849`, `core.rs:2259-2278` |
| T11 | Shutdown during a case | RS, CS, AS(a1), [+a1], AC(Cancelled(Shutdown), 1), SR, [−a1], ASet(Confirmed), CE(true), RE(Failed) | `core.rs:2846-2866` |
| T12 | Recording fails at CE | T1's records up to ASet are durable; CE's sync fails | `core.rs:3016-3031` |
| T13 | Late owner while a completion candidate | RS, CS, AS(a1), [+a1], [−a1], ASet(Confirmed), CE(true), [+i1], IO(i1, a1, Process), AC(Failed(LateOwner), 1), RR, [−i1], IS(i1, Confirmed), RA(1, true), RE(Failed, 1) | `core.rs:1931-1987`, `core.rs:2637-2644`, `core.rs:2801-2837` |
| T14 | Cancelled before admission | RS, CS, AS(a1), AC(Cancelled(Requested), 0), ASet(NotAdmitted), CE(true), RE(Failed) | `core.rs:1626-1697`, `core.rs:1700-1704` |
| T15 | Owner of an unexpected kind | RS, CS, AS(a1, Workspace), [+a1], AF, AC(Failed(UnexpectedOwner), 1), [−a1], ASet(Confirmed), CE(false), RE(Failed) | `core.rs:1855-1922` |

The model checks the following [M: C13]:
- every fixture is accepted;
- every prefix of every fixture is accepted;
- 32 mutations (31 single-rule edits and one regression of an instant) are each rejected by the intended rule.

**Reference checks and fixtures are not conformance proof.** The Rust implementation must later record every journal produced by the real deterministic core runs and classify them (§16.3, I9).

---

## 12. Invariants and bindings

### 12.1 Invariants, with their domains

| Id | Invariant | Domain | Model |
|---|---|---|---|
| INV-1 | **Acknowledged implies durable.** Every acknowledgement the core applied names a block that was durable when `durable_through` was published. | H | C15 |
| INV-2 | **Write-ahead.** Every action the core admitted has a durable `ActionStarted`: admission needs the acknowledgement (`core.rs:1658-1664`) [S], and INV-1 holds. | H | C15, C16 |
| INV-3 | **No false resolution.** Suppose that at a crash point the custody held, had open or had lost any native owner or operation, or had been delivered a late owner (recorded or not). Then restart refuses. | H | C09, C16 |
| INV-4 | **Nothing beyond P (R2).** Let P be `durable_through` when the first fatal cause latched: an I/O failure, recorder loss or vanishing, an invalid or conflicting submission, an unexpected acknowledgement outcome, or a poisoned mutex. No sequence beyond P is ever published or acknowledged. Acknowledgements up to P are applied and never retracted, whatever the cause, including a conflict on an already acknowledged record. The core is told of the failure only at its next unacknowledged record, only once that record is issued, and once. | H, M (recorder behaviour) | C08, C15, C18, C19 |
| INV-5 | **Totality and bounds.** Classification terminates with bounded memory and reads (§6.6, §7.7). | H, M, S | C10, C17 |
| INV-6 | **Determinism.** Equal bytes give equal classes, reports and bindings. | all | C11 |
| INV-7 | **Binding sensitivity.** Any byte change in a journal changes its binding, which explicitly makes a disposition stale. | all, assuming SHA-256 collision resistance [A] | C11 |
| INV-8 | **Disposition authority.** Only root-owned dispositions that bind exactly are accepted. | H, A (for dispositions only) | C12 |
| INV-9 | **Seal meaning.** A valid seal means `close()` returned `Ok`. | **H only.** In domain A an unkeyed seal proves nothing. | C16 |
| INV-10 | **Exclusion.** While an owner is alive, no cooperating owner or maintenance procedure acquires the store lock. Recorder failure does not change this. | H | C05, C06 |
| INV-11 | **Report honesty.** Recorded-unsettled sets are exact for the prefix. No identity is invented. Completeness is claimed only when sealed. Durability is never claimed from a new description's sync. | H, M | C03, C09 |
| INV-12 | **Administrative atomicity.** No partial disposition, `PROVISION` or archive entry is accepted, under every permitted metadata schedule (§4.8). Retirement cannot exclude unresolved, undispositioned history. A changed binding is never silently honoured. | H, with the Owner following §13 | C11, C12, C22 |
| INV-13 | **Admission fence (R2).** Every admission that returned `Admitted` linearized before the fatal latch; from the latch on, every admission is refused without calling the core's admission. No native operation starts without an admission. | H | C18, C19 |
| INV-14 | **Selection authority (R2).** An owner, a verifier or a maintenance session acts on a `PROVISION` selection only after revalidating it under the store lock (§10.5). Two cooperating owners never both hold an authoritative selection. Authority to claim further requires activation (INV-18). | H | C21, C25 |
| INV-15 | **Session authority (R2).** Maintenance verification and mutation run only under the session's retained exclusive lock descriptions, never through a new lock request, a conversion, a release, or an asserted identity. Competitors stay excluded for the whole session. | H, with the Owner following §13 | C20 |
| INV-16 | **Bounded aggregates (R2).** Every directory is counted against its bound before any entry is examined. Reports beyond their detail bound say they are partial. Every decision uses the complete set. | H, M, S | C17, C24 |
| INV-17 | **Protocol parity (R2).** Every durability operation the model performs is a written protocol step, in the written order, and the crash matrix of §15.2 is the one the model computes. | Model | C22, C23 |
| INV-18 | **Durable activation (R3; precise in R4).** No claim, record, acknowledgement or native admission exists on a selection that its owner has not activated (§10.6): every dependency directory synced, the probe passed, and revalidation done, all under the store lock held since the selection was revalidated. No maintenance session verifies or mutates before activating. R4: activation means that every transaction holding a dependency has committed, and that it stays durable under A-S1 and A-S5 (§10.7). It does not mean that every earlier transaction succeeded. | H | C25, C28 |
| INV-19 | **History across recovery (R3).** Once work depends on a selection, every permitted later crash outcome (F1, or F2 under every schedule) and every specified recovery either selects a store whose decision includes every acknowledged, unsealed generation, or refuses explicitly with that generation's bytes retained. Recovery never provisions a fresh root over a root that may hold history. | H, with the Owner following §13 | C25, C26 |
| INV-20 | **Supported profile (R4).** No owner, verifier or session acts on a store whose effective profile is not the qualified supported one (§6.4): the effective options, the journal's location and the kernel are read at every opening and again at A4. Unknown, contradictory or unqualified information refuses before any claim. | H, with the Owner keeping the qualification facts (§13.2) | C27 |

**Why INV-3 holds.**
- Any native owner arises from an admitted action, so its `ActionStarted` is durable (INV-2) and therefore visible at every later startup in domain H.
- Blocks are written once, and each record block is written only after the previous one is synced. So a torn or missing block can only follow the last durable record, which makes such a journal either a shorter valid prefix or Malformed.
- A seal exists only after `close()` returned `Ok`, which needs nothing held (INV-9).
- Hence a generation with outstanding native state is unsealed with an action start, or Malformed, and either way it is refused (§11.2), whatever its recorded-unsettled count. The model checks this at every crash point of every fixture, including the unrecorded late owner [M: C16].

**Residual outside INV-3.** An owner that surfaces after `close()`, for example through an adapter contract violation, is outside custody and outside every store guarantee (§14.4).

### 12.2 Domain notes

| Domain | What holds |
|---|---|
| M | Totality, bounds, determinism and fail-closed structure. Arbitrary bytes can still form a well-formed, misleading journal. |
| S | Detectable faults are refused. Lost or misdirected flushed writes are undetectable (§5.2). R4 adds one more undetectable fault: a cache flush the device fails at a checkpoint. v6.17 discards its status (`checkpoint.c:338-339`), and the tail can still move. A power loss before a later successful flush can then revert committed metadata, including an activated selection's dependencies, after work depended on them. INV-18 and INV-19 do not hold across it (A-S5). Every other flush failure on the profile's paths is reported: through a commit (an abort, then EIO or EROFS at the store's next sync) or through `fsync`'s own flush. |
| A | Only INV-6, INV-7 and INV-8 hold (§3.3, G-AUTH). |

### 12.3 Bindings

`IncidentBinding` (`model.rs:683-702`) [S] is the SHA-256 of fixed-width, big-endian bytes. Every binding starts with:
1. the ASCII prefix `nexus-phase2-custody-incident`, then `00`;
2. the binding version `01`;
3. a kind byte.

| Kind | Bytes after the kind byte | Incident |
|---|---|---|
| `01` journal | root id ‖ claim ‖ generation ‖ content SHA-256 ‖ class (`01` unresolved, `02` malformed) | A pool journal with a valid header that is unsealed with an action start, or Malformed (journal) |
| `02` claim gap | root id ‖ missing claim | A claim in the expected range that has no valid header |
| `03` pool file | root id ‖ pool index (`u32`) ‖ content SHA-256 | Malformed (pool-file) |

- **Journal bindings do not depend on location.** A byte-identical archive copy keeps its binding [M: C11]. A byte change produces a new binding, so the old disposition is reported as **stale**: explicitly, never silently (INV-7, INV-12).
- **Pool-file bindings** are tied to the pool index, because without a valid header the file has no other identity. A `p-` archive entry keeps the index in its name, so the binding survives archival.
- **Claim-gap bindings** have no file.

---

## 13. Administrative procedures (D-7, D-8, R9; R2: maintenance sessions)

### 13.1 Common rules

Every procedure is **offline and performed by root**, inside one **maintenance session** (R2). The session is a specified future interface, a maintenance mode of the owner binary for a later mission. Nothing here authorizes a provisioning executable or a privileged service.

1. **Begin.** The session selects `PROVISION` (§10.5), opens `LOCK` safely, and takes `LOCK_EX | LOCK_NB` on a description it retains. Busy aborts the session. It then revalidates the selection (§10.5) and **activates** it (§10.6, R3), so that verify-before and every decision read a durable namespace. A failed activation ends the session. **Root is not exempt**: a step performed outside a session is outside the procedure and unsupported, and the verifier cannot detect it.
2. **Retain.** The session keeps that description, and every journal lock description it takes, until it ends. It never calls `LOCK_UN`, never converts a lock to `LOCK_SH`, never closes a lock description early, and never passes one to another process. R1's `flock -n <STATE_ROOT>/LOCK <procedure>` wrapper is replaced, because it could not verify: the standalone verifier takes a shared lock and is Busy inside every procedure [M: C20, NC20a].
3. **Journal locks.** It takes `LOCK_EX | LOCK_NB` on every pool file the procedure reads or replaces, and keeps it. Busy aborts.
4. **Verify before**, in-session (§13.11), and confirm the procedure's preconditions from the complete report (§11.2).
5. **Persistence order.**
   - Write each new file under its temporary name in its target directory, created exclusively (`O_CREAT | O_EXCL | O_NOFOLLOW`), and `fsync` it.
   - Publish it with `link` (no-replace: EEXIST aborts) or `rename` (atomic replacement).
   - `fsync` each changed directory after publication (`fsync(2)`, `link(2)`, `rename(2)`) [U].
   - Every operation is listed, in order, in §15.3. No other durability operation is performed [M: C23].
6. **Verify after**, in-session.
7. **End.** Close the session's descriptors, which releases its locks.

**Nested procedures.** A procedure that includes another (recycling and retirement include the §13.3 rewrite; succession includes §13.2 steps 2 to 4) runs it in the same session, within the same lock lifetime. Verify-before, every step, any re-selection and verify-after share one session. No competitor can enter between them [M: C20].

**Leftover temporary entries.** A `.tmp-` entry left in a store directory by an interrupted procedure refuses opening as **MaintenanceIncomplete** (§7.5). Opening never reads, uses or removes it. A leftover `<PROVISION_PATH>.tmp` or `<PROVISION_PATH>.predecessor.tmp` is never read and makes the next exclusive create fail. **Recovery** is a session that unlinks each leftover temporary entry, `fsync`s each directory changed, verifies, and then completes or repeats the interrupted procedure (§15.2) [M: C12].

**Existing descriptors.** Ownership changes never use `chown` or `chmod` on a file the store uid may hold open, because neither revokes an existing descriptor. Procedures copy into new root-owned inodes (archival) or replace names (recycling).

**Unsupported.** Anything not specified in §13.2 to §13.10 is unsupported, including:
- un-archiving, editing journals, and deleting dispositions other than by revocation;
- recycling a Malformed pool file that has not been dispositioned and archived;
- automatic archival, recycling or retirement.

Each procedure's crash points are listed in §15.2 and its operations in §15.3 [M: C12, C22, C23].

### 13.2 Initial provisioning (manual; no executable) [D-8]

1. **Qualify the host.** The Owner establishes all of the following; if any fails, the store is not provisioned (§5.4):
   - ext4 at `<STATE_ROOT>`, identified through the mount;
   - the option strings to pin;
   - block size 4096 and page size 4096;
   - sysfs logical and physical block sizes for the mount's major:minor, each dividing 4096;
   - `write_cache` recorded;
   - the filesystem has a journal and no `fast_commit` feature (A-M1, §4.8);
   - (R4) from the superblock, read as root: the journal is internal, at inode 8, with no journal device and no journal UUID; the feature list has `has_journal` and lacks `fast_commit`. The store uid cannot read these later (§5.3), so the Owner re-qualifies after any change to them, and the store is unsupported until then;
   - (R4) every rule of §6.4 steps 9 and 10, read as the store uid will read it: the effective listing, and the journal's jbd2 name;
   - (R4) the running kernel's build is qualified. The Owner checks the build's sources (the distribution's source package for that release, not only the upstream tag) against every source claim of §5.1 and §10.7. Its identity, `uname(2)` release and version, is pinned as `kernel` (§7.5). A version string alone qualifies nothing;
   - the A-S1 to A-S5, A-M1 and A-M2 attestation for the device and the running kernel. A-S5 (R4): the device is not known to fail cache flushes; a device without a write-back cache meets it by construction (§5.1);
   - the directory of `<PROVISION_PATH>`, the parent of `<STATE_ROOT>` and every ancestor of both exist, are root-owned and neither group- nor other-writable, and lie on the state root's filesystem (§6.2). Each directory created for them is `fsync`ed, and so is its parent, so that no ancestor's entry can roll back (R3).
2. **Create directories.** Create `<STATE_ROOT>`, `journals/`, `dispositions/`, `dispositions/revoked/` and `archive/`, each `root:root 0755`. `fsync` each, and its parent.
3. **Create the lock.** Create `LOCK` as `uid:gid 0600`, empty, and `fsync` it. Then `fsync <STATE_ROOT>`, which now holds the new entry (R2: the R1 model performed this sync without the text saying so).
4. **Create the pool.** For each pool file, created under its final name in the still-unpublished root:
   - `fallocate` mode 0 for (C_pool + 2) × 4096 bytes; EOPNOTSUPP or ENOSPC aborts;
   - write zeros over the whole file and `fsync` it;
   - check with FIEMAP (`FIEMAP_FLAG_SYNC`) that every extent is written and unshared (§6.5).

   Then `fsync journals/`.
5. **Publish `PROVISION`**, with `revision=1`. Write it to `<PROVISION_PATH>.tmp` and `fsync` it. Then `rename` it to `<PROVISION_PATH>` and `fsync` the directory.
6. **Check.** Run the standalone verifier as the store uid. It must report a fresh, openable store.

Provisioning runs before any store exists, so it has no session; nothing can select the store before step 5. `PROVISION` is published **last**. Until its rename is visible, every opening reports Unprovisioned. Between the rename and the directory sync, an opening after F1 sees the fresh store, and after F2 either outcome. After the sync the store is fresh (§15.2). An owner that opens the store in that window activates it first (§10.6), which makes the rename durable before any claim.

**Recovery after Unprovisioned (R3).** Unprovisioned is a refusal, never permission to provision a fresh store over existing roots.
- The Owner first lists every state root under the parent for the uid, together with any leftover `<PROVISION_PATH>.tmp`.
- **A root with any non-zero pool byte may hold history.** The Owner re-publishes `PROVISION` for that root (step 5): its own root id, recorded in its name, and the inodes it actually has. The ordinary refusal and disposition then follow. If several roots hold history, nothing is provisioned until the Architect disposes of them.
- **A root whose pool files are all zero** never had a claim, since every claim writes a header. The Owner may remove it, with any leftover temporary, and provision again.

R2 allowed removing "the incomplete root, which holds no history" without the check. The composed counterexample showed that root holding an acknowledged, unsealed generation (§19.4). Under activation, a root without a durable `PROVISION` can never have had a claim in domain H, so the check is the defence for domains S and A and for stores used before R3 [M: C26].

### 13.3 Rewriting `PROVISION`

Used by recycling, retirement and re-qualification, always inside the procedure's session:
1. The session holds the store lock (§13.1).
2. Write the complete new content, with `revision` one more than the current one, to `<PROVISION_PATH>.tmp`, and `fsync` it.
3. `rename` it over `<PROVISION_PATH>`, and `fsync` the directory.
4. **Re-select** under the session's own lock: read `<PROVISION_PATH>` by a fresh lookup and adopt it as the session's selection. The `LOCK` inode is unchanged, so the session's authority continues.
5. Verify.

`retired-through` and `predecessor` are the only fields that record retirement or succession. Both are root-owned.

**Re-qualification.** When the mount options no longer match `PROVISION`, every opening refuses as Unsupported (§6.4). After re-qualifying the host (§13.2 step 1), the Owner rewrites `PROVISION` with the new option strings. Its session is begun in **re-qualification mode**: identical to §13.1 except that the option comparison of §6.4 step 5 is not applied to the stale strings being replaced; every other check, the lock and the revalidation still apply [M: C12, C21].
- **After a kernel update (R4).** Every opening refuses as Unsupported ("kernel not qualified", §6.4 step 11) until the Owner qualifies the new build and rewrites `PROVISION` with its identity in the same way. Re-qualification mode also skips step 11's comparison, for the identity being replaced.
- **What it never skips.** The rules of §6.4 steps 9 and 10, so a re-qualification never accepts an unsupported profile [M: C27].
- **After any change to the superblock facts of §13.2 step 1.** The Owner re-qualifies.

### 13.4 Disposition publication, reading and revocation

**Publication:**
1. Begin a session.
2. The in-session verification reports the incident and its binding.
3. Write `dispositions/.tmp-<binding>` with exactly the §7.5 content, as `root:root 0444`, and `fsync` it.
4. `link` it to `<binding>.disposition`. EEXIST aborts: the existing disposition must be revoked first.
5. `unlink` the temporary file.
6. `fsync dispositions/`.
7. Verify.

**Reading at startup.** Every `<binding>.disposition` file must:
- be regular, `root:root 0444`, with `st_nlink == 1`;
- be at most 4096 bytes;
- parse exactly (§7.5);
- recompute to the binding in its name;
- carry the root id from `PROVISION`.

Any failure refuses as **Invalid**. The validator given to `apply_disposition` returns a `ValidatedDisposition` only for a binding that has such a file whose fields equal the incident the store computed, including `recorded-unsettled` [M: C12].

**Revocation:**
1. Begin a session.
2. `rename` `<binding>.disposition` to `revoked/<binding>-<YYYYMMDDTHHMMSSZ>.disposition`.
3. `fsync revoked/`, then `dispositions/`.
4. Verify.

After revocation, the incident blocks again (§13.5). Under A-M1 the rename is atomic, so a crash leaves the disposition under exactly one name. Under the per-directory over-approximation (§4.8) a crash between the two syncs can leave both names; the link count is then 2, and opening refuses as **Invalid**. The Owner completes the revocation by unlinking the `dispositions/` name and syncing that directory [M: C12, C22]. For a `u`, `m` or `p-` archive entry, the store refuses as **Invalid** ("archive inconsistent") until the Owner publishes a disposition again; un-archiving is unsupported. Revoked files stay in `revoked/` as evidence.

### 13.5 History: applied or archived

A current incident becomes **history** in either of two ways. It is then no longer passed to `Custody::new`.

| Way | Condition |
|---|---|
| (a) Applied | Some valid header, in a pool file or an archive entry, lists its binding, **and** a valid disposition for it still exists. The custody that applied it wrote the header before `start_run` (§9.7). |
| (b) Archived | It was archived under §13.7, and the archive checks of §11.3 pass |

- **Revocation** (§13.4) removes the disposition, which returns an applied incident to current.
- **Forging an applied list** as the store uid gains nothing, because the root-owned disposition must still exist.

### 13.6 Successor provisioning

A successor store replaces a store that cannot continue: for example a store that is Invalid, or a pool full of files that cannot be recycled.

**Precondition.** The predecessor's in-session verification shows that every bound incident (§12.3) has a valid disposition. The `predecessor-statement` names each store-level Invalid condition the report shows. Those conditions have no binding; the statement is the Owner's explicit, recorded acceptance of them.

**Steps** (R2: one session holds both stores' locks from before the successor can be selected):
1. Begin a session on the predecessor and keep it for the whole procedure. Save the in-session report.
2. Inside that session:
   - **2a. Keep the predecessor's `PROVISION`.** Write a copy to `<PROVISION_PATH>.predecessor.tmp`, `fsync` it, `link` it to `<PROVISION_PATH>.predecessor-<old root id>`, `unlink` the temporary, and `fsync` the directory.
   - **2b. Create the successor's store** at a **new** `<STATE_ROOT>`, with a new root id: §13.2 steps 2 to 4.
   - **2c. Take the successor's lock, then publish.** Open the successor's `LOCK` safely and take `LOCK_EX | LOCK_NB` on a description the session retains. Only then write the successor `PROVISION` (`predecessor` set to the old root id, `predecessor-statement` filled in, `revision` one more than the predecessor's) to `<PROVISION_PATH>.tmp`, `fsync` it, `rename` it over `<PROVISION_PATH>`, and `fsync` the directory.
   - **2d. Re-select** the successor under the session's lock on it (§13.3 step 4), and verify it in-session.
3. End the session. The predecessor's state root is left unchanged, as evidence.

**Predecessor retirement.** From the rename on, while it is visible, no process can revalidate the predecessor, because `PROVISION` no longer names it (§10.5). A startup that read the predecessor's `PROVISION` before the rename and locks after it fails revalidation and re-selects the successor, which is Busy until the session ends. No cooperating process ever treats both stores as current [M: C21].

**Until the rename is durable** (the session's step 2c sync, or the first owner's activation, §10.6), a power loss can restore the predecessor. No work can depend on the successor in that window, because no owner claims before activating. Once activation has made the successor durable, the predecessor cannot return [M: C25, C26].

The successor never scans the predecessor. No unresolved incident is dropped silently: each needs its own disposition first, and each unbound condition is named in the statement.

### 13.7 Archival

**Preconditions,** verified in-session:
- a pool file with a valid header that is Sealed (class `s`), unsealed without an action start (`n`), or unsealed with an action start (`u`) or Malformed (journal) (`m`) with a valid disposition; or
- a Malformed (pool-file) file with a valid disposition (the `p-` form).

AbandonedClaim files are not archived; they are recycled directly (§13.8).

**Steps:**
1. Begin a session and take the file's `LOCK_EX | LOCK_NB`.
2. Read the file through that description and compute its content SHA-256, and its binding where it has one.
3. Create `archive/.tmp-<name>` as `root:root 0444` (`O_CREAT | O_EXCL | O_NOFOLLOW`). Write the bytes and `fsync`. Re-read the copy and compare the digest.
4. `link` it to its final name (§7.5).
5. `unlink` the temporary file, then `fsync archive/`.
6. Verify.

- **The pool file is unchanged.** It is now archived and pending recycling (§11.3), not a duplicate.
- **The binding is preserved,** because it is content-addressed [M: C11].
- **The copy is safe from the store uid.** It is a new root-owned inode, so no store-uid descriptor can write to it.

### 13.8 Recycling

**Preconditions:** the pool file is archived and pending recycling (§11.3), or it is an AbandonedClaim. Both are read from the in-session report, which the Owner saves.

**Steps:**
1. Begin a session and take the journal lock.
2. Create `journals/.tmp-<index>` as `uid:gid 0600`. `fallocate` it, zero-fill it, `fsync` it, and check it with FIEMAP.
3. `rename` it over `journals/j<index>.journal`, then `fsync journals/`.
4. Rewrite `PROVISION` with the new inode (§13.3), in the same session.
5. Verify.

**Evidence lost.** An archived file loses nothing, because the archive holds the bytes. An AbandonedClaim loses a header that never became valid; in domain H no record was ever written to it.

**Old descriptors.** A store-uid process may still hold a descriptor on the old, now unlinked inode. That is harmless: the inode is no longer in the pool.

**Resumption after a crash between step 3 and the end of step 4** (R2). Opening then refuses as **Lost**: the pool name holds an inode that `PROVISION` does not record. The Owner resumes in a new session:
1. remove leftover temporaries (§13.1);
2. the in-session verification must refuse with exactly that cause, for that index;
3. the file now at the pool name, read through the session's journal lock, must be `uid:gid 0600`, have link count 1 and the pool-file size, and be all zero;
4. the saved report of the interrupted session must show the old file archived and pending recycling, or AbandonedClaim;
5. then rewrite `PROVISION` with the inode now at the pool name (§13.3) and verify.

Anything else is not a resumption, and the store stays refused [M: C12].

### 13.9 Retirement

**Preconditions,** for a new `retired-through` of K, evaluated over the **complete** incident set of the in-session report, never its listed detail (§11.2):
- every claim at or below K is either archived as history (`j-`, §13.5) or a claim gap that is history (applied, with a valid disposition);
- no pool file holds a claim at or below K.

**Steps:**
1. Begin a session.
2. Rewrite `PROVISION` with `retired-through=K` (§13.3).
3. Verify.

**After retirement:**
- archive entries with claims at or below K remain, and are checked by name and type only;
- claim gaps at or below K are no longer computed;
- a revocation of their dispositions no longer re-opens them.

**Retirement can never exclude unresolved, undispositioned history.** The preconditions are verified over the complete set, and a pool claim at or below K refuses as Invalid [M: C12, C24].

### 13.10 Incident-limit exhaustion

If the current incidents exceed `incident_limit`, startup **refuses** as Capacity. It reports the exact count, and the incidents in detail up to the report bound, saying when the list is partial (§11.2). It never relabels an incident as resolved, and never decides from a truncated set.

The only resolutions are acts of the Owner:

| Resolution | Effect |
|---|---|
| Publish a disposition for each incident, then archive the dispositioned journals and pool files | Archived history (§13.5 b) leaves the current set. No core counts such dispositions in `RunStarted.dispositioned`; this is disclosed, and the archive and disposition files are the record. |
| Provision a successor (§13.6) | — |
| A future API or limit change (API-2, a gate) | — |

### 13.11 Verification: standalone verifier and in-session verification [D-8]

Both are future modes of the owner binary, implemented in the later mission. They run the same checks and differ only in their authority.

**The standalone verifier**, outside maintenance:
- **Locking.** It selects and revalidates `PROVISION` (§10.5) and takes `LOCK_SH | LOCK_NB` on a new description of the store lock. If an owner or a maintenance session holds the lock, it reports **Busy**. Busy is never success: it has its own exit status and report, and nothing is verified [M: C20, NC20d].
- **It writes nothing and syncs nothing**, so it reports the kernel-visible state, not the durable state (§4.3). It never activates (§10.6), and its report says that what it read may not be durable.

**In-session verification**, inside a maintenance session:
- **Activation.** The session activated at its beginning (§13.1, §10.6), so in-session verification reads a durable namespace.
- **Authority.** The session object itself: the retained `LOCK_EX` description on the inode that the selected root's `LOCK` entry currently names, checked by `fstat` of that description against a fresh `fstatat`, followed by revalidation of the session's selection (§10.5). Not a path, string, PID, descriptor number or caller assertion. A session object without that retained lock is refused as NotAuthorized [M: C20, NC20e].
- **Locking.** No lock request on the store lock at all: no new shared lock (Busy, NC20a), no conversion of the session's exclusive lock (NC20b), and no release around the verification (NC20c). Pool files the session already locked are read through its own descriptions; others are opened and locked `LOCK_EX | LOCK_NB` for the scan, and Busy means Live.
- **Never skipped.** Every verification scans afresh; an earlier report is never reused for verify-after [M: C20, NC20f].
- **No race with mutation.** Verification and the procedure's steps run sequentially in the session's one process, and competitors stay excluded throughout [M: C20].

**Checks**, in both:
- §10.1;
- §10.2 steps 0–2 and 4–8, without the evidence-preservation sync. Step 2 includes (R4) the effective profile, the journal's location and the kernel (§6.4 steps 9–11) [M: C27];
- all of §11.

Each reports every condition it finds rather than stopping at the first.

- **Report.** The store state; the selection's revision; each file's class and report (§11.2); the current incidents and their bindings, with disposition status, bounded and marked partial as §11.2 requires; history; stale and orphan dispositions; pool usage; leftover temporary entries; the `PROVISION` facts.
- **Exit status.** 0 only if the store opens and no current incident blocks. Busy, a partial report and every refusal are non-zero.
- **Durability of its inputs.** `PROVISION` and dispositions are guaranteed durable only once the order in §13 completes: write, `fsync`, publish, `fsync` the directory. Before that they may or may not be durable (§4.8). Neither mode reports what it reads as durable.

### 13.12 Dependency audit (R3)

For each procedure the table shows:
- what a later decision depends on;
- what is guaranteed durable before anything depends on it;
- what may stay merely visible;
- which views a later recovery may see;
- why each view keeps the history or refuses.

"Before anything depends on it" means before an owner's claim or a session's decision, both of which activate first (§10.6). Not every rollback is unsafe: a rollback that ends in a refusal, with the history still in the selected root, is a safe conservative refusal. What must never happen is a rollback that silently drops history.

| Procedure | A later decision depends on | Guaranteed durable before use | May stay merely visible | Permitted views after a crash | Why each is safe |
|---|---|---|---|---|---|
| Initial provisioning (§13.2) | `PROVISION`'s content and entry; the root, `LOCK`, the store's directories, the pool's names and zeroed contents | Steps 2 to 4 sync every store directory, `LOCK` and pool file with their entries; step 5 syncs `PROVISION`'s content before the rename | The `PROVISION` entry, until step 5's directory sync or the first activation | Unprovisioned, or the fresh store | No owner claims before activation makes the entry durable, so Unprovisioned only ever follows a root with no claim. The recovery re-publishes, and never replaces, a root with a non-zero pool byte. |
| Rewrite of `PROVISION`: recycling, retirement, re-qualification (§13.3) | The new `PROVISION` | Its content, written and synced before the rename | The new entry, until the procedure's sync or the next activation | The old revision or the new one | Before an activation both revisions name the same root, so the history is in the same pool and archive. A rolled-back recycling refuses as Lost, and a rolled-back re-qualification as Unsupported. A rolled-back retirement leaves a lower bound, which only adds checking. After an activation, the new revision cannot roll back. |
| Succession (§13.6) | `PROVISION` B, and B's store | B's directories, `LOCK` and pool (2b), the predecessor's copy (2a), and B's `PROVISION` content, all before the rename | The B entry, until 2c's sync or the first activation of B | A or B | No owner claims on B until activation makes B durable, and after that A cannot return. Work on A before the succession is in A's history, whichever store is then selected. |
| Disposition publication (§13.4) | The disposition's name and content | Its content, before the link | The name, until step 6's sync or the next activation | Blocks or published | An owner applies a disposition only after activation has made its name durable. A rollback before that only blocks again: a conservative refusal, with every generation in the same root. |
| Revocation (§13.4) | The name's removal | — | The rename, until step 3's syncs or the next activation | Published or blocks; also Invalid under the per-directory over-approximation | A decision that saw the revocation activated first, so the revocation was durable. A decision that saw the disposition still in force decided as before the revocation. Invalid is a refusal. |
| Archival (§13.7) | The archive entry, read as history | The copy's content, synced and re-read before the link | The names, until step 5's sync or the next activation | Published (the journal stays in the pool, with its disposition) or archived | In either view the generation is in the root: in the archive, or in the pool with its disposition |
| Recycling (§13.8) | The new pool file at the old name, and the new inode in `PROVISION` | The new file's zero content; its name, by step 3's sync of `journals/`; then the rewrite | The rewritten `PROVISION`, as for any rewrite | Archived, Lost, or recycled | Lost is a refusal with the archive intact. Resumption needs an all-zero file, so a pool file that holds a generation is never recycled over. |
| Retirement (§13.9) | `retired-through`, and the archive entries its preconditions read | The archive entries, which the session's activation made durable before verify-before | The rewritten `PROVISION` | The old bound or the new one | An older bound only checks more. A newer bound was decided from durable archive entries, so no claim it retires can lose its archive copy in a later rollback [M: C26, chained administration]. |

Retained unchanged:
- in-session verification under the session's exclusive lock (§13.11);
- post-lock revalidation (§10.5);
- both stores' exclusion during succession (§13.6);
- no automatic maintenance;
- no native ownership reconstructed from records (§14.2);
- content-bound dispositions (§12.3);
- decisions from the complete incident set, never from a truncated report (§11.2).

[M: C20–C22, C24–C26]

---

## 14. Native custody, responsiveness and retention

### 14.1 Responsiveness

- **The owner never waits on storage during a run.** `submit` is a short copy, and applying durability is a non-blocking read of the exchange.
- **The control thread** serves status, cancellation and the lease through `Control` (`core.rs:522-548`, `core.rs:616-623`) [S]. It adds a copy of the recorder status: what is pending and since when, `durable_through`, and the fatal cause with P, distinguishing a stall from a fatal condition (§9.6). No path waits on storage or holds a mutex across it.
- **Under blocked storage:**
  - in-flight native operations complete;
  - cleanup runs;
  - owners stay held;
  - admission waits for acknowledgements (§9.6), or, once a fatal cause is latched, is refused at once (§9.8);
  - `close()` refuses with `EvidencePending`;
  - the process does not exit.
- **Before the claim (R3).** A blocked activation sync (§10.6) holds the owner before its claim. It then serves no request and holds no custody, so nothing waits on it.

### 14.2 Evidence stays data

- **No locators.** Records carry identities, instants, kinds and outcomes only (`codec.rs:40-73`) [S]: no PID, unit, cgroup, path or socket. Restart reconstructs no ownership, and decoding returns plain data (`codec.rs:89-93`) [S].
- **Settlement is a past statement.** A stored `ActionSettled{Confirmed}` or `IncidentSettled` is a past statement by a process that no longer exists. It is not fresh proof of cleanup.
- **A disposition** means "accepted without cleanup confirmation".
- **A seal**, in domain H, means "closed as recorded".

### 14.3 Failure retention

- The fatal latch is never cleared, and a poisoned journal or exchange stays poisoned.
- The core keeps `RecordFailed`, `EvidenceFailed{record, at}` and the closure refusal (`core.rs:2914-2916`) [S].
- The core's in-memory failure log (`core.rs:1105-1137`) [S] is not durable beyond what version-1 records carry (API-1).
- No runtime path alters or removes a durable record. Nothing offers a force-close, drops an owner or clears a failure.

### 14.4 Residuals outside custody

The store cannot know about two kinds of owner, and INV-3 does not cover them:
- an owner that a native adapter produces after `close()`;
- an owner the core returned as `CustodyFull` (`core.rs:246-251`) [S], which the integration must keep (`mod.rs:164-167`) [S].

Both need native-layer guarantees. They are integration gates (G-NATIVE, §18).

---

## 15. Failure-point tables

### 15.1 Runtime

Notation:
- "AS" means the visible prefix holds at least one `ActionStarted`.
- "F1→F2" means process death, then restart, then power loss before any successful sync of the item in question.

| Point | After F1 (restart reads **K**) | After F2 | After F1→F2 | Startup concludes | New execution |
|---|---|---|---|---|---|
| Claim: header written, not synced | Header visible | Absent (Unused), torn (AbandonedClaim) or present | As after F2 | Unused, AbandonedClaim, or unsealed with no native work | Permitted. A later claim may leave a claim gap, which refuses (a disclosed false positive). |
| Claim durable, no record yet | Header | Header | Header | Unsealed, no native work | Permitted |
| Record *n* written, not synced | *n* visible; made durable by the startup preservation sync if that succeeds | *n* absent, present, or torn (Malformed) | Absent unless the sync succeeded | The prefix with or without *n* | Refused if AS, or if torn |
| Record *n* durable, not yet applied | *n* | *n* | *n* | Over-approximation | Refused if AS |
| Record *n* applied (the core may have admitted) | *n* | *n* | *n* | INV-2 | Refused if AS |
| Writer sync fails (latched); process alive | — | — | — | Store Busy | Refused |
| … then the process is destroyed | *n* may be visible though it failed; a new description's sync returns 0 (§4.3) | *n* absent | Absent | Unsealed | Refused if AS |
| Recorder panic | Locks held by `StoreGuard` | — | — | Busy while the process lives | Refused |
| Any fatal cause latched (R2); process alive | — | — | — | Busy while the process lives; admission refused at once (§9.8) | Refused |
| A cause latched while record *n* is between W1 and W6 (R2) | *n* may be visible; it is never acknowledged | *n* absent, present, or torn (Malformed) | Absent unless the startup sync made it durable | The prefix with or without *n* | Refused if AS, or if torn |
| A cause latched with nothing held; `close()` returned `Ok`; seal withheld (R2); process exited | No seal | No seal | No seal | Unsealed | Refused if AS (a disclosed false positive) |
| Activation incomplete or refused (R3): a sync failed, blocked or was interrupted, the probe failed, or revalidation failed | No claim | No claim | No claim | The store as it then reads; nothing of this owner | Permitted to a later owner that activates; this owner made no claim |
| Activated; claim and dependent records durable; then F2 (R3) | The activated selection | The activated selection: activation made it durable | The activated selection | The generation, unsealed if AS | Refused if AS [M: C25, C26] |
| Activated; then the journal aborts, a later error (R4) | The activated selection; the journal stays aborted, so the next activation's probe sees emergency read-only | The activated selection: its transactions had committed and are replayed | The activated selection | After F1, Unreliable, a refusal that keeps the bytes; after F2, the generation | Refused [M: C28] |
| The effective profile changes, or the kernel is updated (R4) | — | — | — | Unsupported until re-qualification (§13.3) | Refused [M: C27] |
| A checkpoint flush the device fails, then F2 (domain S, R4) | — | Committed metadata, an activated selection's included, can revert, and nothing reports it | As after F2 | Possibly a predecessor, without the work | Outside every guarantee (A-S5, §12.2) |
| `RunEnded` applied, not closed | Prefix with `RunEnded` | Same | Same | Unsealed; verdict reported | Refused if AS |
| `close()` succeeded, seal not written | No seal | No seal | No seal | Unsealed | Refused if AS (a disclosed false positive) |
| Seal written, not synced | Seal visible; made durable by the startup sync if that succeeds | Absent, unreadable or present | Absent unless synced | Sealed or unsealed | Permitted, or refused if AS |
| Seal write failed (EIO); process exited | Seal visible, and true in domain H | Absent | Absent | Sealed until page reclaim or inode eviction removes it, then unsealed | Permitted; refused if AS once the seal is gone (a disclosed false positive) |
| Seal durable | Sealed | Sealed | Sealed | Sealed | Permitted |
| Late owner delivered after `RunEnded`; `IncidentOpened` not durable | Possibly visible | Absent | Absent | Unsealed, AS, recorded unsettled 0, native work possible | **Refused** [M: C09] |

### 15.2 Administrative (R2: recomputed)

R1's table was written from the guaranteed-only model of §4.8 and so understated what F2 can leave. R2's table is **computed by the model** and checked against this text in both directions [M: C22; reference check].

- **Crash point *k*** means after operation *k* of the procedure in §15.3; 0 is before the first.
- **After F1**: the session's process dies; the next opening reads the visible state.
- **After F2, ordered**: power loss under every ordered schedule of §4.8 (A-M1), with pending data old and new.
- **Added by the per-directory over-approximation**: outcomes that only the per-directory family produces. It does not describe ext4.
- Each procedure runs on a store with one pool file. Provisioning is also checked with three pool files for operation parity (§15.3).
- An outcome describes what the next opening reads: the visible state after F1, one schedule's state after F2. None claims durability. "fresh", "published" and the other non-refusing outcomes mean that the opening's checks pass on what it reads; until the procedure's last directory sync completes, a later F2 can still change them.

| Procedure | Crash points | After F1 | After F2, ordered (A-M1) | Added by the per-directory over-approximation |
|---|---|---|---|---|
| P-PROV | 0–21 | unprovisioned | unprovisioned | — |
| P-PROV | 22 | fresh | fresh, unprovisioned | — |
| P-PROV | 23 | fresh | fresh | — |
| P-DISP | 0 | blocks | blocks | — |
| P-DISP | 1–4 | maintenance | blocks, maintenance | — |
| P-DISP | 5 | published | blocks, maintenance, published | — |
| P-DISP | 6 | published | published | — |
| P-REVOKE | 0 | published | published | — |
| P-REVOKE | 1 | blocks | blocks, published | invalid |
| P-REVOKE | 2 | blocks | blocks | invalid |
| P-REVOKE | 3 | blocks | blocks | — |
| P-ARCH | 0 | published | published | — |
| P-ARCH | 1–4 | maintenance | maintenance, published | — |
| P-ARCH | 5 | archived | archived, maintenance, published | — |
| P-ARCH | 6 | archived | archived | — |
| P-RECYCLE | 0 | archived | archived | — |
| P-RECYCLE | 1–3 | maintenance | archived, maintenance | — |
| P-RECYCLE | 4 | lost | archived, lost, maintenance | — |
| P-RECYCLE | 5–8 | lost | lost | — |
| P-RECYCLE | 9 | recycled | lost, recycled | — |
| P-RECYCLE | 10 | recycled | recycled | — |
| P-RETIRE | 0–3 | bound 0 | bound 0 | — |
| P-RETIRE | 4 | bound 1 | bound 0, bound 1 | — |
| P-RETIRE | 5 | bound 1 | bound 1 | — |
| P-REQUALIFY | 0–3 | unsupported | unsupported | — |
| P-REQUALIFY | 4 | revision 2 | revision 2, unsupported | — |
| P-REQUALIFY | 5 | revision 2 | revision 2 | — |
| P-SUCCESSOR | 0–27 | predecessor | predecessor | — |
| P-SUCCESSOR | 28 | successor | predecessor, successor | — |
| P-SUCCESSOR | 29 | successor | successor | — |

| Outcome | What the next opening reports |
|---|---|
| unprovisioned | Unprovisioned: no `PROVISION` |
| fresh | A fresh store: it opens, with no incident |
| blocks | The incident blocks: no disposition is in force for it |
| published | The disposition is in force: the incident is dispositioned |
| maintenance | MaintenanceIncomplete: a leftover temporary entry refuses |
| archived | The journal is archived history, pending recycling |
| lost | Lost: the pool name holds an inode that `PROVISION` does not record |
| recycled | The pool file is a fresh, Unused file recorded in `PROVISION` |
| invalid | Invalid: a revoked disposition under both names (link count 2) |
| bound *K* | `retired-through=K` is in force |
| revision *R* | `PROVISION` revision *R* is in force |
| unsupported | Unsupported: the mount options differ from `PROVISION` |
| predecessor, successor | The store `PROVISION` selects. The other is never selected. |

**Safety, at every crash point and under both families** [M: C12]:
- no partial file is accepted and no temporary entry is honoured;
- no incident is lost silently: each still blocks, is dispositioned by its complete final file, or is history;
- recycling never accepts a replaced pool file before `PROVISION` records it;
- retirement never hides an unresolved, undispositioned generation, and a retirement whose preconditions fail is refused;
- succession never changes the predecessor's journals, and the successor is never in force without the predecessor's `PROVISION` kept.

**Recovery** from each refusing outcome, without force:

| Outcome | Recovery |
|---|---|
| unprovisioned, after provisioning began | R3 (§13.2): never a fresh store over existing roots. A root with any non-zero pool byte is re-published; a root whose pool files are all zero may be removed, with any `<PROVISION_PATH>.tmp`, and provisioning repeated. R2's rule ("which holds no history") is withdrawn [M: C26]. |
| maintenance | A session removes the leftover temporaries (§13.1), then completes or repeats the procedure. Modelled for dispositions, archival and recycling: each recovered state is one the procedure allows [M: C12]. |
| lost, during recycling | Resumption (§13.8) [M: C12] |
| invalid, after an interrupted revocation (over-approximation only) | The Owner unlinks the `dispositions/` name and syncs `dispositions/` (§13.4) |
| unsupported, before re-qualification is visible | Repeat the re-qualification (§13.3) |
| predecessor, before the successor's `PROVISION` is visible or durable | The predecessor stays in force. In a session on it, the Owner removes the leftover temporaries and repeats §13.6. The incomplete successor root may be removed only if all its pool files are zero; under activation (§10.6) no owner claimed on it. Step 2a is skipped if `<PROVISION_PATH>.predecessor-<old root id>` already holds exactly the current `PROVISION`'s bytes; any other existing copy aborts the repeat. |

**Coverage.** Every crash point of every procedure, after F1, and after F2 under every schedule of both families with pending data old and new:

| Procedure | Operations | Crash points | Ordered schedules | Per-directory schedules | Outcomes checked | Recoveries checked |
|---|---|---|---|---|---|---|
| P-PROV | 23 | 24 | 54 | 103 | 338 | — |
| P-DISP | 6 | 7 | 15 | 15 | 67 | 32 |
| P-REVOKE | 3 | 4 | 5 | 8 | 30 | — |
| P-ARCH | 6 | 7 | 15 | 15 | 67 | 32 |
| P-RECYCLE | 10 | 11 | 21 | 23 | 99 | 66 |
| P-RETIRE | 5 | 6 | 11 | 12 | 52 | — |
| P-REQUALIFY | 5 | 6 | 11 | 12 | 52 | — |
| P-SUCCESSOR | 29 | 30 | 68 | 117 | 400 | — |

A retirement whose preconditions fail is also checked to be refused.

**Representative schedules.**
- **A disposition crashed after the unlink, before `fsync dispositions/` (P-DISP, point 5).** The pending log is: create `.tmp-<binding>`, link `<binding>.disposition`, unlink `.tmp-<binding>`. The four ordered prefixes give blocks, maintenance, maintenance and published. R1 allowed only "nothing published".
- **A revocation crashed after the rename (P-REVOKE, point 1).** Ordered: the rename persisted or not, giving blocks or published. Per-directory: the removal from `dispositions/` and the addition to `revoked/` persist independently, and the addition alone gives invalid.
- **A recycling crashed after the rename, before `fsync journals/` (P-RECYCLE, point 4).** Ordered prefixes give archived, maintenance and lost; the per-directory family adds a split rename, which leaves the old file in place and gives archived again.

### 15.3 Operations: protocol and model parity (R2)

Every durability operation of every procedure, in order. The model performs exactly these operations, with these step labels, and no others; each crash point of §15.2 follows one of them [M: C23]. R1's model synced the state root when it created `LOCK` without the text saying so. That step is now §13.2 step 3, and the parity check finds no other hidden operation.

Object names: `parent` is the directory holding `<STATE_ROOT>`, and `parent/root` is `<STATE_ROOT>`; `root/LOCK`, `root/journals` and so on are its entries; `provdir` is the directory of `<PROVISION_PATH>`; `provdir/PROVISION` is `<PROVISION_PATH>`; `provdir/tmp` is `<PROVISION_PATH>.tmp`, or `<PROVISION_PATH>.predecessor.tmp` in step 13.6/2a; `provdir/predecessor` is `<PROVISION_PATH>.predecessor-<old root id>`; `journals/pool` is `j<index>.journal`, and `journals/tmp` is `.tmp-<index>`; `dispositions/tmp` and `dispositions/final` are `.tmp-<binding>` and `<binding>.disposition`; `revoked/entry` is `<binding>-<timestamp>.disposition`; `archive/tmp` and `archive/final` are `.tmp-<name>` and the final name. A `create` of a pool file includes its `fallocate`; `write` writes the whole content; `fsync` syncs that file; `fsync_dir` syncs a directory.

For P-PROV and P-SUCCESSOR the pool operations (`create`, `write` and `fsync` of `journals/pool`) repeat for each pool file, in index order, before the one `fsync_dir` of `journals`. R-LEFTOVER and R-RESUME are the recoveries of §13.1 and §13.8, shown for a leftover disposition temporary and for a recycling interrupted after its rename. R-REPUBLISH (R3) is the recovery of §13.2 after Unprovisioned, shown with a leftover `<PROVISION_PATH>.tmp`; without one, its first operation is absent.

| Procedure | Operations | In order (label = section/step) |
|---|---|---|
| P-PROV | 23 | 1 mkdir `parent/root` (13.2/2); 2 mkdir `root/journals` (13.2/2); 3 mkdir `root/dispositions` (13.2/2); 4 mkdir `root/archive` (13.2/2); 5 mkdir `dispositions/revoked` (13.2/2); 6 fsync_dir `revoked` (13.2/2); 7 fsync_dir `dispositions` (13.2/2); 8 fsync_dir `journals` (13.2/2); 9 fsync_dir `archive` (13.2/2); 10 fsync_dir `root` (13.2/2); 11 fsync_dir `parent` (13.2/2); 12 create `root/LOCK` (13.2/3); 13 fsync `root/LOCK` (13.2/3); 14 fsync_dir `root` (13.2/3); 15 create `journals/pool` (13.2/4); 16 write `journals/pool` (13.2/4); 17 fsync `journals/pool` (13.2/4); 18 fsync_dir `journals` (13.2/4); 19 create `provdir/tmp` (13.2/5); 20 write `provdir/tmp` (13.2/5); 21 fsync `provdir/tmp` (13.2/5); 22 rename `provdir/PROVISION` (13.2/5); 23 fsync_dir `provdir` (13.2/5) |
| P-DISP | 6 | 1 create `dispositions/tmp` (13.4/3); 2 write `dispositions/tmp` (13.4/3); 3 fsync `dispositions/tmp` (13.4/3); 4 link `dispositions/final` (13.4/4); 5 unlink `dispositions/tmp` (13.4/5); 6 fsync_dir `dispositions` (13.4/6) |
| P-REVOKE | 3 | 1 rename `revoked/entry` (13.4-revoke/2); 2 fsync_dir `revoked` (13.4-revoke/3); 3 fsync_dir `dispositions` (13.4-revoke/3) |
| P-ARCH | 6 | 1 create `archive/tmp` (13.7/3); 2 write `archive/tmp` (13.7/3); 3 fsync `archive/tmp` (13.7/3); 4 link `archive/final` (13.7/4); 5 unlink `archive/tmp` (13.7/5); 6 fsync_dir `archive` (13.7/5) |
| P-RECYCLE | 10 | 1 create `journals/tmp` (13.8/2); 2 write `journals/tmp` (13.8/2); 3 fsync `journals/tmp` (13.8/2); 4 rename `journals/pool` (13.8/3); 5 fsync_dir `journals` (13.8/3); 6 create `provdir/tmp` (13.3/2); 7 write `provdir/tmp` (13.3/2); 8 fsync `provdir/tmp` (13.3/2); 9 rename `provdir/PROVISION` (13.3/3); 10 fsync_dir `provdir` (13.3/3) |
| P-RETIRE | 5 | 1 create `provdir/tmp` (13.3/2); 2 write `provdir/tmp` (13.3/2); 3 fsync `provdir/tmp` (13.3/2); 4 rename `provdir/PROVISION` (13.3/3); 5 fsync_dir `provdir` (13.3/3) |
| P-REQUALIFY | 5 | 1 create `provdir/tmp` (13.3/2); 2 write `provdir/tmp` (13.3/2); 3 fsync `provdir/tmp` (13.3/2); 4 rename `provdir/PROVISION` (13.3/3); 5 fsync_dir `provdir` (13.3/3) |
| P-SUCCESSOR | 29 | 1 create `provdir/tmp` (13.6/2a); 2 write `provdir/tmp` (13.6/2a); 3 fsync `provdir/tmp` (13.6/2a); 4 link `provdir/predecessor` (13.6/2a); 5 unlink `provdir/tmp` (13.6/2a); 6 fsync_dir `provdir` (13.6/2a); 7 mkdir `parent/root` (13.6/2b); 8 mkdir `root/journals` (13.6/2b); 9 mkdir `root/dispositions` (13.6/2b); 10 mkdir `root/archive` (13.6/2b); 11 mkdir `dispositions/revoked` (13.6/2b); 12 fsync_dir `revoked` (13.6/2b); 13 fsync_dir `dispositions` (13.6/2b); 14 fsync_dir `journals` (13.6/2b); 15 fsync_dir `archive` (13.6/2b); 16 fsync_dir `root` (13.6/2b); 17 fsync_dir `parent` (13.6/2b); 18 create `root/LOCK` (13.6/2b); 19 fsync `root/LOCK` (13.6/2b); 20 fsync_dir `root` (13.6/2b); 21 create `journals/pool` (13.6/2b); 22 write `journals/pool` (13.6/2b); 23 fsync `journals/pool` (13.6/2b); 24 fsync_dir `journals` (13.6/2b); 25 create `provdir/tmp` (13.6/2c); 26 write `provdir/tmp` (13.6/2c); 27 fsync `provdir/tmp` (13.6/2c); 28 rename `provdir/PROVISION` (13.6/2c); 29 fsync_dir `provdir` (13.6/2c) |
| R-LEFTOVER | 2 | 1 unlink `dispositions/tmp` (13.1/recover); 2 fsync_dir `dispositions` (13.1/recover) |
| R-RESUME | 5 | 1 create `provdir/tmp` (13.3/2); 2 write `provdir/tmp` (13.3/2); 3 fsync `provdir/tmp` (13.3/2); 4 rename `provdir/PROVISION` (13.3/3); 5 fsync_dir `provdir` (13.3/3) |
| R-REPUBLISH | 6 | 1 unlink `provdir/tmp` (13.2/recover); 2 create `provdir/tmp` (13.2/recover); 3 write `provdir/tmp` (13.2/recover); 4 fsync `provdir/tmp` (13.2/recover); 5 rename `provdir/PROVISION` (13.2/recover); 6 fsync_dir `provdir` (13.2/recover) |

### 15.4 Composed recovery (R3)

§15.2 checks each procedure's crash points in isolation, so R2 passed it. The R3 checks compose: an administrative operation prefix, a first crash, an ordinary owner startup with activation, a claim, `RunStarted`, `CaseStarted` and `ActionStarted` made durable and acknowledged, an admission through the store gate and a native start, a second crash, and then a fresh startup and the specified recovery [M: C26].

**The oracle.** It sees every root of the fixture. It tracks each generation that acquired an acknowledged `ActionStarted` and was never sealed. After every second crash it requires one of:
- the fresh owner's decision includes that generation; or
- the opening refuses, and the specified recovery (§13.1, §13.2, §13.8) then either brings the generation into the decision or ends in a refusal that keeps its bytes.

A fresh owner that is Ready, or that decides on other incidents, without the generation is a failure. The implementation under test gains no global scan from the oracle's view: it reads only the selected root.

**Dimensions**, each with two pool files so that an owner can claim after every procedure:

| Quantity | Count |
|---|---|
| Administrative prefixes: every operation of every procedure | 101 |
| First crashes: F1, or F2 under every distinct schedule of both families with data old and new | 1135 |
| Runs in which an owner then activated, claimed and acknowledged an `ActionStarted` | 566 |
| Second crashes after that work: F1, or F2 under every distinct schedule with data old and new | 1860 |
| Chained administration: an archival at every point, then a recycling and a retirement, then F1 or F2 | 21 |
| Positive path: a generation dispositioned in a session, then F1 or F2 | 3 |

**Outcomes.**
- Every second crash ended with the generation in the selected decision: PriorUnresolved, the generation blocking.
- The startups that refused before any dependent work were safe refusals. They were Unprovisioned, MaintenanceIncomplete, Lost, PriorUnresolved, Unsupported or Invalid, each before a claim, so nothing depended on them.
- The chained administration kept every retired claim's archive copy, and kept every unretired claim in the decision.
- The dispositioned generation made the fresh owner Ready, with the generation dispositioned.
- A root left unpublished with history was re-published, never replaced.

**Representative traces:**
- **Failing, the R2 protocol** (no activation; NC-ACT-COMPOSE and NC-ACT-VISIBLE).
  - P-SUCCESSOR, first crash F1 after the rename of B's `PROVISION`. An owner selects B, claims, and acknowledges an `ActionStarted`.
  - Second crash F2 under the ordered cut before the rename. The fresh owner is Ready on A, and B's generation is outside the decision.
  - The same with P-PROV: the fresh owner finds no `PROVISION`. R2's recovery then provisions a fresh root, which is Ready without the generation.
- **Passing, R3.** The same prefixes: activation's sync of the `PROVISION` directory commits the rename before the claim. After every second crash the fresh owner is PriorUnresolved on B, with the generation in the decision.
- **Failing without session activation (chained).** An archival is interrupted after its link, then recycling and retirement run. F2 under a per-directory schedule keeps the retirement and the recycled pool file but loses the archive link. The fresh owner is Ready with claim 1 retired and its journal gone. Under A-M1 this does not occur; with session activation it does not occur under either family.

**Bounds.** One generation of dependent work per run, one pool file claimed, every prefix of every procedure, and both crash families. The composed checks are a bounded enumeration, not a proof.

**R4.** Every composed scenario was re-run on the R4 model, with the corrected `fsync` model and the profile checks, on the qualified profile. Every count and outcome above is identical [M: C26].

---

## 16. Verification: the design model now, Rust tests later

### 16.1 Design model (this mission) [M]

`docs/evidence/p2-v1-r3b-i3-p-r4/design_checks.py` uses only the Python standard library, with in-memory byte images and simulated state. It opens no store, calls no native operation, performs no privileged I/O and causes no real crash. R4 carries the R3 model forward, which carried R2's and R1's, and implements:
- the §4 state model: **K**, **D**, pending writeback, errseq with per-description cursors, page reclaim separate from inode eviction, F1, F2, and the §4.8 metadata log with its ordered and per-directory schedules;
- the journal behaviour of A-M2 (R3), as corrected in R4:
  - a directory sync through a new descriptor;
  - a read-only superblock;
  - a commit that fails silently in the commit thread;
  - emergency read-only;
  - the timestamp probe, whose transaction runs or has completed, so that the probe's `fsync` waits only for a running one (F4);
  - an abort that lands between ext4's and jbd2's handle tests;
- (R4) the effective profile (§6.4 steps 9–11): the option listing, the jbd2 entry names and the kernel identity, as fixtures in the form v6.17 prints them, and the Owner's qualification as root;
- (R4) the journal transaction model behind §10.7, `JournalSim`. It is the smallest state the proof needs:
  - transactions running, committing or completed, then committed or failed;
  - the abort and emergency read-only flags, and the probe inode's sync tid;
  - each flush site, checked or discarded, on the internal or external journal and with synchronous or asynchronous commits;
  - checkpointing, the log tail, and recovery from the tail;
- the §5 containment profile, plus an unsupported profile for contrast;
- the §6–§7 formats, with a version-1 encoder and decoder that reproduce all 50 golden record vectors of `codec-tests` (`RECORD_VECTORS`, `codec-tests:121`) [S];
- the §8 lock model, with open-file-description semantics, and the §8.4 session objects;
- the §9 exchange, worker, owner apply step, failure delivery and store admission gate, against a ledger model of `core.rs:1042-1090` and a source-derived model of the core paths they depend on (start, case begin and end, reservation, admission, settlement, commitment, fail-stop and close);
- the §10 startup order, with an interaction log, the §10.5 selection and revalidation, and the §10.6 activation;
- the §11 classifier and grammar, and the §12 bindings;
- the §13 procedures inside maintenance sessions, with crash injection and recovery;
- an enumerator of interleavings over the model's atomic steps, where one step stands for one critical section;
- the composed scenarios and oracle of §15.4 (R3).

It first re-runs every Architect counterexample on the **unchanged R1 and R2 models**, and the R4 findings on the **unchanged R3 model**, each loaded by path and checked against its published SHA-256, and requires each to reproduce (§19.3, §19.4, §19.5).

| Check | Marker | Requirement |
|---|---|---|
| C00 | `[golden-codec]` | The model's frames are version-1 frames |
| C01 | `[F1-volatile]` | Process death keeps visible and durable state apart |
| C02 | `[F1-F2-sync]` | F1, restart, F2 differs with and without an actual sync |
| C03 | `[errseq-reopen]` | A sync failure is not repaired by reopening |
| C04 | `[tear-containment]` | Shared-region tear hazards and the supported profile |
| C05 | `[lock-retained]` | Worker failure keeps exclusion |
| C06 | `[busy-before-sync]` | A competing writer is refused before any startup sync or read |
| C07 | `[dup-bounded]` | Duplicate delivery cannot block the owner or grow state |
| C08 | `[fatal-latched]` | A full or stalled consumer cannot hide a fatal recorder condition |
| C09 | `[late-uncertain]` | A non-durable late incident yields uncertainty and refusal |
| C10 | `[exact-bytes]` | Exact header, seal and record-block consumption |
| C11 | `[archive-binding]` | Archival keeps bindings; byte changes change them explicitly |
| C12 | `[admin-crash]` | No administrative crash point, under either schedule family, installs partial state, loses an incident or retires unresolved history; every refusing outcome of dispositions, archival and recycling recovers |
| C13 | `[grammar-conformance]` | Fixtures, prefixes and mutations against §11.4 |
| C14 | `[safe-open]` | Special files, ext4 identity, mount ambiguity |
| C15 | `[ack-durable]` | INV-1 and INV-4 under write and sync faults |
| C16 | `[no-false-resolution]` | INV-3 at every crash point of every fixture |
| C17 | `[arith-bounds]` | Claim exhaustion and numeric bounds |
| C18 | `[fatal-total]` | Every fatal cause: worker loss with no record; loss between reservation and admission; a conflict on an acknowledged record; zero, out-of-range, foreign and future submissions; a conflict beyond the next record; an append in flight; an unexpected acknowledgement outcome; a poisoned mutex; loss before the claim, after the commitment and during the seal; failure with no native ownership left. Admission is refused at once, true acknowledgements stay, and the failure lands only at an issued record. |
| C19 | `[admission-fence]` | Every interleaving of an admission with a latch, and of the worker's publication with a latch: nothing is admitted or published after the latch |
| C20 | `[session-verify]` | Verification inside a session: no lock request, competitors excluded throughout, nested procedures in one lock lifetime, authority only from the retained lock |
| C21 | `[provision-selection]` | A `PROVISION` replaced between read and lock never regains authority: succession, retirement, re-qualification and recycling; bounded re-selection |
| C22 | `[metadata-schedules]` | Entries durable before a directory sync, a split rename only in the over-approximation, page reclaim with open descriptors, and the computed crash matrix equal to §15.2 |
| C23 | `[step-parity]` | Every modelled durability operation is a §15.3 operation, in order, one crash point each, for every procedure and both recoveries |
| C24 | `[aggregate-bounds]` | Directory counts refused before any entry is examined; a partial report says so; neither a startup nor a retirement decides from a truncated set |
| C25 | `[durable-activation]` | No claim on a selection that is not durable. Covers the two witnesses with every second crash; established stores and completed successions; EIO, EINTR, emergency and read-only states, a silent journal abort, identity failures, and process death or power loss during activation, each refused before any claim; the read-only verifier never activating; sessions activating; one filesystem; stale readers and competing owners; a replacement racing activation; and the activation operations of §10.6 |
| C26 | `[composed-recovery]` | §15.4: every procedure prefix, a first crash, dependent work, a second crash and recovery, with the generation always discoverable or explicitly refused; chained administration; recovery that never provisions a fresh root over history |
| C27 | `[supported-profile]` | INV-20. The qualified profile activates and claims. Each case below refuses before any claim, for owners, the verifier and sessions: an external journal; `data=writeback`, `nobarrier` or `data=journal` in effect but absent from `mountinfo`; `journal_async_commit`; `norecovery`; `fc_debug_force`; emergency or ordinary read-only; another kernel; an unresolved name; an unreadable listing; a contradiction. Also covered: a profile changed during activation, refused at A4; qualification refusing fast commits and an external journal; a kernel update refused until re-qualification, which never accepts an unsupported profile. |
| C28 | `[activation-proof]` | §10.7, safety. In every enumerated transaction state and window, a certified activation has every dependency committed, and it survives every later continuation of domain H and a power loss. A refusal happens only after a failure. Harmless late aborts are certified. Four windows are also run through the whole model, with every second crash and the oracle. |
| C29 | `[journal-conformance]` | §10.7, conformance. Each modelled transition follows its cited branch: the three `fsync` returns; an abort before the probe's test returned as EROFS; the completed-transaction return reached with the journal aborted. Each discarded flush (`checkpoint.c:338-339`, `commit.c:775-778`, `commit.c:883-886`) leaves the journal unaborted and loses a committed operation. The two `commit.c` sites never run on the profile. A-S5 is shown necessary, and the checked failures abort. |

**Safety and conformance are kept apart.** C28 asks whether the protocol is safe on the modelled journal. C29 asks whether the modelled journal is the cited one. A model that wrongly reports more errors (R3's `fsync`, NC-FSYNC-COMPLETED) passes C28 and fails C29. A model that invents an abort where v6.17 discards the status (NC-ERR-DETECTED) would hide the need for A-S5, and fails C29.

**Negative controls.** There are 61: R1's 27 (NC01–NC17, with lettered variants), R2's 21 (NC18a–f, NC19, NC20a–f, NC21a–b, NC22a–b, NC23, NC24a–c) and R3's 6 (NC-ACT-VISIBLE, NC-ACT-ORDER, NC-ACT-ERROR, NC-ACT-PROBE, NC-ACT-REVALIDATE, NC-ACT-COMPOSE), all unchanged in identity and intent, and 7 new ones:

| Control | Target | Behaviour restored |
|---|---|---|
| NC-PROF-JOURNAL | C27 | An external journal accepted: the journal's location never checked |
| NC-PROF-MODE | C27 | An excluded mode in the effective listing accepted |
| NC-ACT-UNQUALIFIED | C27 | An unqualified profile authorizes: the profile taken from the pinned `mountinfo` strings and the attestation, the kernel never compared (R3) |
| NC-PROF-GUARD | C27 | The effective profile not checked again after the syncs (A4) |
| NC-PROOF-ORDER | C28 | The probe before the directory syncs |
| NC-FSYNC-COMPLETED | C29 | R3's `fsync` model: every aborted, unnoticed journal reported as EIO, whatever the state of the inode's transaction |
| NC-ERR-DETECTED | C29 | A discarded flush status treated as detected: an invented abort at an unchecked flush |

- They are listed with their targets in the script and in `coverage.json`.
- Each is an in-memory mutant that restores an incorrect behaviour, usually an earlier revision's, and must fail its target check's assertion carrying that check's marker.
- NC-ACT-COMPOSE restores the whole R2 protocol. It counts as caught only if the earlier snapshot checks C12, C20, C21 and C22 still pass under it: it must be detected by composition alone.
- A syntax, import or fixture error never counts as a caught control.

The output reports separately the R1 and R2 reproductions, the R3 reproductions, baseline passes, intended negative-control failures, the restored baseline, tool failures, the assertion and model changes (with the requirement each keeps), and the assumptions the model cannot establish.

**Model simplifications,** each disclosed:
- directory descriptors and component-by-component `O_PATH` walks are not modelled; directories are addressed by reference;
- mutex discipline (no mutex across I/O, lock order) is a property for code review. The model's exchange performs no I/O by construction, and each critical section is one atomic step;
- the per-directory schedules are an over-approximation, not ext4 behaviour; the ordered schedules rest on A-M1;
- the core is represented by the source-derived fixtures, a model of `Ledger::acknowledge` and `Ledger::fail`, and a source-derived model of the paths the fatal protocol depends on; the real core is not executed;
- the model's verifier stops at the first refusal, whereas the §13.11 contract requires a full report;
- the maintenance session is modelled as an object holding its lock descriptions; no executable exists;
- the journal is one per simulated host, standing for the one filesystem the profile requires (§6.2); the per-directory family over-approximates independent directories, and so independent filesystems;
- the journal's behaviour follows the v6.17 sources cited for A-M2; it is not a kernel;
- (R4) the whole-store model's abort leaves nothing running: it stands for the state after the commit thread has processed the running transaction that the abort itself requests (`journal.c:2565-2589`). `JournalSim` keeps the transaction states that the whole-store model abstracts away;
- (R4) `JournalSim` runs one commit at a time, in order. Background commits and aborts happen at chosen points, and every flush is honoured unless a scenario fails it. Its initial states have at most four events, with one background event or none before each activation step;
- (R4) the option listing, jbd2 names and kernel identity are fixtures; the superblock facts are an object only the qualification reads.

`reference_check.py` verifies against `45898e05` every Rust `file:line` reference in this document and the golden-vector literals in the model. With `--kernel`, it verifies every Linux `file:line` citation against local copies of the v6.17 files, each checked by SHA-256, at the first and last line of each range. It also checks that the following say exactly what the model computes and performs:
- the §15.2 crash matrix and coverage counts;
- the §15.3 operation table;
- the §10.6 activation operations;
- the §15.4 composed dimensions;
- (R4) the §10.7 enumeration counts;
- (R4) the §6.4 step 9 required and excluded options;
- (R4) the `PROVISION` keys of §7.5;
- the §16.1 check table and control count.

Existence and text matches are not semantic conformance. A successful line check is not a semantic proof.

**The model validates this specification's internal consistency and protocols. It does not execute, test or prove the Rust code, the kernel, ext4 or any device.**

### 16.2 Rust verification (later implementation mission)

- **Simulated storage.** A `StoreIo` that implements the §4 state model:
  - errseq, with per-description cursors sampled at open; page reclaim with descriptors open; inode eviction;
  - F1, F2, and F1→F2;
  - tears within the 4096-byte unit;
  - short writes, zero-progress writes and EINTR;
  - directory entries with the §4.8 metadata log, and F2 under both schedule families.
- **Crash-point enumeration** over the real core with stand-ins, covering:
  - the T1–T15 scenarios, and recording failure at every record;
  - duplicate submission after a sink panic;
  - priors with dispositions;
  - pool exhaustion, abandoned claims and claim gaps;
  - stalls, recorder panic and conflicts;
  - every fatal cause of §9.2 at every point of the claim, append and seal, including the F1 cases of §9.10.
- **Interleavings (R2).** A deterministic schedule harness built from explicit step hooks in the store's own code, with no new dependency, that runs the admission gate, the worker's W1–W6, the owner's apply step and the latch in every order, and asserts INV-4 and INV-13 in each.
- **Maintenance sessions (R2).** The session type against simulated storage only: in-session verification without any lock request, nested procedures, the competitor exclusion, `PROVISION` replacement between read and lock, and the §15.2 matrix recomputed from the Rust procedures.
- **Activation (R3).** Simulated storage must model the A-M2 journal behaviour: a directory sync through a new descriptor, a read-only superblock, a silent commit failure followed by emergency read-only, and the timestamp probe. The tests then cover:
  - the activation order of §10.6 and its refusals;
  - no claim before activation;
  - the composed scenarios of §15.4, rerun from the Rust procedures and the real core.
- **Profile and activation proof (R4).**
  - **Simulated storage** must also model the transaction windows of §10.7: running, committing and completed transactions; an abort before, inside and after the probe's handle start; the completed-transaction return; discarded and checked flushes; the log tail.
  - **The tests then cover:**
    - every profile case of C27, with the effective listing, the jbd2 names and the kernel identity supplied by the simulated host;
    - the windows of C28;
    - C29's conformance cases, written against the qualified kernel's sources.
- **Unprivileged real-filesystem tests (R3)**, under `CARGO_TARGET_TMPDIR`, may check that:
  - a directory opened `O_RDONLY | O_DIRECTORY` can be synced and an `O_PATH` one cannot;
  - `futimens` succeeds on a file the test owns;
  - `st_dev` comparison identifies one filesystem.
  They must not inject real I/O errors or power loss.

### 16.3 Invariants for the Rust tests

The Rust tests must hold INV-1 to INV-19 in their domains (§12.1), plus **I9, conformance**: every journal that the real deterministic core runs produce must classify as non-Malformed. Those runs include `Run::every_kind` in `codec-tests` and the h18 adversarial sequences (`core-tests:5237`) [S].

### 16.4 Negative controls for the Rust tests

Each must compile and then fail its intended assertion, with its marker:
- **Acknowledgement:** publish durable before the sync; retry after EIO.
- **Lock ownership:** a worker-owned lock description; a duplicated lock description with `LOCK_UN`.
- **Startup order:** sync or read before the non-blocking lock.
- **Exchange:** non-idempotent submission; failure delivered as a queued message.
- **Refusal:** the outstanding-only rule; "`RunEnded` means resolved".
- **Byte checks:** unchecked reserved bytes, seal tail or padding; a checksum-valid invalid header taken as an abandoned claim.
- **Bindings:** a location-bound binding; a binding that ignores content.
- **Maintenance:**
  - temporary disposition names read;
  - a `PROVISION` inode mismatch ignored;
  - retirement without its preconditions.
- **Filesystem:** ext4 decided by magic only; open without the type check.
- **Arithmetic:** claim wraparound.
- **Fatal protocol (R2):** admission without the store gate; a health check followed by an unprotected admission call; publication after the latch; failure targeted at the cause's own sequence; a future submission stored; an unexpected acknowledgement outcome ignored; `record_failed` for an unissued record counted as delivered.
- **Maintenance (R2):** in-session verification through a new shared lock; a lock converted or released around verification; Busy taken as success; authority from a flag; a verification skipped by reusing an earlier report; no post-lock revalidation; revalidation through the descriptor kept from the read; an undocumented durability step.
- **Persistence and bounds (R2):** entries durable only through a directory sync; no page reclaim with descriptors open; bounds checked after the entries are examined; a startup decision or retirement preconditions taken from a truncated report.
- **Activation (R3):** a revalidated, visible selection treated as activated; a claim, acknowledgement or admission before activation; a failed or uncertain activation sync treated as success; no certification probe; no revalidation around activation; the R2 protocol run through the composed tests while the snapshot tests still pass.
- **Profile and proof (R4):**
  - an external journal accepted;
  - an excluded mode in the effective listing accepted;
  - the profile decided from `mountinfo` strings, the attestation or a kernel version prefix;
  - the profile not rechecked at A4;
  - the probe before the directory syncs;
  - a completed transaction's `fsync` modelled as failing;
  - a discarded flush modelled as an abort.

### 16.5 Fixtures

Ordinary Rust tests use simulated storage only. Unprivileged real-filesystem tests under `CARGO_TARGET_TMPDIR` may check:
- safe-open refusals: a symbolic link, and a FIFO without blocking;
- hard-link detection;
- `flock` semantics, including `LOCK_UN` on a duplicate;
- write and sync wiring;
- mount identification of the fixture's own filesystem, asserting whichever outcome applies.

Root-owned fixtures, host qualification and power-loss rigs are not authorized.

---

## 17. Implementation envelope (proposed, for a later mission)

| Path (under `crates/nexus-verifier-sandbox/tests/`) | Content |
|---|---|
| `support/custody/store/mod.rs` | API (`StoreGuard`, `StartupReport`, `Recorder`, `ExchangeSink`, `RecorderStatus`), claims and non-claims |
| `support/custody/store/format.rs` | Header, seal and record blocks; `PROVISION`, disposition and entry-name grammars; bindings |
| `support/custody/store/classify.rs` | The pure classifier and grammar (§11) |
| `support/custody/store/open.rs` | Safe open, mount identification, the one-filesystem check, (R4) the effective-profile, journal-location and kernel checks of §6.4 steps 9–11, `PROVISION` selection and revalidation, durable activation, startup order (§10) |
| `support/custody/store/io.rs` | The `StoreIo` trait and its Linux implementation: `openat2` through `SYS_openat2` and `open_how`; `statx`; `fstatat`; `fstatvfs`; `flock`; `pwrite`; `fdatasync`; `fsync` of directories and files; `futimens`; `getrandom`; (R4) `readlinkat` and `uname`. All are in `libc` 0.2.183 (R4: `readlinkat` in its `src/unix/mod.rs`, line 2270, and `uname` in its `src/unix/linux_like/mod.rs`, line 2003). |
| `support/custody/store/exchange.rs` | The exchange and its fatal latch, the sink, the owner's apply step and failure delivery, and the store admission gate (§9) |
| `support/custody/store/recorder.rs` | The worker: claim, append, seal, poison |
| `support/custody/store/disposition.rs` | The `DispositionValidator` |
| `support/custody/store/sim.rs` | Simulated storage (§16.2), including the §4.8 metadata log and schedules, the A-M2 journal behaviour, (R4) the transaction windows of §10.7 and a simulated host profile |
| `support/custody/store/maintenance.rs` | The maintenance session type and in-session verification (§13.1, §13.11), exercised against simulated storage only. No executable, binary target or privileged entry point. |
| `support/custody/mod.rs` | `pub mod store;` and the integration-obligation documentation |
| `phase2_custody_store.rs` | A new test target |

**Unchanged:**
- `core.rs`, `model.rs` and `codec.rs`, and the NXCD version-1 bytes;
- production `src/` and the live harness;
- workflows, `Cargo.toml` and `Cargo.lock`.

**Dependencies.** None new: `libc` 0.2.183 (Linux), `sha2` 0.10.9 and `hex` 0.4.3 are already locked.

**Unsafe code.** A small, reviewed FFI surface in `io.rs`, with each call wrapped once.

**Not in this envelope.** API-5 (§2.3) needs its own approval and its own validation. A maintenance or provisioning executable needs a separate Architect authorization (G-HOST).

**Validation for that mission:**
- targeted `cargo test --locked -p nexus-verifier-sandbox` for the custody targets;
- clippy for those targets;
- `cargo fmt --all -- --check`;
- the live harness built only;
- the §16.4 negative controls.

---

## 18. Gates, non-goals and non-claims

### 18.1 External gates

| Gate | Needs |
|---|---|
| **G-AUTH** | An end-to-end authority model against tampering by the store uid (§3.3), before any production or runner use. **Unresolved.** |
| **G-IMPL** | An implementation mission for §17 |
| **G-HOST** | Owner qualification and provisioning on a named host (§13.2). R4 adds to it: the qualification of the running kernel's exact build against the source claims of §5.1 and §10.7; the superblock facts (an internal journal at inode 8, no `fast_commit`); the effective profile of §6.4; and the A-S5 attestation. |
| **G-NATIVE** | Native-layer guarantees for the residuals of §14.4 |
| **G-LIVE** | Integration of the owner process and service, and live validation |
| **G-PWR** | Empirical power-loss qualification, if it is ever required (D-10). It would also be where A-S5, flush success, could be tested empirically. |

API-5 (§2.3) is not a gate of this design: nothing here depends on it. It needs its own approval if it is ever wanted.

### 18.2 Non-goals

This design does not:
- initialize, select or provision any root;
- touch the runner, systemd, buses or the host;
- run any live case;
- design the owner service or its transport;
- resist domain-A tampering, or introduce any anchor or privileged component;
- support XFS or any other filesystem;
- perform automatic maintenance;
- offer a force-close, drop an owner, or clear a failure;
- migrate any format;
- build or authorize a maintenance or provisioning executable, a privileged helper or service, an account, a cloud service, a TPM dependency or a database.

### 18.3 Non-claims

- **Nothing is implemented.** No store, file, directory, pool, `PROVISION`, lock or disposition exists.
- **Nothing ran against the Rust code.** The Python model and the reference check ran only on in-memory byte images and Git objects.
- **No qualification or proof.** There is no tamper resistance against the store uid, no power-loss proof, no filesystem or device qualification, no authentication, and no proof of native cleanup.
- **No completion.** There is no journal-completeness claim beyond §11, no live acceptance, no integration, and no Phase Two completion.
- **No interface exists.** The admission gate, the fatal latch, the maintenance session and its in-session verification, the selection protocol and API-5 are specifications only.
- **A-M1, A-M2 and A-S1 to A-S5 are assumptions.** The per-directory over-approximation shows which conclusions do not depend on A-M1. A-M2 was read from the Linux v6.17 sources, not tested. Nothing shows that a device or a running kernel honours them. A-S5 is new in R4.
- **No activation ran on a host.** No directory was synced, no timestamp probed and no state root created. Activation is a specification and a model.
- **No host was observed in R4, and nothing was reproduced live.** The R4 findings rest on reading the v6.17 sources and on the models. R4 read no profile listing, jbd2 entry or kernel identity from any host. Its profile values are fixtures. No reproduction on the Owner's or any other machine is claimed.
- **Activation does not certify filesystem health.** It establishes the dependencies of one selection under the declared model (§10.7), and nothing about later transactions.
- **No approval.** Publication is not approval.

---

## 19. Review record

### 19.1 Findings and counterexamples

| Finding | Counterexample against the first candidate | Resolution | Model |
|---|---|---|---|
| R1 | F1 made pending bytes durable. A startup sync after the writer had observed EIO returned 0 and was read as durability. | §4: separate state, errseq semantics, certification by the writer only | C01, C02, C03 |
| R2 | Writing slot *k* into a shared page, then F2, tore the page and corrupted slot *k−1*. A seal write could tear the header. | §5, §6.3: one block per record, a separate seal block, write-once blocks, an explicit containment assumption | C04 |
| R3 | A recorder panic dropped the lock descriptors, and a second owner started while custody still held native owners | §8: lock descriptions owned by `StoreGuard`; a separate I/O description | C05 |
| R4 | Startup's sync of a live writer's journal ran before its lock attempt, and a FIFO entry blocked the open | §10: lock first, type-check before open, `O_NONBLOCK` | C06, C14 |
| R5 | A stalled owner filled the event queue, so the worker's failure message was dropped | §9: the exchange plus latches; idempotent submission | C07, C08 |
| R6 | A non-durable `IncidentOpened` left a recorded-unsettled count of 0, and the old superset invariant claimed knowledge nobody could have | §11.2, §12: an honest report, conservative refusal, explicit domains | C09, C13, C16 |
| R7 | Seal bytes 118–127 were unassigned. A checksum-valid but invalid header with empty slots was taken as an abandoned claim. | §7: every byte assigned; the checksum-valid rule | C10, C17 |
| R8 | ext2 or ext3 was accepted through the shared magic | §6.4: identification by mount ID; ext4 only | C14 |
| R9 | Archival changed the binding through the file index, silently invalidating dispositions | §12.3, §13: content-addressed bindings; specified procedures | C11, C12 |

### 19.2 Second review

The revision, the model runs and a second full review covered:
- every lock and descriptor lifetime;
- every persistence step and crash point;
- every `PROVISION`, disposition and archive transition.

Each item found below was resolved at the first repair; none needed a second.

| Found | By | Resolution |
|---|---|---|
| Archived, dispositioned history versus revocation | Review | Revoking the disposition of an archived `u`, `m` or `p-` entry refuses as "archive inconsistent" (§13.4) |
| `RecoveryAttempt` after `RunEnded`: the incident is settled before the attempt record is written | Review | G12 requires an `IncidentOpened` after `RunEnded`, not an unsettled one |
| A record issued before the claim could not be made durable, yet would block closure | Review | No record before the claim (§10.2): a failed claim leaves an untouched custody that closes |
| AbandonedClaim files could neither be archived nor recycled, and so would leak pool capacity | Review | Recycling admits AbandonedClaim files; the only evidence lost is a header that never became valid (§13.8) |
| A failed seal write leaves a kernel-visible seal, and the table claimed the generation stays unsealed | Model (C05) | The visible seal is still true (`close()` returned `Ok`). The generation becomes unsealed only once eviction or power loss removes it (§8.3, §15.1). |
| "A smaller durable view can only add refusal" was too strong: a torn visible block can become a shorter valid prefix | Review | Restated as "less evidence never hides admitted work" (§4.6) |
| Unbounded claim-gap enumeration: a header claiming 2^63 | Model (C17) | Gaps are counted arithmetically and enumerated only within a bound (§7.7) |
| Three cited source ranges ended inside a block | Reference check | Narrowed to whole items (`core.rs:1289-1297`, `core.rs:1744-1752`, `core.rs:2663-2669`) |
| Crash rows did not separate F1 from F2 outcomes for dispositions, archival and recycling | Model (C12) | Rows split by crash point and fault (§15.2) |
| A crash between the two directory syncs of a revocation can leave both names | Model (C12) | Refuses as Invalid (link count 2) until the Owner completes the revocation (§13.4) |

### 19.3 R2: findings F1–F4 and section 8

Each counterexample was first re-run on the **unchanged R1 model** (`docs/evidence/p2-v1-r3b-i3-p-r1/design_checks.py`, loaded by path and checked against its SHA-256), and each reproduced. The R2 model repeats these runs every time it is executed.

| Id | Finding | Observed on the R1 model | Resolution | Model |
|---|---|---|---|---|
| R1-A | F1 | Worker loss with three records acknowledged and none issued: nothing delivered; admission of the acknowledged start still permitted | §9.2, §9.5, §9.8 | C18 |
| R1-B | F1 | A conflict on acknowledged record 1 latched and was never delivered; R1's INV-4 contradicted three applied acknowledgements | §9.3, §9.5, INV-4 | C18 |
| R1-C | F1 | Zero, out-of-range and foreign submissions latched and were never delivered; a future sequence was stored silently; a conflict at record 6 with three applied left records 4–6 pending for ever | §9.3, §9.5 | C18 |
| R1-D | F1 | Loss between a durable reservation and its admission: admission permitted | §9.8 | C18, C19 |
| R1-E | F1 | A record in flight when a conflict latched was published (1 → 2) and acknowledged | §9.4 W6 | C18, C19 |
| R1-F | F1 | Loss during the claim and during the seal left both states `None`, neither failed nor completed | §9.2, §9.7 | C18 |
| R1-F2 | F2 | The verifier inside maintenance was Busy; the modelled procedures took no store lock | §13.1, §13.11 | C20 |
| R1-F3 | F3 | A startup that read `PROVISION` A before a succession and locked afterwards became Ready on A, while a fresh startup became Ready on B | §10.5, §13.6 | C21 |
| R1-F4a | F4 | After a disposition's link and unlink, before the directory sync, F2 had a single outcome: absent | §4.8, §15.2 | C12, C22 |
| R1-F4b | F4 | The model synced the state root when creating `LOCK`, which the text did not say; pool files were durable at creation without a sync | §13.2 step 3, §15.3 | C23 |
| R1-F4c | F4 | Clean pages could not be reclaimed while the writer's descriptor was open | §4.2 | C22 |
| R1-S8 | Section 8 | 5000 orphan dispositions were all read, and 5000 revoked entries all examined: no aggregate bound | §6.6, §7.7, §11.2 | C24 |

**R2 review.** The revision, the model runs, a second full review against the source and the extra interleavings found the items below. Each was resolved at the first repair; none needed a second.

| Found | By | Resolution |
|---|---|---|
| `Control::cancel` would record a recorder failure as a cancellation, which the core forbids (`model.rs:377-384`) | Review | The store admission gate (§9.8); API-5 proposed, not required (§2.3) |
| With no record issued after the latch, `record_failed` at P + 1 would return `NotIssued` | Review | Deliver only at an issued record; the gate covers admission meanwhile (§9.5) |
| `finish_run` after a latch, with nothing pending, would commit before the core learns of the failure | Review | Owner policy (§9.8); such a commitment is never final or published (§9.9); API-5 would close it |
| §8.3 said closure is refused for good after recorder loss; with nothing pending and nothing held, `close()` can succeed | Review | Corrected; the seal is withheld (§8.3, §9.7) |
| A re-qualification session could not begin, because the stale options refused every opening | Model (C12, C21) | Re-qualification mode (§13.3) |
| A successor could be selected before the session held its lock | Review | The session takes the successor's lock before publishing (§13.6 step 2c) |
| A recycling interrupted after its rename refused as Lost, with no specified way out | Review | Resumption (§13.8), modelled for every crash point and schedule [C12] |
| Leftover `PROVISION` temporaries had no defined names, so recovery could not identify them in a shared directory | Model (C12) | Fixed names (§7.5); leftover removal (§13.1) |
| The startup preservation sync was said to make visible bytes durable; a page whose writeback failed is clean and is not rewritten | Review | §4.6, §10.2 wording |
| The verifier contract said its inputs become durable only through the §13 order | Review | "Guaranteed durable only once the order completes" (§13.11) |
| A three-party interleaving (worker, apply, latch) expected a failure report when no record had been issued yet | Model (C19, new assertion) | The protocol was right; the new assertion was corrected to §9.5 step 2 before publication |
| The port of C12 to sessions and schedules had dropped three R1 assertions (recycling and retirement history, the retirement bound) and narrowed two others | Comparison of every R1 assertion with the R2 model | Restored unchanged before publication. The remaining differences are listed, with the requirement each keeps, in the evidence README and the model's output. |
| One control (truncated report) exercised only the startup decision, never the retirement path | Review of the controls | Split into NC24b (startup) and NC24c (retirement); each fails its own assertion |
| Tables in §8.1 and §10.2 held unescaped `\|` inside code spans, which splits a table cell in GitHub Markdown | Table lint | Escaped |

### 19.4 R3: durable activation and composed recovery

The R2 baseline was first re-run and reproduced exactly. The Architect's composed counterexample was then built from the **unchanged R2 model's** own storage, maintenance, selection, startup, claim and recorder helpers (`docs/evidence/p2-v1-r3b-i3-p-r2/design_checks.py`, loaded by path and checked against its SHA-256). The R3 model repeats these runs every time it is executed.

| Id | Observed on the R2 model | Resolution | Model |
|---|---|---|---|
| R2-W-ASIS | As is, the claim on the successor failed: the R2 model's worker rechecks identity (W5) through the fixture's first root. Design W5 and §8.1 recheck through the opened root's retained journals descriptor, so this is an unmodelled transition, not a design guard. | The fixture's default root is bound to the opened root, with no model code changed; the R3 model's worker is corrected the same way | C25 |
| R2-W | A successor's `PROVISION` was visible but not durable when its session died. An owner started on it, claimed, and acknowledged `RunStarted`, `CaseStarted` and `ActionStarted`, then admitted an action. In 10 of 14 power-loss outcomes a fresh owner was Ready on the predecessor, with that generation outside the decision. | Durable activation before any claim (§10.6) | C25, C26 |
| R2-P | The same with a first `PROVISION`: after power loss the opening refused as Unprovisioned (fail closed), but the unpublished root held an unsealed generation with an action start. R2's recovery premise ("holds no history") was false, and following it gave a fresh, Ready store without that history. | Activation; recovery never replaces a root that may hold history (§13.2) | C26 |

**R3 review.** The revision, the model runs, a review against the kernel sources and the composed and chained scenarios found the items below. Each was resolved at the first repair; none needed a second.

| Found | By | Resolution |
|---|---|---|
| A directory `fsync` through a new descriptor returns 0, having committed nothing, when an earlier commit failed silently (v6.17 `__jbd2_journal_force_commit`) | Source review | The certification probe on `LOCK` (§10.6 A3; A-M2) |
| A read-only superblock makes `fsync` return 0 without committing anything (v6.17 `ext4_sync_file`) | Source review | The probe fails with EROFS; the mount is rechecked after the syncs (A4) |
| `PROVISION` could live on another filesystem than the store's `LOCK`, which neither the probe nor the qualified profile would then cover | Review | One filesystem required and verified at every opening (§6.2, §6.4 step 8) |
| The ancestors' entries were never required to be durable | Review | §13.2 step 1 |
| Chained administration: an archival interrupted after its link, then recycling and retirement, could drop the retired claim's archive copy under the per-directory over-approximation (4 outcomes; none under A-M1) | Model (C26, chained) | Sessions activate at their beginning (§13.1) |
| The R2 model keyed `flock` ownership by object identity, so a lock held when a world was cloned never released in the clone (a spurious Busy after F1) | Model (C26 missed NC-ACT-VISIBLE) | Keyed by a serial of the open file description; no retained output changed |
| The model's generation redraw ignored archive headers, which §7.7 includes | Model (C26: most second crashes refused as duplicate generations) | Corrected; the composed owner also draws its own generation instead of a fixed one |
| With one pool file, disposition, revocation and archival could never be followed by dependent work | Model (C26 statistics) | The composed scenarios use two pool files |
| Two new controls were caught by a state assertion or ended in a tool error rather than by the behavioural oracle | Review of the controls | The fault cases run the oracle before any state assertion; every new control now fails on an omitted generation |

### 19.5 R4: the activation proof and the supported profile

The R3 baseline was first re-run on the unchanged R3 evidence. The model's output, the reference check's output and `coverage.json` were byte-identical to those recorded. The Architect's findings F1–F4 were then traced through the complete v6.17 paths (§1.4). The R4 findings were re-run on the **unchanged R3 model** (`docs/evidence/p2-v1-r3b-i3-p-r3/design_checks.py`, loaded by path and checked against its SHA-256). The R4 model repeats these runs every time it is executed. The Architect has not claimed a live reproduction, and none is claimed here.

| Finding | Minimal trace (v6.17) | Verdict | Resolution | Model |
|---|---|---|---|---|
| F1 | `commit.c:775-778` and `commit.c:883-886` call `blkdev_issue_flush` and discard the status (`blk-flush.c:468-474`). The first needs an external journal (`journal.c:1684` versus `journal.c:1641-1657`); the second needs asynchronous commits, set only by `journal_async_commit` (`super.c:4058-4095`), which v6.17 refuses with `data=ordered` (`super.c:4963-4968`). | Confirmed; unreachable on the R4 profile | The profile, established from effective state (§6.4) | C27, C29 |
| F2 | `checkpoint.c:338-339` discards the flush status, then the tail moves (`journal.c:1056-1091`, `journal.c:1852-1885`); recovery starts at the tail (`recovery.c:611`). It is reached from ordinary handle starts (`transaction.c:272-279`). A failed home write is caught (`journal.c:1861-1864`, `buffer.c:1214-1222`); a failed flush is not. | Confirmed and reachable. A threat to required history only when the device fails that flush and a power loss follows before a later successful flush. | **A-S5 (new)**, stated with the event, its effect and the alternative (§5.1, §12.2) | C29 (A-S5 shown necessary) |
| F3 | `journal.c:787-808`: no abort test unless the tid is running or committing (`journal.c:800-805`); reached from `fsync.c:115` through `fast_commit.c:1207-1208`. | Confirmed as a source fact. Its threat to activation is refuted once the certificate is restated: the syncs' own abort tests and the probe's handle-start test (§10.7). R3's wording is withdrawn. | §5.1 A-M2, §10.6, §10.7 | C28, C29 |
| F4 | R3's `SimFS.fsync_file` returned EIO for every aborted, unnoticed journal | Confirmed model inaccuracy | Corrected (§16.1) | C29, NC-FSYNC-COMPLETED |
| Profile | R3's §6.4 step 6 inferred a mode from a string absent from field 11, which omits defaults (`super.c:2949-2951`) | Confirmed | §6.4 steps 9–11, `PROVISION`'s `kernel` | C27 |

| Id | Observed on the R3 model |
|---|---|
| R3-F4 | R3's probe state is one flag, with no running or completed distinction. After the probe's transaction completed and a later commit aborted the journal, R3's `fsync_file` returns EIO. v6.17 returns 0 (`journal.c:800-805`), as the R4 model does. |
| R3-F4-REACH | Over R3's 27 checks, all passing, R3's `fsync_file` ran 3484 times and took the inaccurate branch 0 times. No R3 check result depended on it, and every retained R3 output is identical under the corrected model. |
| R3-PROFILE | R3's host model observes only the `mountinfo` fields, sizes and link sysctls. Its opening is Ready, and claims, on a host whose effective `nobarrier`, or whose external journal, the R4 opening refuses. |

**Changed assumptions.** R3's A-M2 said that an `fsync` of the probed file waits for the probe's transaction, and §10.6 said that a successful probe shows that the journal had not aborted by the time of the probe. Both overstated. The safety requirement is unchanged: no claim on a selection whose dependencies are not durable. Its argument now rests on the restated certificate (§10.7). The R3 evidence is kept unchanged and is not edited to agree.

**R4 review.** The revision, the model runs and a review of the whole document against the sources found the items below. Each was resolved at the first repair; none needed a second.

| Found | By | Resolution |
|---|---|---|
| The writer's first sync after its inode is loaded can also take the completed-transaction return; §4.3 claimed an unconditional flush | Source review | §4.3 states it. The first record's sync flushes the device again before any acknowledgement, so INV-1 is unchanged. |
| Ordered-data wait errors are discarded by the commit (`commit.c:738-744`), and v6.17's `data_err=abort` is not consulted on that path | Source review | Recorded (§1.4). They stay in the file's errseq (`commit.c:240-247`), which every publishing procedure and the recorder read through their own descriptors. |
| The proposed listing is world-readable but not bound to the root descriptor by itself | Review | Bound through the descriptor's `st_dev`, `/sys/dev/block` and the device name ext4 uses (§6.4 step 9). A contradiction with `mountinfo` refuses. |
| The first version of C28's enumeration held every run in memory | Model | It yields one run at a time |
| C29's discarded-flush cases were first written per profile with ad hoc set-up | Review of the check | One event list per profile, through the same event function as the enumeration |
| NC-ACT-UNQUALIFIED was first caught by the external-journal case, not by a claimed-string case | Review of the controls | The cases begin with the hidden-default and kernel cases, so it fails where it should |
| A cited line of `libc`'s own `mod.rs` matched the reference check's pattern for the custody `mod.rs` | Reference check design | Rephrased without the `file:line` form |
