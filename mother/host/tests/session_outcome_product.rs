//! Literal expected-outcome coverage. These are model cases, never claimed
//! as executed production sink schedules.
#![deny(unsafe_code)]
#[path = "session/model.rs"]
#[allow(dead_code)]
mod model;
#[path = "session/outcomes.rs"]
mod outcomes;

use model::{Answer, Mission, Record, CAUSES, MISSIONS};
use outcomes::{expected, Facts, ANSWERS, DISPATCH, HISTORIES};

#[test]
fn literal_product_preserves_history_first_intent_records_and_resource_debt() {
    let mut count = 0;
    for mission in MISSIONS {
        for cause in std::iter::once(None).chain(CAUSES.map(Some)) {
            for dispatch in DISPATCH {
                for (history, &(handed, ever, present, conclusive, recovery)) in
                    HISTORIES.iter().enumerate()
                {
                    for interruption_ordered in [false, true] {
                        for answer in ANSWERS {
                            for exact_terminal_record in [
                                Record::Owed,
                                Record::Recorded,
                                Record::Failed,
                                Record::Unknown,
                            ] {
                                for exact_retained_action in [false, true] {
                                    for resource_debt in [false, true] {
                                        for first in [
                                            None,
                                            Some(Mission::Done),
                                            Some(Mission::Stopped),
                                            Some(Mission::Denied),
                                            Some(Mission::Failed),
                                            Some(Mission::Blocked),
                                        ] {
                                            let result = expected(Facts {
                                                mission,
                                                cause,
                                                first,
                                                dispatch,
                                                history,
                                                interruption_ordered,
                                                answer,
                                                exact_terminal_record,
                                                exact_retained_action,
                                                resource_debt,
                                            });
                                            assert_eq!(result.ever_published, ever);
                                            assert_eq!(result.present, present);
                                            assert_eq!(result.recovery, recovery);
                                            assert_eq!(result.interrupted, interruption_ordered);
                                            assert_eq!(result.resource_debt, resource_debt);
                                            assert_eq!(result.uncertain, !conclusive);
                                            if let Some(first) = first {
                                                assert_eq!(result.terminal, Some(first));
                                            }
                                            if !exact_retained_action
                                                || !conclusive
                                                || exact_terminal_record != Record::Recorded
                                            {
                                                assert!(!result.may_ack);
                                            }
                                            if answer != Answer::Declined || handed {
                                                assert!(!result.owner_denied);
                                            }
                                            if cause.is_some()
                                                || interruption_ordered
                                                || recovery
                                                || first.is_some()
                                                || answer != Answer::Affirmed
                                            {
                                                assert!(!result.ordinary_approved);
                                            }
                                            count += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(count, 7_547_904);
    println!(
        "independent outcome product: {count} executed model cases; no production coverage claim"
    );
}
