// The player's own model as Roblox scripts make the local character: posed,
// spun, its limbs still, its parts marked for hiding.
fn horizon_studio_body_tick(value: (), game: &mut crate::game::Game<'_>) {
    crate::game::hstudio_bridge::body_tick(game);
    value
}
