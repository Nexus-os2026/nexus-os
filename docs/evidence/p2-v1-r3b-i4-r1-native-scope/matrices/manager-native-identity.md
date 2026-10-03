# P2-V1-R3B-I4-R1: manager / native identity

Every value the Pending to Proven transition reads, where it comes from,
what it is, and what a failure of it yields. A LOCATOR never becomes
authority by equality alone: the proven scope's authority is the retained
descriptor, bound to the manager's unit by the exact `ControlGroup` and
`Id` the manager itself reports for the object GetUnit returned. Citations
are to `crates/nexus-verifier-sandbox/src/`.

## The values, in proof order (`scope/pending.rs:301-363`)

| # | Value | Source | Class | Required | A failure yields | Tests, controls |
|---|---|---|---|---|---|---|
| 1 | unit name `nexus-verifier-<32 hex>.scope` | the backend's random nonce (`scope.rs:177-189`) | LOCATOR | — | — | `i4_16`, NC-I4-START-NAME-AUTH |
| 2 | helper process id | the retained, unreaped child | LOCATOR | — | — | `i4_30` |
| 3 | helper identity (serial) | allocated before the spawn, checked, non-wrapping (`launcher.rs:192-197`, `launcher.rs:220`) | RETAINED HELPER IDENTITY | equal to the bound operation's (`scope/pending.rs:150-152`) | another helper's observations are ignored | `i4_a_pending_operation_is_bound_to_its_own_helper`, `i4r1_20`, `i4r1_21` |
| 4 | kernel membership path | `/proc/<pid>/cgroup`'s unified line (`scope/native.rs:50-54`) | LOCATOR | absolute, normal form (no empty, `.` or `..` component), last component the unit's name (`scope/pending.rs:420-425`) | not placed: `NotPlaced` (or the start's own error) | `i4_15`, `i4r1_x_a_membership_path_not_in_normal_form_never_locates_a_candidate`, NC-I4R1-X-MEMBERSHIP-NOT-NORMAL |
| 5 | candidate descriptor | the directory opened from that path, retained at once (`scope/pending.rs:311`) | RETAINED NATIVE CGROUP | cgroup v2; populated; `cgroup.procs` lists the helper; exact limits | `Mismatch` / `Io`; the candidate stays cleanup ownership | `i4_04`, `i4_10`, `i4_12`, NC-I4-NO-EARLY-DIR |
| 6 | unit object path | GetUnit on the generated name (`scope/pending.rs:327-334`) | LOCATOR of the manager's object | present | `Mismatch("unit not loaded")`, or `Bus` when uncertain | `i4_11`, NC-I4-X-UNLOADED-PROVEN |
| 7 | `Id` (`org.freedesktop.systemd1.Unit`) | `Get` at that object (`scope/manager.rs:282-293`) | MANAGER-BOUND UNIT IDENTITY | byte-equal to the generated name (`scope/pending.rs:340-344`) | `Mismatch("unit id")`; `Bus` when uncertain or unavailable; never a launch | `i4r1_09`, `i4r1_17`, NC-I4R1-ID-MISMATCH |
| 8 | `ControlGroup` (`org.freedesktop.systemd1.Scope`) | `Get` at that object (`scope/manager.rs:295-306`) | MANAGER-BOUND UNIT IDENTITY: the binding | byte-equal to the kernel membership path the candidate was opened from (`scope/pending.rs:345-349`); never canonicalized | `Mismatch("unit control group")`; `Bus` when uncertain or unavailable; never a launch | `i4r1_06`, `i4r1_07`, `i4r1_08`, `i4r1_10`, `i4r1_17`, NC-I4R1-NO-CONTROLGROUP, -CONTROLGROUP-BASENAME, -CONTROLGROUP-MISMATCH, -X-CONTROLGROUP-UNCERTAIN |
| 9 | `RuntimeMaxUSec`, `OOMPolicy` | `Get` at that object | DATA | exact | `Mismatch` / `Bus` | `i4_13`, `i4_14`, NC-I4-PROPERTY-ERROR-DROPS |
| 10 | the proven `Scope` | the candidate moved in, in place (`scope/pending.rs:485-502`) | PROVEN SCOPE AUTHORITY | every row above | — | `i4_01`, `i4r1_10` |

## What is refused (section 9)

| Disagreement | Example (model) | Result | Test |
|---|---|---|---|
| another slice, the same basename | the kernel: `/other.slice/<unit>`; the manager: `<slice>/<unit>` | `Mismatch("unit control group")`; the found cgroup (cleanup ownership) ended and confirmed empty | `i4r1_06` |
| a deeper cgroup of the same name | `<slice>/nested.slice/<unit>` | refused | `i4r1_06` |
| a path that merely ends with the name | `<slice>/x-<unit>` | refused | `i4r1_07` |
| empty, relative, root, trailing slash, `//`, `.`, `..` | `""`, `<unit>`, `/`, `<slice>/<unit>/`, `<slice>//<unit>`, `<slice>/./<unit>`, `<slice>/../app.slice/<unit>` | refused (paths are compared byte for byte, never canonicalized) | `i4r1_07` |
| a child cgroup | `<slice>/<unit>/child` | refused | `i4r1_07` |
| a property that is unavailable, uncertain or not a string | timeout, error, malformed, disconnect; a value of another type | `Bus` / `Mismatch`; no launch | `i4r1_08`, `i4r1_09`, `i4r1_17` |
| an `Id` other than the name | `other.scope`, `x<unit>`, `<unit>.alias`, `""` | `Mismatch("unit id")` | `i4r1_09` |
| a membership not in normal form | `<slice>/./<unit>`, `<slice>//<unit>`, `<unit>` | never a candidate: `NotPlaced` | `i4r1_x_a_membership_path_not_in_normal_form_never_locates_a_candidate` |
| a collision whose foreign cgroup has the name and the limits, and whose `Id`/`ControlGroup` would match | `UnitExists` | refused before any proof; the foreign unit never opened, stopped or claimed | `i4r1_16`, `i4r1_15` |

`ControlGroupId` (systemd 251+, listed but not described in the manual) is
not used. The comparison assumes the backend and the user manager see the
same cgroup namespace; a backend that sees other paths proves nothing
(fail closed). Both remain for G-HOST/G-LIVE.
