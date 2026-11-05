//! Frozen C1a oracles. No broker/model transport; the caller only charges.
//! Heuristic predicate: a trimmed line is exactly one of the four PEM BEGIN
//! markers below, OR a whitespace-delimited token is `sk-` plus >=20 ASCII
//! alphanumerics. Applied to labels and text. No normalization/case folding.
//! This is deliberately a heuristic, not a secret-absence guarantee.
//! Plants: E15 -> o30; E16 -> o30c_generation_only (does not clear state);
//! E21 -> o31; E22 -> o30a_prepaid (internal invariant registered explicitly);
//! pending omission -> o30; replay entitlement -> o33b; settlement -> s4_work.
use super::model_calls::{reserve_lane, ModelMeter};
use super::*;
use crate::contract::{ModelChoice, Note};
use crate::journal::MemoryJournal;
use crate::oracles::{
    contract, keyed_lanes, test_private, world, world_with, RecordingPort, World,
};
use nexus_governed_control::inference::{CallMeter, MeterRefusal};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Barrier,
};

fn model() -> ModelChoice {
    ModelChoice {
        name: "qwen3.6:35b-a3b".into(),
        digest: "ab".repeat(32),
        identity: "cd".repeat(32),
    }
}
fn note(text: &str) -> Note {
    Note {
        label: "notes".into(),
        text: text.into(),
    }
}
fn ready(budget: u32) -> (World, MissionId) {
    let w = world();
    let mut c = contract();
    c.budget.max_model_calls = budget;
    let id = w.approved(c);
    w.mother.advance(id).unwrap(); // Start only: no scripted charge.
    (w, id)
}
fn reserve(
    w: &World,
    id: MissionId,
    cap: u32,
) -> Result<ModelMeter<RecordingPort>, CoordinatorError> {
    let generation = w.mother.peek(|i| i.missions[&id].generation);
    let identity = w.lanes[0].identity();
    let input = w
        .mother
        .step(|i| reserve_lane(i, id, generation, &identity, cap, &*w.clock))?;
    let Then::Go(input) = input else {
        return Err(CoordinatorError::Busy);
    };
    Ok(ModelMeter {
        coordinator: Arc::downgrade(&w.mother),
        invocation: input.invocation,
        generation,
    })
}
fn totals(w: &World, id: MissionId) -> u32 {
    w.mother.peek(|i| i.missions[&id].model_calls)
}
fn returned(w: &World, meter: &ModelMeter<RecordingPort>, overtake: bool) {
    w.mother
        .step(|i| {
            let event = if overtake {
                Event::LaneOvertaken {
                    invocation: meter.invocation,
                }
            } else {
                Event::LaneReturned {
                    invocation: meter.invocation,
                    output_hash: "ef".repeat(32),
                }
            };
            i.append(meter.invocation.mission, event)?;
            let m = i.mission(meter.invocation.mission)?;
            m.pending_lane = None;
            m.lane_calls = None;
            m.call_accounting = None;
            Ok(())
        })
        .unwrap();
}
fn replayed(w: &World) -> BTreeMap<MissionId, Mission> {
    let mut ms = BTreeMap::new();
    let mut next = 1;
    let mut session = Session::Never;
    for e in verify(&w.journal.lines()).unwrap() {
        replay(&mut ms, &mut next, &mut session, &e).unwrap();
    }
    ms
}
fn chain(events: Vec<Event>) -> Vec<String> {
    let mut head = GENESIS.to_owned();
    events
        .into_iter()
        .enumerate()
        .map(|(n, event)| {
            let id = if matches!(event, Event::SessionOpened | Event::SessionClosed) {
                SESSION
            } else {
                MissionId(1)
            };
            let e = Entry::chained(n as u64, &head, id, event);
            head = e.hash.clone();
            e.to_line()
        })
        .collect()
}
fn prefix(cap: u32, budget: u32) -> Vec<Event> {
    let c = contract();
    let h = c.hash();
    vec![
        Event::SessionOpened,
        crate::oracles::drafted(&c),
        Event::Transition {
            trigger: Trigger::Submit { hash: h },
        },
        Event::Transition {
            trigger: Trigger::Consent {
                hash: h,
                at_wall_ms: 0,
            },
        },
        Event::Transition {
            trigger: Trigger::Start,
        },
        Event::LaneInvoked {
            invocation: InvocationId {
                mission: MissionId(1),
                seq: 5,
            },
            role: LaneRole::ResearcherA,
            identity_sha256: "ab".repeat(32),
            input_hash: "cd".repeat(32),
            model_cap: cap,
            model_budget: budget,
        },
    ]
}
fn charge(call: u32) -> Event {
    Event::ModelCallCharged {
        invocation: InvocationId {
            mission: MissionId(1),
            seq: 5,
        },
        call,
    }
}

