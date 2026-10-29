// The studio's row in Options > Mods, with its key.
fn horizon_studio_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    mods.push(crate::game::hstudio_bridge::listed());
}
