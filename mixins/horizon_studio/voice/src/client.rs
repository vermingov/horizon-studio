//! The voice chat on one server: the payloads on the Minecraft connection
//! (`ClientManager`, `ClientPlayerStateManager`), and the voice connection
//! they start. At a join it announces the server's channels and asks for
//! the secret in one compatibility version; with no answer in a few seconds
//! (or the server's "incompatible version" line) it asks in the other. The
//! secret starts the voice connection; a new secret, which the server sends
//! when the old connection timed out, starts it again. The game moves
//! payloads both ways (`payload`, `take_payloads`), and tells it where
//! everyone is each frame (`update`).

use crate::audio::Devices;
use crate::mic::{Activation, MAX_GAIN_DB, MIN_GAIN_DB};
use crate::net::{self, Connection, Phase, Shared, World};
use crate::proto::{self, COMPAT_20, COMPAT_21, Clientbound, Group, PlayerState, Secret};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How long to wait for the secret before asking in the other version.
pub const ASK_TIMEOUT: Duration = Duration::from_secs(4);
/// How long connecting may take before the HUD says voice chat is unavailable.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// The player's voice chat settings, kept between games.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub activation: Activation,
    /// The voices' volume, 0 to 3 (1 as they come; SVC's `voice_chat_volume`).
    pub volume: f32,
    /// The microphone's gain in dB, -40 to 24 (SVC's `microphone_gain`).
    pub gain_db: f32,
    /// Voice activation's threshold in dBFS, -127 to 0 (SVC's `voice_activation_threshold`).
    pub threshold_db: f32,
    pub muted: bool,
    /// Hear nothing and send nothing (SVC's "disable voice chat").
    pub deafened: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            activation: Activation::PushToTalk,
            volume: 1.0,
            gain_db: 0.0,
            threshold_db: -50.0,
            muted: false,
            deafened: false,
        }
    }
}

impl Settings {
    /// Every value within its range.
    pub fn clamped(mut self) -> Settings {
        let fix = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        self.volume = fix(self.volume, 0.0, 3.0, 1.0);
        self.gain_db = fix(self.gain_db, MIN_GAIN_DB, MAX_GAIN_DB, 0.0);
        self.threshold_db = fix(self.threshold_db, -127.0, 0.0, -50.0);
        self
    }

    /// `key=value` lines.
    pub fn to_text(&self) -> String {
        format!(
            "activation={}\nvolume={}\ngain_db={}\nthreshold_db={}\nmuted={}\ndeafened={}\n",
            self.activation.name(),
            self.volume,
            self.gain_db,
            self.threshold_db,
            self.muted,
            self.deafened
        )
    }

    /// What `to_text` wrote (unknown lines and bad values are skipped).
    pub fn from_text(text: &str) -> Settings {
        let mut s = Settings::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            let num = v.parse::<f32>().ok();
            match k {
                "activation" => s.activation = Activation::parse(v).unwrap_or(s.activation),
                "volume" => s.volume = num.unwrap_or(s.volume),
                "gain_db" => s.gain_db = num.unwrap_or(s.gain_db),
                "threshold_db" => s.threshold_db = num.unwrap_or(s.threshold_db),
                "muted" => s.muted = v == "true",
                "deafened" => s.deafened = v == "true",
                _ => {}
            }
        }
        s.clamped()
    }
}

/// Where the voice chat is on this server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Not on a server.
    Off,
    /// Asked for the secret; no answer yet.
    Asking,
    /// The server has no voice chat (it never answered).
    NoVoiceChat,
    /// The server's voice chat is a version this client doesn't speak.
    Incompatible,
    /// The voice connection is starting.
    Connecting,
    Connected,
    /// The voice connection timed out; the server sends a new secret.
    Lost,
    /// It couldn't connect (an address that doesn't resolve...).
    Failed,
}

impl State {
    pub fn name(self) -> &'static str {
        match self {
            State::Off => "off",
            State::Asking => "asking",
            State::NoVoiceChat => "none",
            State::Incompatible => "incompatible",
            State::Connecting => "connecting",
            State::Connected => "connected",
            State::Lost => "lost",
            State::Failed => "failed",
        }
    }
}

/// A payload to send on the Minecraft connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Payload {
    pub channel: String,
    pub data: Vec<u8>,
}

