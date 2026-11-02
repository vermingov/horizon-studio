fn horizon_studio_worlds_list(saves: &std::path::Path) -> core::ops::ControlFlow<Vec<WorldEntry>> {
    // The saves' worlds, then saveinstance's in the workspace (hstudio_worlds).
    let Some(_listing) = crate::title::hstudio_worlds::listing() else {
        return core::ops::ControlFlow::Continue(());
    };
    let mut worlds = crate::title::list_worlds(saves);
    let workspace = crate::title::hstudio_worlds::workspace(saves);
    for mut w in crate::title::list_worlds(&workspace) {
        if let Some(folder) = crate::title::hstudio_worlds::listed(saves, &workspace, &w.folder) {
            w.folder = folder;
            worlds.push(w);
        }
    }
    worlds.sort_by_key(|w| std::cmp::Reverse(w.modified));
    core::ops::ControlFlow::Break(worlds)
}
