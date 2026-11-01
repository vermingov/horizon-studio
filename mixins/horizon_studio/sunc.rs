// The executor library's host functions (hstudio_sunc): the sUNC functions
// that need the studio. The executor's and Roblox's environments (getgenv,
// getrenv, getsenv), a sandboxed file system in the studio's `workspace`
// folder, HTTP (`request`, `game:HttpGet`, HttpService) and WebSockets on
// hstudio_net (the internet only), the Drawing library drawn over the HUD,
// Instances (cloneref, gethui, getinstances, fireclickdetector...), signals
// (getconnections, firesignal), scripts (getscripts, getsenv,
// getscriptclosure...) and hidden properties. The built-ins that need only
// the interpreter are in hstudio_exec.

use super::hstudio::{Ctx, GameApi, Owner};
use super::hstudio_lua::{
    Lua, MAX_STRING, Native, Res, Table, TableRef, Throw, Userdata, Value, bad_arg, check_num, check_str, check_text,
    opt_num, opt_str,
};
use super::hstudio_rbx::{ConnRef, Env, Id, InstRef, Rbx, SignalRef, is_a};
use super::hstudio_rbx_types::{Color3, Vector2};
use super::hstudio_rbx_values::{Data, as_c3, as_v2, c3, get, v2};
use std::cell::RefCell;
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

pub const FIRST_ID: u32 = 2000;
pub const LAST_ID: u32 = 2199;

/// The globals and their host ids. Aliases share an id.
pub const FUNCTIONS: &[(&str, u32)] = &[
    ("writefile", 2000),
    ("readfile", 2001),
    ("appendfile", 2002),
    ("isfile", 2003),
    ("isfolder", 2004),
    ("makefolder", 2005),
    ("delfile", 2006),
    ("delfolder", 2007),
    ("listfiles", 2008),
    ("loadfile", 2009),
    ("getcustomasset", 2010),
    ("dofile", 2011),
    ("getgenv", 2020),
    ("getrenv", 2021),
    ("getsenv", 2022),
    ("request", 2030),
    ("http_request", 2030),
    ("cloneref", 2040),
    ("compareinstances", 2041),
    ("gethui", 2042),
    ("getinstances", 2043),
    ("getnilinstances", 2044),
    ("fireclickdetector", 2045),
    ("fireproximityprompt", 2046),
    ("firetouchinterest", 2047),
    ("getcallbackvalue", 2048),
    ("getconnections", 2050),
    ("firesignal", 2051),
    ("replicatesignal", 2052),
    ("getscripts", 2060),
    ("getrunningscripts", 2061),
    ("getloadedmodules", 2062),
    ("getcallingscript", 2063),
    ("getscriptclosure", 2064),
    ("getscriptfunction", 2064),
    ("getscriptbytecode", 2065),
    ("dumpstring", 2065),
    ("getscripthash", 2066),
    ("getscriptfromthread", 2067),
    ("gethiddenproperty", 2070),
    ("sethiddenproperty", 2071),
    ("isscriptable", 2072),
    ("setscriptable", 2073),
    ("cleardrawcache", 2081),
    ("getrenderproperty", 2082),
    ("setrenderproperty", 2083),
    ("isrenderobj", 2084),
    // UNC: the console (routed to the studio console).
    ("rconsoleprint", 2103),
    ("consoleprint", 2103),
    ("rconsoleclear", 2104),
    ("consoleclear", 2104),
    ("rconsolecreate", 2105),
    ("consolecreate", 2105),
    ("rconsoledestroy", 2106),
    ("consoledestroy", 2106),
    ("rconsoleinput", 2107),
    ("consoleinput", 2107),
    ("rconsolesettitle", 2108),
    ("rconsolename", 2108),
    ("consolesettitle", 2108),
    // UNC: input, routed to the Roblox layer's pointer.
    ("mouse1click", 2110),
    ("mouse1press", 2111),
    ("mouse1release", 2112),
    ("mouse2click", 2113),
    ("mouse2press", 2114),
    ("mouse2release", 2115),
    ("mousemoveabs", 2116),
    ("mousemoverel", 2117),
    ("mousescroll", 2118),
    ("isrbxactive", 2119),
    ("isgameactive", 2119),
    // UNC: clipboard and the rest.
    ("setclipboard", 2120),
    ("toclipboard", 2120),
    ("messagebox", 2121),
    ("setfpscap", 2122),
    ("queue_on_teleport", 2123),
    ("queueonteleport", 2123),
    ("setrbxclipboard", 2124),
    ("keypress", 2125),
    ("keyrelease", 2126),
    ("keyclick", 2129),
    ("getclipboard", 2127),
    ("getfpscap", 2128),
];

/// The game's name for the key a Windows virtual-key code is (keypress).
fn vk_key(vk: i64) -> Option<String> {
    let k = |s: &str| Some(format!("key.keyboard.{s}"));
    match vk {
        0x41..=0x5A => k(&((b'a' + (vk - 0x41) as u8) as char).to_string()),
        0x30..=0x39 => k(&((b'0' + (vk - 0x30) as u8) as char).to_string()),
        0x60..=0x69 => k(&format!("keypad.{}", vk - 0x60)),
        0x70..=0x7B => k(&format!("f{}", vk - 0x6F)),
        0x08 => k("backspace"),
        0x09 => k("tab"),
        0x0D => k("enter"),
        0x10 | 0xA0 => k("left.shift"),
        0xA1 => k("right.shift"),
        0x11 | 0xA2 => k("left.control"),
        0xA3 => k("right.control"),
        0x12 | 0xA4 => k("left.alt"),
        0xA5 => k("right.alt"),
        0x14 => k("caps.lock"),
        0x1B => k("escape"),
        0x20 => k("space"),
        0x21 => k("page.up"),
        0x22 => k("page.down"),
        0x23 => k("end"),
        0x24 => k("home"),
        0x25 => k("left"),
        0x26 => k("up"),
        0x27 => k("right"),
        0x28 => k("down"),
        0x2D => k("insert"),
        0x2E => k("delete"),
        0xBA => k("semicolon"),
        0xBB => k("equal"),
        0xBC => k("comma"),
        0xBD => k("minus"),
        0xBE => k("period"),
        0xBF => k("slash"),
        0xC0 => k("grave.accent"),
        0xDB => k("left.bracket"),
        0xDC => k("backslash"),
        0xDD => k("right.bracket"),
        0xDE => k("apostrophe"),
        _ => None,
    }
}

/// The `cache` table's host ids.
const CACHE: &[(&str, u32)] = &[("invalidate", 2100), ("iscached", 2101), ("replace", 2102)];

const WS_OPEN: u32 = 2031;
const CONN_FIRE: u32 = 2053;
const CONN_DEFER: u32 = 2054;
const CONN_DISCONNECT: u32 = 2055;
const CONN_DISABLE: u32 = 2056;
const CONN_ENABLE: u32 = 2057;
const DRAWING_NEW: u32 = 2080;
const DRAWING_INDEX: u32 = 2085;
const DRAWING_NEWINDEX: u32 = 2086;
const DRAWING_DESTROY: u32 = 2087;
const DRAWING_TOSTRING: u32 = 2088;

/// The largest file a script may write (16 MiB).
pub const MAX_FILE: usize = 16 << 20;

/// File extensions scripts may not write (sUNC's list).
const BLOCKED: &[&str] = &[
    "exe", "com", "scr", "pif", "cpl", "msc", "bat", "cmd", "ps1", "psd1", "vbs", "vbe", "js", "jse", "wsf", "wsh",
    "hta", "scf", "lnk", "zip", "rar", "7z", "cab", "iso", "img", "xml", "msi", "msp", "reg", "inf", "url",
];

/// The Lua that turns the host's answers into Roblox's methods and errors.
const PRELUDE: &str = r#"
local request, ws_open = ...
local function http(url, method, body, content_type, headers)
    local h = {}
    if type(headers) == "table" then
        for k, v in headers do h[k] = v end
    end
    if type(content_type) == "string" then h["Content-Type"] = content_type end
    local r = request({ Url = url, Method = method, Body = body, Headers = h })
    if r.StatusCode == 0 then error(r.StatusMessage, 3) end
    return r
end
local function checked(r)
    if not r.Success then error(`HTTP {r.StatusCode} ({r.StatusMessage})`, 3) end
    return r.Body
end
local M = {}
function M.HttpGet(self, url) return http(url, "GET").Body end
function M.HttpGetAsync(self, url) return http(url, "GET").Body end
function M.HttpPost(self, url, data, content_type) return http(url, "POST", data, content_type).Body end
function M.HttpPostAsync(self, url, data, content_type) return http(url, "POST", data, content_type).Body end
function M.GetAsync(self, url, _, headers) return checked(http(url, "GET", nil, nil, headers)) end
function M.PostAsync(self, url, data, content_type, _, headers)
    return checked(http(url, "POST", data, content_type, headers))
end
function M.RequestAsync(self, options)
    local r = request(options)
    if r.StatusCode == 0 then error(r.StatusMessage, 2) end
    return r
end
-- As in Roblox, Invoke before OnInvoke is set waits until it is.
function M.Invoke(self, ...)
    while type(self.OnInvoke) ~= "function" do
        task.wait()
    end
    return self.OnInvoke(...)
end
local function connect(url)
    local ws, err = ws_open(url)
    if not ws then error(err, 2) end
    return ws
end
return M, connect
"#;

// ---------------------------------------------------------------- environments

