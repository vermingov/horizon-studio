//! Simple Voice Chat's wire formats: the custom payloads on the Minecraft
//! connection (`voicechat:*`) and the voice packets over UDP, for
//! compatibility versions 20 (SVC 2.6.x, the stable line) and 21 (2.7.x).
//! Everything is written as Minecraft's `FriendlyByteBuf` writes it:
//! big-endian numbers, UUIDs as two longs, VarInt-prefixed strings and byte
//! arrays. A UUID is a `u128` (most significant long first), as Horizon keeps
//! profiles.

use std::fmt;

/// The compatibility versions this client speaks.
pub const COMPAT_20: i32 = 20;
pub const COMPAT_21: i32 = 21;

/// 48 kHz mono, 20 ms frames, as SVC codes them.
pub const SAMPLE_RATE: u32 = 48_000;
pub const FRAME_SIZE: usize = 960;
/// The largest Opus packet SVC sends or reads.
pub const MAX_OPUS_PAYLOAD: usize = 1275;
/// The largest encrypted payload in one datagram.
pub const MAX_PACKET: usize = 2048;
/// The first byte of every datagram.
pub const MAGIC: u8 = 0xFF;

pub const REGISTER: &str = "minecraft:register";
pub const REQUEST_SECRET: &str = "voicechat:request_secret";
pub const SECRET: &str = "voicechat:secret";
pub const STATES: &str = "voicechat:states";
pub const STATE: &str = "voicechat:state";
pub const REMOVE_STATE: &str = "voicechat:remove_state";
pub const ADD_GROUP: &str = "voicechat:add_group";
pub const REMOVE_GROUP: &str = "voicechat:remove_group";
pub const JOINED_GROUP: &str = "voicechat:joined_group";
pub const ADD_CATEGORY: &str = "voicechat:add_category";
pub const REMOVE_CATEGORY: &str = "voicechat:remove_category";
/// Compat 21's own state (`muted, disabled`), and compat 20's (`disabled`).
pub const CLIENT_STATE: &str = "voicechat:client_state";
pub const UPDATE_STATE: &str = "voicechat:update_state";
pub const SET_GROUP: &str = "voicechat:set_group";
pub const CREATE_GROUP: &str = "voicechat:create_group";
pub const LEAVE_GROUP: &str = "voicechat:leave_group";

/// The channels the server sends on: announced with `minecraft:register`
/// before anything else (NeoForge sends nothing on a channel the client
/// didn't announce).
pub const CLIENTBOUND: [&str; 9] = [
    SECRET,
    STATES,
    STATE,
    REMOVE_STATE,
    ADD_GROUP,
    REMOVE_GROUP,
    JOINED_GROUP,
    ADD_CATEGORY,
    REMOVE_CATEGORY,
];

/// A payload that doesn't parse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtoError(pub String);

impl fmt::Display for ProtoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ProtoError {}

fn err<T>(what: &str) -> Result<T, ProtoError> {
    Err(ProtoError(what.to_owned()))
}

// ----------------------------------------------------------------- buffers

