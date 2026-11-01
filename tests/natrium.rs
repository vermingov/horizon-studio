// Natrium's switches and its fog cut: which sections are wholly inside the fog.

use horizon_studio::natrium as n;

#[test]
fn a_section_is_cut_only_when_all_of_it_is_past_the_fog() {
    // Ahead along +X: its near face at 100 blocks, past a 96-block fog.
    assert!(n::past([100.0, -8.0, -8.0], 96.0));
    // Its near face at 90: some of it is still in view.
    assert!(!n::past([90.0, -8.0, -8.0], 96.0));
    // Within a block of the end: kept (the fog only covers all past it).
    assert!(!n::past([96.5, -8.0, -8.0], 96.0));
    // Behind the camera, the near face is the box's max side.
    assert!(n::past([-120.0, 0.0, 0.0], 96.0));
    assert!(!n::past([-100.0, 0.0, 0.0], 96.0), "its near face is at -84");
    // The section around the camera never is.
    assert!(!n::past([-8.0, -8.0, -8.0], 1.0));
    // Diagonally: the nearest corner counts, not the axes alone.
    assert!(n::past([70.0, 0.0, 70.0], 96.0), "the corner is 99 blocks away");
    assert!(!n::past([60.0, 0.0, 60.0], 96.0), "the corner is 85 blocks away");
}

#[test]
fn columns_and_tight_boxes_are_cut_by_their_nearest_point() {
    // A section's own box is `past` with its 16 blocks spelled out.
    assert!(n::past_box([100.0, -8.0, -8.0], [116.0, 8.0, 8.0], 96.0));
    assert!(!n::past_box([90.0, -8.0, -8.0], [106.0, 8.0, 8.0], 96.0));
    // A chunk column ahead, from the world's bottom to its top: its near face
    // decides, however tall it is.
    assert!(n::past_box([100.0, -128.0, -8.0], [116.0, 256.0, 8.0], 96.0));
    assert!(!n::past_box([90.0, -128.0, -8.0], [106.0, 256.0, 8.0], 96.0));
    // A tight mesh box within a section: only what is drawn counts.
    assert!(n::past_box([98.0, 0.0, 0.0], [99.0, 1.0, 1.0], 96.0));
    assert!(!n::past_box([96.0, 0.0, 0.0], [99.0, 1.0, 1.0], 96.0));
}

#[test]
fn a_column_reaches_the_fog_when_its_far_corner_does() {
    // The camera's column, 16 blocks wide, tall: its far top corner is past a
    // 32-block fog, so its sections are tested one by one.
    assert!(n::reaches([-8.0, -64.0, -8.0], [8.0, 64.0, 8.0], 32.0));
    // Low enough: wholly in front of the fog, nothing in it is cut.
    assert!(!n::reaches([-8.0, -8.0, -8.0], [8.0, 8.0, 8.0], 32.0));
    // Behind the camera, the far side is the box's min.
    assert!(n::reaches([-40.0, 0.0, 0.0], [-24.0, 1.0, 1.0], 32.0));
    assert!(!n::reaches([-30.0, 0.0, 0.0], [-14.0, 1.0, 1.0], 32.0));
    // What `past_box` cuts, `reaches` always sees.
    assert!(n::reaches([100.0, -8.0, -8.0], [116.0, 8.0, 8.0], 96.0));
}

#[test]
fn a_box_stands_for_its_height_in_sections() {
    assert_eq!(n::sections_in([0.0, 0.0, 0.0], [16.0, 16.0, 16.0]), 1);
    assert_eq!(
        n::sections_in([0.0, 3.0, 0.0], [16.0, 5.0, 16.0]),
        1,
        "a tight box is one"
    );
    assert_eq!(n::sections_in([0.0, -64.0, 0.0], [16.0, 320.0, 16.0]), 24);
}

#[test]
fn switches_and_the_fog_cut() {
    n::set(true, true, true);
    assert!(n::particles() && n::fog() && n::ticks());
    n::set_fog_cut(Some(96.0));
    assert_eq!(n::fog_cut(), Some(96.0));
    // Off, or no fog this frame: nothing is cut.
    n::set(true, false, true);
    assert_eq!(n::fog_cut(), None);
    n::set(true, true, true);
    n::set_fog_cut(None);
    assert_eq!(n::fog_cut(), None);
    n::set_fog_cut(Some(f32::NAN));
    assert_eq!(n::fog_cut(), None);
    // What was skipped adds up.
    let (p, s, t) = n::counts();
    n::skipped_particles(3);
    n::skipped_sections(2);
    n::saved_tick_work(5);
    assert_eq!(n::counts(), (p + 3, s + 2, t + 5));
}

#[test]
fn video_settings_reads_natriums_place_and_switches() {
    n::set_video(None);
    assert_eq!(n::video(), None);
    let switches = [true, false, true, false, true];
    n::set_video(Some((7, switches)));
    assert_eq!(n::video(), Some((7, switches)));
    n::set_video(Some((0, [false; 5])));
    assert_eq!(n::video(), Some((0, [false; 5])), "Natrium first, all off, still shown");
    n::set_video(Some((123_456, [true; 5])));
    assert_eq!(n::video(), Some((123_456, [true; 5])));
    n::set_video(None);
    assert_eq!(n::video(), None);
    // Its buttons' captions, the preset after them.
    assert_eq!(n::CAPTIONS.len(), n::PRESET);
    assert_eq!(n::CAPTIONS[4], "Block Face Culling");
}

#[test]
fn counts_read_short() {
    assert_eq!(n::short(0), "0");
    assert_eq!(n::short(999), "999");
    assert_eq!(n::short(1_250), "1.2k");
    assert_eq!(n::short(3_400_000), "3.4M");
    assert_eq!(n::short(5_600_000_000), "5.6G");
    assert!(n::skipped().starts_with("Skipped so far: "));
}
