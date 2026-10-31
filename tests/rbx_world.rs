// The Roblox layer's world on a fake game (hstudio_rbx_world): the
// LocalPlayer is the player playing, and its Character an R6 rig that
// follows them.

mod common;

use common::{FakeGame, log, mod_index, run_for, studio};
use horizon_studio::game::hstudio::{ModState, Studio};

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

/// `v` printed with three decimals, as the scripts below print vectors.
const FMT: &str = r#"
    local function v(x) return string.format("%.3f %.3f %.3f", x.X, x.Y, x.Z) end
"#;

#[test]
fn the_character_is_an_r6_rig_on_the_player() {
    let mut game = FakeGame::new("rbxw-rig");
    let mut st = studio(&mut game);
    let src = format!(
        "{FMT}{}",
        r#"
        local char = game.Players.LocalPlayer.Character
        local root, head, hum = char.HumanoidRootPart, char.Head, char.Humanoid
        print(char.PrimaryPart == root, hum.RootPart == root, hum.RigType, hum.HipHeight)
        print(v(root.Size), v(head.Size))
        print(v(root.Position), v(root.CFrame.LookVector))
        print(v(head.Position), v(head.CFrame.LookVector))
        print(root.Transparency, root.Anchored, root.CanCollide, root.CanTouch, root.Massless, head.CanCollide)
        print(pcall(function() char.Parent = nil end))
        print(pcall(function() root:Destroy() end))
        local tag = Instance.new("Folder")
        tag.Parent = char
        print(tag.Parent == char)
    "#
    );
    load(&mut game, &mut st, "rig", &src, 0.7);
    let p = printed(&st, "rig");
    assert_eq!(p[0], "true    true    Enum.HumanoidRigType.R6    0.703125");
    // The torso's box (8 x 12 x 4 pixels) and the head's cube, 15/16 of a block-sized model.
    assert_eq!(p[1], "0.469 0.703 0.234    0.469 0.469 0.469");
    // At the torso's centre over the feet (10.5, 64, -3.25), facing north (yaw 180).
    assert_eq!(p[2], "10.500 65.055 -3.250    0.000 0.000 -1.000");
    assert_eq!(p[3], "10.500 65.620 -3.250    0.000 0.000 -1.000");
    assert_eq!(p[4], "1    true    false    false    true    false");
    assert!(p[5].starts_with("false") && p[5].contains("locked"), "{}", p[5]);
    assert!(p[6].starts_with("false") && p[6].contains("locked"), "{}", p[6]);
    assert_eq!(p[7], "true");
}
