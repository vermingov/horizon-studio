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
