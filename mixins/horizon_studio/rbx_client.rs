// The Roblox layer's client (hstudio_rbx_client): what Roblox's own client
// does for its scripts, done by Minecraft's. game.JobId is the server's
// address and game.PlaceId a number of the server's own (MarketplaceService
// names the place after it); the server's chat lines come as
// TextChatService.MessageReceived and Player.Chatted, a whisper (/w) goes
// as Minecraft's /msg, and bubble chat draws the lines over the speakers'
// heads; Player:Kick and game:Shutdown leave the server, TeleportService
// takes the player back to it and queue_on_teleport's scripts run once
// they are there; GuiService's error is the game's Disconnected screen;
// the settings Roblox scripts set (the master volume, the graphics quality,
// full screen, 3D rendering, the core GUI's chat and player list) are the
// game's own; VoiceChatInternal mutes other players' voices in the voice
// chat and SoundService's listener is where voices are heard from; the
// LocalPlayer's Idled fires as Roblox's does, and the click a script
// answers it with is activity the server sees. What has no meaning in
// Minecraft (the recorder, the inspect menu, friend requests, other
// places) says so, as Roblox's own prompt would.

use super::hstudio::{ClientAction, GameApi, ServerInfo};
use super::hstudio_lua::{Lua, Res, Value, check_num};
use super::hstudio_rbx::{Env, Id, Rbx};
use super::hstudio_rbx_types::{Color3, UDim2, Vector3};
use super::hstudio_rbx_values::{as_cf, c3, enum_item, enum_value, udim2, v3};
use std::collections::BTreeMap;

/// Roblox's idle time before Player.Idled fires, and again after.
pub const IDLE_SECS: f64 = 120.0;
/// How long a teleport takes to start (TeleportService:TeleportCancel can
/// stop it until then).
pub const TELEPORT_SECS: f64 = 0.5;
/// How long a chat bubble stays up, as Roblox's.
pub const BUBBLE_SECS: f64 = 15.0;
/// Messages kept as TextChatMessages (older ones are destroyed).
const KEPT_MESSAGES: usize = 64;

/// The layer's client: what scripts asked of it, and what it follows.
#[derive(Default)]
pub struct Client {
    /// What to ask of the game this frame.
    pub actions: Vec<ClientAction>,
    /// The server as the last frame saw it.
    server: Option<ServerInfo>,
    seen: bool,
    /// queue_on_teleport's scripts, for the next server joined; and those to
    /// run now (the studio runs them in the console's place).
    pub queued: Vec<String>,
    pub run_now: Vec<String>,
    /// A teleport under way: when it goes, and the data it carries.
    teleport_at: Option<f64>,
    teleport_data: Value,
    /// What GetLocalPlayerTeleportData gives once there.
    arrived_data: Value,
    /// The disconnect's reason as the game shows it: GuiService's error.
    error: Option<String>,
    /// When the player last did something, when Idled last fired, and the
    /// player's place and look then.
    active_at: f64,
    idled_at: f64,
    pose: Option<[f64; 5]>,
    /// Bubble chat (a script turned it on) and the bubbles up, with when
    /// each goes.
    bubbles: bool,
    shown: Vec<(Id, f64)>,
    /// What the HUD was last told of its parts.
    hud: BTreeMap<&'static str, bool>,
    /// The part voices are heard from (SoundService:SetListener), and
    /// whether the voice chat was last told one.
    listener: Option<Id>,
    heard_at: Option<[f64; 3]>,
    rendering: bool,
    /// StarterGui:SetCore("DevConsoleVisible", true): the studio's console.
    pub console: bool,
    /// Each chatting player's TextSource, and the messages kept.
    sources: Vec<(Id, Id)>,
    messages: Vec<Id>,
}

// ---------------------------------------------------------------- the server

/// game.PlaceId for a server: a number of its address's own (its world's
/// name for a world of the player's own), the same each time, as Roblox's
/// is for a place: what scripts keep per place (Infinite Yield's
/// waypoints) stays with the server.
pub fn place_id(s: &ServerInfo) -> f64 {
    let key = if s.own_world {
        format!("world:{}", s.name)
    } else {
        s.address.to_ascii_lowercase()
    };
    // FNV-1a, within what a double holds exactly.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in key.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    (h % 99_999_999_999 + 1) as f64
}

