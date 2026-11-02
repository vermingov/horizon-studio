// The view where a Roblox script's Camera puts it (detached, on another
// entity, around a part), ahead of the game's own.
fn horizon_studio_camera(
    game: &mut crate::game::Game<'_>,
    partial: f32,
) -> core::ops::ControlFlow<Option<(glam::DVec3, f32, f32)>> {
    crate::game::hstudio_bridge::camera_view(game, partial)
}
