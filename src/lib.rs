// The bundle's game-independent modules, at the paths horizon-mixins gives
// them inside the game (`crate::game::<name>`), so they build and test on
// their own. hstudio_bridge is the one module that needs the game; it is
// checked by `python3 tools/studio.py check` against the pinned Horizon.

pub mod game {
    #[path = "../../mixins/horizon_studio/studio.rs"]
    pub mod hstudio;
    #[path = "../../mixins/horizon_studio/examples.rs"]
    pub mod hstudio_examples;
    #[path = "../../mixins/horizon_studio/lua.rs"]
    pub mod hstudio_lua;
    #[path = "../../mixins/horizon_studio/lualib.rs"]
    pub mod hstudio_lualib;
    #[path = "../../mixins/horizon_studio/ui.rs"]
    pub mod hstudio_ui;
}
