//! M2c-A (design v3 §7.1): the test doors' confirmer. There is no blanket
//! always-yes confirmer (N1): every test door is bound to [`Delegating`],
//! which forwards each dialog to the confirmer a helper installed for that
//! one call, on that call's thread. With nothing installed it answers no
//! and counts an unexpected dialog ([`unexpected`]).
//!
//! The call runs on its own thread, as before. The installed confirmer is
//! borrowed by the helper, and safe Rust cannot lend a borrow to a
//! thread-local, so the helper answers the dialogs on a scoped thread of
//! its own while the call waits for each answer: the dialog is still
//! synchronous (the asking call holds no lock and does nothing until it is
//! answered). Test code lives here so that the source scans keep
//! confirmers out of the adapter's files.

use crate::adapter::m2c_support::{HeldOwner, HeldShown};
use nexus_governed_control::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, AgentDoorConfirmation, ControlConfirmer,
    GrantConfirmation, MissionConfirmation, MissionResumeConfirmation, ResumeConfirmation,
};
use std::cell::{Cell, RefCell};
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;

/// One dialog, as the delegating confirmer was asked it.
enum Dialog {
    Action(ActionConfirmation),
    Grant(GrantConfirmation),
    Resume(ResumeConfirmation),
    Acknowledgement(AcknowledgementConfirmation),
    Mission(MissionConfirmation),
    Door(AgentDoorConfirmation),
    Held(MissionResumeConfirmation),
}

struct Ask {
    dialog: Dialog,
    answer: Sender<bool>,
}

thread_local! {
    /// The confirmers installed on this thread, innermost last.
    static INSTALLED: RefCell<Vec<Sender<Ask>>> = const { RefCell::new(Vec::new()) };
    /// Dialogs asked on this thread with nothing installed.
    static UNEXPECTED: Cell<usize> = const { Cell::new(0) };
}

/// The confirmer bound into every test door (and the recording port).
pub(crate) struct Delegating;

/// A fresh handle on the delegating confirmer.
pub(crate) fn delegating() -> Arc<Delegating> {
    Arc::new(Delegating)
}

/// Dialogs asked on this thread with nothing installed (answered no).
pub(crate) fn unexpected() -> usize {
    UNEXPECTED.with(Cell::get)
}

impl Delegating {
    fn ask(&self, dialog: Dialog) -> bool {
        let Some(installed) = INSTALLED.with(|installed| installed.borrow().last().cloned()) else {
            UNEXPECTED.with(|count| count.set(count.get() + 1));
            return false;
        };
        let (answer, answered) = channel();
        installed
            .send(Ask { dialog, answer })
            .expect("the installed confirmer is there for the whole call");
        // The installed confirmer panicked while asked: so does the dialog.
        answered
            .recv()
            .expect("the installed confirmer panicked while asked")
    }
}

impl ControlConfirmer for Delegating {
    fn confirm_action(&self, request: &ActionConfirmation) -> bool {
        self.ask(Dialog::Action(request.clone()))
    }
    fn confirm_grant(&self, request: &GrantConfirmation) -> bool {
        self.ask(Dialog::Grant(request.clone()))
    }
    fn confirm_resume(&self, request: &ResumeConfirmation) -> bool {
        self.ask(Dialog::Resume(request.clone()))
    }
    fn confirm_acknowledgement(&self, request: &AcknowledgementConfirmation) -> bool {
        self.ask(Dialog::Acknowledgement(request.clone()))
    }
    fn confirm_mission(&self, request: &MissionConfirmation) -> bool {
        self.ask(Dialog::Mission(request.clone()))
    }
    fn confirm_agent_door(&self, request: &AgentDoorConfirmation) -> bool {
        self.ask(Dialog::Door(request.clone()))
    }
    fn confirm_held_mission(&self, request: &MissionResumeConfirmation) -> bool {
        self.ask(Dialog::Held(request.clone()))
    }
}

/// Run `call` on this thread with `confirmer` answering every dialog the
/// delegating confirmer is asked on this thread meanwhile. A panic in the
/// call or in a dialog is resumed here.
pub(crate) fn answering<T>(
    confirmer: &(dyn ControlConfirmer + Sync),
    call: impl FnOnce() -> T,
) -> T {
    let (installed, asks) = channel::<Ask>();
    std::thread::scope(|scope| {
        let dialogs = scope.spawn(move || {
            for ask in asks {
                let answer = match &ask.dialog {
                    Dialog::Action(r) => confirmer.confirm_action(r),
                    Dialog::Grant(r) => confirmer.confirm_grant(r),
                    Dialog::Resume(r) => confirmer.confirm_resume(r),
                    Dialog::Acknowledgement(r) => confirmer.confirm_acknowledgement(r),
                    Dialog::Mission(r) => confirmer.confirm_mission(r),
                    Dialog::Door(r) => confirmer.confirm_agent_door(r),
                    Dialog::Held(r) => confirmer.confirm_held_mission(r),
                };
                let _ = ask.answer.send(answer);
            }
        });
        INSTALLED.with(|stack| stack.borrow_mut().push(installed));
        let result = catch_unwind(AssertUnwindSafe(call));
        // The last sender goes: the dialog thread ends.
        INSTALLED.with(|stack| stack.borrow_mut().pop());
        let dialogs = dialogs.join();
        match (dialogs, result) {
            (Err(panic), _) | (Ok(()), Err(panic)) => resume_unwind(panic),
            (Ok(()), Ok(value)) => value,
        }
    })
}

/// The Owner's answer to the door dialog; every other dialog no.
pub(crate) struct DoorAnswer(pub bool);
impl ControlConfirmer for DoorAnswer {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        false
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        false
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
    fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
        self.0
    }
}

/// The Owner at the held-mission dialog, as a [`HeldOwner`] says: keeps
/// what it was shown, runs its hook while asked, answers its `yes`; every
/// other dialog no.
pub(crate) struct HeldConfirmer<'a>(pub &'a HeldOwner);
impl ControlConfirmer for HeldConfirmer<'_> {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        false
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        false
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
    fn confirm_held_mission(&self, request: &MissionResumeConfirmation) -> bool {
        self.0.show(HeldShown {
            mission: request.mission.clone(),
            contract_hash: request.contract_hash.clone(),
            request: request.request.clone(),
            held_reason: request.held_reason.clone(),
        });
        self.0.during.fire();
        self.0.yes
    }
}
