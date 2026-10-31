// The Roblox layer's GUIs on a fake game, beyond tests/roblox.rs: enums and
// properties scripts set and read (ElasticBehavior, ContentText, GuiState,
// the Transparency alias, an ImageButton's hover and pressed images,
// IsLoaded), classes that stay lenient, TextTruncate and
// MaxVisibleGraphemes, layout values fresh when read, TextBox editing
// through the GUI's screen, ScrollingFrames' directions and bars, the
// services scripts ask for, right clicks on the GUI's screen, and the
// drawing cut, turned and coloured once drawn (ClipsDescendants, Rotation,
// UIGradient, CanvasGroup).

mod common;

use common::{FakeGame, log, mod_index, run_for, studio};
use horizon_studio::game::hstudio::{ModState, Studio};
use horizon_studio::game::hstudio_look::draw_screen;
use horizon_studio::game::hstudio_rbx_draw::{
    OUTPUTS, fresh_layout, lay_out, objects_at, pointer_button, pointer_moved, wheel,
};
use horizon_studio::game::hstudio_rbx_values::get;
use horizon_studio::game::hstudio_ui::{Input, Key, Mods, UiResult, handle_screen};

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
    // The image drawn, by its one pixel's colour.
    let image = |drawn: &[String]| {
        drawn
            .iter()
            .find_map(|d| d.strip_prefix("image 1x1 #"))
            .map(|d| d[..8].to_owned())
            .unwrap_or_else(|| panic!("no image: {drawn:#?}"))
    };
    assert_eq!(image(&hud(&mut st, &mut game)), "ff0000ff");
    pointer_moved(&mut st.s.rbx, 10.0, 10.0);
    assert_eq!(image(&hud(&mut st, &mut game)), "00ff00ff", "hovered");
    pointer_button(&mut st.s.rbx, true, true);
    assert_eq!(image(&hud(&mut st, &mut game)), "0000ffff", "pressed");
    pointer_button(&mut st.s.rbx, true, false);
    pointer_moved(&mut st.s.rbx, 300.0, 300.0);
    assert_eq!(image(&hud(&mut st, &mut game)), "ff0000ff", "left alone");
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

/// The texts drawn, in order.
fn texts(drawn: &[String]) -> Vec<String> {
    drawn
        .iter()
        .filter_map(|d| d.strip_prefix("text "))
        .map(|d| d.split(" @").next().unwrap().to_owned())
        .collect()
}

#[test]
fn text_truncates_and_shows_its_first_graphemes() {
    let mut game = FakeGame::new("rbx-truncate");
    let mut st = studio(&mut game);
    // The fake game's characters are 6 dp wide: 66 holds 11.
    let src = r#"
        local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
        local cut = Instance.new("TextLabel", gui)
        cut.Size = UDim2.fromOffset(66, 20)
        cut.Text = "Hello world wide"
        cut.TextTruncate = Enum.TextTruncate.AtEnd
        local typed = Instance.new("TextLabel", gui)
        typed.Position = UDim2.fromOffset(0, 100)
        typed.Size = UDim2.fromOffset(200, 20)
        typed.Text = "Typewriter"
        typed.MaxVisibleGraphemes = 4
        command("more", function() typed.MaxVisibleGraphemes = -1 end)
    "#;
    load(&mut game, &mut st, "truncate", src, 0.6);
    let drawn = hud(&mut st, &mut game);
    assert_eq!(texts(&drawn), ["Hello wo...", "Type"], "{drawn:#?}");
    assert!(st.command("more"));
    run_for(&mut st, &mut game, 0.05);
    assert_eq!(texts(&hud(&mut st, &mut game)), ["Hello wo...", "Typewriter"]);
}

/// Runs a mod's command and gives what it returned (shown in chat).
fn say(st: &mut Studio, game: &mut FakeGame, command: &str) -> String {
    assert!(st.command(command), "/{command}");
    st.frame(game, 0.016);
    game.chats.last().cloned().unwrap_or_default()
}

#[test]
fn absolute_values_are_fresh_when_read() {
    let mut game = FakeGame::new("rbx-fresh");
    let mut st = studio(&mut game);
    let src = r#"
        local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
        local frame = Instance.new("Frame", gui)
        frame.Position = UDim2.fromOffset(10, 20)
        frame.Size = UDim2.fromOffset(100, 50)
        print(frame.AbsoluteSize, frame.AbsolutePosition)
        frame.Size = UDim2.new(0.5, 0, 0, 20)
        print(frame.AbsoluteSize)
        local label = Instance.new("TextLabel", frame)
        label.Size = UDim2.fromScale(1, 1)
        label.Text = "hello"
        print(label.AbsolutePosition, label.AbsoluteSize, label.TextBounds)
        local list = Instance.new("UIListLayout", frame)
        list.Padding = UDim.new(0, 5)
        local second = Instance.new("TextLabel", frame)
        second.Size = UDim2.new(1, 0, 0, 30)
        print(second.AbsolutePosition, list.AbsoluteContentSize)
        local folder = Instance.new("Folder")
        local inside = Instance.new("Frame", folder)
        inside.Size = UDim2.fromOffset(7, 7)
        print(inside.AbsoluteSize)
        folder.Parent = gui
        print(inside.AbsoluteSize, inside.AbsolutePosition)
        label.Name = "Z"
        second.Name = "A"
        list.SortOrder = Enum.SortOrder.Name
        print(second.AbsolutePosition)
        command("again", function()
            return tostring(frame.AbsoluteSize) .. " " .. tostring(frame.AbsolutePosition)
        end)
        command("resize", function()
            frame.Size = UDim2.fromOffset(40, 40)
            return tostring(frame.AbsoluteSize)
        end)
        command("build", function()
            for i = 1, 20 do
                local row = Instance.new("TextLabel")
                row.Text = "row " .. i
                row.Size = UDim2.new(1, 0, 0, 10)
                row.Parent = frame
            end
        end)
    "#;
    load(&mut game, &mut st, "fresh", src, 0.6);
    let p = printed(&st, "fresh");
    assert_eq!(
        p,
        [
            "100, 50    10, 20",
            "480, 20",
            "10, 20    480, 20    30, 8",
            "10, 45    480, 55",
            "0, 0",
            "7, 7    0, 0",
            "10, 20",
        ]
    );
    // After a frame drew the GUI, reading lays nothing out again.
    hud(&mut st, &mut game);
    let before = st.s.rbx.fresh_layouts;
    assert_eq!(say(&mut st, &mut game, "again"), "480, 20 10, 20");
    assert_eq!(st.s.rbx.fresh_layouts, before);
    // A change, then a read: one layout, and a second read needs none.
    assert_eq!(say(&mut st, &mut game, "resize"), "40, 40");
    assert_eq!(st.s.rbx.fresh_layouts, before + 1);
    assert_eq!(say(&mut st, &mut game, "again"), "40, 40 10, 20");
    assert_eq!(st.s.rbx.fresh_layouts, before + 1);
    // Building without reading lays nothing out.
    say(&mut st, &mut game, "build");
    assert_eq!(st.s.rbx.fresh_layouts, before + 1);
}

