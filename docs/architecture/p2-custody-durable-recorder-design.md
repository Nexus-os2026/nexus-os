# P2 custody: durable recorder and refusal store — design (revision R1)

| | |
|---|---|
| Missions | P2-V1-R3B-I3-P (first candidate, `b23ae1ac`), revised by P2-V1-R3B-I3-P-R1. A module of Phase Two, not Phase Three. |
| Status | **Design candidate for independent Architect review.** Not implemented, not provisioned, not qualified on any host, not live-validated, and not approved by being published. |
| Design base | `b23ae1ac040732c8332e8a45cc80b10734ca0b36` (tree `5330c01e…`, parent `45898e05…`). It is not accepted for implementation. |
| Source baseline | `45898e05178a56efaadeb1f8f9ee7a0a521c72c9`. Every `file:line` reference below is to this commit. |
| Validation | `docs/evidence/p2-v1-r3b-i3-p-r1/` holds a standard-library Python **design model**, its checks, its model-level negative controls and a reference check. It validates this specification's protocols against the model's own semantics. It is not the store, an owner service, or a substitute for the later Rust tests (§16). |
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
| 6 | Submission and delivery use a bounded, idempotent, state-based **exchange**, not message queues. Failure and recorder loss are latched flags that never need queue capacity. Four positions are kept separate: last durable, last delivered, last applied, and first not established durable. | §9 |
| 7 | Startup takes every lock non-blocking **before** syncing or reading any journal. It type-checks entries before opening them, opens with non-blocking, no-follow flags, and changes no application state. Its one persistence effect is an evidence-preservation sync, which is disclosed. | §10 |
| 8 | Refusal stays conservative [D-3]. An unsealed generation that recorded any action start is refused, whatever its recorded-unsettled count. The restart report keeps separate the recorded unsettled entries, evidence completeness, whether native work was possible, and the refusal. It never invents an incident identity. | §11 |
| 9 | Every container and text byte is assigned and validated, with full consumption and checked arithmetic. The NXCD version-1 record frame stays byte-identical [D-9]. | §7 |
| 10 | Only ext4 is supported initially [D-6]. It is identified from the retained descriptor's mount ID in `mountinfo`, never from the shared `0xef53` magic alone. XFS is deferred. | §6.4 |
| 11 | Maintenance is offline, root-performed, under the store lock, and either fully specified or unsupported [D-7]. Journal bindings are content-addressed, so archival keeps them. Any byte change creates a new binding, explicitly. | §12.3, §13 |
| 12 | Store A0 is limited crash evidence, **not** tamper-resistant admission authority [D-4]. Production or runner use stays behind the unresolved gate **G-AUTH**. | §3.3, §18 |
| 13 | No change to `core.rs`, `model.rs` or `codec.rs` is required. | §2.3 |

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

- **Governance read.** `AGENTS.md` and `CLAUDE.md` were read at `45898e05`. They are byte-identical at the design base `b23ae1ac`, verified by blob id. No descendant `AGENTS.md` or `CLAUDE.md` exists.
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

**Linux kernel documentation and source** [U]:
- **errseq** (`https://docs.kernel.org/core-api/errseq.html`), on `errseq_sample`: "If the error has been "seen", new callers will not see an old error. If there is an unseen error in eseq, the caller of this function will see it the next time it checks for an error."
- **vfs**, "Handling errors during writeback" (`https://docs.kernel.org/filesystems/vfs.html`): errors are reported "to fsync on all file descriptions that were open at the time that the error occurred."
- **Linux v6.17 `fs/open.c`**, `do_dentry_open`: `f->f_wb_err = filemap_sample_wb_err(f->f_mapping);`. A new open file description samples the error sequence when it is opened.
- **FIEMAP** (`https://docs.kernel.org/filesystems/fiemap.html`): `FIEMAP_EXTENT_UNWRITTEN` (allocated but not initialized), `_SHARED`, `_DELALLOC`, `_UNKNOWN`, `_ENCODED`, `_DATA_INLINE`, `_NOT_ALIGNED`; `FIEMAP_FLAG_SYNC`.
- **sysfs-block ABI** (Linux v6.17 `Documentation/ABI/stable/sysfs-block`):
  - `logical_block_size` is "the smallest unit the storage device can address";
  - `physical_block_size` is "the smallest unit a physical storage device can write atomically";
  - `write_cache` reports the cache mode.

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
| **D-1** | Root-controlled provisioning and administrative disposition are kept. Ordinary opening never creates, repairs or selects anything. The layout is corrected (§6). |
| **D-2** | Paths are chosen by the Owner, never here. Capacities are finite. The initial targets are N=256 pool files and C=512 records per journal; with the safe geometry the pool costs 538,968,064 bytes (§6.6). The Owner may provision a smaller N. Every bound is checked before allocation or traversal (§7.7). |
| **D-3** | Refusal is conservative. An unsealed generation that could have admitted native work is refused. A recorded-unsettled count of zero is never proof (§11). |
| **D-4** | A0 is specified only as limited crash evidence, never as tamper-resistant admission authority. Gate **G-AUTH** stays open for production or runner use. No A1–A5 helper, account, capability, TPM or cloud dependency is introduced. The earlier requirement that data the store uid writes must not lift a refusal is **kept as an open requirement**, not dropped (§3.3). |
| **D-5** | A stall is reported, never acknowledged and never failed by timeout. Admission, failure and late completion are defined exactly. Recorder loss and I/O failure are latched (§9.6). |
| **D-6** | Initial support is ext4 only. It is identified through the descriptor's mount, with the supported options pinned at provisioning. XFS is deferred. Ambiguity refuses (§6.4). Nothing is qualified now. |
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
| `acknowledge` → `Ledger::acknowledge`: in order and exact | `core.rs:2996-3010`, `core.rs:1042-1069` | Applied exactly once per sequence. Any outcome other than `Acknowledged` is an integration fault (§9.4). |
| `record_failed` → `Ledger::fail`: the next unacknowledged record only; `LedgerFailed` afterwards | `core.rs:3016-3031`, `core.rs:1072-1090` | Applied once, for the first record not established durable, after every earlier acknowledgement (§9.5) |
| Capacity: general and control reserve, all-or-nothing reservations; a control reserve of at least 2 | `core.rs:967-968`, `core.rs:980-989`, `core.rs:1004-1011`, `core.rs:1298-1304` | Every issuable record has a pre-written zero block (§6.6) |
| Admission needs an acknowledged start record. Case begin and the commitment need everything acknowledged. | `core.rs:1658-1675`, `core.rs:1542-1544`, `core.rs:2663-2669` | Write-ahead (INV-2) |
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

---

## 3. Threat and fault domains

### 3.1 Domains

