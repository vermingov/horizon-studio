// Chat lines from the server, for the scripts' on("chat") handlers.
fn horizon_studio_chat(
    game: &crate::game::Game<'_>,
    content: &horizon_client::text::Text,
    source: &horizon_client::chat::ChatSource,
) {
    crate::game::hstudio_bridge::chat_line(game, content, source);
}
