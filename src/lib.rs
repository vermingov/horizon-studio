// The bundle's game-independent modules, at the paths horizon-mixins gives
// them inside the game (`crate::game::<name>`), so they build and test on
// their own. hstudio_bridge is the one module that needs the game; it is
// checked by `python3 tools/studio.py check` against the pinned Horizon.
// ViaHorizon's manager goes into the game's library (`horizon_client::net`)
// and its protocol side into horizon-protocol (tests/shim stands in).

pub mod game {
    #[path = "../../mixins/horizon_studio/studio.rs"]
    pub mod hstudio;
    #[path = "../../mixins/horizon_studio/examples.rs"]
    pub mod hstudio_examples;
    #[path = "../../mixins/horizon_studio/look.rs"]
    pub mod hstudio_look;
    #[path = "../../mixins/horizon_studio/lua.rs"]
    pub mod hstudio_lua;
    #[path = "../../mixins/horizon_studio/lualib.rs"]
    pub mod hstudio_lualib;
    #[path = "../../mixins/horizon_studio/ui.rs"]
    pub mod hstudio_ui;
}

pub mod net {
    #[path = "../../mixins/horizon_studio/via.rs"]
    pub mod hstudio_via;
}
