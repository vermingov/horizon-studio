// An opt-in Roblox account for the studio (hstudio_rbx_auth): the one thing
// Roblox won't give an anonymous client is a model's file, so when a script
// calls `game:GetObjects`/`InsertService:LoadLocalAsset` for a Roblox asset
// that isn't cached, the studio downloads it with the player's own
// `.ROBLOSECURITY` cookie, which they paste (or sign in for with Roblox's
// cross-device Quick Login) in Settings or the in-game prompt.
//
// The cookie is the account, so it is kept apart from everything scripts can
// touch: in the studio's own data folder (not the workspace readfile/listfiles
// reach), never in a script's environment, never added to a script's own
// `request`/HttpGet, and never forwarded to the CDN an asset download is
// redirected to. It only ever leaves for Roblox's own HTTPS hosts, built here
// from constants a script can't influence.

use std::path::{Path, PathBuf};

use super::hstudio_net::{self, Request, Response};

/// Roblox's hosts, as constants so a script can never steer where the cookie
/// goes. `Hosts` is the set the download/auth calls use; the real ones in
/// production, a local stand-in in tests.
#[derive(Clone, Debug)]
pub struct Hosts {
    /// Asset delivery (`/v1/asset/?id=`): the model's file, with the cookie.
    pub delivery: String,
    /// The account behind a cookie (`/v1/users/authenticated`).
    pub users: String,
    /// Quick Login (`/auth-token-service/v1/login/...`).
    pub tokens: String,
    /// The login that turns a validated Quick Login code into a cookie
    /// (`/v2/login`).
    pub auth: String,
    /// Addresses on this machine and its network allowed (tests only).
    pub local: bool,
}

impl Default for Hosts {
    fn default() -> Self {
        Hosts {
            delivery: "https://assetdelivery.roblox.com".into(),
            users: "https://users.roblox.com".into(),
            tokens: "https://apis.roblox.com".into(),
            auth: "https://auth.roblox.com".into(),
            local: false,
        }
    }
}

/// The hosts in force. Production uses `Hosts::default()`; a test points
/// them at a local stand-in with `set_hosts`.
pub fn hosts() -> Hosts {
    HOSTS_OVERRIDE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
        .unwrap_or_default()
}

/// Points the account calls at a stand-in server (tests only). `None`
/// restores Roblox's real hosts.
pub fn set_hosts(h: Option<Hosts>) {
    *HOSTS_OVERRIDE.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = h;
}

static HOSTS_OVERRIDE: std::sync::Mutex<Option<Hosts>> = std::sync::Mutex::new(None);

/// A cookie used for this session only ("Use once"), not written to disk.
static SESSION_COOKIE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Sets (or clears) the session-only cookie.
pub fn set_session_cookie(cookie: Option<String>) {
    *SESSION_COOKIE.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = cookie;
}

/// The cookie in force: the session-only one if set, otherwise the stored
/// one. This is what a download uses.
pub fn current_cookie(dir: &Path) -> Option<String> {
    SESSION_COOKIE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
        .or_else(|| load_cookie(dir))
}

/// Whether an account is in force (stored or for this session).
pub fn signed_in(dir: &Path) -> bool {
    current_cookie(dir).is_some()
}

/// A background account call, polled each frame (as a script's `request`
/// is), so the studio keeps drawing while it runs.
pub struct Task<T>(std::sync::mpsc::Receiver<T>);

impl<T> Task<T> {
    /// The result, once it's ready.
    pub fn poll(&self) -> Option<T> {
        self.0.try_recv().ok()
    }
}

/// Runs `f` on a thread of its own and hands back a `Task` to poll.
pub fn spawn<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Task<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = std::thread::Builder::new()
        .name("roblox-account".into())
        .spawn(move || {
            let _ = tx.send(f());
        });
    Task(rx)
}

/// The file the cookie is kept in, in the studio's data folder (beside
/// `workspace/`, so a script's sandboxed file API can't name it).
pub const COOKIE_FILE: &str = "roblox-account.txt";

