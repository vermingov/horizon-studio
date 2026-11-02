fn horizon_studio_pack_override(
    vanilla: &std::path::Path,
    selected: &[String],
    server: &[horizon_client::packs::Pack],
) -> core::ops::ControlFlow<std::io::Result<PathBuf>> {
    // The studio's override pack (scripts' edits of the server's packs) on top.
    let Some(over) = crate::game::hstudio_server_content::override_pack() else {
        return core::ops::ControlFlow::Continue(());
    };
    let all = horizon_client::packs::discover(vanilla, &crate::game::packs::packs_dir());
    let mut enabled = horizon_client::packs::enabled(&all, selected);
    enabled.extend_from_slice(server);
    enabled.push(over);
    core::ops::ControlFlow::Break(horizon_client::packs::resource_root(
        vanilla,
        &enabled,
        &crate::game::packs::cache_dir(),
    ))
}
