// Horizon Studio's voice chat (Simple Voice Chat servers), without the game:
// the engine (the bundle crate hstudio_voice) with the player's settings
// (voice.txt in the studio's folder), the version each server answered in
// last, its keys (push to talk, mute, deafen), what Lua and the HUD see of
// it, its rows in Options > Mods, and the HUD's microphone and speakers.
// hstudio_voice_game feeds it from Horizon's packets and world; the tests
// feed it by hand.

use super::hstudio::{Corner, GameApi, VoiceChange, VoiceInfo};
use hstudio_voice::audio::{self, Devices};
use hstudio_voice::proto::{COMPAT_21, uuid_string};
use hstudio_voice::{Activation, ServerHost, Settings, State, VoiceChat, World};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Push to talk, mute and deafen's keys, as the player can choose them in
/// Options > Mods. SVC 2.6 and 2.7 leave push to talk unbound
/// (`InputConstants.UNKNOWN`, until the player picks one); Caps Lock is
/// ours, a key the game leaves free. Mute and "disable voice chat" are SVC's
/// own M and N.
pub const PTT_KEY: &str = "key.keyboard.caps.lock";
pub const MUTE_KEY: &str = "key.keyboard.m";
pub const DEAFEN_KEY: &str = "key.keyboard.n";
/// The keys' names in Options > Mods: (name, what it does, default).
pub const KEYS: [(&str, &str, &str); 3] = [
    ("voice_ptt", "Voice chat: push to talk", PTT_KEY),
    ("voice_mute", "Voice chat: mute the microphone", MUTE_KEY),
    ("voice_deafen", "Voice chat: deafen", DEAFEN_KEY),
];
/// The settings' file, in the studio's folder.
pub const FILE: &str = "voice.txt";

/// The rows of Options > Mods' steps.
const VOLUMES: [f32; 10] = [0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 2.5, 3.0];
const GAINS: [f32; 9] = [-20.0, -10.0, -5.0, 0.0, 5.0, 10.0, 15.0, 20.0, 24.0];
const THRESHOLDS: [f32; 7] = [-70.0, -60.0, -50.0, -40.0, -30.0, -20.0, -10.0];