#[test]
fn reads_while_a_gui_is_built_lay_out_only_their_way() {
    // A UI library builds rows and reads each one's place as it goes. Each
    // read lays out the way down to its row, and in a list only the sizes
    // of the rows beside it, never the whole GUI again with every text.
    let mut game = FakeGame::new("rbx-build");
    let mut st = studio(&mut game);
    let src = r#"
        local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
        for i = 1, 100 do
            local other = Instance.new("TextLabel", gui)
            other.Text = "elsewhere " .. i
        end
        local panel = Instance.new("Frame", gui)
        panel.Size = UDim2.fromOffset(200, 400)
        local plain = Instance.new("Frame", panel)
        plain.Size = UDim2.fromScale(1, 0.5)
        local listed = Instance.new("Frame", panel)
        listed.Position = UDim2.fromScale(0, 0.5)
        listed.Size = UDim2.fromScale(1, 0.5)
        Instance.new("UIListLayout", listed)
        command("plain", function()
            local y = 0
            for i = 1, 50 do
                local row = Instance.new("TextLabel")
                row.Text = "row " .. i
                row.Size = UDim2.new(1, 0, 0, 10)
                row.Position = UDim2.fromOffset(0, (i - 1) * 10)
                row.Parent = plain
                y = row.AbsolutePosition.Y
            end
            return y
        end)
        command("listed", function()
            local y = 0
            for i = 1, 50 do
                local row = Instance.new("TextLabel")
                row.Text = "row " .. i
                row.Size = UDim2.new(1, 0, 0, 10)
                row.Parent = listed
                y = row.AbsolutePosition.Y
            end
            return y
        end)
    "#;
    load(&mut game, &mut st, "build", src, 0.6);
    let nodes = st.s.rbx.fresh_nodes;
    assert_eq!(say(&mut st, &mut game, "plain"), "490");
    let plain = st.s.rbx.fresh_nodes - nodes;
    // The ScreenGui, the panel, the frame and the row: four a read.
    assert_eq!(plain, 50 * 4, "{plain}");
    let nodes = st.s.rbx.fresh_nodes;
    assert_eq!(say(&mut st, &mut game, "listed"), "690");
    let listed = st.s.rbx.fresh_nodes - nodes;
    // The way down and its layout, then the rows so far, sized only.
    assert_eq!(listed, (1..=50).map(|i| 4 + i).sum::<u64>(), "{listed}");
}

#[test]
fn a_read_lays_out_as_the_whole_gui_does() {
    // Every output a read works out on its own is what laying out the whole
    // GUI gives, with automatic sizes, layouts, padding, scale, constraints,
    // Folders and scrolling canvases on the way.
    let mut game = FakeGame::new("rbx-partial");
    let mut st = studio(&mut game);
    let src = r#"
        local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
        local panel = Instance.new("Frame", gui)
        panel.AnchorPoint = Vector2.new(0.5, 0.5)
        panel.Position = UDim2.fromScale(0.5, 0.5)
        panel.Size = UDim2.fromOffset(300, 0)
        panel.AutomaticSize = Enum.AutomaticSize.Y
        local pad = Instance.new("UIPadding", panel)
        pad.PaddingTop, pad.PaddingLeft, pad.PaddingRight = UDim.new(0, 8), UDim.new(0, 8), UDim.new(0, 8)
        local list = Instance.new("UIListLayout", panel)
        list.Padding = UDim.new(0, 4)
        list.HorizontalAlignment = Enum.HorizontalAlignment.Center
        local title = Instance.new("TextLabel", panel)
        title.Size = UDim2.new(1, 0, 0, 0)
        title.AutomaticSize = Enum.AutomaticSize.Y
        title.TextWrapped = true
        title.Text = "A title long enough to wrap over two or three lines here"
        local row = Instance.new("Frame", panel)
        row.Size = UDim2.new(1, 0, 0, 30)
        Instance.new("UIListLayout", row).FillDirection = Enum.FillDirection.Horizontal
        for i = 1, 3 do
            local b = Instance.new("TextButton", row)
            b.Size = UDim2.fromOffset(0, 30)
            b.AutomaticSize = Enum.AutomaticSize.X
            b.Text = string.rep("b", i * 3)
        end
        local folder = Instance.new("Folder", panel)
        local loose = Instance.new("Frame", folder)
        loose.Position = UDim2.fromOffset(5, 5)
        loose.Size = UDim2.fromOffset(20, 20)
        local scroll = Instance.new("ScrollingFrame", panel)
        scroll.Size = UDim2.new(1, 0, 0, 100)
        scroll.CanvasSize = UDim2.new()
        scroll.AutomaticCanvasSize = Enum.AutomaticSize.Y
        scroll.CanvasPosition = Vector2.new(0, 30)
        local grid = Instance.new("UIGridLayout", scroll)
        grid.CellSize = UDim2.fromOffset(60, 40)
        for i = 1, 7 do
            Instance.new("ImageLabel", scroll).Name = "cell" .. i
        end
        local big = Instance.new("TextLabel", panel)
        big.Size = UDim2.new(0.5, 0, 0, 20)
        big.TextScaled = true
        big.Text = "scaled"
        Instance.new("UIScale", big).Scale = 1.5
        local side = Instance.new("Frame", gui)
        side.Size = UDim2.fromScale(0.5, 0.5)
        Instance.new("UIAspectRatioConstraint", side).AspectRatio = 2
        local limit = Instance.new("UISizeConstraint", side)
        limit.MaxSize = Vector2.new(300, 300)
        local corner = Instance.new("TextBox", side)
        corner.Size = UDim2.fromScale(0.25, 0.25)
        corner.Text = ""
        corner.PlaceholderText = "type"
    "#;
    load(&mut game, &mut st, "partial", src, 0.6);
    let rbx = &mut st.s.rbx;
    let gui = rbx.player_gui;
    let ids: Vec<_> = rbx.descendants(gui).into_iter().filter(|&i| rbx.inst(i).gui).collect();
    let outputs = |rbx: &horizon_studio::game::hstudio_rbx::Rbx, id| -> Vec<String> {
        OUTPUTS
            .iter()
            .map(|k| format!("{k} {:?}", get(&rbx.raw_get(id, k))))
            .collect()
    };
    let mut alone = Vec::new();
    for &id in &ids {
        // Something moved: each read works its own values out.
        rbx.gui_epoch += 1;
        fresh_layout(rbx, &mut game, id);
        alone.push(outputs(rbx, id));
    }
    rbx.gui_epoch += 1;
    lay_out(rbx, &mut game);
    for (i, &id) in ids.iter().enumerate() {
        assert_eq!(alone[i], outputs(rbx, id), "{}", rbx.full_name(id));
    }
}

const NONE: Mods = Mods {
    ctrl: false,
    shift: false,
    alt: false,
};
const CTRL: Mods = Mods { ctrl: true, ..NONE };
const SHIFT: Mods = Mods { shift: true, ..NONE };

/// A key pressed and let go on the screen up (the GUI's, here).
fn press(st: &mut Studio, game: &mut FakeGame, key: Key, mods: Mods) {
    for down in [true, false] {
        handle_screen(
            st,
            game,
            Input::Key {
                key,
                down,
                repeat: false,
                mods,
            },
        );
    }
}

fn typed(st: &mut Studio, game: &mut FakeGame, text: &str) {
    handle_screen(st, game, Input::Text(text.into()));
}

/// Focuses the TextBox `box` with CaptureFocus (the mod's `focus` command)
/// and lets the GUI's screen open, as it does for a focused TextBox.
fn focus_box(st: &mut Studio, game: &mut FakeGame) {
    say(st, game, "focus");
    // The HUD lays the GUI out each frame, which is when it asks.
    hud(st, game);
    st.frame(game, 0.016);
    assert!(st.accept_screen(), "the focused TextBox asked for the pointer");
}

const BOX: &str = r#"
    local UIS = game:GetService("UserInputService")
    local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
    box = Instance.new("TextBox", gui)
    box.Size = UDim2.fromOffset(300, 30)
    box.Text = ""
    box.TextXAlignment = Enum.TextXAlignment.Left
    box.FocusLost:Connect(function(enter) print("lost", enter, box.Text) end)
    UIS.InputBegan:Connect(function(input, processed)
        if input.KeyCode == Enum.KeyCode.Left then print("left", processed) end
    end)
    command("focus", function() box:CaptureFocus() end)
    command("state", function() return box.Text .. "|" .. box.CursorPosition .. "|" .. box.SelectionStart end)
"#;

