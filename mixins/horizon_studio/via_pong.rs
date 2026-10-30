fn horizon_studio_via_pong(body: &[u8]) -> core::ops::ControlFlow<Result<()>> {
    // A pong the network thread already sent for the game (while it drew no
    // frames) is not sent again.
    if crate::hstudio_via_net::drop_pong(body, crate::ids::play::serverbound::PONG) {
        return core::ops::ControlFlow::Break(Ok(()));
    }
    core::ops::ControlFlow::Continue(())
}