Every invariant (§12.1) names the domains in which it holds.

| Domain | Meaning |
|---|---|
| **H**, honest reachable execution | The owner and recorder follow this protocol, and the core behaves as its tests establish. Storage meets the supported profile (§5). The faults are process termination (F1) and machine or power loss (F2), in any order and number, including F1, then restart, then F2. |
| **M**, malformed bytes | Arbitrary content in any store file. Required: totality, bounded resources, determinism, and refusal of anything structurally invalid. **Not** required: semantic truth. Arbitrary bytes can form a well-formed but misleading journal. |
| **S**, storage faults (F3) | Bit flips, torn writes outside the containment unit, misdirected or lost writes, and a device that lies about flushing. Required: refusal of every detectable fault. Undetectable faults are outside every guarantee. |
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
| Directory entries **K_dir** / **D_dir** | Visible names, and durable names (after an `fsync` of the directory) [U] | Unchanged | **K_dir** becomes **D_dir** |
| Advisory locks | Attached to open file descriptions [U] | Released when the last duplicate closes, which process death causes | Gone |

### 4.2 Events

| Event | Effect |
|---|---|
| `pwrite` | Updates **K** for the bytes written, which may be fewer than requested, or none. Marks those pages pending. |
| Background writeback, success | The page's **D** becomes its **K**; the page is clean |
| Background writeback, failure | **E** records an unseen error, and the page is clean but **D** is unchanged. **K** may keep the new bytes until the page is evicted. |
| `fdatasync(d)` | Writes back every pending page of the file and flushes the device. Returns EIO if **E** has advanced past *d*'s cursor (an error recorded after *d* was opened, or after the last error reported through *d*), then advances the cursor and marks the error seen. Otherwise returns 0. |
| Open | The new description's cursor is sampled: before the newest error if no description has seen it yet (the new description will report it), otherwise at it (the new description never reports it) [U] |
| Eviction | Clean pages may be dropped, so **K** reverts to **D**. Evicting the inode drops **E**. |
| F1 | Process-local state is lost. Descriptors close and their locks are released. **K**, **D**, pending writeback and **E** are unchanged. |
| F2 | **K** becomes **D**. Pending writeback, **E** and all cursors are lost. **K_dir** becomes **D_dir**. A block being written may end old, new or indeterminate, but only within its containment unit (§5). |

### 4.3 What a sync certifies

| Who syncs | In domain H, `fdatasync` returning 0 certifies | It does not certify |
|---|---|---|
| **The writer**: the recorder's I/O description, opened at the claim before any of its writes and kept open | Every page the writer dirtied before the call reached **D**, and the device was flushed (A-S1). A writeback failure of those pages, before or during the call, is reported to this still-open description [U, vfs]. | — |
| **A newly opened description**: startup, the verifier, any other process | Only that the pages pending at the time of the call were written successfully | That earlier writebacks succeeded. An error already reported through another description, now closed, is never reported to it ("new callers will not see an old error" [U, errseq]). An error held by an evicted inode is gone. Pages whose writeback failed are clean; they may still show new bytes in **K** while **D** lacks them. |

Neither this design nor any report it specifies may present a sync on a newly opened description as evidence that no earlier writeback failed [M: C03].

### 4.4 What startup may infer from the bytes it reads

- **A visible valid record (domain H).** The recorder wrote it and the core issued it. It is a true statement of what the core issued at that instant. It may not be durable: it may exist only in **K**, and a later F2 or eviction may remove it.
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
- **Less evidence never hides admitted work.** F2 or eviction may later remove visible-only bytes. Re-classification then sees less, and the class may change: a torn visible block, for example, can become a shorter valid prefix. But an action whose start record never became durable was never admitted (INV-2), so no later view hides admitted native work [M: C16].
- **Evidence-preservation sync.** Startup syncs each prior journal after locking it (§10.2 step 5).
  - This **changes persistence state**: pending visible bytes become durable.
  - It changes no application state and certifies no history.
  - An error from it refuses the store as "evidence unreliable" [M: C02, C03].
- **Failed evidence is not cleanup.** A record that is visible but failed is still only a statement of what the core issued. Native cleanup success is established only by the core's own facts at the time. A stored `ActionSettled` is a past statement (§14.2).

### 4.7 Sequences the model exercises [M]

| Sequence | Outcome |
|---|---|
| The writer `pwrite`s record *n*, then F1 | **D** is unchanged and **K** shows *n*. Process death made nothing durable [C01]. |
| F1, restart, F2 before any sync | *n* is lost [C02] |
| F1, restart, a startup sync that returns 0, then F2 | *n* is kept [C02] |
| *n*'s writeback fails, the writer's sync reports EIO, then F1 and restart | A new description's sync returns 0, yet **D** lacks *n* while **K** shows it. After eviction **K** loses *n* too. The report must not claim *n* is durable [C03]. |
| The writer dies before observing *n*'s writeback failure; restart | The error is unseen, so the new description's sync returns EIO and startup refuses [C03] |
| Short write, zero-progress write, EINTR | A block counts as written only once all 4096 bytes are written. A bounded number of zero-progress results or EINTR poisons the journal [C15]. |

---

## 5. Physical storage assumptions and containment (R2)

### 5.1 Assumptions [A]

