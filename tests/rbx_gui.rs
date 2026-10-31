// The Roblox layer's GUIs on a fake game, beyond tests/roblox.rs: enums and
// properties scripts set and read (ElasticBehavior, ContentText, GuiState,
// the Transparency alias, an ImageButton's hover and pressed images,
// IsLoaded), and classes that stay lenient.

mod common;

use common::{FakeGame, log, mod_index, run_for, studio};
use horizon_studio::game::hstudio::{ModState, Studio};
use horizon_studio::game::hstudio_rbx_draw::{pointer_button, pointer_moved};

fn printed(st: &Studio, id: &str) -> Vec<String> {
    let prefix = format!("[{id}] ");
    log(st)
        .iter()
        .filter_map(|l| l.strip_prefix(&prefix).map(str::to_owned))
        .collect()
}

/// Loads `src` as mod `id` and lets it run `seconds`.
fn load(game: &mut FakeGame, st: &mut Studio, id: &str, src: &str, seconds: f64) {
    game.write_mod(&format!("{id}.lua"), src);
    run_for(st, game, seconds);
    let i = mod_index(st, id);
    if let ModState::Failed(msg, trace) = &st.s.mods[i].state {
        panic!("{id} failed: {msg}\n{trace}\n{:?}", log(st));
    }
}

fn hud(st: &mut Studio, game: &mut FakeGame) -> Vec<String> {
    game.drawn.clear();
    st.hud(game);
    std::mem::take(&mut game.drawn)
}

#[test]
fn elastic_behavior_takes_its_enum() {
    let mut game = FakeGame::new("rbx-elastic");
    let mut st = studio(&mut game);
    let src = r#"
        local sf = Instance.new("ScrollingFrame")
        print(sf.ElasticBehavior)
        sf.ElasticBehavior = Enum.ElasticBehavior.Never
        print(sf.ElasticBehavior, sf.ElasticBehavior == Enum.ElasticBehavior.Never)
        sf.ElasticBehavior = "Always"
        print(sf.ElasticBehavior.Name, #Enum.ElasticBehavior:GetEnumItems(), Enum.GuiState.NonInteractable.Value)
        print(pcall(function() sf.ElasticBehavior = "Sometimes" end))
    "#;
    load(&mut game, &mut st, "elastic", src, 0.6);
    let p = printed(&st, "elastic");
    assert_eq!(p[0], "Enum.ElasticBehavior.WhenScrollable");
    assert_eq!(p[1], "Enum.ElasticBehavior.Never    true");
    assert_eq!(p[2], "Always    3    3");
    assert!(
        p[3].starts_with("false    ") && p[3].contains("Enum.ElasticBehavior expected"),
        "{}",
        p[3]
    );
}

#[test]
fn content_text_gui_state_and_transparency() {
    let mut game = FakeGame::new("rbx-derived");
    let mut st = studio(&mut game);
    let src = r#"
        local TweenService = game:GetService("TweenService")
        local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
        local label = Instance.new("TextLabel", gui)
        label.Position = UDim2.fromOffset(0, 200)
        label.RichText = true
        label.Text = "<b>Hi</b> &amp; bye"
        print(label.ContentText)
        label:GetPropertyChangedSignal("ContentText"):Connect(function() print("content", label.ContentText) end)
        label.Text = "<i>new</i>"
        print(pcall(function() label.ContentText = "x" end))
        local plain = Instance.new("TextLabel")
        plain.Text = "<b>kept</b>"
        print(plain.ContentText)
        button = Instance.new("TextButton", gui)
        button.Size = UDim2.fromOffset(100, 40)
        button:GetPropertyChangedSignal("GuiState"):Connect(function() print("state", button.GuiState.Name) end)
        print(button.GuiState)
        print(pcall(function() button.GuiState = Enum.GuiState.Hover end))
        local frame = Instance.new("Frame", gui)
        frame.Transparency = 0.25
        print(frame.BackgroundTransparency, frame.Transparency)
        frame.BackgroundTransparency = 0.5
        print(frame.Transparency)
        local part = Instance.new("Part")
        part.Transparency = 0.75
        print(part.Transparency)
        TweenService:Create(frame, TweenInfo.new(0), { Transparency = 1 }):Play()
        task.wait(0.1)
        print("tweened", frame.BackgroundTransparency)
        command("off", function() button.Interactable = false end)
    "#;
    load(&mut game, &mut st, "derived", src, 0.7);
    let p = printed(&st, "derived");
    assert_eq!(p[0], "Hi & bye");
    assert!(
        p[1].starts_with("false    ") && p[1].contains("Unable to assign property ContentText. Property is read only"),
        "{}",
        p[1]
    );
    assert_eq!(p[2], "<b>kept</b>");
    assert_eq!(p[3], "Enum.GuiState.Idle");
    assert!(p[4].contains("Property is read only"), "{}", p[4]);
    assert_eq!(p[5], "0.25    0.25");
    assert_eq!(p[6], "0.5");
    assert_eq!(p[7], "0.75");
    assert!(p.contains(&"content    new".to_owned()), "{p:#?}");
    assert!(p.contains(&"tweened    1".to_owned()), "{p:#?}");
    // The button's state follows the pointer (which the GUI has, as while its
    // screen is open): over it, pressed, let go, away.
    st.s.rbx.pointer.free = true;
    hud(&mut st, &mut game);
    let mark = printed(&st, "derived").len();
    let states = |st: &Studio| printed(st, "derived")[mark..].to_vec();
    pointer_moved(&mut st.s.rbx, 50.0, 20.0);
    run_for(&mut st, &mut game, 0.05);
    pointer_button(&mut st.s.rbx, true, true);
    run_for(&mut st, &mut game, 0.05);
    pointer_button(&mut st.s.rbx, true, false);
    run_for(&mut st, &mut game, 0.05);
    pointer_moved(&mut st.s.rbx, 500.0, 500.0);
    run_for(&mut st, &mut game, 0.05);
    assert!(st.command("off"));
    run_for(&mut st, &mut game, 0.05);
    assert_eq!(
        states(&st),
        [
            "state    Hover",
            "state    Press",
            "state    Hover",
            "state    Idle",
            "state    NonInteractable"
        ]
    );
}

/// A 1x1 PNG of one colour.
fn dot(rgba: [u8; 4]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, 1, 1);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&rgba).unwrap();
    out
}

