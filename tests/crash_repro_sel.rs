// A console selection in an executor style, its lines drawn twice, never cut inside a letter.
// console's lines are drawn twice (the Console page, and the style's own
// output box under it), wrapped at two widths but under the same keys
// (line, piece); a selection made in one is shaded in the other at the
// first one's bytes, which can fall inside a multi-byte character there.
mod common;
use horizon_studio::game::hstudio_theme::THEMES;
use horizon_studio::game::hstudio_ui::{Input, Tab, draw, handle};

fn redraw(st: &mut horizon_studio::game::hstudio::Studio, game: &mut common::FakeGame) {
    st.s.now += 0.05;
    game.drawn.clear();
    draw(st, game);
}

#[test]
fn a_console_selection_in_an_executor_style_drawn_twice() {
    // ProtoClassic, 340 wide: the Console page wraps the echoed line as
    // [print('[string "console"]:1: unexpected, symbol near '中'')] and the
    // output box, wider, as [... unexpected symbol, near '中'')].
    let mut game = common::FakeGame::new("zz-sel-twice");
    let mut st = common::studio(&mut game);
    st.s.ui.look.theme = THEMES.iter().position(|t| t.id == "protoclassic").unwrap();
    st.s.ui.look.rect = Some([0.0, 0.0, 340.0, 530.0]);
    st.s.ui.open = true;
    st.s.ui.tab = Tab::Console;
    st.s.now = 10.0;
    st.run_chunk(
        &mut game,
        "print('[string \"console\"]:1: unexpected symbol near \\'中\\'')",
        false,
    );
    redraw(&mut st, &mut game);
    let rows: Vec<(String, f32, f32)> = game
        .drawn
        .iter()
        .filter_map(|d| {
            let rest = d.strip_prefix("text ")?;
            let (text, at) = rest.rsplit_once(" @[")?;
            let n: Vec<f32> = at
                .split(']')
                .next()?
                .split(", ")
                .filter_map(|v| v.parse().ok())
                .collect();
            text.starts_with("symbol near").then(|| (text.to_owned(), n[0], n[1]))
        })
        .collect();
    println!("{rows:?}");
    let (_, x, y) = rows[0].clone();
    // The pointer over the row (the draw measures its chars), pressed at its
    // start and dragged 8 chars on ("symbol n"), a draw between each.
    handle(&mut st, &mut game, Input::Mouse(x + 0.5, y + 6.0));
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, Input::Button { left: true, down: true });
    redraw(&mut st, &mut game);
    handle(&mut st, &mut game, Input::Mouse(x + 6.0 * 8.0 + 1.0, y + 6.0));
    redraw(&mut st, &mut game); // panics: text_row shades "near '中'')" at bytes 0..8
    handle(
        &mut st,
        &mut game,
        Input::Button {
            left: true,
            down: false,
        },
    );
    redraw(&mut st, &mut game);
}
