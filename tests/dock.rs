// The Editor tab's panes as a pure layout (`hstudio_dock`): where groups go
// at any size, the moves, where a dragged tab lands, and `layout.txt`.

use Pane::{Agent, Code, Console, Docs, Library, Problems, Scripts};
use horizon_studio::game::hstudio_dock::{
    Dock, DrawnLeaf, DropAt, Leaf, Node, PANES, Pane, Placed, STRIP, Side, file_text, fits, load, save,
};
use std::path::{Path, PathBuf};

/// FakeGame's full-screen Editor area (960 x 540 dp at scale 1).
const FULL: [f32; 4] = [48.0, 80.0, 864.0, 408.0];
/// An 854 x 480 game's default studio window's Editor area, about.
const SMALL: [f32; 4] = [0.0, 0.0, 632.0, 243.0];

const DEFAULT: &str = "row:0.180([scripts*],col:0.600([code*],[console*,problems,agent]))";

/// The group `p` is placed in.
fn leaf_of(placed: &[Placed], p: Pane) -> &Placed {
    placed
        .iter()
        .find(|l| l.tabs.contains(&p))
        .unwrap_or_else(|| panic!("{p:?} has no place in {placed:?}"))
}

fn placed_has(placed: &[Placed], p: Pane) -> bool {
    placed.iter().any(|l| l.tabs.contains(&p))
}

fn group(tabs: &[Pane]) -> Node {
    Node::Leaf(Leaf {
        tabs: tabs.to_vec(),
        active: tabs[0],
        hidden: false,
    })
}

fn split(row: bool, ratio: f32, a: Node, b: Node) -> Node {
    Node::Split {
        row,
        ratio,
        a: Box::new(a),
        b: Box::new(b),
    }
}

/// The groups as the studio draws them: a strip each, its tabs 80 wide
/// from 8 units in, 2 apart.
fn drawn(d: &Dock, ws: [f32; 4], keep: Pane) -> Vec<DrawnLeaf> {
    d.layout(ws, keep)
        .1
        .into_iter()
        .map(|l| DrawnLeaf {
            rect: l.rect,
            strip: [l.rect[0], l.rect[1], l.rect[2], STRIP],
            tabs: l
                .tabs
                .iter()
                .enumerate()
                .map(|(i, p)| (*p, [l.rect[0] + 8.0 + 82.0 * i as f32, l.rect[1] + 3.0, 80.0, 26.0]))
                .collect(),
            active: l.active,
            collapsed: l.collapsed,
        })
        .collect()
}

