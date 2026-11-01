// Each frame: the voice chat learns where everyone is, and sends what it has.
fn horizon_studio_voice_frame(game: &mut crate::game::Game<'_>) {
    crate::game::hstudio_voice_game::frame(game);
}