/// Someone heard now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Speaking {
    pub uuid: u128,
    pub name: String,
    pub whispering: bool,
}

/// What the game shows of the voice chat.
#[derive(Clone, Debug)]
pub struct Info {
    pub state: State,
    /// The compatibility version the server answered in.
    pub compat: Option<i32>,
    /// The microphone is sending.
    pub talking: bool,
    /// The microphone's level, in dBFS.
    pub level_db: f32,
    pub speaking: Vec<Speaking>,
    pub settings: Settings,
    pub ptt: bool,
    /// The players with voice chat, by name: (uuid, name, disabled, connected).
    pub players: Vec<(u128, String, bool, bool)>,
    pub groups: Vec<Group>,
    pub group: Option<u128>,
    pub groups_enabled: bool,
    pub distance: f64,
    pub output: Option<Result<String, String>>,
    pub input: Option<Result<String, String>>,
    pub error: Option<String>,
    pub address: Option<SocketAddr>,
    /// Voice packets sent and received, and frames played.
    pub sent: u64,
    pub received: u64,
    pub played: u64,
}

/// How the Minecraft server's host is found (it may take a DNS lookup):
/// called on the voice thread.
pub type ServerHost = Arc<dyn Fn() -> String + Send + Sync>;

/// One player's voice chat, from join to leave.
pub struct VoiceChat {
    devices: Arc<dyn Devices>,
    shared: Arc<Shared>,
    settings: Settings,
    ptt: bool,
    server: Option<ServerHost>,
    state: State,
    /// The version asked last, those left to ask, and when.
    compat: i32,
    left: Vec<i32>,
    asked_at: Instant,
    answered: bool,
    secret: Option<(Secret, i32)>,
    connecting_since: Instant,
    conn: Option<Connection>,
    states: HashMap<u128, PlayerState>,
    groups: HashMap<u128, Group>,
    group: Option<u128>,
    out: Vec<Payload>,
}

impl VoiceChat {
    pub fn new(devices: Arc<dyn Devices>, settings: Settings) -> VoiceChat {
        let vc = VoiceChat {
            devices,
            shared: Arc::new(Shared::new()),
            settings: settings.clamped(),
            ptt: false,
            server: None,
            state: State::Off,
            compat: COMPAT_21,
            left: Vec::new(),
            asked_at: Instant::now(),
            answered: false,
            secret: None,
            connecting_since: Instant::now(),
            conn: None,
            states: HashMap::new(),
            groups: HashMap::new(),
            group: None,
            out: Vec::new(),
        };
        vc.push_controls();
        vc
    }

    /// The mixer, the microphone buffer and the thread's status (tests drive them).
    pub fn shared(&self) -> &Arc<Shared> {
        &self.shared
    }

    pub fn state(&self) -> State {
        self.state
    }

    fn push_controls(&self) {
        let s = &self.settings;
        let ptt = self.ptt;
        self.shared.set_controls(|c| {
            c.activation = s.activation;
            c.volume = s.volume;
            c.gain_db = s.gain_db;
            c.threshold_db = s.threshold_db;
            c.muted = s.muted;
            c.deafened = s.deafened;
            c.ptt = ptt;
        });
    }

    fn send(&mut self, channel: &str, data: Vec<u8>) {
        self.out.push(Payload {
            channel: channel.to_owned(),
            data,
        });
    }

    fn ask(&mut self, compat: i32, now: Instant) {
        self.compat = compat;
        self.asked_at = now;
        self.state = State::Asking;
        self.send(proto::REQUEST_SECRET, proto::request_secret(compat));
    }

    /// Joined a server (its Login): `server` finds its host, `first` is the
    /// version to ask in first (the one it answered in last time, else 21).
    pub fn join(&mut self, server: ServerHost, first: i32, now: Instant) {
        self.leave();
        self.server = Some(server);
        let first = if first == COMPAT_20 { COMPAT_20 } else { COMPAT_21 };
        self.left = vec![if first == COMPAT_21 { COMPAT_20 } else { COMPAT_21 }];
        self.answered = false;
        self.send(proto::REGISTER, proto::register());
        self.ask(first, now);
    }

