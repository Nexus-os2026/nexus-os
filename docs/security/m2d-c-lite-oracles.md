# M2d-C-lite: registered oracles and mutation plants

Base: `f8aa3204d3e70b28bd36fb46b44f36d22d58598f`.
Authority: the Owner's M2d-C-lite mission, `m2d-c-rescope-proposal.md` and
`m2d-c-lite-design-codex-review.md` (2026-11-04). The latter's complete matrix,
surviving obligations and six binding changes apply. This registration precedes
implementation. Test additions implementing these schedules precede the component
they judge. Earlier-PR frozen tests are unchanged.

| Group | Schedule and required observation | Uncommitted plants |
| --- | --- | --- |
| E: eligibility | A fresh File session consents, publishes real bytes and continues normally. Draft, AwaitingConsent and held competitors prevent exclusive admission. Race admissions: only one reservation wins. Drop and reopen on the same port or a rebuilt port over the broker refuses; the original cell halts. Emergency followed by lift still refuses. Terminal uncertainty never recycles eligibility. | Local-only freshness; reusable ownership; current-stop-only history; omit each active state. |
| O: provenance | Adopt a non-File action, then attempt File admission: refuse. A tagged File run cannot acquire another member, even before authorization. | Allow promotion; allow membership growth. |
| H: Stop ordering | Mission Stop, Owner Stop, budget, typed File Stop-all and emergency engage the original cell before any coordinator, port, registry, journal or owed-write wait. Hold append and owed write separately; poison before and between stages. A captured token's final check after engagement refuses. Recording and withdrawal errors reach the caller; a healthy touch discharges owed recording, preserving first cause. | Late engagement; omit attachment; omit token read; bypass each ingress; swallow each recording/withdrawal error; replace cause. |
| B: dispatch | Hold after lane reservation, before invocation. Stop then release: zero invocation. Engaged cell before File, inference and model-call admission means zero binding, attempt or inference. Check every later transition and dispatch boundary. | Bypass each boundary or shared admission check. |
| P: publication and D3 | Pending, Unknown, unreadable and uncertain failure settle uncertain after Stop; only confirmed nonpublication permits ordinary lapse/Stop. Clean publication continues. Interruption, recovery-required and both together produce truthful terminal evidence, preserving every interruption. Mother's earlier deadline withdraws before its one snapshot. Both ordinary ActionApproved and published settlement count as success in negative assertions. | Pending to lapse; failure to nonpublication; dropped interruption; false ordinary success after halt. |
| R: retention and ack | Both registries at capacity retain unacknowledged File commitments and runs; new work refuses. File A then B, failed ack retry, duplicate ack, wrong id and wrong owner never release another obligation. Unknown/orphaned uncertainty remains pinned. Admission retains the run and pins the commitment atomically or inserts nothing. | Unpinned insertion; eviction; early release; double release; wrong release; acknowledge uncertainty. |
| J: replay, emergency and close | Unresolved journaled File before durable start; start without terminal; terminal before ack. Validate action and first cause; refuse duplicate/conflicting records. A held unresolved File never resumes or bypasses settlement. Emergency without any Mother observation still stops. Refuse clean close for a nonterminal File or owed terminal recording. One indivisible terminal journal line. | Held-Stop bypass; ordinary success after halt; cause replacement; clean close while terminal owed. |
| D8: actual process death | Kill children after durable start; inside terminal append before write, before sync, after sync and with a torn line; inside fence and poison (also after durable start); after terminal before ack; lose in-flight broker state. Exact receipt for every boundary. A fresh broker's production kernel recovery report succeeds with exact expected findings in EVERY case, including torn journals; independently Mother refuses a torn journal. No uncertain ack. | Accept err recovery; skip torn recovery assertion; fake receipt; acknowledge uncertainty. |
| L: legacy controls | Frozen M1 real_broker.rs (including broker_refused:no_broker_route), m2a/m2as, m2c_crash, m2c_door, governed/halt_tests.rs, adapter/file_tests.rs and oracles/stop.rs remain byte-for-byte unchanged. Legacy Stop, adoption, rebuild and emergency lift retain their behavior. Test and production File eligibility are identical. Preserve #81. | Isolation and compatibility bypasses without editing the controls. |

Each plant must turn its oracle red, be reverted without rewriting history, and
never be committed. Logs live in the mission's `scratchpad/lite/plants/`.
Synchronization uses hooks, channels and bounded waits, never sleeps.

The Owner notice is exactly:

> A file may exist in your folder. A write already handed off may still finish. Mother will not retry or continue this mission.

Not claimed: cancellation after a final check can prevent that publication (C6);
restart restores workspace authority for a non-empty folder; automatic resolution
or retry of uncertainty (M2d-R is deferred).

Baseline observation (base production code, new oracle only):
`lite_r_unacknowledged_file_retains_its_run_at_capacity` fails at line 19:
`unacknowledged File run must be retained: capacity must refuse, got Ok(run-33b19801)`.
Result: 0 passed, 1 failed, 696 filtered out. The actual publication and byte
comparison passed before this assertion. Log: `scratchpad/lite/red-retention-baseline.log`.
