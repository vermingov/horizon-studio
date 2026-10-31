// The Roblox layer's GUIs on a fake game, beyond tests/roblox.rs: enums and
// properties scripts set and read (ElasticBehavior, ContentText, GuiState,
// the Transparency alias, an ImageButton's hover and pressed images,
// IsLoaded), and classes that stay lenient.

mod common;

use common::{FakeGame, log, mod_index, run_for, studio};
use horizon_studio::game::hstudio::{ModState, Studio};
use horizon_studio::game::hstudio_look::draw_screen;
use horizon_studio::game::hstudio_rbx_draw::{OUTPUTS, fresh_layout, lay_out, pointer_button, pointer_moved, wheel};
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
    // The row, scrolled and cut to the window as a ScrollingFrame clips.
    let row = "rect [0.0, 0.0, 200.0, 100.0] r0 ffff0000";
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
