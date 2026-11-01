// Keys for the voice chat: push to talk, mute and deafen.
fn horizon_studio_voice_key(game: &crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::hstudio_voice_game::key(game, key, pressed);
}
