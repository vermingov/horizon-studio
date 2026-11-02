// The mouse's turn of the player: its movement reaches the scripts, and a
// script's action that takes it (a free camera's) turns no one.
fn horizon_studio_mouse(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    crate::game::hstudio_bridge::mouse_look(game)
}
