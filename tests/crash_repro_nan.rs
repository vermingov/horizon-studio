// A damaged look.txt (rect=NaN,...) never draws the window at NaN.
mod common;
use horizon_studio::game::hstudio_ui::draw;

#[test]
fn a_nan_in_look_txt_draws_at_nan() {
    let mut game = common::FakeGame::new("zz-nan-look");
    std::fs::create_dir_all(&game.dir).unwrap();
    std::fs::write(game.dir.join("look.txt"), "theme=synapse\nrect=NaN,0,500,400\n").unwrap();
    let mut st = common::studio(&mut game);
    println!("look.rect {:?}", st.s.ui.look.rect);
    st.s.ui.open = true;
    st.s.now = 10.0;
    game.drawn.clear();
    draw(&mut st, &mut game);
    let bad: Vec<&String> = game
        .drawn
        .iter()
        .filter(|d| d.contains("[NaN") || d.contains(", NaN"))
        .collect();
    assert!(
        bad.is_empty(),
        "{} of {} records drawn at NaN, first {:?}",
        bad.len(),
        game.drawn.len(),
        bad.first()
    );
}
