// Packets from the server: voice chat's payloads, and the Login that starts it.
fn horizon_studio_voice_packet(
    game: &mut crate::game::Game<'_>,
    packet: &horizon_protocol::packets::play::Clientbound,
) {
    crate::game::hstudio_voice_game::packet(game, packet);
}
