// A module's files: which files of a module's folder count, how a
// `require` name maps to one of them, and paths in a module written with
// `/` whatever the system. A script mod is one file, `<id>.lua`; a module is
// a folder `<id>/` whose main.lua loads its other `.lua` files with
// `require`. These are plain filesystem rules, shared by the runtime
// (hstudio), the editor and the MCP server (headless.rs), so they all agree
// on what a module holds; with them, which names in the mods folder are
// mods (and so which ids are taken), the starter files of a new script or
// module, the names typed for new files, and the moves the studio makes
// (renaming a file, a script made a module, deleting to the trash).

use std::path::{Path, PathBuf};

/// How many folders deep a module's files may be (`a/b/c/file.lua`).
pub const MAX_DEPTH: usize = 3;

/// A new script, from the New prompt: `{name}` and `{id}` are filled in.
pub const TEMPLATE: &str = "-- name: {name}\n-- version: 0.1.0\n-- description: What this mod does.\n\n-- This file runs when the mod loads, and again each time you save it.\nprint(\"{id} loaded\")\n\ncommand(\"{id}\", function(args)\n  return \"Hello from {name}!\"\nend, \"Says hello\")\n\non(\"tick\", function()\n  -- 20 times a second\nend)\n";

/// A new module's files, `{name}` and `{id}` filled in: main.lua starts it
/// and requires the others, shared.lua holds the values every file sees,
/// lib.lua the functions.
pub const MODULE_FILES: [(&str, &str); 3] = [("main.lua", MAIN), ("shared.lua", SHARED), ("lib.lua", LIB)];

const MAIN: &str = "-- name: {name}\n-- version: 0.1.0\n-- description: What this mod does.\n\n-- main.lua starts the module. require loads its other files: each runs once,\n-- and every file that requires it gets the same value back. Saving any of\n-- them reloads the whole module.\nlocal shared = require(\"shared\")\nlocal lib = require(\"lib\")\n\nprint(\"{id} loaded\")\n\ncommand(\"{id}\", function(args)\n  return lib.greet(args ~= \"\" and args or \"you\")\nend, \"Says hello\")\n\non(\"tick\", function()\n  -- 20 times a second\n  shared.ticks = shared.ticks + 1\nend)\n";

const SHARED: &str = "-- shared.lua: the values the module's files share. require(\"shared\") gives\n-- every file this same table, so what one file changes, the others see.\nlocal shared = {\n  greeting = \"Hello\",\n  ticks = 0,\n}\n\nreturn shared\n";

const LIB: &str = "-- lib.lua: functions for the module's other files, which take them with\n-- local lib = require(\"lib\").\nlocal shared = require(\"shared\")\n\nlocal lib = {}\n\nfunction lib.greet(name)\n  return shared.greeting .. \", \" .. name .. \"! (\" .. shared.ticks .. \" ticks so far)\"\nend\n\nreturn lib\n";

/// A new file of a module: `{file}` is its path in the module, `{require}`
/// the name that loads it, `{name}` the mod's name.
pub const NEW_FILE: &str = "-- {file}: a file of {name}. The module's other files load it with\n-- require(\"{require}\"), and each of them gets the table it returns.\nlocal M = {}\n\nfunction M.hello()\n  return \"Hello from {file}\"\nend\n\nreturn M\n";

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

// ----------------------------------------------------------------- the mods folder

/// The mods in the mods folder, as the studio lists them: (id, main file,
/// folder), in the order of their names. A script is a `.lua` file and a
/// module a folder with a main.lua; each takes its id from its name
/// (`mod_id`: "My Mod.lua" is my_mod), and of two names with one id only
/// the first counts. Names that start with `.` are left out.
pub fn find_mods(mods: &Path) -> Vec<(String, PathBuf, Option<PathBuf>)> {
    let Ok(entries) = std::fs::read_dir(mods) else {
        return Vec::new();
    };
    let mut found: Vec<(String, PathBuf, Option<PathBuf>)> = Vec::new();
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for p in paths {
        let Some(name) = p.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let (id, main, folder) = if p.is_dir() {
            let main = p.join("main.lua");
            if !main.is_file() {
                continue;
            }
            (super::hstudio::mod_id(&name), main, Some(p.clone()))
        } else if let Some(stem) = name.strip_suffix(".lua") {
            (super::hstudio::mod_id(stem), p.clone(), None)
        } else {
            continue;
        };
        if found.iter().any(|f| f.0 == id) {
            continue;
        }
        found.push((id, main, folder));
    }
    found
}

/// The mod the mods folder has under `id`, as the studio lists it
/// (`find_mods`: "Night Light.lua" is night_light, and of two names with one
/// id the first): its main file, and its folder for a module.
pub fn find_mod(mods: &Path, id: &str) -> Option<(PathBuf, Option<PathBuf>)> {
    find_mods(mods)
        .into_iter()
        .find(|f| f.0 == id)
        .map(|(_, main, folder)| (main, folder))
}

