// ViaHorizon's part of the protocol crate (as `horizon_protocol::hstudio_via_net`).
// On the threads that join servers and ping them for the server list, an
// address ViaHorizon routes parses as ViaProxy's internal host (the server
// and version, for ViaProxy to read from the handshake); that host's SRV
// lookup answers with the local ViaProxy, so the connection goes there; and
// OpenAuthMod's sign-in request, which ViaProxy sends for an online-mode
// server, is answered with the player's own session. Only the standard
// library: the client registers what it knows (how to route, where ViaProxy
// listens, how to join a session) as callbacks.

use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What ViaProxy splits an internal handshake host on:
/// `original-host \x07 address:port \x07 version`.
pub const SEP: char = '\u{7}';

/// OpenAuthMod's request to join the session server for a server hash.
pub const JOIN_CHANNEL: &str = "oam:join";

/// Login custom query (clientbound) and its answer (serverbound), protocol 777.
pub const CUSTOM_QUERY: i32 = 4;
pub const CUSTOM_QUERY_ANSWER: i32 = 2;

/// The longest handshake host vanilla sends.
pub const MAX_HOST: usize = 255;

/// What an address is parsed for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
    /// Joining the server (the game's `horizon-net` thread).
    Join,
    /// The server list's ping (`horizon-ping` threads).
    Ping,
}

type Router = Arc<dyn Fn(&str, Use) -> Option<String> + Send + Sync>;
type Proxy = Arc<dyn Fn(Duration) -> Result<u16, String> + Send + Sync>;
type Join = Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>;

static ROUTER: Mutex<Option<Router>> = Mutex::new(None);
static PROXY: Mutex<Option<Proxy>> = Mutex::new(None);
static JOIN: Mutex<Option<Join>> = Mutex::new(None);

/// How to route an address: the internal host to join it through, or `None`.
pub fn set_router(f: impl Fn(&str, Use) -> Option<String> + Send + Sync + 'static) {
    *ROUTER.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(f));
}

/// How to reach ViaProxy: waits up to the given time for it to listen and
/// returns its port, or why it can't be reached.
pub fn set_proxy(f: impl Fn(Duration) -> Result<u16, String> + Send + Sync + 'static) {
    *PROXY.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(f));
}

/// How to join a server's session (Mojang's session server, as the player).
pub fn set_join(f: impl Fn(&str) -> Result<(), String> + Send + Sync + 'static) {
    *JOIN.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(f));
}

/// The internal host for ViaProxy: where to go and as which version.
pub fn internal_host(original: &str, address: &str, version: &str) -> String {
    format!("{original}{SEP}{address}{SEP}{version}")
}

/// Whether a host is one [`internal_host`] made.
pub fn routed(host: &str) -> bool {
    host.contains(SEP)
}

/// The host `address` parses as, when ViaHorizon routes it from this thread.
pub fn route(address: &str) -> Option<String> {
    let using = match std::thread::current().name() {
        Some("horizon-net") => Use::Join,
        Some("horizon-ping") => Use::Ping,
        _ => return None,
    };
    route_for(address, using)
}

/// [`route`] for a use, whatever the thread.
pub fn route_for(address: &str, using: Use) -> Option<String> {
    if routed(address) {
        return None;
    }
    let f = ROUTER.lock().unwrap_or_else(|e| e.into_inner()).clone()?;
    f(address, using).filter(|h| routed(h) && h.len() <= MAX_HOST)
}

/// Where the SRV lookup of a routed host points: the local ViaProxy (or,
/// when it can't be reached, a host that says why). `None`: not routed.
pub fn redirect(srv_name: &str) -> Option<(String, u16)> {
    if !routed(srv_name) {
        return None;
    }
    Some(match proxy_port(Duration::from_secs(10)) {
        Ok(port) => ("127.0.0.1".to_owned(), port),
        Err(why) => (format!("ViaHorizon: {why}"), 25565),
    })
}

/// The address and version an internal host carries.
pub fn target(host: &str) -> Option<(&str, &str)> {
    let mut parts = host.split(SEP);
    let _original = parts.next()?;
    Some((parts.next()?, parts.next()?))
}

/// ViaProxy's port, once it listens.
pub fn proxy_port(wait: Duration) -> Result<u16, String> {
    let f = PROXY.lock().unwrap_or_else(|e| e.into_inner()).clone();
    match f {
        Some(f) => f(wait),
        None => Err("ViaHorizon isn't running".into()),
    }
}

/// The answer to a login packet when it is OpenAuthMod's sign-in request:
/// joins the session server, then the body of the custom query answer to
/// send (`Err`: why the player can't be signed in). `None` for every other
/// packet.
pub fn oam(id: i32, payload: &[u8]) -> Option<Result<Vec<u8>, String>> {
    if id != CUSTOM_QUERY {
        return None;
    }
    let mut r = Reader { data: payload, at: 0 };
    let transaction = r.varint()?;
    if r.string()? != JOIN_CHANNEL {
        return None;
    }
    let hash = r.string()?.to_owned();
    let f = JOIN.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let joined = match f {
        Some(f) => f(&hash),
        None => Err("You are not signed in".into()),
    };
    Some(joined.map(|()| answer(transaction, true)))
}

/// A login custom query answer whose payload is one boolean.
pub fn answer(transaction: i32, ok: bool) -> Vec<u8> {
    let mut out = Vec::new();
    varint(&mut out, CUSTOM_QUERY_ANSWER);
    varint(&mut out, transaction);
    out.push(1); // a payload follows
    out.push(u8::from(ok));
    out
}

/// OpenAuthMod's request, as ViaProxy sends it (for tests and tools).
pub fn join_request(transaction: i32, hash: &str) -> Vec<u8> {
    let mut out = Vec::new();
    varint(&mut out, transaction);
    string(&mut out, JOIN_CHANNEL);
    string(&mut out, hash);
    out
}

pub fn varint(out: &mut Vec<u8>, v: i32) {
    let mut v = v as u32;
    loop {
        if v & !0x7f == 0 {
            out.push(v as u8);
            return;
        }
        out.push((v as u8 & 0x7f) | 0x80);
        v >>= 7;
    }
}

pub fn string(out: &mut Vec<u8>, s: &str) {
    varint(out, s.len() as i32);
    out.extend_from_slice(s.as_bytes());
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn varint(&mut self) -> Option<i32> {
        let mut v: u32 = 0;
        for i in 0..5 {
            let b = *self.data.get(self.at)?;
            self.at += 1;
            v |= u32::from(b & 0x7f) << (7 * i);
            if b & 0x80 == 0 {
                return Some(v as i32);
            }
        }
        None
    }
    fn string(&mut self) -> Option<&'a str> {
        let n = usize::try_from(self.varint()?).ok()?;
        let end = self.at.checked_add(n).filter(|e| *e <= self.data.len() && n <= 32767 * 4)?;
        let s = std::str::from_utf8(&self.data[self.at..end]).ok()?;
        self.at = end;
        Some(s)
    }
}
