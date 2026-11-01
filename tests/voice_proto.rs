// The voice chat's wire formats against Simple Voice Chat's: the payloads on
// the Minecraft connection (compat 20 and 21) and the voice packets, their
// AES-128-GCM and framing. The vectors were written by a Java program that
// encodes as SVC does (FriendlyByteBuf's writes, SecretPacket.toBytes,
// PlayerState.toBytes, MicPacket and the sound packets, Secret.encrypt with
// AES/GCM/NoPadding) with a fixed IV instead of a random one; it is
// tools/svc_vectors/Vectors.java.

use hstudio_voice::crypt::{self, Cipher};
use hstudio_voice::proto::{
    self, COMPAT_20, COMPAT_21, Clientbound, Codec, Group, PlayerState, Reader, Secret, Udp, Writer,
    decode_clientbound, voice_target,
};

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

const KEY: [u8; 16] = [
    0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab, 0xac, 0xad, 0xae, 0xaf,
];
const IV: [u8; 12] = [0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b];
const CONNECTION: u128 = 0x0f1e2d3c_4b5a_6978_8796_a5b4c3d2e1f0;
const PLAYER: u128 = 0x12345678_9abc_def0_1234_56789abcdef0;
const OTHER: u128 = 0xcafebabe_0000_4000_8000_00000000beef;
const GROUP: u128 = 0xaa;
const OPUS: [u8; 5] = [0xf8, 0xff, 0xfe, 0x01, 0x02];

const MIC_PLAIN: &str = "0105f8fffe0102000000000000000700";
const MIC_21: &str = "ff0f1e2d3c4b5a69788796a5b4c3d2e1f02c101112131415161718191a1bb6e75395d834aee75e765cb0f5a4edf48de9e93ae9a9da89c456c4b356926b49";
const MIC_20: &str = "ff123456789abcdef0123456789abcdef02c101112131415161718191a1bb6e75395d834aee75e765cb0f5a4edf48de9e93ae9a9da89c456c4b356926b49";
const SOUND_PLAIN: &str = "02cafebabe00004000800000000000beefcafebabe00004000800000000000beef05f8fffe0102000000000000002a41c00000030970726f78696d697479";
const SOUND_DATAGRAM: &str = "ff5a101112131415161718191a1bb52855d09835aca75ef65cb0f5a4ea4a9c0d8e05b3412e5f534bca05110edff95fa3deca3516551287fb0d068d4ec12fcab98d6020ceed3188798c5068d587993471e2340de829906b0a1c999a19";
const LOCATION_PLAIN: &str = "04cafebabe00004000800000000000beef123456789abcdef0123456789abcdef03ff8000000000000c050000000000000408f40000000000005f8fffe010200000000000000034180000000";
const GROUP_PLAIN: &str =
    "03cafebabe00004000800000000000beef123456789abcdef0123456789abcdef005f8fffe0102000000000000000900";
const AUTH20_PLAIN: &str = "05123456789abcdef0123456789abcdef0a0a1a2a3a4a5a6a7a8a9aaabacadaeaf";
const CHECK21_PLAIN: &str = "09606162636465666768696a6b6c6d6e6f";
const PING_PLAIN: &str = "07cafebabe00004000800000000000beef00000192cc091400";
const SECRET21: &str = "a0a1a2a3a4a5a6a7a8a9aaabacadaeaf0f1e2d3c4b5a69788796a5b4c3d2e1f000005f86123456789abcdef0123456789abcdef000000004fb4048000000000000000003e8010000";
const SECRET20: &str = "a0a1a2a3a4a5a6a7a8a9aaabacadaeaf00005f86123456789abcdef0123456789abcdef000000004fb4048000000000000000003e80117766f6963652e6578616d706c652e6f72673a323434353501";
const STATES21: &str = "0000000211123456789abcdef0123456789abcdef005537465766524cafebabe00004000800000000000beef04416c6578000000000000000000000000000000aa";
const STATES20: &str = "000000020001123456789abcdef0123456789abcdef0055374657665000100cafebabe00004000800000000000beef04416c657801000000000000000000000000000000aa";
const GROUP_PAYLOAD: &str = "000000000000000000000000000000aa07467269656e64730100000002";

