// Crashes and a hang a randomized crash hunt in the game's frame order found, each kept as a test.

mod common;

use common::FakeGame;
use horizon_studio::game::hstudio::{
    Camera, Corner, EntityInfo, Font, GameApi, PlayerEntry, PlayerInfo, PlayerPose, Reply, Studio, TexRef, ViaInfo,
};
use horizon_studio::game::hstudio_theme::THEMES;
use horizon_studio::game::hstudio_ui::{Tab, draw};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// FakeGame on a screen `w` x `h` dp (FakeGame's own is 960 x 540).
struct Screen {
    f: FakeGame,
    w: f32,
    h: f32,
}

impl GameApi for Screen {
    fn chat(&mut self, t: &str) {
        self.f.chat(t)
    }
    fn send(&mut self, l: &str) {
        self.f.send(l)
    }
    fn player(&mut self) -> Option<PlayerInfo> {
        self.f.player()
    }
    fn block(&self, x: i32, y: i32, z: i32) -> Option<String> {
        self.f.block(x, y, z)
    }
    fn clock(&self) -> Option<(i64, i64)> {
        self.f.clock()
    }
    fn ticks(&self) -> u64 {
        self.f.ticks()
    }
    fn fps(&self) -> [u32; 3] {
        self.f.fps()
    }
    fn mapping(&mut self, n: &str) -> Option<(bool, String)> {
        self.f.mapping(n)
    }
    fn screen(&self) -> String {
        self.f.screen()
    }
    fn agent(&mut self, l: &str) -> Result<Reply, String> {
        self.f.agent(l)
    }
    fn data_dir(&self) -> PathBuf {
        self.f.data_dir()
    }
    fn clipboard(&mut self) -> String {
        self.f.clipboard()
    }
    fn set_clipboard(&mut self, t: &str) {
        self.f.set_clipboard(t)
    }
    fn gui_size(&self) -> (f32, f32) {
        (self.w, self.h)
    }
    fn text_width(&self, t: &str) -> f32 {
        self.f.text_width(t)
    }
    fn fill(&mut self, r: [f32; 4], c: u32) {
        self.f.fill(r, c)
    }
    fn text(&mut self, t: &str, at: [f32; 2], rgb: u32, sh: bool) {
        self.f.text(t, at, rgb, sh)
    }
    fn place(&mut self, c: Corner, w: f32, h: f32) -> (f32, f32) {
        self.f.place(c, w, h)
    }
    fn open_folder(&mut self, p: &Path) {
        self.f.open_folder(p)
    }
    fn camera(&self) -> Option<Camera> {
        self.f.camera()
    }
    fn world_quad(&mut self, c: [[f64; 3]; 4], argb: u32) {
        self.f.world_quad(c, argb)
    }
    fn quad(&mut self, c: [[f32; 2]; 4], argb: u32) {
        self.f.quad(c, argb)
    }
    fn sound(&mut self, id: &str, at: Option<[f64; 3]>, v: f32, p: f32) -> Result<(), String> {
        self.f.sound(id, at, v, p)
    }
    fn particle(&mut self, id: &str, at: [f64; 3], vel: [f64; 3], c: Option<[f32; 4]>) -> Result<(), String> {
        self.f.particle(id, at, vel, c)
    }
    fn dp(&mut self) -> (f32, f32, f32) {
        (self.w, self.h, 1.0)
    }
    fn dp_rect(&mut self, r: [f32; 4], radius: f32, argb: u32) {
        self.f.dp_rect(r, radius, argb)
    }
    fn dp_border(&mut self, r: [f32; 4], radius: f32, argb: u32) {
        self.f.dp_border(r, radius, argb)
    }
    fn dp_text(&mut self, t: &str, at: [f32; 2], size: f32, font: Font, argb: u32) -> f32 {
        self.f.dp_text(t, at, size, font, argb)
    }
    fn dp_measure(&mut self, t: &str, size: f32, font: Font) -> f32 {
        self.f.dp_measure(t, size, font)
    }
    fn dp_mark(&mut self) -> Option<usize> {
        self.f.dp_mark()
    }
    fn dp_clip(&mut self, mark: usize, r: [f32; 4]) {
        self.f.dp_clip(mark, r)
    }
    fn dp_image(&mut self, image: TexRef<'_>, r: [f32; 4], src: [f32; 4], argb: u32) -> bool {
        self.f.dp_image(image, r, src, argb)
    }
    fn via(&self) -> ViaInfo {
        self.f.via()
    }
    fn pose(&self) -> Option<PlayerPose> {
        self.f.pose()
    }
    fn entities(&mut self, max: usize) -> Vec<EntityInfo> {
        self.f.entities(max)
    }
    fn players(&mut self) -> Option<Vec<PlayerEntry>> {
        self.f.players()
    }
}

/// Draws the studio in `theme` on a `w` x `h` dp screen at size `scale`,
/// the editor's cursor at the end of its line; false when the draw never
/// came back (20 s).
fn draw_returns(theme: &'static str, w: f32, h: f32, scale: f32, text: &'static str) -> bool {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut game = Screen {
            f: FakeGame::new(&format!("zz-narrow-{theme}-{w}")),
            w,
            h,
        };
        let mut st = Studio::new();
        st.frame(&mut game, 0.016);
        st.s.ui.look.theme = THEMES.iter().position(|t| t.id == theme).unwrap();
        st.s.ui.look.scale = scale;
        st.s.editor.set_text(text);
        let last = st.s.editor.lines.len() - 1;
        st.s.editor.row = last;
        st.s.editor.col = st.s.editor.lines[last].len();
        st.s.ui.open = true;
        st.s.ui.tab = Tab::Editor;
        st.s.now = 10.0;
        draw(&mut st, &mut game);
        let _ = tx.send(());
    });
    rx.recv_timeout(Duration::from_secs(20)).is_ok()
}

