// The world frame's visual lights: those Roblox scripts put in the world.
fn horizon_studio_visual_lights(
    game: &crate::game::Game<'_>,
    lights: &mut Vec<crate::game::native_hooks::NativeColoredLight>,
) {
    crate::game::hstudio_bridge::visual_lights(game, lights);
}