/// A key's name for people: `key.keyboard.caps.lock` is "Caps Lock".
pub fn key_label(key: &str) -> String {
    let rest = key
        .strip_prefix("key.keyboard.")
        .or_else(|| key.strip_prefix("key.mouse.").map(|_| key.trim_start_matches("key.")))
        .unwrap_or(key);
    rest.split('.')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The next value of `steps` after `now` (round again after the last).
fn step(steps: &[f32], now: f32) -> f32 {
    steps.iter().copied().find(|&s| s > now + 1e-3).unwrap_or(steps[0])
}

/// The voice chat, its settings and keys.
pub struct Glue {
    pub vc: VoiceChat,
    dir: Option<PathBuf>,
    /// The version each server answered in last, by its address.
    servers: BTreeMap<String, i32>,
    server: Option<String>,
    keys: [String; 3],
    playing: bool,
}

impl Glue {
    /// The voice chat on these devices, with the settings kept in `dir`.
    pub fn new(devices: Arc<dyn Devices>, dir: Option<PathBuf>) -> Glue {
        let text = dir
            .as_ref()
            .and_then(|d| std::fs::read_to_string(d.join(FILE)).ok())
            .unwrap_or_default();
        let servers = text
            .lines()
            .filter_map(|l| l.strip_prefix("server:"))
            .filter_map(|l| l.rsplit_once('='))
            .filter_map(|(s, c)| Some((s.to_owned(), c.trim().parse().ok()?)))
            .collect();
        Glue {
            vc: VoiceChat::new(devices, Settings::from_text(&text)),
            dir,
            servers,
            server: None,
            keys: KEYS.map(|(_, _, k)| k.to_owned()),
            playing: false,
        }
    }

    /// Where the settings are kept, from now on (read from there unless set before).
    pub fn configure(&mut self, dir: PathBuf) {
        if self.dir.is_some() {
            return;
        }
        if let Ok(text) = std::fs::read_to_string(dir.join(FILE)) {
            self.vc.set_settings(Settings::from_text(&text));
            for l in text.lines().filter_map(|l| l.strip_prefix("server:")) {
                if let Some((s, c)) = l.rsplit_once('=')
                    && let Ok(c) = c.trim().parse()
                {
                    self.servers.insert(s.to_owned(), c);
                }
            }
        }
        self.dir = Some(dir);
    }

    fn save(&self) {
        let Some(dir) = &self.dir else { return };
        let mut text = self.vc.settings().to_text();
        for (s, c) in &self.servers {
            text.push_str(&format!("server:{s}={c}\n"));
        }
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(dir.join(FILE), text);
    }

    /// Joined `server` (its Login): `host` finds its host.
    pub fn joined(&mut self, server: &str, host: ServerHost, now: Instant) {
        let first = self.servers.get(server).copied().unwrap_or(COMPAT_21);
        self.server = Some(server.to_owned());
        self.vc.join(host, first, now);
    }

    pub fn left(&mut self) {
        if self.vc.state() != State::Off {
            self.vc.leave();
        }
        self.server = None;
    }

    /// The server has voice chat (it sent the secret).
    pub fn has_voice(&self) -> bool {
        matches!(
            self.vc.state(),
            State::Connecting | State::Connected | State::Lost | State::Failed
        )
    }

    /// A custom payload from the server: whether voice chat took it.
    pub fn payload(&mut self, channel: &str, data: &[u8], now: Instant) -> bool {
        self.vc.payload(channel, data, now)
    }

    pub fn chat_line(&mut self, text: &str, now: Instant) {
        self.vc.chat_line(text, now);
    }

    /// The payloads to send to the server now: (channel, data).
    pub fn take_payloads(&mut self) -> Vec<(String, Vec<u8>)> {
        self.vc
            .take_payloads()
            .into_iter()
            .map(|p| (p.channel, p.data))
            .collect()
    }

    /// The players whose places the voices need.
    pub fn wanted(&self) -> Vec<u128> {
        self.vc.wanted()
    }

    /// Each frame: where everyone is, whether the player is playing (no
    /// screen: the keys count), and the keys they chose.
    pub fn frame(&mut self, world: World, playing: bool, keys: Option<[String; 3]>, now: Instant) {
        self.playing = playing;
        if let Some(k) = keys {
            self.keys = k;
        }
        if !playing {
            self.vc.set_ptt(false);
        }
        self.vc.update(world, now);
        // The version this server answered in, for next time.
        if let (Some(server), Some(c)) = (&self.server, self.vc.info(now).compat)
            && self.servers.get(server) != Some(&c)
        {
            self.servers.insert(server.clone(), c);
            self.save();
        }
    }

    /// A key went down or up (`key.keyboard.m`): push to talk while held,
    /// mute and deafen at a press. Whether it was one of them.
    pub fn key(&mut self, key: &str, pressed: bool) -> bool {
        if key == self.keys[0] {
            self.vc.set_ptt(pressed && self.playing);
            return true;
        }
        if !pressed || !self.playing || !self.has_voice() {
            return key == self.keys[1] || key == self.keys[2];
        }
        let s = self.vc.settings().clone();
        if key == self.keys[1] {
            let _ = self.set(VoiceChange::Mute(!s.muted));
            return true;
        }
        if key == self.keys[2] {
            let _ = self.set(VoiceChange::Deafen(!s.deafened));
            return true;
        }
        false
    }

    pub fn info(&self, now: Instant) -> VoiceInfo {
        let i = self.vc.info(now);
        let s = &i.settings;
        let mic = match &i.input {
            Some(Ok(name)) => Some(name.clone()),
            Some(Err(e)) => Some(format!("No microphone: {e}")),
            None => None,
        };
        let error = i.error.clone().or_else(|| match i.state {
            State::Incompatible => Some("The server's voice chat is a version this client doesn't speak".into()),
            _ => None,
        });
        VoiceInfo {
            available: true,
            state: i.state.name().to_owned(),
            connected: i.state == State::Connected,
            unavailable: self.vc.unavailable(now),
            talking: i.talking,
            level_db: i.level_db,
            muted: s.muted,
            deafened: s.deafened,
            activation: s.activation.name().to_owned(),
            ptt_key: self.keys[0].clone(),
            ptt: i.ptt,
            volume: s.volume,
            gain_db: s.gain_db,
            threshold_db: s.threshold_db,
            speakers: i.speaking.iter().map(|p| (p.name.clone(), p.whispering)).collect(),
            players: i.players.iter().map(|p| p.1.clone()).collect(),
            groups: i
                .groups
                .iter()
                .map(|g| (uuid_string(g.id), g.name.clone(), g.has_password))
                .collect(),
            group: i.group.map(uuid_string),
            compat: i.compat,
            error,
            mic,
        }
    }

    pub fn set(&mut self, change: VoiceChange) -> Result<(), String> {
        let mut s = self.vc.settings().clone();
        match change {
            VoiceChange::Mute(on) => s.muted = on,
            VoiceChange::Deafen(on) => s.deafened = on,
            VoiceChange::Volume(x) => s.volume = x,
            VoiceChange::Gain(x) => s.gain_db = x,
            VoiceChange::Threshold(x) => s.threshold_db = x,
            VoiceChange::Activation(m) => {
                s.activation =
                    Activation::parse(&m).ok_or_else(|| format!("no activation {m:?}: \"ptt\" or \"voice\""))?;
            }
            VoiceChange::JoinGroup(id, password) => {
                let i = self.vc.info(Instant::now());
                let group = i
                    .groups
                    .iter()
                    .find(|g| uuid_string(g.id) == id.to_ascii_lowercase())
                    .ok_or("no such group")?;
                return self.vc.join_group(group.id, password.as_deref());
            }
            VoiceChange::LeaveGroup => {
                self.vc.leave_group();
                return Ok(());
            }
        }
        self.vc.set_settings(s);
        self.save();
        Ok(())
    }

    /// Options > Mods > Horizon Studio's voice chat rows.
    pub fn settings_buttons(&self) -> Vec<String> {
        let i = self.info(Instant::now());
        let on_off = |b: bool| if b { "On" } else { "Off" };
        vec![
            format!("Voice chat: {}", state_line(&i)),
            match i.activation.as_str() {
                "voice" => format!("Microphone: voice activation (above {:.0} dB)", i.threshold_db),
                _ => format!("Microphone: push to talk ({})", key_label(&i.ptt_key)),
            },
            format!("Mute the microphone: {}", on_off(i.muted)),
            format!("Deafen: {}", on_off(i.deafened)),
            format!("Voice volume: {:.0}%", i.volume * 100.0),
            format!("Microphone gain: {:+.0} dB", i.gain_db),
            format!("Voice activation threshold: {:.0} dB", i.threshold_db),
        ]
    }

    /// A click on one of them (its index among them).
    pub fn settings_click(&mut self, row: usize) {
        let s = self.vc.settings().clone();
        let change = match row {
            1 => VoiceChange::Activation(
                if s.activation == Activation::PushToTalk {
                    "voice"
                } else {
                    "ptt"
                }
                .into(),
            ),
            2 => VoiceChange::Mute(!s.muted),
            3 => VoiceChange::Deafen(!s.deafened),
            4 => VoiceChange::Volume(step(&VOLUMES, s.volume)),
            5 => VoiceChange::Gain(step(&GAINS, s.gain_db)),
            6 => VoiceChange::Threshold(step(&THRESHOLDS, s.threshold_db)),
            _ => return,
        };
        let _ = self.set(change);
    }
}

/// Where the voice chat is, for people: "Connected (3 players)", "Not on a server"...
pub fn state_line(i: &VoiceInfo) -> String {
    match i.state.as_str() {
        "connected" => format!("Connected ({} players)", i.players.len()),
        "connecting" if i.unavailable => "Unavailable (not connecting)".into(),
        "connecting" | "asking" => "Connecting".into(),
        "none" => "This server has none".into(),
        "incompatible" => "Unavailable: another version".into(),
        "lost" => "Unavailable: the server stopped answering".into(),
        "failed" => format!("Unavailable: {}", i.error.clone().unwrap_or_default()),
        _ => "Not on a server".into(),
    }
}

/// The studio's Settings tab's voice rows (`Action::Voice`): 0 push to talk,
/// 1 voice activation, 2 mute, 3 deafen, then down and up: 4 and 5 the
/// volume (10%), 6 and 7 the gain (2 dB), 8 and 9 the threshold (5 dB).
pub fn settings_action(game: &mut dyn GameApi, n: u8) {
    let v = game.voice();
    let change = match n {
        0 => VoiceChange::Activation("ptt".into()),
        1 => VoiceChange::Activation("voice".into()),
        2 => VoiceChange::Mute(!v.muted),
        3 => VoiceChange::Deafen(!v.deafened),
        4 | 5 => {
            VoiceChange::Volume((((v.volume + if n == 5 { 0.1 } else { -0.1 }) * 10.0).round() / 10.0).clamp(0.0, 3.0))
        }
        6 | 7 => VoiceChange::Gain((v.gain_db + if n == 7 { 2.0 } else { -2.0 }).clamp(-40.0, 24.0)),
        8 | 9 => VoiceChange::Threshold((v.threshold_db + if n == 9 { 5.0 } else { -5.0 }).clamp(-127.0, 0.0)),
        _ => return,
    };
    let _ = game.voice_set(change);
}

static GLOBAL: Mutex<Option<Glue>> = Mutex::new(None);

/// The game's voice chat (made at first use, on the system's devices).
pub fn global<R>(f: impl FnOnce(&mut Glue) -> R) -> R {
    let mut g = GLOBAL.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(|| Glue::new(audio::system(), None)))
}

