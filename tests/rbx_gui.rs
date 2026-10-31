// The Roblox layer's GUIs on a fake game, beyond tests/roblox.rs: enums and
// properties scripts set and read (ElasticBehavior, ContentText, GuiState,
// the Transparency alias, an ImageButton's hover and pressed images,
// IsLoaded), and classes that stay lenient.

mod common;

use common::{FakeGame, log, mod_index, run_for, studio};
use horizon_studio::game::hstudio::{ModState, Studio};
use horizon_studio::game::hstudio_rbx_draw::{OUTPUTS, fresh_layout, lay_out, pointer_button, pointer_moved};
use horizon_studio::game::hstudio_rbx_values::get;

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
