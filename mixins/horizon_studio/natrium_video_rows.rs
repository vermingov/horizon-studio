fn horizon_studio_natrium_video(opts: &[Opt]) -> core::ops::ControlFlow<Vec<Row>> {
    // Natrium: its section of Video Settings, after Preferences.
    match natrium_video::preferences(opts) {
        Some(rows) => core::ops::ControlFlow::Break(rows),
        None => core::ops::ControlFlow::Continue(()),
    }
}