/// The keys for Options > Mods: (name, what it does, default).
pub fn mod_keys() -> Vec<[String; 3]> {
    KEYS.iter()
        .map(|(n, d, k)| [(*n).into(), (*d).into(), (*k).into()])
        .collect()
}

// ----------------------------------------------------------------- the HUD

const GREEN: u32 = 0xFF55_FF55;
const RED: u32 = 0xFFFF_5555;
const GREY: u32 = 0xFFAA_AAAA;
const WHITE: u32 = 0xFFFF_FFFF;

/// A microphone 9 pixels wide and 12 high at (x, y); struck through when off.
fn mic_icon(game: &mut dyn GameApi, x: f32, y: f32, argb: u32, struck: bool) {
    for r in [
        [x + 3.0, y, 3.0, 1.0],
        [x + 2.0, y + 1.0, 5.0, 6.0],
        [x + 3.0, y + 7.0, 3.0, 1.0],
        [x, y + 4.0, 1.0, 3.0],
        [x + 8.0, y + 4.0, 1.0, 3.0],
        [x + 1.0, y + 7.0, 1.0, 1.0],
        [x + 7.0, y + 7.0, 1.0, 1.0],
        [x + 2.0, y + 8.0, 5.0, 1.0],
        [x + 4.0, y + 9.0, 1.0, 2.0],
        [x + 2.0, y + 11.0, 5.0, 1.0],
    ] {
        game.fill(r, argb);
    }
    if struck {
        for i in 0..12 {
            game.fill([x + 8.0 - i as f32 * 0.75, y + i as f32, 1.0, 1.0], RED);
        }
    }
}

