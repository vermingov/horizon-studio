// ViaHorizon (as `horizon_client::net::hstudio_via`): every server join goes
// through ViaProxy, which runs beside the game and translates Horizon's 26.3
// to the version chosen here (or the one it detects), as ViaFabricPlus does
// inside a Fabric client. This side keeps the settings, finds Java (or
// downloads a runtime), downloads ViaProxy and the OpenAuthMod plugin with
// pinned checksums, runs ViaProxy under a watchdog that stops it with the
// game, and says how far it got. The hooks ask `route` how to join an
// address; the protocol side (`hstudio_via_net`) does the rest.

use horizon_protocol::hstudio_via_net as wire;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub const VIAPROXY_VERSION: &str = "3.4.14";
const VIAPROXY_FILE: &str = "ViaProxy-3.4.14.jar";
const VIAPROXY_URL: &str = "https://repo.viaversion.com/net/raphimc/ViaProxy/3.4.14/ViaProxy-3.4.14.jar";
const VIAPROXY_SHA256: &str = "727a93545672adf2d4fe00023ec02906b34da837b8a38b15c0bb2c8b75fbdb77";
const OAM_FILE: &str = "ViaProxyOpenAuthMod-1.0.2.jar";
const OAM_URL: &str =
    "https://github.com/ViaVersionAddons/ViaProxyOpenAuthMod/releases/download/v1.0.2/ViaProxyOpenAuthMod-1.0.2.jar";
const OAM_SHA256: &str = "95085aac381ae92fc45d476732958b826254be4fbe2876f2d8b619b2373898fe";
/// ViaProxy 3.4 needs Java 17; the runtime downloaded when there is none is 21.
const JAVA_MIN: u32 = 17;
const JAVA_DOWNLOAD: u32 = 21;

/// The version Horizon speaks: joining as it needs no proxy.
pub const NATIVE: &str = "26.3";
/// ViaProxy's own detection (it asks the server which version it is).
pub const AUTO: &str = "auto";

/// The versions ViaProxy translates to, newest first, named as it names them.
pub const VERSIONS: &[&str] = &[
    AUTO,
    "26.3",
    "26.2",
    "26.1-26.1.2",
    "1.21.11",
    "1.21.9-1.21.10",
    "1.21.7-1.21.8",
    "1.21.6",
    "1.21.5",
    "1.21.4",
    "1.21.2-1.21.3",
    "1.21-1.21.1",
    "1.20.5-1.20.6",
    "1.20.3-1.20.4",
    "1.20.2",
    "1.20-1.20.1",
    "1.19.4",
    "1.19.3",
    "1.19.1-1.19.2",
    "1.19",
    "1.18.2",
    "1.18-1.18.1",
    "1.17.1",
    "1.17",
    "1.16.4-1.16.5",
    "1.16.3",
    "1.16.2",
    "1.16.1",
    "1.16",
    "1.15.2",
    "1.15.1",
    "1.15",
    "1.14.4",
    "1.14.3",
    "1.14.2",
    "1.14.1",
    "1.14",
    "1.13.2",
    "1.13.1",
    "1.13",
    "1.12.2",
    "1.12.1",
    "1.12",
    "1.11.1-1.11.2",
    "1.11",
    "1.10.x",
    "1.9.3-1.9.4",
    "1.9.2",
    "1.9.1",
    "1.9",
    "1.8.x",
    "1.7.6-1.7.10",
    "1.7.2-1.7.5",
    "1.6.4",
    "1.6.2",
    "1.6.1",
    "1.5.2",
    "1.5-1.5.1",
    "1.4.6-1.4.7",
    "1.4.4-1.4.5",
    "1.4.2",
    "1.3.1-1.3.2",
    "1.2.4-1.2.5",
    "1.2.1-1.2.3",
    "1.1",
    "1.0.0-1.0.1",
    "b1.8-b1.8.1",
    "b1.7-b1.7.3",
    "b1.6-b1.6.6",
    "b1.5-b1.5.2",
    "b1.4-b1.4.1",
    "b1.3-b1.3.1",
    "b1.2-b1.2.2",
    "b1.1.2",
    "b1.0-b1.1.1",
    "a1.2.3.5-a1.2.6",
    "a1.2.3-a1.2.3.4",
    "a1.2.2",
    "a1.2.0-a1.2.1.1",
    "a1.1.0-a1.1.2.1",
    "a1.0.17-a1.0.17.4",
    "a1.0.16-a1.0.16.2",
    "a1.0.15",
    "c0.30 CPE",
    "c0.28-c0.30",
    "c0.0.20a-c0.27",
    "c0.0.19a-06",
    "c0.0.18a-02",
    "c0.0.16a-02",
    "c0.0.15a-1",
    "Bedrock 1.26.51",
];

