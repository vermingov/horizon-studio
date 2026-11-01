// The voice chat end to end against a fake Simple Voice Chat server, in
// both compatibility versions: the payloads on the Minecraft connection go
// through the fake game (the studio's voice chat, hstudio_voicechat, as the
// game drives it), the voice packets through a real UDP socket on
// localhost. The client authenticates, checks its connection, answers
// keep-alives and pings, sends its microphone as Opus (which the server
// decrypts and decodes) with the empty packet at the end, and plays the
// voices the server sends into its mixer, at the speaker's place; the
// scripts and the HUD see it.

mod common;

use common::{FakeGame, studio};
use horizon_studio::game::hstudio::Studio;
use horizon_studio::game::hstudio_voicechat::{Glue, PTT_KEY, draw_hud};
use hstudio_voice::audio::NoDevices;
use hstudio_voice::crypt::{self, Cipher};
use hstudio_voice::jitter::{FrameDecoder, Opus};
use hstudio_voice::net::encoder;
use hstudio_voice::proto::{self, COMPAT_20, COMPAT_21, Codec, FRAME_SIZE, Group, PlayerState, Secret, Udp, Writer};
use hstudio_voice::{Listener, World};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CONNECTION: u128 = 0x0f1e2d3c_4b5a_6978_8796_a5b4c3d2e1f0;
const PLAYER: u128 = 0x12345678_9abc_def0_1234_56789abcdef0;
const ALEX: u128 = 0xcafebabe_0000_4000_8000_00000000beef;
const GROUP: u128 = 0xaa;
const NONCE: [u8; 16] = [7; 16];

/// What the fake server saw.
#[derive(Debug)]
enum Seen {
    Authenticate(Option<(u128, [u8; 16])>),
    Check(Option<[u8; 16]>),
    KeepAlive,
    Ping(u128),
    Mic(Vec<u8>, i64),
    /// A datagram with the wrong id or that didn't open.
    Bad,
}

/// The voice server's half: answers authentication and the check as SVC's
/// `Server` does, and reports everything else.
struct FakeServer {
    socket: Arc<UdpSocket>,
    cipher: Arc<Cipher>,
    client: Arc<Mutex<Option<SocketAddr>>>,
    seen: mpsc::Receiver<Seen>,
}

impl FakeServer {
    fn start(compat: i32, key: [u8; 16]) -> FakeServer {
        let socket = Arc::new(UdpSocket::bind("127.0.0.1:0").unwrap());
        socket.set_read_timeout(Some(Duration::from_millis(20))).unwrap();
        let cipher = Arc::new(Cipher::new(&key));
        let client = Arc::new(Mutex::new(None));
        let (tx, seen) = mpsc::channel();
        let (s, c, cl) = (socket.clone(), cipher.clone(), client.clone());
        let id = if compat == COMPAT_21 { CONNECTION } else { PLAYER };
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                let Ok((n, from)) = s.recv_from(&mut buf) else {
                    if tx.send(Seen::Bad).is_err() {
                        return;
                    }
                    continue;
                };
                let packet = crypt::from_client(&buf[..n])
                    .filter(|(got, _)| *got == id)
                    .and_then(|(_, sealed)| c.decrypt(&sealed))
                    .and_then(|p| Udp::decode(&p).ok());
                let reply = |p: Udp| {
                    let _ = s.send_to(&crypt::to_client(&c.encrypt(&p.encode())), from);
                };
                let seen = match packet {
                    Some(Udp::Authenticate { player }) => {
                        *cl.lock().unwrap() = Some(from);
                        reply(Udp::AuthenticateAck {
                            nonce: (compat == COMPAT_21).then_some(NONCE),
                        });
                        Seen::Authenticate(player)
                    }
                    Some(Udp::ConnectionCheck { nonce }) => {
                        if compat == COMPAT_20 || nonce == Some(NONCE) {
                            reply(Udp::ConnectionCheckAck);
                        }
                        Seen::Check(nonce)
                    }
                    Some(Udp::KeepAlive) => Seen::KeepAlive,
                    Some(Udp::Ping { id, .. }) => Seen::Ping(id),
                    Some(Udp::Mic { data, seq, .. }) => Seen::Mic(data, seq),
                    _ => Seen::Bad,
                };
                if tx.send(seen).is_err() {
                    return;
                }
            }
        });
        FakeServer {
            socket,
            cipher,
            client,
            seen,
        }
    }
    fn port(&self) -> u16 {
        self.socket.local_addr().unwrap().port()
    }
    fn send(&self, p: &Udp) {
        let to = self.client.lock().unwrap().expect("the client has authenticated");
        self.socket
            .send_to(&crypt::to_client(&self.cipher.encrypt(&p.encode())), to)
            .unwrap();
    }
    /// What it saw next (Bad ones skipped), within `secs`.
    fn next(&self, secs: f32, want: impl Fn(&Seen) -> bool) -> Seen {
        let end = Instant::now() + Duration::from_secs_f32(secs);
        while Instant::now() < end {
            if let Ok(s) = self.seen.recv_timeout(Duration::from_millis(50))
                && want(&s)
            {
                return s;
            }
        }
        panic!("the fake server didn't see it in {secs} s");
    }
}

