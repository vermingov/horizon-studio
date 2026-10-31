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
    Lua, MAX_STRING, Native, Res, Table, TableRef, Throw, Userdata, Value, bad_arg, check_str, check_text, opt_num,
    opt_str,
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
];

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
        ] {
            rbx.lua_methods.push((class, name, m.borrow().get_str(name)));
        }
        let mut ws = Table::new();
        ws.set_str("connect", connect.clone());
        ws.set_str("Connect", connect.clone());
        genv.borrow_mut().set_str("WebSocket", Value::table(ws));
    }
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

/// Decoded images for Drawing's Image (and ImageLabels with custom assets),
/// drawn as blocks of colour (the HUD has no image quads).
#[derive(Default)]
pub struct ImageCache {
    items: Vec<(u64, Rc<super::hstudio_meshes::Texture>)>,
    files: Vec<(PathBuf, Rc<Vec<u8>>)>,
}

/// Image cells a side at most: an image draws as up to 48 x 48 blocks.
const CELLS: u32 = 48;

impl ImageCache {
    /// A workspace file's bytes (getcustomasset's images), read once.
    pub fn file(&mut self, workspace: &Path, rel: &str) -> Option<Rc<Vec<u8>>> {
        let p = workspace.join(rel);
        if !p.starts_with(workspace) || rel.contains("..") {
            return None;
        }
        if let Some((_, b)) = self.files.iter().find(|(q, _)| *q == p) {
            return Some(b.clone());
        }
        let bytes = Rc::new(std::fs::read(&p).ok().filter(|b| b.len() <= MAX_FILE)?);
        if self.files.len() >= 32 {
            self.files.remove(0);
        }
        self.files.push((p, bytes.clone()));
        Some(bytes)
    }

    fn get(&mut self, bytes: &[u8]) -> Option<Rc<super::hstudio_meshes::Texture>> {
        let key = {
            let h = super::hstudio_crypt::sha256(bytes);
            u64::from_le_bytes(h[..8].try_into().unwrap_or_default())
        };
        if let Some((_, t)) = self.items.iter().find(|(k, _)| *k == key) {
            return Some(t.clone());
        }
        let t = Rc::new(super::hstudio_meshes::decode_png(bytes).ok()?);
        if self.items.len() >= 32 {
            self.items.remove(0);
        }
        self.items.push((key, t.clone()));
        Some(t)
    }

    /// Draws a PNG's pixels into `r` (dp), averaged into blocks.
    pub fn draw(&mut self, game: &mut dyn GameApi, bytes: &[u8], r: [f32; 4], alpha: f64) {
        let Some(t) = self.get(bytes) else { return };
        if t.w == 0 || t.h == 0 || r[2] <= 0.0 || r[3] <= 0.0 {
            return;
        }
        let (cw, ch) = (t.w.min(CELLS), t.h.min(CELLS));
        let (bw, bh) = (r[2] / cw as f32, r[3] / ch as f32);
        for cy in 0..ch {
            for cx in 0..cw {
                let (x0, x1) = (cx * t.w / cw, ((cx + 1) * t.w / cw).max(cx * t.w / cw + 1));
                let (y0, y1) = (cy * t.h / ch, ((cy + 1) * t.h / ch).max(cy * t.h / ch + 1));
                let mut sum = [0u32; 4];
                let mut n = 0;
                for y in y0..y1.min(t.h) {
                    for x in x0..x1.min(t.w) {
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
                let argb = a << 24 | r_ << 16 | g << 8 | b;
                game.dp_rect(
                    [r[0] + cx as f32 * bw, r[1] + cy as f32 * bh, bw + 0.3, bh + 0.3],
                    0.0,
                    argb,
                );
            }
        }
    }
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

    /// A mod's script Instance and its index, when it runs.
    fn script_mod(&self, id: Id) -> Option<usize> {
        self.s.mods.iter().position(|m| m.script == Some(id) && m.running())
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
                let f = lua
                    .load_bytes(&src, &format!("@{path}"), &env)
                    .map_err(|e| lua.rt_error(e))?;
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
                match self.script_mod(sid) {
                    Some(i) => one(self.s.mods[i].env.clone().map_or(Value::Nil, Value::Table)),
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
                // The world's entities are instances too: they are mirrored first.
                super::hstudio_rbx_world::look(self);
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
                    .filter(|&i| id == 2060 || self.script_mod(i).is_some())
                    .collect();
                ids.sort_unstable();
                one(rbx.list(&ids))
            }
            2062 => {
                let rbx = &self.s.rbx;
                let ids: Vec<Id> = rbx.loaded_modules.iter().map(|m| m.0).collect();
                one(rbx.list(&ids))
            }
            // Every thread here is the executor's: no game script calls.
            2063 | 2067 => one(Value::Nil),
            2064..=2066 => {
                let sid = inst(self, lua, &args, 0, fname)?;
                let main = self.script_mod(sid).and_then(|i| self.s.mods[i].main_fn.clone());
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
                Ok(vec![n.extra.get(&prop).cloned().unwrap_or_default(), Value::Bool(true)])
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
                let hidden = n.extra.contains_key(&prop);
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
            2122..=2124 => {
                // setfpscap, queue_on_teleport, setrbxclipboard: nothing to do here.
                one(if id == 2124 { Value::Bool(true) } else { Value::Nil })
            }
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