#[test]
fn e_o30_cost_order_and_zero_send() {
    for calls in 0..=4 {
        let (w, id) = ready(4);
        let meter = reserve(&w, id, 4).unwrap();
        assert_eq!(
            w.mother.peek(|i| i.missions[&id].pending_lane),
            Some(meter.invocation),
            "explicit live pending install"
        );
        assert!(matches!(
            verify(&w.journal.lines()).unwrap().last().unwrap().event,
            Event::LaneInvoked {
                model_cap: 4,
                model_budget: 4,
                ..
            }
        ));
        for k in 1..=calls {
            assert_eq!(meter.charge().unwrap().call, k);
            assert!(
                matches!(verify(&w.journal.lines()).unwrap().last().unwrap().event, Event::ModelCallCharged { call, .. } if call == k),
                "charge durable before return"
            );
            meter.settle(k);
        }
        assert_eq!(totals(&w, id), calls.max(1));
        assert_eq!(replayed(&w)[&id].model_calls, totals(&w, id));
        returned(&w, &meter, false);
        assert_eq!(replayed(&w)[&id].model_calls, calls.max(1));
        assert!(w.port.effects().is_empty());
    }
}
#[test]
fn e_o30a_prepaid_is_single_use() {
    let (w, id) = ready(2);
    let m = reserve(&w, id, 2).unwrap();
    assert_eq!(m.charge().unwrap().call, 1);
    assert!(
        !w.mother
            .peek(|i| i.missions[&id].call_accounting.as_ref().unwrap().prepaid),
        "prepaid consumed invariant"
    );
    assert_eq!(m.charge(), Err(MeterRefusal::InFlight));
    m.settle(99);
    assert_eq!(m.charge(), Err(MeterRefusal::InFlight));
    m.settle(1);
    m.settle(1);
    assert_eq!(m.charge().unwrap().call, 2);
    assert_eq!(totals(&w, id), 2);
    m.settle(1);
    assert_eq!(m.charge(), Err(MeterRefusal::InFlight));
    m.settle(2);
    assert_eq!(m.charge(), Err(MeterRefusal::Exhausted));
    assert_eq!(totals(&w, id), 2);
}
#[test]
fn e_o30b_concurrent_single_flight() {
    let (w, id) = ready(2);
    let m = Arc::new(reserve(&w, id, 2).unwrap());
    let b = Arc::new(Barrier::new(3));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let m = m.clone();
            let b = b.clone();
            std::thread::spawn(move || {
                b.wait();
                m.charge()
            })
        })
        .collect();
    b.wait();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| **r == Err(MeterRefusal::InFlight))
            .count(),
        1
    );
    m.settle(1);
    assert_eq!(m.charge().unwrap().call, 2);
}
#[test]
fn e_o30c_generation_only_unmasked() {
    let (w, id) = ready(2);
    let m = reserve(&w, id, 2).unwrap();
    w.mother
        .with_lock(|i| i.missions.get_mut(&id).unwrap().generation += 1);
    assert_eq!(
        m.charge(),
        Err(MeterRefusal::Stale),
        "generation check without clearing lane_calls"
    );
}
#[test]
fn e_o30c_retained_meters_and_settlement() {
    for end in [
        "return",
        "overtake",
        "transition",
        "stop",
        "fence",
        "close",
        "intent",
        "held",
    ] {
        let (w, id) = ready(4);
        let m = reserve(&w, id, 2).unwrap();
        m.charge().unwrap();
        m.settle(1);
        match end {
            "return" | "overtake" => returned(&w, &m, end == "overtake"),
            "transition" => {
                w.mother
                    .step(|i| i.fire(id, Trigger::PhaseDone, 0))
                    .unwrap();
            }
            "stop" => {
                w.mother.stop(id).unwrap();
            }
            "fence" => w.mother.with_lock(|i| {
                i.broken = true;
                let owed = (i.fence)();
                i.owed.extend(owed);
            }),
            "close" => {
                w.mother.close_ref().unwrap();
            }
            "intent" => w.mother.with_lock(|i| {
                i.missions.get_mut(&id).unwrap().terminal_record = Some((
                    Trigger::Stop {
                        reason: StopReason::Owner,
                    },
                    None,
                ));
            }),
            "held" => w.mother.with_lock(|i| {
                i.missions.get_mut(&id).unwrap().held = Some(HeldReason::UncleanShutdown)
            }),
            _ => unreachable!(),
        }
        assert_eq!(m.charge(), Err(MeterRefusal::Stale), "{end}");
        m.settle(1);
        assert_eq!(m.charge(), Err(MeterRefusal::Stale));
        assert_eq!(totals(&w, id), 1);
    }
    let (w, id) = ready(4);
    let old = reserve(&w, id, 2).unwrap();
    old.charge().unwrap();
    returned(&w, &old, false);
    let new = reserve(&w, id, 2).unwrap();
    new.charge().unwrap();
    old.settle(1);
    assert_eq!(
        new.charge(),
        Err(MeterRefusal::InFlight),
        "old settlement cannot reopen new invocation"
    );
}
// Probe the actual journal/fence boundary while the coordinator mutex is
// still held. Reading the retained latch must not acquire that mutex.
struct LivenessProbeJournal {
    journal: MemoryJournal,
    probe: Option<Box<dyn FnOnce() + Send>>,
}
impl JournalStore for LivenessProbeJournal {
    fn load(&self) -> Result<Vec<String>, JournalError> {
        self.journal.load()
    }
    fn append(&mut self, line: &str) -> Result<(), JournalError> {
        if let Some(probe) = self.probe.take() {
            probe();
        }
        self.journal.append(line)
    }
    fn poison(&mut self) -> Result<(), JournalError> {
        self.journal.poison()
    }
}

