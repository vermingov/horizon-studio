// Roblox Lighting's FogEnd: fog from the camera to that many blocks, when
// it is nearer than the game's own.
fn horizon_studio_fx_fog(mut value: FogData) -> FogData {
    if let Some(end) = crate::game::hstudio_bridge::fog_end() {
        let end = end.max(1.0);
        if end < value.environmental_end {
            value.environmental_start = 0.0;
            value.environmental_end = end;
        }
        if end < value.render_distance_end {
            value.render_distance_start = 0.0;
            value.render_distance_end = end;
        }
    }
    value
}
