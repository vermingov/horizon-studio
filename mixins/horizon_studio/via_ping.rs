fn horizon_studio_via_ping(reader: &mut ConnReader) -> core::ops::ControlFlow<Result<(i32, Vec<u8>)>> {
    // While the game draws no frames, the server's pings are answered here,
    // on the network thread, as keep-alives always are, after the ones the
    // game still owes. The reads this makes come through here again and go
    // straight on.
    if crate::hstudio_via_net::reading() {
        return core::ops::ControlFlow::Continue(());
    }
    let _reading = crate::hstudio_via_net::Reading::start();
    loop {
        let got = reader.recv();
        if let Ok((id, payload)) = &got {
            let ping = crate::ids::play::clientbound::PING;
            let pong = crate::ids::play::serverbound::PONG;
            if crate::hstudio_via_net::on_ping(*id, payload, ping, pong) {
                continue;
            }
        }
        return core::ops::ControlFlow::Break(got);
    }
}
