// ViaHorizon's part of the game's network code that reaches the server
// connection's writer (a private field of Session): it is lent to the
// network thread, which answers the server's pings with it while the game
// draws no frames (`horizon_protocol::hstudio_via_net::answer_ping`).

use std::cell::Cell;

thread_local! {
    /// The writer lent, by address, so it is lent once.
    static LENT: Cell<usize> = const { Cell::new(0) };
}

/// Lends the session's writer, or takes it back (`None`: no server).
pub fn lend(session: Option<&super::Session>) {
    let at = session.map_or(0, |s| std::sync::Arc::as_ptr(&s.writer) as usize);
    if LENT.with(Cell::get) == at {
        return;
    }
    LENT.with(|l| l.set(at));
    match session {
        Some(s) => {
            let writer = s.writer.clone();
            horizon_protocol::hstudio_via_net::set_pong(move |body| {
                let mut w = writer.lock().unwrap_or_else(|e| e.into_inner());
                w.as_mut().is_some_and(|w| w.send(body).is_ok())
            });
        }
        None => horizon_protocol::hstudio_via_net::clear_pong(),
    }
}