/// A version's name for people.
pub fn label(version: &str) -> String {
    match version {
        AUTO => "Auto detect".into(),
        v if v.starts_with("Bedrock") => format!("{v} (experimental)"),
        v => v.into(),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// ViaHorizon is turned off: joins go straight to the server.
    Off,
    /// Getting ready: what it is doing ("Downloading ViaProxy 42%").
    Preparing(String),
    /// ViaProxy is starting.
    Starting,
    /// ViaProxy listens on this port.
    Ready(u16),
    /// It couldn't be started, and why.
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub enabled: bool,
    pub version: String,
}

struct Proxy {
    child: Child,
    stdin: Option<ChildStdin>,
    /// The Java process's id (under the watchdog shell, its child).
    java_pid: Option<u32>,
}

struct State {
    dir: Option<PathBuf>,
    /// Horizon Studio is on (Options > Mods): off, ViaHorizon routes nothing.
    active: bool,
    settings: Settings,
    status: Status,
    /// The Java that runs ViaProxy, as people read it.
    java: Option<String>,
    log: VecDeque<String>,
    proxy: Option<Proxy>,
    /// Bumped by each start and stop, so a superseded worker gives up.
    run: u64,
}

static STATE: Mutex<State> = Mutex::new(State {
    dir: None,
    active: false,
    settings: Settings {
        enabled: true,
        version: String::new(),
    },
    status: Status::Off,
    java: None,
    log: VecDeque::new(),
    proxy: None,
    run: 0,
});
static CHANGED: Condvar = Condvar::new();

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

fn set_status(run: u64, status: Status) {
    let mut s = state();
    if s.run == run {
        s.status = status;
        CHANGED.notify_all();
    }
}

fn note(line: impl Into<String>) {
    let mut s = state();
    if s.log.len() >= 200 {
        s.log.pop_front();
    }
    s.log.push_back(line.into());
}

// ------------------------------------------------------------ settings

/// `enabled=` and `version=` lines.
pub fn parse_settings(text: &str) -> Settings {
    let mut out = Settings {
        enabled: true,
        version: AUTO.into(),
    };
    for line in text.lines() {
        match line.split_once('=') {
            Some(("enabled", v)) => out.enabled = v.trim() != "false",
            Some(("version", v)) => {
                if let Some(known) = find_version(v.trim()) {
                    out.version = known.into();
                }
            }
            _ => {}
        }
    }
    out
}

/// The version of that name (any case), as ViaProxy names it.
pub fn find_version(name: &str) -> Option<&'static str> {
    let name = name.trim();
    if name.eq_ignore_ascii_case("auto detect") {
        return Some(AUTO);
    }
    VERSIONS.iter().copied().find(|v| v.eq_ignore_ascii_case(name))
}

fn save(dir: &Path, s: &Settings) {
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(
        dir.join("settings.txt"),
        format!("enabled={}\nversion={}\n", s.enabled, s.version),
    );
}

/// Starts ViaHorizon with its folder (once; later calls change nothing).
pub fn configure(dir: PathBuf) {
    // ViaProxy runs in a folder of its own: its paths can't be relative.
    let dir = std::path::absolute(&dir).unwrap_or(dir);
    let start = {
        let mut s = state();
        if s.dir.is_some() {
            return;
        }
        let text = std::fs::read_to_string(dir.join("settings.txt")).unwrap_or_default();
        s.settings = parse_settings(&text);
        s.dir = Some(dir);
        s.active = true;
        s.settings.enabled
    };
    wire::set_proxy(wait);
    wire::set_router(|address, using| match using {
        wire::Use::Join => route(address),
        wire::Use::Ping => route_ping(address),
    });
    if start {
        ensure();
    }
}

