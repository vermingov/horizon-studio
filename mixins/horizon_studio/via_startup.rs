fn horizon_studio_via_startup(value: (), loading: &mut super::Loading, batch: &mut horizon_client::hud::HudBatch) {
    // The startup menu's title screen: ViaHorizon's button there too.
    let studio_on = !loading.settings.mods.disabled.contains(crate::game::hstudio::ID);
    if let Some(gui) = &loading.gui {
        crate::game::hstudio_bridge::via_startup(
            &gui.atlas,
            batch,
            loading.menu.title.as_ref(),
            loading.menu.cursor,
            studio_on,
        );
    }
    value
}