#[test]
fn text_boxes_edit_like_text_fields() {
    let mut game = FakeGame::new("rbx-edit");
    let mut st = studio(&mut game);
    let src = format!(
        "{BOX}\ncommand('caret', function() box.CursorPosition = 3 box.SelectionStart = 1 end)\ncommand('multi', function() box.MultiLine = true end)\n"
    );
    load(&mut game, &mut st, "edit", &src, 0.6);
    let state = |st: &mut Studio, game: &mut FakeGame| say(st, game, "state");
    assert_eq!(state(&mut st, &mut game), "|1|-1");
    focus_box(&mut st, &mut game);
    typed(&mut st, &mut game, "hello");
    assert_eq!(state(&mut st, &mut game), "hello|6|-1");
    press(&mut st, &mut game, Key::Left, NONE);
    press(&mut st, &mut game, Key::Left, NONE);
    typed(&mut st, &mut game, "X");
    assert_eq!(state(&mut st, &mut game), "helXlo|5|-1");
    press(&mut st, &mut game, Key::Home, NONE);
    press(&mut st, &mut game, Key::Delete, NONE);
    assert_eq!(state(&mut st, &mut game), "elXlo|1|-1");
    press(&mut st, &mut game, Key::End, NONE);
    assert_eq!(state(&mut st, &mut game), "elXlo|6|-1");
    // Shift selects; Control+C copies; typing replaces the selection.
    press(&mut st, &mut game, Key::Left, SHIFT);
    press(&mut st, &mut game, Key::Left, SHIFT);
    assert_eq!(state(&mut st, &mut game), "elXlo|4|6");
    press(&mut st, &mut game, Key::Char('c'), CTRL);
    assert_eq!(game.clipboard, "lo");
    typed(&mut st, &mut game, "!");
    assert_eq!(state(&mut st, &mut game), "elX!|5|-1");
    // Control+A, then typing replaces everything; Control+Backspace takes a word.
    press(&mut st, &mut game, Key::Char('a'), CTRL);
    assert_eq!(state(&mut st, &mut game), "elX!|5|1");
    typed(&mut st, &mut game, "hello world");
    press(&mut st, &mut game, Key::Backspace, CTRL);
    assert_eq!(state(&mut st, &mut game), "hello |7|-1");
    // Control+V pastes; Shift+Home and Control+X cut everything.
    game.clipboard = "there".into();
    press(&mut st, &mut game, Key::Char('v'), CTRL);
    assert_eq!(state(&mut st, &mut game), "hello there|12|-1");
    press(&mut st, &mut game, Key::Home, SHIFT);
    assert_eq!(state(&mut st, &mut game), "hello there|1|12");
    press(&mut st, &mut game, Key::Char('x'), CTRL);
    assert_eq!(game.clipboard, "hello there");
    assert_eq!(state(&mut st, &mut game), "|1|-1");
    // A script's CursorPosition and SelectionStart move the caret and select.
    typed(&mut st, &mut game, "abcdef");
    say(&mut st, &mut game, "caret");
    typed(&mut st, &mut game, "Z");
    assert_eq!(state(&mut st, &mut game), "Zcdef|2|-1");
    // Enter ends a single line's editing.
    press(&mut st, &mut game, Key::Enter, NONE);
    run_for(&mut st, &mut game, 0.05);
    assert!(st.s.screen.is_none(), "nothing else wants the pointer");
    assert_eq!(state(&mut st, &mut game), "Zcdef|-1|-1");
    // MultiLine: Enter is a new line and the box keeps the focus.
    say(&mut st, &mut game, "multi");
    focus_box(&mut st, &mut game);
    typed(&mut st, &mut game, "a");
    press(&mut st, &mut game, Key::Enter, NONE);
    typed(&mut st, &mut game, "b");
    assert_eq!(state(&mut st, &mut game), "a\nb|4|-1");
    // Escape lets go of the box first, and the screen stays for that press.
    let r = handle_screen(
        &mut st,
        &mut game,
        Input::Key {
            key: Key::Escape,
            down: true,
            repeat: false,
            mods: NONE,
        },
    );
    assert_eq!(r, UiResult::Used);
    run_for(&mut st, &mut game, 0.05);
    let p = printed(&st, "edit");
    assert!(
        p.contains(&"left    true".to_owned()),
        "keys typed are the GUI's: {p:#?}"
    );
    // (The console shows the second text's two lines apart.)
    let lost: Vec<&String> = p.iter().filter(|l| l.starts_with("lost")).collect();
    assert_eq!(lost, ["lost    true    Zcdef", "lost    false    a"]);
    assert!(st.s.screen.is_none());
}

#[test]
fn up_and_down_move_a_drawn_line() {
    let mut game = FakeGame::new("rbx-lines");
    let mut st = studio(&mut game);
    // Ten characters of 6 dp a line: "aaaa bbbb" over "cccc".
    let src = format!(
        "{BOX}\nbox.MultiLine = true box.TextWrapped = true box.ClearTextOnFocus = false\nbox.TextYAlignment = Enum.TextYAlignment.Top box.Size = UDim2.fromOffset(60, 100)\nbox.Text = 'aaaa bbbb cccc'\n"
    );
    load(&mut game, &mut st, "lines", &src, 0.6);
    focus_box(&mut st, &mut game);
    let state = |st: &mut Studio, game: &mut FakeGame| say(st, game, "state");
    assert_eq!(state(&mut st, &mut game), "aaaa bbbb cccc|15|-1");
    // Up keeps the caret's x, four characters in.
    press(&mut st, &mut game, Key::Up, NONE);
    assert_eq!(state(&mut st, &mut game), "aaaa bbbb cccc|5|-1");
    press(&mut st, &mut game, Key::Down, SHIFT);
    assert_eq!(state(&mut st, &mut game), "aaaa bbbb cccc|15|5");
    // Past the last line is the end, and above the first the start.
    press(&mut st, &mut game, Key::Down, NONE);
    assert_eq!(state(&mut st, &mut game), "aaaa bbbb cccc|15|-1");
    press(&mut st, &mut game, Key::Up, NONE);
    press(&mut st, &mut game, Key::Up, NONE);
    assert_eq!(state(&mut st, &mut game), "aaaa bbbb cccc|1|-1");
}

#[test]
fn clicks_and_drags_place_the_caret_and_select() {
    let mut game = FakeGame::new("rbx-select");
    let mut st = studio(&mut game);
    let src = format!("{BOX}\nbox.ClearTextOnFocus = false\nbox.Text = 'hello world'\n");
    load(&mut game, &mut st, "select", &src, 0.6);
    focus_box(&mut st, &mut game);
    let state = |st: &mut Studio, game: &mut FakeGame| say(st, game, "state");
    // A press nearer the end of "hello" than the start of the space.
    handle_screen(&mut st, &mut game, Input::Mouse(32.0, 15.0));
    handle_screen(&mut st, &mut game, Input::Button { left: true, down: true });
    assert_eq!(state(&mut st, &mut game), "hello world|6|-1");
    // Dragging to the end selects " world".
    handle_screen(&mut st, &mut game, Input::Mouse(70.0, 15.0));
    handle_screen(
        &mut st,
        &mut game,
        Input::Button {
            left: true,
            down: false,
        },
    );
    assert_eq!(state(&mut st, &mut game), "hello world|12|6");
    // The selection is drawn behind the text, and the caret at its end.
    st.s.now = 10.0;
    game.drawn.clear();
    draw_screen(&mut st, &mut game);
    assert!(
        game.drawn
            .contains(&"rect [30.0, 11.0, 36.0, 8.0] r0 730078d7".to_owned()),
        "{:#?}",
        game.drawn
    );
    assert!(
        game.drawn.iter().any(|d| d.starts_with("rect [67.0, 11.0, 1.5, 8.0]")),
        "{:#?}",
        game.drawn
    );
    press(&mut st, &mut game, Key::Char('c'), CTRL);
    assert_eq!(game.clipboard, " world");
    // A press in the box without a drag drops the selection.
    handle_screen(&mut st, &mut game, Input::Mouse(1.0, 15.0));
    handle_screen(&mut st, &mut game, Input::Button { left: true, down: true });
    handle_screen(
        &mut st,
        &mut game,
        Input::Button {
            left: true,
            down: false,
        },
    );
    assert_eq!(state(&mut st, &mut game), "hello world|1|-1");
}

/// The rectangles drawn after the first that starts with `first`.
fn drawn_after(drawn: &[String], first: &str) -> Vec<String> {
    let at = drawn
        .iter()
        .position(|d| d.starts_with(first))
        .unwrap_or_else(|| panic!("{first} not drawn: {drawn:#?}"));
    drawn[at + 1..].to_vec()
}

