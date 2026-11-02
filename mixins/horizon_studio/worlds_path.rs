fn horizon_studio_worlds_path(saves: &std::path::Path, folder: &str) -> core::ops::ControlFlow<crate::Result<PathBuf>> {
    // A world saveinstance saved in the workspace: where it is.
    match crate::title::hstudio_worlds::resolve(saves, folder) {
        Some(world) => core::ops::ControlFlow::Break(Ok(world)),
        None => core::ops::ControlFlow::Continue(()),
    }
}