/// The executor's and Roblox's globals: built once, shared by every mod.
/// `renv` holds Roblox's (the standard library, game, Instance, the data
/// types, task); `genv` the executor's and the studio's API, falling back
/// to `renv`; each mod's own globals fall back to `genv`.
pub fn executor_globals(lua: &mut Lua, genv: &TableRef, rbx: &mut Rbx) {
    let mut g = genv.borrow_mut();
    for (name, v) in super::hstudio_exec::globals() {
        g.set_str(name, v);
    }
    for (name, id) in FUNCTIONS {
        let f = Native::host(name, *id);
        super::hstudio_exec::mark(&f);
        g.set_str(name, f);
    }
    let request = g.get_str("request");
    let mut http = Table::new();
    http.set_str("request", request.clone());
    g.set_str("http", Value::table(http));
    // Drawing.
    let mut d = Table::new();
    let new = Native::host("new", DRAWING_NEW);
    super::hstudio_exec::mark(&new);
    d.set_str("new", new);
    d.set_str("clear", g.get_str("cleardrawcache"));
    let mut fonts = Table::new();
    for (i, f) in ["UI", "System", "Plex", "Monospace"].iter().enumerate() {
        fonts.set_str(f, Value::Num(i as f64));
    }
    d.set_str("Fonts", Value::table(fonts));
    g.set_str("Drawing", Value::table(d));
    // cache: the Instance registry's refs.
    let mut cache = Table::new();
    for (name, id) in CACHE {
        let f = Native::host(name, *id);
        super::hstudio_exec::mark(&f);
        cache.set_str(name, f);
    }
    g.set_str("cache", Value::table(cache));
    // The prelude: HttpGet and friends, and WebSocket.connect.
    let ws_open = Native::host("connect", WS_OPEN);
    drop(g);
    let f = lua.load(PRELUDE, "=[executor]", genv).expect("the prelude compiles");
    let r = lua
        .call(&mut super::hstudio_lua::NoHost::default(), &f, vec![request, ws_open])
        .unwrap_or_default();
    if let (Some(Value::Table(m)), Some(connect)) = (r.first(), r.get(1)) {
        for (class, name) in [
            ("DataModel", "HttpGet"),
            ("DataModel", "HttpGetAsync"),
            ("DataModel", "HttpPost"),
            ("DataModel", "HttpPostAsync"),
            ("HttpService", "GetAsync"),
            ("HttpService", "PostAsync"),
            ("HttpService", "RequestAsync"),
            ("BindableFunction", "Invoke"),
        ] {
            rbx.lua_methods.push((class, name, m.borrow().get_str(name)));
        }
        let mut ws = Table::new();
        ws.set_str("connect", connect.clone());
        ws.set_str("Connect", connect.clone());
        genv.borrow_mut().set_str("WebSocket", Value::table(ws));
    }
    aliases(genv);
}

/// Luau's vector library, in Roblox's environment `renv` (it has Vector3
/// and math): in Roblox a vector is a Vector3.
pub fn vector_lib(lua: &mut Lua, renv: &TableRef) {
    if let Ok(f) = lua.load(VECTOR, "=[vector]", renv) {
        let _ = lua.call(&mut super::hstudio_lua::NoHost::default(), &f, Vec::new());
    }
}

/// Luau's vector library over Roblox's Vector3.
const VECTOR: &str = r#"
local V3 = Vector3
local floor, ceil, abs, max, min, sign = math.floor, math.ceil, math.abs, math.max, math.min, math.sign
local function each(v, f) return V3.new(f(v.X), f(v.Y), f(v.Z)) end
vector = table.freeze({
    create = function(x, y, z) return V3.new(x, y, z or 0) end,
    magnitude = function(v) return v.Magnitude end,
    normalize = function(v) return v.Unit end,
    dot = function(a, b) return a:Dot(b) end,
    cross = function(a, b) return a:Cross(b) end,
    angle = function(a, b, axis)
        local angle = math.atan2(a:Cross(b).Magnitude, a:Dot(b))
        if axis and a:Cross(b):Dot(axis) < 0 then return -angle end
        return angle
    end,
    floor = function(v) return each(v, floor) end,
    ceil = function(v) return each(v, ceil) end,
    abs = function(v) return each(v, abs) end,
    sign = function(v) return each(v, sign) end,
    clamp = function(v, lo, hi)
        return V3.new(math.clamp(v.X, lo.X, hi.X), math.clamp(v.Y, lo.Y, hi.Y), math.clamp(v.Z, lo.Z, hi.Z))
    end,
    max = function(v, ...)
        for _, w in {...} do v = V3.new(max(v.X, w.X), max(v.Y, w.Y), max(v.Z, w.Z)) end
        return v
    end,
    min = function(v, ...)
        for _, w in {...} do v = V3.new(min(v.X, w.X), min(v.Y, w.Y), min(v.Z, w.Z)) end
        return v
    end,
    zero = V3.zero,
    one = V3.one,
})
"#;

/// Other executors' names for the same functions, and Synapse X's `syn`
/// table, which scripts made for it call.
fn aliases(genv: &TableRef) {
    let mut g = genv.borrow_mut();
    for (alias, name) in [
        ("getsynasset", "getcustomasset"),
        ("iswindowactive", "isrbxactive"),
        ("printconsole", "rconsoleprint"),
        ("openfile", "readfile"),
    ] {
        let f = g.get_str(name);
        if !f.is_nil() {
            g.set_str(alias, f);
        }
    }
    let mut syn = Table::new();
    for (key, name) in [
        ("request", "request"),
        ("queue_on_teleport", "queue_on_teleport"),
        ("write_clipboard", "setclipboard"),
        ("set_thread_identity", "setthreadidentity"),
        ("get_thread_identity", "getthreadidentity"),
    ] {
        syn.set_str(key, g.get_str(name));
    }
    // protect_gui and unprotect_gui hide a GUI from the game's scripts in
    // Synapse: here nothing looks, so they do nothing.
    let nothing = Native::host("protect_gui", 2105);
    super::hstudio_exec::mark(&nothing);
    syn.set_str("protect_gui", nothing.clone());
    syn.set_str("unprotect_gui", nothing);
    syn.set_str("crypt", g.get_str("crypt"));
    syn.set_str("websocket", g.get_str("WebSocket"));
    g.set_str("syn", Value::table(syn));
}

// ---------------------------------------------------------------- files

/// A script's path inside the workspace folder: relative, no `..`.
fn sandboxed(lua: &Lua, root: &Path, path: &str, fname: &str) -> Result<PathBuf, Throw> {
    let path = path.replace('\\', "/");
    let rel = Path::new(path.trim_start_matches("./"));
    if rel.is_absolute() || path.contains(':') {
        return Err(lua.rt_error(format!("{fname}: '{path}' is outside the workspace folder")));
    }
    let mut out = root.to_path_buf();
    for c in rel.components() {
        match c {
            Component::Normal(p) => out.push(p),
            Component::CurDir => {}
            _ => return Err(lua.rt_error(format!("{fname}: '{path}' is outside the workspace folder"))),
        }
    }
    Ok(out)
}

fn writable(lua: &Lua, path: &Path, fname: &str) -> Result<(), Throw> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if BLOCKED.contains(&ext.as_str()) {
        return Err(lua.rt_error(format!("{fname}: .{ext} files can't be written")));
    }
    Ok(())
}

fn io_err(lua: &Lua, fname: &str, e: std::io::Error) -> Throw {
    lua.rt_error(format!("{fname}: {e}"))
}

/// A stable value for the identifying headers `request` adds, made from the
/// studio's folder (not the machine).
fn fingerprint(dir: &Path) -> String {
    let h = super::hstudio_crypt::sha256(dir.to_string_lossy().as_bytes());
    super::hstudio_crypt::hex(&h[..16])
}

// ---------------------------------------------------------------- HTTP

/// `request`'s options as a request.
fn request_of(lua: &Lua, t: &TableRef, dir: &Path) -> Result<super::hstudio_net::Request, Throw> {
    let t = t.borrow();
    let url = t
        .get_str("Url")
        .as_text()
        .ok_or_else(|| lua.rt_error("request: Url expected"))?;
    let method = t
        .get_str("Method")
        .as_text()
        .unwrap_or_else(|| "GET".into())
        .to_ascii_uppercase();
    if !["GET", "HEAD", "POST", "PUT", "DELETE", "OPTIONS", "PATCH"].contains(&method.as_str()) {
        return Err(lua.rt_error(format!("request: unknown method '{method}'")));
    }
    let mut headers: Vec<(String, String)> = Vec::new();
    if let Value::Table(h) = t.get_str("Headers") {
        for (k, v) in h.borrow().entries() {
            headers.push((k.plain_text(), v.plain_text()));
        }
    }
    if let Value::Table(c) = t.get_str("Cookies") {
        let cookie: Vec<String> = c
            .borrow()
            .entries()
            .iter()
            .map(|(k, v)| format!("{}={}", k.plain_text(), v.plain_text()))
            .collect();
        if !cookie.is_empty() {
            headers.push(("Cookie".into(), cookie.join("; ")));
        }
    }
    let id = fingerprint(dir);
    headers.push(("Horizon-Studio-User-Identifier".into(), id.clone()));
    headers.push(("Horizon-Studio-Fingerprint".into(), id));
    let body = t.get_str("Body").as_bytes().map(<[u8]>::to_vec).unwrap_or_default();
    Ok(super::hstudio_net::Request {
        url,
        method,
        headers,
        body,
    })
}

/// A response as `request` gives it: Success, StatusCode, StatusMessage,
/// Headers and Body; a request that never got an answer has StatusCode 0
/// and the reason in StatusMessage.
pub fn response_value(r: Result<super::hstudio_net::Response, String>) -> Value {
    let mut t = Table::new();
    match r {
        Ok(r) => {
            t.set_str("Success", Value::Bool(r.success()));
            t.set_str("StatusCode", Value::Num(f64::from(r.status)));
            t.set_str("StatusMessage", Value::str(&r.status_message));
            let mut h = Table::new();
            for (k, v) in &r.headers {
                h.set_str(k, Value::str(v));
            }
            t.set_str("Headers", Value::table(h));
            t.set_str("Body", Value::bytes(&r.body));
        }
        Err(e) => {
            t.set_str("Success", Value::Bool(false));
            t.set_str("StatusCode", Value::Num(0.0));
            t.set_str("StatusMessage", Value::str(&e));
            t.set_str("Headers", Value::table(Table::new()));
            t.set_str("Body", Value::str(""));
        }
    }
    Value::table(t)
}

/// Answers and socket events since the last frame, to the threads and
/// signals waiting for them.
pub fn poll_network(rbx: &mut Rbx) {
    let mut i = 0;
    while i < rbx.http.len() {
        match rbx.http[i].pending.poll() {
            Some(r) => {
                let w = rbx.http.remove(i);
                rbx.ready.push((w.owner, w.thread, vec![response_value(r)]));
            }
            None => i += 1,
        }
    }
    let mut i = 0;
    while i < rbx.sockets.len() {
        let events = rbx.sockets[i].socket.events();
        let id = rbx.sockets[i].id;
        let mut closed = false;
        for ev in events {
            match ev {
                super::hstudio_net::Event::Open => {
                    if let Some(t) = rbx.sockets[i].opening.take() {
                        let owner = rbx.sockets[i].owner;
                        let v = rbx.value(id);
                        rbx.ready.push((owner, t, vec![v]));
                    }
                }
                super::hstudio_net::Event::Message(bytes, _) => {
                    rbx.fire(id, "OnMessage", vec![Value::bytes(&bytes)]);
                }
                super::hstudio_net::Event::Closed(why) => {
                    match rbx.sockets[i].opening.take() {
                        Some(t) => {
                            let owner = rbx.sockets[i].owner;
                            let why = why.unwrap_or_else(|| "the connection closed".into());
                            rbx.ready.push((owner, t, vec![Value::Nil, Value::str(&why)]));
                        }
                        None => rbx.fire(id, "OnClose", Vec::new()),
                    }
                    closed = true;
                    break;
                }
            }
        }
        if closed {
            rbx.sockets.remove(i);
        } else {
            i += 1;
        }
    }
}