fn secret21() -> Secret {
    Secret {
        secret: KEY,
        connection_id: Some(CONNECTION),
        server_port: 24454,
        player: PLAYER,
        codec: Codec::Voip,
        mtu: 1275,
        distance: 48.0,
        keep_alive: 1000,
        groups: true,
        voice_host: String::new(),
        allow_recording: false,
    }
}

#[test]
fn secrets_read_and_write_as_svc_writes_them_in_both_versions() {
    let s21 = secret21();
    assert_eq!(Secret::decode(&hex(SECRET21), COMPAT_21).unwrap(), s21);
    assert_eq!(s21.encode(COMPAT_21), hex(SECRET21));
    let s20 = Secret {
        connection_id: None,
        voice_host: "voice.example.org:24455".into(),
        allow_recording: true,
        ..s21.clone()
    };
    assert_eq!(Secret::decode(&hex(SECRET20), COMPAT_20).unwrap(), s20);
    assert_eq!(s20.encode(COMPAT_20), hex(SECRET20));
    // The wrong layout doesn't read exactly; decode_any finds the right one.
    assert!(Secret::decode(&hex(SECRET20), COMPAT_21).is_err());
    assert!(Secret::decode(&hex(SECRET21), COMPAT_20).is_err());
    assert_eq!(Secret::decode_any(&hex(SECRET20), COMPAT_21).unwrap(), (s20, COMPAT_20));
    assert_eq!(Secret::decode_any(&hex(SECRET21), COMPAT_20).unwrap(), (s21, COMPAT_21));
}

#[test]
fn player_states_read_in_both_versions() {
    let steve = |muted, disconnected| PlayerState {
        uuid: PLAYER,
        name: "Steve".into(),
        muted,
        disconnected,
        ..PlayerState::default()
    };
    let alex = PlayerState {
        uuid: OTHER,
        name: "Alex".into(),
        disabled: true,
        group: Some(GROUP),
        ..PlayerState::default()
    };
    let Some(Ok(Clientbound::States(v))) = decode_clientbound(proto::STATES, &hex(STATES21), COMPAT_21) else {
        panic!("compat 21 states")
    };
    assert_eq!(v, vec![steve(true, true), alex.clone()]);
    let Some(Ok(Clientbound::States(v))) = decode_clientbound(proto::STATES, &hex(STATES20), COMPAT_20) else {
        panic!("compat 20 states")
    };
    // Compat 20 has no mute flags.
    assert_eq!(v, vec![steve(false, true), alex.clone()]);
    // And back.
    for (compat, bytes) in [(COMPAT_21, STATES21), (COMPAT_20, STATES20)] {
        let mut w = Writer::new();
        w.i32(2);
        steve(compat == COMPAT_21, true).write(&mut w, compat);
        alex.write(&mut w, compat);
        assert_eq!(w.finish(), hex(bytes), "compat {compat}");
    }
}

