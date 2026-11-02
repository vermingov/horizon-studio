// What the server sent the client, for the studio's ReplicatedStorage.Server
// (hstudio_server_content): the server's resource packs as the game has
// them, the plugin channels it registered (minecraft:register, in the
// configuration through hstudio_channels and in play) or sent on, its
// command tree, its registries and tags; each part with a version, so the
// studio makes again only what changed. And the studio's override pack:
// the files scripts edited, stacked above the server's packs in every
// resource reload while the client is on that server (the pack hook in
// game::packs::startup_root), reloaded as a server's packs are (a reload
// that fails leaves the resources as they were, and the player's own
// pack selection as it was).

use super::Game;
use horizon_protocol::packets::play::Clientbound as C;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The studio's override pack's folder, while the client is on a server.
static OVERRIDE: Mutex<Option<PathBuf>> = Mutex::new(None);

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

#[derive(Default)]
struct State {
    /// The connection the rest is of (its registries' allocation).
    connection: usize,
    session: u64,
    /// Every version handed out, the latest: each part's is one of them.
    clock: u64,
    versions: [u64; 4],
    packs: Vec<String>,
    channels: BTreeSet<String>,
    commands: Option<serde_json::Value>,
    /// The override pack wants a resource reload.
    reload: bool,
    applied: bool,
}

impl State {
    fn bump(&mut self, part: usize) {
        self.clock += 1;
        self.versions[part] = self.clock;
    }
}

const PACKS: usize = super::hstudio_rbx_server::PACKS;
const CHANNELS: usize = super::hstudio_rbx_server::CHANNELS;
const COMMANDS: usize = super::hstudio_rbx_server::COMMANDS;
const REGISTRIES: usize = super::hstudio_rbx_server::REGISTRIES;

/// The most channels kept of a connection.
const MAX_CHANNELS: usize = 4096;

fn protocol(ch: &str) -> bool {
    matches!(
        ch,
        "minecraft:brand" | "minecraft:register" | "minecraft:unregister" | "REGISTER" | "UNREGISTER"
    )
}

/// The server's packets: its command tree, the channels it registers or
/// sends on, its tags anew.
pub(super) fn heard(packet: &C) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        match packet {
            C::Commands(p) => {
                s.commands = Some(super::hstudio_world_files::command_tree(p));
                s.bump(COMMANDS);
            }
            C::CustomPayload(p) => {
                let named = if matches!(p.channel.as_str(), "minecraft:register" | "REGISTER") {
                    horizon_client::net::hstudio_channels::registered(&p.data)
                } else if protocol(&p.channel) {
                    Vec::new()
                } else {
                    vec![p.channel.clone()]
                };
                let mut new = false;
                for ch in named {
                    if s.channels.len() < MAX_CHANNELS && s.channels.insert(ch) {
                        new = true;
                    }
                }
                if new {
                    s.bump(CHANNELS);
                }
            }
            C::UpdateTags(_) => s.bump(REGISTRIES),
            _ => {}
        }
    });
}

/// What the studio hasn't the version of; None off a server or when it has it all.
pub(super) fn content(
    g: &Game<'_>,
    session: Option<u64>,
    versions: [u64; 4],
) -> Option<super::hstudio_rbx_server::ServerContent> {
    let r = g.remote.as_ref()?;
    let connection = std::sync::Arc::as_ptr(&r.registries) as usize;
    let packs: Vec<String> = g.server_packs.active.iter().map(|p| p.id.clone()).collect();
    let fresh = STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.connection != connection {
            // Another connection (or another server behind a proxy): all anew.
            let clock = s.clock;
            *s = State {
                connection,
                session: clock + 1,
                clock: clock + 1,
                reload: s.reload,
                applied: s.applied,
                ..State::default()
            };
            for part in 0..4 {
                s.bump(part);
            }
        }
        let config = horizon_client::net::hstudio_channels::take();
        let mut new = false;
        for ch in config.into_iter().filter(|c| !protocol(c)) {
            if s.channels.len() < MAX_CHANNELS && s.channels.insert(ch) {
                new = true;
            }
        }
        if new {
            s.bump(CHANNELS);
        }
        if s.packs != packs {
            s.packs = packs;
            s.bump(PACKS);
        }
        if session == Some(s.session) && versions == s.versions {
            return None;
        }
        let new = |i: usize| session != Some(s.session) || versions[i] != s.versions[i];
        Some((
            s.session,
            s.versions,
            [new(0), new(1), new(2), new(3)],
            s.channels.clone(),
            s.commands.clone(),
        ))
    })?;
    let (session, versions, new, channels, commands) = fresh;
    let packs = new[PACKS].then(|| {
        g.server_packs
            .active
            .iter()
            .filter_map(|p| match &p.source {
                horizon_client::packs::Source::Zip(path) | horizon_client::packs::Source::Dir(path) => {
                    Some(super::hstudio_rbx_server::ServerPack {
                        id: p.id.clone(),
                        path: path.clone(),
                    })
                }
            })
            .collect()
    });
    Some(super::hstudio_rbx_server::ServerContent {
        session,
        versions,
        packs,
        channels: new[CHANNELS].then(|| channels.into_iter().collect()),
        commands: if new[COMMANDS] { commands } else { None },
        registries: new[REGISTRIES].then(|| registries(r)),
    })
}

