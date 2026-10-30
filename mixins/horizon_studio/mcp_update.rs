// Updating horizon-studio-mcp, the MCP server for AI agents, to its newest
// release: from the game (Options > Mods > Horizon Studio), or from the
// server itself (`horizon-studio-mcp update`, its update_server tool). The
// release's archive for this platform is downloaded, checked against the
// release's SHA256SUMS, and its binary put in place of the installed one in
// one rename; a running server switches to it at its next request
// (`mcp::serve`). The server notes where it is installed in
// ~/.horizon-studio/mcp.json, which is how the game finds it.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

pub const REPO: &str = "vermingov/horizon-studio";

/// This platform's release archive.
pub fn asset() -> Option<&'static str> {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("horizon-studio-mcp-x86_64-unknown-linux-musl.tar.gz")
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        Some("horizon-studio-mcp-aarch64-unknown-linux-musl.tar.gz")
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("horizon-studio-mcp-aarch64-apple-darwin.tar.gz")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("horizon-studio-mcp-x86_64-apple-darwin.tar.gz")
    } else if cfg!(all(windows, target_arch = "x86_64")) {
        Some("horizon-studio-mcp-x86_64-pc-windows-msvc.zip")
    } else {
        None
    }
}

/// The binary's name inside the archive.
fn binary_name() -> &'static str {
    if cfg!(windows) {
        "horizon-studio-mcp.exe"
    } else {
        "horizon-studio-mcp"
    }
}

/// `~/.horizon-studio`.
pub fn studio_home() -> Option<PathBuf> {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })?;
    Some(PathBuf::from(home).join(".horizon-studio"))
}

/// Notes in `dir`/mcp.json where the server is installed, and its version.
pub fn note_installed(dir: &Path, exe: &Path, version: &str) {
    let text = serde_json::json!({ "path": exe.to_string_lossy(), "version": version }).to_string();
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(dir.join("mcp.json"), text);
}

/// The server noted in `dir`/mcp.json, if it is still there: its path and version.
pub fn installed(dir: &Path) -> Option<(PathBuf, String)> {
    let j: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("mcp.json")).ok()?).ok()?;
    let path = PathBuf::from(j["path"].as_str()?);
    path.is_file()
        .then(|| (path, j["version"].as_str().unwrap_or("").to_owned()))
}

/// Whether version `a` is newer than `b` ("0.10.0" than "0.9.2").
pub fn newer(a: &str, b: &str) -> bool {
    let n = |v: &str| -> Vec<u64> {
        v.trim_start_matches('v')
            .split(['.', '-'])
            .map_while(|x| x.parse().ok())
            .collect()
    };
    n(a) > n(b)
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .try_proxy_from_env(true)
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(60))
        .user_agent("horizon-studio-mcp-updater")
        .build()
}

/// The newest release's version, from where `releases/latest` leads.
pub fn latest() -> Result<String, String> {
    let r = agent()
        .get(&format!("https://github.com/{REPO}/releases/latest"))
        .call()
        .map_err(|e| format!("can't reach GitHub: {e}"))?;
    let url = r.get_url().to_owned();
    url.rsplit_once("/tag/")
        .map(|(_, tag)| tag.trim_start_matches('v').to_owned())
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "no release found".to_owned())
}

fn download(url: &str, limit: u64) -> Result<Vec<u8>, String> {
    let r = agent().get(url).call().map_err(|e| format!("{url}: {e}"))?;
    let mut out = Vec::new();
    r.into_reader()
        .take(limit)
        .read_to_end(&mut out)
        .map_err(|e| format!("{url}: {e}"))?;
    Ok(out)
}

/// The file `name` in a gzipped tar archive.
fn from_tar_gz(archive: &[u8], name: &str) -> Result<Vec<u8>, String> {
    let mut tar = Vec::new();
    flate2::read::GzDecoder::new(archive)
        .read_to_end(&mut tar)
        .map_err(|e| format!("not a .tar.gz: {e}"))?;
    let mut at = 0;
    while at + 512 <= tar.len() {
        let h = &tar[at..at + 512];
        if h.iter().all(|b| *b == 0) {
            break;
        }
        let field = |r: std::ops::Range<usize>| {
            let f = &h[r];
            String::from_utf8_lossy(&f[..f.iter().position(|b| *b == 0).unwrap_or(f.len())]).into_owned()
        };
        let size = u64::from_str_radix(field(124..136).trim(), 8).unwrap_or(0) as usize;
        let entry = field(0..100);
        let start = at + 512;
        if (h[156] == b'0' || h[156] == 0) && entry.trim_start_matches("./") == name {
            return tar
                .get(start..start + size)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| "a cut-off archive".to_owned());
        }
        at = start + size.div_ceil(512) * 512;
    }
    Err(format!("the archive has no {name}"))
}

