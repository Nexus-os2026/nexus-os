# P2-V1-R3B-I4-R3-R1: negative controls and historical adaptations

Every control mutates the production sources of an isolated clean snapshot
of the candidate, compiles the library's unit tests, runs exactly its one
intended test (`--exact`), requires it to fail with the control's marker in
the failing assertion, and writes the mutated files back byte for byte
(SHA-256 verified, the checkout's Git status unchanged). The tests and the
model are never mutated. Results: `controls/` (summaries and run output),
the counts in `REPORT.md`.

## R3-R1 controls (`scripts/r3r1_controls.py`, mission section 19)

| Control | Mutation | Intended test | Marker |
|---|---|---|---|
| NC-R3R1-01-MEMBERSHIP-OWNS | helper membership alone grants cleanup ownership: the candidate settling retains is owned at once | `r3r1_02` | the replacement's cgroup was owned through the helper's membership |
| NC-R3R1-02-KILL-AT-OPEN | the candidate is killed as soon as it is opened | `r3r1_04` | a cgroup not bound to the captured identity was ended |
| NC-R3R1-03-LOST-COLLISION-OWNS | lost `UnitExists` + helper placement become cleanup authority (retained without an identity, owned when uncertain) | `r3r1_01` | the foreign unit was killed |
| NC-R3R1-04-LOST-COLLISION-PROVEN | lost `UnitExists` + matching manager properties become Proven (the uncertain start goes on to the proof, which takes the unit's later identity) | `r3r1_01` | the foreign cgroup was launched into |
| NC-R3R1-05-IDENTITY-MISMATCH-IGNORED | a replacement's (another) `InvocationID` is ignored | `r3r1_04` | launched into an unbound cgroup |
| NC-R3R1-06-LATER-UNBOUND-IDENTITY | the identity captured merely from an unbound later read by the unit's name | `r3r1_02` | the replacement was launched into |
| NC-R3R1-07-MALFORMED-IDENTITY-ACCEPTED | a wrong-length or all-zero (malformed) identity accepted | `scope::tests::r3r1_05_…` | a wrong, zero or malformed identity was accepted |
| NC-R3R1-08-UNCERTAIN-GRANTED-IDENTITY | an uncertain start granted an identity | `r3r1_07` | an uncertain start was launched into |
| NC-R3R1-09-ONE-FLAG | cleanup ownership and scope authority collapsed into one flag (owned only once every policy is proven) | `r3r1_06` | the owned candidate was not ended through its descriptor |
| NC-R3R1-10-OWNED-BEFORE-PROOF | a panic sets ownership before the proof (the proof's candidate owned as it is opened) | `r3r1_09` | a cgroup was ended before it was bound |
| NC-R3R1-11-RECONSTRUCTED-FROM-PATH | a candidate descriptor reconstructed from its path after loss (a removed candidate reopened, keeping its ownership) | `r3r1_02` | a removed owned candidate was not confirmed through its descriptor |

## Historical controls (mission section 20)

Each accepted runner (and each R3 adapter) is run unchanged, its SHA-256
verified before it is imported; the first, unadapted compatibility run on
the candidate is kept (`controls/compat-run1/`). R3-R1's adapters wrap R3's
adapters, which run unchanged: R3's adaptations (supersessions, retargets)
stay exactly as recorded, and R3-R1 changes only what its own code moved,
just before the accepted runner's main runs. No control is deleted; the
unadapted outcome of each adapted control is recorded below.

| Suite (accepted) | Runner as accepted | Unadapted on the candidate | R3-R1 adaptation |
|---|---|---|---|
| R3 controls (10) | `r3_controls.py` | stops at NC-R3-01: its anchor (settling's observation call with R3's comment) is gone; 7 controls anchor on changed code | `accepted_r3_controls_r3r1.py`: NC-R3-01, -03, -05 inserted between R3-R1's `end_owned()` and the observation; NC-R3-04 after R3-R1's `acquire` head, retaining an owned `Candidate`; NC-R3-06 the same weakening of `self.accepted && absent(...)`; NC-R3-08 on R3-R1's uncertain arm; NC-R3-09 the by-name kill replacing `end_owned()`. Same tests and markers; 02, 07, 10 unchanged |
| R2 controls (10; 5 superseded by R3) | `accepted_r2_controls_r3.py` → `r2_controls.py` | stops at NC-R2-03: R3's insertion point is gone; 4 controls affected | `accepted_r2_controls_r3r1.py`: NC-R2-03, -08 before R3-R1's `end_owned()`; NC-R2-05 drops `self.accepted` from R3-R1's absence rule; NC-R2-X follows R3-R1's observed `Candidate` retention. Same tests and markers |
| Q1-R1 controls (15; 2 superseded by R3) | `accepted_r1_controls_r3.py` → `r1_controls.py` | anchors all present; 12 of 13 pass; Q1-R1-NC1-FOREIGN-KILLED is no longer detected: its mutation (`acquire()` in the collided arm) is harmless, since R3-R1's `acquire` only observes and only with a captured identity | `accepted_r1_controls_r3r1.py`: the same harm by an equivalent mutation in the same place (a cgroup of the unit's name the kernel reports the helper in, opened and ended through its descriptor, as `acquire` did until R3). Same anchor, test and marker |
| Q1 controls (10) | `q1_controls.py` | all 10 pass | none |
| I4 and I4-R1 controls (38; 4 superseded and 1 retargeted by R3, 1 R2 adaptation kept) | `accepted_scope_controls_r3.py` → `scope_controls.py` | stops at NC-I4-START-TIMEOUT-ABSENT; 10 controls affected | `accepted_scope_controls_r3r1.py`: NC-I4-START-TIMEOUT-ABSENT retargeted (its test asserted an uncertain start's unit is discovered and proven, which R3-R1 forbids; the renamed `i4_03` asserts it stays owned; marker "cleanup reported confirmed without proof"); NC-I4-START-NAME-AUTH, NC-I4-MEMBERSHIP-NAME-ONLY on R3-R1's process-list check; NC-I4-NO-EARLY-DIR an equivalent mutation (a failed proof drops the candidate: a candidate kept only after the proof could never be bound); NC-I4-X-NO-CANDIDATE-KILL empties `end_owned()`; NC-I4-X-COLLISION-CLAIMED an equivalent mutation (its replacement no longer type-checks, and a collision has no identity: the collision arm adopts the loaded unit's identity); NC-I4R1-NO-CONTROLGROUP, -CONTROLGROUP-BASENAME, -X-CONTROLGROUP-UNCERTAIN in R3-R1's `bind`; NC-I4R1-ABSENCE-GETUNIT drops `self.accepted` from the absence rule |
| I4-R1 source guards (14, R3: SG-I4-DESTINATION adapted, SG-I4R3-NO-NAME-ACTUATION added) | `accepted_source_guards_r3.py` → `source_guards.py` | stops at a self-test anchor (`group == path`) | `accepted_source_guards_r3r1.py`: SG-I4-DESTINATION, SG-I4-ONE-WAY, SG-I4R1-BINDING, SG-I4R1-UNCERTAIN, SG-I4R3-NO-NAME-ACTUATION keep their invariants at R3-R1's code; SG-R3R1-CANDIDATE-OWNERSHIP added; two self-tests re-anchored, twelve added (41 in all) |
| I4-R1 API/type guards (27) | `api_guards.py` | stops at P-I4-PENDING-CANDIDATE's reopen anchor; P-I4-SCOPE-FORGE also affected | `accepted_api_guards_r3r1.py`: the candidate probe's reopen at `Option<Candidate>` (and `Candidate` made public, as `CgroupDir` was); the forge probe names `Scope`'s three fields, the same privacy error for the three |
| I4-R1 normal-build API guards (9) | `normal_api_probes.py` | as accepted | none |

The custody hashes, the R3 store controls (179), the I2-R1 controls (32 +
4) and R3's API probes (28) rerun with their accepted runners unchanged
(`scripts/accepted_reruns.sh`).
