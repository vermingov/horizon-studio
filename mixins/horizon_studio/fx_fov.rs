// The Roblox Camera's FieldOfView: the view's field of view.
fn horizon_studio_fx_fov(value: f32, game: &crate::game::Game<'_>) -> f32 {
    crate::game::hstudio_bridge::field_of_view(value, game.settings.fov as i32 as f32)
}