fn cookie_path(dir: &Path) -> PathBuf {
    dir.join(COOKIE_FILE)
}

/// The stored cookie, if the player connected an account.
pub fn load_cookie(dir: &Path) -> Option<String> {
    let s = std::fs::read_to_string(cookie_path(dir)).ok()?;
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_owned())
}

/// Keeps the cookie (owner-only where the platform allows it). It is written
/// to the studio's folder, which the installer makes and only the player's
/// tools use; scripts never see this path.
pub fn store_cookie(dir: &Path, cookie: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = cookie_path(dir);
    std::fs::write(&path, cookie.trim())?;
    owner_only(&path);
    Ok(())
}

/// Forgets the stored cookie.
pub fn clear_cookie(dir: &Path) {
    let _ = std::fs::remove_file(cookie_path(dir));
}

/// Whether an account is connected.
pub fn connected(dir: &Path) -> bool {
    load_cookie(dir).is_some()
}

#[cfg(unix)]
fn owner_only(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn owner_only(_path: &Path) {}

/// A pasted cookie as it is stored: the raw value, or a `.ROBLOSECURITY=…`
/// line (as a browser's cookie editor shows it). The warning prefix Roblox
/// puts in front of the value is part of the cookie and is kept.
pub fn normalize_cookie(input: &str) -> String {
    let mut s = input.trim();
    // Cookie header or editor line: ".ROBLOSECURITY=value" (possibly after
    // "Cookie:").
    if let Some(rest) = s.to_ascii_lowercase().find(".roblosecurity=") {
        s = input[rest + ".roblosecurity=".len()..].trim_start();
    }
    // A single quoted value, or a trailing "; Domain=..." part.
    let s = s.trim_matches('"').trim_matches('\'');
    let s = s.split(';').next().unwrap_or(s).trim();
    s.to_owned()
}

/// Whether a string looks like a Roblox session cookie (so an obvious typo is
/// caught before a request). Not a guarantee it is valid.
pub fn looks_like_cookie(cookie: &str) -> bool {
    let c = cookie.trim();
    c.len() >= 32 && !c.contains(char::is_whitespace)
}

// ------------------------------------------------------------------ errors

/// Why an account call failed, with the message the player sees. The cookie
/// is never part of a message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthError {
    /// The cookie is wrong or expired (401/403): the account must be
    /// connected again.
    BadCredential,
    /// The asset doesn't exist, or the account can't see it (404).
    NotFound,
    /// Roblox is rate-limiting us (429): try again shortly.
    RateLimited,
    /// Another HTTP status.
    Status(u16, String),
    /// The request never got an answer (no network, DNS, TLS…).
    Network(String),
    /// Roblox's answer didn't parse.
    Parse(String),
}

impl AuthError {
    fn of_status(status: u16, msg: &str) -> AuthError {
        match status {
            401 | 403 => AuthError::BadCredential,
            404 => AuthError::NotFound,
            429 => AuthError::RateLimited,
            _ => AuthError::Status(status, msg.to_owned()),
        }
    }
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::BadCredential => write!(
                f,
                "Roblox rejected the account (the cookie is wrong or has expired): connect it again in Settings > Roblox account"
            ),
            AuthError::NotFound => write!(f, "the asset doesn't exist or the account can't see it (404)"),
            AuthError::RateLimited => write!(
                f,
                "Roblox is rate-limiting the studio (429): wait a moment and try again"
            ),
            AuthError::Status(s, m) if m.is_empty() => write!(f, "Roblox answered {s}"),
            AuthError::Status(s, m) => write!(f, "Roblox answered {s} ({m})"),
            AuthError::Network(e) => write!(f, "{e}"),
            AuthError::Parse(e) => write!(f, "{e}"),
        }
    }
}

// ------------------------------------------------------------------ requests

