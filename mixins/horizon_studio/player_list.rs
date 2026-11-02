// The player list (Tab): not while a Roblox script hid it. The skin slots it
// shares with the players' heads still start a new frame.
fn horizon_studio_player_list(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if crate::game::hstudio_bridge::player_list_hidden(game) {
        game.flush_skin_slots();
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