/// Follows Horizon Studio being on or off in Options > Mods.
pub fn set_active(on: bool) {
    let changed = {
        let mut s = state();
        let changed = s.active != on && s.dir.is_some();
        s.active = on;
        changed
    };
    if changed {
        if on {
            ensure();
        } else {
            stop();
        }
    }
}

pub fn settings() -> Settings {
    state().settings.clone()
}

pub fn status() -> Status {
    state().status.clone()
}

/// What ViaHorizon is doing, in a few words.
pub fn status_text() -> String {
    let s = state();
    match &s.status {
        Status::Off if !s.settings.enabled => "Off, joins go straight to servers".into(),
        Status::Off => "Waiting to start".into(),
        Status::Preparing(what) => what.clone(),
        Status::Starting => "Starting ViaProxy".into(),
        Status::Ready(_) => match &s.java {
            Some(java) => format!("Ready (ViaProxy {VIAPROXY_VERSION}, {java})"),
            None => format!("Ready (ViaProxy {VIAPROXY_VERSION})"),
        },
        Status::Failed(why) => format!("Failed: {why}"),
    }
}

/// ViaProxy's last lines (and ViaHorizon's own notes).
pub fn log() -> Vec<String> {
    state().log.iter().cloned().collect()
}

pub fn set_enabled(on: bool) {
    let dir = {
        let mut s = state();
        s.settings.enabled = on;
        s.dir.clone()
    };
    if let Some(dir) = &dir {
        save(dir, &settings());
    }
    if on {
        ensure();
    } else {
        stop();
    }
}

/// Chooses the version joins go as (`auto`, or one of [`VERSIONS`]).
pub fn set_version(version: &str) -> Result<(), String> {
    let v = find_version(version).ok_or_else(|| format!("ViaProxy has no version {version}"))?;
    let dir = {
        let mut s = state();
        s.settings.version = v.into();
        s.dir.clone()
    };
    if let Some(dir) = &dir {
        save(dir, &settings());
    }
    Ok(())
}

/// The version after (or before) the chosen one, in [`VERSIONS`].
pub fn step_version(by: isize) -> &'static str {
    let now = settings().version;
    let i = VERSIONS.iter().position(|v| *v == now).unwrap_or(0) as isize;
    let n = VERSIONS.len() as isize;
    VERSIONS[(i + by).rem_euclid(n) as usize]
}

// ------------------------------------------------------------ routing

/// How to join `address`: through ViaProxy (the host to connect to instead,
/// in ViaProxy's internal form) or straight (`None`).
pub fn route(address: &str) -> Option<String> {
    let (on, version, status) = {
        let s = state();
        (
            s.settings.enabled && s.active,
            s.settings.version.clone(),
            s.status.clone(),
        )
    };
    if !on {
        return None;
    }
    let version = if version.is_empty() { AUTO.to_owned() } else { version };
    let port = match status {
        Status::Ready(p) => Some(p),
        _ => None,
    };
    let routed = route_with(address, &version, port)?;
    if matches!(status, Status::Off | Status::Failed(_)) {
        ensure();
    }
    // Detection falls back to a straight join while ViaProxy can't run.
    if matches!(status, Status::Failed(_)) && version == AUTO {
        return None;
    }
    Some(routed)
}

/// How to ping `address` for the server list: through ViaProxy when a
/// version is chosen (so the list shows the server as that version);
/// detection pings straight.
pub fn route_ping(address: &str) -> Option<String> {
    let (on, version, status) = {
        let s = state();
        (
            s.settings.enabled && s.active,
            s.settings.version.clone(),
            s.status.clone(),
        )
    };
    if !on || version.is_empty() || version == AUTO || !matches!(status, Status::Ready(_)) {
        return None;
    }
    let port = match status {
        Status::Ready(p) => Some(p),
        _ => None,
    };
    route_with(address, &version, port)
}