#[test]
fn the_other_payloads_read_and_write() {
    let g = Group {
        id: GROUP,
        name: "Friends".into(),
        has_password: true,
        persistent: false,
        hidden: false,
        kind: 2,
    };
    assert_eq!(
        decode_clientbound(proto::ADD_GROUP, &hex(GROUP_PAYLOAD), COMPAT_21),
        Some(Ok(Clientbound::AddGroup(g.clone())))
    );
    let mut w = Writer::new();
    g.write(&mut w);
    assert_eq!(w.finish(), hex(GROUP_PAYLOAD));
    assert_eq!(proto::request_secret(21), hex("00000015"));
    assert_eq!(
        proto::client_state(COMPAT_21, true, false),
        (proto::CLIENT_STATE, hex("0100"))
    );
    assert_eq!(
        proto::client_state(COMPAT_20, true, false),
        (proto::UPDATE_STATE, hex("00"))
    );
    let mut joined = Writer::new();
    joined.bool(true).uuid(GROUP).bool(false);
    assert_eq!(
        decode_clientbound(proto::JOINED_GROUP, &joined.finish(), COMPAT_21),
        Some(Ok(Clientbound::JoinedGroup {
            group: Some(GROUP),
            wrong_password: false
        }))
    );
    assert_eq!(
        decode_clientbound(proto::REMOVE_STATE, &OTHER.to_be_bytes(), COMPAT_20),
        Some(Ok(Clientbound::RemoveState(OTHER)))
    );
    // Categories are taken and ignored; another mod's channel isn't voice chat's.
    assert_eq!(
        decode_clientbound(proto::ADD_CATEGORY, b"\x01\x02", COMPAT_21),
        Some(Ok(Clientbound::Category))
    );
    assert_eq!(decode_clientbound("minecraft:brand", b"x", COMPAT_21), None);
    // A broken voice chat payload is an error, not a panic; so is an unknown one.
    assert!(matches!(
        decode_clientbound(proto::STATES, &hex("00000005ff"), COMPAT_21),
        Some(Err(_))
    ));
    assert!(matches!(
        decode_clientbound(proto::STATES, &hex("7fffffff"), COMPAT_21),
        Some(Err(_))
    ));
    assert!(matches!(
        decode_clientbound("voicechat:future", b"", COMPAT_21),
        Some(Err(_))
    ));
    // minecraft:register names every channel the server sends on, NUL between them.
    let reg = String::from_utf8(proto::register()).unwrap();
    let names: Vec<&str> = reg.split('\0').collect();
    assert_eq!(names, proto::CLIENTBOUND.to_vec());
    assert!(names.contains(&"voicechat:secret"));
}

#[test]
fn voice_packets_read_and_write_as_svc_writes_them() {
    let mic = Udp::Mic {
        data: OPUS.to_vec(),
        seq: 7,
        whispering: false,
    };
    let sound = Udp::PlayerSound {
        channel: OTHER,
        sender: OTHER,
        data: OPUS.to_vec(),
        seq: 42,
        distance: 24.0,
        whispering: true,
        category: Some("proximity".into()),
    };
    let location = Udp::LocationSound {
        channel: OTHER,
        sender: PLAYER,
        pos: [1.5, -64.0, 1000.0],
        data: OPUS.to_vec(),
        seq: 3,
        distance: 16.0,
        category: None,
    };
    let group = Udp::GroupSound {
        channel: OTHER,
        sender: PLAYER,
        data: OPUS.to_vec(),
        seq: 9,
        category: None,
    };
    let auth20 = Udp::Authenticate {
        player: Some((PLAYER, KEY)),
    };
    let check21 = Udp::ConnectionCheck {
        nonce: Some(core::array::from_fn(|i| 0x60 + i as u8)),
    };
    let ping = Udp::Ping {
        id: OTHER,
        timestamp: 1_730_000_000_000,
    };
    for (p, bytes) in [
        (&mic, MIC_PLAIN),
        (&sound, SOUND_PLAIN),
        (&location, LOCATION_PLAIN),
        (&group, GROUP_PLAIN),
        (&auth20, AUTH20_PLAIN),
        (&check21, CHECK21_PLAIN),
        (&ping, PING_PLAIN),
    ] {
        assert_eq!(p.encode(), hex(bytes), "{p:?}");
        assert_eq!(&Udp::decode(&hex(bytes)).unwrap(), p);
    }
    // Compat 21's authentication is empty, compat 20's acknowledgement too.
    assert_eq!(Udp::Authenticate { player: None }.encode(), vec![0x05]);
    assert_eq!(Udp::decode(&[0x06]).unwrap(), Udp::AuthenticateAck { nonce: None });
    let mut ack = vec![0x06];
    ack.extend(0..16u8);
    assert_eq!(
        Udp::decode(&ack).unwrap(),
        Udp::AuthenticateAck {
            nonce: Some(core::array::from_fn(|i| i as u8))
        }
    );
    assert_eq!(Udp::decode(&[0x08]).unwrap(), Udp::KeepAlive);
    assert_eq!(Udp::decode(&[0x0A]).unwrap(), Udp::ConnectionCheckAck);
    // Broken packets are errors.
    assert!(Udp::decode(&[0x0B]).is_err());
    assert!(Udp::decode(&[0x02, 0x00]).is_err());
    assert!(Udp::decode(&[]).is_err());
    // An Opus payload larger than SVC reads is refused.
    let mut big = Writer::new();
    big.u8(0x01).byte_array(&[0u8; 1276]).i64(0).bool(false);
    assert!(Udp::decode(&big.finish()).is_err());
}