/// The place's name (MarketplaceService:GetProductInfo(game.PlaceId)):
/// the server's address or the world's name; "Minecraft" out of a world.
pub fn place_name(rbx: &Rbx) -> String {
    match &rbx.client.server {
        Some(s) if !s.name.is_empty() => s.name.clone(),
        Some(s) if !s.address.is_empty() => s.address.clone(),
        _ => "Minecraft".into(),
    }
}

/// The game's server, as game.JobId (its address: what a server is called
/// by in Minecraft), game.PlaceId and game.GameId say it. Out of a server
/// they stay the last one's, as Roblox's do once the player is kicked.
fn server_changed(rbx: &mut Rbx, now: Option<&ServerInfo>) {
    let game = rbx.game;
    let (job, place) = match now {
        Some(s) if s.own_world => (String::new(), place_id(s)),
        Some(s) => (s.address.clone(), place_id(s)),
        None => return,
    };
    rbx.raw_set(game, "JobId", Value::str(&job));
    rbx.raw_set(game, "PlaceId", Value::Num(place));
    rbx.raw_set(game, "GameId", Value::Num(place));
}

/// Each frame: the server and the game's error; then, while scripts use the
/// layer, what they asked of the client.
pub fn frame(rbx: &mut Rbx, game: &mut dyn GameApi, now: f64) {
    let info = game.client();
    if !rbx.client.seen || info.server != rbx.client.server {
        let was = rbx.client.server.take();
        server_changed(rbx, info.server.as_ref());
        let joined = match (&was, &info.server) {
            (None, Some(_)) => true,
            (Some(a), Some(b)) => a.address != b.address || a.name != b.name,
            _ => false,
        };
        if joined && rbx.client.seen {
            rbx.client.run_now.append(&mut rbx.client.queued);
            rbx.client.arrived_data = std::mem::take(&mut rbx.client.teleport_data);
        }
        rbx.client.server = info.server.clone();
        rbx.client.seen = true;
    }
    if info.disconnected != rbx.client.error {
        rbx.client.error = info.disconnected.clone();
        if let (Some(why), Some(gs)) = (info.disconnected, rbx.service("GuiService")) {
            rbx.fire(gs, "ErrorMessageChanged", vec![Value::str(&why)]);
        }
    }
    if !rbx.active {
        return;
    }
    idle(rbx, game, now);
    if rbx.client.teleport_at.is_some_and(|t| now >= t) {
        rbx.client.teleport_at = None;
        rbx.client.actions.push(ClientAction::Rejoin);
    }
    bubbles_lapse(rbx, now);
    hud(rbx);
    listen(rbx);
    for a in std::mem::take(&mut rbx.client.actions) {
        if let Err(e) = game.client_do(a) {
            rbx.warnings.push(e);
        }
    }
}

/// The player did something (a key, a click): not idle.
pub fn input(rbx: &mut Rbx) {
    rbx.client.active_at = rbx.now;
}

/// Player.Idled, as Roblox fires it: after two minutes of the player doing
/// nothing (not moving, turning or pressing), and every two minutes after,
/// with how long they have been idle.
fn idle(rbx: &mut Rbx, game: &mut dyn GameApi, now: f64) {
    if let Some(p) = game.player() {
        let pose = [p.x, p.y, p.z, f64::from(p.yaw), f64::from(p.pitch)];
        let moved = rbx
            .client
            .pose
            .is_none_or(|q| q.iter().zip(pose).any(|(a, b)| (a - b).abs() > 1e-3));
        rbx.client.pose = Some(pose);
        if moved {
            rbx.client.active_at = now;
        }
    } else {
        rbx.client.active_at = now;
    }
    let c = &rbx.client;
    let idle_for = now - c.active_at;
    if idle_for >= IDLE_SECS && now - c.idled_at.max(c.active_at) >= IDLE_SECS {
        rbx.client.idled_at = now;
        let lp = rbx.local_player;
        rbx.fire(lp, "Idled", vec![Value::Num(idle_for.floor())]);
    }
}

