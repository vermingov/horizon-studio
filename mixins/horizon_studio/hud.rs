// The HUD: the scripts' on("hud") handlers and hud.panel boxes.
fn horizon_studio_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    crate::game::hstudio_bridge::hud(game, batch, corners);
}