fn header<'a>(resp: &'a Response, name: &str) -> Option<&'a str> {
    resp.headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

/// A GET with the cookie set.
fn get_with_cookie(url: &str, cookie: &str, local: bool, redirects: u32) -> Result<Response, AuthError> {
    let req = Request {
        url: url.to_owned(),
        method: "GET".into(),
        headers: vec![("Cookie".into(), format!(".ROBLOSECURITY={cookie}"))],
        body: Vec::new(),
    };
    hstudio_net::fetch_redirects(&req, local, redirects).map_err(AuthError::Network)
}

/// A JSON POST with the cookie, retrying once with the CSRF token Roblox
/// hands back on a 403 (`x-csrf-token`). The body is JSON.
fn post_csrf(url: &str, cookie: Option<&str>, body: &[u8], local: bool) -> Result<Response, AuthError> {
    let mut headers = vec![("Content-Type".into(), "application/json".into())];
    if let Some(c) = cookie {
        headers.push(("Cookie".into(), format!(".ROBLOSECURITY={c}")));
    }
    let make = |extra: Option<&str>| {
        let mut h = headers.clone();
        if let Some(tok) = extra {
            h.push(("x-csrf-token".into(), tok.to_owned()));
        }
        Request {
            url: url.to_owned(),
            method: "POST".into(),
            headers: h,
            body: body.to_vec(),
        }
    };
    let first = hstudio_net::fetch_redirects(&make(None), local, 5).map_err(AuthError::Network)?;
    if first.status == 403
        && let Some(token) = header(&first, "x-csrf-token").map(str::to_owned)
    {
        return hstudio_net::fetch_redirects(&make(Some(&token)), local, 5).map_err(AuthError::Network);
    }
    Ok(first)
}

// ------------------------------------------------------------------ account

/// Who a cookie belongs to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Account {
    pub id: u64,
    pub name: String,
    pub display_name: String,
}