    /// Left the server: everything stops.
    pub fn leave(&mut self) {
        self.conn = None;
        self.server = None;
        self.secret = None;
        self.state = State::Off;
        self.states.clear();
        self.groups.clear();
        self.group = None;
        self.left.clear();
        self.out.clear();
        self.shared.mixer.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    /// A custom payload from the server: whether it was voice chat's (all of
    /// them are taken, and one that doesn't parse is dropped).
    pub fn payload(&mut self, channel: &str, data: &[u8], now: Instant) -> bool {
        let Some(packet) = proto::decode_clientbound(channel, data, self.compat) else {
            return false;
        };
        if self.state == State::Off {
            return true;
        }
        self.answered = true;
        let Ok(packet) = packet else { return true };
        match packet {
            Clientbound::Secret(_) => {
                if let Ok((secret, compat)) = Secret::decode_any(data, self.compat) {
                    self.start(secret, compat, now);
                }
            }
            Clientbound::States(list) => {
                self.states = list.into_iter().map(|s| (s.uuid, s)).collect();
            }
            Clientbound::State(s) => {
                if s.disconnected {
                    self.shared
                        .mixer
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .remove(s.uuid);
                }
                self.states.insert(s.uuid, s);
            }
            Clientbound::RemoveState(u) => {
                self.states.remove(&u);
                self.shared.mixer.lock().unwrap_or_else(|e| e.into_inner()).remove(u);
            }
            Clientbound::AddGroup(g) => {
                self.groups.insert(g.id, g);
            }
            Clientbound::RemoveGroup(id) => {
                self.groups.remove(&id);
            }
            Clientbound::JoinedGroup { group, .. } => self.group = group,
            Clientbound::Category => {}
        }
        true
    }

    /// A line in the player's chat: the server's "incompatible version"
    /// message asks again in the other version at once.
    pub fn chat_line(&mut self, text: &str, now: Instant) {
        if self.state != State::Asking {
            return;
        }
        let t = text.to_ascii_lowercase();
        let incompatible = t.contains("incompatible_version")
            || (t.contains("voice") && t.contains("chat") && (t.contains("version") || t.contains("compatible")));
        if !incompatible {
            return;
        }
        match self.left.pop() {
            Some(c) => self.ask(c, now),
            None => self.state = State::Incompatible,
        }
    }

    fn start(&mut self, secret: Secret, compat: i32, now: Instant) {
        self.conn = None;
        self.compat = compat;
        self.left.clear();
        let (channel, data) = proto::client_state(compat, self.settings.muted, self.settings.deafened);
        self.send(channel, data);
        let server = self.server.clone();
        let (voice_host, port) = (secret.voice_host.clone(), secret.server_port);
        let address = Box::new(move || {
            let host = server.as_ref().map_or_else(|| "127.0.0.1".to_owned(), |f| f());
            let (host, port) = proto::voice_target(&voice_host, &host, port);
            net::resolve(&host, port)
        });
        let config = net::Config {
            secret: secret.clone(),
            compat,
            address,
        };
        self.conn = Some(Connection::start(config, self.shared.clone(), self.devices.clone()));
        self.secret = Some((secret, compat));
        self.state = State::Connecting;
        self.connecting_since = now;
    }

    /// Each frame: where the listener and the others are, and the timers.
    pub fn update(&mut self, world: World, now: Instant) {
        self.shared.set_world(world);
        if self.state == State::Asking && !self.answered && now - self.asked_at >= ASK_TIMEOUT {
            match self.left.pop() {
                Some(c) => self.ask(c, now),
                None => self.state = State::NoVoiceChat,
            }
        }
        if self.conn.is_some() {
            self.state = match self.shared.status().phase {
                Phase::Connected => State::Connected,
                Phase::TimedOut => State::Lost,
                Phase::Failed => State::Failed,
                Phase::Resolving | Phase::Authenticating | Phase::Checking => State::Connecting,
            };
            if self.conn.as_ref().is_some_and(Connection::finished) {
                self.conn = None;
            }
        }
    }

    /// The payloads to send now.
    pub fn take_payloads(&mut self) -> Vec<Payload> {
        std::mem::take(&mut self.out)
    }

    /// The players whose places the voices need.
    pub fn wanted(&self) -> Vec<u128> {
        let mut v: Vec<u128> = self.states.keys().copied().collect();
        for u in self.shared.status().speaking.keys() {
            if !v.contains(u) {
                v.push(*u);
            }
        }
        v
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// New settings: the thread follows at once; a change of mute or deafen
    /// tells the server.
    pub fn set_settings(&mut self, settings: Settings) {
        let settings = settings.clamped();
        let told = (self.settings.muted, self.settings.deafened) != (settings.muted, settings.deafened);
        self.settings = settings;
        self.push_controls();
        if told && self.secret.is_some() {
            let (channel, data) = proto::client_state(self.compat, self.settings.muted, self.settings.deafened);
            self.send(channel, data);
        }
    }

    /// Mutes or unmutes a player's voice (by UUID), or everyone's (None,
    /// which also forgets each player's own): for this player alone, the
    /// server isn't told.
    pub fn mute_voice(&mut self, who: Option<u128>, muted: bool) {
        self.shared.set_controls(|c| match who {
            Some(u) => {
                c.muted_players.retain(|x| *x != u);
                if muted {
                    c.muted_players.push(u);
                }
            }
            None => {
                c.muted_players.clear();
                c.muted_voices = muted;
            }
        });
    }

    /// Whether a player's voice is muted here (on its own or with everyone's).
    pub fn voice_muted(&self, who: u128) -> bool {
        let c = self.shared.controls();
        c.muted_voices || c.muted_players.contains(&who)
    }

    /// The push to talk key, held or not.
    pub fn set_ptt(&mut self, held: bool) {
        if self.ptt != held {
            self.ptt = held;
            self.push_controls();
        }
    }

    /// Joins a group (its password if it has one).
    pub fn join_group(&mut self, id: u128, password: Option<&str>) -> Result<(), String> {
        if self.secret.as_ref().is_none_or(|(s, _)| !s.groups) {
            return Err("this server's voice chat has no groups".into());
        }
        if !self.groups.contains_key(&id) {
            return Err("no such group".into());
        }
        self.send(proto::SET_GROUP, proto::set_group(id, password));
        Ok(())
    }

    pub fn leave_group(&mut self) {
        if self.group.is_some() {
            self.send(proto::LEAVE_GROUP, Vec::new());
        }
    }

    fn name_of(&self, uuid: u128) -> String {
        self.states
            .get(&uuid)
            .map_or_else(|| proto::uuid_string(uuid), |s| s.name.clone())
    }

    pub fn info(&self, now: Instant) -> Info {
        let status = self.shared.status();
        let mut speaking: Vec<Speaking> = status
            .speaking
            .iter()
            .map(|(u, (_, whispering))| Speaking {
                uuid: *u,
                name: self.name_of(*u),
                whispering: *whispering,
            })
            .collect();
        speaking.sort_by(|a, b| a.name.cmp(&b.name));
        let mut players: Vec<(u128, String, bool, bool)> = self
            .states
            .values()
            .map(|s| (s.uuid, s.name.clone(), s.disabled, !s.disconnected))
            .collect();
        players.sort_by(|a, b| a.1.cmp(&b.1));
        let mut groups: Vec<Group> = self.groups.values().cloned().collect();
        groups.sort_by(|a, b| a.name.cmp(&b.name));
        let _ = now;
        Info {
            state: self.state,
            compat: self.secret.as_ref().map(|(_, c)| *c),
            talking: status.talking,
            level_db: status.level_db,
            speaking,
            settings: self.settings.clone(),
            ptt: self.ptt,
            players,
            groups,
            group: self.group,
            groups_enabled: self.secret.as_ref().is_some_and(|(s, _)| s.groups),
            distance: self.secret.as_ref().map_or(0.0, |(s, _)| s.distance),
            output: status.output,
            input: status.input,
            error: status.error,
            address: status.address,
            sent: status.sent,
            received: status.received,
            played: status.played,
        }
    }

    /// Whether the HUD should say voice chat is unavailable: the server has
    /// it, but it isn't working.
    pub fn unavailable(&self, now: Instant) -> bool {
        match self.state {
            State::Incompatible | State::Lost | State::Failed => true,
            State::Connecting => now - self.connecting_since >= CONNECT_TIMEOUT,
            _ => false,
        }
    }
}