/// [`route`]'s rule, for one version and ViaProxy's port (when it runs).
pub fn route_with(address: &str, version: &str, proxy_port: Option<u16>) -> Option<String> {
    let address = address.trim();
    // Friends' worlds, the integrated server, IPv6 literals and ViaProxy's
    // own address (a transfer it rewrote) are joined as they are.
    if address.is_empty()
        || version == NATIVE
        || address.starts_with("friend:")
        || address == "local"
        || address.contains(']')
        || wire::routed(address)
    {
        return None;
    }
    if let Some(p) = proxy_port {
        let own = [format!("127.0.0.1:{p}"), format!("localhost:{p}")];
        if own.iter().any(|o| o.eq_ignore_ascii_case(address)) {
            return None;
        }
    }
    let host = match address.rsplit_once(':') {
        Some((h, port)) if port.parse::<u16>().is_ok() && !h.contains(':') => h,
        _ => address,
    };
    let internal = wire::internal_host(host, address, version);
    (internal.len() <= wire::MAX_HOST).then_some(internal)
}

/// Waits for ViaProxy to listen (the protocol side asks, from the network
/// thread); while it downloads, for a while, then says so.
fn wait(timeout: Duration) -> Result<u16, String> {
    let start = Instant::now();
    let mut s = state();
    loop {
        let limit = match &s.status {
            Status::Ready(p) => return Ok(*p),
            Status::Failed(why) => return Err(format!("ViaHorizon couldn't start ViaProxy: {why}")),
            Status::Off if !s.settings.enabled => return Err("ViaHorizon is turned off".into()),
            Status::Off | Status::Starting => timeout.max(Duration::from_secs(45)),
            Status::Preparing(_) => timeout.max(Duration::from_secs(20)),
        };
        let left = limit.saturating_sub(start.elapsed());
        if left.is_zero() {
            return Err(match &s.status {
                Status::Preparing(what) => {
                    format!("ViaHorizon is still getting ready ({what}). Try again in a moment.")
                }
                _ => "ViaProxy is taking long to start. Try again in a moment.".into(),
            });
        }
        s = CHANGED
            .wait_timeout(s, left.min(Duration::from_millis(500)))
            .map(|r| r.0)
            .unwrap_or_else(|e| e.into_inner().0);
    }
}

// ------------------------------------------------------------ the worker

/// Starts ViaProxy unless it runs or is on its way.
pub fn ensure() {
    let (run, dir) = {
        let mut s = state();
        let busy = matches!(s.status, Status::Preparing(_) | Status::Starting | Status::Ready(_));
        if busy || !s.settings.enabled || !s.active {
            return;
        }
        let Some(dir) = s.dir.clone() else { return };
        s.run += 1;
        s.status = Status::Preparing("Getting ready".into());
        CHANGED.notify_all();
        (s.run, dir)
    };
    // A superseded worker (stopped, then started again) gives up at its next step.
    let spawned = std::thread::Builder::new().name("viahorizon".into()).spawn(move || {
        if let Err(why) = prepare_and_launch(run, &dir) {
            set_status(run, Status::Failed(why.clone()));
            note(format!("[ViaHorizon] {why}"));
        }
    });
    if spawned.is_err() {
        set_status(run, Status::Failed("couldn't start a thread".into()));
    }
}

