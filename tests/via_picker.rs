// ViaHorizon's version picker on the multiplayer screen, drawn on a recording
// painter: what it shows, and what clicks, the wheel and keys do.

use horizon_studio::game::hstudio_via_picker::{Act, Info, Key, Paint, Picker, Tone, filtered, tag};

/// Records text with where it went; 7 dp per character.
#[derive(Default)]
struct Rec {
    texts: Vec<(String, f32, f32)>,
}

impl Paint for Rec {
    fn rect(&mut self, _: [f32; 4], _: f32, _: u32) {}
    fn border(&mut self, _: [f32; 4], _: f32, _: u32) {}
    fn text(&mut self, s: &str, x: f32, y: f32, _: f32, _: bool, _: u32) -> f32 {
        self.texts.push((s.to_owned(), x, y));
        self.measure(s, 0.0, false)
    }
    fn measure(&mut self, s: &str, _: f32, _: bool) -> f32 {
        7.0 * s.chars().count() as f32
    }
}

impl Rec {
    fn at(&self, s: &str) -> Option<(f32, f32)> {
        self.texts.iter().find(|t| t.0 == s).map(|t| (t.1 + 3.0, t.2 + 8.0))
    }
    fn has(&self, s: &str) -> bool {
        self.texts.iter().any(|t| t.0 == s)
    }
}

fn info() -> Info {
    let ids = [
        "auto",
        "26.3",
        "1.21.4",
        "1.20.5-1.20.6",
        "1.16.4-1.16.5",
        "1.12.2",
        "1.12.1",
        "1.8.x",
        "b1.7-b1.7.3",
        "c0.30 CPE",
        "Bedrock 1.26.51",
    ];
    Info {
        enabled: true,
        version: "1.8.x".into(),
        status: "Ready (ViaProxy 3.4.14, Java 21)".into(),
        tone: Tone::Ready,
        versions: ids
            .iter()
            .map(|v| {
                (
                    (*v).into(),
                    if *v == "auto" {
                        "Auto detect".into()
                    } else {
                        (*v).into()
                    },
                )
            })
            .collect(),
    }
}

/// The picker drawn at time `now` on a 960 x 540 dp screen.
fn frame(p: &mut Picker, info: &Info, now: f64) -> Rec {
    let mut rec = Rec::default();
    p.draw(&mut rec, info, 960.0, 540.0, now);
    rec
}

