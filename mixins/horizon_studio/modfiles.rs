// A module's files: which files of a module's folder count, how a
// `require` name maps to one of them, and paths in a module written with
// `/` whatever the system. A script mod is one file, `<id>.lua`; a module is
// a folder `<id>/` whose main.lua loads its other `.lua` files with
// `require`. These are plain filesystem rules, shared by the runtime
// (hstudio), the editor and the MCP server (headless.rs), so they all agree
// on what a module holds.

use std::path::{Path, PathBuf};

/// How many folders deep a module's files may be (`a/b/c/file.lua`).
pub const MAX_DEPTH: usize = 3;

/// A module's `.lua` files, as `/`-separated paths in its folder: up to
/// three folders deep, names that start with `.` left out, main.lua first
/// and the others sorted. Hot reload, `require` and the script list all go
/// by this list.
pub fn lua_files(folder: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![(folder.to_path_buf(), String::new(), 0)];
    while let Some((dir, prefix, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let Some(name) = e.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            let path = e.path();
            let rel = format!("{prefix}{name}");
            if path.is_dir() {
                if depth < MAX_DEPTH {
                    stack.push((path, format!("{rel}/"), depth + 1));
                }
            } else if path.extension().is_some_and(|x| x == "lua") {
                out.push(rel);
            }
        }
    }
    out.sort();
    if let Some(i) = out.iter().position(|f| f == "main.lua") {
        let main = out.remove(i);
        out.insert(0, main);
    }
    out
}

/// A file's place in a module ("lib/util.lua") as a path on disk.
pub fn path_in(folder: &Path, rel: &str) -> PathBuf {
    rel.split('/').fold(folder.to_path_buf(), |p, part| p.join(part))
}

/// A path on disk as its place in a module's folder ("lib/util.lua"), if
/// it is inside it.
pub fn rel_in(folder: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(folder).ok()?;
    let parts: Vec<&str> = rel
        .components()
        .map(|c| c.as_os_str().to_str())
        .collect::<Option<_>>()?;
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The file a `require` name loads, as its path in the module:
/// "lib.util", "lib/util" and "lib/util.lua" are all "lib/util.lua". None
/// for a name that is not a file of the module: each part is letters,
/// digits, `_` or `-`, with at most three folders, so no name can reach
/// outside the module's folder.
pub fn require_path(name: &str) -> Option<String> {
    let stem = name.strip_suffix(".lua").unwrap_or(name).replace('.', "/");
    let parts: Vec<&str> = stem.split('/').collect();
    let ok = parts.len() <= MAX_DEPTH + 1
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'));
    ok.then(|| format!("{stem}.lua"))
}

/// The name that requires a file: "lib/util.lua" is "lib.util".
pub fn require_name(rel: &str) -> String {
    rel.strip_suffix(".lua").unwrap_or(rel).replace('/', ".")
}