#[test]
fn image_buttons_show_hover_and_pressed_images() {
    let mut game = FakeGame::new("rbx-images");
    let mut st = studio(&mut game);
    let ws = st.s.rbx.workspace_dir.clone();
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("red.png"), dot([255, 0, 0, 255])).unwrap();
    std::fs::write(ws.join("green.png"), dot([0, 255, 0, 255])).unwrap();
    std::fs::write(ws.join("blue.png"), dot([0, 0, 255, 255])).unwrap();
    std::fs::write(ws.join("bad.png"), "not a picture").unwrap();
    let src = r#"
        local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
        local b = Instance.new("ImageButton", gui)
        b.Size = UDim2.fromOffset(48, 48)
        b.BackgroundTransparency = 1
        b.Image = getcustomasset("red.png")
        b.HoverImage = getcustomasset("green.png")
        b.PressedImage = getcustomasset("blue.png")
        print(b.IsLoaded, b.TileSize, b.HoverImage)
        local other = Instance.new("ImageLabel")
        print(other.IsLoaded)
        other.Image = "rbxassetid://12345"
        print(other.IsLoaded)
        other.Image = getcustomasset("bad.png")
        print(other.IsLoaded, pcall(function() other.IsLoaded = true end))
    "#;
    load(&mut game, &mut st, "images", src, 0.6);
    let p = printed(&st, "images");
    assert_eq!(p[0], "true    {1, 0}, {1, 0}    rbxasset://horizon-studio/green.png");
    assert_eq!(p[1], "false");
    assert_eq!(p[2], "false");
    assert!(
        p[3].starts_with("false    false    ") && p[3].contains("read only"),
        "{}",
        p[3]
    );
    let cell = |drawn: &[String]| {
        drawn
            .iter()
            .find(|d| d.starts_with("rect [0.0, 0.0, 48.3, 48.3]"))
            .cloned()
            .unwrap_or_else(|| panic!("no image cell: {drawn:#?}"))
    };
    assert!(cell(&hud(&mut st, &mut game)).ends_with("ffff0000"));
    pointer_moved(&mut st.s.rbx, 10.0, 10.0);
    assert!(cell(&hud(&mut st, &mut game)).ends_with("ff00ff00"), "hovered");
    pointer_button(&mut st.s.rbx, true, true);
    assert!(cell(&hud(&mut st, &mut game)).ends_with("ff0000ff"), "pressed");
    pointer_button(&mut st.s.rbx, true, false);
    pointer_moved(&mut st.s.rbx, 300.0, 300.0);
    assert!(cell(&hud(&mut st, &mut game)).ends_with("ffff0000"), "left alone");
}

#[test]
fn kept_classes_take_any_property() {
    // Classes the layer only keeps take properties it doesn't declare, and
    // must go on doing so as they become real.
    let mut game = FakeGame::new("rbx-lenient");
    let mut st = studio(&mut game);
    let src = r#"
        local view = Instance.new("ViewportFrame")
        view.CurrentCamera = workspace.CurrentCamera
        view.Ambient = Color3.new(1, 0, 0)
        local video = Instance.new("VideoFrame")
        video.Video = "rbxassetid://1"
        video.Playing = true
        local board = Instance.new("BillboardGui")
        board.PlayerToHideFrom = game.Players.LocalPlayer
        board.StudsOffset = Vector3.new(0, 2, 0)
        local pages = Instance.new("UIPageLayout")
        pages.GamepadInputEnabled = false
        local grid = Instance.new("UITableLayout")
        grid.FillEmptySpaceRows = true
        local flex = Instance.new("UIFlexItem")
        flex.FlexMode = "Grow"
        print(view.CurrentCamera == workspace.CurrentCamera, video.Video, video.Playing, board.PlayerToHideFrom.Name)
        print(pages.GamepadInputEnabled, grid.FillEmptySpaceRows, flex.FlexMode, view.Nothing)
    "#;
    load(&mut game, &mut st, "kept", src, 0.6);
    let p = printed(&st, "kept");
    assert_eq!(
        p,
        [
            "true    rbxassetid://1    true    Player1",
            "false    true    Grow    nil"
        ]
    );
}