#[test]
fn datagrams_are_sealed_and_framed_as_svc_seals_them() {
    let cipher = Cipher::new(&KEY);
    // Compat 21 sends the connection id, compat 20 the player's UUID, before the sealed packet.
    let sealed = cipher.encrypt_with_iv(IV, &hex(MIC_PLAIN));
    assert_eq!(crypt::to_server(CONNECTION, &sealed), hex(MIC_21));
    assert_eq!(crypt::to_server(PLAYER, &sealed), hex(MIC_20));
    let (id, payload) = crypt::from_client(&hex(MIC_21)).unwrap();
    assert_eq!(id, CONNECTION);
    assert_eq!(cipher.decrypt(&payload).unwrap(), hex(MIC_PLAIN));
    // The server's datagrams carry no id.
    let sound = crypt::from_server(&hex(SOUND_DATAGRAM)).unwrap();
    assert_eq!(cipher.decrypt(&sound).unwrap(), hex(SOUND_PLAIN));
    assert_eq!(
        crypt::to_client(&cipher.encrypt_with_iv(IV, &hex(SOUND_PLAIN))),
        hex(SOUND_DATAGRAM)
    );
    // A random IV each time, and what another key sealed (or a changed byte) doesn't open.
    let a = cipher.encrypt(b"\x08");
    let b = cipher.encrypt(b"\x08");
    assert_ne!(a[..12], b[..12]);
    assert_eq!(cipher.decrypt(&a).unwrap(), b"\x08");
    assert!(Cipher::new(&[0; 16]).decrypt(&a).is_none());
    let mut bad = a.clone();
    *bad.last_mut().unwrap() ^= 1;
    assert!(cipher.decrypt(&bad).is_none());
    assert!(cipher.decrypt(&a[..20]).is_none());
    assert!(crypt::from_server(&[0xFE, 0x00]).is_none());
}

#[test]
fn the_voice_server_is_found_as_svc_finds_it() {
    assert_eq!(
        voice_target("", "mc.example.org", 24454),
        ("mc.example.org".into(), 24454)
    );
    assert_eq!(
        voice_target("24500", "mc.example.org", 24454),
        ("mc.example.org".into(), 24500)
    );
    assert_eq!(
        voice_target("0", "mc.example.org", 24454),
        ("mc.example.org".into(), 24454)
    );
    assert_eq!(
        voice_target("voice.example.org", "mc", 24454),
        ("voice.example.org".into(), 24454)
    );
    assert_eq!(
        voice_target("voice.example.org:1234", "mc", 24454),
        ("voice.example.org".into(), 1234)
    );
    assert_eq!(voice_target("[::1]:1234", "mc", 24454), ("::1".into(), 1234));
    assert_eq!(
        voice_target("voicechat://10.0.0.2:99", "mc", 24454),
        ("10.0.0.2".into(), 99)
    );
}

#[test]
fn readers_check_lengths_and_strings() {
    let mut w = Writer::new();
    w.varint(300).varint(-1).utf("héllo");
    let bytes = w.finish();
    let mut r = Reader::new(&bytes);
    assert_eq!(r.varint().unwrap(), 300);
    assert_eq!(r.varint().unwrap(), -1);
    assert_eq!(r.utf(16).unwrap(), "héllo");
    assert!(r.end().is_ok());
    let mut long = Writer::new();
    long.utf("seventeen chars!!");
    assert!(Reader::new(&long.finish()).utf(16).is_err());
    assert!(Reader::new(&[0xff, 0xff, 0xff, 0xff, 0xff, 0x01]).varint().is_err());
}