/// WebSocket:Send and WebSocket:Close.
pub fn socket_method(rbx: &mut Rbx, id: Id, name: &str, data: Option<Vec<u8>>) {
    let Some(i) = rbx.sockets.iter().position(|s| s.id == id) else {
        return;
    };
    match name {
        "Send" => {
            let d = data.unwrap_or_default();
            let text = std::str::from_utf8(&d).is_ok();
            rbx.sockets[i].socket.send(&d, text);
        }
        _ => {
            let s = rbx.sockets.remove(i);
            s.socket.close();
            rbx.fire(id, "OnClose", Vec::new());
        }
    }
}

// ---------------------------------------------------------------- Drawing

/// A Drawing object.
pub struct Drawing {
    pub id: u32,
    pub kind: &'static str,
    pub owner: Owner,
    pub props: Vec<(&'static str, Value)>,
}

/// What a Drawing userdata holds.
#[derive(Clone, Copy)]
pub struct DrawRef(pub u32);

const KINDS: &[&str] = &["Line", "Text", "Image", "Circle", "Square", "Quad", "Triangle"];

/// A kind's properties and their defaults (after the shared ones).
fn defaults(kind: &str) -> Vec<(&'static str, Value)> {
    let zero = || v2(Vector2::ZERO);
    let mut p: Vec<(&'static str, Value)> = vec![
        ("Visible", Value::Bool(false)),
        ("ZIndex", Value::Num(1.0)),
        ("Transparency", Value::Num(0.0)),
        ("Color", c3(Color3::new(1.0, 1.0, 1.0))),
    ];
    let more: Vec<(&'static str, Value)> = match kind {
        "Line" => vec![("From", zero()), ("To", zero()), ("Thickness", Value::Num(1.0))],
        "Text" => vec![
            ("Text", Value::str("")),
            ("Font", Value::Num(0.0)),
            ("Size", Value::Num(16.0)),
            ("Position", zero()),
            ("Center", Value::Bool(false)),
            ("Outline", Value::Bool(false)),
            ("OutlineColor", c3(Color3::new(0.0, 0.0, 0.0))),
        ],
        "Image" => vec![
            ("Data", Value::str("")),
            ("Size", zero()),
            ("Position", zero()),
            ("Rounding", Value::Num(0.0)),
        ],
        "Circle" => vec![
            ("NumSides", Value::Num(64.0)),
            ("Radius", Value::Num(0.0)),
            ("Position", zero()),
            ("Thickness", Value::Num(1.0)),
            ("Filled", Value::Bool(false)),
        ],
        "Square" => vec![
            ("Size", zero()),
            ("Position", zero()),
            ("Thickness", Value::Num(1.0)),
            ("Filled", Value::Bool(false)),
        ],
        "Quad" => vec![
            ("PointA", zero()),
            ("PointB", zero()),
            ("PointC", zero()),
            ("PointD", zero()),
            ("Thickness", Value::Num(1.0)),
            ("Filled", Value::Bool(false)),
        ],
        _ => vec![
            ("PointA", zero()),
            ("PointB", zero()),
            ("PointC", zero()),
            ("Thickness", Value::Num(1.0)),
            ("Filled", Value::Bool(false)),
        ],
    };
    p.extend(more);
    p
}

thread_local! {
    static DRAWING_META: TableRef = {
        let mut t = Table::new();
        t.set_type_name("Drawing");
        t.set_str("__index", Native::host("__index", DRAWING_INDEX));
        t.set_str("__newindex", Native::host("__newindex", DRAWING_NEWINDEX));
        t.set_str("__tostring", Native::host("__tostring", DRAWING_TOSTRING));
        Rc::new(RefCell::new(t))
    };
}

fn draw_ref(v: &Value) -> Option<u32> {
    match v {
        Value::Userdata(u) => u.data.borrow().downcast_ref::<DrawRef>().map(|d| d.0),
        _ => None,
    }
}

fn prop_get(d: &Drawing, k: &str) -> Value {
    d.props
        .iter()
        .find(|p| p.0 == k)
        .map(|p| p.1.clone())
        .unwrap_or_default()
}

fn num(d: &Drawing, k: &str) -> f64 {
    prop_get(d, k).to_num().unwrap_or(0.0)
}

fn vec2(d: &Drawing, k: &str) -> Vector2 {
    as_v2(&prop_get(d, k)).unwrap_or_default()
}

fn font(d: &Drawing) -> super::hstudio::Font {
    match num(d, "Font") as i64 {
        2 => super::hstudio::Font::Medium,
        3 => super::hstudio::Font::Mono,
        _ => super::hstudio::Font::Regular,
    }
}

fn argb(c: Color3, alpha: f64) -> u32 {
    let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
    let ch = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u32;
    a << 24 | ch(c.r) << 16 | ch(c.g) << 8 | ch(c.b)
}

/// Every visible Drawing, lowest ZIndex first, over the HUD (in dp).
pub fn draw_drawings(rbx: &Rbx, game: &mut dyn GameApi, textures: &mut ImageCache) {
    if rbx.drawings.is_empty() {
        return;
    }
    textures.begin(rbx.now);
    let (dw, _, _) = game.dp();
    let (gw, _) = game.gui_size();
    // game.quad takes GUI pixels; Drawings are in dp.
    let k = if dw > 0.0 { gw / dw } else { 1.0 };
    let mut order: Vec<&Drawing> = rbx
        .drawings
        .iter()
        .filter(|d| prop_get(d, "Visible").truthy())
        .collect();
    order.sort_by(|a, b| num(a, "ZIndex").total_cmp(&num(b, "ZIndex")));
    for d in order {
        let alpha = 1.0 - num(d, "Transparency");
        if alpha <= 0.0 {
            continue;
        }
        let col = argb(as_c3(&prop_get(d, "Color")).unwrap_or_default(), alpha);
        let thick = num(d, "Thickness").max(0.5) as f32;
        let filled = prop_get(d, "Filled").truthy();
        let mut line = |game: &mut dyn GameApi, a: Vector2, b: Vector2, w: f32| {
            let (dx, dy) = ((b.x - a.x) as f32, (b.y - a.y) as f32);
            let len = (dx * dx + dy * dy).sqrt();
            if len < 0.01 {
                return;
            }
            let (nx, ny) = (-dy / len * w / 2.0, dx / len * w / 2.0);
            let (ax, ay, bx, by) = (a.x as f32, a.y as f32, b.x as f32, b.y as f32);
            game.quad(
                [
                    [(ax - nx) * k, (ay - ny) * k],
                    [(ax + nx) * k, (ay + ny) * k],
                    [(bx + nx) * k, (by + ny) * k],
                    [(bx - nx) * k, (by - ny) * k],
                ],
                col,
            );
        };
        let poly =
            |game: &mut dyn GameApi, pts: &[Vector2], line: &mut dyn FnMut(&mut dyn GameApi, Vector2, Vector2, f32)| {
                if filled {
                    for w in 1..pts.len().saturating_sub(1) {
                        let (a, b, c) = (pts[0], pts[w], pts[w + 1]);
                        game.quad(
                            [
                                [a.x as f32 * k, a.y as f32 * k],
                                [b.x as f32 * k, b.y as f32 * k],
                                [c.x as f32 * k, c.y as f32 * k],
                                [c.x as f32 * k, c.y as f32 * k],
                            ],
                            col,
                        );
                    }
                } else {
                    for i in 0..pts.len() {
                        line(game, pts[i], pts[(i + 1) % pts.len()], thick);
                    }
                }
            };
        match d.kind {
            "Line" => line(game, vec2(d, "From"), vec2(d, "To"), thick),
            "Square" => {
                let (p, s) = (vec2(d, "Position"), vec2(d, "Size"));
                if filled {
                    game.dp_rect([p.x as f32, p.y as f32, s.x as f32, s.y as f32], 0.0, col);
                } else {
                    let c = [p, Vector2::new(p.x + s.x, p.y), p + s, Vector2::new(p.x, p.y + s.y)];
                    poly(game, &c, &mut line);
                }
            }
            "Circle" => {
                let (c, r) = (vec2(d, "Position"), num(d, "Radius"));
                let n = num(d, "NumSides").clamp(3.0, 256.0) as usize;
                let pts: Vec<Vector2> = (0..n)
                    .map(|i| {
                        let a = std::f64::consts::TAU * i as f64 / n as f64;
                        Vector2::new(c.x + a.cos() * r, c.y + a.sin() * r)
                    })
                    .collect();
                poly(game, &pts, &mut line);
            }
            "Quad" => {
                let pts = ["PointA", "PointB", "PointC", "PointD"].map(|p| vec2(d, p));
                poly(game, &pts, &mut line);
            }
            "Triangle" => {
                let pts = ["PointA", "PointB", "PointC"].map(|p| vec2(d, p));
                poly(game, &pts, &mut line);
            }
            "Text" => {
                let text = prop_get(d, "Text").as_text().unwrap_or_default();
                let size = num(d, "Size").max(1.0) as f32;
                let f = font(d);
                let mut p = vec2(d, "Position");
                if prop_get(d, "Center").truthy() {
                    p.x -= f64::from(game.dp_measure(&text, size, f)) / 2.0;
                }
                let (x, y) = (p.x as f32, p.y as f32 - size * 0.225);
                if prop_get(d, "Outline").truthy() {
                    let oc = argb(as_c3(&prop_get(d, "OutlineColor")).unwrap_or_default(), alpha);
                    for (ox, oy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
                        game.dp_text(&text, [x + ox, y + oy], size, f, oc);
                    }
                }
                game.dp_text(&text, [x, y], size, f, col);
            }
            _ => {
                let data = prop_get(d, "Data");
                let (p, s) = (vec2(d, "Position"), vec2(d, "Size"));
                if let Some(bytes) = data.as_bytes() {
                    textures.draw(game, bytes, [p.x as f32, p.y as f32, s.x as f32, s.y as f32], alpha);
                }
            }
        }
    }
}

/// What the image cache keeps: each entry with the frame (the studio's
/// clock) it was last drawn in, the least recently drawn first (`keep`).
type Kept<T> = Vec<(f64, T)>;

/// Decoded images for Drawing's Image and the GUI's images (getcustomasset's
/// files), shared by the HUD and the GUI's screen: drawn as textures where
/// the game draws images, else as blocks of colour.
#[derive(Default)]
pub struct ImageCache {
    /// Images decoded, and workspace files read with the key in `items` of
    /// the image they hold.
    items: Kept<(u64, Rc<super::hstudio_meshes::Texture>)>,
    files: Kept<(PathBuf, Rc<Vec<u8>>, u64)>,
    /// The key each image had in the game's atlas (its place there), by its
    /// key in `items`: decoded again, it takes the same one, so the atlas
    /// finds it where it is rather than placing it anew.
    places: std::collections::HashMap<u64, u32>,
    /// Large images scaled down for the game's atlas, by the image's key,
    /// which a copy takes (the image itself never goes there): made again,
    /// it is found where the last was put.
    small: Kept<(u32, Rc<super::hstudio_meshes::Texture>)>,
    /// This frame (the studio's clock), and the images it put in the
    /// game's atlas (by key) with their pixels in all.
    frame: f64,
    in_atlas: Vec<u32>,
    texels: usize,
    /// How many times image bytes were hashed to find their image (once a
    /// file, and once a frame for each Drawing's image), and decoded.
    pub hashed: u64,
    pub decoded: u64,
    /// Roblox images being fetched (by their file in the workspace's
    /// `.roblox-assets`), and those that couldn't be.
    fetching: std::collections::HashMap<String, std::sync::mpsc::Receiver<bool>>,
    failed: std::collections::HashSet<String>,
}

/// Where a Roblox image's thumbnail is asked for, for an image id
/// (`rbxassetid://123`, `http://www.roblox.com/asset/?id=123`, `123`) or a
/// `rbxthumb://type=...&id=...&w=...&h=...`: the thumbnails API's address,
/// and the file it is kept in under the workspace. Roblox's own asset
/// downloads need a signed-in account; its thumbnails don't, and an image
/// asset's thumbnail is the image (at most 700 pixels a side).
pub fn roblox_image(content: &str) -> Option<(String, String)> {
    let s = content.trim();
    let digits = |t: &str| -> Option<u64> {
        let d: String = t.chars().take_while(char::is_ascii_digit).collect();
        d.parse().ok().filter(|_| !d.is_empty())
    };
    let asset = |id: u64| {
        Some((
            format!("https://thumbnails.roblox.com/v1/assets?assetIds={id}&size=700x700&format=Png"),
            format!(".roblox-assets/{id}.png"),
        ))
    };
    if let Some(rest) = s.strip_prefix("rbxthumb://") {
        let field = |k: &str| {
            rest.split('&').find_map(|kv| {
                kv.split_once('=')
                    .filter(|(a, _)| a.eq_ignore_ascii_case(k))
                    .map(|(_, v)| v)
            })
        };
        let id = digits(field("id")?)?;
        let w = field("w").and_then(digits).unwrap_or(150);
        let kind = field("type").unwrap_or("Asset");
        let (path, size) = match kind {
            "AvatarHeadShot" => ("users/avatar-headshot", [48, 60, 150, 352, 420, 720]),
            "AvatarBust" => ("users/avatar-bust", [48, 60, 75, 100, 150, 352]),
            "Avatar" => ("users/avatar", [48, 60, 100, 150, 352, 720]),
            _ => return asset(id),
        };
        let size = size.iter().copied().find(|&n| n >= w).unwrap_or(size[5]);
        let ids = "userIds";
        return Some((
            format!("https://thumbnails.roblox.com/v1/{path}?{ids}={id}&size={size}x{size}&format=Png"),
            format!(".roblox-assets/{kind}-{id}-{size}.png"),
        ));
    }
    let lower = s.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("rbxassetid://") {
        return asset(digits(rest)?);
    }
    if lower.contains("roblox.com/asset") {
        let at = lower.find("id=")?;
        return asset(digits(&lower[at + 3..])?);
    }
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) {
        return asset(digits(s)?);
    }
    None
}