/// `FriendlyByteBuf`'s writes.
#[derive(Default, Clone, Debug)]
pub struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        Writer { buf: Vec::new() }
    }
    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.buf.push(v);
        self
    }
    pub fn bool(&mut self, v: bool) -> &mut Self {
        self.u8(u8::from(v))
    }
    pub fn i16(&mut self, v: i16) -> &mut Self {
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub fn i32(&mut self, v: i32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub fn i64(&mut self, v: i64) -> &mut Self {
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub fn f32(&mut self, v: f32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub fn f64(&mut self, v: f64) -> &mut Self {
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub fn uuid(&mut self, v: u128) -> &mut Self {
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub fn bytes(&mut self, v: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(v);
        self
    }
    pub fn varint(&mut self, v: i32) -> &mut Self {
        let mut v = v as u32;
        loop {
            if v & !0x7F == 0 {
                self.buf.push(v as u8);
                return self;
            }
            self.buf.push((v & 0x7F) as u8 | 0x80);
            v >>= 7;
        }
    }
    pub fn byte_array(&mut self, v: &[u8]) -> &mut Self {
        self.varint(v.len() as i32);
        self.bytes(v)
    }
    pub fn utf(&mut self, v: &str) -> &mut Self {
        self.byte_array(v.as_bytes())
    }
    pub fn finish(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.buf)
    }
}

/// `FriendlyByteBuf`'s reads, failing past the end.
#[derive(Clone, Debug)]
pub struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Reader { data, at: 0 }
    }
    pub fn left(&self) -> usize {
        self.data.len() - self.at
    }
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], ProtoError> {
        if self.left() < n {
            return err("the payload ends early");
        }
        let s = &self.data[self.at..self.at + n];
        self.at += n;
        Ok(s)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], ProtoError> {
        let mut a = [0; N];
        a.copy_from_slice(self.take(N)?);
        Ok(a)
    }
    pub fn u8(&mut self) -> Result<u8, ProtoError> {
        Ok(self.take(1)?[0])
    }
    pub fn bool(&mut self) -> Result<bool, ProtoError> {
        Ok(self.u8()? != 0)
    }
    pub fn i16(&mut self) -> Result<i16, ProtoError> {
        Ok(i16::from_be_bytes(self.array()?))
    }
    pub fn i32(&mut self) -> Result<i32, ProtoError> {
        Ok(i32::from_be_bytes(self.array()?))
    }
    pub fn i64(&mut self) -> Result<i64, ProtoError> {
        Ok(i64::from_be_bytes(self.array()?))
    }
    pub fn f32(&mut self) -> Result<f32, ProtoError> {
        Ok(f32::from_be_bytes(self.array()?))
    }
    pub fn f64(&mut self) -> Result<f64, ProtoError> {
        Ok(f64::from_be_bytes(self.array()?))
    }
    pub fn uuid(&mut self) -> Result<u128, ProtoError> {
        Ok(u128::from_be_bytes(self.array()?))
    }
    pub fn bytes16(&mut self) -> Result<[u8; 16], ProtoError> {
        self.array()
    }
    pub fn varint(&mut self) -> Result<i32, ProtoError> {
        let mut v: u32 = 0;
        for i in 0..5 {
            let b = self.u8()?;
            v |= u32::from(b & 0x7F) << (7 * i);
            if b & 0x80 == 0 {
                return Ok(v as i32);
            }
        }
        err("a VarInt is too long")
    }
    /// A byte array of at most `max` bytes.
    pub fn byte_array(&mut self, max: usize) -> Result<Vec<u8>, ProtoError> {
        let n = self.varint()?;
        if n < 0 || n as usize > max {
            return err("a byte array is too long");
        }
        Ok(self.take(n as usize)?.to_vec())
    }
    /// A string of at most `max` characters (and `3 * max` bytes, as Minecraft reads them).
    pub fn utf(&mut self, max: usize) -> Result<String, ProtoError> {
        let n = self.varint()?;
        if n < 0 || n as usize > max * 3 {
            return err("a string is too long");
        }
        let s = std::str::from_utf8(self.take(n as usize)?).map_err(|_| ProtoError("a string isn't UTF-8".into()))?;
        if s.encode_utf16().count() > max {
            return err("a string is too long");
        }
        Ok(s.to_owned())
    }
    /// Nothing may follow.
    pub fn end(&self) -> Result<(), ProtoError> {
        if self.left() == 0 {
            Ok(())
        } else {
            err("the payload has bytes left over")
        }
    }
}

// ----------------------------------------------------------------- the Minecraft connection

/// The Opus application the server asks for (`ServerConfig.Codec`, by ordinal).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Voip,
    Audio,
    RestrictedLowDelay,
}

impl Codec {
    pub fn from_ordinal(b: u8) -> Codec {
        match b {
            1 => Codec::Audio,
            2 => Codec::RestrictedLowDelay,
            _ => Codec::Voip,
        }
    }
    pub fn ordinal(self) -> u8 {
        match self {
            Codec::Voip => 0,
            Codec::Audio => 1,
            Codec::RestrictedLowDelay => 2,
        }
    }
}

/// `voicechat:secret`: what the UDP connection needs.
#[derive(Clone, Debug, PartialEq)]
pub struct Secret {
    /// The AES-128 key.
    pub secret: [u8; 16],
    /// Compat 21: the id the client's datagrams carry (compat 20 sends the player's UUID).
    pub connection_id: Option<u128>,
    pub server_port: i32,
    pub player: u128,
    pub codec: Codec,
    pub mtu: i32,
    pub distance: f64,
    /// The server's keep-alive interval, in milliseconds.
    pub keep_alive: i32,
    pub groups: bool,
    /// Empty: the Minecraft server's host. A port alone, or `host[:port]`.
    pub voice_host: String,
    pub allow_recording: bool,
}

