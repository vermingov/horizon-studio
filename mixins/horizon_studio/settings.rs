// The studio's page in Options > Mods, and Natrium's.
fn horizon_studio_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == crate::game::hstudio::ID {
        crate::game::hstudio_bridge::settings(buttons);
    } else if mod_id == crate::game::hstudio_natrium::ID {
        crate::game::hstudio_natrium::page(game, buttons);
    }
}