/// The registries the server sent, each entry as a data pack's JSON, and
/// their tags by name.
fn registries(r: &super::remote::Remote) -> super::hstudio_rbx_server::Registries {
    let mut out = super::hstudio_rbx_server::Registries::default();
    for (registry, entries) in &r.registries.registries {
        let list = entries
            .iter()
            .map(|(name, tag)| {
                let data = tag
                    .as_ref()
                    .map_or(serde_json::Value::Null, super::hstudio_world_files::nbt_json);
                (name.clone(), data)
            })
            .collect();
        out.entries.push((registry.clone(), list));
    }
    out.entries.sort_by(|a, b| a.0.cmp(&b.0));
    for t in horizon_protocol::hstudio_tags::last() {
        let tags = t
            .tags
            .iter()
            .map(|tag| {
                let ids = tag
                    .ids
                    .iter()
                    .filter_map(|&id| {
                        r.builtin
                            .get(&t.registry)
                            .and_then(|ids| ids.name(id))
                            .or_else(|| r.registries.name(&t.registry, id))
                            .map(str::to_owned)
                    })
                    .collect();
                (tag.name.clone(), ids)
            })
            .collect();
        out.tags.push((t.registry.clone(), tags));
    }
    out
}

// ---------------------------------------------------------------- the override pack

/// The studio's override pack set (with its pack.mcmeta) or taken away,
/// and a resource reload asked for.
pub(super) fn set_override(g: &Game<'_>, pack: Option<&Path>) -> Result<(), String> {
    match pack {
        Some(dir) => {
            if g.remote.is_none() {
                return Err("not on a server".into());
            }
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            let (major, minor) = horizon_client::packs::RESOURCE_FORMAT;
            let meta = serde_json::json!({
                "pack": {
                    "description": "Horizon Studio: your edits of the server's resource packs",
                    "min_format": [major, minor],
                    "max_format": [major, minor],
                    "pack_format": major,
                }
            });
            std::fs::write(dir.join("pack.mcmeta"), meta.to_string()).map_err(|e| format!("pack.mcmeta: {e}"))?;
            *OVERRIDE.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(dir.to_path_buf());
        }
        None => *OVERRIDE.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None,
    }
    STATE.with(|s| s.borrow_mut().reload = true);
    Ok(())
}

/// The override pack, for the pack stack: on top of the server's.
pub(super) fn override_pack() -> Option<horizon_client::packs::Pack> {
    let dir = OVERRIDE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()?;
    let parent = dir.parent()?;
    let name = dir.file_name()?.to_string_lossy().into_owned();
    let mut pack = horizon_client::packs::folder(parent, horizon_client::packs::RESOURCE_FORMAT)
        .into_iter()
        .find(|p| p.id == format!("file/{name}"))?;
    pack.id = format!("horizon_studio/{name}");
    Some(pack)
}

/// The session ended: the override pack (its edits) leaves the resources,
/// and the next session's content is all new.
pub(super) fn session_end() {
    let had = OVERRIDE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .is_some();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.connection = 0;
        if had {
            s.reload = true;
        }
    });
}

/// Each frame: off the server the override pack goes; a reload it wants
/// starts once no other reload is under way (a server's pack reload, so
/// one that fails changes nothing).
pub(super) fn frame(g: &mut Game<'_>) {
    let on = OVERRIDE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .is_some();
    if on && g.remote.is_none() {
        *OVERRIDE.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        STATE.with(|s| s.borrow_mut().reload = true);
    }
    let wanted = STATE.with(|s| s.borrow().reload);
    if !wanted
        || g.pack_reload.is_some()
        || g.pending_pack_reload
        || g.rest_loader.is_some()
        || g.world_loader.is_some()
    {
        return;
    }
    let applied = STATE.with(|s| s.borrow().applied);
    let on = OVERRIDE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .is_some();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.reload = false;
        s.applied = on;
    });
    // Nothing of ours in the resources and none to put there.
    if !on && !applied {
        return;
    }
    if let Err(e) = g.spawn_pack_reload(super::packs::ReloadKind::Server) {
        log::warn!("Horizon Studio: the resources couldn't reload with the override pack: {e}");
    }
}
