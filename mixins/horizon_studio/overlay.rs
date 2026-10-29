// The last thing drawn in a world: the studio's screen, while it is open.
fn horizon_studio_overlay(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    crate::game::hstudio_bridge::overlay(game, batch);
}