#[test]
fn e_o30d_liveness_revoked_before_boundary_release() {
    for boundary in [
        "return",
        "overtake",
        "transition",
        "intent",
        "fence",
        "close",
    ] {
        let (w, id) = ready(4);
        let meter = reserve(&w, id, 2).unwrap();
        let latch = w.mother.peek(|i| {
            i.missions[&id]
                .lane_calls
                .as_ref()
                .unwrap()
                .liveness
                .clone()
        });
        assert!(!latch.is_revoked(), "fresh invocation starts live");
        meter.charge().unwrap();
        meter.settle(1);
        assert!(
            !latch.is_revoked(),
            "charge and settlement do not revoke or establish continuing liveness"
        );
        let seen = Arc::new(Mutex::new(None));
        let observation = {
            let mother = w.mother.clone();
            let latch = latch.clone();
            let seen = seen.clone();
            move || {
                let lock_still_held = mother.inner.try_lock().is_err();
                let revoked = latch.is_revoked();
                *seen.lock().unwrap() = Some((revoked, lock_still_held));
            }
        };
        if boundary == "fence" {
            w.mother.with_lock(|i| {
                let fence = std::mem::replace(&mut i.fence, Box::new(Owed::default));
                i.fence = Box::new(move || {
                    observation();
                    fence()
                });
            });
            w.journal.fail_after(0);
            assert_eq!(meter.charge(), Err(MeterRefusal::NotRecorded));
        } else {
            w.mother.with_lock(|i| {
                i.store = Box::new(LivenessProbeJournal {
                    journal: w.journal.clone(),
                    probe: Some(Box::new(observation)),
                });
            });
            match boundary {
                "return" | "overtake" => returned(&w, &meter, boundary == "overtake"),
                "transition" => {
                    w.mother
                        .step(|i| i.fire(id, Trigger::PhaseDone, 0))
                        .unwrap();
                }
                // Append only: terminal intent has begun, but apply has NOT
                // run. Clearing the latch only in apply is too late.
                "intent" => {
                    w.mother
                        .step(|i| {
                            i.append(
                                id,
                                Event::Transition {
                                    trigger: Trigger::Stop {
                                        reason: StopReason::Owner,
                                    },
                                },
                            )
                        })
                        .unwrap();
                }
                "close" => w.mother.close_ref().unwrap(),
                _ => unreachable!(),
            }
        }
        assert_eq!(
            *seen.lock().unwrap(),
            Some((true, true)),
            "revocation precedes boundary release: {boundary}"
        );
        assert!(
            latch.is_revoked(),
            "retained latch stays denied: {boundary}"
        );
        assert_eq!(totals(&w, id), 1, "invalidation does not refund");
        if matches!(boundary, "return" | "overtake") {
            let new_meter = reserve(&w, id, 2).unwrap();
            let new_latch = w.mother.peek(|i| {
                i.missions[&id]
                    .lane_calls
                    .as_ref()
                    .unwrap()
                    .liveness
                    .clone()
            });
            assert!(!new_latch.is_revoked());
            assert!(
                latch.is_revoked(),
                "new invocation never reactivates old latch"
            );
            assert_eq!(new_meter.charge().unwrap().call, 1);
        }
    }
}

