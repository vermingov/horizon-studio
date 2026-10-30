// A button of the studio's page in Options > Mods, or of Natrium's.
fn horizon_studio_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    if mod_id == crate::game::hstudio::ID {
        crate::game::hstudio_bridge::settings_click(game, button);
    } else if mod_id == crate::game::hstudio_natrium::ID {
        crate::game::hstudio_natrium::click(game, button);
    }
}
