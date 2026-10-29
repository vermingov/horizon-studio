// view.zoom: the strongest zoom a script asks for, eased like the spyglass.
fn horizon_studio_zoom(zoom: &mut Option<crate::game::native_hooks::NativeZoom>) {
    if zoom.is_none()
        && let Some(factor) = crate::game::hstudio_bridge::zoom()
    {
        *zoom = Some(crate::game::native_hooks::NativeZoom {
            factor,
            smooth: true,
            turn: (1.0 / factor).max(0.2),
        });
    }
}