#[test]
fn scrolling_frames_scroll_their_way() {
    let mut game = FakeGame::new("rbx-scroll");
    let mut st = studio(&mut game);
    // A 200 x 100 window on a 400 x 300 canvas, with 10 dp bars; a row as
    // wide as the canvas lies under them.
    let src = r#"
        local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
        local sf = Instance.new("ScrollingFrame", gui)
        sf.Size = UDim2.fromOffset(200, 100)
        sf.CanvasSize = UDim2.fromOffset(400, 300)
        sf.ScrollBarThickness = 10
        sf.ScrollingDirection = Enum.ScrollingDirection.X
        sf.BorderSizePixel = 0
        local row = Instance.new("Frame", sf)
        row.Size = UDim2.new(1, 0, 0, 300)
        row.BackgroundColor3 = Color3.new(1, 0, 0)
        row.BorderSizePixel = 0
        command("pos", function() return tostring(sf.CanvasPosition) .. " " .. tostring(sf.ClipsDescendants) end)
        command("both", function() sf.ScrollingDirection = Enum.ScrollingDirection.XY end)
        command("left", function() sf.VerticalScrollBarPosition = Enum.VerticalScrollBarPosition.Left end)
    "#;
    load(&mut game, &mut st, "scroll", src, 0.6);
    st.s.rbx.pointer.free = true;
    // The row, drawn whole and cut to the window, as a ScrollingFrame clips.
    let row = "clip 1 [0.0, 0.0, 200.0, 100.0]";
    // ScrollingDirection X: the wheel scrolls across, and only the bottom bar shows.
    hud(&mut st, &mut game);
    pointer_moved(&mut st.s.rbx, 50.0, 50.0);
    wheel(&mut st.s.rbx, -1.0);
    assert_eq!(say(&mut st, &mut game, "pos"), "40, 0 true");
    let over = drawn_after(&hud(&mut st, &mut game), row);
    assert_eq!(
        over,
        ["rect [20.0, 90.0, 100.0, 10.0] r5 b3000000"],
        "the bar is over the row"
    );
    // XY: both bars, which leave each other the corner, and the wheel scrolls down.
    say(&mut st, &mut game, "both");
    let over = drawn_after(&hud(&mut st, &mut game), row);
    assert_eq!(
        over,
        [
            "rect [190.0, 0.0, 10.0, 30.0] r5 b3000000",
            "rect [19.0, 90.0, 95.0, 10.0] r5 b3000000"
        ]
    );
    wheel(&mut st.s.rbx, -1.0);
    assert_eq!(say(&mut st, &mut game, "pos"), "40, 40 true");
    // VerticalScrollBarPosition Left.
    say(&mut st, &mut game, "left");
    let over = drawn_after(&hud(&mut st, &mut game), row);
    assert_eq!(
        over,
        [
            "rect [0.0, 12.0, 10.0, 30.0] r5 b3000000",
            "rect [29.0, 90.0, 95.0, 10.0] r5 b3000000"
        ]
    );
    // Dragging the thumb: held 8 below its top, which goes to 30 of 60.
    pointer_moved(&mut st.s.rbx, 5.0, 20.0);
    pointer_button(&mut st.s.rbx, true, true);
    pointer_moved(&mut st.s.rbx, 5.0, 38.0);
    pointer_button(&mut st.s.rbx, true, false);
    assert_eq!(say(&mut st, &mut game, "pos"), "40, 100 true");
    // A press on the track below the thumb brings it there, to the end.
    hud(&mut st, &mut game);
    pointer_moved(&mut st.s.rbx, 5.0, 85.0);
    pointer_button(&mut st.s.rbx, true, true);
    pointer_button(&mut st.s.rbx, true, false);
    assert_eq!(say(&mut st, &mut game, "pos"), "40, 200 true");
    // The bottom bar drags across.
    hud(&mut st, &mut game);
    pointer_moved(&mut st.s.rbx, 30.0, 95.0);
    pointer_button(&mut st.s.rbx, true, true);
    pointer_moved(&mut st.s.rbx, 0.0, 95.0);
    pointer_button(&mut st.s.rbx, true, false);
    assert_eq!(say(&mut st, &mut game, "pos"), "0, 200 true");
}

#[test]
fn services_scripts_ask_for_exist() {
    let mut game = FakeGame::new("rbx-services");
    let mut st = studio(&mut game);
    let src = r#"
        local names = { "TeleportService", "VirtualUser", "Stats", "PathfindingService", "VirtualInputManager", "MaterialService" }
        for _, name in names do
            local s = game:GetService(name)
            assert(s and s == game:FindService(name) and s.ClassName == name and s.Parent == game, name)
        end
        local tp = game:GetService("TeleportService")
        tp.CustomLoadingScreen = "kept"
        print(tp.CustomLoadingScreen, tp.Nothing, game.TeleportService == tp)
        print(pcall(function() game:GetService("NoSuchService") end))
    "#;
    load(&mut game, &mut st, "services", src, 0.6);
    let p = printed(&st, "services");
    assert_eq!(p[0], "kept    nil    true");
    assert!(
        p[1].starts_with("false    ") && p[1].contains("'NoSuchService' is not a valid Service name"),
        "{}",
        p[1]
    );
}

#[test]
fn right_clicks_reach_the_gui_through_its_screen() {
    // Whatever the studio does with right clicks elsewhere, one on the GUI's
    // own screen is the GUI's MouseButton2.
    let mut game = FakeGame::new("rbx-right");
    let mut st = studio(&mut game);
    let src = r#"
        local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
        local b = Instance.new("TextButton", gui)
        b.Size = UDim2.fromOffset(100, 40)
        b.Modal = true
        b.MouseButton2Down:Connect(function() print("down") end)
        b.MouseButton2Up:Connect(function() print("up") end)
        b.MouseButton2Click:Connect(function() print("click") end)
    "#;
    load(&mut game, &mut st, "right", src, 0.6);
    hud(&mut st, &mut game);
    st.frame(&mut game, 0.016);
    assert!(st.accept_screen(), "the Modal button asked for the pointer");
    handle_screen(&mut st, &mut game, Input::Mouse(50.0, 20.0));
    for down in [true, false] {
        handle_screen(&mut st, &mut game, Input::Button { left: false, down });
    }
    run_for(&mut st, &mut game, 0.05);
    assert_eq!(printed(&st, "right"), ["down", "up", "click"]);
}

/// Where each drawing call is in `drawn`, the first that starts with `what`.
#[track_caller]
fn index_of(drawn: &[String], what: &str) -> usize {
    drawn
        .iter()
        .position(|d| d.starts_with(what))
        .unwrap_or_else(|| panic!("{what} not drawn: {drawn:#?}"))
}

#[test]
fn clipping_cuts_what_scrolls_out() {
    let mut game = FakeGame::new("rbx-clip");
    let mut st = studio(&mut game);
    // A 200 x 100 window with nothing to scroll, a label half out of it on
    // the right and one well inside.
    let src = r#"
        local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
        local sf = Instance.new("ScrollingFrame", gui)
        sf.Position = UDim2.fromOffset(10, 10)
        sf.Size = UDim2.fromOffset(200, 100)
        sf.CanvasSize = UDim2.fromOffset(200, 100)
        sf.BackgroundTransparency = 1
        local half = Instance.new("TextLabel", sf)
        half.Position = UDim2.fromOffset(150, 20)
        half.Size = UDim2.fromOffset(100, 20)
        half.BorderSizePixel = 0
        half.Text = "half out"
        local inside = Instance.new("TextLabel", sf)
        inside.Position = UDim2.fromOffset(10, 60)
        inside.Size = UDim2.fromOffset(60, 20)
        inside.BorderSizePixel = 0
        inside.Text = "in"
    "#;
    load(&mut game, &mut st, "clip", src, 0.6);
    let drawn = hud(&mut st, &mut game);
    // The label is drawn whole, then cut to the window, its text with it.
    let at = index_of(&drawn, "rect [160.0, 30.0, 100.0, 20.0] r0");
    assert!(drawn[at + 1].starts_with("text half out @"), "{drawn:#?}");
    assert_eq!(drawn[at + 2], format!("clip {at} [10.0, 10.0, 200.0, 100.0]"));
    // What lies inside needs no cut.
    let at = index_of(&drawn, "rect [20.0, 70.0, 60.0, 20.0] r0");
    assert!(drawn[at + 1].starts_with("text in @"));
    assert_eq!(drawn.len(), at + 2, "{drawn:#?}");
}