#[test]
fn e_o31_full_cap_and_invalid_caps() {
    for cap in [0, 1, 2, 4] {
        let (w, id) = ready(cap.max(1));
        let m = reserve(&w, id, cap).unwrap();
        assert_eq!(totals(&w, id), 1);
        if cap == 0 {
            assert_eq!(m.charge(), Err(MeterRefusal::Stale));
        } else {
            assert_eq!(m.charge().unwrap().call, 1);
        }
    }
    let (w, id) = ready(1);
    assert!(reserve(&w, id, 2).is_err(), "entire cap must fit");
    assert_eq!(totals(&w, id), 0);
    let (w, id) = ready(20);
    assert!(reserve(&w, id, 5).is_err());
    assert_eq!(totals(&w, id), 0);
}
#[test]
fn e_o33_tampered_valid_chains() {
    for (cap, budget, calls) in [
        (0, 0, vec![1]),
        (0, 2, vec![]),
        (5, 10, vec![]),
        (2, 1, vec![]),
        (2, 2, vec![1, 1]),
        (2, 2, vec![2]),
        (2, 2, vec![1, 2, 3]),
        (2, 0, vec![]),
    ] {
        let mut es = prefix(cap, budget);
        es.extend(calls.into_iter().map(charge));
        assert!(inspect_history_lines(&chain(es)).is_err(), "{cap}/{budget}");
    }
    for ending in [
        None,
        Some(Event::LaneReturned {
            invocation: InvocationId {
                mission: MissionId(1),
                seq: 5,
            },
            output_hash: "ab".repeat(32),
        }),
        Some(Event::Transition {
            trigger: Trigger::Stop {
                reason: StopReason::Owner,
            },
        }),
        Some(Event::LaneOvertaken {
            invocation: InvocationId {
                mission: MissionId(1),
                seq: 5,
            },
        }),
    ] {
        let mut es = prefix(2, 2);
        if let Some(e) = ending {
            es.push(e)
        } else {
            es.pop();
        }
        es.push(charge(1));
        assert!(inspect_history_lines(&chain(es)).is_err());
    }
    let mut es = prefix(2, 2);
    es.push(charge(1));
    es.push(charge(2));
    es.push(Event::LaneReturned {
        invocation: InvocationId {
            mission: MissionId(1),
            seq: 5,
        },
        output_hash: "ab".repeat(32),
    });
    es.push(Event::LaneInvoked {
        invocation: InvocationId {
            mission: MissionId(1),
            seq: 9,
        },
        role: LaneRole::ResearcherA,
        identity_sha256: "ab".repeat(32),
        input_hash: "ab".repeat(32),
        model_cap: 1,
        model_budget: 2,
    });
    assert!(
        inspect_history_lines(&chain(es)).is_err(),
        "aggregate overflow"
    );
    for k in 0..=2 {
        let mut es = prefix(2, 2);
        es.extend((1..=k).map(charge));
        assert!(inspect_history_lines(&chain(es)).is_ok());
    }
    let mut es = prefix(2, 2);
    es.push(Event::ModelCallCharged {
        invocation: InvocationId {
            mission: MissionId(1),
            seq: 4,
        },
        call: 1,
    });
    assert!(inspect_history_lines(&chain(es)).is_err());
}
// Correctly re-chained histories, including the review's exact close/reopen attack.
#[test]
fn e_o33c_session_boundaries_reject_charge() {
    for boundary in ["close", "return", "overtake", "transition"] {
        let mut es = prefix(2, 2);
        es.push(charge(1));
        match boundary {
            "close" => es.extend([Event::SessionClosed, Event::SessionOpened]),
            "return" => es.push(Event::LaneReturned {
                invocation: InvocationId {
                    mission: MissionId(1),
                    seq: 5,
                },
                output_hash: "ef".repeat(32),
            }),
            "overtake" => es.push(Event::LaneOvertaken {
                invocation: InvocationId {
                    mission: MissionId(1),
                    seq: 5,
                },
            }),
            "transition" => es.push(Event::Transition {
                trigger: Trigger::PhaseDone,
            }),
            _ => unreachable!(),
        }
        assert!(
            inspect_history_lines(&chain(es.clone())).is_ok(),
            "valid boundary prefix: {boundary}"
        );
        es.push(charge(2));
        let lines = chain(es);
        let journal = MemoryJournal::new();
        journal.set_lines(lines.clone());
        for result in [inspect_history_lines(&lines), inspect_history(&journal)] {
            assert!(
                matches!(
                    result,
                    Err(JournalError::Inconsistent {
                        code: ReplayViolation::ModelChargeOutOfPlace,
                        ..
                    })
                ),
                "report-only rejects post-invalidation charge: {boundary}"
            );
        }
    }
}

