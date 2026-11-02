// The wheel while playing: past the hotbar's ends it goes into the slots of
// the scripts' Roblox tools (hstudio_rbx_tools), which then keep it.
fn horizon_studio_wheel(game: &mut crate::game::Game<'_>, steps: f32, claimed: &mut bool) {
    if !*claimed && crate::game::hstudio_bridge::wheel(game, steps) {
        *claimed = true;
    }
}