/// Whether a new mod can't take `id`: the mods folder has `<id>.lua` or
/// `<id>` already, or a mod whose own name gives that id ("My Mod.lua" is
/// my_mod's). Of two mods with one id only one is listed, so the new one
/// or the other would be lost.
pub fn id_taken(mods: &Path, id: &str) -> bool {
    mods.join(format!("{id}.lua")).exists() || mods.join(id).exists() || find_mods(mods).iter().any(|f| f.0 == id)
}

// ----------------------------------------------------------------- names typed for new files

/// What was typed for a file of a module, as its path in it: "util" is
/// "util.lua", "lib/util" (or "lib\util") makes it in a folder. Each part
/// is letters, digits, `_` or `-`: a dot would read as a folder in
/// `require`, so none is allowed but the `.lua` at the end.
pub fn file_name(typed: &str) -> Result<String, String> {
    let t = typed.trim().replace('\\', "/");
    let stem = t.strip_suffix(".lua").unwrap_or(&t);
    let parts: Vec<&str> = stem.split('/').collect();
    if stem.is_empty() {
        return Err("a file needs a name".into());
    }
    if parts.contains(&"..") {
        return Err("'..' would be outside the module: name a file in it".into());
    }
    if stem.starts_with('/') {
        return Err("a path in the module starts with a name, not '/'".into());
    }
    if parts.iter().any(|p| p.starts_with('.')) {
        return Err("a name that starts with '.' is hidden: the module wouldn't see the file".into());
    }
    if stem.contains('.') {
        return Err("dots are for require: name it a/b or a_b".into());
    }
    if parts.iter().any(|p| p.is_empty()) {
        return Err("each '/' needs a name after it".into());
    }
    if let Some(c) = stem
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '/')))
    {
        return Err(if c == ' ' {
            "no spaces in a file's name: a_b, or a/b for a folder".into()
        } else {
            format!("'{c}' can't be in a file's name: letters, digits, '_', '-', and '/' for folders")
        });
    }
    if parts.len() > MAX_DEPTH + 1 {
        return Err("a file can be at most three folders deep".into());
    }
    Ok(format!("{stem}.lua"))
}

/// The line under the New prompt's field: what will be made
/// ("night_light/: main.lua, shared.lua and lib.lua"), nothing yet for an
/// empty name, or why it can't be (an Err, and Enter does nothing).
pub fn plan_mod(mods: &Path, typed: &str, module: bool) -> Result<String, String> {
    let name = typed.trim();
    if name.is_empty() {
        return Ok(String::new());
    }
    let id = super::hstudio::mod_id(name);
    if id_taken(mods, &id) {
        return Err(format!("{id} already exists"));
    }
    Ok(if module {
        format!("{id}/: main.lua, shared.lua and lib.lua")
    } else {
        format!("{id}.lua: one file")
    })
}

/// The line under the New file and Rename fields: where the file goes and
/// the name that requires it, nothing yet for an empty name (or the name it
/// has), or why it can't be. `from` is the file being renamed, by its path
/// in the module.
pub fn plan_file(folder: &Path, typed: &str, from: Option<&str>) -> Result<String, String> {
    if typed.trim().is_empty() {
        return Ok(String::new());
    }
    let rel = file_name(typed)?;
    if from == Some(rel.as_str()) {
        return Ok(String::new());
    }
    if path_in(folder, &rel).exists() {
        return Err(format!("{rel} is there already"));
    }
    let dir = folder
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let name = require_name(&rel);
    Ok(match from {
        None => format!("{dir}/{rel}: require(\"{name}\") loads it"),
        Some(old) => format!(
            "{dir}/{rel}: require(\"{name}\") loads it (other files' require(\"{}\") stay as they are)",
            require_name(old)
        ),
    })
}

// ----------------------------------------------------------------- making, moving, deleting

fn fill(text: &str, id: &str, name: &str) -> String {
    text.replace("{id}", id).replace("{name}", name)
}

/// Writes a new module `<mods>/<id>/` with the three starter files, in a
/// hidden folder first and then renamed into place, so a scan never sees
/// half of it; not while the id is taken. Returns its main.lua.
pub fn write_module(mods: &Path, id: &str, name: &str) -> Result<PathBuf, String> {
    let dest = mods.join(id);
    if id_taken(mods, id) {
        return Err(format!("{id} already exists"));
    }
    let fresh = mods.join(format!(".{id}.creating"));
    let _ = std::fs::remove_dir_all(&fresh);
    let written = std::fs::create_dir_all(&fresh).and_then(|()| {
        MODULE_FILES
            .iter()
            .try_for_each(|(file, text)| std::fs::write(fresh.join(file), fill(text, id, name)))
    });
    if let Err(e) = written.and_then(|()| std::fs::rename(&fresh, &dest)) {
        let _ = std::fs::remove_dir_all(&fresh);
        return Err(e.to_string());
    }
    Ok(dest.join("main.lua"))
}