fn json_str(body: &str, key: &str) -> Option<String> {
    let at = body.find(&format!("\"{key}\""))?;
    let rest = &body[at + key.len() + 2..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

fn json_num(body: &str, key: &str) -> Option<u64> {
    let at = body.find(&format!("\"{key}\""))?;
    let rest = &body[at + key.len() + 2..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// The account a cookie signs in as, from `users/authenticated`. This is how
/// a pasted cookie is checked before it is stored.
pub fn authenticated(hosts: &Hosts, cookie: &str) -> Result<Account, AuthError> {
    let url = format!("{}/v1/users/authenticated", hosts.users);
    let resp = get_with_cookie(&url, cookie, hosts.local, 5)?;
    let body = String::from_utf8_lossy(&resp.body);
    if !resp.success() {
        return Err(AuthError::of_status(resp.status, body.trim()));
    }
    let id = json_num(&body, "id").ok_or_else(|| AuthError::Parse("Roblox's answer had no account id".into()))?;
    let name = json_str(&body, "name").unwrap_or_default();
    let display_name = json_str(&body, "displayName").unwrap_or_else(|| name.clone());
    Ok(Account { id, name, display_name })
}

// ------------------------------------------------------------------ download

/// A Roblox asset's file, downloaded with the cookie and cached by the
/// caller. The cookie goes only to the asset-delivery host; the CDN the
/// answer points at is fetched without it.
pub fn download_asset(hosts: &Hosts, id: u64, cookie: &str) -> Result<Vec<u8>, AuthError> {
    let url = format!("{}/v1/asset/?id={id}", hosts.delivery);
    // redirects(0): the CDN location comes back as a 3xx we follow ourselves,
    // so the cookie is never sent to the CDN.
    let resp = get_with_cookie(&url, cookie, hosts.local, 0)?;
    if (300..400).contains(&resp.status) {
        let location = header(&resp, "location")
            .ok_or_else(|| AuthError::Parse("Roblox redirected the download with no address".into()))?
            .to_owned();
        // No cookie, and no other header from the first request.
        let cdn = Request {
            url: location,
            method: "GET".into(),
            headers: Vec::new(),
            body: Vec::new(),
        };
        let got = hstudio_net::fetch_redirects(&cdn, hosts.local, 5).map_err(AuthError::Network)?;
        if !got.success() {
            return Err(AuthError::of_status(got.status, got.status_message.trim()));
        }
        return Ok(got.body);
    }
    if !resp.success() {
        let msg = String::from_utf8_lossy(&resp.body);
        return Err(AuthError::of_status(resp.status, msg.trim()));
    }
    Ok(resp.body)
}

/// A download running on a thread of its own, polled each frame (as a
/// script's `request` is), so the game keeps drawing while a script waits.
pub struct Download(std::sync::mpsc::Receiver<Result<Vec<u8>, AuthError>>);

impl Download {
    /// The bytes, once they're here.
    pub fn poll(&self) -> Option<Result<Vec<u8>, AuthError>> {
        match self.0.try_recv() {
            Ok(r) => Some(r),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                Some(Err(AuthError::Network("the download was lost".into())))
            }
        }
    }
}

/// Starts an asset download off the frame. The cookie is moved onto the
/// worker thread and dropped there; it never touches a script's data.
pub fn start_download(hosts: &Hosts, id: u64, cookie: &str) -> Download {
    let (tx, rx) = std::sync::mpsc::channel();
    let hosts = hosts.clone();
    let cookie = cookie.to_owned();
    let _ = std::thread::Builder::new().name("roblox-asset".into()).spawn(move || {
        let _ = tx.send(download_asset(&hosts, id, &cookie));
    });
    Download(rx)
}

// ------------------------------------------------------------------ quick login

/// A Quick Login code the player enters on a device already signed in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuickCode {
    pub code: String,
    pub private_key: String,
    /// Where the player enters the code.
    pub enter_at: String,
}

/// The place the player enters a Quick Login code.
pub const QUICK_LOGIN_URL: &str = "https://www.roblox.com/crossdevicelogin";

/// Starts Quick Login: Roblox makes a code the player types at
/// `crossdevicelogin` on a device that's already signed in.
pub fn quick_create(hosts: &Hosts) -> Result<QuickCode, AuthError> {
    let url = format!("{}/auth-token-service/v1/login/create", hosts.tokens);
    let resp = post_csrf(&url, None, b"{}", hosts.local)?;
    let body = String::from_utf8_lossy(&resp.body);
    if !resp.success() {
        return Err(AuthError::of_status(resp.status, body.trim()));
    }
    let code = json_str(&body, "code").ok_or_else(|| AuthError::Parse("Roblox gave no Quick Login code".into()))?;
    let private_key =
        json_str(&body, "privateKey").ok_or_else(|| AuthError::Parse("Roblox gave no Quick Login key".into()))?;
    Ok(QuickCode {
        code,
        private_key,
        enter_at: QUICK_LOGIN_URL.into(),
    })
}

/// Where a Quick Login code is in its life.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuickStatus {
    /// Waiting for the player to enter and approve the code.
    Waiting,
    /// Approved: the account's name, if Roblox gave it.
    Validated(Option<String>),
    /// The code expired or was cancelled.
    Cancelled,
}

/// Where a Quick Login code stands. Polled until it is validated.
pub fn quick_status(hosts: &Hosts, code: &QuickCode) -> Result<QuickStatus, AuthError> {
    let url = format!("{}/auth-token-service/v1/login/status", hosts.tokens);
    let body = format!(
        "{{\"code\":{},\"privateKey\":{}}}",
        json_quote(&code.code),
        json_quote(&code.private_key)
    );
    let resp = post_csrf(&url, None, body.as_bytes(), hosts.local)?;
    let text = String::from_utf8_lossy(&resp.body);
    if resp.status == 404 {
        return Ok(QuickStatus::Cancelled);
    }
    if !resp.success() {
        return Err(AuthError::of_status(resp.status, text.trim()));
    }
    match json_str(&text, "status").as_deref() {
        Some("Validated") => Ok(QuickStatus::Validated(json_str(&text, "accountName"))),
        Some("Cancelled") => Ok(QuickStatus::Cancelled),
        _ => Ok(QuickStatus::Waiting),
    }
}

/// Completes Quick Login once the code is validated: Roblox answers with the
/// session cookie in a `Set-Cookie` header, which becomes the stored
/// credential. Returns the `.ROBLOSECURITY` value.
pub fn quick_complete(hosts: &Hosts, code: &QuickCode) -> Result<String, AuthError> {
    let url = format!("{}/v2/login", hosts.auth);
    let body = format!(
        "{{\"ctype\":\"AuthToken\",\"cvalue\":{},\"password\":{}}}",
        json_quote(&code.code),
        json_quote(&code.private_key)
    );
    let resp = post_csrf(&url, None, body.as_bytes(), hosts.local)?;
    if !resp.success() {
        let msg = String::from_utf8_lossy(&resp.body);
        return Err(AuthError::of_status(resp.status, msg.trim()));
    }
    cookie_from_set_cookie(&resp).ok_or_else(|| {
        // Which cookies did come (their names only, never values), to tell
        // what Roblox did instead.
        let names: Vec<&str> = resp
            .headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case("set-cookie"))
            .filter_map(|(_, v)| v.split('=').next())
            .map(str::trim)
            .collect();
        let got = if names.is_empty() { "none".to_owned() } else { names.join(", ") };
        AuthError::Parse(format!(
            "Roblox signed in but sent no session cookie (cookies sent: {got}). Paste your .ROBLOSECURITY cookie instead."
        ))
    })
}