/// Fetches a Roblox image's thumbnail (the thumbnails API's answer names
/// where its PNG is) into `path`: whether it is there.
fn fetch_roblox_image(api: &str, path: &Path) -> bool {
    let get = |url: &str| {
        super::hstudio_net::fetch(
            &super::hstudio_net::Request {
                url: url.to_owned(),
                method: "GET".into(),
                headers: Vec::new(),
                body: Vec::new(),
            },
            false,
        )
        .ok()
        .filter(super::hstudio_net::Response::success)
    };
    // A thumbnail not made yet is Pending a moment.
    for attempt in 0..4 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
        let Some(answer) = get(api) else { continue };
        let Ok(j) = serde_json::from_slice::<serde_json::Value>(&answer.body) else {
            return false;
        };
        let item = &j["data"][0];
        match (item["state"].as_str(), item["imageUrl"].as_str()) {
            (Some("Completed"), Some(url)) => {
                let Some(png) = get(url) else { return false };
                let tmp = path.with_extension("part");
                let _ = std::fs::create_dir_all(path.parent().unwrap_or(path));
                return std::fs::write(&tmp, &png.body).is_ok() && std::fs::rename(&tmp, path).is_ok();
            }
            (Some("Pending"), _) => continue,
            _ => return false,
        }
    }
    false
}

/// Image cells a side at most: an image draws as up to 48 x 48 blocks.
const CELLS: u32 = 48;
/// A block is at least this many dp a side, so a small or tiled image
/// draws in few blocks.
const CELL_DP: f32 = 4.0;
/// An image's longest side in the game's atlas: a larger one goes there
/// scaled down.
const ATLAS_SIDE: u32 = 256;
/// The pixels a frame's images take in the game's atlas at most, which
/// they share with the interface's glyphs; an image past them draws in
/// blocks.
const ATLAS_TEXELS: usize = 1 << 18;
/// The images kept decoded and the workspace files kept read: as many as a
/// big GUI draws in a frame (a shop's or an inventory's icons), within this
/// many pixels and bytes in all (32 images of the largest, 512 a side, as
/// many as fit 32 MiB). The least recently drawn go first, never one the
/// frame has drawn (see `keep`).
const KEPT: usize = 256;
const KEPT_TEXELS: usize = 1 << 23;
const KEPT_BYTES: usize = 32 << 20;
/// The images whose place in the atlas is remembered.
const PLACES: usize = 4096;
/// The copies of large images kept scaled down for the atlas: a frame's
/// take at most its share of the atlas, and these hold four frames' worth.
const SMALL_TEXELS: usize = 4 * ATLAS_TEXELS;

/// Moves entry `i` of `kept` to its end (the most recently drawn), drawn
/// in `frame`, and returns it.
fn touch<T>(kept: &mut Kept<T>, i: usize, frame: f64) -> &T {
    let mut e = kept.remove(i);
    e.0 = frame;
    kept.push(e);
    &kept[kept.len() - 1].1
}

/// Keeps `kept` within KEPT entries and `budget` of `size` in all, its
/// newest entry just put in. The least recently drawn go first, but never
/// one `frame` drew: the GUI asks for its images in the same order every
/// frame, and the oldest would be the next it asks for. Over the limits
/// with only this frame's entries left, the newest goes (it is made again
/// each time it is drawn), so a frame that draws more than the limits hold
/// keeps the first it drew, frame after frame, rather than none.
fn keep<T>(kept: &mut Kept<T>, frame: f64, budget: usize, size: impl Fn(&T) -> usize) {
    let mut total: usize = kept.iter().map(|(_, e)| size(e)).sum();
    while kept.len() > 1 && (kept.len() > KEPT || total > budget) {
        match kept.iter().position(|(drawn, _)| *drawn != frame) {
            Some(i) => total -= size(&kept.remove(i).1),
            None => {
                kept.pop();
                break;
            }
        }
    }
}

impl ImageCache {
    /// A workspace file's bytes (getcustomasset's images), read once.
    pub fn file(&mut self, workspace: &Path, rel: &str) -> Option<Rc<Vec<u8>>> {
        self.read(workspace, rel).map(|(bytes, _)| bytes)
    }

    /// A workspace file's bytes and their image's key, read and hashed once.
    fn read(&mut self, workspace: &Path, rel: &str) -> Option<(Rc<Vec<u8>>, u64)> {
        let p = workspace.join(rel);
        if !p.starts_with(workspace) || rel.contains("..") {
            return None;
        }
        if let Some(i) = self.files.iter().position(|(_, (q, _, _))| *q == p) {
            let (_, b, key) = touch(&mut self.files, i, self.frame);
            return Some((b.clone(), *key));
        }
        let bytes = Rc::new(std::fs::read(&p).ok().filter(|b| b.len() <= MAX_FILE)?);
        let key = self.key(&bytes);
        self.files.push((self.frame, (p, bytes.clone(), key)));
        keep(&mut self.files, self.frame, KEPT_BYTES, |(_, b, _)| b.len());
        Some((bytes, key))
    }

    /// A workspace file's image: None when the file can't be read, else the
    /// PNG it holds if it decodes. A frame finds it by its name.
    pub fn file_image(&mut self, workspace: &Path, rel: &str) -> Option<Option<Rc<super::hstudio_meshes::Texture>>> {
        let (bytes, key) = self.read(workspace, rel)?;
        Some(self.decoded(key, &bytes))
    }