/// A voice: a 150 Hz buzz with harmonics.
fn voice(frames: usize) -> Vec<f32> {
    (0..frames * FRAME_SIZE)
        .map(|i| {
            let t = i as f32 / 48_000.0;
            0.3 * (1..10)
                .map(|h| (2.0 * std::f32::consts::PI * 150.0 * h as f32 * t).sin() / h as f32)
                .sum::<f32>()
        })
        .collect()
}

fn rms(v: &[f32]) -> f32 {
    (v.iter().map(|s| s * s).sum::<f32>() / v.len().max(1) as f32).sqrt()
}

fn glue(game: &mut FakeGame) -> &mut Glue {
    game.voice_glue.as_mut().unwrap()
}

fn world() -> World {
    World {
        listener: Listener {
            pos: [0.0, 65.62, 0.0],
            yaw: 0.0,
            pitch: 0.0,
        },
        own: PLAYER,
        // Alex is 3 blocks east: on the left of a player facing south.
        eyes: HashMap::from([(ALEX, [3.0, 65.62, 0.0])]),
    }
}

fn frame(game: &mut FakeGame) {
    glue(game).frame(world(), true, None, Instant::now());
}

fn console(st: &mut Studio, game: &mut FakeGame, src: &str) -> Vec<String> {
    st.run_console(game, src).into_iter().map(|(_, t)| t).collect()
}

/// Frames until `ok`, within `secs`.
fn until(game: &mut FakeGame, secs: f32, what: &str, ok: impl Fn(&mut FakeGame) -> bool) {
    let end = Instant::now() + Duration::from_secs_f32(secs);
    while Instant::now() < end {
        frame(game);
        if ok(game) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "not {what} in {secs} s: {:?}",
        game.voice_glue.as_ref().unwrap().info(Instant::now())
    );
}

fn payloads(game: &mut FakeGame) -> Vec<(String, Vec<u8>)> {
    glue(game).take_payloads()
}

