// saveinstance's world downloader inside the game (hstudio_world_dump): the
// world the client has, saved as a Minecraft world with Horizon's own save
// code, which Singleplayer then opens. On the frame it takes copies (the
// loaded chunks with their block entities a few at a time, the entities,
// the player and the level as `Level`, the maps, the scoreboard); Horizon's
// region writer encodes the chunks and writes them on its thread, and a
// thread of its own writes the rest (level.dat, the player, the maps, the
// server's registries and tags as a data pack, the command tree, the icon
// and the server's resource pack: hstudio_world_files). Everything goes into the one world
// folder hstudio_dump::prepare made in the saves. What the server never
// sends (its plugins, its data packs' functions, chunks the player never
// had loaded) can't be in it.

use super::Game;
use super::hstudio_world_files::Meta;
use horizon_client::save::{SaveDir, SaveTables, SessionLock};
use horizon_protocol::packets::configuration::RegistryTags;
use horizon_protocol::packets::play::{Clientbound as C, Commands, Serverbound as S};
use horizon_world::nbt::Tag;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

type Items = Vec<Option<horizon_client::items::ItemStack>>;

thread_local! {
    static JOBS: RefCell<Vec<Job>> = const { RefCell::new(Vec::new()) };
    static NEXT: Cell<u64> = const { Cell::new(1) };
    /// What the server sent that the game keeps no copy of: its command
    /// tree, its icon and its tags (of a reload; the login's come through
    /// horizon_protocol::hstudio_tags).
    static HEARD: RefCell<Heard> = RefCell::new(Heard::default());
    /// The containers the player opened, as they last showed them (the
    /// game empties a server's container when it closes), by dimension
    /// and block; and the ender chest's.
    static SEEN: RefCell<Seen> = RefCell::new(Seen::default());
    /// The block the player last used an item on: the container the
    /// server opens next is that block's.
    static USED_ON: Cell<Option<[i32; 3]>> = const { Cell::new(None) };
}

#[derive(Default)]
struct Heard {
    commands: Option<Commands>,
    icon: Option<Vec<u8>>,
    tags: Option<Vec<RegistryTags>>,
}

#[derive(Default)]
struct Seen {
    containers: HashMap<(String, (i32, i32, i32)), Items>,
    ender: Option<Items>,
}

/// The most containers remembered.
const MAX_SEEN: usize = 8192;
/// Chunks copied a frame at most, and the time they may take.
const CHUNKS_PER_FRAME: usize = 64;
const FRAME_BUDGET: Duration = Duration::from_millis(3);
/// Finished jobs kept for a poll that never comes (a script that stopped).
const MAX_DONE: usize = 16;

struct Job {
    id: u64,
    world: PathBuf,
    stage: Stage,
}

enum Stage {
    Start,
    Chunks(Box<Writing>, VecDeque<(i32, i32)>),
    Finishing(Box<Writing>),
    Done(Result<(), String>),
}

struct Writing {
    save: Arc<SaveDir>,
    _lock: SessionLock,
    meta: Option<std::thread::JoinHandle<Result<(), String>>>,
    dimension: String,
}

// ---------------------------------------------------------------- the studio's calls

/// saveinstance's dump of the world into `world` begins at the next frame.
pub(super) fn start(g: &Game<'_>, world: &Path) -> Result<u64, String> {
    if g.player.is_none() || g.world.chunks.is_empty() {
        return Err("there is no world loaded to save (join a server or open a world first)".into());
    }
    // Only a folder of the saves that hstudio_dump::prepare made (or made
    // before): its marker says it is a dump.
    let saves = std::env::current_dir().map(|d| d.join("saves")).unwrap_or_default();
    if world.parent().is_none_or(|p| !same_folder(p, &saves)) || !world.join(super::hstudio_dump::MARKER).is_file() {
        return Err(format!(
            "'{}' is not a world saveinstance made in the saves folder",
            world.display()
        ));
    }
    if !g.world_root.as_os_str().is_empty() && same_folder(&g.world_root, world) {
        return Err("that is the world being played: saveinstance writes only into worlds of its own".into());
    }
    if SessionLock::is_locked(world) {
        return Err(format!("'{}' is open in a game: close it first", world.display()));
    }
    if JOBS.with(|j| {
        j.borrow()
            .iter()
            .any(|j| j.world == world && !matches!(j.stage, Stage::Done(_)))
    }) {
        return Err("that world is being saved already".into());
    }
    let id = NEXT.with(|n| n.replace(n.get() + 1));
    JOBS.with(|j| {
        j.borrow_mut().push(Job {
            id,
            world: world.to_path_buf(),
            stage: Stage::Start,
        })
    });
    Ok(id)
}