impl Secret {
    pub fn decode(data: &[u8], compat: i32) -> Result<Secret, ProtoError> {
        let mut r = Reader::new(data);
        let secret = r.bytes16()?;
        let connection_id = if compat >= COMPAT_21 { Some(r.uuid()?) } else { None };
        let s = Secret {
            secret,
            connection_id,
            server_port: r.i32()?,
            player: r.uuid()?,
            codec: Codec::from_ordinal(r.u8()?),
            mtu: r.i32()?,
            distance: r.f64()?,
            keep_alive: r.i32()?,
            groups: r.bool()?,
            voice_host: r.utf(32767)?,
            allow_recording: r.bool()?,
        };
        r.end()?;
        Ok(s)
    }
    /// Whichever layout reads the payload exactly, `compat`'s first: a server
    /// answering an earlier request may answer as the other version.
    pub fn decode_any(data: &[u8], compat: i32) -> Result<(Secret, i32), ProtoError> {
        let other = if compat == COMPAT_21 { COMPAT_20 } else { COMPAT_21 };
        match Secret::decode(data, compat) {
            Ok(s) => Ok((s, compat)),
            Err(e) => Secret::decode(data, other).map(|s| (s, other)).map_err(|_| e),
        }
    }
    pub fn encode(&self, compat: i32) -> Vec<u8> {
        let mut w = Writer::new();
        w.bytes(&self.secret);
        if compat >= COMPAT_21 {
            w.uuid(self.connection_id.unwrap_or(0));
        }
        w.i32(self.server_port)
            .uuid(self.player)
            .u8(self.codec.ordinal())
            .i32(self.mtu)
            .f64(self.distance)
            .i32(self.keep_alive)
            .bool(self.groups)
            .utf(&self.voice_host)
            .bool(self.allow_recording);
        w.finish()
    }
}

/// A player's voice chat state, as the server tells it.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PlayerState {
    pub uuid: u128,
    pub name: String,
    pub muted: bool,
    pub force_muted: bool,
    pub disabled: bool,
    pub force_disabled: bool,
    /// No voice connection (or none yet).
    pub disconnected: bool,
    pub group: Option<u128>,
}

impl PlayerState {
    pub fn read(r: &mut Reader<'_>, compat: i32) -> Result<PlayerState, ProtoError> {
        if compat >= COMPAT_21 {
            let flags = r.u8()?;
            let uuid = r.uuid()?;
            let name = r.utf(256)?;
            let group = if flags & 0x20 != 0 { Some(r.uuid()?) } else { None };
            Ok(PlayerState {
                uuid,
                name,
                muted: flags & 0x01 != 0,
                force_muted: flags & 0x02 != 0,
                disabled: flags & 0x04 != 0,
                force_disabled: flags & 0x08 != 0,
                disconnected: flags & 0x10 != 0,
                group,
            })
        } else {
            let disabled = r.bool()?;
            let disconnected = r.bool()?;
            let uuid = r.uuid()?;
            let name = r.utf(32767)?;
            let group = if r.bool()? { Some(r.uuid()?) } else { None };
            Ok(PlayerState {
                uuid,
                name,
                disabled,
                disconnected,
                group,
                ..PlayerState::default()
            })
        }
    }
    pub fn write(&self, w: &mut Writer, compat: i32) {
        if compat >= COMPAT_21 {
            let flags = u8::from(self.muted)
                | u8::from(self.force_muted) << 1
                | u8::from(self.disabled) << 2
                | u8::from(self.force_disabled) << 3
                | u8::from(self.disconnected) << 4
                | u8::from(self.group.is_some()) << 5;
            w.u8(flags).uuid(self.uuid).utf(&self.name);
            if let Some(g) = self.group {
                w.uuid(g);
            }
        } else {
            w.bool(self.disabled)
                .bool(self.disconnected)
                .uuid(self.uuid)
                .utf(&self.name);
            w.bool(self.group.is_some());
            if let Some(g) = self.group {
                w.uuid(g);
            }
        }
    }
}