fn talk_and_listen(compat: i32) {
    let key = [0x5a; 16];
    let server = FakeServer::start(compat, key);
    let mut game = FakeGame::new(&format!("voice-{compat}"));
    let dir = game.dir.clone();
    game.voice_glue = Some(Glue::new(Arc::new(NoDevices), Some(dir.clone())));
    let mut st = studio(&mut game);
    assert_eq!(console(&mut st, &mut game, "voice.status().state"), vec!["\"off\""]);

    // The join: the channels, then the secret asked for in compat 21.
    glue(&mut game).joined("127.0.0.1", Arc::new(|| "127.0.0.1".into()), Instant::now());
    let sent = payloads(&mut game);
    assert_eq!(sent[0].0, "minecraft:register");
    assert!(String::from_utf8_lossy(&sent[0].1).contains("voicechat:secret\0voicechat:states"));
    assert_eq!(
        sent[1],
        ("voicechat:request_secret".into(), proto::request_secret(COMPAT_21))
    );
    if compat == COMPAT_20 {
        // A 2.6 server answers 21 with its chat line; the client asks again in 20.
        glue(&mut game).chat_line("message.voicechat.incompatible_version", Instant::now());
        assert_eq!(
            payloads(&mut game),
            vec![("voicechat:request_secret".into(), proto::request_secret(COMPAT_20))]
        );
    }

    // The server's answer: the states, a group, then the secret.
    let mut states = Writer::new();
    states.i32(2);
    let steve = PlayerState {
        uuid: PLAYER,
        name: "Steve".into(),
        disconnected: true,
        ..PlayerState::default()
    };
    let alex = PlayerState {
        uuid: ALEX,
        name: "Alex".into(),
        ..PlayerState::default()
    };
    steve.write(&mut states, compat);
    alex.write(&mut states, compat);
    let mut group = Writer::new();
    Group {
        id: GROUP,
        name: "Friends".into(),
        has_password: false,
        persistent: true,
        hidden: false,
        kind: 0,
    }
    .write(&mut group);
    let secret = Secret {
        secret: key,
        connection_id: (compat == COMPAT_21).then_some(CONNECTION),
        server_port: i32::from(server.port()),
        player: PLAYER,
        codec: Codec::Voip,
        mtu: 1275,
        distance: 48.0,
        keep_alive: 1000,
        groups: true,
        voice_host: String::new(),
        allow_recording: false,
    };
    let now = Instant::now();
    assert!(glue(&mut game).payload(proto::STATES, &states.finish(), now));
    assert!(glue(&mut game).payload(proto::ADD_GROUP, &group.finish(), now));
    assert!(glue(&mut game).payload(proto::ADD_CATEGORY, b"anything", now));
    assert!(glue(&mut game).payload(proto::SECRET, &secret.encode(compat), now));
    assert!(!glue(&mut game).payload("minecraft:brand", b"x", now));
    // Its own state, as the version has it.
    let (channel, data) = proto::client_state(compat, false, false);
    assert_eq!(payloads(&mut game), vec![(channel.to_owned(), data)]);

    // UDP: authentication (compat 20 with the player and the secret), the check.
    let auth = server.next(5.0, |s| matches!(s, Seen::Authenticate(_)));
    match compat {
        COMPAT_21 => assert!(matches!(auth, Seen::Authenticate(None))),
        _ => assert!(matches!(auth, Seen::Authenticate(Some((PLAYER, k))) if k == key)),
    }
    let check = server.next(5.0, |s| matches!(s, Seen::Check(_)));
    match compat {
        COMPAT_21 => assert!(matches!(check, Seen::Check(Some(NONCE)))),
        _ => assert!(matches!(check, Seen::Check(None))),
    }
    until(&mut game, 5.0, "connected", |g| {
        g.voice_glue.as_ref().unwrap().info(Instant::now()).connected
    });
    assert_eq!(console(&mut st, &mut game, "voice.connected()"), vec!["true"]);
    assert_eq!(
        console(&mut st, &mut game, "voice.status().compat"),
        vec![compat.to_string()]
    );
    assert_eq!(
        console(&mut st, &mut game, "voice.groups()[1].name"),
        vec!["\"Friends\""]
    );

    // Keep-alives are answered, pings echoed.
    server.send(&Udp::KeepAlive);
    server.next(3.0, |s| matches!(s, Seen::KeepAlive));
    server.send(&Udp::Ping { id: 99, timestamp: 1 });
    assert!(matches!(
        server.next(3.0, |s| matches!(s, Seen::Ping(_))),
        Seen::Ping(99)
    ));

    // The microphone: push to talk held for 10 frames, Opus the server decodes.
    let input = voice(20);
    assert!(glue(&mut game).key(PTT_KEY, true));
    frame(&mut game);
    glue(&mut game).vc.shared().mic.push(&input[..10 * FRAME_SIZE]);
    let mut decoder = Opus::new();
    let mut heard = Vec::new();
    let mut seqs = Vec::new();
    for _ in 0..10 {
        let Seen::Mic(data, seq) = server.next(5.0, |s| matches!(s, Seen::Mic(..))) else {
            unreachable!()
        };
        assert!(!data.is_empty());
        heard.extend(decoder.decode(&data));
        seqs.push(seq);
    }
    assert_eq!(seqs, (0..10).collect::<Vec<i64>>());
    assert!(
        rms(&heard) > 0.5 * rms(&input[..10 * FRAME_SIZE]),
        "the server decodes a voice"
    );
    until(&mut game, 2.0, "talking", |g| {
        g.voice_glue.as_ref().unwrap().info(Instant::now()).talking
    });
    assert_eq!(console(&mut st, &mut game, "voice.talking()"), vec!["true"]);
    // Let go: 5 frames more (SVC's delay), then the empty packet.
    assert!(glue(&mut game).key(PTT_KEY, false));
    frame(&mut game);
    std::thread::sleep(Duration::from_millis(50));
    glue(&mut game).vc.shared().mic.push(&input[10 * FRAME_SIZE..]);
    let mut tail = Vec::new();
    loop {
        let Seen::Mic(data, seq) = server.next(5.0, |s| matches!(s, Seen::Mic(..))) else {
            unreachable!()
        };
        tail.push(data.len());
        if data.is_empty() {
            assert_eq!(seq, 15);
            break;
        }
    }
    assert_eq!(tail.len(), 6, "{tail:?}");

    // Muted: nothing more goes, and the server is told (compat 21 has the flag).
    console(&mut st, &mut game, "voice.mute(true)");
    let (channel, data) = proto::client_state(compat, true, false);
    assert_eq!(payloads(&mut game), vec![(channel.to_owned(), data)]);
    assert!(glue(&mut game).key(PTT_KEY, true));
    frame(&mut game);
    glue(&mut game).vc.shared().mic.push(&input[..5 * FRAME_SIZE]);
    std::thread::sleep(Duration::from_millis(200));
    assert!(server.seen.try_iter().all(|s| !matches!(s, Seen::Mic(..))));
    glue(&mut game).key(PTT_KEY, false);
    console(&mut st, &mut game, "voice.mute(false)");
    payloads(&mut game);

    // Alex talks: his voice reaches the mixer, from the left, and he is listed.
    let mut enc = encoder(Codec::Voip);
    let mut buf = [0u8; 1275];
    for (seq, f) in voice(12).as_chunks::<FRAME_SIZE>().0.iter().enumerate() {
        let n = enc.encode(f, FRAME_SIZE, &mut buf).unwrap();
        server.send(&Udp::PlayerSound {
            channel: ALEX,
            sender: ALEX,
            data: buf[..n].to_vec(),
            seq: seq as i64,
            distance: 48.0,
            whispering: false,
            category: Some("proximity".into()),
        });
    }
    until(&mut game, 3.0, "hearing Alex", |g| {
        g.voice_glue.as_ref().unwrap().vc.shared().status().played >= 12
    });
    assert_eq!(
        console(&mut st, &mut game, "voice.speakers()[1].name"),
        vec!["\"Alex\""]
    );
    game.drawn.clear();
    draw_hud(&mut game);
    assert!(
        game.drawn.iter().any(|d| d.starts_with("text Alex")),
        "{:?}",
        game.drawn
    );
    let mut out = vec![0f32; 2 * FRAME_SIZE * 12];
    glue(&mut game).vc.shared().mixer.lock().unwrap().render(&mut out);
    let left: Vec<f32> = out.iter().step_by(2).copied().collect();
    let right: Vec<f32> = out.iter().skip(1).step_by(2).copied().collect();
    assert!(rms(&left) > 0.01, "Alex is heard: {}", rms(&left));
    assert!(
        rms(&left) > 3.0 * rms(&right),
        "from the left: {} {}",
        rms(&left),
        rms(&right)
    );
    // His end packet ends his channel.
    server.send(&Udp::PlayerSound {
        channel: ALEX,
        sender: ALEX,
        data: Vec::new(),
        seq: 12,
        distance: 48.0,
        whispering: false,
        category: None,
    });
    until(&mut game, 3.0, "Alex stopping", |g| {
        g.voice_glue.as_ref().unwrap().info(Instant::now()).speakers.is_empty()
    });
    // Someone farther than the distance isn't heard; a group's voice is, without a place.
    let n = enc.encode(&voice(1), FRAME_SIZE, &mut buf).unwrap();
    let before = glue(&mut game).vc.shared().status().played;
    server.send(&Udp::LocationSound {
        channel: 5,
        sender: 5,
        pos: [100.0, 64.0, 0.0],
        data: buf[..n].to_vec(),
        seq: 0,
        distance: 16.0,
        category: None,
    });
    server.send(&Udp::GroupSound {
        channel: 6,
        sender: ALEX,
        data: buf[..n].to_vec(),
        seq: 0,
        category: None,
    });
    until(&mut game, 3.0, "the group's voice", |g| {
        g.voice_glue.as_ref().unwrap().vc.shared().status().played > before
    });
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(glue(&mut game).vc.shared().status().played, before + 1);

    // Leaving stops it all.
    glue(&mut game).left();
    assert_eq!(console(&mut st, &mut game, "voice.status().state"), vec!["\"off\""]);
    // The version that worked is remembered for this server.
    let saved = std::fs::read_to_string(dir.join("voice.txt")).unwrap();
    assert!(saved.contains(&format!("server:127.0.0.1={compat}")), "{saved}");
}