/// A folder of its own, gone after the test.
struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Folder {
        let dir = std::env::temp_dir().join(format!("hstudio-dock-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Folder(dir)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn the_default_dock_is_todays_editor_tab() {
    let d = Dock::default();
    assert_eq!(d.to_text(), DEFAULT);
    let (bars, placed) = d.layout(FULL, Code);
    let rects: Vec<([f32; 4], Vec<Pane>, Pane, bool)> = placed
        .iter()
        .map(|l| (l.rect, l.tabs.clone(), l.active, l.collapsed))
        .collect();
    assert_eq!(
        rects,
        [
            ([48.0, 80.0, 154.0, 408.0], vec![Scripts], Scripts, false),
            ([210.0, 80.0, 702.0, 240.0], vec![Code], Code, false),
            (
                [210.0, 328.0, 702.0, 160.0],
                vec![Console, Problems, Agent],
                Console,
                false
            ),
        ]
    );
    let bars: Vec<(u16, [f32; 4], bool, f32, f32)> = bars.iter().map(|b| (b.path, b.rect, b.row, b.lo, b.hi)).collect();
    assert_eq!(
        bars,
        [
            (1, [202.0, 80.0, 8.0, 408.0], true, 188.0, 604.0),
            (3, [210.0, 320.0, 702.0, 8.0], false, 220.0, 360.0),
        ]
    );
    assert!(!d.has(Library) && !d.has(Docs));
    assert!(d.is_front(Code) && d.is_front(Console));
    assert!(d.shown(Agent) && !d.is_front(Agent), "behind the console");
    assert_eq!(d.settled(Agent), Console);
    assert_eq!(d.settled(Code), Code);
    assert_eq!(d.leaf_tabs(Problems), [Console, Problems, Agent]);
    assert_eq!((d.ratio(1), d.ratio(3), d.ratio(2)), (Some(0.18), Some(0.6), None));
    for p in PANES {
        assert_eq!(Pane::from_id(p.id()), Some(p));
        assert_eq!(PANES[p.index()], p);
    }
    assert_eq!(
        PANES.map(|p| (p.takes_keys(), p.closable())),
        [
            (false, false),
            (true, false),
            (true, false),
            (true, false),
            (true, false),
            (true, true),
            (true, true),
        ]
    );
}

#[test]
fn docks_read_back_what_they_wrote() {
    let mut d = Dock::default();
    assert_eq!(Dock::parse(&d.to_text()), Some(d.clone()));
    assert!(d.move_pane(
        Agent,
        DropAt::Beside {
            anchor: Code,
            side: Side::Right
        }
    ));
    assert!(d.move_pane(Scripts, DropAt::Edge(Side::Bottom)));
    assert!(d.show(Docs));
    assert!(d.move_pane(Problems, DropAt::Into { anchor: Docs, at: 0 }));
    assert_eq!(Dock::parse(&d.to_text()), Some(d.clone()), "{}", d.to_text());
    assert!(d.set_hidden(Console, true));
    assert!(d.to_text().contains("-[console*]"), "{}", d.to_text());
    assert_eq!(Dock::parse(&d.to_text()), Some(d.clone()));
    // A ratio set by a bar reads back as it was.
    let mut d = Dock::default();
    assert!(d.set_ratio(3, 0.123_456));
    assert_eq!(d.ratio(3), Some(0.123));
    assert!(d.to_text().contains("col:0.123("), "{}", d.to_text());
    assert_eq!(Dock::parse(&d.to_text()), Some(d.clone()));
    assert!(!d.set_ratio(3, 0.1231), "the same thousandth");
    let before = d.clone();
    assert!(!d.set_ratio(3, f32::NAN));
    assert!(!d.set_ratio(3, f32::INFINITY));
    assert!(!d.set_ratio(3, f32::NEG_INFINITY));
    assert!(!d.set_ratio(2, 0.5), "the script list's group is no split");
    assert!(!d.set_ratio(0, 0.5));
    assert!(!d.set_ratio(7, 0.5), "no such split");
    assert_eq!(d, before);
    // Kept in range.
    assert!(d.set_ratio(1, 2.0));
    assert_eq!(d.ratio(1), Some(0.95));
    assert!(d.set_ratio(1, -1.0));
    assert_eq!(d.ratio(1), Some(0.05));
}

#[test]
fn damaged_files_fall_back_or_heal() {
    let deep = format!("{}[code*]", "row:0.5(".repeat(10));
    let long = format!(
        "row:0.18{}([scripts*],col:0.600([code*],[console*,problems,agent]))",
        "0".repeat(5000)
    );
    assert!(long.len() > 4096 && long.len() < 5100);
    let bad = [
        "",
        "nonsense",
        "row:0.180([scripts*],col:0.600([code*],[console*,problems,agent])",
        "row:0.180([scripts*],col:0.600([console*],[problems,agent]))",
        "row:0.180([scripts*],col:0.600([code*],[console*,problems]))",
        &deep,
        &long,
        "row:0.180([scripts*],col:0.600([code*],[console*,problems,agent]))x",
        "row:0.180([scripts*],col:0.600([code*],[]))",
        "row:0.180([scripts*],col:0.600([code*],[console*,,agent]))",
        "row:0.180([scripts*],col:0.600([code*],[console*,problems,agent]),[docs])",
        "[code*,scripts,console,problems,agent",
    ];
    for text in bad {
        assert_eq!(Dock::parse(text), None, "{text:?}");
    }
    // Ratios are digits: Rust's float parsing would take the rest, and NaN
    // survives clamping, so it would reach every rect.
    let bad_ratios: Vec<String> = [
        "nan",
        "NaN",
        "inf",
        "1e40",
        "-0.5",
        "0.5e1",
        "+0.5",
        "1.",
        ".5",
        &"9".repeat(50),
    ]
    .iter()
    .map(|ratio| DEFAULT.replacen("0.180", ratio, 1))
    .collect();
    for text in &bad_ratios {
        assert_eq!(Dock::parse(text), None, "{text}");
    }
    // Splits nested eight deep read (those around the unknown groups go);
    // nine deep don't.
    let mut nested = String::from("[code*]");
    let rest = [Scripts, Console, Problems, Agent, Library, Docs];
    for p in rest {
        nested = format!("row:0.5({nested},[{}])", p.id());
    }
    nested = format!("col:0.5(col:0.5({nested},[bogus]),[bogus])");
    assert!(Dock::parse(&nested).is_some(), "eight deep: {nested}");
    assert!(
        Dock::parse(&format!("col:0.5({nested},[bogus])")).is_none(),
        "nine deep"
    );
    // Damage that heals: repeats and unknown panes dropped, a ratio kept
    // in range, an empty group's split gone, whitespace ignored.
    let healed = DEFAULT.replace("[code*]", "[code*,code,bogus]");
    assert_eq!(Dock::parse(&healed), Some(Dock::default()));
    let seven = Dock::parse(&DEFAULT.replacen("0.180", "7", 1)).unwrap();
    assert_eq!(seven.ratio(1), Some(0.95));
    let bogus = DEFAULT.replace("[code*]", "row:0.5([code*],[bogus])");
    assert_eq!(Dock::parse(&bogus), Some(Dock::default()));
    let spaced = " row : 0.180 ( [ scripts* ] ,\n col:0.600([code*],\t[console*, problems, agent]))\n";
    assert_eq!(Dock::parse(spaced), Some(Dock::default()));
    let unstarred = Dock::parse(&DEFAULT.replace("console*", "console")).unwrap();
    assert!(
        unstarred.is_front(Console),
        "a group without a * has its first tab at the front"
    );
    let twice = Dock::parse(&DEFAULT.replace("[console*,problems,agent]", "[console,problems*,agent*]")).unwrap();
    assert!(twice.is_front(Problems), "the first *");
    let hidden_code = Dock::parse(&DEFAULT.replace("[code*]", "-[code*]")).unwrap();
    assert_eq!(hidden_code, Dock::default(), "the code's group never hides");
    assert_eq!(
        Dock::parse(&DEFAULT.replace("[scripts*]", "[scripts*,code]"))
            .unwrap()
            .leaf_tabs(Code),
        [Scripts, Code],
        "the first place a pane is named keeps it"
    );

    // The file: no file, another version, one too big to be read, or a
    // damaged dock gives the default with the tip showing.
    let dir = Folder::new("damaged");
    let file = dir.0.join("layout.txt");
    assert_eq!(load(&dir.0), (Dock::default(), true), "no file");
    let mut moved = Dock::default();
    assert!(moved.move_pane(
        Agent,
        DropAt::Beside {
            anchor: Code,
            side: Side::Right
        }
    ));
    save(&dir.0, &moved, false);
    assert_eq!(load(&dir.0), (moved.clone(), false));
    let written = std::fs::read_to_string(&file).unwrap();
    assert_eq!(written, file_text(&moved, false));
    assert_eq!(
        written,
        format!(
            "# Horizon Studio's Editor panes. Delete this file for the default.\nversion=1\ndock={}\ntips=0\n",
            moved.to_text()
        )
    );
    assert!(
        !file_text(&moved, true).contains("tips"),
        "the tip showing is the default"
    );
    save(&dir.0, &moved, true);
    assert_eq!(load(&dir.0), (moved.clone(), true));
    std::fs::write(&file, format!("future=1\nversion = 1\n\ndock = {}\n", moved.to_text())).unwrap();
    assert_eq!(
        load(&dir.0),
        (moved.clone(), true),
        "unknown keys and spaces are no harm"
    );
    let big = format!(
        "version=1\ndock={}\ntips=0\n{}",
        moved.to_text(),
        format!("# {}\n", "x".repeat(97)).repeat(205)
    );
    assert!(big.len() > 20 * 1024, "{}", big.len());
    let mut damaged: Vec<String> = bad
        .iter()
        .copied()
        .chain(bad_ratios.iter().map(String::as_str))
        .map(|dock| format!("version=1\ndock={dock}\ntips=0\n"))
        .collect();
    damaged.extend([
        "garbage".to_owned(),
        format!("version=2\ndock={}\ntips=0\n", moved.to_text()),
        format!("dock={}\n", moved.to_text()),
        format!(
            "version=1\ndock={}\ntips=0\n",
            moved.to_text().replacen("0.180", "nan", 1)
        ),
        "version=1\ntips=0\n".to_owned(),
        big,
    ]);
    for text in damaged {
        std::fs::write(&file, &text).unwrap();
        assert_eq!(
            load(&dir.0),
            (Dock::default(), true),
            "{:?}",
            &text[..text.len().min(80)]
        );
    }
    std::fs::write(&file, [0xff, 0xfe, 0x00, b'\n']).unwrap();
    assert_eq!(load(&dir.0), (Dock::default(), true), "not text");
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap();
    assert_eq!(load(&dir.0), (Dock::default(), true), "a folder of that name");
    // A studio that never started has no folder: nothing is read or
    // written (the tests' working folder is the repository).
    save(Path::new(""), &moved, false);
    assert!(!Path::new("layout.txt").exists());
    assert_eq!(load(Path::new("")), (Dock::default(), true));
}

#[test]
fn a_pane_dropped_beside_another_splits_it() {
    let mut d = Dock::default();
    assert!(d.move_pane(
        Agent,
        DropAt::Beside {
            anchor: Code,
            side: Side::Right
        }
    ));
    assert_eq!(
        d.to_text(),
        "row:0.180([scripts*],col:0.600(row:0.500([code*],[agent*]),[console*,problems]))"
    );
    let (_, placed) = d.layout(FULL, Code);
    assert_eq!(leaf_of(&placed, Code).rect, [210.0, 80.0, 347.0, 240.0]);
    assert_eq!(leaf_of(&placed, Agent).rect, [565.0, 80.0, 347.0, 240.0]);
    let mut d = Dock::default();
    assert!(d.move_pane(
        Agent,
        DropAt::Beside {
            anchor: Code,
            side: Side::Top
        }
    ));
    assert_eq!(
        d.to_text(),
        "row:0.180([scripts*],col:0.600(col:0.500([agent*],[code*]),[console*,problems]))"
    );
    let mut d = Dock::default();
    assert!(d.move_pane(
        Problems,
        DropAt::Beside {
            anchor: Scripts,
            side: Side::Bottom
        }
    ));
    assert_eq!(
        d.to_text(),
        "row:0.180(col:0.500([scripts*],[problems*]),col:0.600([code*],[console*,agent]))"
    );
    // A pane named for its own group splits it, when it has other tabs.
    let mut d = Dock::default();
    assert!(d.move_pane(
        Console,
        DropAt::Beside {
            anchor: Console,
            side: Side::Left
        }
    ));
    assert_eq!(
        d.to_text(),
        "row:0.180([scripts*],col:0.600([code*],row:0.500([console*],[problems*,agent])))"
    );
}

#[test]
fn a_pane_dropped_among_a_groups_tabs_joins_it_there() {
    let mut d = Dock::default();
    assert!(d.move_pane(Agent, DropAt::Into { anchor: Console, at: 0 }));
    assert_eq!(d.leaf_tabs(Console), [Agent, Console, Problems]);
    assert!(d.is_front(Agent));
    // Among its own group's tabs: reordered.
    let mut d = Dock::default();
    assert!(d.move_pane(Console, DropAt::Into { anchor: Console, at: 2 }));
    assert_eq!(d.leaf_tabs(Console), [Problems, Agent, Console]);
    assert!(d.is_front(Console));
    // A place past the end is the end, and a hidden group shows.
    let mut d = Dock::default();
    assert!(d.set_hidden(Agent, true));
    assert!(d.move_pane(Code, DropAt::Into { anchor: Agent, at: 9 }));
    assert_eq!(d.leaf_tabs(Code), [Console, Problems, Agent, Code]);
    assert!(d.is_front(Code));
    assert_eq!(d.to_text(), "row:0.180([scripts*],[console,problems,agent,code*])");
}

#[test]
fn an_emptied_group_closes_up() {
    let mut d = Dock::default();
    assert!(d.move_pane(Scripts, DropAt::Into { anchor: Console, at: 3 }));
    assert_eq!(d.to_text(), "col:0.600([code*],[console,problems,agent,scripts*])");
    let (bars, placed) = d.layout(FULL, Code);
    assert_eq!(leaf_of(&placed, Code).rect, [48.0, 80.0, 864.0, 240.0]);
    assert_eq!(bars.len(), 1);
    assert_eq!(bars[0].path, 1);
    // The front tab that leaves gives the front to the tab before it.
    let mut d = Dock::default();
    assert!(d.move_pane(Problems, DropAt::Into { anchor: Code, at: 1 }));
    assert!(d.move_pane(
        Agent,
        DropAt::Into {
            anchor: Problems,
            at: 2
        }
    ));
    assert!(d.move_pane(Agent, DropAt::Edge(Side::Right)));
    assert!(d.is_front(Problems), "{}", d.to_text());
}

#[test]
fn edges_wrap_the_whole_tab() {
    let mut d = Dock::default();
    assert!(d.move_pane(Console, DropAt::Edge(Side::Bottom)));
    assert_eq!(
        d.to_text(),
        "col:0.750(row:0.180([scripts*],col:0.600([code*],[problems*,agent])),[console*])"
    );
    let (_, placed) = d.layout(FULL, Code);
    assert_eq!(
        leaf_of(&placed, Console).rect,
        [48.0, 368.0, 864.0, 120.0],
        "the whole width"
    );
    // A closed pane comes in.
    let mut d = Dock::default();
    assert!(d.move_pane(Docs, DropAt::Edge(Side::Right)));
    assert_eq!(d.to_text(), format!("row:0.750({DEFAULT},[docs*])"));
    let (_, placed) = d.layout(FULL, Code);
    assert_eq!(
        leaf_of(&placed, Docs).rect,
        [672.0, 80.0, 240.0, 408.0],
        "the whole height"
    );
    let mut d = Dock::default();
    assert!(d.move_pane(Agent, DropAt::Edge(Side::Left)));
    assert_eq!(
        d.to_text(),
        "row:0.250([agent*],row:0.180([scripts*],col:0.600([code*],[console*,problems])))"
    );
}

#[test]
fn drops_that_change_nothing_are_refused() {
    let mut d = Dock::default();
    assert!(d.set_ratio(1, 0.3));
    let before = d.clone();
    assert!(!d.move_pane(Scripts, DropAt::Into { anchor: Scripts, at: 0 }));
    assert!(
        !d.move_pane(Scripts, DropAt::Edge(Side::Left)),
        "the shape is kept, so its ratio isn't turned into a quarter"
    );
    assert!(!d.move_pane(
        Scripts,
        DropAt::Beside {
            anchor: Scripts,
            side: Side::Right
        }
    ));
    assert!(!d.move_pane(Console, DropAt::Into { anchor: Console, at: 0 }));
    assert!(!d.move_pane(
        Console,
        DropAt::Into {
            anchor: Problems,
            at: 0
        }
    ));
    assert!(
        !d.move_pane(Agent, DropAt::Into { anchor: Docs, at: 0 }),
        "Docs is closed"
    );
    assert!(!d.move_pane(
        Agent,
        DropAt::Beside {
            anchor: Library,
            side: Side::Top
        }
    ));
    assert!(!d.move_pane(Docs, DropAt::Into { anchor: Docs, at: 0 }));
    assert_eq!(d, before);
    assert_eq!(d.ratio(1), Some(0.3));
}

#[test]
fn bars_stop_at_the_least_sizes() {
    let d = Dock::default();
    let (bars, _) = d.layout(FULL, Code);
    let col = bars.iter().find(|b| b.path == 3).unwrap();
    assert_eq!(col.lo - FULL[1], 140.0, "the code keeps 140");
    assert_eq!(FULL[1] + FULL[3] - (col.hi + 8.0), 120.0, "the panel keeps 120");
    assert_eq!((col.start, col.len), (80.0, 400.0));
    let row = bars.iter().find(|b| b.path == 1).unwrap();
    assert_eq!(row.lo - FULL[0], 140.0, "the script list keeps 140");
    assert_eq!(FULL[0] + FULL[2] - (row.hi + 8.0), 300.0, "the code keeps 300");
    // Ratios at their ends still lay the panes out at their least.
    let mut d = Dock::default();
    assert!(d.set_ratio(3, 0.05));
    assert!(d.set_ratio(1, 0.95));
    let (_, placed) = d.layout(FULL, Code);
    assert_eq!(leaf_of(&placed, Scripts).rect, [48.0, 80.0, 556.0, 408.0]);
    assert_eq!(leaf_of(&placed, Code).rect, [612.0, 80.0, 300.0, 140.0]);
    assert!(d.set_ratio(3, 0.95));
    let (_, placed) = d.layout(FULL, Code);
    assert_eq!(leaf_of(&placed, Console).rect, [612.0, 368.0, 300.0, 120.0]);
}

#[test]
fn a_group_keeps_room_for_its_tabs() {
    // Each tab at its least: its label cut to 28 units, its padding and the
    // gap after it (54), and what follows its label, the problems' count
    // (30) and the agent's or the code's dot (14); then the strip's margins
    // (16) and its × (30, which the code's group hasn't). Where that is more
    // than its panes' least, the bar stops there.
    let room = |text: &str, p: Pane| {
        let d = Dock::parse(text).unwrap();
        leaf_of(&d.layout(FULL, Code).1, p).rect[2]
    };
    assert_eq!(
        room(
            "row:0.950(row:0.500(-[scripts*],[code*]),[console*,problems,agent])",
            Console
        ),
        252.0
    );
    assert_eq!(
        room(
            "row:0.950([code*],[console*,problems,agent,scripts,library,docs])",
            Console
        ),
        414.0
    );
    assert_eq!(
        room(
            "row:0.050([code*,console,problems,agent,scripts],[library*,docs])",
            Code
        ),
        344.0
    );
    assert_eq!(
        room("row:0.950([code*],[agent*,scripts,console,problems])", Agent),
        306.0
    );
    // Fewer tabs: the panes' own least.
    assert_eq!(
        room("row:0.950([code*,console,problems,agent],[library*,scripts])", Library),
        350.0
    );
}

#[test]
fn a_cramped_split_keeps_the_pane_in_use_and_a_strip() {
    let d = Dock::default();
    let ws = [0.0, 0.0, 600.0, 200.0];
    let (bars, placed) = d.layout(ws, Code);
    assert_eq!(leaf_of(&placed, Scripts).rect, [0.0, 0.0, 140.0, 200.0]);
    let code = leaf_of(&placed, Code);
    assert_eq!((code.rect, code.collapsed), ([148.0, 0.0, 452.0, 160.0], false));
    let panel = leaf_of(&placed, Console);
    assert_eq!((panel.rect, panel.collapsed), ([148.0, 168.0, 452.0, 32.0], true));
    assert_eq!(
        bars.iter().map(|b| b.path).collect::<Vec<_>>(),
        [1],
        "no bar for the column"
    );
    // With the keys in the panel, it takes the room and the code keeps its strip.
    let (_, placed) = d.layout(ws, Console);
    let panel = leaf_of(&placed, Agent);
    assert_eq!((panel.rect, panel.collapsed), ([148.0, 40.0, 452.0, 160.0], false));
    let code = leaf_of(&placed, Code);
    assert_eq!((code.rect, code.collapsed), ([148.0, 0.0, 452.0, 32.0], true));
    // Under 140 + 8 + 32 high there is no room for the strip either.
    let (_, placed) = d.layout([0.0, 0.0, 600.0, 170.0], Code);
    assert!(!placed_has(&placed, Console));
    assert_eq!(leaf_of(&placed, Code).rect, [148.0, 0.0, 452.0, 170.0]);
    let (_, placed) = d.layout([0.0, 0.0, 600.0, 179.0], Code);
    assert!(!placed_has(&placed, Console), "179 isn't");
    let (_, placed) = d.layout([0.0, 0.0, 600.0, 180.0], Code);
    assert!(leaf_of(&placed, Console).collapsed, "180 is enough");
    assert_eq!(leaf_of(&placed, Code).rect, [148.0, 0.0, 452.0, 140.0]);
    // Side by side, a group that gives way leaves nothing.
    let (_, placed) = d.layout([0.0, 0.0, 400.0, 400.0], Code);
    assert!(!placed_has(&placed, Scripts));
    assert_eq!(leaf_of(&placed, Code).rect, [0.0, 0.0, 400.0, 235.0]);
    // Neither side with the keys: the code's.
    let (_, placed) = d.layout([0.0, 0.0, 400.0, 400.0], Library);
    assert!(!placed_has(&placed, Scripts));
    // bodies() leaves out collapsed groups, and bodies under a line.
    let b = d.bodies(ws, Code);
    assert_eq!(
        PANES.map(|p| b[p.index()]),
        [true, true, false, false, false, false, false]
    );
    let b = d.bodies(ws, Agent);
    assert_eq!(
        PANES.map(|p| b[p.index()]),
        [true, false, true, false, false, false, false],
        "the console is its group's front pane"
    );
    let body = |active: Pane, h: f32| {
        Placed {
            rect: [0.0, 0.0, 300.0, h],
            tabs: vec![active],
            active,
            collapsed: false,
        }
        .has_body()
    };
    assert!(
        body(Code, 51.0) && !body(Code, 50.0),
        "the code's body is the group less the strip and a unit"
    );
    assert!(
        body(Console, 60.0) && !body(Console, 59.0),
        "the panel's less the strip and 10"
    );
    assert_eq!(Code.body([10.0, 20.0, 300.0, 200.0]), [11.0, 52.0, 298.0, 167.0]);
    assert_eq!(Agent.body([10.0, 20.0, 300.0, 200.0]), [24.0, 57.0, 272.0, 158.0]);
}

#[test]
fn hidden_groups_give_their_room_away() {
    let mut d = Dock::default();
    assert!(d.set_hidden(Console, true));
    assert!(!d.set_hidden(Agent, true), "already hidden, with its group");
    let (bars, placed) = d.layout(FULL, Code);
    assert_eq!(leaf_of(&placed, Code).rect, [210.0, 80.0, 702.0, 408.0]);
    assert!(!placed_has(&placed, Console));
    assert_eq!(bars.iter().map(|b| b.path).collect::<Vec<_>>(), [1]);
    assert!(!d.shown(Problems) && d.has(Problems));
    assert_eq!(d.settled(Console), Code);
    let mut code = Dock::default();
    assert!(!code.set_hidden(Code, true), "the code's group never hides");
    assert!(code.move_pane(Agent, DropAt::Into { anchor: Code, at: 1 }));
    assert!(!code.set_hidden(Agent, true), "nor does a pane sharing it");
    assert!(d.show(Problems));
    assert!(d.shown(Console) && d.is_front(Problems));
    assert!(!d.show(Problems), "shown already");
    // A split with a hidden side counts no bar for it: 140 + 8 + 300.
    let d = Dock {
        root: split(
            true,
            0.3,
            group(&[Scripts]),
            split(
                true,
                0.5,
                group(&[Code]),
                Node::Leaf(Leaf {
                    tabs: vec![Agent],
                    active: Agent,
                    hidden: true,
                }),
            ),
        ),
    };
    for (w, code) in [(448.0, [148.0, 0.0, 300.0, 300.0]), (450.0, [148.0, 0.0, 302.0, 300.0])] {
        let (bars, placed) = d.layout([0.0, 0.0, w, 300.0], Code);
        assert_eq!(leaf_of(&placed, Scripts).rect, [0.0, 0.0, 140.0, 300.0], "{w}");
        assert_eq!(leaf_of(&placed, Code).rect, code, "{w}");
        assert_eq!(bars.len(), 1, "{w}");
    }
    let (_, placed) = d.layout([0.0, 0.0, 447.0, 300.0], Code);
    assert!(!placed_has(&placed, Scripts));
    assert_eq!(leaf_of(&placed, Code).rect, [0.0, 0.0, 447.0, 300.0]);
}

#[test]
fn only_library_and_docs_close_and_they_come_back_home() {
    let default = Dock::default();
    let mut d = default.clone();
    for p in [Scripts, Code, Console, Problems, Agent] {
        assert!(!d.close(p), "{p:?}");
    }
    assert!(!d.close(Docs), "not docked");
    assert_eq!(d, default);
    // The Docs' home is a column at the right, and the Library's is with
    // the Docs.
    assert!(d.show(Docs));
    assert_eq!(d.to_text(), format!("row:0.700({DEFAULT},[docs*])"));
    assert!(d.show(Library));
    assert_eq!(d.leaf_tabs(Docs), [Docs, Library]);
    assert!(d.is_front(Library));
    assert!(d.close(Library));
    assert!(d.is_front(Docs), "the tab before it");
    assert!(d.close(Docs));
    assert_eq!(d, default);
    // Where that column would push the code's out, the Library goes among
    // the code's tabs, at the front.
    let mut d = default.clone();
    assert!(d.show_fitting(Library, Some([0.0, 0.0, 715.0, 291.0]), Code));
    assert_eq!(d.leaf_tabs(Code), [Code, Library]);
    assert!(d.is_front(Library));
    let mut d = default.clone();
    assert!(d.show_fitting(Library, Some([0.0, 0.0, 1200.0, 600.0]), Code));
    assert_eq!(d.to_text(), format!("row:0.700({DEFAULT},[library*])"));
    let (_, placed) = d.layout([0.0, 0.0, 1200.0, 600.0], Library);
    assert_eq!(leaf_of(&placed, Library).rect, [842.0, 0.0, 358.0, 600.0]);
    let mut d = default.clone();
    assert!(d.show_fitting(Library, None, Code), "never drawn: its home");
    assert_eq!(d.to_text(), format!("row:0.700({DEFAULT},[library*])"));
    // A pane in the dock is only brought to the front.
    let mut d = default.clone();
    assert!(d.show_fitting(Agent, Some([0.0, 0.0, 100.0, 100.0]), Code));
    assert!(d.is_front(Agent));
    assert_eq!(d.leaf_tabs(Agent), [Console, Problems, Agent]);
}

#[test]
fn drop_at_reads_the_pointer() {
    let d = Dock::default();
    let leaves = drawn(&d, FULL, Code);
    assert_eq!(leaves.len(), 3);
    let problems_tab = leaves[2].tabs[1].1;
    assert_eq!(problems_tab, [300.0, 331.0, 80.0, 26.0]);
    let spot = |dragged: Pane, at: (f32, f32)| d.drop_at(FULL, &leaves, dragged, Code, at);
    // Over the left half of the Problems tab: before it.
    let s = spot(Agent, (310.0, 340.0)).unwrap();
    assert_eq!(s.at, DropAt::Into { anchor: Console, at: 1 });
    assert_eq!(s.rect, [210.0, 328.0, 702.0, 160.0]);
    assert_eq!(
        s.bar,
        Some([299.0, 332.0, 2.0, 24.0]),
        "a bar at the Problems tab's left edge"
    );
    // Past the tabs: after the last other one.
    let s = spot(Agent, (700.0, 340.0)).unwrap();
    assert_eq!(s.at, DropAt::Into { anchor: Console, at: 2 });
    assert_eq!(s.bar, Some([379.0, 332.0, 2.0, 24.0]));
    // The code's right quarter: beside it, the preview where Agent lands.
    let s = spot(Agent, (890.0, 200.0)).unwrap();
    assert_eq!(
        s.at,
        DropAt::Beside {
            anchor: Code,
            side: Side::Right
        }
    );
    assert_eq!((s.rect, s.bar), ([565.0, 80.0, 347.0, 240.0], None));
    let s = spot(Agent, (560.0, 130.0)).unwrap();
    assert_eq!(
        s.at,
        DropAt::Beside {
            anchor: Code,
            side: Side::Top
        }
    );
    // Its middle: among its tabs, last.
    let s = spot(Agent, (560.0, 200.0)).unwrap();
    assert_eq!(s.at, DropAt::Into { anchor: Code, at: 1 });
    assert_eq!((s.rect, s.bar), ([210.0, 80.0, 702.0, 240.0], None));
    // Along the bottom: a row the whole width.
    let s = spot(Agent, (560.0, 478.0)).unwrap();
    assert_eq!(s.at, DropAt::Edge(Side::Bottom));
    assert_eq!(s.rect, [48.0, 368.0, 864.0, 120.0]);
    let s = spot(Console, (906.0, 200.0)).unwrap();
    assert_eq!(s.at, DropAt::Edge(Side::Right));
    // A bar's gap, or outside: nowhere.
    assert_eq!(spot(Agent, (206.0, 200.0)), None);
    assert_eq!(spot(Agent, (500.0, 324.0)), None);
    assert_eq!(spot(Agent, (20.0, 20.0)), None);
    assert_eq!(spot(Agent, (950.0, 300.0)), None);
    // The script list over its own group, its strip or the edge it fills.
    assert_eq!(spot(Scripts, (120.0, 300.0)), None);
    assert_eq!(spot(Scripts, (100.0, 90.0)), None);
    assert_eq!(spot(Scripts, (50.0, 300.0)), None);
    // It joins another group's tabs (it takes no keys: the code keeps them).
    let s = spot(Scripts, (560.0, 340.0)).unwrap();
    assert_eq!(s.at, DropAt::Into { anchor: Console, at: 3 });
    assert_eq!(s.rect, [48.0, 328.0, 864.0, 160.0]);
    // A lone group has no other tab to name it by.
    let mut alone = Dock::default();
    assert!(alone.move_pane(
        Agent,
        DropAt::Beside {
            anchor: Code,
            side: Side::Right
        }
    ));
    let leaves = drawn(&alone, FULL, Code);
    let agent = leaf_of(&alone.layout(FULL, Code).1, Agent).rect;
    let middle = (agent[0] + agent[2] / 2.0, agent[1] + agent[3] / 2.0);
    for at in [
        middle,
        (agent[0] + 10.0, agent[1] + 10.0),
        (agent[0] + 30.0, agent[1] + 100.0),
    ] {
        assert_eq!(alone.drop_at(FULL, &leaves, Agent, Code, at), None, "{at:?}");
    }

    // In an 854 x 480 game's window there is no room beside the code: the
    // drop goes among its tabs, and the preview says so.
    let leaves = drawn(&d, SMALL, Code);
    assert_eq!(
        leaves.iter().map(|l| (l.rect, l.collapsed)).collect::<Vec<_>>(),
        [
            ([0.0, 0.0, 140.0, 243.0], false),
            ([148.0, 0.0, 484.0, 203.0], false),
            ([148.0, 211.0, 484.0, 32.0], true),
        ]
    );
    let s = d.drop_at(SMALL, &leaves, Agent, Code, (600.0, 120.0)).unwrap();
    assert_eq!(s.at, DropAt::Into { anchor: Code, at: 1 });
    assert_eq!((s.rect, s.bar), ([148.0, 0.0, 484.0, 203.0], None));
    let mut beside = d.clone();
    assert!(beside.move_pane(
        Agent,
        DropAt::Beside {
            anchor: Code,
            side: Side::Right
        }
    ));
    assert!(
        !fits(&d, &beside, SMALL, Code, Agent, Agent),
        "beside the code, Agent would push the script list out"
    );
    let mut into = d.clone();
    assert!(into.move_pane(Agent, s.at));
    assert!(fits(&d, &into, SMALL, Code, Agent, Agent));
    assert!(leaf_of(&into.layout(SMALL, Agent).1, Console).collapsed);
    // A collapsed group's strip takes tabs too. A pane with keys dropped
    // there takes the room, the code keeping its strip; the script list
    // would be only a tab there, with no body, so it doesn't land.
    assert_eq!(d.drop_at(SMALL, &leaves, Scripts, Code, (160.0, 220.0)), None);
    let mut shared = d.clone();
    assert!(shared.move_pane(Agent, DropAt::Into { anchor: Code, at: 1 }));
    let leaves = drawn(&shared, SMALL, Agent);
    assert!(leaves[2].collapsed);
    let s = shared.drop_at(SMALL, &leaves, Agent, Agent, (300.0, 220.0)).unwrap();
    assert_eq!(s.at, DropAt::Into { anchor: Console, at: 2 });
    assert_eq!(s.rect, [148.0, 40.0, 484.0, 203.0]);
    assert_eq!(s.bar, Some([317.0, 215.0, 2.0, 24.0]));
}

#[test]
fn covers_and_settled_name_what_a_change_hides() {
    let d = Dock::default();
    assert_eq!(d.covers(Agent), Some(Console));
    assert_eq!(d.covers(Scripts), None);
    assert_eq!(d.covers(Console), None);
    let mut d = Dock::default();
    assert!(d.move_pane(Agent, DropAt::Into { anchor: Code, at: 1 }));
    assert!(d.activate(Code));
    assert!(!d.activate(Code), "at the front already");
    assert_eq!(d.covers(Agent), Some(Code));
    assert_eq!(d.covers(Code), None);
    assert_eq!(d.covers(Docs), None, "a column of its own");
    assert!(d.show(Docs));
    assert_eq!(d.covers(Library), Some(Docs));
    let mut d = Dock::default();
    assert!(d.set_hidden(Console, true));
    assert_eq!(d.settled(Agent), Code);
    assert_eq!(d.settled(Library), Code, "closed");
    let mut d = Dock::default();
    assert!(d.move_pane(Scripts, DropAt::Into { anchor: Code, at: 1 }));
    assert_eq!(d.settled(Code), Scripts);
    assert_eq!(d.settled(Library), Scripts, "the front of the code's group");
}

/// A small generator of numbers, the same every run.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as u32
    }
    fn below(&mut self, n: usize) -> usize {
        self.next() as usize % n
    }
    fn pane(&mut self) -> Pane {
        PANES[self.below(PANES.len())]
    }
    fn side(&mut self) -> Side {
        [Side::Left, Side::Right, Side::Top, Side::Bottom][self.below(4)]
    }
}

