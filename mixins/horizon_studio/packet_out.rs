// The game's serverbound packets: through the scripts' __namecall hook (as
// remote:FireServer) while there is one, which may stop them.
fn horizon_studio_packet_out(
    game: &crate::game::Game<'_>,
    packet: &horizon_protocol::packets::play::Serverbound,
) -> core::ops::ControlFlow<()> {
    if crate::game::hstudio_bridge::packet_out(game, packet) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