- **A-S1, flush honesty.** When `fdatasync` returns 0, the data is on stable media: the device honours cache flushes. sysfs `write_cache` may report a write-back cache [U]; flushes must still be honoured.
- **A-S2, containment.** A failed or interrupted write of one aligned 4096-byte block, issued by the filesystem for one page, can leave only that block indeterminate. It never alters any other block's durable content.
- **A-S3, no silent loss.** After a successful flush the device neither loses nor misdirects writes. This is the boundary of domain S.
- **A-S4, read stability.** Reading durable data returns those bytes or an error.

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
| Device logical and physical block sizes (sysfs, for the mount's major:minor); pool extents written, not shared, not delayed (FIEMAP); `fallocate` support | Verified at provisioning as part of the Owner's qualification, then pinned in `PROVISION` (§13.2). Not re-read at runtime. |
| A-S1 to A-S4 | Assumed. The Owner attests them in `PROVISION` (`storage-attestation`). They can be qualified empirically only (D-10), never proven. |

### 5.4 When the assumptions cannot be established

- **A verifiable property does not match.** Opening refuses as **Unsupported**.
- **The Owner cannot attest A-S1 to A-S4 for the device.** The store must not be provisioned there; it stays Unprovisioned, and every run is refused.
- **A torn block in domain H** (non-zero and invalid) is never read as benign. The journal is Malformed and refused (§11.1). A torn unacknowledged write and corruption cannot be told apart.

---

## 6. Authority, layout and filesystem profile

### 6.1 Actors

| Actor | Identity | May write | Authority |
|---|---|---|---|
| **Owner** | root | `PROVISION`; root-owned directories; creation and replacement of pool files; dispositions; the archive | Provisioning, disposition, revocation, archival, recycling, retirement and succession. Each is an explicit root-owned artifact made through a §13 procedure. |
| **Custody owner process** | store uid | Through its recorder, the bytes of the one pool file it claimed. Locks on `LOCK` and that file. | None. It produces evidence; the core decides. |
| **Read-only verifier** | store uid or root | Nothing | None |
| **Other store-uid processes**, including job code | store uid | Whatever that uid can write (domain A) | None recognized |

### 6.2 Layout

```
<PROVISION_PATH>               root:root 0444  identity, parameters, attestation (text, §7.5)
<STATE_ROOT>/                  root:root 0755
  LOCK                         uid:gid   0600  store lock (flock); never written
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
- The proposed forms are `/etc/nexus-os/phase2-custody/uid-<uid>.provision` and `/var/lib/nexus-os/phase2-custody/uid-<uid>-<root id>`.
- Every ancestor of both must be root-owned and neither group- nor other-writable.
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

### 6.4 Filesystem profile: ext4 only [D-6]

`statfs` magic `0xef53` is shared by ext2, ext3 and ext4 [U]. It is never used to identify ext4.

Identification is bound to the retained root descriptor:

1. `statx(root_fd, "", AT_EMPTY_PATH, STATX_MNT_ID)` yields `stx_mnt_id` [U].
2. Read `/proc/<pid>/mountinfo`: this process's own pid, not the `self` symlink, opened with `RESOLVE_NO_SYMLINKS`. Exactly one record must have field 1 equal to `stx_mnt_id`. Zero or several matches, or an unparsable line, refuse.
3. That record's field 3 (major:minor) must equal `fstat(root_fd).st_dev` [U]. While the root descriptor is held open, the mount cannot be unmounted normally. A lazily detached mount disappears from `mountinfo`, which refuses at step 2.
4. Field 9 (filesystem type) must be exactly `ext4`.
5. Field 6 (per-mount options) and field 11 (per-superblock options) must equal `mount-options` and `super-options` in `PROVISION`, byte for byte. Changed options refuse until the Owner re-qualifies (§13.3).
6. The pinned option strings must not contain `ro`, `nobarrier`, `barrier=0` or `data=writeback`. This is a provisioning rule, re-checked at every opening.
7. `fstatvfs(root_fd)` must report `f_bsize` and `f_frsize` of 4096, and `sysconf(_SC_PAGESIZE)` must be 4096.

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
| Archive entries | At most 4096 are scanned; more refuses |
| Startup read volume | Every pool file in full, plus blocks 0 and 1 of each archive entry: 538,968,064 + 33,554,432 bytes at the targets and limits |
| Scan memory | One 4096-byte block buffer, a SHA-256 state, and per-journal grammar state proportional to `capacity`. One bounded summary per file. |
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
digest=<64 hex: SHA-256 of every preceding byte>
```

- `predecessor-statement` is `none` if and only if `predecessor` is `none`.
- The digest is integrity only. Authority comes from root ownership at a root-controlled path.

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
- the archive holds at most 4096 entries;
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
| Store lock description on `LOCK` | Long-lived advisory exclusion: `LOCK_EX`, taken non-blocking | `StoreGuard`, in the owner's main frame beside `Custody` | Only at process exit. No `LOCK_UN` call exists. | Opened `O_RDONLY | O_NONBLOCK | O_NOCTTY | O_NOFOLLOW | O_CLOEXEC` |
| Journal lock description on the claimed pool file | Long-lived advisory exclusion: `LOCK_EX`, non-blocking | `StoreGuard` | Only at process exit | A **separate** open, never shared with the recorder |
| Journal I/O description (`O_RDWR`) | Data descriptor; holds **no** lock | Recorder worker | When the worker exits or unwinds | The worker never calls `flock`. A `LOCK_UN` on this description, through a bug, releases nothing, because no lock is attached to it. |
| Root and journals directory descriptors (`O_PATH | O_DIRECTORY`) | — | `StoreGuard` | Process exit | Used for `*at` calls and the identity recheck after each record |
| Scan descriptors, one per pool file | Short advisory lock: `LOCK_SH`, non-blocking | Startup scanner | Closed once that file is classified (§10.2) | — |
| Archive directory and entries, `PROVISION`, dispositions directories and files, `mountinfo` | Read-only | Scanner, disposition reader, mount check | Closed when startup ends | — |
| Exchange (§9.2) | **Short mutex**, held only for a copy | `Arc`, shared by owner and worker | Process exit | Never held across I/O, a callback or a wait |
| Recorder status (§14.1) | Short mutex | `Arc` | Process exit | Copy only |
| Failure latches | Inside the exchange; set once, never cleared | `Arc` | — | Set by the worker, by its drop guard, or by the owner |
| Worker join handle | — | Owner | Process exit | `is_finished()` without a latched normal exit latches recorder loss |
| `Custody` and native owners | — | Owner's main frame | `close()` returning `Ok`, or destruction of the process | Never dropped while it holds anything (`mod.rs:139-143`) [S] |

### 8.2 Rules

1. **Separate descriptions.** Lock-bearing descriptions live only in `StoreGuard` and are never `dup`ed into worker code. A duplicate shares the description, and `LOCK_UN` on any duplicate releases the lock (`flock(2)`) [U] [M: C05].
2. **Release only at process exit.** `StoreGuard` releases its locks only when the process exits. Closing a lock-bearing descriptor earlier is a defect.
3. **Close-on-exec.** Every descriptor is `O_CLOEXEC`. A child between `fork` and `execve` shares the descriptions, which only keeps the locks held longer. The descriptors close at `execve`.
4. **No storage under a mutex.** No status, control or exchange mutex is held across `pwrite`, `fdatasync`, `fstat`, an open, a read, a callback or a wait. The core's own control-side locks follow the same rule (`core.rs:431-442`) [S].
5. **Advisory scope.** The locks exclude cooperating owners and maintenance (§13.1), not domain-A processes.

### 8.3 Behaviour by case [M: C05]

| Case | Locks | Custody | Notes |
|---|---|---|---|
| Normal closure | Held until exit | `close()` returns `Ok`; the seal is written; the process exits | Released by exit |
| Recorder panic or worker disconnect | Held by `StoreGuard` | Kept. The owner latches recorder loss and reports `record_failed` (§9.5). Closure is refused for good (`core.rs:3012-3015`) [S]. | The process stays until administrative destruction, and the store is Busy for everyone else |
| Failed startup, before the claim | Held during the scan | NotStarted and untouched; it closes | Released by exit |
| Failed claim | Held | NotStarted and untouched (no record was issued, §10.2); it closes | The file stays an abandoned claim or a header-only journal (§11.1). Released by exit. |
| Failed append | Held | Kept; fail-stop; closure refused | As for recorder panic |
| Failed seal | Held until exit | Already closed; nothing native | The seal may stay kernel-visible, and is then still true: `close()` returned `Ok`. Once eviction or power loss removes it, the generation is unsealed, and a startup refuses it if it recorded an action start [M: C05]. |
| I/O blocked in the kernel | Held | Kept; the owner stays responsive (§14.1) | A process cannot finish exiting while a thread is in uninterruptible I/O, so the locks stay held until that I/O ends |

---

## 9. Recorder protocol (R5, D-5)

### 9.1 Roles

- **The owner thread** owns `Custody`. It calls `flush_records` (`core.rs:2958-2991`) with the exchange-backed sink, and applies durability to the core at safe points.
- **The worker** owns only the journal I/O description and its own position.

### 9.2 The exchange

```
Exchange {                                   // one Mutex, held only for copies
  generation, capacity,                      // fixed at the claim
  slots: [Option<RecordIntent>; capacity],   // index = sequence − 1, allocated once
  claim: ClaimState,                         // Requested(header) | Claimed | Failed(class)
  seal:  SealState,                          // None | Requested(seal) | Sealed | Failed(class)
  durable_through: u64,                      // set only by the worker, after W4 and W5
  failed: Option<(u64, FailureClass)>,       // first sequence not established durable; latched
  lost: bool,                                // recorder loss; latched
  conflict: Option<(u64, Conflict)>,         // an invalid submission; latched
  pending: Option<(u64, Tick)>,              // the record in flight, for status only
}
```

The exchange holds **no message queue**, so nothing in it can fill up, overflow or be dropped.

### 9.3 Submission: `RecordSink::submit`

Submission is non-blocking apart from the short copy, and idempotent:

| Condition | Effect |
|---|---|
| Before `Claimed`; sequence 0; beyond capacity; another generation | Latch `conflict`, which is fatal |
| Slot empty | Store the intent |
| Slot holds the same digest | **No effect.** This is a duplicate, such as a re-submission after a sink panic (`core.rs:2974-2985`) [S]. |
| Slot holds a different digest | Latch `conflict` |

After storing, the sink wakes the worker with a condition-variable hint. A missed hint is harmless, because the worker also rechecks the slots on each loop. Two cases need no special handling:
- **A panic after the store, before `submit` returns.** The core keeps the record unsent and records a failure. Its next flush resubmits the same intent, which is a no-op.
- **Repeated submissions.** They change nothing and cost one comparison each [M: C07].

### 9.4 Delivery: state, not messages

The worker's loop works on `next = durable_through + 1`:

| Step | Action |
|---|---|
| W1 | If `slots[next]` is present and nothing is latched, copy it and set `pending` |
| W2 | `codec::encode_record` must succeed (it refuses a digest mismatch, `codec.rs:230-236`) [S] and give 73 to 123 bytes |
| W3 | `pwrite` the 4096-byte block (frame, then zeros) at offset 4096 × (next + 1). Loop on short counts. A zero-progress result or EINTR is retried at most 3 times, then poisons. |
| W4 | `fdatasync` on the I/O description. EINTR is retried at most 3 times; any other result is final. |
| W5 | Identity recheck: `fstat` of the I/O description shows `st_nlink == 1` and an unchanged size, and `fstatat(journals_fd, name, AT_SYMLINK_NOFOLLOW)` shows the same inode |
| W6 | Under the mutex: set `durable_through = next`, clear `pending`, wake the owner |

Any failure in W2 to W5 latches `failed = (next, class)` and stops all writing.

The owner's apply step runs at safe points and never blocks:
1. Copy `durable_through`, `failed`, `lost` and `conflict`. The copied `durable_through` is the delivered position.
2. For each sequence from `applied_through + 1` up to the delivered position, call `acknowledge(RecordAck { id, digest })`, built from the owner's own retained intent. The result must be `Acknowledged`; then `applied_through` advances.
3. Any other outcome latches an owner-side integration fault and stops flushing.

The protocol keeps four positions apart:

| Position | Holder | Meaning |
|---|---|---|
| Last durably written | Worker: `durable_through` | Set after W5 |
| Last delivered | Owner: its latest copy of `durable_through` | A read of state; it cannot be lost or duplicated |
| Last applied by the core | Owner: `applied_through`, equal to `EvidenceView.acknowledged` (`model.rs:837-845`) [S] | — |
| First not established durable | `failed.0`; after recorder loss, `durable_through + 1` | — |

Order of application does not imply that the core has applied earlier records. The owner always applies every acknowledgement up to the delivered position first (§9.5).

**Duplicates.** A duplicate submission is a no-op. Each sequence is acknowledged to the core exactly once, from state. Re-acknowledgement does not exist. An `acknowledge` result of `Duplicate` would mean an owner defect, and is treated as an integration fault [M: C07].

### 9.5 Failure delivery and poisoning

When `failed = (f, class)`:
1. The owner first applies every acknowledgement up to `durable_through`, which equals f − 1.
2. It then calls `record_failed(id_f)` (`core.rs:3016-3031`) [S] and expects `FailureRecorded`.
3. The core fail-stops: every later acknowledgement is `LedgerFailed`, and closure is refused for good (`core.rs:1072-1090`, `core.rs:3012-3015`) [S].

**Recorder loss** (`lost`) is latched in either of two ways:
- by the worker's drop guard, while it unwinds;
- by the owner, when the join handle has finished without a latched normal exit.

The owner then treats `durable_through + 1` as the first record not established durable:
- **if that record has been submitted,** the owner reports it once `applied_through` equals `durable_through`;
- **otherwise,** the owner reports it as soon as the core issues it, and flushes nothing further.

`conflict` is handled like `failed`, at the conflicting sequence.

These signals are latched state:
- they never wait for queue space;
- they cannot be lost in delivery;
- they are never cleared.

**No undelivered signal is ever treated as delivered, and no failure is treated as durable** [M: C08].

A failed sync is never retried, and nothing is acknowledged after one. errseq reports an error once per description, so a retry can return 0 for data that was lost ([U]; corroborated by PostgreSQL `data_sync_retry`) [M: C15]. The record may or may not be durable; startup treats whatever is visible as evidence (§4.4).

### 9.6 Stall semantics [D-5]

| Concern | Behaviour |
|---|---|
| Admission while a record is pending | Refused by the core. A start record must be acknowledged, and a new case or the terminal commitment needs everything acknowledged (`core.rs:1658-1675`, `core.rs:1542-1544`, `core.rs:2663-2669`) [S]. |
| Failure on stall | **None.** A stall is neither a failure nor latched as one. Status shows "record *n* pending since *t*" (§14.1). |
| Late completion: the stalled write and sync succeed | `durable_through` advances and the acknowledgement is applied late; progress resumes |
| Late completion: they fail | Latched as a failure; then §9.5 |
| Recorder loss | Latched; then §9.5 |
| Timeouts | Never become an acknowledgement or a failure. Timing out a waiting thread does not cancel a kernel operation. |
| Operator action | Administrative destruction of the process only. That is authority loss, and refusal follows (§11.2). |

### 9.7 Claim and seal

**Claim.** The owner requests it and the worker performs it:
1. Write the header block (§7.2).
2. `fdatasync`.
3. Recheck identity (W5).
4. Publish `Claimed`, or latch `Failed`.

`start_run` is called only after `Claimed`.

**Seal.** Requested after `close()` returns `Ok`:
1. Write the seal block (§7.3), with records equal to `durable_through`.
2. `fdatasync`.
3. Publish `Sealed` or `Failed`.

Nothing native is held by then, so the owner exits either way.

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
| 1 | Parse `PROVISION` (§7.5), opened safely and read with a limit | Unprovisioned, Invalid |
| 2 | Open `<STATE_ROOT>`; identify ext4 through the mount (§6.4); check block size, page size, the link sysctls and the ancestors | Lost, Unsupported |
| 3 | Open `LOCK` safely and take `flock(LOCK_EX | LOCK_NB)` | **Busy**, before any journal content is touched [M: C06] |
| 4 | Enumerate the store directories, entry names only. Every name must be accepted (§7.5); each entry is type-checked as in §10.1 step 1. | Invalid, MaintenanceIncomplete, Lost |
| 5 | For each pool file, in index order: <ul><li>open it safely;</li><li>take `flock(LOCK_SH | LOCK_NB)`; failure means **Live**, and the file is neither synced nor read;</li><li>`fdatasync` to preserve evidence; EIO refuses as **Unreliable**;</li><li>read the file and classify it (§11.1);</li><li>close it, which releases the shared lock.</li></ul> | Live, Unreliable |
| 6 | Archive: safely open each entry and read blocks 0 and 1 (§11.3) | Invalid |
| 7 | Dispositions: read and validate every file (§13.4) | Invalid |
| 8 | Pool-level checks (§11.3); then split the incidents into history and current (§13.5). More current incidents than `incident_limit` refuse (§13.10). | Invalid, Capacity |
| 9 | Draw the generation (§7.7). Call `Custody::new(config, generation, current, now)`, then `apply_disposition` for each current incident; the disposition reader holds the `dispositions/` descriptor. If any incident still blocks (`snapshot().prior`, `model.rs:860-865`) [S], close the NotStarted custody and exit. | PriorUnresolved |
| 10 | Claim handoff (§10.3), then `start_run`. No Unused pool file means PoolExhausted. | PoolExhausted, ClaimExhausted, ClaimFailed |

- **No record before the claim.** Until the claim is published, the owner serves no control request and calls no core method that could issue a record. A refused or failed claim therefore leaves an untouched NotStarted custody. Its `close()` is permitted, since nothing was issued (`core.rs:2881-2924`) [S], and the process exits.
- **No writes before step 10.** Startup writes no store byte before then.
- **Disclosed persistence effect.** The step 5 sync makes pending visible bytes durable (§4.6).

### 10.3 Scan-to-claim handoff

1. Choose the lowest-index pool file classified **Unused**.
2. Open a **new** lock description on it (safe open) and take `LOCK_EX | LOCK_NB`. The scan's shared lock was already released in step 5, and the store lock excludes cooperating owners. If a domain-A process holds a lock anyway, the claim is refused.
3. Open the worker's I/O description with `O_RDWR | O_NONBLOCK | O_NOCTTY | O_NOFOLLOW | O_CLOEXEC` and the same resolve flags. Its `fstat` identity must equal the lock description's.
4. Re-read the whole file through the I/O description; it must still be all zero.
5. Move the I/O description into the worker. The lock description stays in `StoreGuard`.
6. Request the claim (§9.7). The claim number is the highest of `retired-through` and every claim seen, plus 1 (§7.7). The header lists the bindings of all current incidents, each with the reason from its disposition.

### 10.4 Descriptor classes after startup

| Kept for the life of the process | Closed by the end of startup |
|---|---|
| Root directory, journals directory, store lock description, journal lock description (all in `StoreGuard`) | Scan descriptors; archive directory and entries; dispositions directories and files; `PROVISION`; `mountinfo` |
| The worker's I/O description (until the worker ends) | |

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
| INV-4 | **No acknowledgement after an error or loss.** After a latched failure, recorder loss or conflict at sequence *f*, no sequence ≥ *f* is acknowledged. | H, M (recorder behaviour) | C08, C15 |
| INV-5 | **Totality and bounds.** Classification terminates with bounded memory and reads (§6.6, §7.7). | H, M, S | C10, C17 |
| INV-6 | **Determinism.** Equal bytes give equal classes, reports and bindings. | all | C11 |
| INV-7 | **Binding sensitivity.** Any byte change in a journal changes its binding, which explicitly makes a disposition stale. | all, assuming SHA-256 collision resistance [A] | C11 |
| INV-8 | **Disposition authority.** Only root-owned dispositions that bind exactly are accepted. | H, A (for dispositions only) | C12 |
| INV-9 | **Seal meaning.** A valid seal means `close()` returned `Ok`. | **H only.** In domain A an unkeyed seal proves nothing. | C16 |
| INV-10 | **Exclusion.** While an owner is alive, no cooperating owner or maintenance procedure acquires the store lock. Recorder failure does not change this. | H | C05, C06 |
| INV-11 | **Report honesty.** Recorded-unsettled sets are exact for the prefix. No identity is invented. Completeness is claimed only when sealed. Durability is never claimed from a new description's sync. | H, M | C03, C09 |
| INV-12 | **Administrative atomicity.** No partial disposition, `PROVISION` or archive entry is accepted. Retirement cannot exclude unresolved, undispositioned history. A changed binding is never silently honoured. | H, with the Owner following §13 | C11, C12 |

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
| S | Detectable faults are refused. Lost or misdirected flushed writes are undetectable (§5.2). |
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

## 13. Administrative procedures (D-7, D-8, R9)

### 13.1 Common rules

Every procedure is **offline and performed by root**:

1. **Store lock.** Hold the store lock exclusively and non-blocking for the whole procedure, as in `flock -n <STATE_ROOT>/LOCK <procedure>`; never use `-o` (`flock(1)`) [U]. If the lock is busy, abort. **Root is not exempt.** A step performed without the lock is outside the procedure and unsupported, and the verifier cannot detect it.
2. **Journal locks.** Take `LOCK_EX | LOCK_NB` on every pool file the procedure reads or replaces.
3. **Verify before.** Run the read-only verifier (§13.11) and confirm the procedure's preconditions.
4. **Persistence order.**
   - Write each new file under a `.tmp-` name in its target directory and `fsync` it.
   - Publish it with `link` (no-replace: EEXIST aborts) or `rename` (atomic replacement).
   - `fsync` each changed directory after publication (`fsync(2)`, `link(2)`, `rename(2)`) [U].
5. **Verify after.** Run the verifier again.

**Leftover temporary files.** A `.tmp-` entry left in a store directory by an interrupted procedure refuses opening as **MaintenanceIncomplete** (§7.5). The Owner removes it, or completes the procedure, under the lock. Opening never reads, uses or removes it.

**Existing descriptors.** Ownership changes never use `chown` or `chmod` on a file the store uid may hold open, because neither revokes an existing descriptor. Procedures copy into new root-owned inodes (archival) or replace names (recycling).

**Unsupported.** Anything not specified in §13.2 to §13.10 is unsupported, including:
- un-archiving, editing journals, and deleting dispositions other than by revocation;
- recycling a Malformed pool file that has not been dispositioned and archived;
- automatic archival, recycling or retirement.

Each procedure's crash points are listed in §15.2 [M: C12].

### 13.2 Initial provisioning (manual; no executable) [D-8]

1. **Qualify the host.** The Owner establishes all of the following; if any fails, the store is not provisioned (§5.4):
   - ext4 at `<STATE_ROOT>`, identified through the mount;
   - the option strings to pin;
   - block size 4096 and page size 4096;
   - sysfs logical and physical block sizes for the mount's major:minor, each dividing 4096;
   - `write_cache` recorded;
   - the A-S1 to A-S4 attestation for the device.
2. **Create directories.** Create `<STATE_ROOT>`, `journals/`, `dispositions/`, `dispositions/revoked/` and `archive/`, each `root:root 0755`. `fsync` each, and its parent.
3. **Create the lock.** Create `LOCK` as `uid:gid 0600`, empty, and `fsync` it.
4. **Create the pool.** For each pool file, created under its final name in the still-unpublished root:
   - `fallocate` mode 0 for (C_pool + 2) × 4096 bytes; EOPNOTSUPP or ENOSPC aborts;
   - write zeros over the whole file and `fsync` it;
   - check with FIEMAP (`FIEMAP_FLAG_SYNC`) that every extent is written and unshared (§6.5).

   Then `fsync journals/`.
5. **Publish `PROVISION`.** Write it to a temporary name in its directory and `fsync` it. Then `rename` it into place and `fsync` the directory.
6. **Check.** Run the verifier as the store uid. It must report a fresh, openable store.

`PROVISION` is published **last**: until its rename is durable, the store is Unprovisioned.

### 13.3 Rewriting `PROVISION`

Used by recycling, retirement and re-qualification:
1. Take the store lock.
2. Write the complete new content to a temporary file, and `fsync` it.
3. `rename` it over `PROVISION`, and `fsync` the directory.
4. Verify.

`retired-through` and `predecessor` are the only fields that record retirement or succession. Both are root-owned.

### 13.4 Disposition publication, reading and revocation

**Publication:**
1. Take the store lock.
2. The verifier reports the incident and its binding.
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
1. Take the store lock.
2. `rename` `<binding>.disposition` to `revoked/<binding>-<YYYYMMDDTHHMMSSZ>.disposition`.
3. `fsync revoked/`, then `dispositions/`.
4. Verify.

After revocation, the incident blocks again (§13.5). If a crash leaves the file under both names, its link count is 2 and opening refuses as **Invalid**. That can happen where a rename across directories is not made durable atomically. The Owner completes the revocation by unlinking the `dispositions/` name and syncing that directory [M: C12]. For a `u`, `m` or `p-` archive entry, the store refuses as **Invalid** ("archive inconsistent") until the Owner publishes a disposition again; un-archiving is unsupported. Revoked files stay in `revoked/` as evidence.

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

**Precondition.** The predecessor's verifier report shows that every bound incident (§12.3) has a valid disposition. The `predecessor-statement` names each store-level Invalid condition the report shows. Those conditions have no binding; the statement is the Owner's explicit, recorded acceptance of them.

**Steps:**
1. Take the predecessor's store lock and keep it for the whole procedure. Save the verifier report.
2. Provision the successor (§13.2):
   - at a **new** `<STATE_ROOT>`, with a new root id;
   - `predecessor` set to the old root id, and `predecessor-statement` filled in;
   - before step 5 of §13.2, keep the old `PROVISION` as `<PROVISION_PATH>.predecessor-<old root id>` (written to a temporary name, `fsync`ed, `link`ed, and the directory `fsync`ed).
3. The predecessor's state root is left unchanged, as evidence.

The successor never scans the predecessor. No unresolved incident is dropped silently: each needs its own disposition first, and each unbound condition is named in the statement.

### 13.7 Archival

**Preconditions,** verified:
- a pool file with a valid header that is Sealed (class `s`), unsealed without an action start (`n`), or unsealed with an action start (`u`) or Malformed (journal) (`m`) with a valid disposition; or
- a Malformed (pool-file) file with a valid disposition (the `p-` form).

AbandonedClaim files are not archived; they are recycled directly (§13.8).

**Steps:**
1. Take the store lock and the file's `LOCK_EX | LOCK_NB`.
2. Read the file and compute its content SHA-256, and its binding where it has one.
3. Create `archive/.tmp-<name>` as `root:root 0444` (`O_CREAT | O_EXCL | O_NOFOLLOW`). Write the bytes and `fsync`. Re-read the copy and compare the digest.
4. `link` it to its final name (§7.5).
5. `unlink` the temporary file, then `fsync archive/`.
6. Verify.

- **The pool file is unchanged.** It is now archived and pending recycling (§11.3), not a duplicate.
- **The binding is preserved,** because it is content-addressed [M: C11].
- **The copy is safe from the store uid.** It is a new root-owned inode, so no store-uid descriptor can write to it.

### 13.8 Recycling

**Preconditions:** the pool file is archived and pending recycling (§11.3), or it is an AbandonedClaim.

**Steps:**
1. Take the store lock and the journal lock.
2. Create `journals/.tmp-<index>` as `uid:gid 0600`. `fallocate` it, zero-fill it, `fsync` it, and check it with FIEMAP.
3. `rename` it over `journals/j<index>.journal`, then `fsync journals/`.
4. Rewrite `PROVISION` with the new inode (§13.3).
5. Verify.

**Evidence lost.** An archived file loses nothing, because the archive holds the bytes. An AbandonedClaim loses a header that never became valid; in domain H no record was ever written to it.

**Old descriptors.** A store-uid process may still hold a descriptor on the old, now unlinked inode. That is harmless: the inode is no longer in the pool.

### 13.9 Retirement

**Preconditions,** for a new `retired-through` of K:
- every claim at or below K is either archived as history (`j-`, §13.5) or a claim gap that is history (applied, with a valid disposition);
- no pool file holds a claim at or below K.

**Steps:**
1. Take the store lock.
2. Rewrite `PROVISION` with `retired-through=K` (§13.3).
3. Verify.

**After retirement:**
- archive entries with claims at or below K remain, and are checked by name and type only;
- claim gaps at or below K are no longer computed;
- a revocation of their dispositions no longer re-opens them.

**Retirement can never exclude unresolved, undispositioned history.** The preconditions are verified, and a pool claim at or below K refuses as Invalid [M: C12].

### 13.10 Incident-limit exhaustion

If the current incidents exceed `incident_limit`, startup **refuses**. It reports the count and every incident. It never truncates the list, and never relabels an incident as resolved.

The only resolutions are acts of the Owner:

| Resolution | Effect |
|---|---|
| Publish a disposition for each incident, then archive the dispositioned journals and pool files | Archived history (§13.5 b) leaves the current set. No core counts such dispositions in `RunStarted.dispositioned`; this is disclosed, and the archive and disposition files are the record. |
| Provision a successor (§13.6) | — |
| A future API or limit change (API-2, a gate) | — |

### 13.11 Read-only verifier contract [D-8]

The verifier is a future mode of the owner binary, implemented in the later mission.

- **Locking.** It takes `LOCK_SH | LOCK_NB` on the store lock; if an owner is live, it reports Busy. It writes nothing and syncs nothing, so it reports the kernel-visible state, not the durable state (§4.3).
- **Checks.** It performs:
  - §10.1;
  - §10.2 steps 0–2 and 4–8, without the evidence-preservation sync;
  - all of §11.

  It reports every condition it finds rather than stopping at the first.
- **Report.** The store state; each file's class and report (§11.2); the current incidents and their bindings, with disposition status; history; stale and orphan dispositions; pool usage; leftover temporary entries; the `PROVISION` facts.
- **Exit status.** 0 only if the store opens and no current incident blocks.
- **Durability of its inputs.** `PROVISION` and dispositions become durable only through the order in §13: write, `fsync`, publish, `fsync` the directory. The verifier reports what it reads and never asserts durability.

---

## 14. Native custody, responsiveness and retention

### 14.1 Responsiveness

- **The owner never waits on storage during a run.** `submit` is a short copy, and applying durability is a non-blocking read of the exchange.
- **The control thread** serves status, cancellation and the lease through `Control` (`core.rs:522-548`, `core.rs:616-623`) [S]. It adds a copy of the recorder status: what is pending and since when, `durable_through`, and the latches. No path waits on storage or holds a mutex across it.
- **Under blocked storage:**
  - in-flight native operations complete;
  - cleanup runs;
  - owners stay held;
  - admission waits for acknowledgements (§9.6);
  - `close()` refuses with `EvidencePending`;
  - the process does not exit.

### 14.2 Evidence stays data

- **No locators.** Records carry identities, instants, kinds and outcomes only (`codec.rs:40-73`) [S]: no PID, unit, cgroup, path or socket. Restart reconstructs no ownership, and decoding returns plain data (`codec.rs:89-93`) [S].
- **Settlement is a past statement.** A stored `ActionSettled{Confirmed}` or `IncidentSettled` is a past statement by a process that no longer exists. It is not fresh proof of cleanup.
- **A disposition** means "accepted without cleanup confirmation".
- **A seal**, in domain H, means "closed as recorded".

### 14.3 Failure retention

- Latches are never cleared, and a poisoned journal stays poisoned.
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
| `RunEnded` applied, not closed | Prefix with `RunEnded` | Same | Same | Unsealed; verdict reported | Refused if AS |
| `close()` succeeded, seal not written | No seal | No seal | No seal | Unsealed | Refused if AS (a disclosed false positive) |
| Seal written, not synced | Seal visible; made durable by the startup sync if that succeeds | Absent, unreadable or present | Absent unless synced | Sealed or unsealed | Permitted, or refused if AS |
| Seal write failed (EIO); process exited | Seal visible, and true in domain H | Absent | Absent | Sealed until eviction, then unsealed | Permitted; refused if AS once the seal is gone (a disclosed false positive) |
| Seal durable | Sealed | Sealed | Sealed | Sealed | Permitted |
| Late owner delivered after `RunEnded`; `IncidentOpened` not durable | Possibly visible | Absent | Absent | Unsealed, AS, recorded unsettled 0, native work possible | **Refused** [M: C09] |

### 15.2 Administrative

| Procedure | Crash point | Outcome at the next opening | Evidence retained | Recovery |
|---|---|---|---|---|
| Provisioning | Before `PROVISION`'s rename is durable | Unprovisioned: refused | None yet | The Owner may delete the incomplete root, which holds no history, and restart |
| Provisioning | After it | Fresh | — | — |
| `PROVISION` rewrite | Before the rename | The old `PROVISION` stays in force; a temporary file in its directory is never read | Old `PROVISION` | Owner completes or removes |
| `PROVISION` rewrite | After the rename, before the directory sync | Old or new `PROVISION` after F2 | Either complete version | The verifier decides; the Owner repeats if needed |
| Disposition | Before the temporary entry is unlinked | After F1: MaintenanceIncomplete. After F2: nothing published; the incident still blocks. | The incident | Owner |
| Disposition | After the unlink, before the directory sync | After F1: published. After F2: nothing published; the incident still blocks. | The incident; the disposition if it survives | The Owner re-checks |
| Disposition | After the directory sync | Published | The incident and its disposition | — |
| Revocation | Before the directory syncs | Effective; after F2 either not effective, or Invalid (both names, link count 2) | The disposition under one or both names | The verifier shows which; the Owner completes |
| Archival | Before the directory sync | After F1: MaintenanceIncomplete while the temporary entry exists, archived once it is unlinked. After F2: nothing archived. | Pool file unchanged | Owner |
| Archival | After the directory sync | Archived, pending recycling | Pool file and copy | — |
| Recycling | Before the rename is durable | After F1: MaintenanceIncomplete (temporary entry), or Lost (inode mismatch) once renamed. After F2: unchanged, pending recycling. | Archive copy, or the AbandonedClaim bytes | Owner |
| Recycling | After the directory sync, before the `PROVISION` rewrite is durable | Inode mismatch: refused as Lost | Archive copy | The Owner completes the rewrite |
| Retirement | Before or after the rename | Old or new K; the preconditions were verified | Archive | — |
| Successor | Before the new `PROVISION` is durable | The predecessor stays in force | Everything | Owner |

No crash point installs a partial file, honours a temporary entry, or retires undispositioned history [M: C12].

---

## 16. Verification: the design model now, Rust tests later

### 16.1 Design model (this mission) [M]

`docs/evidence/p2-v1-r3b-i3-p-r1/design_checks.py` uses only the Python standard library, with in-memory byte images and simulated state. It opens no store, calls no native operation, performs no privileged I/O and causes no real crash. It implements:
- the §4 state model: **K**, **D**, pending writeback, errseq with per-description cursors, eviction, F1, F2, and directory entries;
- the §5 containment profile, plus an unsupported profile for contrast;
- the §6–§7 formats, with a version-1 encoder and decoder that reproduce all 50 golden record vectors of `codec-tests` (`RECORD_VECTORS`, `codec-tests:121`) [S];
- the §8 lock model, with open-file-description semantics;
- the §9 exchange and worker, against a ledger model of `core.rs:1042-1090`;
- the §10 startup order, with an interaction log;
- the §11 classifier and grammar, and the §12 bindings;
- the §13 procedures, with crash injection.

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
| C12 | `[admin-crash]` | No administrative crash point installs partial state or retires unresolved history |
| C13 | `[grammar-conformance]` | Fixtures, prefixes and mutations against §11.4 |
| C14 | `[safe-open]` | Special files, ext4 identity, mount ambiguity |
| C15 | `[ack-durable]` | INV-1 and INV-4 under write and sync faults |
| C16 | `[no-false-resolution]` | INV-3 at every crash point of every fixture |
| C17 | `[arith-bounds]` | Claim exhaustion and numeric bounds |

**Negative controls.** There are 27 (NC01–NC17, with lettered variants), listed in the script and in `coverage.json`. They are in-memory mutants that restore an incorrect behaviour, such as the earlier candidate's. Each must fail its check's assertion carrying the check's marker. A syntax, import or fixture error never counts as a caught control. The output reports four things separately:
- baseline passes;
- intended negative-control failures;
- tool failures;
- assumptions the model cannot establish.

**Model simplifications,** each disclosed:
- directory descriptors and component-by-component `O_PATH` walks are not modelled; directories are addressed by reference;
- mutex discipline (no mutex across I/O) is a property for code review; the model's exchange performs no I/O by construction;
- a rename is modelled per directory rather than as an atomic cross-directory operation. This is more conservative than ext4's journal.
- the core is represented by the source-derived fixtures and a model of `Ledger::acknowledge` and `Ledger::fail`; the real core is not executed;
- the model's verifier stops at the first refusal, whereas the §13.11 contract requires a full report.

`reference_check.py` verifies two things against `45898e05`: every `file:line` reference in this document, and the golden-vector literals in the model.

**The model validates this specification's internal consistency and protocols. It does not execute, test or prove the Rust code, the kernel, ext4 or any device.**

### 16.2 Rust verification (later implementation mission)

- **Simulated storage.** A `StoreIo` that implements the §4 state model:
  - errseq, with per-description cursors sampled at open, and eviction;
  - F1, F2, and F1→F2;
  - tears within the 4096-byte unit;
  - short writes, zero-progress writes and EINTR;
  - directory entries.
- **Crash-point enumeration** over the real core with stand-ins, covering:
  - the T1–T15 scenarios, and recording failure at every record;
  - duplicate submission after a sink panic;
  - priors with dispositions;
  - pool exhaustion, abandoned claims and claim gaps;
  - stalls, recorder panic and conflicts.

### 16.3 Invariants for the Rust tests

The Rust tests must hold INV-1 to INV-12 in their domains (§12.1), plus **I9, conformance**: every journal that the real deterministic core runs produce must classify as non-Malformed. Those runs include `Run::every_kind` in `codec-tests` and the h18 adversarial sequences (`core-tests:5237`) [S].

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
| `support/custody/store/open.rs` | Safe open, mount identification, startup order (§10) |
| `support/custody/store/io.rs` | The `StoreIo` trait and its Linux implementation: `openat2` through `SYS_openat2` and `open_how`; `statx`; `fstatat`; `fstatvfs`; `flock`; `pwrite`; `fdatasync`; `getrandom` |
| `support/custody/store/exchange.rs` | The exchange, the sink and the owner's apply step (§9) |
| `support/custody/store/recorder.rs` | The worker: claim, append, seal, poison |
| `support/custody/store/disposition.rs` | The `DispositionValidator` |
| `support/custody/store/sim.rs` | Simulated storage (§16.2) |
| `support/custody/mod.rs` | `pub mod store;` and the integration-obligation documentation |
| `phase2_custody_store.rs` | A new test target |

**Unchanged:**
- `core.rs`, `model.rs` and `codec.rs`, and the NXCD version-1 bytes;
- production `src/` and the live harness;
- workflows, `Cargo.toml` and `Cargo.lock`.

**Dependencies.** None new: `libc` 0.2.183 (Linux), `sha2` 0.10.9 and `hex` 0.4.3 are already locked.

**Unsafe code.** A small, reviewed FFI surface in `io.rs`, with each call wrapped once.

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
| **G-HOST** | Owner qualification and provisioning on a named host (§13.2) |
| **G-NATIVE** | Native-layer guarantees for the residuals of §14.4 |
| **G-LIVE** | Integration of the owner process and service, and live validation |
| **G-PWR** | Empirical power-loss qualification, if it is ever required (D-10) |

### 18.2 Non-goals

This design does not:
- initialize, select or provision any root;
- touch the runner, systemd, buses or the host;
- run any live case;
- design the owner service or its transport;
- resist domain-A tampering, or introduce any anchor or privileged component;
- support XFS or any other filesystem;
- perform automatic maintenance;
- offer a force-close;
- migrate any format.

### 18.3 Non-claims

- **Nothing is implemented.** No store, file, directory, pool, `PROVISION`, lock or disposition exists.
- **Nothing ran against the Rust code.** The Python model and the reference check ran only on in-memory byte images and Git objects.
- **No qualification or proof.** There is no tamper resistance against the store uid, no power-loss proof, no filesystem or device qualification, no authentication, and no proof of native cleanup.
- **No completion.** There is no journal-completeness claim beyond §11, no live acceptance, no integration, and no Phase Two completion.
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
