// The chat's completion requests: one for a script's command stays in the game.
fn horizon_studio_suggest(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if crate::game::hstudio_bridge::suggestion_request(game) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