#[test]
fn rotation_turns_objects_and_their_children() {
    let mut game = FakeGame::new("rbx-rotate");
    let mut st = studio(&mut game);
    let src = r#"
        local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
        local frame = Instance.new("Frame", gui)
        frame.Position = UDim2.fromOffset(100, 100)
        frame.Size = UDim2.fromOffset(100, 50)
        frame.BorderSizePixel = 0
        frame.Rotation = 45
        local child = Instance.new("Frame", frame)
        child.Position = UDim2.fromOffset(10, 10)
        child.Size = UDim2.fromOffset(20, 20)
        child.BorderSizePixel = 0
        child.Rotation = 10
        local still = Instance.new("Frame", frame)
        still.Position = UDim2.fromOffset(50, 0)
        still.Size = UDim2.fromOffset(10, 10)
        still.BorderSizePixel = 0
        -- A turned frame in a clipping one is cut, though its upright box fits.
        local window = Instance.new("Frame", gui)
        window.Position = UDim2.fromOffset(400, 0)
        window.Size = UDim2.fromOffset(100, 100)
        window.BorderSizePixel = 0
        window.ClipsDescendants = true
        local turned = Instance.new("Frame", window)
        turned.Position = UDim2.fromOffset(40, 40)
        turned.Size = UDim2.fromOffset(20, 20)
        turned.BorderSizePixel = 0
        turned.Rotation = 30
        print(child.AbsoluteRotation, child.AbsolutePosition)
    "#;
    load(&mut game, &mut st, "rotate", src, 0.6);
    // The layout stays upright, as Roblox's does.
    assert_eq!(printed(&st, "rotate"), ["55    110, 110"]);
    let drawn = hud(&mut st, &mut game);
    let frame = index_of(&drawn, "rect [100.0, 100.0, 100.0, 50.0]");
    assert_eq!(drawn[frame + 1], format!("rotate {frame} [150.0, 125.0] 45"));
    // The child turns about its own centre, then about its parent's.
    let child = index_of(&drawn, "rect [110.0, 110.0, 20.0, 20.0]");
    assert_eq!(
        drawn[child + 1..child + 3],
        [
            format!("rotate {child} [120.0, 120.0] 10"),
            format!("rotate {child} [150.0, 125.0] 45")
        ]
    );
    let still = index_of(&drawn, "rect [150.0, 100.0, 10.0, 10.0]");
    assert_eq!(drawn[still + 1], format!("rotate {still} [150.0, 125.0] 45"));
    let turned = index_of(&drawn, "rect [440.0, 40.0, 20.0, 20.0]");
    assert_eq!(
        drawn[turned + 1..turned + 3],
        [
            format!("rotate {turned} [450.0, 50.0] 30"),
            format!("clip {turned} [400.0, 0.0, 100.0, 100.0]")
        ]
    );
}

/// The gradients drawn since the last `hud`, each with what it coloured first.
fn shades(game: &FakeGame, drawn: &[String]) -> Vec<(String, horizon_studio::game::hstudio_quads::Shade)> {
    game.shades
        .iter()
        .map(|(mark, s)| (drawn.get(*mark).cloned().unwrap_or_default(), s.clone()))
        .collect()
}

#[test]
fn ui_gradient_shades_across_the_object() {
    let mut game = FakeGame::new("rbx-gradient");
    let mut st = studio(&mut game);
    let src = r#"
        local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
        local frame = Instance.new("Frame", gui)
        frame.Size = UDim2.fromOffset(100, 50)
        frame.BackgroundColor3 = Color3.new(1, 1, 1)
        frame.BorderSizePixel = 0
        local g = Instance.new("UIGradient", frame)
        g.Color = ColorSequence.new(Color3.new(1, 1, 1), Color3.new(0, 0, 0))
        command("turn", function() g.Rotation = 90 end)
        command("shift", function()
            g.Rotation = 0
            g.Offset = Vector2.new(0.5, 0)
        end)
        command("fade", function()
            g.Offset = Vector2.zero
            g.Transparency = NumberSequence.new(0, 1)
        end)
        command("three", function()
            g.Transparency = NumberSequence.new(0)
            g.Color = ColorSequence.new({
                ColorSequenceKeypoint.new(0, Color3.new(1, 0, 0)),
                ColorSequenceKeypoint.new(0.5, Color3.new(0, 1, 0)),
                ColorSequenceKeypoint.new(1, Color3.new(0, 0, 1)),
            })
        end)
        -- Text is coloured too, a UIStroke only by a gradient of its own.
        local label = Instance.new("TextLabel", gui)
        label.Position = UDim2.fromOffset(0, 100)
        label.Size = UDim2.fromOffset(100, 20)
        label.BackgroundTransparency = 1
        label.Text = "faded"
        Instance.new("UIGradient", label).Color = ColorSequence.new(Color3.new(1, 0, 0), Color3.new(0, 0, 1))
        local stroke = Instance.new("UIStroke", label)
        stroke.ApplyStrokeMode = Enum.ApplyStrokeMode.Border
        command("stroke", function()
            Instance.new("UIGradient", stroke).Color = ColorSequence.new(Color3.new(0, 1, 0))
        end)
    "#;
    load(&mut game, &mut st, "gradient", src, 0.6);
    let look = |st: &mut Studio, game: &mut FakeGame, command: Option<&str>| {
        if let Some(c) = command {
            say(st, game, c);
        }
        game.shades.clear();
        let drawn = hud(st, game);
        (shades(game, &drawn), drawn)
    };
    let (found, drawn) = look(&mut st, &mut game, None);
    let (what, s) = &found[0];
    assert!(what.starts_with("rect [0.0, 0.0, 100.0, 50.0]"), "{what}");
    assert_eq!(s.at([0.0, 25.0]), 0xFFFF_FFFF, "white at the left");
    assert_eq!(s.at([100.0, 25.0]), 0xFF00_0000, "black at the right");
    assert_eq!(s.at([50.0, 0.0]), 0xFF80_8080);
    // The label's text is coloured by its gradient; its stroke is not.
    let text = index_of(&drawn, "text faded @");
    assert_eq!(drawn[text + 1], format!("shade {text}"));
    let (_, ls) = found.iter().find(|(w, _)| w.starts_with("text faded")).unwrap();
    assert_eq!(ls.at([0.0, 110.0]), 0xFFFF_0000);
    assert_eq!(ls.at([100.0, 110.0]), 0xFF00_00FF);
    let border = index_of(&drawn, "border ");
    assert!(
        !drawn[border..text].iter().any(|d| d.starts_with("shade")),
        "{drawn:#?}"
    );
    // Rotation 90 runs it top to bottom.
    let (found, _) = look(&mut st, &mut game, Some("turn"));
    assert_eq!(found[0].1.at([50.0, 0.0]), 0xFFFF_FFFF);
    assert_eq!(found[0].1.at([50.0, 50.0]), 0xFF00_0000);
    // Offset moves it: half the width to the right.
    let (found, _) = look(&mut st, &mut game, Some("shift"));
    assert_eq!(found[0].1.at([50.0, 25.0]), 0xFFFF_FFFF);
    assert_eq!(found[0].1.at([100.0, 25.0]), 0xFF80_8080);
    // The Transparency sequence lowers its alpha.
    let (found, _) = look(&mut st, &mut game, Some("fade"));
    assert_eq!(found[0].1.at([0.0, 25.0]), 0xFFFF_FFFF);
    assert_eq!(found[0].1.at([100.0, 25.0]), 0x0000_0000);
    // Three keypoints: the middle one shows at the middle.
    let (found, _) = look(&mut st, &mut game, Some("three"));
    let s = &found[0].1;
    assert_eq!(s.stops.len(), 3);
    assert_eq!(
        (s.at([0.0, 25.0]), s.at([50.0, 25.0]), s.at([100.0, 25.0])),
        (0xFFFF_0000, 0xFF00_FF00, 0xFF00_00FF)
    );
    // A gradient in the UIStroke colours the stroke.
    let (found, drawn) = look(&mut st, &mut game, Some("stroke"));
    let border = index_of(&drawn, "border ");
    assert!(
        found
            .iter()
            .any(|(w, s)| *w == drawn[border] && s.at([50.0, 110.0]) == 0xFF00_FF00),
        "{drawn:#?}"
    );
}