/// Stops ViaProxy (asks it, then makes sure).
pub fn stop() {
    let proxy = {
        let mut s = state();
        s.run += 1;
        s.status = Status::Off;
        CHANGED.notify_all();
        s.proxy.take()
    };
    if let Some(mut p) = proxy {
        if let Some(stdin) = &mut p.stdin {
            let _ = stdin.write_all(b"stop\n");
            let _ = stdin.flush();
        }
        let deadline = Instant::now() + Duration::from_secs(4);
        while Instant::now() < deadline {
            if matches!(p.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if let Some(pid) = p.java_pid {
            kill_pid(pid);
        }
        let _ = p.child.kill();
        let _ = p.child.wait();
    }
}

fn prepare_and_launch(run: u64, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("can't make {}: {e}", dir.display()))?;
    set_status(run, Status::Preparing("Looking for Java".into()));
    let java = match find_java(dir) {
        Some(found) => found,
        None => {
            download_java(run, dir)?;
            find_java(dir).ok_or("the downloaded Java runtime doesn't run")?
        }
    };
    state().java = Some(format!("Java {}", java.1));
    note(format!("[ViaHorizon] Java {} at {}", java.1, java.0.display()));
    let jars = dir.join("jars");
    let jar = jars.join(VIAPROXY_FILE);
    fetch(run, VIAPROXY_URL, &jar, VIAPROXY_SHA256, "ViaProxy")?;
    let run_dir = dir.join("run");
    let plugins = run_dir.join("plugins");
    std::fs::create_dir_all(&plugins).map_err(|e| e.to_string())?;
    fetch(run, OAM_URL, &plugins.join(OAM_FILE), OAM_SHA256, "its sign-in plugin")?;
    // Only this plugin: an older copy would register its auth method twice.
    if let Ok(entries) = std::fs::read_dir(&plugins) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("ViaProxyOpenAuthMod") && name != OAM_FILE {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    if state().run != run {
        return Ok(());
    }
    set_status(run, Status::Starting);
    launch(run, &java.0, &jar, &run_dir)
}

// ------------------------------------------------------------ Java

/// A Java of at least [`JAVA_MIN`]: its path and major version.
fn find_java(dir: &Path) -> Option<(PathBuf, u32)> {
    let mut candidates = Vec::new();
    // Our own runtime first: it is known to work.
    candidates.extend(homes_below(&dir.join("java")).into_iter().map(|h| java_in(&h)));
    if let Some(home) = std::env::var_os("JAVA_HOME").filter(|h| !h.is_empty()) {
        candidates.push(java_in(Path::new(&home)));
    }
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|d| d.join(exe("java"))));
    }
    for runtime in minecraft_runtimes() {
        for component in children(&runtime) {
            for platform in children(&component) {
                let home = platform.join(component.file_name().unwrap_or_default());
                candidates.push(java_in(&home.join("jre.bundle/Contents/Home")));
                candidates.push(java_in(&home));
            }
        }
    }
    for parent in jdk_parents() {
        candidates.extend(homes_below(&parent).into_iter().map(|h| java_in(&h)));
    }
    let mut seen = Vec::new();
    for c in candidates {
        if !c.is_file() || seen.contains(&c) {
            continue;
        }
        seen.push(c.clone());
        if let Some(major) = java_major(&c)
            && major >= JAVA_MIN
        {
            return Some((c, major));
        }
    }
    None
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

fn java_in(home: &Path) -> PathBuf {
    home.join("bin").join(exe("java"))
}

fn children(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|r| r.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect())
        .unwrap_or_default();
    out.sort();
    out.reverse();
    out
}

/// Java homes one folder below `parent` (a macOS bundle's in Contents/Home).
fn homes_below(parent: &Path) -> Vec<PathBuf> {
    children(parent)
        .into_iter()
        .map(|p| {
            let bundle = p.join("Contents/Home");
            if bundle.is_dir() { bundle } else { p }
        })
        .collect()
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

/// The official launcher's runtime folders (it downloads Java for the game).
fn minecraft_runtimes() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if cfg!(windows) {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            out.push(PathBuf::from(appdata).join(".minecraft/runtime"));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            out.push(
                PathBuf::from(local).join("Packages/Microsoft.4297127D64EC6_8wekyb3d8bbwe/LocalCache/Local/runtime"),
            );
        }
    } else if let Some(home) = home_dir() {
        if cfg!(target_os = "macos") {
            out.push(home.join("Library/Application Support/minecraft/runtime"));
        } else {
            out.push(home.join(".minecraft/runtime"));
            out.push(home.join(".var/app/com.mojang.Minecraft/.minecraft/runtime"));
        }
    }
    out
}

/// Where Java installers put their JDKs and JREs.
fn jdk_parents() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if cfg!(windows) {
        for base in ["ProgramFiles", "ProgramW6432"].iter().filter_map(std::env::var_os) {
            for vendor in [
                "Eclipse Adoptium",
                "Java",
                "Microsoft",
                "Zulu",
                "BellSoft",
                "Amazon Corretto",
            ] {
                out.push(PathBuf::from(&base).join(vendor));
            }
        }
    } else if cfg!(target_os = "macos") {
        out.push(PathBuf::from("/Library/Java/JavaVirtualMachines"));
    } else {
        out.extend(["/usr/lib/jvm", "/usr/java", "/usr/local/lib/jvm"].map(PathBuf::from));
    }
    if let Some(home) = home_dir() {
        out.push(home.join(".jdks"));
        out.push(home.join(".sdkman/candidates/java"));
    }
    out
}

