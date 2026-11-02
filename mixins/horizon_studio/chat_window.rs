// The chat's lines over the HUD: not while a Roblox script hid the chat.
fn horizon_studio_chat_window(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if crate::game::hstudio_bridge::chat_hidden(game) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
