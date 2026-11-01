// Chat lines, for the voice chat's "incompatible version" answer.
fn horizon_studio_voice_chat(game: &crate::game::Game<'_>, content: &horizon_client::text::Text) {
    crate::game::hstudio_voice_game::chat_line(game, content);
}
