//! The voice connection's thread: one UDP socket to the voice server
//! (`ClientVoicechatConnection`). It authenticates (every second until
//! answered), checks the connection (the same, with compat 21's nonce),
//! answers keep-alives and pings, decodes the speakers' packets into the
//! mixer at their places, and sends the microphone's frames as SVC's
//! `MicThread` does: Opus at 48 kHz mono, then one empty packet when the
//! speech stops. It stops when told, or when the server falls silent for ten
//! keep-alive intervals (the server then sends a new secret).

use crate::audio::{Devices, Stream};
use crate::crypt::{self, Cipher};
use crate::jitter::{Opus, Packet, Played, Speaker};
use crate::mic::{Activation, LOWEST_DB, MicBuffer, MicProcessor};
use crate::mixer::Mixer;
use crate::opus::{Application, OpusEncoder, SignalType};
use crate::proto::{COMPAT_21, Codec, FRAME_SIZE, MAX_OPUS_PAYLOAD, MAX_PACKET, SAMPLE_RATE, Secret, Udp};
use crate::spatial::{self, Listener};
use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How often authentication and the connection check are sent until answered.
const RETRY: Duration = Duration::from_millis(1000);
/// A speaker idle this long is forgotten (`AudioChannel.canKill`).
const SPEAKER_IDLE: Duration = Duration::from_secs(30);
/// How long a speaker counts as talking after their last frame.
const TALKING_FOR: Duration = Duration::from_millis(250);
/// How long before a microphone that failed to open is tried again.
const MIC_RETRY: Duration = Duration::from_secs(5);

/// The player's choices, as the network thread reads them each turn.
#[derive(Clone, Debug, PartialEq)]
pub struct Controls {
    pub activation: Activation,
    /// The voices' volume, 0 to 3.
    pub volume: f32,
    pub gain_db: f32,
    pub threshold_db: f32,
    pub muted: bool,
    /// Nothing heard and nothing sent (SVC's "disabled").
    pub deafened: bool,
    /// The push to talk key is held.
    pub ptt: bool,
    /// Players this player doesn't hear (by UUID), and whether they hear
    /// no one else at all: their own choice, the server isn't told.
    pub muted_players: Vec<u128>,
    pub muted_voices: bool,
}

impl Default for Controls {
    fn default() -> Self {
        Controls {
            activation: Activation::PushToTalk,
            volume: 1.0,
            gain_db: 0.0,
            threshold_db: -50.0,
            muted: false,
            deafened: false,
            ptt: false,
            muted_players: Vec::new(),
            muted_voices: false,
        }
    }
}

/// Where the listener and the other players are (their eyes), by UUID.
#[derive(Clone, Debug, Default)]
pub struct World {
    pub listener: Listener,
    /// The player's own UUID (and the camera's entity): heard without a place.
    pub own: u128,
    pub eyes: HashMap<u128, [f64; 3]>,
}

/// Where the connection is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Phase {
    /// Finding the voice server's address.
    #[default]
    Resolving,
    Authenticating,
    Checking,
    /// Up: voices come and the microphone goes.
    Connected,
    /// The server stopped answering.
    TimedOut,
    /// It couldn't start (no address, no socket).
    Failed,
}

/// What the thread tells the game.
#[derive(Clone, Debug, Default)]
pub struct Status {
    pub phase: Phase,
    pub error: Option<String>,
    /// The microphone is sending.
    pub talking: bool,
    pub level_db: f32,
    /// Who is heard now: their last frame's time, and whether they whisper.
    pub speaking: HashMap<u128, (Instant, bool)>,
    pub sent: u64,
    pub received: u64,
    /// Frames decoded into the mixer.
    pub played: u64,
    pub lost: u64,
    /// The speakers' and the microphone's device (or why there is none);
    /// None until tried.
    pub output: Option<Result<String, String>>,
    pub input: Option<Result<String, String>>,
    pub address: Option<SocketAddr>,
}

/// What the game, the thread and the audio streams share.
pub struct Shared {
    pub mixer: Arc<Mutex<Mixer>>,
    pub mic: Arc<MicBuffer>,
    pub controls: Mutex<Controls>,
    pub world: Mutex<World>,
    pub status: Mutex<Status>,
}