fn quiet(program: &Path) -> Command {
    let command = Command::new(program);
    #[cfg(windows)]
    let command = {
        use std::os::windows::process::CommandExt;
        let mut command = command;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        command
    };
    command
}

/// `java -version`'s major version (8 for `1.8.0_x`).
fn java_major(java: &Path) -> Option<u32> {
    let out = quiet(java).arg("-version").stdin(Stdio::null()).output().ok()?;
    let text = String::from_utf8_lossy(&out.stderr).into_owned() + &String::from_utf8_lossy(&out.stdout);
    if !out.status.success() {
        return None;
    }
    parse_java_version(&text)
}

pub fn parse_java_version(text: &str) -> Option<u32> {
    let at = text.find("version \"")? + 9;
    let version = text[at..].split('"').next()?;
    let mut parts = version.split(['.', '-', '+', '_']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

/// Eclipse Temurin's runtime for this computer, from Adoptium, into `java/`.
fn download_java(run: u64, dir: &Path) -> Result<(), String> {
    let os = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    };
    set_status(
        run,
        Status::Preparing(format!("No Java {JAVA_MIN}+ found: finding Java {JAVA_DOWNLOAD}")),
    );
    let url = format!(
        "https://api.adoptium.net/v3/assets/latest/{JAVA_DOWNLOAD}/hotspot?os={os}&architecture={arch}&image_type=jre&vendor=eclipse"
    );
    let text = agent()
        .get(&url)
        .call()
        .map_err(|e| format!("can't reach Adoptium for Java: {e}"))?
        .into_string()
        .map_err(|e| format!("Adoptium's answer: {e}"))?;
    let listing: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("Adoptium's answer: {e}"))?;
    let package = &listing[0]["binary"]["package"];
    let (Some(link), Some(sum), Some(name)) = (
        package["link"].as_str(),
        package["checksum"].as_str(),
        package["name"].as_str(),
    ) else {
        return Err(format!("Adoptium has no Java {JAVA_DOWNLOAD} for {os} {arch}"));
    };
    let archive = dir.join("downloads").join(name);
    fetch(run, link, &archive, sum, &format!("Java {JAVA_DOWNLOAD}"))?;
    set_status(run, Status::Preparing(format!("Unpacking Java {JAVA_DOWNLOAD}")));
    let target = dir.join("java");
    let staging = dir.join("java.part");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let file = std::fs::File::open(&archive).map_err(|e| e.to_string())?;
    if name.ends_with(".zip") {
        unzip(file, &staging)?;
    } else {
        untar(flate2::read::GzDecoder::new(file), &staging)?;
    }
    let _ = std::fs::remove_dir_all(&target);
    std::fs::rename(&staging, &target).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&archive);
    Ok(())
}

// ------------------------------------------------------------ downloads

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .try_proxy_from_env(true)
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(60))
        .user_agent("HorizonStudio-ViaHorizon/1.0")
        .build()
}

