// Horizon Studio's voice chat in the game (Simple Voice Chat servers): what
// hstudio_voicechat needs of Horizon. At a server's Login it starts asking
// for the voice secret; the server's `voicechat:*` payloads go to it, which
// Horizon would otherwise drop; its own payloads go out with Game::send.
// Each frame it learns where the camera is and where the players it hears
// are (their eyes, by profile UUID), the keys chosen in Options > Mods, and
// whether the player is playing; away from a server, or with the studio
// off, it stops. Written against Horizon 4aa6854.

use super::Game;
use super::hstudio::ID;
use super::hstudio_voicechat::{self as voice, KEYS};
use horizon_protocol::packets::configuration::CustomPayload;
use horizon_protocol::packets::play::{Clientbound, Serverbound};
use hstudio_voice::{Listener, ServerHost, World};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

static CONFIGURED: AtomicBool = AtomicBool::new(false);

/// The server's host as the Minecraft connection reached it: its typed
/// address with Minecraft's SRV redirect (on the voice thread: a DNS lookup).
fn server_host(address: &str) -> String {
    let (host, port) = horizon_protocol::servers::parse_address(address).unwrap_or_else(|| (address.to_owned(), 25565));
    horizon_protocol::server_redirect(&host, port).0
}

/// Sends what the voice chat has for the server.
fn send_all(game: &Game<'_>) {
    for (channel, data) in voice::global(|g| g.take_payloads()) {
        game.send(Serverbound::CustomPayload(CustomPayload { channel, data }));
    }
}

/// A packet from the server, before the game handles it.
pub(super) fn packet(game: &mut Game<'_>, packet: &Clientbound) {
    if !game.native_on(ID) {
        return;
    }
    match packet {
        // A join, or a proxy's move to another server: the voice chat starts over.
        Clientbound::Login(_) => {
            let Some(address) = game.remote.as_ref().map(|r| r.session.address.clone()) else {
                return;
            };
            let dir = game.native_data_dir(ID);
            let key = address.clone();
            let host: ServerHost = Arc::new(move || server_host(&address));
            voice::global(|g| {
                g.configure(dir);
                g.joined(&key, host, Instant::now());
            });
            send_all(game);
        }
        Clientbound::CustomPayload(p) if p.channel.starts_with("voicechat:") => {
            voice::global(|g| g.payload(&p.channel, &p.data, Instant::now()));
            send_all(game);
        }
        _ => {}
    }
}

/// Where the camera is and the players the voices need are.
fn world(game: &Game<'_>, wanted: &[u128]) -> World {
    // A Roblox script's SoundService listener, facing as the camera does.
    let heard_at = voice::global(|g| g.listener_at);
    let listener = match game.locator_camera {
        _ if heard_at.is_some() => Listener {
            pos: heard_at.unwrap_or_default(),
            yaw: game.yaw,
            pitch: game.pitch,
        },
        Some((pos, yaw, pitch, _)) => Listener { pos, yaw, pitch },
        None => game.native_player().map_or_else(Listener::default, |p| Listener {
            pos: [p.x, p.y + 1.62, p.z],
            yaw: p.yaw,
            pitch: p.pitch,
        }),
    };
    let mut eyes = HashMap::new();
    let mut own = 0;
    if let Some(r) = game.remote.as_ref() {
        own = r.profile();
        let alpha = game.clock.fraction();
        for &uuid in wanted {
            let Some(id) = r.player_entity(uuid) else { continue };
            if let Some(m) = game.mobs.iter().find(|m| m.id == id && !m.removed) {
                let at = |a: f64, b: f64| a + (b - a) * alpha;
                eyes.insert(
                    uuid,
                    [
                        at(m.previous.x, m.position.x),
                        at(m.previous.y, m.position.y) + m.eye_height(),
                        at(m.previous.z, m.position.z),
                    ],
                );
            }
        }
    }
    World { listener, own, eyes }
}

/// Each frame (native_frame).
pub(super) fn frame(game: &mut Game<'_>) {
    // The settings are read from (and kept in) the studio's folder from the first frame on.
    if !CONFIGURED.swap(true, Ordering::Relaxed) {
        let dir = game.native_data_dir(ID);
        voice::global(|g| g.configure(dir));
    }
    if !game.native_on(ID) || game.remote.is_none() {
        voice::global(|g| g.left());
        return;
    }
    let keys = KEYS.map(|(name, _, default)| game.native_mod_key(ID, name, default));
    let playing = game.native_playing();
    let wanted = voice::global(|g| g.wanted());
    let world = world(game, &wanted);
    voice::global(|g| g.frame(world, playing, Some(keys), Instant::now()));
    send_all(game);
}

/// A key or mouse button (native_key): push to talk, mute and deafen.
pub(super) fn key(game: &Game<'_>, key: &str, pressed: bool) {
    if game.native_on(ID) {
        voice::global(|g| g.key(key, pressed));
    }
}

/// A chat line: the server's "incompatible version" line asks in the other version.
pub(super) fn chat_line(game: &Game<'_>, text: &horizon_client::text::Text) {
    if game.native_on(ID) && game.remote.is_some() {
        let text = text.plain();
        voice::global(|g| g.chat_line(&text, Instant::now()));
    }
}
