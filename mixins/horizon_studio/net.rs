// The network for scripts (hstudio_net): HTTP requests (`request`,
// `game:HttpGet`) and WebSockets (`WebSocket.connect`). Scripts reach the
// internet only, not this machine or its network: every address a name
// resolves to is checked, so a script can't talk to the studio's AI link, a
// router or anything else on the LAN. Work happens on threads of its own and
// is polled from the frame, so a script waits for an answer (its thread
// yields) while the game keeps drawing.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::Duration;

/// The largest body or message taken in (16 MiB).
pub const MAX_BODY: u64 = 16 << 20;
const TIMEOUT: Duration = Duration::from_secs(30);

// ------------------------------------------------------------------ where

/// Whether `ip` is on the internet: not this machine, a private or
/// link-local network, carrier-grade NAT, multicast or unspecified.
pub fn public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || a == 0
                || (a == 100 && (64..128).contains(&b))
                || (a == 192 && b == 0 && v4.octets()[2] == 0))
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return public(IpAddr::V4(v4));
            }
            let first = v6.segments()[0];
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80)
        }
    }
}

/// `host:port`'s addresses that may be reached (all of them with `local`).
fn resolve(host_port: &str, local: bool) -> Result<Vec<SocketAddr>, String> {
    let all: Vec<SocketAddr> = host_port
        .to_socket_addrs()
        .map_err(|e| format!("can't find {host_port}: {e}"))?
        .collect();
    let ok: Vec<SocketAddr> = all.iter().copied().filter(|a| local || public(a.ip())).collect();
    if ok.is_empty() && !all.is_empty() {
        return Err(format!(
            "{host_port} is on this computer or its network, which scripts can't reach"
        ));
    }
    Ok(ok)
}

// ------------------------------------------------------------------ TLS

/// The system's root certificates, read once: a bundle file on Linux and
/// macOS, the machine's store (through PowerShell) on Windows.
fn tls() -> Result<Arc<ureq::rustls::ClientConfig>, String> {
    static CONFIG: OnceLock<Result<Arc<ureq::rustls::ClientConfig>, String>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            use ureq::rustls::pki_types::CertificateDer;
            use ureq::rustls::pki_types::pem::PemObject;
            let mut roots = ureq::rustls::RootCertStore::empty();
            for path in [
                "/etc/ssl/certs/ca-certificates.crt",
                "/etc/pki/tls/certs/ca-bundle.crt",
                "/etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem",
                "/etc/ssl/ca-bundle.pem",
                "/etc/ssl/cert.pem",
                "/usr/local/etc/openssl/cert.pem",
                "/opt/homebrew/etc/openssl@3/cert.pem",
            ] {
                if let Ok(pem) = std::fs::read(path) {
                    for der in CertificateDer::pem_slice_iter(&pem).flatten() {
                        let _ = roots.add(der);
                    }
                    if !roots.is_empty() {
                        break;
                    }
                }
            }
            if roots.is_empty() && cfg!(windows) {
                for der in windows_roots() {
                    let _ = roots.add(CertificateDer::from(der));
                }
            }
            if roots.is_empty() {
                return Err(
                    "no root certificates found on this computer, so secure connections can't be checked".into(),
                );
            }
            let provider = Arc::new(ureq::rustls::crypto::ring::default_provider());
            let config = ureq::rustls::ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .map_err(|e| e.to_string())?
                .with_root_certificates(roots)
                .with_no_client_auth();
            Ok(Arc::new(config))
        })
        .clone()
}

/// The Windows machine's trusted roots, as DER.
fn windows_roots() -> Vec<Vec<u8>> {
    let script = "Get-ChildItem Cert:\\LocalMachine\\Root,Cert:\\CurrentUser\\Root | \
                  ForEach-Object { [Convert]::ToBase64String($_.RawData) }";
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", script]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let Ok(out) = cmd.output() else { return Vec::new() };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| unbase64(l.trim()))
        .collect()
}

// ------------------------------------------------------------------ HTTP

/// A request, as `request{...}` takes it.
#[derive(Clone, Debug, Default)]
pub struct Request {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// What came back.
#[derive(Clone, Debug, Default)]
pub struct Response {
    pub status: u16,
    pub status_message: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// A request on its way: `poll` it each frame.
pub struct Pending(mpsc::Receiver<Result<Response, String>>);

impl Pending {
    /// The answer, once there is one.
    pub fn poll(&self) -> Option<Result<Response, String>> {
        match self.0.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err("the request was lost".into())),
        }
    }
    /// Waits for the answer (where a script can't yield).
    pub fn wait(self) -> Result<Response, String> {
        self.0
            .recv_timeout(TIMEOUT + Duration::from_secs(5))
            .unwrap_or_else(|_| Err("the request timed out".into()))
    }
}

