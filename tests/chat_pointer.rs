// While the game's chat is open (the pointer free), the pointer reaches the
// scripts' Roblox GUI: a press on it is the GUI's, not the chat's, and a
// script's own drag code moves its window.

mod common;

use common::{FakeGame, log, run_for, studio};
use horizon_studio::game::hstudio_ui::Input;

const DRAG: &str = r#"
local UIS = game:GetService("UserInputService")
local gui = Instance.new("ScreenGui")
gui.Parent = game.Players.LocalPlayer:WaitForChild("PlayerGui")
local frame = Instance.new("Frame")
frame.Position = UDim2.new(0, 100, 0, 100)
frame.Size = UDim2.new(0, 200, 0, 50)
frame.Active = true
frame.Parent = gui
local dragging, start, startPos
frame.InputBegan:Connect(function(i)
    if i.UserInputType == Enum.UserInputType.MouseButton1 then
        dragging, start, startPos = true, i.Position, frame.Position
    end
end)
UIS.InputEnded:Connect(function(i)
    if i.UserInputType == Enum.UserInputType.MouseButton1 and dragging then
        dragging = false
        print("at", frame.Position.X.Offset, frame.Position.Y.Offset)
    end
end)
UIS.InputChanged:Connect(function(i)
    if dragging and i.UserInputType == Enum.UserInputType.MouseMovement then
        local d = i.Position - start
        frame.Position = UDim2.new(0, startPos.X.Offset + d.X, 0, startPos.Y.Offset + d.Y)
    end
end)
"#;

#[test]
fn the_chats_free_pointer_drags_a_scripts_window() {
    let mut game = FakeGame::new("chat-pointer-drag");
    let mut st = studio(&mut game);
    game.write_mod("drag.lua", DRAG);
    for _ in 0..12 {
        run_for(&mut st, &mut game, 0.05);
        st.hud(&mut game);
    }
    let k = st.s.ui.dp_factor();
    let px = |v: f32| v / k;
    // Off the window: the chat keeps its press.
    assert!(!st.chat_pointer(&Input::Mouse(px(20.0), px(20.0))));
    assert!(!st.chat_pointer(&Input::Button { left: true, down: true }));
    assert!(!st.chat_pointer(&Input::Button {
        left: true,
        down: false
    }));
    // On it: the GUI's, and the script's drag moves the window.
    st.chat_pointer(&Input::Mouse(px(150.0), px(120.0)));
    assert!(
        st.chat_pointer(&Input::Button { left: true, down: true }),
        "the press is the GUI's"
    );
    run_for(&mut st, &mut game, 0.05);
    st.chat_pointer(&Input::Mouse(px(190.0), px(150.0)));
    run_for(&mut st, &mut game, 0.05);
    assert!(
        st.chat_pointer(&Input::Button {
            left: true,
            down: false
        }),
        "its release too"
    );
    run_for(&mut st, &mut game, 0.05);
    assert!(
        log(&st).contains(&"[drag] at    140    130".to_owned()),
        "{:?}",
        log(&st)
    );
    // The wheel over it is the GUI's, elsewhere the chat's.
    st.hud(&mut game);
    st.chat_pointer(&Input::Mouse(px(190.0), px(150.0)));
    assert!(st.chat_pointer(&Input::Wheel(1.0)));
    st.chat_pointer(&Input::Mouse(px(600.0), px(400.0)));
    assert!(!st.chat_pointer(&Input::Wheel(1.0)));
}

#[test]
fn stop_ends_what_the_console_ran_and_it_runs_again() {
    let mut game = FakeGame::new("console-stop");
    let mut st = studio(&mut game);
    run_for(&mut st, &mut game, 0.2);
    let src = r#"
        if getgenv().LOADED then print("already loaded") return end
        getgenv().LOADED = true
        local gui = Instance.new("ScreenGui")
        gui.Name = "Thing"
        gui.Parent = gethui()
        task.spawn(function() while true do task.wait(0.1) _G.ticks = (_G.ticks or 0) + 1 end end)
        print("loaded")
    "#;
    st.run_console(&mut game, &format!("loadstring({src:?})()"));
    run_for(&mut st, &mut game, 0.5);
    assert!(st.s.console_running(), "its thread and its GUI");
    let out = st.run_console(&mut game, "stop()");
    assert_eq!(out.last().map(|l| l.1.as_str()), Some("Stopped what the console ran"));
    assert!(!st.s.console_running(), "nothing of it is left");
    let out = st.run_console(&mut game, "print(gethui():FindFirstChild('Thing'))");
    assert_eq!(out.last().map(|l| l.1.as_str()), Some("nil"), "its GUI went");
    // It runs again: getgenv starts afresh.
    let out = st.run_console(&mut game, &format!("loadstring({src:?})()"));
    assert_eq!(out.last().map(|l| l.1.as_str()), Some("loaded"));
    let out = st.run_console(&mut game, "stop() stop()");
    assert_eq!(
        out.last().map(|l| l.1.as_str()),
        Some("Nothing the console ran is running")
    );
}

#[test]
fn with_the_chat_open_the_pointer_stays_where_the_mouse_is_between_frames() {
    let mut game = FakeGame::new("chat-pointer-stays");
    let mut st = studio(&mut game);
    game.write_mod(
        "btn.lua",
        r#"
        local gui = Instance.new("ScreenGui")
        local b = Instance.new("TextButton", gui)
        b.Position = UDim2.new(0, 100, 0, 100)
        b.Size = UDim2.new(0, 120, 0, 40)
        b.MouseButton1Click:Connect(function() print("clicked") end)
        b.MouseEnter:Connect(function() print("over") end)
        gui.Parent = game.Players.LocalPlayer:WaitForChild("PlayerGui")
        "#,
    );
    st.set_chat_open(true);
    for _ in 0..12 {
        run_for(&mut st, &mut game, 0.05);
        st.hud(&mut game);
    }
    let k = st.s.ui.dp_factor();
    st.chat_pointer(&Input::Mouse(150.0 / k, 120.0 / k));
    // Frames go by with no mouse event: the pointer is still over the
    // button, not at the crosshair, so the click is the button's.
    for _ in 0..6 {
        run_for(&mut st, &mut game, 0.05);
        st.hud(&mut game);
    }
    assert!(st.chat_pointer(&Input::Button { left: true, down: true }));
    run_for(&mut st, &mut game, 0.05);
    assert!(st.chat_pointer(&Input::Button {
        left: true,
        down: false
    }));
    run_for(&mut st, &mut game, 0.1);
    let printed: Vec<String> = log(&st)
        .iter()
        .filter_map(|l| l.strip_prefix("[btn] ").map(str::to_owned))
        .collect();
    assert_eq!(printed, ["over", "clicked"], "{:?}", log(&st));
    // The chat closes: the pointer is the game's crosshair again.
    st.set_chat_open(false);
    run_for(&mut st, &mut game, 0.05);
    st.hud(&mut game);
    assert_ne!((st.s.rbx.pointer.x, st.s.rbx.pointer.y), (150.0, 120.0));
}
