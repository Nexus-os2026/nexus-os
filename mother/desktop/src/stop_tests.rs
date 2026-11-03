//! Decision 2, findings 1 and 2: the Stop route's bounded join, on the
//! schedules production cannot force, through the route's test seams. The
//! Stop body here is a stand-in (one broker per process; the live tests
//! drive the real `emergency_stop()`); the join is what is judged.

use super::{Desk, Joined, Seams, StopRoute};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A seam that holds its thread until the test opens it.
#[derive(Clone)]
struct Hold {
    reached: Arc<Mutex<Option<mpsc::Sender<()>>>>,
    open: Arc<Mutex<mpsc::Receiver<()>>>,
}

impl Hold {
    /// The hold, the receiver of "a thread reached it", and the opener.
    fn new() -> (Self, mpsc::Receiver<()>, mpsc::Sender<()>) {
        let (reached_tx, reached_rx) = mpsc::channel();
        let (open_tx, open_rx) = mpsc::channel();
        (
            Self {
                reached: Arc::new(Mutex::new(Some(reached_tx))),
                open: Arc::new(Mutex::new(open_rx)),
            },
            reached_rx,
            open_tx,
        )
    }

    fn seam(&self) -> Box<dyn Fn() + Send + Sync> {
        let hold = self.clone();
        Box::new(move || {
            if let Some(reached) = hold.reached.lock().unwrap().take() {
                let _ = reached.send(());
                let _ = hold.open.lock().unwrap().recv();
            }
        })
    }
}

const WAIT: Duration = Duration::from_secs(10);

/// `route.join_within(within)` on its own thread; `None` if it did not
/// return within `WAIT` (an unbounded join).
fn join_bounded(route: &Arc<StopRoute>, within: Duration) -> Option<Joined> {
    let (tx, rx) = mpsc::channel();
    let route = route.clone();
    std::thread::spawn(move || {
        let _ = tx.send(route.join_within(within));
    });
    rx.recv_timeout(WAIT).ok()
}

#[test]
fn f1_a_worker_that_published_done_but_has_not_returned_is_still_running() {
    let (hold, reached, open) = Hold::new();
    let route = StopRoute::with_body(
        Desk::new(),
        || {},
        Seams {
            after_done: Some(hold.seam()),
            before_register: None,
        },
    );
    route.press();
    // The interval: `done` is published, the thread has not returned.
    reached
        .recv_timeout(WAIT)
        .expect("the worker published done");
    assert_eq!(route.receipts().finished, 1);
    let joined = join_bounded(&route, Duration::from_millis(200));
    assert_eq!(
        joined,
        Some(Joined::TimedOut { still_running: 1 }),
        "a join that waits past its bound, or reports an unreturned worker as joined"
    );
    open.send(()).unwrap();
    assert_eq!(
        join_bounded(&route, WAIT),
        Some(Joined::All { joined: 1 }),
        "the returned worker is joined"
    );
}