/// A group the server lists (`ClientGroup`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub id: u128,
    pub name: String,
    pub has_password: bool,
    pub persistent: bool,
    pub hidden: bool,
    /// 0 normal, 1 open, 2 isolated.
    pub kind: i16,
}

impl Group {
    pub fn read(r: &mut Reader<'_>) -> Result<Group, ProtoError> {
        Ok(Group {
            id: r.uuid()?,
            // 2.7 names have at most 24 characters, 2.6's 512.
            name: r.utf(512)?,
            has_password: r.bool()?,
            persistent: r.bool()?,
            hidden: r.bool()?,
            kind: r.i16()?,
        })
    }
    pub fn write(&self, w: &mut Writer) {
        w.uuid(self.id)
            .utf(&self.name)
            .bool(self.has_password)
            .bool(self.persistent)
            .bool(self.hidden)
            .i16(self.kind);
    }
}

/// A `voicechat:*` payload from the server.
#[derive(Clone, Debug, PartialEq)]
pub enum Clientbound {
    Secret(Secret),
    States(Vec<PlayerState>),
    State(PlayerState),
    RemoveState(u128),
    AddGroup(Group),
    RemoveGroup(u128),
    JoinedGroup {
        group: Option<u128>,
        wrong_password: bool,
    },
    /// Volume categories: nothing this client uses.
    Category,
}

/// A server payload: `None` for a channel that isn't voice chat's.
pub fn decode_clientbound(channel: &str, data: &[u8], compat: i32) -> Option<Result<Clientbound, ProtoError>> {
    let mut r = Reader::new(data);
    let packet = match channel {
        SECRET => return Some(Secret::decode_any(data, compat).map(|(s, _)| Clientbound::Secret(s))),
        STATES => (|| {
            let n = r.i32()?;
            if !(0..=100_000).contains(&n) {
                return err("too many states");
            }
            let mut v = Vec::with_capacity(n as usize);
            for _ in 0..n {
                v.push(PlayerState::read(&mut r, compat)?);
            }
            Ok(Clientbound::States(v))
        })(),
        STATE => PlayerState::read(&mut r, compat).map(Clientbound::State),
        REMOVE_STATE => r.uuid().map(Clientbound::RemoveState),
        ADD_GROUP => Group::read(&mut r).map(Clientbound::AddGroup),
        REMOVE_GROUP => r.uuid().map(Clientbound::RemoveGroup),
        JOINED_GROUP => (|| {
            let group = if r.bool()? { Some(r.uuid()?) } else { None };
            let wrong_password = r.bool()?;
            Ok(Clientbound::JoinedGroup { group, wrong_password })
        })(),
        ADD_CATEGORY | REMOVE_CATEGORY => return Some(Ok(Clientbound::Category)),
        _ if channel.starts_with("voicechat:") => return Some(err("an unknown voice chat payload")),
        _ => return None,
    };
    Some(packet)
}

/// `minecraft:register`'s body: the channels, NUL between them.
pub fn register() -> Vec<u8> {
    CLIENTBOUND.join("\0").into_bytes()
}

/// `voicechat:request_secret`.
pub fn request_secret(compat: i32) -> Vec<u8> {
    Writer::new().i32(compat).finish()
}

/// The player's own state: compat 21's `client_state(muted, disabled)`,
/// compat 20's `update_state(disabled)`.
pub fn client_state(compat: i32, muted: bool, disabled: bool) -> (&'static str, Vec<u8>) {
    if compat >= COMPAT_21 {
        (CLIENT_STATE, Writer::new().bool(muted).bool(disabled).finish())
    } else {
        (UPDATE_STATE, Writer::new().bool(disabled).finish())
    }
}

/// `voicechat:set_group`: joins a group, with its password if it has one.
pub fn set_group(group: u128, password: Option<&str>) -> Vec<u8> {
    let mut w = Writer::new();
    w.uuid(group).bool(password.is_some());
    if let Some(p) = password {
        w.utf(p);
    }
    w.finish()
}

// ----------------------------------------------------------------- UDP