// ---------------------------------------------------------------- chat

/// Minecraft's formatting codes (§ and a character) out of a line.
fn unformatted(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\u{a7}' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}

/// A word with the brackets and marks servers put around names taken off.
fn bare(word: &str) -> &str {
    word.trim_matches(|c: char| !(c.is_alphanumeric() || c == '_'))
}

/// Who said what in a chat line, if a player did: vanilla's "<Name> text",
/// its whispers ("Name whispers to you: text"), and the servers' own forms
/// ("[Rank] Name: text", "Name » text", "Name > text"). `names` are the
/// players on the server; the name must be one of them. The third value
/// says it was whispered.
pub fn parse_chat(line: &str, names: &[String]) -> Option<(String, String, bool)> {
    let line = unformatted(line);
    let line = line.trim();
    let known = |w: &str| names.iter().find(|n| n.eq_ignore_ascii_case(bare(w))).cloned();
    if let Some(rest) = line.strip_prefix('<')
        && let Some((inside, text)) = rest.split_once('>')
        && let Some(name) = inside.split_whitespace().last().and_then(known)
    {
        return Some((name, text.trim().to_owned(), false));
    }
    if let Some((who, text)) = line.split_once(" whispers to you: ")
        && let Some(name) = who.split_whitespace().last().and_then(known)
    {
        return Some((name, text.trim().to_owned(), true));
    }
    let mut best: Option<(usize, String, String)> = None;
    for sep in [": ", " \u{bb} ", " > ", " >> ", " \u{2192} ", " | "] {
        let Some(at) = line.find(sep) else { continue };
        if at > 64 || best.as_ref().is_some_and(|b| b.0 <= at) {
            continue;
        }
        let Some(name) = line[..at].split_whitespace().last().and_then(known) else {
            continue;
        };
        best = Some((at, name, line[at + sep.len()..].trim().to_owned()));
    }
    best.map(|(_, name, text)| (name, text, false))
}

/// A chat line the player received: a player's message is the
/// TextChatService's MessageReceived (and RBXGeneral's), with a
/// TextChatMessage from that player's TextSource, and their Player's
/// Chatted; with bubble chat on, it shows over their head.
pub fn chat_line(rbx: &mut Rbx, game: &mut dyn GameApi, line: &str) {
    if !rbx.active || line.trim().is_empty() {
        return;
    }
    // The Players by name: the LocalPlayer, and the others the layer has.
    let mut players: Vec<(Id, String)> = super::hstudio_rbx_world::players_of(rbx)
        .into_iter()
        .filter_map(|p| rbx.try_inst(p).map(|i| (p, i.name.clone())))
        .collect();
    if let Some(me) = game.player().filter(|p| !p.name.is_empty()) {
        players[0].1 = me.name;
    }
    let names: Vec<String> = players.iter().map(|p| p.1.clone()).collect();
    let Some((name, text, whispered)) = parse_chat(line, &names) else {
        return;
    };
    let Some(&(player, _)) = players.iter().find(|p| p.1 == name) else {
        return;
    };
    message(rbx, player, &text, whispered);
}

fn message(rbx: &mut Rbx, player: Id, text: &str, whispered: bool) {
    let Some(tcs) = rbx.service("TextChatService") else {
        return;
    };
    let channels = rbx.child_named(tcs, "TextChannels");
    let general = channels.and_then(|c| rbx.child_named(c, "RBXGeneral"));
    let source = text_source(rbx, player, general);
    let msg = rbx.make("TextChatMessage", None);
    rbx.inst_mut(msg).name = "TextChatMessage".into();
    let id = rbx.client.messages.len() + rbx.client.sources.len();
    let name = rbx.inst(player).name.clone();
    let extra = &mut rbx.inst_mut(msg).extra;
    extra.insert("Text".into(), Value::str(text));
    extra.insert("PrefixText".into(), Value::str(&name));
    extra.insert("MessageId".into(), Value::str(&format!("hs-{msg}-{id}")));
    extra.insert(
        "Status".into(),
        enum_value(enum_item("TextChatMessageStatus", "Success")),
    );
    let msgv = rbx.value(msg);
    if let Some(s) = source {
        let sv = rbx.value(s);
        rbx.inst_mut(msg).extra.insert("TextSource".into(), sv);
    }
    if let Some(g) = general {
        let gv = rbx.value(g);
        rbx.inst_mut(msg).extra.insert("TextChannel".into(), gv);
        rbx.fire(g, "MessageReceived", vec![msgv.clone()]);
    }
    rbx.fire(tcs, "MessageReceived", vec![msgv]);
    let recipient = if whispered {
        rbx.value(rbx.local_player)
    } else {
        Value::Nil
    };
    rbx.fire(player, "Chatted", vec![Value::str(text), recipient]);
    rbx.client.messages.push(msg);
    if rbx.client.messages.len() > KEPT_MESSAGES {
        let old = rbx.client.messages.remove(0);
        rbx.destroy(old);
    }
    if rbx.client.bubbles && !whispered {
        bubble(rbx, player, text);
    }
}

/// A player's TextSource in RBXGeneral, as Roblox gives each one who can
/// chat there.
fn text_source(rbx: &mut Rbx, player: Id, channel: Option<Id>) -> Option<Id> {
    if let Some(&(_, s)) = rbx
        .client
        .sources
        .iter()
        .find(|(p, s)| *p == player && rbx.try_inst(*s).is_some())
    {
        return Some(s);
    }
    let s = rbx.make("TextSource", None);
    let name = rbx.inst(player).name.clone();
    let uid = rbx.raw_get(player, "UserId");
    let shown = rbx.raw_get(player, "DisplayName");
    rbx.inst_mut(s).name = name.clone();
    let extra = &mut rbx.inst_mut(s).extra;
    extra.insert("UserId".into(), uid);
    extra.insert("Username".into(), Value::str(&name));
    extra.insert("DisplayName".into(), shown);
    extra.insert("CanSend".into(), Value::Bool(true));
    if let Some(c) = channel {
        let _ = rbx.set_parent(s, Some(c));
    }
    rbx.client.sources.retain(|(p, _)| *p != player);
    rbx.client.sources.push((player, s));
    Some(s)
}

/// Roblox's chat bubble: a BillboardGui over the speaker's head with the
/// line in a rounded white bubble, for BUBBLE_SECS (the newest of a
/// player's takes the place of the one before).
fn bubble(rbx: &mut Rbx, player: Id, text: &str) {
    let Some(character) = rbx.id_of(&rbx.raw_get(player, "Character")) else {
        return;
    };
    let Some(head) = rbx.child_named(character, "Head") else {
        return;
    };
    let core = rbx.core_gui;
    let folder = match rbx.child_named(core, "BubbleChat") {
        Some(f) => f,
        None => {
            let f = rbx.make("Folder", None);
            rbx.inst_mut(f).name = "BubbleChat".into();
            let _ = rbx.set_parent(f, Some(core));
            f
        }
    };
    let name = rbx.inst(player).name.clone();
    if let Some(old) = rbx.child_named(folder, &name) {
        rbx.client.shown.retain(|(g, _)| *g != old);
        rbx.destroy(old);
    }
    let chars = text.chars().count() as f64;
    let width = (chars * 8.0 + 24.0).clamp(48.0, 260.0);
    let lines = (chars * 8.0 / 236.0).ceil().clamp(1.0, 6.0);
    let height = lines * 18.0 + 14.0;
    let gui = rbx.make("BillboardGui", None);
    rbx.inst_mut(gui).name = name;
    let hv = rbx.value(head);
    rbx.raw_set(gui, "Adornee", hv);
    rbx.raw_set(gui, "Size", udim2(UDim2::new(0.0, width, 0.0, height)));
    rbx.raw_set(gui, "StudsOffset", v3(Vector3::new(0.0, 1.6, 0.0)));
    rbx.raw_set(gui, "MaxDistance", Value::Num(100.0));
    let label = rbx.make("TextLabel", None);
    rbx.raw_set(label, "Size", udim2(UDim2::new(1.0, 0.0, 1.0, 0.0)));
    rbx.raw_set(label, "BackgroundColor3", c3(Color3::new(1.0, 1.0, 1.0)));
    rbx.raw_set(label, "BackgroundTransparency", Value::Num(0.1));
    rbx.raw_set(label, "TextColor3", c3(Color3::new(0.22, 0.22, 0.22)));
    rbx.raw_set(label, "TextSize", Value::Num(16.0));
    rbx.raw_set(label, "TextWrapped", Value::Bool(true));
    rbx.raw_set(label, "Text", Value::str(text));
    let _ = rbx.set_parent(label, Some(gui));
    let corner = rbx.make("UICorner", None);
    rbx.raw_set(
        corner,
        "CornerRadius",
        super::hstudio_rbx_values::udim(super::hstudio_rbx_types::UDim::new(0.0, 12.0)),
    );
    let _ = rbx.set_parent(corner, Some(label));
    let _ = rbx.set_parent(gui, Some(folder));
    let until = rbx.now + BUBBLE_SECS;
    rbx.client.shown.push((gui, until));
}

fn bubbles_lapse(rbx: &mut Rbx, now: f64) {
    let (gone, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut rbx.client.shown)
        .into_iter()
        .partition(|(g, until)| now >= *until || rbx.try_inst(*g).is_none_or(|i| i.destroyed));
    rbx.client.shown = kept;
    for (g, _) in gone {
        if rbx.try_inst(g).is_some_and(|i| !i.destroyed) {
            rbx.destroy(g);
        }
    }
}

/// A Roblox whisper command (/w Name text, /whisper Name text) as
/// Minecraft's private message; None for any other line.
pub fn whisper(line: &str) -> Option<String> {
    let (cmd, rest) = line.split_once(' ')?;
    if !matches!(cmd.to_ascii_lowercase().as_str(), "/w" | "/whisper") {
        return None;
    }
    let (name, text) = rest.trim().split_once(' ')?;
    let (name, text) = (bare(name), text.trim());
    if name.is_empty() || text.is_empty() {
        return None;
    }
    Some(format!("/msg {name} {text}"))
}

// ---------------------------------------------------------------- what scripts set

/// A property of Roblox's API a script set (one the layer keeps without a
/// class of its own): those that reach the client.
pub fn extra_set(rbx: &mut Rbx, id: Id, key: &str) {
    let inst = rbx.inst(id);
    let class = inst.class.name;
    let v = inst.extra.get(key).cloned().unwrap_or_default();
    match (class, key) {
        ("BubbleChatConfiguration", "Enabled") | ("Chat", "BubbleChatEnabled") => {
            rbx.client.bubbles = v.truthy();
            if !rbx.client.bubbles {
                bubbles_lapse(rbx, f64::INFINITY);
            }
        }
        ("UserGameSettings", "MasterVolume") => {
            let x = v.to_num().unwrap_or(1.0);
            if x.is_finite() {
                rbx.client
                    .actions
                    .push(ClientAction::MasterVolume(x.clamp(0.0, 1.0) as f32));
            }
        }
        ("RenderSettings", "QualityLevel") | ("UserGameSettings", "SavedQualityLevel") => {
            let level = super::hstudio_rbx_values::get(&v)
                .and_then(|d| match d {
                    super::hstudio_rbx_values::Data::Item(i) => Some(i.value),
                    _ => None,
                })
                .or_else(|| v.to_num().map(|n| n as i32))
                .unwrap_or(0);
            // Roblox's lowest levels (1 to 3) are the game's graphics lowered;
            // Automatic and the rest, the player's own.
            rbx.client
                .actions
                .push(ClientAction::LowGraphics((1..=3).contains(&level)));
        }
        _ => {}
    }
}

/// The HUD's chat and player list as the core GUI's switches
/// (StarterGui:SetCoreGuiEnabled) and TextChatService's chat window have
/// them; told to the game as they change.
fn hud(rbx: &mut Rbx) {
    let off = |t: &str| rbx.core_gui_off.contains(t);
    let window = rbx
        .service("TextChatService")
        .and_then(|t| rbx.child_named(t, "ChatWindowConfiguration"))
        .is_none_or(|w| rbx.inst(w).extra.get("Enabled").is_none_or(Value::truthy));
    let want = [
        ("all", !off("All")),
        ("chat", !off("All") && !off("Chat") && window),
        ("playerlist", !off("All") && !off("PlayerList")),
    ];
    for (part, shown) in want {
        let was = rbx.client.hud.get(part).copied().unwrap_or(true);
        if was != shown {
            rbx.client.hud.insert(part, shown);
            rbx.client.actions.push(ClientAction::Hud(part.into(), shown));
        }
    }
}

/// Voices are heard from the listener part's place while there is one.
fn listen(rbx: &mut Rbx) {
    let at = rbx
        .client
        .listener
        .filter(|&p| rbx.try_inst(p).is_some_and(|i| !i.destroyed))
        .and_then(|p| as_cf(&rbx.raw_get(p, "CFrame")))
        .map(|c| {
            let p = c.position();
            [p.x, p.y, p.z]
        });
    if at != rbx.client.heard_at {
        rbx.client.heard_at = at;
        rbx.client.actions.push(ClientAction::VoiceListener(at));
    }
}

// ---------------------------------------------------------------- methods

/// The methods this module has (`hstudio_rbx`'s METHODS lists them too).
pub fn takes(class: &str, name: &str) -> bool {
    matches!(
        (class, name),
        ("DataModel", "Shutdown")
            | (
                "TeleportService",
                "Teleport"
                    | "TeleportAsync"
                    | "TeleportToPlaceInstance"
                    | "TeleportToPrivateServer"
                    | "TeleportCancel"
                    | "GetLocalPlayerTeleportData"
            )
            | (
                "GuiService",
                "ClearError" | "GetErrorMessage" | "ToggleFullscreen" | "InspectPlayerFromUserId" | "CloseInspectMenu"
            )
            | ("RunService", "Set3dRenderingEnabled")
            | ("CoreGui", "TakeScreenshot" | "ToggleRecording")
            | ("VoiceChatInternal", "SubscribePause" | "SubscribePauseAll")
            | ("SoundService", "SetListener" | "GetListener")
            | ("Player", "RequestFriendship" | "RevokeFriendship")
    )
}

/// Something Roblox would put up a prompt for, said as the layer's
/// notifications are: a line in the game's own chat.
fn note(rbx: &mut Rbx, text: &str) {
    rbx.warnings.push(format!("\u{1}[Roblox] {text}"));
}

/// The players' UserId's player, and their UUID (none for the LocalPlayer).
fn other_by_user_id(rbx: &Rbx, uid: f64) -> Option<u128> {
    rbx.world
        .others
        .iter()
        .find(|o| rbx.raw_get(o.player, "UserId").to_num() == Some(uid))
        .map(|o| o.uuid)
}

pub fn method(env: &mut dyn Env, lua: &mut Lua, _id: Id, class: &str, name: &str, args: &[Value]) -> Res {
    let now = env.now();
    let rbx = env.rbx();
    let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
    match (class, name) {
        // Roblox's client closes; Minecraft's leaves the server, as its
        // Disconnect button does.
        ("DataModel", "Shutdown") => {
            rbx.client.actions.push(ClientAction::Leave(None));
            Ok(Vec::new())
        }
        ("TeleportService", "Teleport" | "TeleportToPlaceInstance") => {
            let place = check_num(lua, args, 1, name)?;
            let job = (name == "TeleportToPlaceInstance").then(|| arg(2).as_text().unwrap_or_default());
            let data = if name == "Teleport" { arg(3) } else { arg(5) };
            let here = rbx.raw_get(rbx.game, "PlaceId").to_num() == Some(place) && place != 0.0;
            let this_job = rbx.raw_get(rbx.game, "JobId").as_text().unwrap_or_default();
            let rejoin = rbx.client.server.is_some() || rbx.client.error.is_some();
            if here && job.as_ref().is_none_or(|j| j.is_empty() || *j == this_job) && rejoin {
                teleport(rbx, now, data);
            } else {
                let why = if here {
                    format!(
                        "there is no other server of this one to join (servers are joined by their address: this is {this_job})"
                    )
                } else if place == 0.0 || !rejoin {
                    "the player is on no server to teleport from".into()
                } else {
                    format!(
                        "place {} is no Minecraft server (only this server's PlaceId, {}, takes the player back to it)",
                        super::hstudio_lua::fmt_num(place),
                        super::hstudio_lua::fmt_num(rbx.raw_get(rbx.game, "PlaceId").to_num().unwrap_or(0.0))
                    )
                };
                teleport_failed(rbx, &why);
            }
            Ok(Vec::new())
        }
        ("TeleportService", "TeleportAsync" | "TeleportToPrivateServer") => {
            Err(lua.rt_error(format!("{name} can only be called from the server")))
        }
        ("TeleportService", "TeleportCancel") => {
            if rbx.client.teleport_at.take().is_some() {
                rbx.client.teleport_data = Value::Nil;
                let lp = rbx.local_player;
                rbx.fire(
                    lp,
                    "OnTeleport",
                    vec![enum_value(enum_item("TeleportState", "Failed")), Value::Nil, Value::Nil],
                );
            }
            Ok(Vec::new())
        }
        ("TeleportService", "GetLocalPlayerTeleportData") => Ok(vec![rbx.client.arrived_data.clone()]),
        // The error prompt is the game's Disconnected screen: there is
        // nothing of it over the world to clear.
        ("GuiService", "ClearError") => {
            rbx.client.error = None;
            Ok(Vec::new())
        }
        ("GuiService", "GetErrorMessage") => Ok(vec![Value::str(rbx.client.error.as_deref().unwrap_or(""))]),
        ("GuiService", "ToggleFullscreen") => {
            rbx.client.actions.push(ClientAction::ToggleFullscreen);
            Ok(Vec::new())
        }
        ("GuiService", "InspectPlayerFromUserId") => {
            let uid = arg(1).to_num().unwrap_or(0.0);
            let who = super::hstudio_rbx_world::players_of(rbx)
                .into_iter()
                .find(|&p| rbx.raw_get(p, "UserId").to_num() == Some(uid))
                .map_or_else(
                    || format!("user {}", super::hstudio_lua::fmt_num(uid)),
                    |p| rbx.inst(p).name.clone(),
                );
            note(rbx, &format!("Minecraft has no inspect menu to show {who}'s avatar in"));
            Ok(Vec::new())
        }
        ("GuiService", "CloseInspectMenu") => Ok(Vec::new()),
        ("RunService", "Set3dRenderingEnabled") => {
            let on = arg(1).truthy();
            if on != !rbx.client.rendering {
                rbx.client.rendering = !on;
                rbx.client.actions.push(ClientAction::WorldRendering(on));
            }
            Ok(Vec::new())
        }
        ("CoreGui", "TakeScreenshot") => {
            let (_, game) = env.rbx_and_game();
            let dir = game.data_dir().join("screenshots");
            let stamp = stamp();
            let path = dir.join(format!("{stamp}.png"));
            let said = std::fs::create_dir_all(&dir)
                .map_err(|e| e.to_string())
                .and_then(|()| game.screenshot(&path));
            // Roblox's says so in a toast; Infinite Yield shows what its
            // command returns.
            Ok(vec![Value::str(&match said {
                Ok(()) => format!("Screenshot saved to {}", path.display()),
                Err(e) => format!("No screenshot: {e}"),
            })])
        }
        ("CoreGui", "ToggleRecording") => Ok(vec![Value::str(
            "Minecraft has no video recorder (record with your system's screen recorder)",
        )]),
        ("VoiceChatInternal", "SubscribePause") => {
            let uid = check_num(lua, args, 1, name)?;
            let paused = arg(2).truthy();
            if let Some(uuid) = other_by_user_id(rbx, uid) {
                rbx.client.actions.push(ClientAction::MuteVoice(Some(uuid), paused));
            }
            Ok(Vec::new())
        }
        ("VoiceChatInternal", "SubscribePauseAll") => {
            let paused = arg(1).truthy();
            rbx.client.actions.push(ClientAction::MuteVoice(None, paused));
            Ok(Vec::new())
        }
        ("SoundService", "SetListener") => {
            let kind = super::hstudio_rbx_draw::enum_name(&arg(1));
            rbx.client.listener = match kind {
                "ObjectPosition" | "ObjectCFrame" => rbx.id_of(&arg(2)),
                _ => None,
            };
            Ok(Vec::new())
        }
        ("SoundService", "GetListener") => Ok(match rbx.client.listener {
            Some(p) => vec![enum_value(enum_item("ListenerType", "ObjectPosition")), rbx.value(p)],
            None => vec![enum_value(enum_item("ListenerType", "Camera")), Value::Nil],
        }),
        ("Player", "RequestFriendship" | "RevokeFriendship") => {
            let who = rbx
                .id_of(&arg(1))
                .map_or_else(|| "them".to_owned(), |p| rbx.inst(p).name.clone());
            let what = if name == "RequestFriendship" {
                "send a friend request to"
            } else {
                "unfriend"
            };
            note(rbx, &format!("Minecraft has no friends list to {what} {who} in"));
            Ok(Vec::new())
        }
        _ => Err(lua.rt_error(format!("{name} is not available here"))),
    }
}

/// A teleport back to this server: it starts (LocalPlayer.OnTeleport), and
/// once TELEPORT_SECS have passed the player rejoins, the data it carries
/// waiting for GetLocalPlayerTeleportData there.
fn teleport(rbx: &mut Rbx, now: f64, data: Value) {
    rbx.client.teleport_at = Some(now + TELEPORT_SECS);
    rbx.client.teleport_data = data;
    let lp = rbx.local_player;
    rbx.fire(
        lp,
        "OnTeleport",
        vec![
            enum_value(enum_item("TeleportState", "Started")),
            Value::Nil,
            Value::Nil,
        ],
    );
}

/// A teleport that can't be: TeleportInitFailed and OnTeleport say so, and
/// the player is told, as Roblox's own prompt tells them.
fn teleport_failed(rbx: &mut Rbx, why: &str) {
    let lp = rbx.local_player;
    let lpv = rbx.value(lp);
    if let Some(ts) = rbx.service("TeleportService") {
        rbx.fire(
            ts,
            "TeleportInitFailed",
            vec![
                lpv,
                enum_value(enum_item("TeleportResult", "GameNotFound")),
                Value::str(why),
            ],
        );
    }
    rbx.fire(
        lp,
        "OnTeleport",
        vec![enum_value(enum_item("TeleportState", "Failed")), Value::Nil, Value::Nil],
    );
    note(rbx, &format!("Teleport failed: {why}"));
}

/// A method of a virtual input object (VirtualInputManager's clicks and
/// keys, VirtualUser's): the player's activity, as Roblox's client counts
/// it, which the server sees too (no idle kick).
pub fn called(rbx: &mut Rbx, id: Id, name: &str) {
    let class = rbx.inst(id).class.name;
    let input = match class {
        "VirtualInputManager" => name.starts_with("Send"),
        "VirtualUser" => matches!(
            name,
            "Button1Down"
                | "Button1Up"
                | "Button2Down"
                | "Button2Up"
                | "ClickButton1"
                | "ClickButton2"
                | "TypeKey"
                | "SetKeyDown"
                | "SetKeyUp"
                | "MoveMouse"
        ),
        _ => false,
    };
    if input {
        rbx.client.active_at = rbx.now;
        if !rbx.client.actions.contains(&ClientAction::Activity) {
            rbx.client.actions.push(ClientAction::Activity);
        }
    }
}

/// Now in UTC as Minecraft names its screenshots (2026-11-02_12.30.45).
fn stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}_{:02}.{:02}.{:02}",
        rest / 3600,
        rest / 60 % 60,
        rest % 60
    )
}