fn from_zip(archive: &[u8], name: &str) -> Result<Vec<u8>, String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive)).map_err(|e| format!("not a .zip: {e}"))?;
    let mut f = zip.by_name(name).map_err(|_| format!("the archive has no {name}"))?;
    let mut out = Vec::new();
    f.read_to_end(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Puts the binary in `archive` (the release's `asset`, checked against its
/// `sums`) in place of `target`: written beside it, run once (`--version`),
/// then renamed over it. Returns the new binary's `--version` line.
pub fn install_archive(target: &Path, asset: &str, archive: &[u8], sums: &str) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let want = sums
        .lines()
        .find_map(|l| {
            let mut w = l.split_whitespace();
            let (sum, file) = (w.next()?, w.next()?);
            (file.trim_start_matches('*') == asset).then(|| sum.to_lowercase())
        })
        .ok_or_else(|| format!("the release's SHA256SUMS has no {asset}"))?;
    let got = format!("{:x}", Sha256::digest(archive));
    if got != want {
        return Err("the download doesn't match the release's checksum".into());
    }
    let bytes = if asset.ends_with(".zip") {
        from_zip(archive, binary_name())?
    } else {
        from_tar_gz(archive, binary_name())?
    };
    let name = target.file_name().ok_or("no file name")?.to_string_lossy().into_owned();
    let fresh = target.with_file_name(format!("{name}.new"));
    std::fs::write(&fresh, &bytes).map_err(|e| format!("can't write {}: {e}", fresh.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&fresh, std::fs::Permissions::from_mode(0o755));
    }
    // It runs here before it takes the old one's place.
    let out = std::process::Command::new(&fresh)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("the new server doesn't run here: {e}"))?;
    let version = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if !version.starts_with("horizon-studio-mcp") {
        let _ = std::fs::remove_file(&fresh);
        return Err("the new server doesn't run here".into());
    }
    if cfg!(windows) {
        // A running .exe can be renamed, not replaced.
        let old = target.with_file_name(format!("{name}.old"));
        let _ = std::fs::remove_file(&old);
        if target.exists() {
            std::fs::rename(target, &old).map_err(|e| format!("can't move the old server aside: {e}"))?;
        }
    }
    std::fs::rename(&fresh, target).map_err(|e| format!("can't put the new server in place: {e}"))?;
    Ok(version)
}

/// Updates the server at `target` (version `installed`) to the newest
/// release, and notes it in ~/.horizon-studio/mcp.json: its new version, or
/// None when it is the newest already.
pub fn update(target: &Path, installed: &str) -> Result<Option<String>, String> {
    let asset = asset().ok_or("no release is built for this platform")?;
    let version = latest()?;
    if !newer(&version, installed) {
        return Ok(None);
    }
    let base = format!("https://github.com/{REPO}/releases/download/v{version}");
    let sums = String::from_utf8_lossy(&download(&format!("{base}/SHA256SUMS"), 1 << 16)?).into_owned();
    let archive = download(&format!("{base}/{asset}"), 256 << 20)?;
    install_archive(target, asset, &archive, &sums)?;
    if let Some(dir) = studio_home() {
        note_installed(&dir, target, &version);
    }
    Ok(Some(version))
}

// ------------------------------------------------------------------ the game's button

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Unknown,
    Checking,
    /// Never run on this machine (no mcp.json).
    NotInstalled,
    UpToDate(String),
    Available {
        installed: String,
        latest: String,
    },
    Updating,
    Updated(String),
    Failed(String),
}

static STATE: Mutex<State> = Mutex::new(State::Unknown);

fn set(s: State) {
    *STATE.lock().unwrap_or_else(|e| e.into_inner()) = s;
}

pub fn state() -> State {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Looks for a newer release, in the background (once, or again when asked).
pub fn check() {
    if matches!(state(), State::Checking | State::Updating) {
        return;
    }
    set(State::Checking);
    let _ = std::thread::Builder::new().name("mcp-update-check".into()).spawn(|| {
        let Some((_, version)) = studio_home().and_then(|d| installed(&d)) else {
            return set(State::NotInstalled);
        };
        set(match latest() {
            Ok(latest) if newer(&latest, &version) => State::Available {
                installed: version,
                latest,
            },
            Ok(_) => State::UpToDate(version),
            Err(e) => State::Failed(e),
        });
    });
}

/// Updates the installed server, in the background.
pub fn start_update() {
    let Some((path, version)) = studio_home().and_then(|d| installed(&d)) else {
        return set(State::NotInstalled);
    };
    if matches!(state(), State::Updating) {
        return;
    }
    set(State::Updating);
    let _ = std::thread::Builder::new().name("mcp-update".into()).spawn(move || {
        set(match update(&path, &version) {
            Ok(Some(v)) => State::Updated(v),
            Ok(None) => State::UpToDate(version),
            Err(e) => State::Failed(e),
        });
    });
}

/// The button's label, looking for a release the first time.
pub fn label() -> String {
    if state() == State::Unknown {
        check();
    }
    match state() {
        State::Unknown | State::Checking => "MCP server: looking for updates...".into(),
        State::NotInstalled => "MCP server: not set up here (copy the AI setup prompt)".into(),
        State::UpToDate(v) => format!("MCP server {v}: up to date"),
        State::Available { installed, latest } => format!("Update the MCP server: {installed} to {latest}"),
        State::Updating => "Updating the MCP server...".into(),
        State::Updated(v) => format!("MCP server updated to {v}: agents switch at their next call"),
        State::Failed(e) => format!("MCP update failed: {e} (click to try again)"),
    }
}

/// A click on the button: update when there is one, else look again.
pub fn click() {
    match state() {
        State::Available { .. } => start_update(),
        State::Failed(_) if studio_home().and_then(|d| installed(&d)).is_some() => start_update(),
        State::Checking | State::Updating => {}
        _ => check(),
    }
}
