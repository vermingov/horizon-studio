// The worlds saveinstance saved, in Singleplayer's list (hstudio_worlds, in
// the game's title module). saveinstance writes the server's world into the
// studio's workspace folder, where executors write what they save and where
// the player looks; the title's world list shows those worlds beside the
// saves (the ones a 0.10 saveinstance wrote into the saves are there as
// ever). The game opens a world at `saves/<folder>`, so a workspace world's
// folder is its path from the saves (`../horizon-mods/.../workspace/name`):
// playing, renaming, the icon and deleting reach it where it is, with no
// link in the saves (Windows makes those only with privileges). Only a
// folder directly in the workspace with saveinstance's marker and a
// level.dat is listed. Plain paths only: the studio's tests build it too.

use std::cell::Cell;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

/// saveinstance's marker in a world it made (hstudio_dump::MARKER).
pub const MARKER: &str = "horizon-studio-dump.json";

/// The studio's workspace folder, once the studio named it.
static WORKSPACE: Mutex<Option<PathBuf>> = Mutex::new(None);

thread_local! {
    /// The world list is being read for the workspace's worlds: the hook
    /// lets the game's own listing run.
    static LISTING: Cell<bool> = const { Cell::new(false) };
}

/// Where the studio's workspace folder is (its data folder's `workspace`).
pub fn set_workspace(dir: PathBuf) {
    *WORKSPACE.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(dir);
}

/// The workspace folder: the one the studio named, else the one beside
/// these saves.
pub fn workspace(saves: &Path) -> PathBuf {
    match WORKSPACE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
    {
        Some(dir) => dir,
        None => workspace_beside(saves),
    }
}

/// Where the studio's workspace is beside the saves: the game folder's
/// horizon-mods data.
pub fn workspace_beside(saves: &Path) -> PathBuf {
    saves
        .parent()
        .unwrap_or(saves)
        .join("horizon-mods")
        .join(".horizon-mod-data")
        .join("horizon_studio")
        .join("workspace")
}

/// Whether `world` is a world saveinstance made: its marker and a level.dat.
pub fn is_dump(world: &Path) -> bool {
    world.join(MARKER).is_file() && (world.join("level.dat").is_file() || world.join("level.dat_old").is_file())
}

/// `path` made absolute with its `.` and `..` worked out, by its text.
fn plain(path: &Path) -> PathBuf {
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// The folder the game is to open `world` by: its path from `saves`
/// (`../…`), or the whole path when the two share no root (another drive).
pub fn folder_for(saves: &Path, world: &Path) -> String {
    let (from, to) = (plain(saves), plain(world));
    let a: Vec<Component<'_>> = from.components().collect();
    let b: Vec<Component<'_>> = to.components().collect();
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    // Not even the drive in common.
    if common == 0 {
        return to.to_string_lossy().into_owned();
    }
    let mut parts: Vec<String> = vec!["..".into(); a.len() - common];
    parts.extend(b[common..].iter().map(|c| c.as_os_str().to_string_lossy().into_owned()));
    parts.join("/")
}

/// The workspace world a world list's folder names: one of more than one
/// part (a world in the saves has a plain name) that comes to a world
/// saveinstance made, right in the workspace.
pub fn resolve(saves: &Path, folder: &str) -> Option<PathBuf> {
    if Path::new(folder).components().count() < 2 {
        return None;
    }
    resolve_in(saves, &workspace(saves), folder)
}

/// `resolve` with the workspace given.
pub fn resolve_in(saves: &Path, workspace: &Path, folder: &str) -> Option<PathBuf> {
    if Path::new(folder).components().count() < 2 {
        return None;
    }
    let world = plain(&saves.join(folder));
    let ws = plain(workspace);
    (world.parent() == Some(ws.as_path()) && is_dump(&world)).then_some(world)
}

/// The folder the list shows a workspace world under, given its name
/// there: none for a folder that isn't saveinstance's world.
pub fn listed(saves: &Path, workspace: &Path, name: &str) -> Option<String> {
    let world = workspace.join(name);
    if Path::new(name).components().count() != 1 || !is_dump(&world) {
        return None;
    }
    Some(folder_for(saves, &world))
}

/// While the world list reads the workspace's worlds.
pub struct Listing;

impl Drop for Listing {
    fn drop(&mut self) {
        LISTING.with(|l| l.set(false));
    }
}

/// The world list's own listing may run with the workspace's worlds added:
/// none while that is under way (the listing within).
pub fn listing() -> Option<Listing> {
    if LISTING.with(|l| l.replace(true)) {
        return None;
    }
    Some(Listing)
}