/// `path`, downloaded from `url` unless it is there with this SHA-256.
fn fetch(run: u64, url: &str, path: &Path, sha256: &str, what: &str) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    if let Ok(bytes) = std::fs::read(path)
        && hex(&Sha256::digest(&bytes)) == sha256.to_ascii_lowercase()
    {
        return Ok(());
    }
    set_status(run, Status::Preparing(format!("Downloading {what}")));
    let response = agent()
        .get(url)
        .call()
        .map_err(|e| format!("can't download {what}: {e}"))?;
    let total: Option<u64> = response.header("Content-Length").and_then(|v| v.parse().ok());
    let parent = path.parent().ok_or("no folder")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let part = path.with_extension("part");
    let mut out = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut reader = response.into_reader();
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let (mut done, mut shown) = (0u64, u64::MAX);
    loop {
        if state().run != run {
            return Err("stopped".into());
        }
        let n = reader.read(&mut buf).map_err(|e| format!("downloading {what}: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        let pct = total.map(|t| done * 100 / t.max(1));
        if pct.unwrap_or(done >> 20) != shown {
            shown = pct.unwrap_or(done >> 20);
            let text = match pct {
                Some(p) => format!("Downloading {what} {p}%"),
                None => format!("Downloading {what} ({} MB)", done >> 20),
            };
            set_status(run, Status::Preparing(text));
        }
    }
    drop(out);
    let got = hex(&hasher.finalize());
    if got != sha256.to_ascii_lowercase() {
        let _ = std::fs::remove_file(&part);
        return Err(format!("{what} didn't download correctly (its checksum is wrong)"));
    }
    std::fs::rename(&part, path).map_err(|e| e.to_string())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A path inside an archive, as a relative path without `..`.
fn inside(dest: &Path, name: &str) -> Option<PathBuf> {
    let rel = Path::new(name);
    if name.is_empty()
        || !rel
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return None;
    }
    Some(dest.join(rel))
}

fn octal(field: &[u8]) -> u64 {
    let text = String::from_utf8_lossy(field);
    u64::from_str_radix(text.trim_matches(|c: char| c == '\0' || c == ' '), 8).unwrap_or(0)
}

fn cstr(field: &[u8]) -> String {
    let end = field.iter().position(|b| *b == 0).unwrap_or(field.len());
    String::from_utf8_lossy(&field[..end]).into_owned()
}

/// Unpacks a tar stream's folders and files (links are left out: a runtime
/// has only its licence notices as links).
pub fn untar(mut r: impl Read, dest: &Path) -> Result<(), String> {
    let mut long_name: Option<String> = None;
    let mut header = [0u8; 512];
    loop {
        match r.read_exact(&mut header) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.to_string()),
        }
        if header.iter().all(|b| *b == 0) {
            break;
        }
        let size = octal(&header[124..136]);
        let kind = header[156];
        let padded = size.div_ceil(512) * 512;
        let name = long_name.take().unwrap_or_else(|| {
            let name = cstr(&header[0..100]);
            let prefix = if &header[257..263] == b"ustar\0" {
                cstr(&header[345..500])
            } else {
                String::new()
            };
            if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            }
        });
        match kind {
            b'L' | b'x' => {
                let mut data = vec![0u8; usize::try_from(size).map_err(|e| e.to_string())?.min(1 << 20)];
                r.read_exact(&mut data).map_err(|e| e.to_string())?;
                skip(&mut r, padded - data.len() as u64)?;
                long_name = if kind == b'L' {
                    Some(cstr(&data))
                } else {
                    String::from_utf8_lossy(&data)
                        .lines()
                        .find_map(|l| l.split_once(" path=").map(|(_, p)| p.to_owned()))
                };
            }
            b'0' | b'\0' | b'7' => {
                let path = inside(dest, &name).ok_or_else(|| format!("unsafe path in the archive: {name}"))?;
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let mut out = std::fs::File::create(&path).map_err(|e| e.to_string())?;
                std::io::copy(&mut (&mut r).take(size), &mut out).map_err(|e| e.to_string())?;
                skip(&mut r, padded - size)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mode = (octal(&header[100..108]) & 0o777) as u32 | 0o600;
                    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode));
                }
            }
            b'5' => {
                let path = inside(dest, name.trim_end_matches('/'))
                    .ok_or_else(|| format!("unsafe path in the archive: {name}"))?;
                std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
            }
            _ => skip(&mut r, padded)?,
        }
    }
    Ok(())
}