#[test]
fn canvas_group_tints_its_subtree() {
    let mut game = FakeGame::new("rbx-group");
    let mut st = studio(&mut game);
    let src = r#"
        local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
        local group = Instance.new("CanvasGroup", gui)
        group.Size = UDim2.fromOffset(200, 100)
        group.BackgroundColor3 = Color3.new(1, 1, 1)
        group.BorderSizePixel = 0
        group.GroupColor3 = Color3.new(1, 0, 0)
        local child = Instance.new("Frame", group)
        child.Size = UDim2.fromOffset(50, 50)
        child.BackgroundColor3 = Color3.new(1, 1, 1)
        child.BorderSizePixel = 0
        local label = Instance.new("TextLabel", group)
        label.Position = UDim2.fromOffset(0, 60)
        label.Size = UDim2.fromOffset(100, 20)
        label.BackgroundTransparency = 1
        label.TextColor3 = Color3.new(1, 1, 0)
        label.Text = "tinted"
        command("half", function() group.GroupTransparency = 0.5 end)
        command("fade", function()
            Instance.new("UIGradient", group).Transparency = NumberSequence.new(0, 1)
        end)
    "#;
    load(&mut game, &mut st, "group", src, 0.6);
    let drawn = hud(&mut st, &mut game);
    assert!(
        drawn.contains(&"rect [0.0, 0.0, 200.0, 100.0] r0 ffff0000".to_owned()),
        "{drawn:#?}"
    );
    assert!(drawn.contains(&"rect [0.0, 0.0, 50.0, 50.0] r0 ffff0000".to_owned()));
    assert!(
        drawn
            .iter()
            .any(|d| d.starts_with("text tinted @") && d.ends_with("ff0000"))
    );
    say(&mut st, &mut game, "half");
    let drawn = hud(&mut st, &mut game);
    assert!(
        drawn.contains(&"rect [0.0, 0.0, 50.0, 50.0] r0 80ff0000".to_owned()),
        "{drawn:#?}"
    );
    // A CanvasGroup's gradient colours all it holds, over the group's box.
    say(&mut st, &mut game, "fade");
    game.shades.clear();
    let drawn = hud(&mut st, &mut game);
    let found = shades(&game, &drawn);
    for what in [
        "rect [0.0, 0.0, 200.0, 100.0]",
        "rect [0.0, 0.0, 50.0, 50.0]",
        "text tinted @",
    ] {
        let (_, s) = found
            .iter()
            .find(|(w, _)| w.starts_with(what))
            .unwrap_or_else(|| panic!("{what} not coloured: {drawn:#?}"));
        assert_eq!(s.at([0.0, 50.0]), 0xFFFF_FFFF);
        assert_eq!(s.at([200.0, 50.0]), 0x00FF_FFFF);
    }
}

#[test]
fn without_marks_the_gui_is_drawn_as_best_it_can_be() {
    // A game that can't change what it drew (the headless one) gets the
    // GUI cut to whole shapes and lines, upright, a gradient's middle colour.
    let mut game = FakeGame::new("rbx-nomarks");
    game.no_marks = true;
    let mut st = studio(&mut game);
    let src = r#"
        local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
        local window = Instance.new("Frame", gui)
        window.Size = UDim2.fromOffset(200, 100)
        window.BackgroundTransparency = 1
        window.ClipsDescendants = true
        local half = Instance.new("Frame", window)
        half.Position = UDim2.fromOffset(150, 20)
        half.Size = UDim2.fromOffset(100, 20)
        half.BorderSizePixel = 0
        half.Rotation = 20
        half.BackgroundColor3 = Color3.new(1, 1, 1)
        Instance.new("UIGradient", half).Color = ColorSequence.new(Color3.new(1, 0, 0), Color3.new(0, 0, 1))
        local image = Instance.new("ImageLabel", window)
        image.Position = UDim2.fromOffset(150, 60)
        image.Size = UDim2.fromOffset(100, 20)
        image.BackgroundTransparency = 1
        image.Image = getcustomasset("wide.png")
    "#;
    let ws = st.s.rbx.workspace_dir.clone();
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("wide.png"), png(2, 1, &[[255, 0, 0, 255], [0, 0, 255, 255]])).unwrap();
    load(&mut game, &mut st, "nomarks", src, 0.6);
    let drawn = hud(&mut st, &mut game);
    // The image is cut too, the part of it that shows with it.
    assert_eq!(
        drawn,
        [
            "rect [150.0, 20.0, 50.0, 20.0] r0 ff800080",
            "image 2x1 #ff0000ff [150.0, 60.0, 50.0, 20.0] from [0.0, 0.0, 1.0, 1.0] ffffffff"
        ]
    );
}

#[test]
fn viewport_frames_draw_as_frames() {
    // A ViewportFrame's 3D view and a VideoFrame's video aren't drawn, but
    // both are frames: a background, and children laid out inside.
    let mut game = FakeGame::new("rbx-viewport");
    let mut st = studio(&mut game);
    let src = r#"
        local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
        local view = Instance.new("ViewportFrame", gui)
        view.Position = UDim2.fromOffset(10, 10)
        view.Size = UDim2.fromOffset(100, 80)
        view.BackgroundColor3 = Color3.new(0, 0, 1)
        view.BorderSizePixel = 0
        view.CurrentCamera = Instance.new("Camera", view)
        local label = Instance.new("TextLabel", view)
        label.Size = UDim2.fromScale(1, 0.5)
        label.BorderSizePixel = 0
        label.Text = "inside"
        local video = Instance.new("VideoFrame", gui)
        video.Position = UDim2.fromOffset(200, 10)
        video.Size = UDim2.fromOffset(50, 50)
        video.BorderSizePixel = 0
        video.Video = "rbxassetid://5"
        video.Played:Connect(function(v) print("played", v, video.Playing) end)
        print(view.AbsoluteSize, label.AbsoluteSize, video.IsLoaded, pcall(function() video.IsLoaded = true end))
        video:Play()
    "#;
    load(&mut game, &mut st, "viewport", src, 0.6);
    let p = printed(&st, "viewport");
    assert!(
        p[0].starts_with("100, 80    100, 40    false    false    ") && p[0].contains("read only"),
        "{p:#?}"
    );
    assert_eq!(p[1], "played    rbxassetid://5    true");
    let drawn = hud(&mut st, &mut game);
    for want in [
        "rect [10.0, 10.0, 100.0, 80.0] r0 ff0000ff",
        "rect [10.0, 10.0, 100.0, 40.0] r0 ffa3a2a5",
        "rect [200.0, 10.0, 50.0, 50.0] r0 ffa3a2a5",
    ] {
        assert!(drawn.contains(&want.to_owned()), "{want}: {drawn:#?}");
    }
    assert!(drawn.iter().any(|d| d.starts_with("text inside @")));
}

/// A PNG of `w` x `h` pixels, row by row.
fn png(w: u32, h: u32, pixels: &[[u8; 4]]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&pixels.concat()).unwrap();
    out
}

/// The images drawn: where, from which part of the image, with what tint.
fn images(drawn: &[String]) -> Vec<String> {
    drawn
        .iter()
        .filter_map(|d| d.strip_prefix("image "))
        .map(|d| d.splitn(3, ' ').nth(2).unwrap_or(d).to_owned())
        .collect()
}