/// A job's end, once: None while it goes on.
pub(super) fn poll(id: u64) -> Option<Result<(), String>> {
    JOBS.with(|j| {
        let mut j = j.borrow_mut();
        let i = j.iter().position(|job| job.id == id)?;
        if !matches!(j[i].stage, Stage::Done(_)) {
            return None;
        }
        match j.remove(i).stage {
            Stage::Done(r) => Some(r),
            _ => None,
        }
    })
}

fn same_folder(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

// ---------------------------------------------------------------- what the game hears

/// The server's packets the game keeps no copy of.
pub(super) fn heard(packet: &C) {
    match packet {
        C::Commands(p) => HEARD.with(|h| h.borrow_mut().commands = Some(p.clone())),
        C::ServerData(p) => HEARD.with(|h| h.borrow_mut().icon = p.icon.clone()),
        C::UpdateTags(p) => HEARD.with(|h| h.borrow_mut().tags = Some(p.registries.clone())),
        _ => {}
    }
}

/// The game's packets: the block an item is used on opens the container.
pub(super) fn sent(packet: &S) {
    if let S::UseItemOn(p) = packet {
        USED_ON.with(|u| u.set(Some(p.pos)));
    }
}

/// Each frame: the open container remembered, and the dumps' work.
pub(super) fn frame(game: &mut Game<'_>) {
    remember_container(game);
    let busy = JOBS.with(|j| j.borrow().iter().any(|j| !matches!(j.stage, Stage::Done(_))));
    if !busy {
        return;
    }
    let started = Instant::now();
    let mut jobs = JOBS.with(|j| std::mem::take(&mut *j.borrow_mut()));
    for job in &mut jobs {
        if started.elapsed() >= FRAME_BUDGET {
            break;
        }
        let stage = std::mem::replace(&mut job.stage, Stage::Start);
        job.stage = match step(game, &job.world, stage, started) {
            Ok(next) => next,
            Err(why) => Stage::Done(Err(why)),
        };
    }
    // Jobs nothing polls any more go, the oldest first.
    while jobs.iter().filter(|j| matches!(j.stage, Stage::Done(_))).count() > MAX_DONE {
        if let Some(i) = jobs.iter().position(|j| matches!(j.stage, Stage::Done(_))) {
            jobs.remove(i);
        }
    }
    JOBS.with(|j| {
        let mut j = j.borrow_mut();
        jobs.append(&mut j);
        *j = jobs;
    });
}

fn remember_container(game: &Game<'_>) {
    let Some(key) = game.container else { return };
    if game.remote.is_none() {
        return;
    }
    let Some(entity) = game.world.block_entities.get(&key) else {
        return;
    };
    let slots = entity.slots().to_vec();
    let dimension = game.dim.id().to_owned();
    SEEN.with(|seen| {
        let mut seen = seen.borrow_mut();
        if seen.containers.len() >= MAX_SEEN && !seen.containers.contains_key(&(dimension.clone(), key)) {
            return;
        }
        if key.0 != i32::MIN {
            seen.containers.insert((dimension, key), slots);
            return;
        }
        // A container no one block holds: an ender chest's, or a double
        // chest's (the half whose `type` is right holds the first 27).
        let Some(at) = USED_ON.with(Cell::get) else { return };
        let state = game.world.block(at[0], at[1], at[2]);
        let Some(block) = game.pack.assets.registry.state(state) else {
            return;
        };
        let name = block.name.as_str();
        if name == "minecraft:ender_chest" && slots.len() == 27 {
            seen.ender = Some(slots);
        } else if matches!(name, "minecraft:chest" | "minecraft:trapped_chest") && slots.len() == 54 {
            let kind = block.properties.get("type").map(String::as_str);
            let facing = block.properties.get("facing").map(String::as_str);
            // ChestBlock.getConnectedDirection: a left half's other half is
            // clockwise of its facing, a right half's counterclockwise.
            let cw = |f: &str| match f {
                "north" => Some([1, 0]),
                "east" => Some([0, 1]),
                "south" => Some([-1, 0]),
                "west" => Some([0, -1]),
                _ => None,
            };
            let (Some(kind), Some(d)) = (kind, facing.and_then(cw)) else {
                return;
            };
            let (dx, dz) = if kind == "left" { (d[0], d[1]) } else { (-d[0], -d[1]) };
            let other = (at[0] + dx, at[1], at[2] + dz);
            let here = (at[0], at[1], at[2]);
            let (right, left) = if kind == "right" { (here, other) } else { (other, here) };
            seen.containers.insert((dimension.clone(), right), slots[..27].to_vec());
            seen.containers.insert((dimension, left), slots[27..].to_vec());
        }
    });
}

// ---------------------------------------------------------------- the dump

fn step(game: &mut Game<'_>, world: &Path, stage: Stage, started: Instant) -> Result<Stage, String> {
    match stage {
        Stage::Start => begin(game, world),
        Stage::Chunks(w, mut keys) => {
            let biomes = match &game.world_biomes {
                Some(b) => b.clone(),
                None => game.res.biomes().clone(),
            };
            // The player moved to another dimension meanwhile: the chunks
            // there are another dimension's, and stay out of this one.
            if game.dim.id() != w.dimension {
                keys.clear();
            }
            while !keys.is_empty() && started.elapsed() < FRAME_BUDGET {
                let batch: Vec<(i32, i32)> = keys.drain(..keys.len().min(CHUNKS_PER_FRAME / 4)).collect();
                let mut chunks = game.with_entities(batch.iter().filter_map(|k| game.world.chunks.get(k)));
                SEEN.with(|seen| {
                    let seen = seen.borrow();
                    for c in &mut chunks {
                        let Some(c) = Arc::get_mut(c) else { continue };
                        for (p, e) in c.block_entities.iter_mut() {
                            let Some(items) = seen.containers.get(&(w.dimension.clone(), *p)) else {
                                continue;
                            };
                            let slots = e.slots_mut();
                            if slots.len() == items.len() && slots.iter().all(Option::is_none) {
                                slots.clone_from_slice(items);
                            }
                        }
                    }
                });
                w.save
                    .save_later(&chunks, game.pack.assets.registry, &biomes)
                    .map_err(|e| format!("the chunks weren't saved: {e}"))?;
            }
            Ok(if keys.is_empty() {
                Stage::Finishing(w)
            } else {
                Stage::Chunks(w, keys)
            })
        }
        Stage::Finishing(mut w) => {
            let chunks_done = w
                .save
                .writes_finished()
                .map_err(|e| format!("the chunks weren't written: {e}"))?;
            if w.meta.as_ref().is_some_and(|m| m.is_finished()) {
                match w.meta.take().map(std::thread::JoinHandle::join) {
                    Some(Ok(Err(why))) => return Err(why),
                    Some(Err(_)) => return Err("writing level.dat failed".into()),
                    _ => {}
                }
            }
            Ok(if chunks_done && w.meta.is_none() {
                Stage::Done(Ok(()))
            } else {
                Stage::Finishing(w)
            })
        }
        done @ Stage::Done(_) => Ok(done),
    }
}

fn begin(game: &mut Game<'_>, world: &Path) -> Result<Stage, String> {
    if game.player.is_none() {
        return Err("the world was left before it was saved".into());
    }
    let lock = SessionLock::acquire(world).map_err(|e| format!("the world couldn't be locked: {e}"))?;
    let dimension = game.dim.id().to_owned();
    let save = Arc::new(SaveDir::new(
        horizon_client::save::dimension_dir(world, &dimension),
        game.world.shape,
    ));
    let _ = save.tables.set(Arc::new(
        SaveTables::new(&game.pack.assets).with_block_entities(game.res.facts()),
    ));
    // The nearest chunks first.
    let (px, pz) = game.player.as_ref().map_or((0, 0), |p| {
        ((p.position.x.floor() as i32) >> 4, (p.position.z.floor() as i32) >> 4)
    });
    let mut keys: Vec<(i32, i32)> = game.world.chunks.keys().copied().collect();
    keys.sort_by_key(|&(x, z)| i64::from(x - px).pow(2) + i64::from(z - pz).pow(2));
    // The entities, all at once (each is a few hundred bytes of NBT): the
    // players aren't saved with chunks.
    let set: HashSet<(i32, i32)> = keys.iter().copied().collect();
    let mut entities = game.entity_chunks(&set, false);
    for (_, list) in &mut entities {
        list.retain(|t| horizon_client::save::text(t.get("id")).as_deref() != Some("minecraft:player"));
    }
    save.save_entities_later(entities)
        .map_err(|e| format!("the entities weren't saved: {e}"))?;
    let meta = metadata(game, world);
    let meta = std::thread::Builder::new()
        .name("hstudio-saveinstance".into())
        .spawn(move || meta.write())
        .map_err(|e| format!("couldn't start writing: {e}"))?;
    Ok(Stage::Chunks(
        Box::new(Writing {
            save,
            _lock: lock,
            meta: Some(meta),
            dimension,
        }),
        keys.into(),
    ))
}

fn metadata(game: &mut Game<'_>, world: &Path) -> Meta {
    // The game's own Level for its singleplayer save, filled in from the
    // client now; the game keeps its own as it was.
    let kept = game.level.clone();
    game.sync_level();
    let mut level = std::mem::replace(&mut game.level, kept);
    level.name = world
        .file_name()
        .map_or_else(|| "saveinstance".into(), |n| n.to_string_lossy().into_owned());
    // Cheats on, so the one who opens it can look around it freely.
    level.allow_commands = Some(true);
    if let Some(uuid) = game.remote.as_ref().map(|r| r.profile()).filter(|&u| u != 0) {
        level.player_uuid = Some([
            (uuid >> 96) as i32,
            (uuid >> 64) as i32,
            (uuid >> 32) as i32,
            uuid as i32,
        ]);
    }
    if let Some(ender) = SEEN.with(|s| s.borrow().ender.clone()) {
        level.ender_items = ender;
    }
    let maps = game
        .maps
        .data
        .iter()
        .map(|(id, m)| {
            let flag = |b: bool| Tag::Byte(i8::from(b));
            let tag = Tag::compound([
                (
                    "data",
                    Tag::compound([
                        ("dimension", Tag::String(m.dimension.as_str().into())),
                        ("xCenter", Tag::Int(m.center[0])),
                        ("zCenter", Tag::Int(m.center[1])),
                        ("scale", Tag::Byte(m.scale as i8)),
                        ("locked", flag(m.locked)),
                        ("trackingPosition", flag(m.tracking)),
                        ("unlimitedTracking", flag(m.unlimited)),
                        ("colors", Tag::ByteArray(m.colors.clone())),
                        ("banners", Tag::List(Vec::new())),
                        ("frames", Tag::List(Vec::new())),
                    ]),
                ),
                ("DataVersion", Tag::Int(horizon_world::DATA_VERSION)),
            ]);
            (*id, tag)
        })
        .collect();
    let registries = game
        .remote
        .as_ref()
        .map(|r| (r.builtin.clone(), r.registries.clone(), r.dimension.name.clone()));
    let tags = HEARD
        .with(|h| h.borrow().tags.clone())
        .unwrap_or_else(horizon_protocol::hstudio_tags::last);
    let packs = game
        .server_packs
        .active
        .iter()
        .filter_map(|p| match &p.source {
            horizon_client::packs::Source::Zip(path) => Some(path.clone()),
            horizon_client::packs::Source::Dir(_) => None,
        })
        .collect();
    let (commands, icon) = HEARD.with(|h| {
        let h = h.borrow();
        (h.commands.clone(), h.icon.clone())
    });
    Meta {
        world: world.to_path_buf(),
        level,
        maps,
        scoreboard: game.score.board.pack(),
        border: game.border.pack(),
        registries,
        tags,
        commands,
        icon,
        packs,
    }
}

// ---------------------------------------------------------------- structures

/// The blocks from `min` to `max` (inclusive) with their states and the
/// block entities the client knows, for saveinstance(part)'s structure file.
pub(super) fn region(g: &Game<'_>, min: [i32; 3], max: [i32; 3]) -> Option<super::hstudio_dump::Region> {
    g.player.as_ref()?;
    let registry = g.pack.assets.registry;
    let mut blocks = Vec::new();
    for y in min[1]..=max[1] {
        for z in min[2]..=max[2] {
            for x in min[0]..=max[0] {
                g.world.chunks.get(&(x >> 4, z >> 4))?;
                let state = registry.state(g.world.block(x, y, z))?;
                let nbt = g.world.block_entities.get(&(x, y, z)).and_then(|e| {
                    let mut tag = horizon_client::save::encode_block_entity((x, y, z), e, &state.name);
                    if let Tag::Compound(m) = &mut tag {
                        for k in ["x", "y", "z"] {
                            m.remove(&k.into());
                        }
                    }
                    let bytes = horizon_world::nbt::write_network(&tag, horizon_world::nbt::Limits::saved()).ok()?;
                    (bytes.first() == Some(&10)).then(|| bytes[1..].to_vec())
                });
                blocks.push(super::hstudio_dump::RegionBlock {
                    pos: [x - min[0], y - min[1], z - min[2]],
                    name: state.name.clone(),
                    properties: state.properties.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                    nbt,
                });
            }
        }
    }
    Some(super::hstudio_dump::Region {
        data_version: horizon_world::DATA_VERSION,
        blocks,
    })
}