/// Sends `req` on a thread of its own (`local`: addresses on this machine
/// and its network allowed, for tests).
pub fn send(req: Request, local: bool) -> Pending {
    let (tx, rx) = mpsc::channel();
    let _ = std::thread::Builder::new().name("script-http".into()).spawn(move || {
        let _ = tx.send(fetch(&req, local));
    });
    Pending(rx)
}

/// The request, done here.
pub fn fetch(req: &Request, local: bool) -> Result<Response, String> {
    let method = if req.method.is_empty() {
        "GET".to_owned()
    } else {
        req.method.to_ascii_uppercase()
    };
    if !req.url.starts_with("http://") && !req.url.starts_with("https://") {
        return Err(format!("not a web address: {}", req.url));
    }
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout(TIMEOUT)
        .redirects(5)
        .resolver(move |hp: &str| resolve(hp, local).map_err(std::io::Error::other))
        .tls_config(tls()?)
        .user_agent("Horizon Studio")
        .build();
    let mut r = agent.request(&method, &req.url);
    for (k, v) in &req.headers {
        r = r.set(k, v);
    }
    let result = if req.body.is_empty() && matches!(method.as_str(), "GET" | "HEAD" | "DELETE" | "OPTIONS") {
        r.call()
    } else {
        r.send_bytes(&req.body)
    };
    let resp = match result {
        Ok(resp) | Err(ureq::Error::Status(_, resp)) => resp,
        Err(ureq::Error::Transport(t)) => return Err(transport(&t)),
    };
    let status = resp.status();
    let status_message = resp.status_text().to_owned();
    let headers = resp
        .headers_names()
        .into_iter()
        .filter_map(|n| resp.header(&n).map(|v| (n.clone(), v.to_owned())))
        .collect();
    let mut body = Vec::new();
    resp.into_reader()
        .take(MAX_BODY + 1)
        .read_to_end(&mut body)
        .map_err(|e| e.to_string())?;
    if body.len() as u64 > MAX_BODY {
        return Err("the answer is larger than 16 MiB".into());
    }
    Ok(Response {
        status,
        status_message,
        headers,
        body,
    })
}

/// A transport error's message, with the address policy's own when that
/// is why.
fn transport(t: &ureq::Transport) -> String {
    let text = t.to_string();
    match text.find("scripts can't reach") {
        Some(_) => text
            .split(": ")
            .find(|p| p.contains("scripts can't reach"))
            .unwrap_or(&text)
            .to_owned(),
        None => text,
    }
}

// ------------------------------------------------------------------ WebSockets

/// What a socket did, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Open,
    /// A message: its bytes, and whether it was text.
    Message(Vec<u8>, bool),
    /// The socket closed: why, if it wasn't asked to.
    Closed(Option<String>),
}

enum Out {
    Text(Vec<u8>),
    Binary(Vec<u8>),
    Close,
}

/// A WebSocket: it connects and talks on a thread of its own.
pub struct Socket {
    events: Arc<Mutex<VecDeque<Event>>>,
    out: mpsc::Sender<Out>,
}

impl Socket {
    /// Opens a socket to `url` (ws:// or wss://); its first event is Open,
    /// or Closed with why it couldn't.
    pub fn open(url: &str, local: bool) -> Socket {
        let events = Arc::new(Mutex::new(VecDeque::new()));
        let (out, rx) = mpsc::channel();
        let (url, ev) = (url.to_owned(), events.clone());
        let _ = std::thread::Builder::new()
            .name("script-websocket".into())
            .spawn(move || {
                let push = |e: Event| ev.lock().unwrap_or_else(|p| p.into_inner()).push_back(e);
                match connect(&url, local) {
                    Ok((stream, tcp)) => {
                        push(Event::Open);
                        // Reading blocks and the TLS stream can't be shared with
                        // a reader thread, so this one alternates: short reads,
                        // and what is to be sent between them.
                        let _ = tcp.set_read_timeout(Some(Duration::from_millis(20)));
                        let why = run(stream, &rx, &push);
                        push(Event::Closed(why));
                    }
                    Err(e) => push(Event::Closed(Some(e))),
                }
            });
        Socket { events, out }
    }
    /// What happened since the last call.
    pub fn events(&self) -> Vec<Event> {
        self.events
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .drain(..)
            .collect()
    }
    pub fn send(&self, data: &[u8], text: bool) {
        let _ = self.out.send(if text {
            Out::Text(data.to_vec())
        } else {
            Out::Binary(data.to_vec())
        });
    }
    pub fn close(&self) {
        let _ = self.out.send(Out::Close);
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        let _ = self.out.send(Out::Close);
    }
}

