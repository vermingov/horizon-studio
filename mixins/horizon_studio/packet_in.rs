// The server's packets: each fires its remote's OnClientEvent while a script listens.
fn horizon_studio_packet_in(
    game: &crate::game::Game<'_>,
    packet: &horizon_protocol::packets::play::Clientbound,
) {
    crate::game::hstudio_bridge::packet_in(game, packet);
}