#[test]
fn talks_and_listens_on_a_compat_21_server() {
    talk_and_listen(COMPAT_21);
}

#[test]
fn talks_and_listens_on_a_compat_20_server() {
    talk_and_listen(COMPAT_20);
}

#[test]
fn a_server_without_voice_chat_is_asked_twice_then_left_alone() {
    let mut game = FakeGame::new("voice-none");
    game.voice_glue = Some(Glue::new(Arc::new(NoDevices), None));
    let t0 = Instant::now();
    glue(&mut game).joined("mc.example.org", Arc::new(|| "mc.example.org".into()), t0);
    assert_eq!(payloads(&mut game).len(), 2);
    glue(&mut game).frame(world(), true, None, t0 + Duration::from_secs(1));
    assert!(payloads(&mut game).is_empty());
    // No answer in four seconds: compat 20.
    glue(&mut game).frame(world(), true, None, t0 + Duration::from_secs(5));
    assert_eq!(
        payloads(&mut game),
        vec![("voicechat:request_secret".into(), proto::request_secret(COMPAT_20))]
    );
    glue(&mut game).frame(world(), true, None, t0 + Duration::from_secs(10));
    assert!(payloads(&mut game).is_empty());
    let info = glue(&mut game).info(Instant::now());
    assert_eq!(info.state, "none");
    assert!(!info.unavailable);
    // Nothing on the HUD; the keys do nothing.
    game.drawn.clear();
    draw_hud(&mut game);
    assert!(game.drawn.is_empty());
    assert!(glue(&mut game).key("key.keyboard.m", true));
    assert!(!glue(&mut game).info(Instant::now()).muted);
    // Group packets and broken ones never crash it.
    for (channel, data) in [
        (proto::ADD_GROUP, vec![1, 2, 3]),
        (proto::REMOVE_GROUP, vec![]),
        (proto::JOINED_GROUP, vec![1]),
        (proto::SECRET, vec![0; 7]),
        ("voicechat:something_new", vec![9; 40]),
    ] {
        assert!(glue(&mut game).payload(channel, &data, Instant::now()));
    }
    // A remembered server is asked in its own version first; another in 21.
    let dir = game.dir.join("remembered");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("voice.txt"), "volume=2\nserver:mc.example.org=20\n").unwrap();
    let mut g = Glue::new(Arc::new(NoDevices), Some(dir));
    assert_eq!(g.info(Instant::now()).volume, 2.0);
    g.joined("mc.example.org", Arc::new(|| "x".into()), Instant::now());
    assert_eq!(g.take_payloads()[1].1, proto::request_secret(COMPAT_20));
    g.joined("other.example.org", Arc::new(|| "x".into()), Instant::now());
    assert_eq!(g.take_payloads()[1].1, proto::request_secret(COMPAT_21));
}