    /// A Roblox image (an asset id or a rbxthumb:// as `roblox_image` reads
    /// them): its thumbnail, fetched once in the background into the
    /// workspace's `.roblox-assets` and kept there. None while it comes (or
    /// when it can't), else the image.
    pub fn roblox(&mut self, workspace: &Path, content: &str) -> Option<Rc<super::hstudio_meshes::Texture>> {
        let (api, rel) = roblox_image(content)?;
        if let Some(rx) = self.fetching.get(&rel) {
            match rx.try_recv() {
                Ok(true) => {
                    self.fetching.remove(&rel);
                }
                Ok(false) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.fetching.remove(&rel);
                    self.failed.insert(rel);
                    return None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return None,
            }
        }
        if let Some(t) = self.file_image(workspace, &rel) {
            return t;
        }
        if self.failed.contains(&rel) || self.fetching.len() >= 16 {
            return None;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let path = workspace.join(&rel);
        let _ = std::thread::Builder::new().name("roblox-image".into()).spawn(move || {
            let _ = tx.send(fetch_roblox_image(&api, &path));
        });
        self.fetching.insert(rel, rx);
        None
    }

    /// Whether a workspace file is an image that draws (a PNG it can decode).
    pub fn loads(&mut self, workspace: &Path, rel: &str) -> bool {
        self.file_image(workspace, rel).flatten().is_some()
    }

    /// The key `bytes` are found by.
    fn key(&mut self, bytes: &[u8]) -> u64 {
        self.hashed += 1;
        let h = super::hstudio_crypt::sha256(bytes);
        u64::from_le_bytes(h[..8].try_into().unwrap_or_default())
    }

    fn get(&mut self, bytes: &[u8]) -> Option<Rc<super::hstudio_meshes::Texture>> {
        let key = self.key(bytes);
        self.decoded(key, bytes)
    }

    /// The image of `bytes`, found by their `key`, decoded once, in the
    /// place in the game's atlas it had.
    fn decoded(&mut self, key: u64, bytes: &[u8]) -> Option<Rc<super::hstudio_meshes::Texture>> {
        if let Some(i) = self.items.iter().position(|(_, (k, _))| *k == key) {
            return Some(touch(&mut self.items, i, self.frame).1.clone());
        }
        self.decoded += 1;
        let mut t = super::hstudio_meshes::decode_png(bytes).ok()?;
        if self.places.len() >= PLACES && !self.places.contains_key(&key) {
            self.places.clear();
        }
        t.key = *self.places.entry(key).or_insert(t.key);
        let t = Rc::new(t);
        self.items.push((self.frame, (key, t.clone())));
        keep(&mut self.items, self.frame, KEPT_TEXELS, |(_, t)| t.texels());
        Some(t)
    }

    /// A frame begins drawing at `now` (the studio's clock): its images
    /// have the atlas's share to themselves. Drawing again in the same
    /// frame (the HUD's Drawings and the GUI's screen) shares it.
    pub fn begin(&mut self, now: f64) {
        if self.frame != now {
            self.frame = now;
            self.in_atlas.clear();
            self.texels = 0;
        }
    }

    /// Draws a PNG's pixels into `r` (dp).
    pub fn draw(&mut self, game: &mut dyn GameApi, bytes: &[u8], r: [f32; 4], alpha: f64) {
        let Some(t) = self.get(bytes) else { return };
        self.draw_part(game, &t, r, [0.0, 0.0, t.w as f32, t.h as f32], 0xFF_FFFF, alpha);
    }

    /// What of `t` goes in the game's atlas this frame: `t`, or a copy at
    /// most ATLAS_SIDE a side. None once the frame's images fill their
    /// share of the atlas.
    fn for_atlas(&mut self, t: &Rc<super::hstudio_meshes::Texture>) -> Option<Rc<super::hstudio_meshes::Texture>> {
        let (w, h) = atlas_size(t.w, t.h);
        if !self.in_atlas.contains(&t.key) {
            let texels = (w * h) as usize;
            if self.texels + texels > ATLAS_TEXELS {
                return None;
            }
            self.in_atlas.push(t.key);
            self.texels += texels;
        }
        if (w, h) == (t.w, t.h) {
            return Some(t.clone());
        }
        if let Some(i) = self.small.iter().position(|(_, (k, _))| *k == t.key) {
            return Some(touch(&mut self.small, i, self.frame).1.clone());
        }
        // The copy is the image in the atlas: it takes the image's key, its
        // place there, so one made again is drawn from the same place.
        let mut s = shrink(t, w, h)?;
        (s.key, s.version) = (t.key, t.version);
        let s = Rc::new(s);
        self.small.push((self.frame, (t.key, s.clone())));
        keep(&mut self.small, self.frame, SMALL_TEXELS, |(_, s)| s.texels());
        Some(s)
    }

    /// Draws the part `src` (x, y, width and height in pixels) of an image
    /// into `r` (dp), its pixels multiplied by `rgb` (0xRRGGBB) and `alpha`:
    /// as a texture where the game draws images and the frame's share of
    /// its atlas has room, else averaged into blocks.
    pub fn draw_part(
        &mut self,
        game: &mut dyn GameApi,
        t: &Rc<super::hstudio_meshes::Texture>,
        r: [f32; 4],
        src: [f32; 4],
        rgb: u32,
        alpha: f64,
    ) {
        if t.w == 0 || t.h == 0 || r[2] <= 0.0 || r[3] <= 0.0 || !(src[2] > 0.0 && src[3] > 0.0) {
            return;
        }
        let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
        if let Some(at) = self.for_atlas(t) {
            // The part, in the pixels of what went in the atlas.
            let (kx, ky) = (at.w as f32 / t.w as f32, at.h as f32 / t.h as f32);
            let image = super::hstudio::TexRef {
                key: at.key,
                version: at.version,
                w: at.w,
                h: at.h,
                rgba: &at.rgba,
            };
            let part = [src[0] * kx, src[1] * ky, src[2] * kx, src[3] * ky];
            if game.dp_image(image, r, part, a << 24 | (rgb & 0xFF_FFFF)) {
                return;
            }
        }
        // The pixels the part covers, a block of them to each cell.
        let (px0, py0) = (src[0].floor().max(0.0) as u32, src[1].floor().max(0.0) as u32);
        let px1 = ((src[0] + src[2]).ceil().max(0.0) as u32).min(t.w);
        let py1 = ((src[1] + src[3]).ceil().max(0.0) as u32).min(t.h);
        if px0 >= px1 || py0 >= py1 {
            return;
        }
        let (sw, sh) = (px1 - px0, py1 - py0);
        let tint = [(rgb >> 16) & 0xFF, (rgb >> 8) & 0xFF, rgb & 0xFF];
        let most = |dp: f32| ((dp / CELL_DP).ceil() as u32).clamp(1, CELLS);
        let (cw, ch) = (sw.min(most(r[2])), sh.min(most(r[3])));
        let (bw, bh) = (r[2] / cw as f32, r[3] / ch as f32);
        for cy in 0..ch {
            for cx in 0..cw {
                let (x0, x1) = (cx * sw / cw, ((cx + 1) * sw / cw).max(cx * sw / cw + 1));
                let (y0, y1) = (cy * sh / ch, ((cy + 1) * sh / ch).max(cy * sh / ch + 1));
                let mut sum = [0u32; 4];
                let mut n = 0;
                for y in py0 + y0..(py0 + y1).min(py1) {
                    for x in px0 + x0..(px0 + x1).min(px1) {
                        let i = ((y * t.w + x) * 4) as usize;
                        for (c, s) in sum.iter_mut().enumerate() {
                            *s += u32::from(t.rgba[i + c]);
                        }
                        n += 1;
                    }
                }
                if n == 0 {
                    continue;
                }
                let [r_, g, b, a] = sum.map(|s| s / n);
                let a = (f64::from(a) * alpha) as u32;
                if a == 0 {
                    continue;
                }
                let tinted = |c: u32, k: u32| (c * k + 127) / 255;
                let argb = a << 24 | tinted(r_, tint[0]) << 16 | tinted(g, tint[1]) << 8 | tinted(b, tint[2]);
                game.dp_rect(
                    [r[0] + cx as f32 * bw, r[1] + cy as f32 * bh, bw + 0.3, bh + 0.3],
                    0.0,
                    argb,
                );
            }
        }
    }
}

/// An image's size in the game's atlas: its own, or scaled down to
/// ATLAS_SIDE on its longer side.
fn atlas_size(w: u32, h: u32) -> (u32, u32) {
    let long = w.max(h);
    if long <= ATLAS_SIDE {
        return (w, h);
    }
    let k = f64::from(ATLAS_SIDE) / f64::from(long);
    let side = |n: u32| ((f64::from(n) * k).round() as u32).max(1);
    (side(w), side(h))
}

/// `t` scaled down to `w` x `h`: each pixel the average of those it
/// covers, weighted by their alpha so that no dark fringe shows.
fn shrink(t: &super::hstudio_meshes::Texture, w: u32, h: u32) -> Option<super::hstudio_meshes::Texture> {
    let mut out = super::hstudio_meshes::Texture::new(w, h, 0).ok()?;
    for y in 0..h {
        let (y0, y1) = (y * t.h / h, ((y + 1) * t.h / h).max(y * t.h / h + 1).min(t.h));
        for x in 0..w {
            let (x0, x1) = (x * t.w / w, ((x + 1) * t.w / w).max(x * t.w / w + 1).min(t.w));
            // Each colour weighted by its alpha, and the alpha.
            let mut sum = [0u64; 4];
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let i = ((sy * t.w + sx) * 4) as usize;
                    let px = &t.rgba[i..i + 4];
                    let alpha = u64::from(px[3]);
                    for (s, &c) in sum.iter_mut().zip(&px[..3]) {
                        *s += u64::from(c) * alpha;
                    }
                    sum[3] += alpha;
                }
            }
            let n = u64::from((y1 - y0) * (x1 - x0)).max(1);
            let i = ((y * w + x) * 4) as usize;
            let (colour, weight) = sum.split_at(3);
            for (o, &c) in out.rgba[i..i + 3].iter_mut().zip(colour) {
                *o = c.checked_div(weight[0]).unwrap_or(0) as u8;
            }
            out.rgba[i + 3] = (weight[0] / n) as u8;
        }
    }
    Some(out)
}

// ---------------------------------------------------------------- the calls

fn one(v: Value) -> Res {
    Ok(vec![v])
}

fn none() -> Res {
    Ok(Vec::new())
}

