// Roblox's NetworkSettings.IncomingReplicationLag in the game (hstudio_lag):
// Infinite Yield's replicationlag (backtrack). The server's packets reach the
// game that many seconds after they arrive, all of them in the order they
// came, as a slow connection would bring them: the other players and the
// mobs are seen where they were then, and the server sees the client's
// answers (its keep-alives and pings too) that much later, as lag it knows.
// The lag-in hook (poll_remote's HEAD) gives the connection's events to
// `held`, and the game takes those whose time has come. Set to 0, what is
// held goes to the game at once; a connection that ends drops what is held.

use horizon_client::net::Event;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

thread_local! {
    static LAG: Cell<f64> = const { Cell::new(0.0) };
    static HELD: RefCell<VecDeque<(Instant, Event)>> = const { RefCell::new(VecDeque::new()) };
}

/// How long the server's packets are held, in seconds (0: not at all).
pub fn set(seconds: f64) {
    LAG.with(|l| l.set(if seconds.is_finite() { seconds.max(0.0) } else { 0.0 }));
}

/// The connection's events this frame, through the hold: None when nothing
/// is held back (the game polls the connection itself), else the events
/// whose time has come, in order, after taking in what `poll` brings.
pub fn held(poll: impl FnOnce() -> Vec<Event>) -> Option<Vec<Event>> {
    let lag = LAG.with(Cell::get);
    HELD.with(|h| {
        let mut h = h.borrow_mut();
        if lag <= 0.0 && h.is_empty() {
            return None;
        }
        let now = Instant::now();
        h.extend(poll().into_iter().map(|e| (now, e)));
        let wait = Duration::from_secs_f64(lag);
        let mut due = Vec::new();
        while let Some((at, _)) = h.front() {
            if lag > 0.0 && now.duration_since(*at) < wait {
                break;
            }
            if let Some((_, e)) = h.pop_front() {
                due.push(e);
            }
        }
        Some(due)
    })
}

/// The connection ended: what was held is dropped.
pub fn dropped() {
    HELD.with(|h| h.borrow_mut().clear());
}