#[test]
fn a_narrow_synapse_code_never_comes_back_from_its_draw() {
    // Synapse on a screen 200 dp wide: the window is the screen's width,
    // its script list keeps 110, and the code is left 62 wide, less than
    // its gutter. The cursor at the end of its line (an empty buffer, as
    // Clear leaves it): draw_code's hscroll loop never breaks.
    assert!(
        draw_returns("synapse", 200.0, 400.0, 1.0, ""),
        "draw() never returned: draw_code's hscroll loop runs forever"
    );
}

#[test]
fn a_wide_enough_synapse_code_draws() {
    assert!(draw_returns("synapse", 320.0, 400.0, 1.0, ""));
}

#[test]
fn a_lua_error_inside_a_function_in_a_loop() {
    // The live check (and a mod's load, and the console) parses this; the
    // function's body fails, its FuncState stays pushed, and loop_block's
    // `self.fs().loops -= 1` takes 1 from the inner function's 0.
    for src in [
        "while x do f = function(",
        "for i = 1, 2 do local g = function() x = end",
    ] {
        let lines: Vec<String> = src.split('\n').map(str::to_owned).collect();
        let problems = horizon_studio::game::hstudio_assist::check(&lines, "t.lua", None);
        assert!(!problems.is_empty(), "{src}: a syntax error");
    }
}

/// The studio on the Editor tab with pb.lua open and checked: one problem,
/// "player has no 'qqqqq'" at bytes 7..12 of line 2. Then the player types
/// a 3-byte letter just before the word (byte 7 is now inside it).
fn stale_problem() -> (FakeGame, Studio) {
    use horizon_studio::game::hstudio_ui::{Input, handle};
    let mut game = FakeGame::new("zz-goto-stale");
    let mut st = common::studio(&mut game);
    game.write_mod("pb.lua", "local a = 1\nplayer.qqqqq()\n");
    st.scan(&mut game);
    let i = common::mod_index(&st, "pb");
    st.edit_mod(i);
    st.s.ui.open = true;
    st.s.ui.tab = Tab::Editor;
    st.s.now = 10.0;
    draw(&mut st, &mut game);
    let probs: Vec<(usize, Option<(usize, usize)>)> = st.s.editor.problems.iter().map(|p| (p.line, p.span)).collect();
    assert_eq!(probs, vec![(2, Some((7, 12)))]);
    st.s.editor.click(1, 6, false);
    handle(&mut st, &mut game, Input::Text("中".into()));
    assert_eq!(st.s.editor.lines[1], "player中.qqqqq()");
    // A frame later: the check waits for a second's pause, so the problem
    // is still the one for the text before the letter.
    st.s.now += 0.05;
    draw(&mut st, &mut game);
    assert_eq!(st.s.editor.problems[0].span, Some((7, 12)));
    (game, st)
}

fn enter() -> horizon_studio::game::hstudio_ui::Input {
    horizon_studio::game::hstudio_ui::Input::Key {
        key: horizon_studio::game::hstudio_ui::Key::Enter,
        down: true,
        repeat: false,
        mods: Default::default(),
    }
}

#[test]
fn a_stale_problem_gone_to_after_a_wide_char() {
    // The Problems pane to the front with the keys (its tab clicked), then
    // Enter on the problem: goto_problem slices the line at byte 7.
    use horizon_studio::game::hstudio_ui::{Pane, handle};
    let (mut game, mut st) = stale_problem();
    st.s.ui.dock.activate(Pane::Problems);
    st.s.ui.focus = Pane::Problems;
    st.s.ui.problem_sel = 0;
    handle(&mut st, &mut game, enter());
    let ed = &st.s.editor;
    assert!(ed.lines[ed.row].is_char_boundary(ed.col));
}

#[test]
fn a_stale_problem_without_the_suggestions() {
    // The same, the letter put in as typing does but past the suggestions
    // (which crashed on it before 0e74853), for the older commits.
    use horizon_studio::game::hstudio_ui::{Pane, handle};
    let mut game = FakeGame::new("zz-goto-stale-2");
    let mut st = common::studio(&mut game);
    game.write_mod("pb.lua", "local a = 1\nplayer.qqqqq()\n");
    st.scan(&mut game);
    let i = common::mod_index(&st, "pb");
    st.edit_mod(i);
    st.s.ui.open = true;
    st.s.ui.tab = Tab::Editor;
    st.s.now = 10.0;
    draw(&mut st, &mut game);
    st.s.editor.click(1, 6, false);
    st.s.editor.type_text("中");
    st.s.editor.edited_at = st.s.now;
    st.s.now += 0.05;
    draw(&mut st, &mut game);
    st.s.ui.dock.activate(Pane::Problems);
    st.s.ui.focus = Pane::Problems;
    handle(&mut st, &mut game, enter());
    let ed = &st.s.editor;
    assert!(ed.lines[ed.row].is_char_boundary(ed.col));
}
