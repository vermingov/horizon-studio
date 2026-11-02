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
    #[path = "../../mixins/horizon_studio/chatcmd.rs"]
    pub mod hstudio_chatcmd;
    #[path = "../../mixins/horizon_studio/cmdtree.rs"]
    pub mod hstudio_cmdtree;
    #[path = "../../mixins/horizon_studio/coach.rs"]
    pub mod hstudio_coach;
    #[path = "../../mixins/horizon_studio/crypt.rs"]
    pub mod hstudio_crypt;
    #[path = "../../mixins/horizon_studio/dock.rs"]
    pub mod hstudio_dock;
    #[path = "../../mixins/horizon_studio/dump.rs"]
    pub mod hstudio_dump;
    #[path = "../../mixins/horizon_studio/examples.rs"]
    pub mod hstudio_examples;
    #[path = "../../mixins/horizon_studio/exec.rs"]
    pub mod hstudio_exec;
    #[path = "../../mixins/horizon_studio/link.rs"]
    pub mod hstudio_link;
    #[path = "../../mixins/horizon_studio/look.rs"]
    pub mod hstudio_look;
    #[path = "../../mixins/horizon_studio/lua.rs"]
    pub mod hstudio_lua;
    #[path = "../../mixins/horizon_studio/lualib.rs"]
    pub mod hstudio_lualib;
    #[path = "../../mixins/horizon_studio/menus.rs"]
    pub mod hstudio_menus;
    #[path = "../../mixins/horizon_studio/meshes.rs"]
    pub mod hstudio_meshes;
    #[path = "../../mixins/horizon_studio/models.rs"]
    pub mod hstudio_models;
    #[path = "../../mixins/horizon_studio/modfiles.rs"]
    pub mod hstudio_modfiles;
    #[path = "../../mixins/horizon_studio/net.rs"]
    pub mod hstudio_net;
    #[path = "../../mixins/horizon_studio/quads.rs"]
    pub mod hstudio_quads;
    #[path = "../../mixins/horizon_studio/rbx.rs"]
    pub mod hstudio_rbx;
    #[path = "../../mixins/horizon_studio/rbx_api.rs"]
    pub mod hstudio_rbx_api;
    #[path = "../../mixins/horizon_studio/rbx_api_data.rs"]
    pub mod hstudio_rbx_api_data;
    #[path = "../../mixins/horizon_studio/rbx_body.rs"]
    pub mod hstudio_rbx_body;
    #[path = "../../mixins/horizon_studio/rbx_client.rs"]
    pub mod hstudio_rbx_client;
    #[path = "../../mixins/horizon_studio/rbx_draw.rs"]
    pub mod hstudio_rbx_draw;
    #[path = "../../mixins/horizon_studio/rbx_file.rs"]
    pub mod hstudio_rbx_file;
    #[path = "../../mixins/horizon_studio/rbx_handles.rs"]
    pub mod hstudio_rbx_handles;
    #[path = "../../mixins/horizon_studio/rbx_image.rs"]
    pub mod hstudio_rbx_image;
    #[path = "../../mixins/horizon_studio/rbx_layout.rs"]
    pub mod hstudio_rbx_layout;
    #[path = "../../mixins/horizon_studio/rbx_methods.rs"]
    pub mod hstudio_rbx_methods;
    #[path = "../../mixins/horizon_studio/rbx_parts.rs"]
    pub mod hstudio_rbx_parts;
    #[path = "../../mixins/horizon_studio/rbx_query.rs"]
    pub mod hstudio_rbx_query;
    #[path = "../../mixins/horizon_studio/rbx_remote.rs"]
    pub mod hstudio_rbx_remote;
    #[path = "../../mixins/horizon_studio/rbx_tools.rs"]
    pub mod hstudio_rbx_tools;
    #[path = "../../mixins/horizon_studio/rbx_types.rs"]
    pub mod hstudio_rbx_types;
    #[path = "../../mixins/horizon_studio/rbx_values.rs"]
    pub mod hstudio_rbx_values;
    #[path = "../../mixins/horizon_studio/rbx_world.rs"]
    pub mod hstudio_rbx_world;
    #[path = "../../mixins/horizon_studio/skins.rs"]
    pub mod hstudio_skins;
    #[path = "../../mixins/horizon_studio/sunc.rs"]
    pub mod hstudio_sunc;
    #[path = "../../mixins/horizon_studio/theme.rs"]
    pub mod hstudio_theme;
    #[path = "../../mixins/horizon_studio/ui.rs"]
    pub mod hstudio_ui;
    #[path = "../../mixins/horizon_studio/via_picker.rs"]
    pub mod hstudio_via_picker;
    #[path = "../../mixins/horizon_studio/voicechat.rs"]
    pub mod hstudio_voicechat;
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
