// /studio, /lua and the scripts' own commands.
fn horizon_studio_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    if !*ran && crate::game::hstudio_bridge::command(game, command) {
        *ran = true;
    }
}