#[test]
fn images_scale_fit_crop_tile_and_slice() {
    let mut game = FakeGame::new("rbx-scale");
    let mut st = studio(&mut game);
    let ws = st.s.rbx.workspace_dir.clone();
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("wide.png"), png(2, 1, &[[255, 0, 0, 255], [0, 0, 255, 255]])).unwrap();
    std::fs::write(ws.join("box.png"), png(4, 4, &[[255, 255, 255, 255]; 16])).unwrap();
    let src = r#"
        local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
        local image = Instance.new("ImageLabel", gui)
        image.Size = UDim2.fromOffset(100, 100)
        image.BackgroundTransparency = 1
        image.Image = getcustomasset("wide.png")
        local function set(props)
            return function()
                for k, v in props do
                    image[k] = v
                end
            end
        end
        command("fit", set({ ScaleType = Enum.ScaleType.Fit }))
        command("crop", set({ ScaleType = Enum.ScaleType.Crop }))
        command("tile", set({ ScaleType = Enum.ScaleType.Tile, TileSize = UDim2.fromScale(0.5, 0.5) }))
        command("tile40", set({ TileSize = UDim2.fromOffset(40, 40) }))
        command("slice", set({
            Image = getcustomasset("box.png"),
            Size = UDim2.fromOffset(100, 60),
            ScaleType = Enum.ScaleType.Slice,
            SliceCenter = Rect.new(1, 1, 3, 3),
            SliceScale = 10,
        }))
        command("part", set({
            Image = getcustomasset("wide.png"),
            Size = UDim2.fromOffset(100, 100),
            ScaleType = Enum.ScaleType.Stretch,
            ImageRectOffset = Vector2.new(1, 0),
            ImageRectSize = Vector2.new(1, 1),
            ImageColor3 = Color3.new(1, 0, 0),
            ImageTransparency = 0.5,
        }))
    "#;
    load(&mut game, &mut st, "scale", src, 0.6);
    let mut look = |command: Option<&str>| {
        if let Some(c) = command {
            say(&mut st, &mut game, c);
        }
        images(&hud(&mut st, &mut game))
    };
    assert_eq!(
        look(None),
        ["[0.0, 0.0, 100.0, 100.0] from [0.0, 0.0, 2.0, 1.0] ffffffff"]
    );
    // Fit: as large as fits, in the middle.
    assert_eq!(
        look(Some("fit")),
        ["[0.0, 25.0, 100.0, 50.0] from [0.0, 0.0, 2.0, 1.0] ffffffff"]
    );
    // Crop: covers the box; only the image's middle shows.
    assert_eq!(
        look(Some("crop")),
        ["[0.0, 0.0, 100.0, 100.0] from [0.5, 0.0, 1.0, 1.0] ffffffff"]
    );
    // Tile: repeated at TileSize, the last ones cut at the edge.
    assert_eq!(
        look(Some("tile")),
        [
            "[0.0, 0.0, 50.0, 50.0] from [0.0, 0.0, 2.0, 1.0] ffffffff",
            "[50.0, 0.0, 50.0, 50.0] from [0.0, 0.0, 2.0, 1.0] ffffffff",
            "[0.0, 50.0, 50.0, 50.0] from [0.0, 0.0, 2.0, 1.0] ffffffff",
            "[50.0, 50.0, 50.0, 50.0] from [0.0, 0.0, 2.0, 1.0] ffffffff"
        ]
    );
    let tiles = look(Some("tile40"));
    assert_eq!(tiles.len(), 9);
    assert_eq!(tiles[8], "[80.0, 80.0, 20.0, 20.0] from [0.0, 0.0, 1.0, 0.5] ffffffff");
    // Slice: the corners keep their size (times SliceScale), the edges and
    // the middle stretch.
    assert_eq!(
        look(Some("slice")),
        [
            "[0.0, 0.0, 10.0, 10.0] from [0.0, 0.0, 1.0, 1.0] ffffffff",
            "[10.0, 0.0, 80.0, 10.0] from [1.0, 0.0, 2.0, 1.0] ffffffff",
            "[90.0, 0.0, 10.0, 10.0] from [3.0, 0.0, 1.0, 1.0] ffffffff",
            "[0.0, 10.0, 10.0, 40.0] from [0.0, 1.0, 1.0, 2.0] ffffffff",
            "[10.0, 10.0, 80.0, 40.0] from [1.0, 1.0, 2.0, 2.0] ffffffff",
            "[90.0, 10.0, 10.0, 40.0] from [3.0, 1.0, 1.0, 2.0] ffffffff",
            "[0.0, 50.0, 10.0, 10.0] from [0.0, 3.0, 1.0, 1.0] ffffffff",
            "[10.0, 50.0, 80.0, 10.0] from [1.0, 3.0, 2.0, 1.0] ffffffff",
            "[90.0, 50.0, 10.0, 10.0] from [3.0, 3.0, 1.0, 1.0] ffffffff"
        ]
    );
    // ImageRectOffset and ImageRectSize show a part; ImageColor3 tints it
    // and ImageTransparency fades it.
    assert_eq!(
        look(Some("part")),
        ["[0.0, 0.0, 100.0, 100.0] from [1.0, 0.0, 1.0, 1.0] 80ff0000"]
    );
}

#[test]
fn images_without_textures_draw_in_blocks() {
    // Where the game can't draw an image as such, its part is drawn as
    // blocks of colour, tinted the same.
    let mut game = FakeGame::new("rbx-blocks");
    game.no_images = true;
    let mut st = studio(&mut game);
    let ws = st.s.rbx.workspace_dir.clone();
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("wide.png"), png(2, 1, &[[255, 0, 0, 255], [0, 0, 255, 255]])).unwrap();
    let src = r#"
        local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
        local image = Instance.new("ImageLabel", gui)
        image.Size = UDim2.fromOffset(100, 100)
        image.BackgroundTransparency = 1
        image.Image = getcustomasset("wide.png")
        command("part", function()
            image.ImageRectOffset = Vector2.new(1, 0)
            image.ImageRectSize = Vector2.new(1, 1)
            image.ImageColor3 = Color3.new(1, 1, 0.5)
        end)
    "#;
    load(&mut game, &mut st, "blocks", src, 0.6);
    assert_eq!(
        hud(&mut st, &mut game),
        [
            "rect [0.0, 0.0, 50.3, 100.3] r0 ffff0000",
            "rect [50.0, 0.0, 50.3, 100.3] r0 ff0000ff"
        ]
    );
    say(&mut st, &mut game, "part");
    assert_eq!(hud(&mut st, &mut game), ["rect [0.0, 0.0, 100.3, 100.3] r0 ff000080"]);
}

#[test]
fn rich_text_draws_its_runs() {
    let mut game = FakeGame::new("rbx-rich");
    let mut st = studio(&mut game);
    let src = r##"
        local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
        local label = Instance.new("TextLabel", gui)
        label.Size = UDim2.fromOffset(300, 40)
        label.BackgroundTransparency = 1
        label.TextXAlignment = Enum.TextXAlignment.Left
        label.TextColor3 = Color3.new(1, 1, 1)
        label.RichText = true
        label.Text = '<font color="#FF0000">Red</font> <u>under</u><br/><mark color="#00FF00">marked</mark>'
        print(label.ContentText == "Red under\nmarked", label.TextBounds)
    "##;
    load(&mut game, &mut st, "rich", src, 0.6);
    assert_eq!(printed(&st, "rich"), ["true    54, 16"]);
    let drawn = hud(&mut st, &mut game);
    // Two lines of 8 in the middle of 40: from 12 and 20, each run in its colour.
    assert_eq!(texts(&drawn), ["Red", " ", "under", "marked"], "{drawn:#?}");
    assert!(drawn.contains(&"text Red @[0.0, 10.2] ff0000".to_owned()), "{drawn:#?}");
    assert!(drawn.contains(&"text under @[24.0, 10.2] ffffff".to_owned()));
    // The underline, under "under".
    let under = index_of(&drawn, "text under @");
    assert!(
        drawn[under + 1].starts_with("rect [24.0, ") && drawn[under + 1].ends_with(", 30.0, 1.0] r0 ffffffff"),
        "{drawn:#?}"
    );
    // The mark, behind "marked".
    let marked = index_of(&drawn, "text marked @");
    assert_eq!(drawn[marked - 1], "rect [0.0, 20.0, 36.0, 8.0] r0 ff00ff00");
}

/// A drawn rectangle's x, y, width and height.
fn rect_of(drawn: &str) -> [f32; 4] {
    let inner = drawn.split('[').nth(1).and_then(|r| r.split(']').next()).unwrap_or("");
    let n: Vec<f32> = inner.split(',').map(|v| v.trim().parse().unwrap()).collect();
    [n[0], n[1], n[2], n[3]]
}

#[track_caller]
fn near_rect(got: [f32; 4], want: [f32; 4]) {
    assert!(
        got.iter().zip(want).all(|(g, w)| (g - w).abs() < 0.01),
        "{got:?} is not {want:?}"
    );
}

