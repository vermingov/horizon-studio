// The player's own model, unless Roblox scripts took the whole character away.
fn horizon_studio_body_shown(value: bool, game: &crate::game::Game<'_>) -> bool {
    value && crate::game::hstudio_bridge::body_whole(game)
}