#[test]
fn e_o33d_session_boundaries_retain_evidence() {
    for clean in [true, false] {
        let mut es = prefix(2, 2);
        es.push(charge(1));
        if clean {
            es.push(Event::SessionClosed);
        }
        es.push(Event::SessionOpened);
        let mut ms = BTreeMap::new();
        let mut next = 1;
        let mut session = Session::Never;
        for entry in verify(&chain(es)).unwrap() {
            replay(&mut ms, &mut next, &mut session, &entry).unwrap();
        }
        let m = &ms[&MissionId(1)];
        assert_eq!(m.model_calls, 1, "charges never refunded");
        assert_eq!(m.declared_model_budgets, vec![2]);
        assert_eq!(
            m.pending_lane,
            Some(InvocationId {
                mission: MissionId(1),
                seq: 5
            }),
            "pending settlement evidence retained"
        );
        assert!(m.lane_calls.is_none(), "replay has no live latch or meter");
        assert!(
            m.call_accounting.is_none(),
            "session boundary invalidates accounting: clean={clean}"
        );
    }
}

#[test]
fn e_o33b_closed_budget_equality_and_no_entitlement() {
    let (w, id) = ready(4);
    let m = reserve(&w, id, 2).unwrap();
    m.charge().unwrap();
    returned(&w, &m, false);
    let m = reserve(&w, id, 2).unwrap();
    m.charge().unwrap();
    let ms = replayed(&w);
    assert!(
        ms[&id].lane_calls.is_none(),
        "replay never restores executable entitlement"
    );
    assert_eq!(ms[&id].declared_model_budgets.len(), 2);
    assert!(inspect_history_lines(&w.journal.lines()).is_ok());
    for earlier in [true, false] {
        let entries = verify(&w.journal.lines()).unwrap();
        let mut es: Vec<_> = entries.into_iter().map(|e| e.event).collect();
        let indices: Vec<_> = es
            .iter()
            .enumerate()
            .filter_map(|(i, e)| matches!(e, Event::LaneInvoked { .. }).then_some(i))
            .collect();
        if let Event::LaneInvoked { model_budget, .. } = &mut es[indices[usize::from(!earlier)]] {
            *model_budget = 5;
        }
        let journal = MemoryJournal::new();
        for line in chain(es) {
            journal.clone().append(&line).unwrap();
        }
        assert!(
            inspect_history(&journal).is_ok(),
            "report-only does not claim private contract equality"
        );
        assert!(
            matches!(
                Coordinator::open(
                    journal,
                    w.port.clone(),
                    Lanes::new(keyed_lanes()).unwrap(),
                    w.clock.clone(),
                    test_private()
                ),
                Err(CoordinatorError::Journal(JournalError::Inconsistent {
                    code: ReplayViolation::ModelBudgetMismatch,
                    ..
                }))
            ),
            "earlier and current budget compared to loaded contract"
        );
    }
}
#[test]
fn e_o41_consent_hash_and_redacted_diagnostics() {
    let w = world();
    let mut c = contract();
    c.model = Some(model());
    c.notes = vec![note("C1_NOTE_CANARY\n面包🍞")];
    c.deliverable = Some("index.html".into());
    let id = w.approved(c.clone());
    let s = w.mother.confirmation(id).unwrap();
    let lines = s.lines().join("\n");
    for part in [
        "C1_NOTE_CANARY",
        "面包🍞",
        "qwen3.6:35b-a3b",
        &"ab".repeat(32),
        &"cd".repeat(32),
        "File Mother may draft: index.html",
        "model calls (each lane call counts as at least one)",
    ] {
        assert!(lines.contains(part), "complete quoted consent");
    }
    assert!(lines.contains("│ C1_NOTE_CANARY\n│ 面包🍞"));
    for debug in [format!("{c:?}"), format!("{s:?}"), format!("{:?}", c.notes)] {
        assert!(!debug.contains("C1_NOTE_CANARY"));
    }
    let asked_id = w.submitted(c.clone());
    let request = w.mother.consent_request(asked_id).unwrap();
    let cached = w.mother.peek(|i| i.missions[&asked_id].asking.clone());
    let text = request.0.text();
    assert!(request.0.message().contains("C1_NOTE_CANARY"));
    assert!(
        text.plain().contains("面包🍞"),
        "native display stays complete"
    );
    for debug in [
        format!("{:?}", request.0),
        format!("{:#?}", request.0),
        format!("{request:?}"),
        format!("{request:#?}"),
        format!("{cached:?}"),
        format!("{cached:#?}"),
        format!("{text:?}"),
        format!("{text:#?}"),
    ] {
        assert!(
            !debug.contains("C1_NOTE_CANARY"),
            "native diagnostics redact notes"
        );
        assert!(
            !debug.contains("面包"),
            "native diagnostics redact Unicode notes"
        );
    }
    for field in 0..6 {
        let mut changed = c.clone();
        match field {
            0 => changed.model.as_mut().unwrap().name.push('a'),
            1 => changed.model.as_mut().unwrap().digest = "ef".repeat(32),
            2 => changed.model.as_mut().unwrap().identity = "ef".repeat(32),
            3 => changed.notes[0].text.push('a'),
            4 => changed.notes[0].label.push('a'),
            _ => changed.deliverable = None,
        };
        assert_ne!(c.hash(), changed.hash());
        assert!(!w
            .mother
            .peek(|i| i.missions[&id].consent.as_ref().unwrap().covers(&changed)));
    }
    assert!(!w.journal.lines().join("\n").contains("C1_NOTE_CANARY"));
    // Old canonical bytes and hashes are frozen independently in the companion fixture.
    let old = contract();
    assert_eq!(
        serde_json::to_string(&old).unwrap(),
        r#"{"owner_words":"  Make me a small website for my bakery, \"Rosa's\".\n  Keep it simple!  ","restatement":"A one-page site for Rosa's bakery","requested_folder":"~/Sites/rosas-bakery","budget":{"max_duration_ms":3600000,"max_model_calls":50,"max_money_minor":0},"forbidden_effects":[],"questions":[{"id":"q1","text":"Which colours do you like?","answer":"Blue and cream"}],"pass_criteria":[{"id":"p1","text":"The page names the bakery"}]}"#
    );
    assert_eq!(
        old.hash().to_hex(),
        "f7301d50be4f0027a09717beb500d1bac79b1a72fa0ab6b77b6fedcd4e9161c7"
    );
    let e = Entry::chained(5, GENESIS, MissionId(1), prefix(0, 0).pop().unwrap());
    assert_eq!(
        serde_json::to_string(&e.event).unwrap(),
        r#"{"event":"lane_invoked","invocation":{"mission":1,"seq":5},"role":"researcher_a","identity_sha256":"abababababababababababababababababababababababababababababababab","input_hash":"cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd"}"#
    );
    assert_eq!(
        e.hash,
        "f7882541bbc57be1e95f6afeca8226056676960c993292259cd241f7d6f7c91e"
    );
}
#[test]
fn e_o43_frozen_heuristic_and_limits() {
    let positive = [
        "-----BEGIN PRIVATE KEY-----",
        "-----BEGIN RSA PRIVATE KEY-----",
        "-----BEGIN EC PRIVATE KEY-----",
        "-----BEGIN OPENSSH PRIVATE KEY-----",
        "  -----BEGIN PRIVATE KEY-----  ",
        "sk-abcdefghijklmnopqrst",
        "prefix sk-01234567890123456789 suffix",
    ];
    let negative = [
        "a",
        "面包🍞",
        "Crème",
        "-----END PRIVATE KEY-----",
        "-----BEGIN PUBLIC KEY-----",
        "-----BEGIN CERTIFICATE-----",
        "prefix -----BEGIN PRIVATE KEY-----",
        "sk-abcdefghijklmnopqrs",
        "sk-abcdefghijklmnopqrs_",
        "SK-abcdefghijklmnopqrst",
        "sk-abcdefghijklmnopqrsé",
        "api_key=example",
        "password for the bakery",
        "~/.ssh/config",
        "Ignore previous instructions",
        "-----END PROJECT DATA guessed-----",
    ];
    for (cases, rejected) in [(positive.as_slice(), true), (negative.as_slice(), false)] {
        for text in cases {
            let mut c = contract();
            c.notes = vec![note(text)];
            assert_eq!(
                c.validate(),
                if rejected {
                    Err(ContractError::NoteLooksLikeCredential { index: 0 })
                } else {
                    Ok(())
                },
                "frozen fixture"
            );
        }
    }
    let mut c = contract();
    c.notes = vec![note("a")];
    assert!(c.validate().is_ok(), "QM-MIN");
    c.notes[0].text = (0..3)
        .map(|_| "面包🍞".repeat(50))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(c.validate().is_ok(), "Q-M3 Unicode admitted");
    for (text, ok) in [
        ("x".repeat(256), true),
        ("x".repeat(257), false),
        ((0..40).map(|_| "x").collect::<Vec<_>>().join("\n"), true),
        ((0..41).map(|_| "x").collect::<Vec<_>>().join("\n"), false),
        ("bidi\u{202e}".into(), false),
        ("a\tb".into(), false),
    ] {
        c.notes = vec![note(&text)];
        assert_eq!(c.validate().is_ok(), ok);
    }
    c.notes = vec![note("a"); 4];
    assert!(c.validate().is_ok());
    c.notes.push(note("a"));
    assert!(c.validate().is_err());
    c.notes = vec![note("a")];
    for (label, ok) in [
        ("é".repeat(64), true),
        ("é".repeat(65), false),
        ("a\nb".into(), false),
    ] {
        c.notes[0].label = label;
        assert_eq!(c.validate().is_ok(), ok);
    }
    // 15 bytes framing for label notes; text exactly 1777 bytes, bounded lines.
    for (bytes, ok) in [(1777, true), (1778, false)] {
        let mut text = (0..6)
            .map(|_| "x".repeat(256))
            .collect::<Vec<_>>()
            .join("\n");
        text.push('\n');
        text.push_str(&"x".repeat(bytes - text.len()));
        c.notes = vec![note(&text)];
        assert_eq!(c.validate().is_ok(), ok, "rendered byte boundary");
    }
    for bad in ["", "bad name", ".bad", "é"] {
        c.notes.clear();
        c.model = Some(model());
        c.model.as_mut().unwrap().name = bad.into();
        assert!(c.validate().is_err());
    }
    for bad in ["AB".repeat(32), "g".repeat(64), "a".repeat(63)] {
        c.model = Some(model());
        c.model.as_mut().unwrap().identity = bad;
        assert!(c.validate().is_err());
    }
    c.model = Some(model());
    c.deliverable = Some("other.html".into());
    assert!(c.validate().is_err());
}
struct ModelLane(AtomicUsize);
impl Lane for ModelLane {
    fn identity(&self) -> LaneIdentity {
        LaneIdentity {
            role: LaneRole::Builder,
            name: "test-model".into(),
            version: "1".into(),
        }
    }
    fn max_model_calls(&self) -> u32 {
        2
    }
    fn invoke(&self, _: &LaneInput) -> crate::lane::LaneOutput {
        self.0.fetch_add(1, Ordering::SeqCst);
        panic!("production model invocation forbidden in C1a")
    }
}
#[test]
fn c1a_production_admission_closed() {
    let model = Arc::new(ModelLane(AtomicUsize::new(0)));
    let mut lanes = keyed_lanes();
    lanes.retain(|l| l.identity().role != LaneRole::Builder);
    lanes.push(model.clone());
    let w = world_with(lanes);
    let mut c = contract();
    c.model = Some(self::model());
    c.deliverable = Some("index.html".into());
    let id = w.mother.draft(c).unwrap();
    assert_eq!(
        w.mother.submit(id),
        Err(CoordinatorError::Contract(
            ContractError::ModelLaneUnavailable
        ))
    );
    assert_eq!(model.0.load(Ordering::SeqCst), 0);
    assert!(w.port.effects().is_empty());
}
#[test]
fn c1_s4_append_failures_and_live_replay() {
    for successful in 0..=2 {
        let (w, id) = ready(4);
        let m = if successful == 0 {
            None
        } else {
            Some(reserve(&w, id, 2).unwrap())
        };
        if successful == 2 {
            m.as_ref().unwrap().charge().unwrap();
            m.as_ref().unwrap().settle(1);
        }
        w.journal.fail_after(0);
        if let Some(m) = m {
            assert_eq!(m.charge(), Err(MeterRefusal::NotRecorded));
            assert_eq!(m.charge(), Err(MeterRefusal::Stale));
        } else {
            assert!(reserve(&w, id, 2).is_err());
        }
        assert_eq!(totals(&w, id), u32::from(successful > 0));
        assert_eq!(replayed(&w)[&id].model_calls, totals(&w, id));
        assert!(w.port.effects().is_empty());
    }
}
#[test]
fn c1_s4_work_not_settlement() {
    // Work/settlement classification is itself a security invariant. A real
    // session regression remains in E-O44; its filesystem fixture is unavailable
    // under this sandbox's parent-ownership policy (baseline receipt).
    assert!(journal_work(&charge(1)), "charge is work, never settlement");
    assert!(!journal_work(&Event::LaneOvertaken {
        invocation: InvocationId {
            mission: MissionId(1),
            seq: 5
        }
    }));
}