#[test]
fn versions_filter_by_name_and_kind() {
    let i = info();
    assert_eq!(filtered(&i.versions, "").len(), i.versions.len());
    let names = |f: &str| {
        filtered(&i.versions, f)
            .iter()
            .map(|&k| i.versions[k].0.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names("1.12"), vec!["1.12.2", "1.12.1"]);
    assert_eq!(names("BETA"), vec!["b1.7-b1.7.3"]);
    assert_eq!(names("classic"), vec!["c0.30 CPE"]);
    assert_eq!(names("auto"), vec!["auto"]);
    assert_eq!(tag("Bedrock 1.26.51"), "experimental");
    assert_eq!(tag("1.8.x"), "");
}

#[test]
fn it_opens_on_the_chosen_version_and_picks_with_a_click() {
    let i = info();
    let mut p = Picker::default();
    assert!(!p.visible(0.0));
    p.show(0.0, &i);
    assert!(p.is_open());
    let rec = frame(&mut p, &i, 1.0);
    for t in [
        "ViaHorizon",
        "Join servers through ViaProxy",
        "Ready (ViaProxy 3.4.14, Java 21)",
        "1.8.x",
        "Beta",
    ] {
        assert!(rec.has(t), "{t} not drawn: {:?}", rec.texts);
    }
    // Scrolled so the chosen version shows, near the top.
    assert!(!rec.has("Auto detect"));
    // A click on a version picks it and closes, with its exit drawn first.
    let at = rec.at("1.12.2").unwrap();
    p.point(at);
    assert_eq!(p.click(1.1, &i), Act::Pick("1.12.2".into()));
    assert!(!p.is_open());
    assert!(p.visible(1.15) && !p.visible(1.4));
}

#[test]
fn the_switch_toggles_and_outside_closes() {
    let i = info();
    let mut p = Picker::default();
    p.show(0.0, &i);
    let rec = frame(&mut p, &i, 1.0);
    p.point(rec.at("Join servers through ViaProxy").unwrap());
    assert_eq!(p.click(1.0, &i), Act::Toggle);
    assert!(p.is_open(), "the switch keeps it open");
    p.point((5.0, 5.0));
    assert_eq!(p.click(1.0, &i), Act::None);
    assert!(!p.is_open(), "a click outside the card closes it");
    // So does the close button, and Escape.
    p.show(2.0, &i);
    let rec = frame(&mut p, &i, 3.0);
    p.point(rec.at("×").unwrap());
    assert_eq!(p.click(3.0, &i), Act::None);
    assert!(!p.is_open());
    p.show(4.0, &i);
    assert_eq!(p.key(4.0, Key::Escape, &i), Act::None);
    assert!(!p.is_open());
}

#[test]
fn typing_searches_and_enter_picks() {
    let i = info();
    let mut p = Picker::default();
    p.show(0.0, &i);
    frame(&mut p, &i, 1.0);
    p.key(1.0, Key::Text("1.1".into()), &i);
    p.key(1.0, Key::Text("2".into()), &i);
    assert_eq!(p.filter, "1.12");
    let rec = frame(&mut p, &i, 1.1);
    assert!(
        rec.has("1.12.2") && rec.has("1.12.1") && !rec.has("1.8.x"),
        "{:?}",
        rec.texts
    );
    assert!(rec.has("1.12"), "the search shows");
    p.key(1.1, Key::Down, &i);
    assert_eq!(p.key(1.1, Key::Enter, &i), Act::Pick("1.12.1".into()));
    // Nothing matches: it says so; Backspace widens it again.
    p.show(2.0, &i);
    p.key(2.0, Key::Text("zz".into()), &i);
    let rec = frame(&mut p, &i, 3.0);
    assert!(rec.has("No version matches \"zz\""), "{:?}", rec.texts);
    p.key(3.0, Key::Backspace, &i);
    p.key(3.0, Key::Backspace, &i);
    assert!(frame(&mut p, &i, 3.1).has("26.3"));
}

#[test]
fn the_list_scrolls_with_the_wheel_and_the_keys() {
    let mut i = info();
    i.version = "auto".into();
    for n in 0..60 {
        i.versions.push((format!("x{n}"), format!("Extra {n}")));
    }
    let mut p = Picker::default();
    p.show(0.0, &i);
    let rec = frame(&mut p, &i, 1.0);
    assert!(rec.has("Auto detect"));
    p.wheel(-3.0, &i);
    let rec = frame(&mut p, &i, 1.1);
    assert!(!rec.has("Auto detect"), "scrolled down: {:?}", rec.texts);
    p.wheel(10.0, &i);
    assert!(frame(&mut p, &i, 1.2).has("Auto detect"), "back at the top");
    // Down past the last row shown scrolls it into view.
    for _ in 0..30 {
        p.key(1.2, Key::Down, &i);
    }
    let rec = frame(&mut p, &i, 1.3);
    let focus = &i.versions[filtered(&i.versions, "")[30]].1;
    assert!(rec.has(focus), "{focus}: {:?}", rec.texts);
}

#[test]
fn an_off_viahorizon_shows_its_switch_off() {
    let mut i = info();
    i.enabled = false;
    i.tone = Tone::Off;
    i.status = "Off, joins go straight to servers".into();
    let mut p = Picker::default();
    p.show(0.0, &i);
    assert!(frame(&mut p, &i, 1.0).has("Off, joins go straight to servers"));
}
