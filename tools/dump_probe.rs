// saveinstance's world, written and read back by the game's own code: built
// inside the modded game's workspace by tools/dump_e2e.py (as
// crates/horizon-client/examples/hstudio_dump_probe.rs, with the bundle's
// world_files.rs beside it). It saves a chunk (blocks, a chest with its
// items), an entity and the world's other files as hstudio_world_dump does:
// Horizon's SaveDir (save_later, save_entities_later, writes_finished) and
// hstudio_world_files' Meta (level.dat, the player, the void generator, a
// map, the server's registries and tags as a data pack, the command tree).
// Then it reads them back as Singleplayer would: the world list
// (title::list_worlds), the level and player (save::Level::read), the
// region file (horizon_world's RegionFile), the chunk (SaveDir::load), the
// entities, the generator settings and the data pack (datapack::discover).
//
//     hstudio_dump_probe <empty folder>
//
// Prints one line per check (OK ...); exits 1 on the first that fails.

#[path = "hstudio_world_files.rs"]
#[allow(dead_code)]
mod world_files;

use horizon_client::save::{Level, SaveDir};
use horizon_protocol::packets::configuration::{RegistryData, RegistryEntry, RegistryTags, TagEntry};
use horizon_protocol::packets::play::{CommandNode, Commands};
use horizon_protocol::registry::{Builtin, Dynamic};
use horizon_world::nbt::Tag;
use std::path::Path;
use std::sync::Arc;

fn check(what: &str, ok: bool, detail: impl std::fmt::Debug) {
    if ok {
        println!("OK {what}");
    } else {
        println!("FAILED {what}: {detail:?}");
        std::process::exit(1);
    }
}

fn s(v: &str) -> Tag {
    Tag::String(v.into())
}

/// A registry of a few states, as the prepared resources' would be.
fn registry() -> &'static horizon_world::registry::BlockRegistry {
    let state = |id: &str, props: &[(&str, &str)]| {
        let mut m = vec![("id", s(id))];
        if !props.is_empty() {
            m.push((
                "properties",
                Tag::compound(props.iter().map(|(k, v)| (*k, s(v))).collect::<Vec<_>>()),
            ));
        }
        Tag::compound(m)
    };
    let states = vec![
        state("minecraft:air", &[]),
        state("minecraft:stone", &[]),
        state(
            "minecraft:chest",
            &[("facing", "north"), ("type", "single"), ("waterlogged", "false")],
        ),
    ];
    let tag = Tag::compound([
        ("DataVersion", Tag::Int(horizon_world::DATA_VERSION)),
        ("states", Tag::List(states)),
        (
            "defaults",
            Tag::compound([
                ("minecraft:air", Tag::Int(0)),
                ("minecraft:stone", Tag::Int(1)),
                ("minecraft:chest", Tag::Int(2)),
            ]),
        ),
    ]);
    Box::leak(Box::new(
        horizon_world::registry::BlockRegistry::from_nbt(&tag).expect("registry"),
    ))
}