fn leaves_in<'a>(node: &'a Node, out: &mut Vec<&'a Leaf>) {
    match node {
        Node::Leaf(l) => out.push(l),
        Node::Split { ratio, a, b, .. } => {
            assert!(
                (0.05..=0.95).contains(ratio) && (ratio * 1000.0).round() / 1000.0 == *ratio,
                "{ratio}"
            );
            leaves_in(a, out);
            leaves_in(b, out);
        }
    }
}

fn overlap(a: [f32; 4], b: [f32; 4]) -> bool {
    let e = 0.01;
    a[0] < b[0] + b[2] - e && b[0] < a[0] + a[2] - e && a[1] < b[1] + b[3] - e && b[1] < a[1] + a[3] - e
}

#[test]
fn any_sequence_of_edits_keeps_the_layout_whole() {
    let sizes = [
        [0.0, 0.0, 296.0, 89.0],
        [10.0, 20.0, 632.0, 243.0],
        [0.0, 0.0, 640.0, 300.0],
        FULL,
        [0.0, 0.0, 1920.0, 1000.0],
        [3.5, 7.25, 813.33, 348.6],
    ];
    let mut rng = Lcg(0x5eed);
    let mut d = Dock::default();
    let (mut moves, mut drops) = (0, 0);
    for step in 0..500 {
        let p = rng.pane();
        match rng.below(10) {
            0..=2 => {
                let at = match rng.below(3) {
                    0 => DropAt::Into {
                        anchor: rng.pane(),
                        at: rng.below(6),
                    },
                    1 => DropAt::Beside {
                        anchor: rng.pane(),
                        side: rng.side(),
                    },
                    _ => DropAt::Edge(rng.side()),
                };
                moves += usize::from(d.move_pane(p, at));
            }
            3 => {
                d.show(p);
            }
            4 => {
                let closed = d.close(p);
                assert!(!closed || p.closable(), "{p:?} closed");
            }
            5 => {
                d.set_hidden(p, rng.below(3) > 0);
            }
            6 => {
                let r = [
                    f32::NAN,
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                    rng.below(1200) as f32 / 1000.0,
                ][rng.below(4)];
                let before = d.clone();
                let changed = d.set_ratio(rng.below(16) as u16, r);
                assert!(r.is_finite() || (!changed && d == before), "{r}");
            }
            7 => {
                d.activate(p);
            }
            8 if rng.below(8) == 0 => {
                d.reset();
            }
            _ => {
                // A drop where the pointer is, which lands where it says.
                let ws = sizes[rng.below(sizes.len())];
                let focus = d.settled(rng.pane());
                let leaves = drawn(&d, ws, focus);
                let at = (
                    ws[0] + rng.below(ws[2] as usize) as f32,
                    ws[1] + rng.below(ws[3] as usize) as f32,
                );
                if let Some(s) = d.drop_at(ws, &leaves, p, focus, at) {
                    let mut after = d.clone();
                    assert!(after.move_pane(p, s.at), "{s:?}");
                    let keep = if p.takes_keys() { p } else { after.settled(focus) };
                    assert!(fits(&d, &after, ws, focus, keep, p), "{s:?}");
                    assert_eq!(leaf_of(&after.layout(ws, keep).1, p).rect, s.rect);
                    d = after;
                    drops += 1;
                } else {
                    d.show_fitting(p, Some(ws), focus);
                }
            }
        }
        // Each pane once, the five always there, the code's group shown,
        // no empty group, the front one of its tabs, ratios in range.
        let mut leaves = Vec::new();
        leaves_in(&d.root, &mut leaves);
        let panes: Vec<Pane> = leaves.iter().flat_map(|l| l.tabs.clone()).collect();
        for p in PANES {
            assert!(
                panes.iter().filter(|q| **q == p).count() <= 1,
                "step {step}: {}",
                d.to_text()
            );
        }
        for p in [Scripts, Code, Console, Problems, Agent] {
            assert!(d.has(p), "step {step}: {p:?} in {}", d.to_text());
        }
        assert!(d.shown(Code), "step {step}: {}", d.to_text());
        assert!(leaves.iter().all(|l| l.tabs.contains(&l.active)));
        assert_eq!(Dock::parse(&d.to_text()).as_ref(), Some(&d), "step {step}");
        for ws in sizes {
            for keep in [Code, d.settled(rng.pane())] {
                let (bars, placed) = d.layout(ws, keep);
                assert!(
                    !leaf_of(&placed, keep).collapsed,
                    "step {step}: {keep:?} in {}",
                    d.to_text()
                );
                let mut rects: Vec<[f32; 4]> = placed.iter().map(|l| l.rect).collect();
                rects.extend(bars.iter().map(|b| b.rect));
                let e = 0.01;
                for r in &rects {
                    assert!(r.iter().all(|v| v.is_finite()), "{r:?}");
                    assert!(
                        r[0] >= ws[0] - e
                            && r[1] >= ws[1] - e
                            && r[0] + r[2] <= ws[0] + ws[2] + e
                            && r[1] + r[3] <= ws[1] + ws[3] + e,
                        "step {step}: {r:?} outside {ws:?} in {}",
                        d.to_text()
                    );
                }
                for (i, a) in rects.iter().enumerate() {
                    for b in &rects[i + 1..] {
                        assert!(!overlap(*a, *b), "step {step}: {a:?} and {b:?} in {}", d.to_text());
                    }
                }
                assert!(placed.iter().all(|l| !l.collapsed || l.rect[3] == STRIP));
                assert!(
                    bars.iter()
                        .all(|b| b.lo <= b.hi && b.lo.is_finite() && b.hi.is_finite())
                );
            }
        }
    }
    assert!(moves > 40 && drops > 10, "{moves} moves and {drops} drops");
}
