//! Narrow fault and interleaving injection for the store's tests
//! (P2-V1-R3B-I3-I1-R1).
//!
//! Every operation here acts on a [`StoreOwner`] that [`super::owner::start_owner`]
//! built, through the store's own transitions, and can only make the
//! recorder fail closed or interleave its real steps:
//! - [`latch`] is monotonic: the first cause and P stay, a later cause is
//!   counted, nothing is cleared.
//! - [`poison`] poisons the exchange mutex, as a panic in a critical section
//!   would.
//! - [`spawn_worker_with_hooks`] runs the owner's own worker on a thread with
//!   hooks that can pause it or make it unwind at a point. Hooks decide
//!   nothing and change no state.
//! - [`vanish_worker`] ends the worker's thread without running it and
//!   without recording an exit.
//! - [`submit_invalid`] submits through the real sink only an intent the
//!   sink refuses, latches on, or already holds. It refuses to inject a
//!   record the sink would store as the next one.
//! - [`panic_next_submission`] makes the next flush's sink panic after
//!   storing, which the core contains as a recorder fault.
//! - [`acknowledge_durable_again`] acknowledges to the core, out of band, a
//!   record that is already durable and not yet applied. The store's own
//!   apply step then meets an unexpected outcome. It never acknowledges a
//!   record that is not durable.
//!
//! What this interface never offers: another constructor of an owner, a
//! worker, a recorder or an exchange; a way to point a worker at another
//! file; a write of `durable_through`, the claim state or the seal state; a
//! way to clear a latch; and any closure, seal or disposition. It is a safe
//! API boundary, not a defence against `unsafe` code.

use std::sync::Arc;

use super::super::{AckOutcome, RecordAck, RecordIntent, RecordSink, Resource, Tick};
use super::exchange::{stores_as_new, Cause};
use super::open::Platform;
use super::owner::{StoreOwner, WorkerSlot};
use super::recorder::WorkerHooks;

/// Why an injection was refused: it would not have been a fault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotAFault(pub &'static str);

/// Latch `cause`, as the store would on that fault. Monotonic.
pub fn latch<R: Resource, P: Platform + 'static>(owner: &StoreOwner<R, P>, cause: Cause) {
    owner.recorder.exchange().latch(cause);
}

/// Poison the exchange mutex.
pub fn poison<R: Resource, P: Platform + 'static>(owner: &StoreOwner<R, P>) {
    owner.recorder.exchange().poison();
}

/// Run the owner's own worker on a thread, with `hooks` called before each
/// step. `false` if the worker already runs on a thread.
pub fn spawn_worker_with_hooks<R: Resource, P: Platform + 'static>(
    owner: &mut StoreOwner<R, P>,
    hooks: Arc<dyn WorkerHooks>,
) -> bool {
    owner.spawn_with(Some(hooks))
}

/// End the worker's thread without running the worker and without
/// recording an exit: a vanished worker, which the next apply step latches.
/// `false` if the worker already runs on a thread.
pub fn vanish_worker<R: Resource, P: Platform + 'static>(owner: &mut StoreOwner<R, P>) -> bool {
    let WorkerSlot::Stepped(worker) = std::mem::replace(&mut owner.worker, WorkerSlot::Threaded)
    else {
        return false;
    };
    // The thread drops the worker without stepping it: no exit is recorded
    // and no drop guard is armed. Wait (bounded) until it has ended, so the
    // owner sees an ended thread with no exit recorded.
    let handle = super::exchange::WorkerHandle::new(std::thread::spawn(move || drop(worker)));
    for _ in 0..5_000 {
        if handle.is_finished() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let finished = handle.is_finished();
    owner.recorder.attach_worker(handle);
    finished
}

/// Submit `intent` through the owner's real sink, if the sink would refuse
/// it, latch on it, or recognize it as a duplicate. An intent the sink
/// would store as the next record is refused: that would not be a fault.
pub fn submit_invalid<R: Resource, P: Platform + 'static>(
    owner: &mut StoreOwner<R, P>,
    intent: &RecordIntent,
) -> Result<(), NotAFault> {
    {
        let held = owner.recorder.exchange().acquire();
        if stores_as_new(&held, intent) {
            return Err(NotAFault("the sink would store it as the next record"));
        }
    }
    owner.recorder.sink().submit(intent);
    Ok(())
}

/// The next flush's sink panics after storing its first record.
pub fn panic_next_submission<R: Resource, P: Platform + 'static>(owner: &mut StoreOwner<R, P>) {
    owner.panic_next_submission = true;
}

/// Acknowledge record `seq` to the core out of band, as a second recorder
/// would, only if it is already durable and not yet applied: the store's own
/// apply step then meets an unexpected outcome.
pub fn acknowledge_durable_again<R: Resource, P: Platform + 'static>(
    owner: &mut StoreOwner<R, P>,
    seq: u64,
    now: Tick,
) -> Result<AckOutcome, NotAFault> {
    let durable = owner.recorder.status().durable_through;
    if seq == 0 || seq > durable {
        return Err(NotAFault("the record is not durable"));
    }
    if seq <= owner.recorder.applied_through() {
        return Err(NotAFault("the record is already applied"));
    }
    let Some(intent) = owner.recorder.retained().get((seq - 1) as usize).cloned() else {
        return Err(NotAFault("no such record"));
    };
    Ok(owner.custody.acknowledge(&RecordAck::of(&intent), now))
}