/// A plain or TLS stream.
trait Conn: Read + Write + Send {}
impl<T: Read + Write + Send> Conn for T {}

/// Connects and does the opening handshake: the stream, and its socket
/// (for its timeouts).
fn connect(url: &str, local: bool) -> Result<(Box<dyn Conn>, TcpStream), String> {
    let (secure, rest) = if let Some(r) = url.strip_prefix("wss://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("ws://") {
        (false, r)
    } else {
        return Err(format!("not a WebSocket address (ws:// or wss://): {url}"));
    };
    let (authority, path) = match rest.find(['/', '?']) {
        Some(i) if rest.as_bytes()[i] == b'?' => (&rest[..i], format!("/{}", &rest[i..])),
        Some(i) => (&rest[..i], rest[i..].to_owned()),
        None => (rest, "/".to_owned()),
    };
    let authority = authority.rsplit_once('@').map_or(authority, |(_, a)| a);
    let (host, port) = match authority.rsplit_once(':').filter(|(_, p)| p.parse::<u16>().is_ok()) {
        Some((h, p)) => (h.trim_matches(['[', ']']), p.parse::<u16>().unwrap_or(0)),
        None => (authority.trim_matches(['[', ']']), if secure { 443 } else { 80 }),
    };
    if host.is_empty() {
        return Err(format!("no host in {url}"));
    }
    let addrs = resolve(&format!("{host}:{port}"), local)?;
    let tcp = addrs
        .iter()
        .find_map(|a| TcpStream::connect_timeout(a, Duration::from_secs(10)).ok())
        .ok_or_else(|| format!("can't connect to {host}:{port}"))?;
    tcp.set_read_timeout(Some(TIMEOUT)).map_err(|e| e.to_string())?;
    let _ = tcp.set_nodelay(true);
    let socket = tcp.try_clone().map_err(|e| e.to_string())?;
    let mut stream: Box<dyn Conn> = if secure {
        let name = ureq::rustls::pki_types::ServerName::try_from(host.to_owned()).map_err(|e| e.to_string())?;
        let conn = ureq::rustls::ClientConnection::new(tls()?, name).map_err(|e| e.to_string())?;
        Box::new(ureq::rustls::StreamOwned::new(conn, tcp))
    } else {
        Box::new(tcp)
    };
    let key = base64(&random16());
    let host_header = if (secure && port == 443) || (!secure && port == 80) {
        host.to_owned()
    } else {
        format!("{host}:{port}")
    };
    let hello = format!(
        "GET {path} HTTP/1.1\r\nHost: {host_header}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\nUser-Agent: Horizon Studio\r\n\r\n"
    );
    stream.write_all(hello.as_bytes()).map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())?;
    // The answer's head, byte by byte (it's short), up to its blank line.
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() > 16 << 10 {
            return Err("the server's answer is too long".into());
        }
        match stream.read(&mut byte) {
            Ok(1) => head.push(byte[0]),
            Ok(_) => return Err("the server closed the connection".into()),
            Err(e) => return Err(e.to_string()),
        }
    }
    let head = String::from_utf8_lossy(&head);
    let status = head.lines().next().unwrap_or("");
    if !status.split_whitespace().nth(1).is_some_and(|c| c == "101") {
        return Err(format!("the server didn't accept the WebSocket: {status}"));
    }
    let accept = head
        .lines()
        .find_map(|l| {
            l.split_once(':')
                .filter(|(k, _)| k.trim().eq_ignore_ascii_case("sec-websocket-accept"))
        })
        .map(|(_, v)| v.trim().to_owned())
        .unwrap_or_default();
    let want = {
        use sha1::{Digest, Sha1};
        base64(&Sha1::digest(
            format!("{key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11").as_bytes(),
        ))
    };
    if accept != want {
        return Err("the server's handshake is wrong".into());
    }
    Ok((stream, socket))
}

