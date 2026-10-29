// The studio's page in Options > Mods.
fn horizon_studio_page(mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == crate::game::hstudio::ID {
        crate::game::hstudio_bridge::settings(buttons);
    }
}
