// options.txt gets the player's own settings, not those Roblox scripts
// changed for the session (hstudio_kept): the volume, the graphics, the
// chat's background, full screen.
fn horizon_studio_settings_save(settings: &Settings, path: &Path) -> core::ops::ControlFlow<()> {
    if hstudio_kept::save_own(settings, path) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