/// A voice packet, decrypted (`NetworkMessage`'s type byte and body).
#[derive(Clone, Debug, PartialEq)]
pub enum Udp {
    /// 0x01, client to server: an Opus frame (empty: the speaker stopped).
    Mic { data: Vec<u8>, seq: i64, whispering: bool },
    /// 0x02: a player's voice, heard at their place (`distance` its reach).
    PlayerSound {
        channel: u128,
        sender: u128,
        data: Vec<u8>,
        seq: i64,
        distance: f32,
        whispering: bool,
        category: Option<String>,
    },
    /// 0x03: a group member's voice, heard as it is.
    GroupSound {
        channel: u128,
        sender: u128,
        data: Vec<u8>,
        seq: i64,
        category: Option<String>,
    },
    /// 0x04: a voice at a place.
    LocationSound {
        channel: u128,
        sender: u128,
        pos: [f64; 3],
        data: Vec<u8>,
        seq: i64,
        distance: f32,
        category: Option<String>,
    },
    /// 0x05: compat 20 carries the player and the secret; compat 21 nothing.
    Authenticate { player: Option<(u128, [u8; 16])> },
    /// 0x06: compat 21 carries the nonce the connection check returns.
    AuthenticateAck { nonce: Option<[u8; 16]> },
    /// 0x07: echoed back as it came.
    Ping { id: u128, timestamp: i64 },
    /// 0x08, both ways.
    KeepAlive,
    /// 0x09: compat 21 returns the nonce.
    ConnectionCheck { nonce: Option<[u8; 16]> },
    /// 0x0A: the connection is up.
    ConnectionCheckAck,
}

const WHISPER: u8 = 0x01;
const HAS_CATEGORY: u8 = 0x02;

fn write_flags(w: &mut Writer, whispering: bool, category: &Option<String>) {
    let flags = if whispering { WHISPER } else { 0 } | if category.is_some() { HAS_CATEGORY } else { 0 };
    w.u8(flags);
    if let Some(c) = category {
        w.utf(c);
    }
}

fn read_flags(r: &mut Reader<'_>) -> Result<(bool, Option<String>), ProtoError> {
    let flags = r.u8()?;
    let category = if flags & HAS_CATEGORY != 0 {
        Some(r.utf(16)?)
    } else {
        None
    };
    Ok((flags & WHISPER != 0, category))
}

impl Udp {
    pub fn kind(&self) -> u8 {
        match self {
            Udp::Mic { .. } => 0x01,
            Udp::PlayerSound { .. } => 0x02,
            Udp::GroupSound { .. } => 0x03,
            Udp::LocationSound { .. } => 0x04,
            Udp::Authenticate { .. } => 0x05,
            Udp::AuthenticateAck { .. } => 0x06,
            Udp::Ping { .. } => 0x07,
            Udp::KeepAlive => 0x08,
            Udp::ConnectionCheck { .. } => 0x09,
            Udp::ConnectionCheckAck => 0x0A,
        }
    }

