// A player's signed chat message: who sent it (their UUID) and what they
// wrote, with the line it makes, for the Roblox layer's chat.
fn horizon_studio_chat_signed(
    game: &crate::game::Game<'_>,
    sender: u128,
    content: &str,
    decorated: &horizon_client::text::Text,
) {
    crate::game::hstudio_bridge::player_chat(game, sender, content, decorated);
}