/// Writes a new file of a module (`rel`, its path in it) from NEW_FILE,
/// making its folders, never over a file that is there. Returns its path.
pub fn write_file(folder: &Path, rel: &str, mod_name: &str) -> Result<PathBuf, String> {
    let path = path_in(folder, rel);
    if path.exists() {
        return Err(format!("{rel} is there already"));
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = NEW_FILE
        .replace("{file}", rel)
        .replace("{require}", &require_name(rel))
        .replace("{name}", mod_name);
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Renames a file of a module, from one path in it to another (main.lua
/// keeps its name): the new folders are made, and those the move leaves
/// empty removed. Returns the file's new path.
pub fn rename_file(folder: &Path, from: &str, to: &str) -> Result<PathBuf, String> {
    if from == "main.lua" {
        return Err("main.lua starts the module: it keeps its name".into());
    }
    let (old, new) = (path_in(folder, from), path_in(folder, to));
    if !old.is_file() {
        return Err(format!("no {from} in the module"));
    }
    if new.exists() {
        return Err(format!("{to} is there already"));
    }
    if let Some(dir) = new.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&old, &new).map_err(|e| e.to_string())?;
    prune_empty(folder, old.parent());
    Ok(new)
}

/// Removes the folders inside a module that a move or a delete left empty,
/// from `dir` up; the module's own folder stays.
pub fn prune_empty(folder: &Path, dir: Option<&Path>) {
    let mut at = dir.map(Path::to_path_buf);
    while let Some(d) = at {
        if d == folder || !d.starts_with(folder) || std::fs::remove_dir(&d).is_err() {
            break;
        }
        at = d.parent().map(Path::to_path_buf);
    }
}

/// Turns a script into a module: its file becomes `<mods>/<id>/main.lua`
/// (an empty folder of that name is used; one with files in it is not).
/// Returns main.lua's path.
pub fn make_module(mods: &Path, script: &Path, id: &str) -> Result<PathBuf, String> {
    let dir = mods.join(id);
    if dir.exists() && !std::fs::read_dir(&dir).is_ok_and(|mut d| d.next().is_none()) {
        return Err(format!("there is a {id} folder with files in it already"));
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let main = dir.join("main.lua");
    if let Err(e) = std::fs::rename(script, &main) {
        let _ = std::fs::remove_dir(&dir);
        return Err(e.to_string());
    }
    Ok(main)
}

/// Moves a file or folder of the mods folder to the trash, at
/// `trash/<its path in the mods folder>`, which keeps every copy and never
/// deletes one: the newest takes the name, and an older copy there is moved
/// beside it under a number ("test (2).lua", then "test (3).lua"). A folder
/// joins the one of its name there (a module deleted after some of its
/// files keeps them beside its own), a file both have numbered the same
/// way. Returns where it went.
pub fn to_trash(trash: &Path, mods: &Path, path: &Path) -> Result<PathBuf, String> {
    let rel = path
        .strip_prefix(mods)
        .ok()
        .filter(|r| !r.as_os_str().is_empty())
        .ok_or_else(|| format!("{} is not in the mods folder", path.display()))?;
    let dest = trash.join(rel);
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    move_over(path, &dest).map_err(|e| e.to_string())?;
    Ok(dest)
}

/// Moves `from` to `to`: a folder into a folder already there entry by
/// entry, so what only `to` has stays, and anything else to `to` once what
/// is there has moved aside to a free numbered name (links are moved as
/// links, never followed). Nothing at `to` is deleted.
fn move_over(from: &Path, to: &Path) -> std::io::Result<()> {
    let folder = |p: &Path| std::fs::symlink_metadata(p).is_ok_and(|m| m.is_dir());
    if folder(from) && folder(to) {
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            move_over(&entry.path(), &to.join(entry.file_name()))?;
        }
        return std::fs::remove_dir(from);
    }
    if std::fs::symlink_metadata(to).is_ok() {
        std::fs::rename(to, numbered(to))?;
    }
    std::fs::rename(from, to)
}

/// The first free name beside `path` with a number from 2 in it, for an
/// older copy: "test.lua" is "test (2).lua", a folder "kit" is "kit (2)".
pub fn numbered(path: &Path) -> PathBuf {
    let name = |p: Option<&std::ffi::OsStr>| p.map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    let (stem, ext) = (
        name(path.file_stem()),
        path.extension().map(|e| format!(".{}", e.to_string_lossy())),
    );
    (2u32..)
        .map(|n| path.with_file_name(format!("{stem} ({n}){}", ext.as_deref().unwrap_or(""))))
        .find(|p| std::fs::symlink_metadata(p).is_err())
        .unwrap_or_else(|| path.to_path_buf())
}
