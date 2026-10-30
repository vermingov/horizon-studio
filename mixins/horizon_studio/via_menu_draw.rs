fn horizon_studio_via_menu(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    // After the title screen: ViaHorizon's button on the multiplayer list.
    crate::game::hstudio_bridge::via_menu(game, batch);
}
