// The connection's events, held back while a Roblox script sets
// IncomingReplicationLag (hstudio_lag); the backlog takes those due.
fn horizon_studio_lag_in(game: &mut crate::game::Game<'_>) {
    let Some(r) = game.remote.as_mut() else {
        crate::game::hstudio_lag::dropped();
        return;
    };
    let Some(due) = crate::game::hstudio_lag::held(|| r.session.poll()) else {
        return;
    };
    for event in due {
        // As poll_remote starts a chunk's decoding as it comes.
        let item = match event {
            Event::Packet(p) if matches!(*p, C::LevelChunkWithLight(_)) => {
                match r.decoder.submit(r.chunk_format.clone(), p) {
                    None => Received::Chunk(r.chunk_format.clone()),
                    Some(p) => Received::Event(Event::Packet(p)),
                }
            }
            event => Received::Event(event),
        };
        r.backlog.push_back(item);
    }
}