#[derive(Clone)]
struct Uncertain {
    journal: MemoryJournal,
    fail: Arc<std::sync::atomic::AtomicBool>,
}
impl JournalStore for Uncertain {
    fn load(&self) -> Result<Vec<String>, JournalError> {
        self.journal.load()
    }
    fn append(&mut self, line: &str) -> Result<(), JournalError> {
        self.journal.append(line)?;
        if self.fail.swap(false, Ordering::SeqCst) {
            Err(JournalError::Store(StoreFailure::Injected))
        } else {
            Ok(())
        }
    }
    fn poison(&mut self) -> Result<(), JournalError> {
        self.journal.poison()
    }
}
#[test]
fn c1_s4_uncertain_append_never_grants_a_charge() {
    for stage in 0..=2 {
        let (w, id) = ready(4);
        let meter = if stage == 0 {
            None
        } else {
            Some(reserve(&w, id, 2).unwrap())
        };
        if stage == 2 {
            let m = meter.as_ref().unwrap();
            m.charge().unwrap();
            m.settle(1);
        }
        w.mother.with_lock(|i| {
            i.store = Box::new(Uncertain {
                journal: w.journal.clone(),
                fail: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            })
        });
        if let Some(m) = meter {
            assert_eq!(m.charge(), Err(MeterRefusal::NotRecorded));
            assert_eq!(m.charge(), Err(MeterRefusal::Stale));
        } else {
            assert!(reserve(&w, id, 2).is_err());
        }
        assert!(
            w.mother.peek(|i| i.broken),
            "uncertain write fences, never retries"
        );
        assert!(w.journal.is_poisoned());
        assert_eq!(
            replayed(&w)[&id].model_calls,
            if stage == 2 { 2 } else { 1 },
            "durable prefix counts uncertain work once; it supplies no live entitlement"
        );
        assert!(w.port.effects().is_empty());
    }
}

#[test]
fn e_o41b_held_resume_request_redacts_notes() {
    let w = world();
    let mut c = contract();
    c.notes = vec![note("C1_RESUME_NOTE_CANARY 面包🍞")];
    let id = w.approved(c);
    w.mother.with_lock(|i| {
        i.missions.get_mut(&id).unwrap().held = Some(HeldReason::UncleanShutdown);
    });
    let request = w.mother.resume_request(id).unwrap();
    let cached = w.mother.peek(|i| i.missions[&id].resume_asking.clone());
    assert!(
        request.0.message().contains("C1_RESUME_NOTE_CANARY 面包🍞"),
        "resume display remains complete"
    );
    for debug in [
        format!("{:?}", request.0),
        format!("{:#?}", request.0),
        format!("{request:?}"),
        format!("{request:#?}"),
        format!("{cached:?}"),
        format!("{cached:#?}"),
    ] {
        assert!(
            !debug.contains("C1_RESUME_NOTE_CANARY"),
            "held resume diagnostics redact notes"
        );
        assert!(
            !debug.contains("面包"),
            "held resume diagnostics redact Unicode notes"
        );
    }
}