/// Reads and writes frames until the socket closes: why it closed, if it
/// wasn't asked to.
fn run(mut stream: Box<dyn Conn>, out: &mpsc::Receiver<Out>, push: &dyn Fn(Event)) -> Option<String> {
    let mut buf: Vec<u8> = Vec::new();
    let mut message: Vec<u8> = Vec::new();
    let mut message_text = false;
    let mut chunk = [0u8; 16 << 10];
    loop {
        // What is to be sent.
        loop {
            match out.try_recv() {
                Ok(Out::Text(d)) => {
                    if let Err(e) = write_frame(&mut stream, 0x1, &d) {
                        return Some(e);
                    }
                }
                Ok(Out::Binary(d)) => {
                    if let Err(e) = write_frame(&mut stream, 0x2, &d) {
                        return Some(e);
                    }
                }
                Ok(Out::Close) | Err(mpsc::TryRecvError::Disconnected) => {
                    let _ = write_frame(&mut stream, 0x8, &1000u16.to_be_bytes());
                    return None;
                }
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
        // What came in.
        match stream.read(&mut chunk) {
            Ok(0) => return Some("the server closed the connection".into()),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(e) => return Some(e.to_string()),
        }
        while let Some((fin, op, payload, used)) = match parse_frame(&buf) {
            Ok(f) => f,
            Err(e) => return Some(e),
        } {
            buf.drain(..used);
            match op {
                0x0..=0x2 => {
                    if op != 0 {
                        message.clear();
                        message_text = op == 0x1;
                    }
                    message.extend_from_slice(&payload);
                    if message.len() as u64 > MAX_BODY {
                        return Some("a message larger than 16 MiB".into());
                    }
                    if fin {
                        push(Event::Message(std::mem::take(&mut message), message_text));
                    }
                }
                0x8 => {
                    let _ = write_frame(&mut stream, 0x8, payload.get(..2).unwrap_or(&[]));
                    let reason = if payload.len() > 2 {
                        String::from_utf8_lossy(&payload[2..]).into_owned()
                    } else {
                        String::new()
                    };
                    return Some(if reason.is_empty() {
                        "the server closed the connection".into()
                    } else {
                        format!("the server closed the connection: {reason}")
                    });
                }
                0x9 => {
                    if let Err(e) = write_frame(&mut stream, 0xA, &payload) {
                        return Some(e);
                    }
                }
                _ => {}
            }
        }
    }
}

/// A frame from the server at the start of `buf`: (fin, opcode, payload,
/// bytes used), or None until all of it is there.
#[allow(clippy::type_complexity)]
fn parse_frame(buf: &[u8]) -> Result<Option<(bool, u8, Vec<u8>, usize)>, String> {
    if buf.len() < 2 {
        return Ok(None);
    }
    let fin = buf[0] & 0x80 != 0;
    let op = buf[0] & 0x0F;
    let masked = buf[1] & 0x80 != 0;
    let mut len = u64::from(buf[1] & 0x7F);
    let mut at = 2;
    if len == 126 {
        if buf.len() < 4 {
            return Ok(None);
        }
        len = u64::from(u16::from_be_bytes([buf[2], buf[3]]));
        at = 4;
    } else if len == 127 {
        if buf.len() < 10 {
            return Ok(None);
        }
        len = u64::from_be_bytes(buf[2..10].try_into().unwrap_or([0; 8]));
        at = 10;
    }
    if len > MAX_BODY {
        return Err("a message larger than 16 MiB".into());
    }
    let mask = if masked {
        if buf.len() < at + 4 {
            return Ok(None);
        }
        let m = [buf[at], buf[at + 1], buf[at + 2], buf[at + 3]];
        at += 4;
        Some(m)
    } else {
        None
    };
    let end = at + len as usize;
    if buf.len() < end {
        return Ok(None);
    }
    let mut payload = buf[at..end].to_vec();
    if let Some(m) = mask {
        for (i, b) in payload.iter_mut().enumerate() {
            *b ^= m[i % 4];
        }
    }
    Ok(Some((fin, op, payload, end)))
}

/// Writes one masked frame (clients mask theirs).
fn write_frame(stream: &mut Box<dyn Conn>, op: u8, payload: &[u8]) -> Result<(), String> {
    let mut f = Vec::with_capacity(payload.len() + 14);
    f.push(0x80 | op);
    match payload.len() {
        n if n < 126 => f.push(0x80 | n as u8),
        n if n <= 0xFFFF => {
            f.push(0x80 | 126);
            f.extend_from_slice(&(n as u16).to_be_bytes());
        }
        n => {
            f.push(0x80 | 127);
            f.extend_from_slice(&(n as u64).to_be_bytes());
        }
    }
    let mask = random16();
    let mask = [mask[0], mask[1], mask[2], mask[3]];
    f.extend_from_slice(&mask);
    f.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
    stream.write_all(&f).map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ helpers

/// Sixteen bytes nobody can guess in advance (the handshake's key, masks).
fn random16() -> [u8; 16] {
    use sha2::{Digest, Sha256};
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let stack = &n as *const u64 as usize;
    let h = Sha256::digest(format!("{t}:{n}:{stack}:{:?}", std::thread::current().id()).as_bytes());
    let mut out = [0u8; 16];
    out.copy_from_slice(&h[..16]);
    out
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n =
            (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn unbase64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes().filter(|c| *c != b'=') {
        let v = B64.iter().position(|b| *b == c)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}