/// Biomes from a resources folder of two biomes and blank colormaps.
fn biomes(root: &Path) -> Arc<horizon_client::biome::Biomes> {
    let dir = root.join("data/minecraft/worldgen/biome");
    std::fs::create_dir_all(&dir).unwrap();
    for b in ["plains", "the_void"] {
        std::fs::write(dir.join(format!("{b}.json")), "{}").unwrap();
    }
    let maps = root.join("assets/minecraft/textures/colormap");
    std::fs::create_dir_all(&maps).unwrap();
    for m in ["grass", "foliage", "dry_foliage"] {
        let file = std::fs::File::create(maps.join(format!("{m}.png"))).unwrap();
        let mut png = png::Encoder::new(file, 1, 1);
        png.set_color(png::ColorType::Rgb);
        png.write_header().unwrap().write_image_data(&[0, 128, 0]).unwrap();
    }
    Arc::new(horizon_client::biome::Biomes::load(root).expect("biomes"))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let base = Path::new(&args[1]);
    let saves = base.join("saves");
    let world = saves.join("play.example.net 2026-11-02");
    std::fs::create_dir_all(&world).unwrap();
    let registry = registry();
    let biomes = biomes(&base.join("resources"));
    let stone = registry.default_state("minecraft:stone").unwrap();
    let chest = registry.default_state("minecraft:chest").unwrap();

    // The chunk, as with_entities copies it: blocks and a chest seen full.
    let mut chunk = horizon_client::chunk::Chunk::empty(2, -1);
    for x in 0..16 {
        for z in 0..16 {
            chunk.set(x, 60, z, stone.0);
        }
    }
    chunk.set(3, 61, 4, chest.0);
    let item = horizon_client::items::ItemStack {
        item: Arc::from("minecraft:diamond"),
        count: 7,
        damage: 0,
        extra: None,
    };
    let mut items = vec![None; 27];
    items[5] = Some(item);
    chunk.block_entities = vec![((35, 61, -12), horizon_client::containers::BlockEntity::Chest(items))];
    let dimension = horizon_client::save::dimension_dir(&world, "minecraft:overworld");
    let save = SaveDir::new(dimension.clone(), horizon_client::chunk::LevelShape::OVERWORLD);
    save.save_later(&[Arc::new(chunk)], registry, &biomes)
        .expect("save_later");
    let pig = Tag::compound([
        ("id", s("minecraft:pig")),
        (
            "Pos",
            Tag::List(vec![
                Tag::Double(40.5f64.to_bits()),
                Tag::Double(61.0f64.to_bits()),
                Tag::Double((-10.5f64).to_bits()),
            ]),
        ),
        ("UUID", Tag::IntArray(vec![1, 2, 3, 4])),
    ]);
    save.save_entities_later(vec![((2, -1), vec![pig])]).expect("entities");

    // The rest, as metadata() fills it from the game.
    let level = Level {
        name: "play.example.net 2026-11-02".into(),
        position: Some([40.5, 61.0, -10.5]),
        rotation: [90.0, 10.0],
        health: 18.0,
        food: 17,
        dimension: "minecraft:overworld".into(),
        inventory: vec![(0, "minecraft:diamond_pickaxe".into(), 1, 3)],
        spawn: Some([40.5, 61.0, -10.5]),
        allow_commands: Some(true),
        player_uuid: Some([11, 22, 33, 44]),
        game_mode: 2,
        ..Level::default()
    };
    let mut dynamic = Dynamic::default();
    dynamic.add(RegistryData {
        registry: "minecraft:dimension_type".into(),
        entries: vec![RegistryEntry {
            id: "minecraft:overworld".into(),
            data: Some(Tag::compound([
                ("min_y", Tag::Int(-64)),
                ("height", Tag::Int(448)),
                ("logical_height", Tag::Int(448)),
                ("has_skylight", Tag::Byte(1)),
                ("coordinate_scale", Tag::Double(1f64.to_bits())),
            ])),
        }],
    });
    dynamic.add(RegistryData {
        registry: "minecraft:painting_variant".into(),
        entries: vec![
            RegistryEntry {
                id: "myserver:logo".into(),
                data: Some(Tag::compound([
                    ("asset_id", s("myserver:logo")),
                    ("width", Tag::Int(2)),
                    ("height", Tag::Int(1)),
                ])),
            },
            // A name that would leave the pack is never written.
            RegistryEntry {
                id: "myserver:../../escape".into(),
                data: Some(Tag::compound([("width", Tag::Int(1))])),
            },
        ],
    });
    let builtin = Builtin::parse(
        r#"{"minecraft:block":{"entries":{"minecraft:air":{"protocol_id":0},"minecraft:stone":{"protocol_id":1}}}}"#,
    )
    .unwrap();
    let tags = vec![RegistryTags {
        registry: "minecraft:block".into(),
        tags: vec![TagEntry {
            name: "myserver:hard".into(),
            ids: vec![1],
        }],
    }];
    let node = |kind: u8, name: &str, children: Vec<i32>, executable: bool| CommandNode {
        kind,
        executable,
        restricted: false,
        children,
        redirect: None,
        name: name.into(),
        parser: None,
        suggestions: None,
    };
    let commands = Commands {
        nodes: vec![node(0, "", vec![1], false), node(1, "spawn", vec![], true)],
        root: 0,
    };
    let map = Tag::compound([
        (
            "data",
            Tag::compound([
                ("dimension", s("minecraft:overworld")),
                ("xCenter", Tag::Int(64)),
                ("zCenter", Tag::Int(64)),
                ("scale", Tag::Byte(0)),
                ("colors", Tag::ByteArray(vec![4; 128 * 128])),
            ]),
        ),
        ("DataVersion", Tag::Int(horizon_world::DATA_VERSION)),
    ]);
    let meta = world_files::Meta {
        world: world.clone(),
        level,
        maps: vec![(3, map)],
        scoreboard: Tag::compound([("Objectives", Tag::List(Vec::new()))]),
        border: Tag::compound([("size", Tag::Double(1000f64.to_bits()))]),
        registries: Some((Arc::new(builtin), Arc::new(dynamic), "minecraft:overworld".into())),
        tags,
        commands: Some(commands),
        icon: None,
        packs: Vec::new(),
    };
    meta.write()
        .unwrap_or_else(|e| check("the world's files are written", false, e));
    for _ in 0..600 {
        if save.writes_finished().expect("writer") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    check(
        "the region writer finished",
        save.writes_finished().unwrap_or(false),
        (),
    );
    drop(save);

    // Singleplayer's list shows it, playable.
    let worlds = horizon_client::title::list_worlds(&saves);
    let entry = worlds.iter().find(|w| w.folder == "play.example.net 2026-11-02");
    check(
        "Singleplayer lists it, readable and of this version",
        entry.is_some_and(|e| !e.corrupted && e.incompatible.is_none() && !e.locked && e.commands),
        worlds
            .iter()
            .map(|w| (&w.folder, w.corrupted, &w.incompatible))
            .collect::<Vec<_>>(),
    );
    check(
        "its level data needs no recovery",
        horizon_client::save::level_data_needs_recovery(&world).is_none(),
        (),
    );
    let read = Level::read(&world).expect("level.dat");
    check(
        "level.dat names it",
        read.name == "play.example.net 2026-11-02",
        &read.name,
    );
    check(
        "the player is where they were, with their inventory",
        read.position == Some([40.5, 61.0, -10.5])
            && read.rotation == [90.0, 10.0]
            && read
                .inventory
                .iter()
                .any(|(slot, id, n, _)| *slot == 0 && id == "minecraft:diamond_pickaxe" && *n == 1)
            && read.game_mode == 2,
        (&read.position, &read.inventory, read.game_mode),
    );
    let generator = horizon_client::save::world_gen_settings(&world).and_then(|g| {
        g.get("dimensions")?
            .get("minecraft:overworld")?
            .get("generator")
            .cloned()
    });
    check(
        "the generator is a void flat world",
        generator.as_ref().is_some_and(|g| {
            horizon_client::save::WorldPreset::of_generator(g) == horizon_client::save::WorldPreset::Flat
                && horizon_client::save::text(g.get("settings").and_then(|s| s.get("biome"))).as_deref()
                    == Some("minecraft:the_void")
        }),
        &generator,
    );

    // The chunk, through the game's own region files.
    let region_path = dimension.join("region").join("r.0.-1.mca");
    let mut region = horizon_world::region::RegionFile::open(&region_path).expect("region file");
    let record = region
        .read(horizon_core::position::ChunkPos::new(2, -1))
        .expect("read")
        .expect("the chunk is in its region file");
    let root = record.nbt(horizon_world::nbt::Limits::saved()).expect("nbt");
    check(
        "the region file holds a full chunk of this version",
        root.tag.get("Status") == Some(&s("minecraft:full"))
            && root.tag.get("DataVersion") == Some(&Tag::Int(horizon_world::DATA_VERSION)),
        root.tag.get("Status"),
    );
    let reader = SaveDir::new(dimension.clone(), horizon_client::chunk::LevelShape::OVERWORLD);
    let back = reader.load(2, -1, registry, &biomes);
    check("the chunk loads as the game loads it", back.is_some(), ());
    let back = back.unwrap();
    check(
        "its blocks came back",
        back.block(5, 60, 5) == stone && back.block(3, 61, 4) == chest && back.block(3, 62, 4).0 == 0,
        (back.block(5, 60, 5), back.block(3, 61, 4)),
    );
    let saved_chest = back.block_entities.iter().find(|(p, _)| *p == (35, 61, -12));
    check(
        "the chest came back with the items seen in it",
        saved_chest.is_some_and(|(_, e)| {
            e.slots()
                .get(5)
                .and_then(Option::as_ref)
                .is_some_and(|i| &*i.item == "minecraft:diamond" && i.count == 7)
        }),
        saved_chest.map(|(_, e)| e.slots().to_vec()),
    );
    let entities = reader.load_entities(2, -1);
    check(
        "the entity came back",
        entities.iter().any(|e| e.get("id") == Some(&s("minecraft:pig"))),
        &entities,
    );

    // The rest of the world's files.
    let maps = world.join("data/minecraft/maps");
    check(
        "the map and its last id",
        horizon_client::save::read_dat(&maps.join("3.dat")).is_some()
            && horizon_client::save::read_dat(&maps.join("last_id.dat"))
                .and_then(|t| t.get("data")?.get("map").cloned())
                == Some(Tag::Int(3)),
        (),
    );
    let packs = horizon_client::datapack::discover(&world);
    check(
        "the server's data pack is the world's",
        packs.iter().any(|p| p.id == format!("file/{}", world_files::DATAPACK)),
        packs.iter().map(|p| p.id.clone()).collect::<Vec<_>>(),
    );
    let pack = world.join("datapacks").join(world_files::DATAPACK).join("data");
    let painting: serde_json::Value =
        serde_json::from_slice(&std::fs::read(pack.join("myserver/painting_variant/logo.json")).unwrap()).unwrap();
    check(
        "a registry entry as JSON",
        painting["width"] == 2 && painting["asset_id"] == "myserver:logo",
        &painting,
    );
    let dim: serde_json::Value =
        serde_json::from_slice(&std::fs::read(pack.join("minecraft/dimension_type/overworld.json")).unwrap()).unwrap();
    check(
        "a byte boolean is a boolean",
        dim["has_skylight"] == true && dim["min_y"] == -64,
        &dim,
    );
    // The game reads the server's height of the dimension from the pack (as
    // it opens the world: world_loading::level_shapes).
    let enabled: Vec<&horizon_client::datapack::DataPack> = packs.iter().collect();
    let (overrides, _) = horizon_client::datapack::worldgen_overrides(&enabled, |b| biomes.contains(b));
    let height = overrides.level_height("minecraft:overworld");
    check(
        "the game takes the server's dimension height from it",
        height
            .as_ref()
            .is_some_and(|h| h.min_y == -64 && h.height == 448 && h.has_skylight),
        height.map(|h| (h.min_y, h.height)),
    );
    let tag: serde_json::Value =
        serde_json::from_slice(&std::fs::read(pack.join("myserver/tags/block/hard.json")).unwrap()).unwrap();
    check(
        "a tag by its entries' names",
        tag["values"] == serde_json::json!(["minecraft:stone"]),
        &tag,
    );
    check(
        "nothing outside the world",
        !base.join("escape.json").exists()
            && !saves.join("escape.json").exists()
            && !world.join("escape.json").exists(),
        (),
    );
    let tree: serde_json::Value =
        serde_json::from_slice(&std::fs::read(world.join("server_commands.json")).unwrap()).unwrap();
    check(
        "the command tree",
        tree["children"]["spawn"]["executable"] == true,
        &tree,
    );
    println!("ALL OK {}", world.display());
}
