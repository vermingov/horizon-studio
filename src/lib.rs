// The bundle's game-independent modules, at the paths horizon-mixins gives
// them inside the game (`crate::game::<name>`), so they build and test on
// their own. hstudio_bridge is the one module that needs the game; it is
// checked by `python3 tools/studio.py check` against the pinned Horizon.
// ViaHorizon's manager goes into the game's library (`horizon_client::net`)
// and its protocol side into horizon-protocol (tests/shim stands in).

pub mod game {
    #[path = "../../mixins/horizon_studio/studio.rs"]
    pub mod hstudio;
    #[path = "../../mixins/horizon_studio/agent.rs"]
    pub mod hstudio_agent;
    #[path = "../../mixins/horizon_studio/assist.rs"]
    pub mod hstudio_assist;
    #[path = "../../mixins/horizon_studio/examples.rs"]
    pub mod hstudio_examples;
    #[path = "../../mixins/horizon_studio/link.rs"]
    pub mod hstudio_link;
    #[path = "../../mixins/horizon_studio/look.rs"]
    pub mod hstudio_look;
    #[path = "../../mixins/horizon_studio/lua.rs"]
    pub mod hstudio_lua;
    #[path = "../../mixins/horizon_studio/lualib.rs"]
    pub mod hstudio_lualib;
    #[path = "../../mixins/horizon_studio/meshes.rs"]
    pub mod hstudio_meshes;
    #[path = "../../mixins/horizon_studio/models.rs"]
    pub mod hstudio_models;
    #[path = "../../mixins/horizon_studio/skins.rs"]
    pub mod hstudio_skins;
    #[path = "../../mixins/horizon_studio/theme.rs"]
    pub mod hstudio_theme;
    #[path = "../../mixins/horizon_studio/ui.rs"]
    pub mod hstudio_ui;
    #[path = "../../mixins/horizon_studio/via_picker.rs"]
    pub mod hstudio_via_picker;
}

pub mod net {
    #[path = "../../mixins/horizon_studio/mcp_update.rs"]
    pub mod hstudio_mcp_update;
    #[path = "../../mixins/horizon_studio/via.rs"]
    pub mod hstudio_via;
}

/// Natrium's switches, as horizon-core has them (`horizon_core::natrium`).
#[path = "../mixins/horizon_studio/natrium_flags.rs"]
pub mod natrium;

/// Script mods on a simulated game, without Horizon (the MCP server's tests).
pub mod headless;

/// horizon-studio-mcp: the studio for AI agents, over MCP.
pub mod mcp;
pub mod mcp_docs;

/// `horizon-studio-mcp install`: registering it with the agents here.
pub mod mcp_install;