    /// The plaintext: the type byte, then the body.
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.u8(self.kind());
        match self {
            Udp::Mic { data, seq, whispering } => {
                w.byte_array(data).i64(*seq).bool(*whispering);
            }
            Udp::PlayerSound {
                channel,
                sender,
                data,
                seq,
                distance,
                whispering,
                category,
            } => {
                w.uuid(*channel).uuid(*sender).byte_array(data).i64(*seq).f32(*distance);
                write_flags(&mut w, *whispering, category);
            }
            Udp::GroupSound {
                channel,
                sender,
                data,
                seq,
                category,
            } => {
                w.uuid(*channel).uuid(*sender).byte_array(data).i64(*seq);
                write_flags(&mut w, false, category);
            }
            Udp::LocationSound {
                channel,
                sender,
                pos,
                data,
                seq,
                distance,
                category,
            } => {
                w.uuid(*channel).uuid(*sender).f64(pos[0]).f64(pos[1]).f64(pos[2]);
                w.byte_array(data).i64(*seq).f32(*distance);
                write_flags(&mut w, false, category);
            }
            Udp::Authenticate { player } => {
                if let Some((uuid, secret)) = player {
                    w.uuid(*uuid).bytes(secret);
                }
            }
            Udp::AuthenticateAck { nonce } | Udp::ConnectionCheck { nonce } => {
                if let Some(n) = nonce {
                    w.bytes(n);
                }
            }
            Udp::Ping { id, timestamp } => {
                w.uuid(*id).i64(*timestamp);
            }
            Udp::KeepAlive | Udp::ConnectionCheckAck => {}
        }
        w.finish()
    }

    /// A plaintext, either way. Bytes after a packet's body are left alone,
    /// as SVC reads them.
    pub fn decode(data: &[u8]) -> Result<Udp, ProtoError> {
        let mut r = Reader::new(data);
        let nonce = |r: &mut Reader<'_>| -> Result<Option<[u8; 16]>, ProtoError> {
            Ok(if r.left() >= 16 { Some(r.bytes16()?) } else { None })
        };
        Ok(match r.u8()? {
            0x01 => Udp::Mic {
                data: r.byte_array(MAX_OPUS_PAYLOAD)?,
                seq: r.i64()?,
                whispering: r.bool()?,
            },
            0x02 => {
                let channel = r.uuid()?;
                let sender = r.uuid()?;
                let data = r.byte_array(MAX_OPUS_PAYLOAD)?;
                let seq = r.i64()?;
                let distance = r.f32()?;
                let (whispering, category) = read_flags(&mut r)?;
                Udp::PlayerSound {
                    channel,
                    sender,
                    data,
                    seq,
                    distance,
                    whispering,
                    category,
                }
            }
            0x03 => {
                let channel = r.uuid()?;
                let sender = r.uuid()?;
                let data = r.byte_array(MAX_OPUS_PAYLOAD)?;
                let seq = r.i64()?;
                let (_, category) = read_flags(&mut r)?;
                Udp::GroupSound {
                    channel,
                    sender,
                    data,
                    seq,
                    category,
                }
            }
            0x04 => {
                let channel = r.uuid()?;
                let sender = r.uuid()?;
                let pos = [r.f64()?, r.f64()?, r.f64()?];
                let data = r.byte_array(MAX_OPUS_PAYLOAD)?;
                let seq = r.i64()?;
                let distance = r.f32()?;
                let (_, category) = read_flags(&mut r)?;
                Udp::LocationSound {
                    channel,
                    sender,
                    pos,
                    data,
                    seq,
                    distance,
                    category,
                }
            }
            0x05 => Udp::Authenticate {
                player: if r.left() >= 32 {
                    Some((r.uuid()?, r.bytes16()?))
                } else {
                    None
                },
            },
            0x06 => Udp::AuthenticateAck { nonce: nonce(&mut r)? },
            0x07 => Udp::Ping {
                id: r.uuid()?,
                timestamp: r.i64()?,
            },
            0x08 => Udp::KeepAlive,
            0x09 => Udp::ConnectionCheck { nonce: nonce(&mut r)? },
            0x0A => Udp::ConnectionCheckAck,
            other => return Err(ProtoError(format!("an unknown voice packet type {other}"))),
        })
    }
}

/// The UUID as Java prints it (`8-4-4-4-12` lowercase hex).
pub fn uuid_string(u: u128) -> String {
    let h = format!("{u:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

/// Where the voice server is: `voice_host` from the secret if it says
/// (a port alone replaces the port; `host[:port]` both), else the Minecraft
/// server's host and the secret's port.
pub fn voice_target(voice_host: &str, server_host: &str, server_port: i32) -> (String, u16) {
    let fallback_port = u16::try_from(server_port).unwrap_or(24454);
    let v = voice_host.trim();
    if v.is_empty() {
        return (server_host.to_owned(), fallback_port);
    }
    if let Ok(p) = v.parse::<u32>() {
        if (1..=65535).contains(&p) {
            return (server_host.to_owned(), p as u16);
        }
        return (server_host.to_owned(), fallback_port);
    }
    let v = v.strip_prefix("voicechat://").unwrap_or(v);
    let v = v.split('/').next().unwrap_or(v);
    // [v6]:port, v6, host:port, host
    if let Some(rest) = v.strip_prefix('[')
        && let Some((host, after)) = rest.split_once(']')
    {
        let port = after
            .strip_prefix(':')
            .and_then(|p| p.parse().ok())
            .unwrap_or(fallback_port);
        return (host.to_owned(), port);
    }
    if v.matches(':').count() == 1
        && let Some((host, port)) = v.split_once(':')
    {
        return (host.to_owned(), port.parse().unwrap_or(fallback_port));
    }
    (v.to_owned(), fallback_port)
}