fn skip(r: &mut impl Read, n: u64) -> Result<(), String> {
    std::io::copy(&mut r.take(n), &mut std::io::sink())
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn unzip(file: std::fs::File, dest: &Path) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = entry.enclosed_name() else { continue };
        let path = dest.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = std::fs::File::create(&path).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ------------------------------------------------------------ the process

/// Watches the game from a shell: when it is gone, so is ViaProxy (even if
/// the game crashed). Its first line names ViaProxy's process id.
const WATCHDOG: &str = r#"parent=$1; shift
"$@" <&0 &
child=$!
echo "viahorizon-pid $child"
trap 'kill "$child" 2>/dev/null' TERM INT HUP
while kill -0 "$parent" 2>/dev/null && kill -0 "$child" 2>/dev/null; do sleep 1; done
kill "$child" 2>/dev/null
wait "$child""#;

/// ViaProxy's arguments: listen on the loopback, take each join's server
/// and version from its handshake, sign in through OpenAuthMod.
pub fn viaproxy_args(jar: &Path, port: u16) -> Vec<String> {
    [
        "-Xms64m",
        "-Xmx1G",
        "-Dfile.encoding=UTF-8",
        "-jar",
        &jar.to_string_lossy(),
        "cli",
        "--bind-address",
        &format!("127.0.0.1:{port}"),
        "--wildcard-domain-handling",
        "INTERNAL",
        "--auth-method",
        "OPENAUTHMOD",
        "--log-ips",
        "false",
    ]
    .map(str::to_owned)
    .to_vec()
}

fn free_port() -> Result<u16, String> {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .map_err(|e| format!("no free port for ViaProxy: {e}"))
}

fn launch(run: u64, java: &Path, jar: &Path, run_dir: &Path) -> Result<(), String> {
    let port = free_port()?;
    let args = viaproxy_args(jar, port);
    let mut command = if cfg!(unix) {
        let mut c = Command::new("/bin/sh");
        c.arg("-c")
            .arg(WATCHDOG)
            .arg("viahorizon")
            .arg(std::process::id().to_string())
            .arg(java)
            .args(&args);
        c
    } else {
        let mut c = quiet(java);
        c.args(&args);
        c
    };
    let mut child = command
        .current_dir(run_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("can't run Java: {e}"))?;
    let java_pid = if cfg!(unix) { None } else { Some(child.id()) };
    if cfg!(windows) {
        // The same watch on Windows: PowerShell waits for the game, then stops ViaProxy.
        let script = format!(
            "Wait-Process -Id {}; Stop-Process -Id {} -Force -ErrorAction SilentlyContinue",
            std::process::id(),
            child.id()
        );
        let _ = quiet(Path::new("powershell"))
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-WindowStyle",
                "Hidden",
                "-Command",
                &script,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdin = child.stdin.take();
    {
        let mut s = state();
        if s.run != run {
            drop(s);
            let _ = child.kill();
            return Ok(());
        }
        s.proxy = Some(Proxy { child, stdin, java_pid });
    }
    note(format!(
        "[ViaHorizon] starting ViaProxy {VIAPROXY_VERSION} on port {port}"
    ));
    if let Some(err) = stderr {
        let _ = std::thread::Builder::new()
            .name("viahorizon-err".into())
            .spawn(move || {
                for line in BufReader::new(err).lines().map_while(Result::ok) {
                    note(strip_ansi(&line));
                }
            });
    }
    let Some(out) = stdout else {
        return Err("no output from ViaProxy".into());
    };
    for line in BufReader::new(out).lines().map_while(Result::ok) {
        let line = strip_ansi(&line);
        if let Some(pid) = line.strip_prefix("viahorizon-pid ") {
            if let Some(p) = state().proxy.as_mut() {
                p.java_pid = pid.trim().parse().ok();
            }
            continue;
        }
        if line.contains("Binding proxy server to") {
            // Ready once the port takes connections (the line comes as it binds).
            let at = std::net::SocketAddr::from(([127, 0, 0, 1], port));
            for _ in 0..50 {
                if std::net::TcpStream::connect_timeout(&at, Duration::from_millis(200)).is_ok() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            set_status(run, Status::Ready(port));
            note(format!("[ViaHorizon] ViaProxy is ready on port {port}"));
        }
        note(line);
    }
    // Its output ended: it stopped.
    let mut s = state();
    if s.run == run {
        let last = s
            .log
            .iter()
            .rev()
            .find(|l| l.contains("ERROR") || l.contains("Exception"))
            .cloned();
        s.status = Status::Failed(match last {
            Some(l) => format!("ViaProxy stopped ({})", l.trim()),
            None => "ViaProxy stopped".into(),
        });
        s.proxy = None;
        CHANGED.notify_all();
    }
    Ok(())
}

fn kill_pid(pid: u32) {
    let pid = pid.to_string();
    let _ = if cfg!(windows) {
        quiet(Path::new("taskkill")).args(["/PID", &pid, "/F"]).output()
    } else {
        Command::new("kill").args(["-9", &pid]).output()
    };
}

/// ViaProxy's log lines without terminal colours.
pub fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for d in chars.by_ref() {
                if d.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}