/// The `.ROBLOSECURITY` value out of a response's `Set-Cookie` headers.
pub fn cookie_from_set_cookie(resp: &Response) -> Option<String> {
    for (k, v) in &resp.headers {
        if !k.eq_ignore_ascii_case("set-cookie") {
            continue;
        }
        // Each Set-Cookie arrives on its own; its attributes (an Expires date
        // has a comma in it) follow the first ';'.
        if let Some(at) = v.to_ascii_lowercase().find(".roblosecurity=") {
            let rest = &v[at + ".roblosecurity=".len()..];
            let val = rest.split(';').next().unwrap_or(rest).trim();
            if !val.is_empty() {
                return Some(val.to_owned());
            }
        }
    }
    None
}

/// A JSON string literal (for the small request bodies above).
fn json_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_accepts_raw_and_line() {
        assert_eq!(normalize_cookie("  ABC123  "), "ABC123");
        assert_eq!(normalize_cookie(".ROBLOSECURITY=ABC123"), "ABC123");
        assert_eq!(
            normalize_cookie("Cookie: .ROBLOSECURITY=ABC123; Domain=.roblox.com"),
            "ABC123"
        );
        assert_eq!(
            normalize_cookie("\"_|WARNING:-DO-NOT-SHARE|_abcd\""),
            "_|WARNING:-DO-NOT-SHARE|_abcd"
        );
    }

    #[test]
    fn set_cookie_parsed() {
        let resp = Response {
            headers: vec![(
                "set-cookie".into(),
                ".ROBLOSECURITY=_|WARN|_tok; Domain=.roblox.com; HttpOnly".into(),
            )],
            ..Default::default()
        };
        assert_eq!(cookie_from_set_cookie(&resp).as_deref(), Some("_|WARN|_tok"));
    }

    #[test]
    fn status_maps() {
        assert_eq!(AuthError::of_status(401, ""), AuthError::BadCredential);
        assert_eq!(AuthError::of_status(403, ""), AuthError::BadCredential);
        assert_eq!(AuthError::of_status(404, ""), AuthError::NotFound);
        assert_eq!(AuthError::of_status(429, ""), AuthError::RateLimited);
    }
}