impl Default for Shared {
    fn default() -> Self {
        Shared::new()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Shared {
    pub fn new() -> Shared {
        Shared {
            mixer: Arc::new(Mutex::new(Mixer::new())),
            mic: Arc::new(MicBuffer::new()),
            controls: Mutex::new(Controls::default()),
            world: Mutex::new(World::default()),
            status: Mutex::new(Status::default()),
        }
    }
    pub fn controls(&self) -> Controls {
        lock(&self.controls).clone()
    }
    pub fn set_controls(&self, f: impl FnOnce(&mut Controls)) {
        f(&mut lock(&self.controls));
    }
    pub fn set_world(&self, world: World) {
        *lock(&self.world) = world;
    }
    pub fn status(&self) -> Status {
        lock(&self.status).clone()
    }
    fn set_status(&self, f: impl FnOnce(&mut Status)) {
        f(&mut lock(&self.status));
    }
}

/// What the connection needs.
pub struct Config {
    pub secret: Secret,
    pub compat: i32,
    /// The voice server's address, worked out on the thread (it may look up DNS).
    pub address: Box<dyn FnOnce() -> Result<SocketAddr, String> + Send>,
}

/// The running thread; dropping it stops it.
pub struct Connection {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Connection {
    pub fn start(config: Config, shared: Arc<Shared>, devices: Arc<dyn Devices>) -> Connection {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        shared.set_status(|s| *s = Status::default());
        let thread = std::thread::Builder::new()
            .name("hstudio-voice".into())
            .spawn(move || run(config, &shared, devices.as_ref(), &flag))
            .ok();
        Connection { stop, thread }
    }

    /// Whether the thread has ended (timed out, failed, or stopped).
    pub fn finished(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// The first of a name's addresses, IPv4 first (as Horizon connects).
pub fn resolve(host: &str, port: u16) -> Result<SocketAddr, String> {
    let all: Vec<SocketAddr> = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("Unknown host {host}: {e}"))?
        .collect();
    all.iter()
        .find(|a| a.is_ipv4())
        .or(all.first())
        .copied()
        .ok_or_else(|| format!("{host} has no address"))
}

/// How a speaker's packet is heard.
#[derive(Clone, Copy, Debug)]
enum Heard {
    Player {
        sender: u128,
        distance: f32,
        whispering: bool,
    },
    Group {
        sender: u128,
    },
    Location {
        sender: u128,
        pos: [f64; 3],
        distance: f32,
    },
}

impl Heard {
    fn sender(&self) -> u128 {
        match *self {
            Heard::Player { sender, .. } | Heard::Group { sender } | Heard::Location { sender, .. } => sender,
        }
    }
}

struct Link<'a> {
    socket: UdpSocket,
    cipher: Cipher,
    id: u128,
    shared: &'a Shared,
}

impl Link<'_> {
    fn send(&self, packet: &Udp) {
        let datagram = crypt::to_server(self.id, &self.cipher.encrypt(&packet.encode()));
        if self.socket.send(&datagram).is_ok() {
            self.shared.set_status(|s| s.sent += 1);
        }
    }
}

fn application(codec: Codec) -> Application {
    match codec {
        Codec::Voip => Application::Voip,
        Codec::Audio => Application::Audio,
        Codec::RestrictedLowDelay => Application::RestrictedLowDelay,
    }
}

/// SVC's encoder: the server's application, in-band FEC at 5% expected
/// loss, and libopus's automatic bitrate for 48 kHz mono 20 ms frames
/// (`60 * Fs / frame + Fs`), since SVC sets none.
pub fn encoder(codec: Codec) -> OpusEncoder {
    let mut e = OpusEncoder::new(SAMPLE_RATE as i32, 1, application(codec)).expect("48 kHz mono is a valid encoder");
    e.use_inband_fec = true;
    e.packet_loss_perc = 5;
    e.bitrate_bps = (60 * SAMPLE_RATE / FRAME_SIZE as u32 + SAMPLE_RATE) as i32;
    if codec == Codec::Voip {
        // Speech: SILK and hybrid, as libopus picks for a voice.
        e.signal_type = Some(SignalType::Voice);
    }
    e
}

fn run(config: Config, shared: &Shared, devices: &dyn Devices, stop: &AtomicBool) {
    let Config {
        secret,
        compat,
        address,
    } = config;
    let fail = |e: String| {
        shared.set_status(|s| {
            s.phase = Phase::Failed;
            s.error = Some(e);
        })
    };
    let addr = match address() {
        Ok(a) => a,
        Err(e) => return fail(e),
    };
    let bind: SocketAddr = if addr.is_ipv4() {
        ([0, 0, 0, 0], 0).into()
    } else {
        (std::net::Ipv6Addr::UNSPECIFIED, 0).into()
    };
    let socket = match UdpSocket::bind(bind).and_then(|s| s.connect(addr).map(|()| s)) {
        Ok(s) => s,
        Err(e) => return fail(format!("No voice socket: {e}")),
    };
    let _ = socket.set_read_timeout(Some(Duration::from_millis(5)));
    let id = if compat >= COMPAT_21 {
        secret.connection_id.unwrap_or(secret.player)
    } else {
        secret.player
    };
    let link = Link {
        socket,
        cipher: Cipher::new(&secret.secret),
        id,
        shared,
    };
    shared.set_status(|s| {
        s.phase = Phase::Authenticating;
        s.address = Some(addr);
    });
    // The speakers play from the start; the microphone opens once connected.
    let output: Option<Stream> = match devices.output(shared.mixer.clone()) {
        Ok(s) => {
            shared.set_status(|st| st.output = Some(Ok(s.name.clone())));
            Some(s)
        }
        Err(e) => {
            shared.set_status(|st| st.output = Some(Err(e)));
            None
        }
    };
    let keep_alive = Duration::from_millis(u64::try_from(secret.keep_alive.max(1000)).unwrap_or(1000));
    let mut phase = Phase::Authenticating;
    let mut nonce: Option<[u8; 16]> = None;
    let mut last_try: Option<Instant> = None;
    let mut heard_at = Instant::now();
    let mut input: Option<Stream> = None;
    let mut input_tried: Option<Instant> = None;
    let mut speakers: HashMap<u128, (Speaker<Opus, Heard>, Instant)> = HashMap::new();
    let mut processor = MicProcessor::new();
    let mut enc: Option<OpusEncoder> = None;
    let mut seq: i64 = 0;
    let mut sending = false;
    let max_payload = usize::try_from(secret.mtu)
        .unwrap_or(MAX_OPUS_PAYLOAD)
        .clamp(16, MAX_OPUS_PAYLOAD);
    let mut packet = vec![0u8; max_payload];
    let mut buf = vec![0u8; MAX_PACKET + 64];
    while !stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        // Authentication, then the connection check, every second until answered.
        if matches!(phase, Phase::Authenticating | Phase::Checking) && last_try.is_none_or(|t| now - t >= RETRY) {
            last_try = Some(now);
            if phase == Phase::Authenticating {
                let player = (compat < COMPAT_21).then_some((secret.player, secret.secret));
                link.send(&Udp::Authenticate { player });
            } else {
                link.send(&Udp::ConnectionCheck { nonce });
            }
        }
        // What came.
        match link.socket.recv(&mut buf) {
            Ok(n) => {
                let plain = crypt::from_server(&buf[..n]).and_then(|e| link.cipher.decrypt(&e));
                if let Some(Ok(p)) = plain.map(|p| Udp::decode(&p)) {
                    heard_at = now;
                    shared.set_status(|s| s.received += 1);
                    match p {
                        Udp::AuthenticateAck { nonce: n } => {
                            if phase == Phase::Authenticating {
                                nonce = n;
                                phase = Phase::Checking;
                                last_try = None;
                            }
                        }
                        Udp::ConnectionCheckAck => {
                            if phase == Phase::Checking {
                                phase = Phase::Connected;
                            }
                        }
                        Udp::KeepAlive => link.send(&Udp::KeepAlive),
                        Udp::Ping { .. } => link.send(&p),
                        Udp::PlayerSound {
                            channel,
                            sender,
                            data,
                            seq,
                            distance,
                            whispering,
                            ..
                        } => heard(
                            shared,
                            &mut speakers,
                            channel,
                            Packet {
                                seq,
                                data,
                                meta: Heard::Player {
                                    sender,
                                    distance,
                                    whispering,
                                },
                            },
                            now,
                        ),
                        Udp::GroupSound {
                            channel,
                            sender,
                            data,
                            seq,
                            ..
                        } => heard(
                            shared,
                            &mut speakers,
                            channel,
                            Packet {
                                seq,
                                data,
                                meta: Heard::Group { sender },
                            },
                            now,
                        ),
                        Udp::LocationSound {
                            channel,
                            sender,
                            pos,
                            data,
                            seq,
                            distance,
                            ..
                        } => heard(
                            shared,
                            &mut speakers,
                            channel,
                            Packet {
                                seq,
                                data,
                                meta: Heard::Location { sender, pos, distance },
                            },
                            now,
                        ),
                        Udp::Mic { .. } | Udp::Authenticate { .. } | Udp::ConnectionCheck { .. } => {}
                    }
                }
            }
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            // Nothing listens there (yet): an ICMP error, the next try says again.
            Err(_) => std::thread::sleep(Duration::from_millis(5)),
        }
        // The server fell silent (`keepAlive * 10`).
        if now - heard_at > keep_alive * 10 {
            phase = Phase::TimedOut;
            shared.set_status(|s| {
                s.phase = Phase::TimedOut;
                s.error = Some("The voice server stopped answering".into());
                s.talking = false;
            });
            break;
        }
        let controls = shared.controls();
        {
            let mut m = lock(&shared.mixer);
            m.volume = controls.volume;
            m.deafened = controls.deafened;
        }
        // The microphone: open while connected and not deafened.
        let want_mic = phase == Phase::Connected && !controls.deafened;
        if !want_mic && input.is_some() {
            input = None;
            shared.set_status(|s| s.input = None);
        }
        if want_mic && input.is_none() && input_tried.is_none_or(|t| now - t >= MIC_RETRY) {
            input_tried = Some(now);
            shared.mic.clear();
            match devices.input(shared.mic.clone()) {
                Ok(s) => {
                    shared.set_status(|st| st.input = Some(Ok(s.name.clone())));
                    input = Some(s);
                }
                Err(e) => shared.set_status(|st| st.input = Some(Err(e))),
            }
        }
        while let Some(mut frame) = shared.mic.frame() {
            let send = phase == Phase::Connected
                && !controls.deafened
                && processor.process(
                    &mut frame,
                    controls.activation,
                    controls.gain_db,
                    controls.threshold_db,
                    controls.ptt,
                    controls.muted,
                );
            if send {
                let e = enc.get_or_insert_with(|| encoder(secret.codec));
                if let Ok(n) = e.encode(&frame, FRAME_SIZE, &mut packet) {
                    link.send(&Udp::Mic {
                        data: packet[..n].to_vec(),
                        seq,
                        whispering: false,
                    });
                    seq += 1;
                    sending = true;
                }
            } else if sending {
                // The speech stopped: one empty packet, and the encoder starts over.
                link.send(&Udp::Mic {
                    data: Vec::new(),
                    seq,
                    whispering: false,
                });
                seq += 1;
                sending = false;
                enc = None;
            }
        }
        // Deafened mid-speech: the microphone closed, so no frame ends it.
        if sending && controls.deafened {
            link.send(&Udp::Mic {
                data: Vec::new(),
                seq,
                whispering: false,
            });
            seq += 1;
            sending = false;
            enc = None;
        }
        speakers.retain(|_, (_, at)| now - *at < SPEAKER_IDLE);
        shared.set_status(|s| {
            s.phase = phase;
            s.talking = sending;
            s.level_db = if input.is_some() { processor.level_db } else { LOWEST_DB };
            s.speaking.retain(|_, (at, _)| now - *at < TALKING_FOR);
        });
    }
    // Stopping mid-speech: the listeners' channels end too.
    if sending && phase == Phase::Connected {
        link.send(&Udp::Mic {
            data: Vec::new(),
            seq,
            whispering: false,
        });
    }
    drop(input);
    drop(output);
    lock(&shared.mixer).clear();
    shared.set_status(|s| {
        s.talking = false;
        s.speaking.clear();
    });
}

/// A speaker's packet: in order, decoded, placed, into the mixer.
fn heard(
    shared: &Shared,
    speakers: &mut HashMap<u128, (Speaker<Opus, Heard>, Instant)>,
    channel: u128,
    packet: Packet<Heard>,
    now: Instant,
) {
    let (speaker, at) = speakers
        .entry(channel)
        .or_insert_with(|| (Speaker::new(Opus::new()), now));
    *at = now;
    let played = speaker.push(packet);
    let lost = speaker.lost;
    if played.is_empty() {
        return;
    }
    let world = lock(&shared.world).clone();
    let (muted_players, muted_voices) = {
        let c = lock(&shared.controls);
        (c.muted_players.clone(), c.muted_voices)
    };
    let mut mixer = lock(&shared.mixer);
    for p in played {
        match p {
            Played::End(meta) => {
                mixer.end(channel);
                shared.set_status(|s| {
                    s.speaking.remove(&meta.sender());
                });
            }
            Played::Frames(frames, meta) => {
                // A voice this player muted is dropped, as SVC drops one at volume 0.
                let sender = meta.sender();
                if sender != world.own && (muted_voices || muted_players.contains(&sender)) {
                    continue;
                }
                let gains = match meta {
                    Heard::Player { sender, .. } if sender == world.own => Some(spatial::centre()),
                    Heard::Player { sender, distance, .. } => world
                        .eyes
                        .get(&sender)
                        .and_then(|&eyes| spatial::positional(&world.listener, eyes, distance)),
                    Heard::Group { .. } => Some(spatial::centre()),
                    Heard::Location { pos, distance, .. } => spatial::positional(&world.listener, pos, distance),
                };
                // Not in the world here, or too far: SVC drops the frame.
                let Some(gains) = gains else { continue };
                for f in &frames {
                    mixer.push(channel, f, gains);
                }
                let whispering = matches!(meta, Heard::Player { whispering: true, .. });
                let audible = gains[0] + gains[1] > 0.0;
                shared.set_status(|s| {
                    s.played += frames.len() as u64;
                    s.lost = s.lost.max(lost);
                    if audible {
                        s.speaking.insert(meta.sender(), (now, whispering));
                    }
                });
            }
        }
    }
}