fn inst(env: &mut dyn Env, lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<Id, Throw> {
    args.get(i)
        .and_then(|v| env.rbx().id_of(v))
        .ok_or_else(|| bad_arg(lua, i, fname, "Instance expected"))
}

fn signal_arg(lua: &Lua, args: &[Value], fname: &str) -> Result<SignalRef, Throw> {
    match args.first() {
        Some(Value::Userdata(u)) => u.data.borrow().downcast_ref::<SignalRef>().cloned(),
        _ => None,
    }
    .ok_or_else(|| bad_arg(lua, 0, fname, "RBXScriptSignal expected"))
}

/// A Connection object for getconnections.
fn connection_value(rbx: &Rbx, inst: Id, c: &super::hstudio_rbx::Conn) -> Value {
    let mut t = Table::new();
    t.set_str("Enabled", Value::Bool(c.enabled));
    t.set_str("ForeignState", Value::Bool(false));
    t.set_str("LuaConnection", Value::Bool(true));
    t.set_str("Function", c.f.clone());
    t.set_str("Thread", c.thread.clone().map_or(Value::Nil, Value::Thread));
    for (name, id) in [
        ("Fire", CONN_FIRE),
        ("Defer", CONN_DEFER),
        ("Disconnect", CONN_DISCONNECT),
        ("Disable", CONN_DISABLE),
        ("Enable", CONN_ENABLE),
    ] {
        t.set_str(name, Native::host(name, id));
    }
    t.set_str("__connection", Userdata::value(ConnRef { inst, id: c.id }, None));
    let _ = rbx;
    Value::table(t)
}

fn conn_of(args: &[Value]) -> Option<(TableRef, ConnRef)> {
    let Some(Value::Table(t)) = args.first() else {
        return None;
    };
    let r = match t.borrow().get_str("__connection") {
        Value::Userdata(u) => u.data.borrow().downcast_ref::<ConnRef>().copied(),
        _ => None,
    }?;
    Some((t.clone(), r))
}

/// The bytecode a script has here: its main function, serialized (nil
/// for a script that isn't a running mod's).
fn bytecode(f: &Value) -> Option<Vec<u8>> {
    let Value::Func(c) = f else { return None };
    fn dump(p: &super::hstudio_lua::Proto, out: &mut Vec<u8>) {
        out.extend_from_slice(format!("proto {} {} {} {}\n", p.name, p.line, p.nparams, p.vararg).as_bytes());
        for k in p.constant_slots() {
            out.extend_from_slice(format!("k {:?}\n", k.map(|v| v.plain_text())).as_bytes());
        }
        for op in &p.code {
            out.extend_from_slice(format!("{op:?}\n").as_bytes());
        }
        for child in &p.protos {
            dump(child, out);
        }
    }
    let mut out = b"HSBC1\n".to_vec();
    dump(&c.proto(), &mut out);
    Some(out)
}

impl Ctx<'_> {
    fn workspace(&self) -> PathBuf {
        self.s.dir.join("workspace")
    }

    /// A script that runs, as the mod it is in and the function it runs:
    /// a running mod's `script` (its main file), or the ModuleScript of a
    /// file of a running module that `require` has run (the file).
    /// The environment Roblox's own script `id` runs in: `script` is it, and
    /// the rest is the game's globals.
    fn script_env(&mut self, id: Id) -> Value {
        if let Some((_, v)) = self.s.rbx.script_envs.iter().find(|(i, _)| *i == id) {
            return v.clone();
        }
        let mut env = Table::new();
        env.set_str("script", self.s.rbx.value(id));
        if let Some(renv) = self.s.renv.clone() {
            let mut mt = Table::new();
            mt.set_str("__index", Value::Table(renv));
            if let Value::Table(m) = Value::table(mt) {
                env.meta = Some(m);
            }
        }
        let v = Value::table(env);
        self.s.rbx.script_envs.push((id, v.clone()));
        v
    }
    /// A script's Source compiled to a function, in its own environment
    /// (none when it has no source or the source doesn't compile).
    fn compile_source(&mut self, lua: &mut Lua, id: Id) -> Option<Value> {
        let rbx = &self.s.rbx;
        let n = rbx.inst(id);
        if !is_a(n.class, "LuaSourceContainer") {
            return None;
        }
        let src = match n.props.iter().find(|(k, _)| **k == "Source").map(|(_, v)| v) {
            Some(v) if v.as_bytes().is_some_and(|b| !b.is_empty()) => v.as_bytes().map(<[u8]>::to_vec),
            _ => n.extra.get("Source").and_then(Value::as_bytes).map(<[u8]>::to_vec),
        }
        .filter(|b| !b.is_empty())?;
        let chunk = rbx.full_name(id);
        let Value::Table(env) = self.script_env(id) else {
            return None;
        };
        lua.load_bytes(&src, &chunk, &env).ok()
    }
    fn running_script(&self, id: Id) -> Option<(usize, Value)> {
        self.s
            .mods
            .iter()
            .enumerate()
            .filter(|(_, m)| m.running())
            .find_map(|(i, m)| {
                if m.script == Some(id) {
                    m.main_fn.clone().map(|f| (i, f))
                } else {
                    m.file_fns.iter().find(|(ms, _)| *ms == id).map(|(_, f)| (i, f.clone()))
                }
            })
    }

    /// The host functions of the executor library.
    pub(crate) fn sunc(&mut self, lua: &mut Lua, id: u32, args: Vec<Value>) -> Res {
        let fname = FUNCTIONS.iter().find(|f| f.1 == id).map_or("sunc", |f| f.0);
        match id {
            // ---- files
            2000 | 2002 => {
                let path = check_text(lua, &args, 0, fname)?;
                let data = check_str(lua, &args, 1, fname)?;
                let p = sandboxed(lua, &self.workspace(), &path, fname)?;
                writable(lua, &p, fname)?;
                if p.is_dir() {
                    return Err(lua.rt_error(format!("{fname}: '{path}' is a folder")));
                }
                let old = if id == 2002 {
                    std::fs::metadata(&p).map_or(0, |m| m.len() as usize)
                } else {
                    0
                };
                if old + data.len() > MAX_FILE {
                    return Err(lua.rt_error(format!("{fname}: files are 16 MiB at most")));
                }
                if let Some(dir) = p.parent() {
                    std::fs::create_dir_all(dir).map_err(|e| io_err(lua, fname, e))?;
                }
                if id == 2000 {
                    std::fs::write(&p, &data[..]).map_err(|e| io_err(lua, fname, e))?;
                } else {
                    use std::io::Write;
                    let mut f = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&p)
                        .map_err(|e| io_err(lua, fname, e))?;
                    f.write_all(&data).map_err(|e| io_err(lua, fname, e))?;
                }
                none()
            }
            2001 => {
                let path = check_text(lua, &args, 0, fname)?;
                let p = sandboxed(lua, &self.workspace(), &path, fname)?;
                if !p.is_file() {
                    return Err(lua.rt_error(format!("readfile: '{path}' does not exist")));
                }
                let data = std::fs::read(&p).map_err(|e| io_err(lua, fname, e))?;
                if data.len() > MAX_STRING {
                    return Err(lua.rt_error("readfile: the file is too large"));
                }
                one(Value::bytes(&data))
            }
            2003 | 2004 => {
                let path = check_text(lua, &args, 0, fname)?;
                let Ok(p) = sandboxed(lua, &self.workspace(), &path, fname) else {
                    return one(Value::Bool(false));
                };
                one(Value::Bool(if id == 2003 { p.is_file() } else { p.is_dir() }))
            }
            2005 => {
                let path = check_text(lua, &args, 0, fname)?;
                let p = sandboxed(lua, &self.workspace(), &path, fname)?;
                std::fs::create_dir_all(&p).map_err(|e| io_err(lua, fname, e))?;
                none()
            }
            2006 => {
                let path = check_text(lua, &args, 0, fname)?;
                let p = sandboxed(lua, &self.workspace(), &path, fname)?;
                if p.is_file() {
                    std::fs::remove_file(&p).map_err(|e| io_err(lua, fname, e))?;
                }
                none()
            }
            2007 => {
                let path = check_text(lua, &args, 0, fname)?;
                let p = sandboxed(lua, &self.workspace(), &path, fname)?;
                if p == self.workspace() {
                    return Err(lua.rt_error("delfolder: the workspace folder itself stays"));
                }
                if p.is_dir() {
                    std::fs::remove_dir_all(&p).map_err(|e| io_err(lua, fname, e))?;
                }
                none()
            }
            2008 => {
                let path = opt_str(lua, &args, 0, fname)?
                    .map(|s| String::from_utf8_lossy(&s).into_owned())
                    .unwrap_or_default();
                let p = sandboxed(lua, &self.workspace(), &path, fname)?;
                let _ = std::fs::create_dir_all(self.workspace());
                if !p.is_dir() {
                    return Err(lua.rt_error(format!("listfiles: '{path}' is not a folder")));
                }
                let mut names: Vec<String> = std::fs::read_dir(&p)
                    .map_err(|e| io_err(lua, fname, e))?
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect();
                names.sort();
                let prefix = path.trim_matches('/');
                let list = names
                    .into_iter()
                    .map(|n| {
                        if prefix.is_empty() {
                            Value::str(&n)
                        } else {
                            Value::str(&format!("{prefix}/{n}"))
                        }
                    })
                    .collect();
                one(Value::table(Table::from_list(list)))
            }
            2009 | 2011 => {
                let path = check_text(lua, &args, 0, fname)?;
                let p = sandboxed(lua, &self.workspace(), &path, fname)?;
                let src = std::fs::read(&p).map_err(|_| lua.rt_error(format!("{fname}: '{path}' does not exist")))?;
                let env = super::hstudio_exec::caller_env(lua).ok_or_else(|| lua.rt_error("no environment"))?;
                let f = match lua.load_bytes(&src, &format!("@{path}"), &env) {
                    Ok(f) => f,
                    // As loadstring: loadfile gives nil and the error.
                    Err(e) if id == 2009 => return Ok(vec![Value::Nil, Value::str(&e)]),
                    Err(e) => return Err(lua.rt_error(e)),
                };
                if id == 2009 {
                    return one(f);
                }
                lua.call(self, &f, Vec::new())
            }
            2010 => {
                let path = check_text(lua, &args, 0, fname)?;
                let p = sandboxed(lua, &self.workspace(), &path, fname)?;
                if !p.is_file() {
                    return Err(lua.rt_error(format!("getcustomasset: '{path}' does not exist")));
                }
                one(Value::str(&format!(
                    "rbxasset://horizon-studio/{}",
                    path.trim_start_matches("./")
                )))
            }
            // ---- environments
            2020 => one(self.s.genv.clone().map_or(Value::Nil, Value::Table)),
            2021 => one(self.s.renv.clone().map_or(Value::Nil, Value::Table)),
            2022 => {
                let sid = inst(self, lua, &args, 0, fname)?;
                match self.running_script(sid) {
                    // A module's file: its own environment (`script` is its ModuleScript).
                    Some((i, Value::Func(f))) if self.s.mods[i].script != Some(sid) => one(Value::Table(f.env())),
                    Some((i, _)) => one(self.s.mods[i].env.clone().map_or(Value::Nil, Value::Table)),
                    None if self.s.rbx.core_scripts.contains(&sid) => one(self.script_env(sid)),
                    None => Err(lua.rt_error("getsenv: the script is not running")),
                }
            }
            // ---- the network
            2030 => {
                let Some(Value::Table(opts)) = args.first() else {
                    return Err(bad_arg(lua, 0, "request", "a table of options expected"));
                };
                let req = request_of(lua, opts, &self.s.dir)?;
                if !lua.is_yieldable() {
                    return one(response_value(super::hstudio_net::fetch(&req, false)));
                }
                let owner = self.owner;
                let rbx = &mut self.s.rbx;
                rbx.active = true;
                rbx.http.push(super::hstudio_rbx::HttpWait {
                    owner,
                    thread: lua.current_thread(),
                    pending: super::hstudio_net::send(req, false),
                });
                Err(Throw::Yield(Vec::new()))
            }
            WS_OPEN => {
                let url = check_text(lua, &args, 0, "WebSocket.connect")?;
                if !url.starts_with("ws://") && !url.starts_with("wss://") {
                    return Ok(vec![
                        Value::Nil,
                        Value::str("WebSocket.connect: a ws:// or wss:// address expected"),
                    ]);
                }
                if !lua.is_yieldable() {
                    return Err(lua.rt_error(
                        "WebSocket.connect can't wait here: this code isn't in a thread that can wait (run it with task.spawn)",
                    ));
                }
                let owner = self.owner;
                let rbx = &mut self.s.rbx;
                rbx.active = true;
                let ws = rbx.make("WebSocket", Some(owner));
                rbx.inst_mut(ws).name = "WebSocket".into();
                rbx.sockets.push(super::hstudio_rbx::SocketEntry {
                    id: ws,
                    owner,
                    socket: super::hstudio_net::Socket::open(&url, false),
                    opening: Some(lua.current_thread()),
                });
                Err(Throw::Yield(Vec::new()))
            }
            // ---- instances
            2040 => {
                let i = inst(self, lua, &args, 0, fname)?;
                let rbx = &mut self.s.rbx;
                rbx.active = true;
                one(rbx.new_reference(i))
            }
            2041 => {
                let a = args.first().and_then(|v| self.s.rbx.id_of(v));
                let b = args.get(1).and_then(|v| self.s.rbx.id_of(v));
                one(Value::Bool(a.is_some() && a == b))
            }
            2042 => {
                let rbx = &mut self.s.rbx;
                rbx.active = true;
                one(rbx.value(rbx.hui))
            }
            2043 | 2044 => {
                // The world's entities are instances too: getinstances mirrors
                // them first. getnilinstances needn't: they are in the Workspace.
                if id == 2043 {
                    super::hstudio_rbx_world::look(self);
                }
                let rbx = &self.s.rbx;
                let mut ids: Vec<Id> = rbx
                    .insts
                    .values()
                    .filter(|i| !i.destroyed && i.id != rbx.hui)
                    .filter(|i| id == 2043 || (i.parent.is_none() && i.id != rbx.game))
                    .map(|i| i.id)
                    .collect();
                ids.sort_unstable();
                lua.charge(ids.len() * 16)?;
                one(rbx.list(&ids))
            }
            2045 => {
                let cd = inst(self, lua, &args, 0, fname)?;
                let event = opt_str(lua, &args, 2, fname)?.map(|s| String::from_utf8_lossy(&s).into_owned());
                let event = event.unwrap_or_else(|| "MouseClick".into());
                let rbx = &mut self.s.rbx;
                if rbx.inst(cd).class.name != "ClickDetector" {
                    return Err(bad_arg(lua, 0, fname, "ClickDetector expected"));
                }
                if !["MouseClick", "RightMouseClick", "MouseHoverEnter", "MouseHoverLeave"].contains(&event.as_str()) {
                    return Err(bad_arg(lua, 2, fname, "a ClickDetector event expected"));
                }
                let p = rbx.value(rbx.local_player);
                rbx.fire(cd, &event, vec![p]);
                none()
            }
            2046 => {
                let pp = inst(self, lua, &args, 0, fname)?;
                let rbx = &mut self.s.rbx;
                if rbx.inst(pp).class.name != "ProximityPrompt" {
                    return Err(bad_arg(lua, 0, fname, "ProximityPrompt expected"));
                }
                let p = rbx.value(rbx.local_player);
                for ev in [
                    "PromptButtonHoldBegan",
                    "Triggered",
                    "PromptButtonHoldEnded",
                    "TriggerEnded",
                ] {
                    rbx.fire(pp, ev, vec![p.clone()]);
                }
                none()
            }
            2047 => {
                let a = inst(self, lua, &args, 0, fname)?;
                let b = inst(self, lua, &args, 1, fname)?;
                let touch = match args.get(2) {
                    Some(Value::Bool(t)) => *t,
                    Some(Value::Num(n)) => *n == 0.0,
                    _ => return Err(bad_arg(lua, 2, fname, "boolean or number expected")),
                };
                let rbx = &mut self.s.rbx;
                let ev = if touch { "Touched" } else { "TouchEnded" };
                let (av, bv) = (rbx.value(a), rbx.value(b));
                rbx.fire(b, ev, vec![av]);
                rbx.fire(a, ev, vec![bv]);
                none()
            }
            2048 => {
                let i = inst(self, lua, &args, 0, fname)?;
                let prop = check_text(lua, &args, 1, fname)?;
                let rbx = &self.s.rbx;
                let n = rbx.inst(i);
                let v = n
                    .props
                    .iter()
                    .find(|(k, _)| **k == prop)
                    .map(|(_, v)| v.clone())
                    .or_else(|| n.extra.get(&prop).cloned())
                    .filter(Value::is_callable);
                one(v.unwrap_or_default())
            }
            // ---- signals
            2050 => {
                let s = signal_arg(lua, &args, fname)?;
                let rbx = &self.s.rbx;
                let list: Vec<Value> = rbx
                    .try_inst(s.inst)
                    .map(|i| {
                        i.conns
                            .iter()
                            .filter(|c| c.event == s.event)
                            .map(|c| connection_value(rbx, s.inst, c))
                            .collect()
                    })
                    .unwrap_or_default();
                one(Value::table(Table::from_list(list)))
            }
            2051 => {
                let s = signal_arg(lua, &args, fname)?;
                let fs: Vec<Value> = self
                    .s
                    .rbx
                    .try_inst(s.inst)
                    .map(|i| {
                        i.conns
                            .iter()
                            .filter(|c| c.event == s.event && c.enabled)
                            .map(|c| c.f.clone())
                            .collect()
                    })
                    .unwrap_or_default();
                for f in fs {
                    self.spawn_now(lua, f, args[1..].to_vec());
                }
                none()
            }
            2052 => Err(lua.rt_error("replicatesignal: there is no server here to replicate to")),
            CONN_FIRE | CONN_DEFER => {
                let (t, _) =
                    conn_of(&args).ok_or_else(|| lua.rt_error("expected ':' calling a Connection's method"))?;
                let f = t.borrow().get_str("Function");
                if f.is_callable() {
                    if id == CONN_FIRE {
                        self.spawn_now(lua, f, args[1..].to_vec());
                    } else {
                        let owner = self.owner;
                        self.s.rbx.deferred.push((owner, f, args[1..].to_vec()));
                    }
                }
                none()
            }
            CONN_DISCONNECT | CONN_DISABLE | CONN_ENABLE => {
                let (t, c) =
                    conn_of(&args).ok_or_else(|| lua.rt_error("expected ':' calling a Connection's method"))?;
                let rbx = &mut self.s.rbx;
                if let Some(i) = rbx.insts.get_mut(&c.inst) {
                    match id {
                        CONN_DISCONNECT => i.conns.retain(|x| x.id != c.id),
                        _ => {
                            for x in i.conns.iter_mut().filter(|x| x.id == c.id) {
                                x.enabled = id == CONN_ENABLE;
                            }
                        }
                    }
                }
                t.borrow_mut().set_str("Enabled", Value::Bool(id == CONN_ENABLE));
                none()
            }
            // ---- scripts
            2060 | 2061 => {
                let rbx = &self.s.rbx;
                let mut ids: Vec<Id> = rbx
                    .insts
                    .values()
                    .filter(|i| !i.destroyed && is_a(i.class, "LuaSourceContainer"))
                    .map(|i| i.id)
                    .filter(|&i| id == 2060 || self.running_script(i).is_some() || rbx.core_scripts.contains(&i))
                    .collect();
                ids.sort_unstable();
                one(rbx.list(&ids))
            }
            2062 => {
                let rbx = &self.s.rbx;
                // Roblox's own modules ran when the client started.
                let mut ids: Vec<Id> = rbx
                    .core_scripts
                    .iter()
                    .copied()
                    .filter(|&i| !rbx.inst(i).destroyed && rbx.inst(i).class.name == "ModuleScript")
                    .collect();
                for &(m, _) in &rbx.loaded_modules {
                    if !ids.contains(&m) {
                        ids.push(m);
                    }
                }
                one(rbx.list(&ids))
            }
            // Every thread here is the executor's: no game script calls.
            2063 | 2067 => one(Value::Nil),
            2064..=2066 => {
                let sid = inst(self, lua, &args, 0, fname)?;
                let mut main = self.running_script(sid).map(|(_, f)| f);
                if main.is_none() {
                    // A script that isn't running: what its Source compiles to.
                    main = self.compile_source(lua, sid);
                }
                match (id, main) {
                    (2064, Some(Value::Func(c))) => {
                        one(Value::Func(Rc::new(super::hstudio_lua::Closure::from_body(c.body()))))
                    }
                    (2065, Some(f)) => one(bytecode(&f).map_or(Value::Nil, |b| Value::bytes(&b))),
                    (2066, Some(f)) => one(bytecode(&f).map_or(Value::Nil, |b| {
                        Value::str(&super::hstudio_crypt::hex(&super::hstudio_crypt::sha384(&b)))
                    })),
                    _ => one(Value::Nil),
                }
            }
            // ---- hidden properties
            2070 => {
                let i = inst(self, lua, &args, 0, fname)?;
                let prop = check_text(lua, &args, 1, fname)?;
                let rbx = &self.s.rbx;
                let n = rbx.inst(i);
                match prop.as_str() {
                    "Name" => return Ok(vec![Value::str(&n.name), Value::Bool(false)]),
                    "Parent" => return Ok(vec![n.parent.map_or(Value::Nil, |p| rbx.value(p)), Value::Bool(false)]),
                    "ClassName" => return Ok(vec![Value::str(n.class.name), Value::Bool(false)]),
                    _ => {}
                }
                if let Some((_, v)) = n.props.iter().find(|(k, _)| **k == prop) {
                    return Ok(vec![v.clone(), Value::Bool(false)]);
                }
                let v = n
                    .extra
                    .get(&prop)
                    .cloned()
                    .or_else(|| super::hstudio_rbx::hidden_default(n.class.name, &prop).map(Value::Num))
                    .unwrap_or_default();
                Ok(vec![v, Value::Bool(true)])
            }
            2071 => {
                let i = inst(self, lua, &args, 0, fname)?;
                let prop = check_text(lua, &args, 1, fname)?;
                let v = args.get(2).cloned().unwrap_or_default();
                let rbx = &mut self.s.rbx;
                let declared = rbx.inst(i).props.keys().find(|k| **k == prop).copied();
                match declared {
                    Some(k) => {
                        rbx.raw_set(i, k, v);
                        one(Value::Bool(false))
                    }
                    None => {
                        rbx.inst_mut(i).extra.insert(prop, v);
                        one(Value::Bool(true))
                    }
                }
            }
            2072 | 2073 => {
                let i = inst(self, lua, &args, 0, fname)?;
                let prop = check_text(lua, &args, 1, fname)?;
                let rbx = &mut self.s.rbx;
                let n = rbx.inst_mut(i);
                let declared =
                    matches!(prop.as_str(), "Name" | "Parent" | "ClassName") || n.props.keys().any(|k| **k == prop);
                let hidden =
                    n.extra.contains_key(&prop) || super::hstudio_rbx::hidden_default(n.class.name, &prop).is_some();
                if !declared && !hidden {
                    return one(Value::Nil);
                }
                let now = if declared {
                    !n.unscriptable.contains(&prop)
                } else {
                    n.revealed.contains(&prop)
                };
                if id == 2073 {
                    let on = args.get(2).is_some_and(Value::truthy);
                    match (declared, on) {
                        (true, true) => {
                            n.unscriptable.remove(&prop);
                        }
                        (true, false) => {
                            n.unscriptable.insert(prop);
                        }
                        (false, true) => {
                            n.revealed.insert(prop);
                        }
                        (false, false) => {
                            n.revealed.remove(&prop);
                        }
                    }
                }
                one(Value::Bool(now))
            }
            // ---- Drawing
            DRAWING_NEW => {
                let kind = check_text(lua, &args, 0, "Drawing.new")?;
                let kind = KINDS.iter().find(|k| **k == kind).ok_or_else(|| {
                    bad_arg(
                        lua,
                        0,
                        "Drawing.new",
                        "Line, Text, Image, Circle, Square, Quad or Triangle expected",
                    )
                })?;
                let owner = self.owner;
                let rbx = &mut self.s.rbx;
                if rbx.drawings.len() >= 10_000 {
                    return Err(lua.rt_error("too many Drawing objects (10000)"));
                }
                rbx.active = true;
                let did = rbx.next_drawing;
                rbx.next_drawing += 1;
                rbx.drawings.push(Drawing {
                    id: did,
                    kind,
                    owner,
                    props: defaults(kind),
                });
                let meta = DRAWING_META.with(Rc::clone);
                one(Userdata::value(DrawRef(did), Some(meta)))
            }
            2081 => {
                self.s.rbx.drawings.clear();
                none()
            }
            2084 => one(Value::Bool(args.first().and_then(draw_ref).is_some())),
            2082 | DRAWING_INDEX => {
                let did = args
                    .first()
                    .and_then(draw_ref)
                    .ok_or_else(|| bad_arg(lua, 0, fname, "Drawing expected"))?;
                let key = check_text(lua, &args, 1, fname)?;
                self.drawing_get(lua, did, &key)
            }
            2083 | DRAWING_NEWINDEX => {
                let did = args
                    .first()
                    .and_then(draw_ref)
                    .ok_or_else(|| bad_arg(lua, 0, fname, "Drawing expected"))?;
                let key = check_text(lua, &args, 1, fname)?;
                let v = args.get(2).cloned().unwrap_or_default();
                self.drawing_set(lua, did, &key, v)?;
                none()
            }
            DRAWING_DESTROY => {
                if let Some(did) = args.first().and_then(draw_ref) {
                    self.s.rbx.drawings.retain(|d| d.id != did);
                }
                none()
            }
            DRAWING_TOSTRING => one(Value::str("Drawing")),
            // ---- cache (the Instance registry)
            2100 => {
                if let Some(i) = args.first().and_then(|v| self.s.rbx.id_of(v)) {
                    // A fresh reference: the old one is no longer the cached one.
                    let new = self.s.rbx.new_reference(i);
                    self.s.rbx.inst_mut(i).value = new;
                }
                none()
            }
            2101 => {
                let cached = match (args.first(), args.first().and_then(|v| self.s.rbx.id_of(v))) {
                    (Some(v), Some(i)) => v.raw_eq(&self.s.rbx.inst(i).value),
                    _ => false,
                };
                one(Value::Bool(cached))
            }
            2102 => {
                let a = inst(self, lua, &args, 0, fname)?;
                let b = inst(self, lua, &args, 1, fname)?;
                // `a` now resolves to `b`: its stored value becomes a ref to b.
                let bv = self.s.rbx.new_reference(b);
                self.s.rbx.inst_mut(a).value = bv;
                let _ = a;
                none()
            }
            // ---- the console
            2103 => {
                let text = check_text(lua, &args, 0, fname)?;
                self.s.log(super::hstudio::LogKind::Output, text);
                none()
            }
            2104 => {
                self.s.log.clear();
                none()
            }
            2105 | 2106 => none(),
            2107 => one(Value::str("")),
            2108 => {
                let _ = opt_str(lua, &args, 0, fname)?;
                none()
            }
            // ---- input, onto the Roblox layer's pointer
            2110..=2118 => {
                let rbx = &mut self.s.rbx;
                rbx.active = true;
                match id {
                    2110..=2115 => {
                        let left = id < 2113;
                        if matches!(id, 2110 | 2113) {
                            super::hstudio_rbx_draw::pointer_button(rbx, left, true);
                            super::hstudio_rbx_draw::pointer_button(rbx, left, false);
                        } else {
                            super::hstudio_rbx_draw::pointer_button(rbx, left, matches!(id, 2111 | 2114));
                        }
                    }
                    2116 => {
                        let x = opt_num(lua, &args, 0, fname, rbx.pointer.x)?;
                        let y = opt_num(lua, &args, 1, fname, rbx.pointer.y)?;
                        super::hstudio_rbx_draw::pointer_moved(rbx, x, y);
                    }
                    2117 => {
                        let (x, y) = (rbx.pointer.x, rbx.pointer.y);
                        let dx = opt_num(lua, &args, 0, fname, 0.0)?;
                        let dy = opt_num(lua, &args, 1, fname, 0.0)?;
                        super::hstudio_rbx_draw::pointer_moved(rbx, x + dx, y + dy);
                    }
                    _ => {
                        let steps = opt_num(lua, &args, 0, fname, 0.0)?;
                        super::hstudio_rbx_draw::wheel(rbx, steps);
                    }
                }
                none()
            }
            2119 => one(Value::Bool(self.game.screen() == "playing")),
            // ---- clipboard and the rest
            2120 => {
                let text = check_text(lua, &args, 0, fname)?;
                self.game.set_clipboard(&text);
                none()
            }
            2121 => {
                let text = check_text(lua, &args, 0, fname)?;
                self.s.log(super::hstudio::LogKind::Info, format!("messagebox: {text}"));
                // Roblox's MessageBox returns the button pressed; 1 (OK).
                one(Value::Num(1.0))
            }
            2122 => {
                // The game keeps its own frame rate: the cap is kept for getfpscap.
                let cap = opt_num(lua, &args, 0, fname, 0.0)?;
                self.s.rbx.fps_cap = if cap.is_finite() && cap > 0.0 { cap } else { 0.0 };
                none()
            }
            2123 | 2124 => {
                // queue_on_teleport, setrbxclipboard: nothing to do here.
                one(if id == 2124 { Value::Bool(true) } else { Value::Nil })
            }
            // keypress, keyrelease, keyclick: a Windows virtual-key code, as
            // the game's own key would come (UserInputService, the mouse's
            // KeyDown, ContextActionService).
            2125 | 2126 | 2129 => {
                let vk = check_num(lua, &args, 0, fname)? as i64;
                let key = vk_key(vk).ok_or_else(|| lua.rt_error(format!("{fname}: no key with code {vk}")))?;
                let rbx = &mut self.s.rbx;
                rbx.active = true;
                if id != 2126 {
                    super::hstudio_rbx_draw::game_input(rbx, &key, true, false);
                }
                if id != 2125 {
                    super::hstudio_rbx_draw::game_input(rbx, &key, false, false);
                }
                none()
            }
            2127 => one(Value::str(&self.game.clipboard())),
            2128 => one(Value::Num(if self.s.rbx.fps_cap > 0.0 {
                self.s.rbx.fps_cap
            } else {
                60.0
            })),
            _ => Err(lua.rt_error(format!("unknown executor function {id}"))),
        }
    }

    fn drawing_get(&mut self, lua: &Lua, did: u32, key: &str) -> Res {
        if key == "__OBJECT_EXISTS" {
            return one(Value::Bool(self.s.rbx.drawings.iter().any(|d| d.id == did)));
        }
        if matches!(key, "Destroy" | "Remove") {
            return one(Native::host(key, DRAWING_DESTROY));
        }
        let Some(d) = self.s.rbx.drawings.iter().find(|d| d.id == did) else {
            return Err(lua.rt_error("this Drawing was destroyed"));
        };
        if key == "TextBounds" && d.kind == "Text" {
            let text = prop_get(d, "Text").as_text().unwrap_or_default();
            let size = num(d, "Size").max(1.0);
            let f = font(d);
            let w = f64::from(self.game.dp_measure(&text, size as f32, f));
            let lines = text.split('\n').count().max(1) as f64;
            return one(v2(Vector2::new(w, size * lines)));
        }
        match d.props.iter().find(|p| p.0 == key) {
            Some((_, v)) => one(v.clone()),
            None => Err(lua.rt_error(format!("{key} is not a valid member of {}", d.kind))),
        }
    }

    fn drawing_set(&mut self, lua: &Lua, did: u32, key: &str, v: Value) -> Result<(), Throw> {
        let Some(d) = self.s.rbx.drawings.iter_mut().find(|d| d.id == did) else {
            return Err(lua.rt_error("this Drawing was destroyed"));
        };
        let Some(slot) = d.props.iter_mut().find(|p| p.0 == key) else {
            return Err(lua.rt_error(format!("{key} is not a valid member of {}", d.kind)));
        };
        // A value of the property's own type (a number for a Font index too).
        let ok = match (&slot.1, &v) {
            (Value::Num(_), Value::Num(_)) | (Value::Bool(_), Value::Bool(_)) | (Value::Str(_), Value::Str(_)) => true,
            (a, b) => matches!(
                (get(a), get(b)),
                (Some(Data::V2(_)), Some(Data::V2(_))) | (Some(Data::C3(_)), Some(Data::C3(_)))
            ),
        };
        if !ok {
            return Err(lua.rt_error(format!(
                "Unable to assign property {key}. {} expected, got {}",
                lua.type_of(&slot.1),
                lua.type_of(&v)
            )));
        }
        slot.1 = v;
        Ok(())
    }
}

/// Every executor global's name, for the docs and the editor.
pub fn names() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = super::hstudio_exec::globals().into_iter().map(|g| g.0).collect();
    out.extend(FUNCTIONS.iter().map(|f| f.0));
    out.extend(CACHE.iter().map(|c| c.0));
    out.extend(["http", "Drawing", "WebSocket", "crypt", "base64", "cache"]);
    out
}

/// Whether a value is an Instance (also a cloneref's).
pub fn is_instance(rbx: &Rbx, v: &Value) -> bool {
    rbx.id_of(v).is_some()
}

impl Rbx {
    /// A second reference to an Instance, not `==` to the first (cloneref).
    pub fn new_reference(&self, id: Id) -> Value {
        let meta = match &self.inst(id).value {
            Value::Userdata(u) => u.meta.borrow().clone(),
            Value::Table(t) => t.borrow().meta.clone(),
            _ => None,
        };
        Userdata::value(InstRef(id), meta)
    }
}