#[test]
fn billboard_guis_float_over_parts() {
    let mut game = FakeGame::new("rbx-billboard");
    let mut st = studio(&mut game);
    // The camera is at the player's eyes (10.5, 65.62, -3.25) looking north
    // (-z) with 70 degrees, on a 960 x 540 screen: the part is 10 ahead, at
    // the screen's middle.
    let src = r#"
        local part = Instance.new("Part", workspace)
        part.Size = Vector3.new(1, 1, 1)
        part.Transparency = 1
        part.CFrame = CFrame.new(10.5, 65.62, -13.25)
        local board = Instance.new("BillboardGui", part)
        board.Size = UDim2.fromOffset(100, 20)
        local tag = Instance.new("Frame", board)
        tag.Size = UDim2.fromScale(1, 1)
        tag.BackgroundColor3 = Color3.new(1, 0, 0)
        tag.BorderSizePixel = 0
        local gui = Instance.new("ScreenGui", game.Players.LocalPlayer.PlayerGui)
        local over = Instance.new("Frame", gui)
        over.Size = UDim2.fromOffset(10, 10)
        over.BorderSizePixel = 0
        local function set(target, props)
            return function()
                for k, v in props do
                    target[k] = v
                end
                return tostring(tag.AbsolutePosition) .. " " .. tostring(tag.AbsoluteSize)
            end
        end
        command("up", set(board, { StudsOffset = Vector3.new(0, 1, 0) }))
        command("behind", set(part, { CFrame = CFrame.new(10.5, 65.62, 6.75) }))
        command("back", set(part, { CFrame = CFrame.new(10.5, 65.62, -13.25) }))
        command("far", set(board, { MaxDistance = 5 }))
        command("near", set(board, { MaxDistance = math.huge }))
        command("top", set(board, { AlwaysOnTop = true }))
        command("studs", set(board, { StudsOffset = Vector3.zero, Size = UDim2.new(2, 0, 1, 0) }))
        command("closer", set(part, { CFrame = CFrame.new(10.5, 65.62, -8.25) }))
        command("hide", set(board, { PlayerToHideFrom = game.Players.LocalPlayer }))
        command("show", function() board.PlayerToHideFrom = nil end)
        command("away", function()
            board.Size = UDim2.fromOffset(100, 20)
            board.Adornee = part
            board.Parent = game.Players.LocalPlayer.PlayerGui
            local button = Instance.new("TextButton", board)
            button.Size = UDim2.fromScale(1, 1)
            button.BackgroundTransparency = 1
            button.Text = ""
        end)
        command("active", set(board, { Active = true }))
    "#;
    load(&mut game, &mut st, "board", src, 0.6);
    let tag = |st: &mut Studio, game: &mut FakeGame| -> Option<[f32; 4]> {
        let drawn = hud(st, game);
        let at = drawn.iter().position(|d| d.ends_with(" r0 ffff0000"))?;
        // Billboards go under the ScreenGuis.
        let over = index_of(&drawn, "rect [0.0, 0.0, 10.0, 10.0]");
        assert!(at < over, "{drawn:#?}");
        Some(rect_of(&drawn[at]))
    };
    near_rect(tag(&mut st, &mut game).unwrap(), [430.0, 260.0, 100.0, 20.0]);
    // Inside a billboard, AbsolutePosition is from its corner.
    assert_eq!(say(&mut st, &mut game, "up"), "0, 0 100, 20");
    // A stud up, 10 away: 270 less 270 x tan(35) / 10 x ... in pixels.
    let per_stud = 540.0 / (2.0 * 35f32.to_radians().tan() * 10.0);
    near_rect(tag(&mut st, &mut game).unwrap(), [430.0, 260.0 - per_stud, 100.0, 20.0]);
    // Behind the camera, or past MaxDistance, it isn't drawn.
    say(&mut st, &mut game, "behind");
    assert_eq!(tag(&mut st, &mut game), None);
    say(&mut st, &mut game, "back");
    assert!(tag(&mut st, &mut game).is_some());
    say(&mut st, &mut game, "far");
    assert_eq!(tag(&mut st, &mut game), None);
    say(&mut st, &mut game, "near");
    // A block between hides it, unless AlwaysOnTop.
    game.blocks.insert((10, 65, -8), "minecraft:stone".into());
    assert_eq!(tag(&mut st, &mut game), None);
    say(&mut st, &mut game, "top");
    assert!(tag(&mut st, &mut game).is_some());
    game.blocks.clear();
    // A Size in studs shrinks with the distance: twice as big at half of it.
    say(&mut st, &mut game, "studs");
    let far = tag(&mut st, &mut game).unwrap();
    assert!(
        (far[2] - 2.0 * per_stud).abs() < 0.01 && (far[3] - per_stud).abs() < 0.01,
        "{far:?}"
    );
    say(&mut st, &mut game, "closer");
    let near = tag(&mut st, &mut game).unwrap();
    assert!((near[2] - 4.0 * per_stud).abs() < 0.01, "{near:?}");
    // Hidden from this player.
    say(&mut st, &mut game, "hide");
    assert_eq!(tag(&mut st, &mut game), None);
    say(&mut st, &mut game, "show");
    say(&mut st, &mut game, "back");
    // In the PlayerGui with an Adornee; Active ones take the pointer.
    say(&mut st, &mut game, "away");
    near_rect(tag(&mut st, &mut game).unwrap(), [430.0, 260.0, 100.0, 20.0]);
    assert!(objects_at(&st.s.rbx, 480.0, 270.0).is_empty(), "not Active");
    say(&mut st, &mut game, "active");
    hud(&mut st, &mut game);
    assert_eq!(objects_at(&st.s.rbx, 480.0, 270.0).len(), 2, "the button and the frame");
}

#[test]
fn billboard_offsets_follow_the_adornee_box() {
    let mut game = FakeGame::new("rbx-extents");
    let mut st = studio(&mut game);
    // A model of two parts, 10 ahead: its box runs from 64.12 to 67.12 up,
    // its middle at the camera's height.
    let src = r#"
        local model = Instance.new("Model", workspace)
        for _, y in { 64.62, 66.62 } do
            local p = Instance.new("Part", model)
            p.Size = Vector3.new(1, 1, 1)
            p.Transparency = 1
            p.CFrame = CFrame.new(10.5, y, -13.25)
        end
        local board = Instance.new("BillboardGui", model)
        board.Size = UDim2.fromOffset(40, 10)
        local f = Instance.new("Frame", board)
        f.Size = UDim2.fromScale(1, 1)
        f.BackgroundColor3 = Color3.new(0, 1, 0)
        f.BorderSizePixel = 0
        command("top", function() board.ExtentsOffset = Vector3.new(0, 1, 0) end)
        command("world", function()
            board.ExtentsOffset = Vector3.zero
            board.StudsOffsetWorldSpace = Vector3.new(0, 2, 0)
        end)
        command("shift", function()
            board.StudsOffsetWorldSpace = Vector3.zero
            board.SizeOffset = Vector2.new(0.5, 0.5)
        end)
    "#;
    load(&mut game, &mut st, "extents", src, 0.6);
    let green = |st: &mut Studio, game: &mut FakeGame| {
        let drawn = hud(st, game);
        rect_of(&drawn[index_of(&drawn, "rect ")])
    };
    let per_stud = 540.0 / (2.0 * 35f32.to_radians().tan() * 10.0);
    near_rect(green(&mut st, &mut game), [460.0, 265.0, 40.0, 10.0]);
    // ExtentsOffset is in halves of the box: the top of the model.
    say(&mut st, &mut game, "top");
    near_rect(green(&mut st, &mut game), [460.0, 265.0 - 1.5 * per_stud, 40.0, 10.0]);
    // StudsOffsetWorldSpace, in studs along the adornee's axes.
    say(&mut st, &mut game, "world");
    near_rect(green(&mut st, &mut game), [460.0, 265.0 - 2.0 * per_stud, 40.0, 10.0]);
    // SizeOffset, in fractions of the billboard's size, y up.
    say(&mut st, &mut game, "shift");
    near_rect(green(&mut st, &mut game), [480.0, 260.0, 40.0, 10.0]);
}
