// The studio's row in Options > Mods, with its key, and Natrium's.
fn horizon_studio_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    mods.push(crate::game::hstudio_bridge::listed());
    mods.push(crate::game::hstudio_natrium::listed());
}