/// A loudspeaker 7 pixels wide and 8 high at (x, y).
fn speaker_icon(game: &mut dyn GameApi, x: f32, y: f32, argb: u32) {
    for r in [
        [x, y + 3.0, 2.0, 2.0],
        [x + 2.0, y + 2.0, 1.0, 4.0],
        [x + 3.0, y + 1.0, 1.0, 6.0],
        [x + 4.0, y, 1.0, 8.0],
        [x + 6.0, y + 2.0, 1.0, 4.0],
    ] {
        game.fill(r, argb);
    }
}

/// The HUD: the microphone while it sends, muted, deafened or "voice chat
/// unavailable", and the names of those talking. Nothing where the server
/// has no voice chat.
pub fn draw_hud(game: &mut dyn GameApi) {
    let v = game.voice();
    if !v.available
        || !matches!(
            v.state.as_str(),
            "connected" | "connecting" | "lost" | "failed" | "incompatible"
        )
    {
        return;
    }
    let status: Option<(&str, u32, bool)> = if v.unavailable {
        Some(("Voice chat unavailable", GREY, true))
    } else if v.deafened {
        Some(("Deafened", RED, true))
    } else if v.muted {
        Some(("Muted", RED, true))
    } else if v.talking {
        Some(("", GREEN, false))
    } else {
        None
    };
    let speakers: Vec<&(String, bool)> = if v.connected && !v.deafened {
        v.speakers.iter().take(8).collect()
    } else {
        Vec::new()
    };
    if status.is_none() && speakers.is_empty() {
        return;
    }
    let row = 13.0;
    let mut w: f32 = 0.0;
    if let Some((label, _, _)) = status {
        w = w.max(
            12.0 + if label.is_empty() {
                0.0
            } else {
                game.text_width(label) + 3.0
            },
        );
    }
    for (name, _) in &speakers {
        w = w.max(11.0 + game.text_width(name));
    }
    let lines = usize::from(status.is_some()) + speakers.len();
    let (pw, ph) = (w + 6.0, row * lines as f32 + 3.0);
    let (x, y) = game.place(Corner::TopLeft, pw, ph);
    game.fill([x, y, pw, ph], 0x7800_0000);
    let mut ly = y + 2.0;
    if let Some((label, color, struck)) = status {
        mic_icon(game, x + 3.0, ly, color, struck);
        if !label.is_empty() {
            game.text(label, [x + 15.0, ly + 2.0], color & 0xFF_FFFF, true);
        }
        ly += row;
    }
    for (name, whispering) in speakers {
        speaker_icon(game, x + 3.0, ly + 2.0, if *whispering { GREY } else { GREEN });
        game.text(name, [x + 14.0, ly + 2.0], WHITE & 0xFF_FFFF, true);
        ly += row;
    }
}