#[test]
fn a_secret_for_an_unreachable_server_says_voice_chat_is_unavailable() {
    let mut game = FakeGame::new("voice-unreachable");
    game.voice_glue = Some(Glue::new(Arc::new(NoDevices), None));
    let mut st = studio(&mut game);
    let t0 = Instant::now();
    glue(&mut game).joined("nowhere.invalid", Arc::new(|| "nowhere.invalid".into()), t0);
    let secret = Secret {
        secret: [1; 16],
        connection_id: Some(CONNECTION),
        server_port: 24454,
        player: PLAYER,
        codec: Codec::Voip,
        mtu: 1275,
        distance: 48.0,
        keep_alive: 1000,
        groups: false,
        voice_host: String::new(),
        allow_recording: false,
    };
    glue(&mut game).payload(proto::SECRET, &secret.encode(COMPAT_21), t0);
    until(&mut game, 10.0, "failed", |g| {
        g.voice_glue.as_ref().unwrap().info(Instant::now()).state == "failed"
    });
    let info = glue(&mut game).info(Instant::now());
    assert!(info.unavailable && info.error.is_some(), "{info:?}");
    game.drawn.clear();
    draw_hud(&mut game);
    assert!(
        game.drawn.iter().any(|d| d.contains("Voice chat unavailable")),
        "{:?}",
        game.drawn
    );
    let out = console(
        &mut st,
        &mut game,
        "voice.join_group('00000000-0000-0000-0000-0000000000aa')",
    );
    assert!(
        out[0].contains("no such group") || out[0].contains("no groups"),
        "{out:?}"
    );
    // The settings: deafen and the volume, from Lua and kept.
    assert_eq!(console(&mut st, &mut game, "voice.deafen(true)"), vec!["true"]);
    assert_eq!(console(&mut st, &mut game, "voice.volume(5)"), vec!["3"]);
    assert_eq!(
        console(&mut st, &mut game, "voice.activation('voice')"),
        vec!["\"voice\""]
    );
    let out = console(&mut st, &mut game, "voice.activation('loud')");
    assert!(out[0].contains("\"ptt\" or \"voice\""), "{out:?}");
    game.drawn.clear();
    draw_hud(&mut game);
    assert!(game.drawn.iter().any(|d| d.contains("Voice chat unavailable")));
}
