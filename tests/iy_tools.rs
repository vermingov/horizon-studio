// The Backpack as the player's inventory and hotbar, the tools scripts make
// (BTools' HopperBins, F3X's Tool), and Infinite Yield's tool and part
// commands, as the Roblox calls they make, on the fake game's inventory.

mod common;
use common::{FakeGame, log, run_for, studio};
use horizon_studio::game::hstudio::{EntityInfo, Studio};

/// What the console printed for `code`, after `secs` more.
fn lua(st: &mut Studio, game: &mut FakeGame, code: &str, secs: f64) -> Vec<String> {
    // A line only the run prints (the console echoes the code): what
    // follows it is the code's.
    st.run_console(game, &format!("print(string.char(1)) {code}"));
    run_for(st, game, secs);
    let all = log(st);
    let from = all.iter().rposition(|l| l == "\u{1}").map_or(all.len(), |i| i + 1);
    all[from..].to_vec()
}

/// What the layer asked of the game's hotbar and world.
fn acts(game: &FakeGame) -> Vec<String> {
    let kinds = ["select ", "use ", "attack ", "drop ", "block ", "hide "];
    game.moves
        .iter()
        .filter(|m| kinds.iter().any(|k| m.starts_with(k)))
        .cloned()
        .collect()
}

/// A game whose player has a pickaxe in slot 1, bread in slot 3 (slot 2
/// empty) and stone above the hotbar, with the layer awake.
fn stocked(name: &str) -> (FakeGame, Studio) {
    let mut game = FakeGame::new(name);
    game.give(&[
        (0, "minecraft:diamond_pickaxe", 1),
        (2, "minecraft:bread", 5),
        (12, "minecraft:stone", 64),
    ]);
    let mut st = studio(&mut game);
    lua(
        &mut st,
        &mut game,
        "lp = game.Players.LocalPlayer bp = lp.Backpack ch = lp.Character",
        0.1,
    );
    (game, st)
}

const LIST: &str = r#"
function names(p)
  local t = {}
  for _, c in ipairs(p:GetChildren()) do
    if c:IsA("BackpackItem") then t[#t + 1] = c.Name end
  end
  table.sort(t)
  return table.concat(t, ",")
end
"#;

#[test]
fn the_backpack_holds_a_tool_for_each_stack_and_the_character_the_one_in_hand() {
    let (mut game, mut st) = stocked("tools-mirror");
    let out = lua(
        &mut st,
        &mut game,
        &format!(
            "{LIST} print(names(bp)) print(names(ch))
            local t = ch:FindFirstChildOfClass('Tool')
            print(t.ClassName, t:GetAttribute('ItemId'), t:GetAttribute('Count'), t:GetAttribute('Slot'), t.TextureId)
            print(t.Handle.ClassName, bp.Bread:GetAttribute('Count'), bp.Stone:GetAttribute('Slot'))"
        ),
        0.1,
    );
    assert_eq!(
        out,
        [
            "Bread,Stone",
            "Diamond Pickaxe",
            "Tool    minecraft:diamond_pickaxe    1    1    rbxasset://minecraft/textures/item/diamond_pickaxe.png",
            "Part    5    13",
        ]
    );
    // A stack that changes, one that comes and one that goes.
    let inv = game.inventory.as_mut().unwrap();
    inv[2] = Some(("minecraft:bread".into(), 3));
    inv[5] = Some(("minecraft:torch".into(), 16));
    inv[12] = None;
    run_for(&mut st, &mut game, 0.1);
    let out = lua(
        &mut st,
        &mut game,
        "print(names(bp), bp.Bread:GetAttribute('Count'), bp.Torch:GetAttribute('Slot'))",
        0.1,
    );
    assert_eq!(out, ["Bread,Torch    3    6"]);
}

#[test]
fn choosing_a_slot_in_the_game_moves_the_tools_with_their_events() {
    let (mut game, mut st) = stocked("tools-choose");
    lua(
        &mut st,
        &mut game,
        "for _, t in ipairs(bp:GetChildren()) do
           t.Equipped:Connect(function(m) print('equipped', t.Name, m == lp:GetMouse()) end)
         end
         ch['Diamond Pickaxe'].Unequipped:Connect(function() print('unequipped pickaxe') end)",
        0.1,
    );
    game.player.as_mut().unwrap().slot = 2;
    run_for(&mut st, &mut game, 0.1);
    let out = lua(
        &mut st,
        &mut game,
        &format!("{LIST} print(names(bp), '|', names(ch))"),
        0.1,
    );
    let all = log(&st);
    assert!(all.contains(&"unequipped pickaxe".to_owned()), "{all:?}");
    assert!(all.contains(&"equipped    Bread    true".to_owned()), "{all:?}");
    assert_eq!(out, ["Diamond Pickaxe,Stone    |    Bread"]);
    // An empty slot: nothing in hand.
    game.player.as_mut().unwrap().slot = 1;
    run_for(&mut st, &mut game, 0.1);
    let out = lua(&mut st, &mut game, "print(names(ch) == '')", 0.1);
    assert_eq!(out, ["true"]);
    assert!(
        acts(&game).is_empty(),
        "the player's own choice asks nothing: {:?}",
        game.moves
    );
}

#[test]
fn equipping_a_tool_selects_its_slot_and_unequipping_selects_an_empty_one() {
    let (mut game, mut st) = stocked("tools-equip");
    lua(&mut st, &mut game, "ch.Humanoid:EquipTool(bp.Bread)", 0.1);
    assert_eq!(acts(&game), ["select 2"]);
    assert_eq!(game.player.as_ref().unwrap().slot, 2);
    let out = lua(
        &mut st,
        &mut game,
        &format!("{LIST} print(names(bp), '|', names(ch))"),
        0.1,
    );
    assert_eq!(out, ["Diamond Pickaxe,Stone    |    Bread"]);
    // UnequipTools: the nearest empty hotbar slot (slot 2, the second).
    lua(&mut st, &mut game, "ch.Humanoid:UnequipTools()", 0.1);
    assert_eq!(acts(&game), ["select 2", "select 1"]);
    let out = lua(&mut st, &mut game, "print(names(ch) == '', #bp:GetChildren())", 0.1);
    assert_eq!(out, ["true    3"]);
    // Parent = character does the same as EquipTool.
    lua(&mut st, &mut game, "bp['Diamond Pickaxe'].Parent = ch", 0.1);
    assert_eq!(acts(&game).last().unwrap(), "select 0");
}

#[test]
fn unequipping_with_a_full_hotbar_holds_nothing_and_the_number_keys_give_it_back() {
    let mut game = FakeGame::new("tools-full");
    let stacks: Vec<(usize, String, u32)> = (0..9).map(|i| (i, format!("minecraft:item{i}"), 1)).collect();
    let refs: Vec<(usize, &str, u32)> = stacks.iter().map(|(i, s, n)| (*i, s.as_str(), *n)).collect();
    game.give(&refs);
    let mut st = studio(&mut game);
    lua(&mut st, &mut game, "lp = game.Players.LocalPlayer", 0.1);
    let out = lua(
        &mut st,
        &mut game,
        "lp.Character.Humanoid:UnequipTools() task.wait(0.1) print(lp.Character:FindFirstChildOfClass('Tool'))",
        0.3,
    );
    assert_eq!(out, ["nil"]);
    assert!(acts(&game).is_empty(), "{:?}", game.moves);
    // Pressing the slot's own number takes it back in hand.
    st.key("key.keyboard.1", true);
    st.key("key.keyboard.1", false);
    run_for(&mut st, &mut game, 0.1);
    let out = lua(
        &mut st,
        &mut game,
        "print(lp.Character:FindFirstChildOfClass('Tool').Name)",
        0.1,
    );
    assert_eq!(out, ["Item0"]);
}

#[test]
fn using_the_item_fires_activated_and_activate_uses_it() {
    let (mut game, mut st) = stocked("tools-use");
    lua(
        &mut st,
        &mut game,
        "local t = ch['Diamond Pickaxe']
         t.Activated:Connect(function() print('activated') end)
         t.Deactivated:Connect(function() print('deactivated') end)",
        0.1,
    );
    st.key("key.mouse.right", true);
    run_for(&mut st, &mut game, 0.1);
    st.key("key.mouse.right", false);
    run_for(&mut st, &mut game, 0.1);
    let all = log(&st);
    let fired: Vec<&String> = all.iter().filter(|l| l.ends_with("activated")).collect();
    assert_eq!(fired, ["activated", "deactivated"]);
    assert!(
        acts(&game).is_empty(),
        "the game uses the item itself: {:?}",
        game.moves
    );
    // Tool:Activate uses the item, its slot selected first (a tick before).
    lua(&mut st, &mut game, "bp.Bread.Parent = ch bp.Bread = nil", 0.0);
    lua(&mut st, &mut game, "ch.Bread:Activate()", 0.3);
    assert_eq!(acts(&game), ["select 2", "use 2"]);
}

#[test]
fn a_tool_put_in_the_workspace_drops_its_stack() {
    let (mut game, mut st) = stocked("tools-drop");
    // IY's droptools: every tool into the character, then into the Workspace.
    lua(
        &mut st,
        &mut game,
        "for _, v in ipairs(bp:GetChildren()) do v.Parent = ch end
         task.wait()
         for _, v in ipairs(ch:GetChildren()) do if v:IsA('Tool') then v.Parent = workspace end end",
        0.6,
    );
    assert!(game.moves.contains(&"drop 0 all".to_owned()), "{:?}", game.moves);
    assert!(game.moves.contains(&"drop 2 all".to_owned()), "{:?}", game.moves);
    let inv = game.inventory.as_ref().unwrap();
    assert!(inv[0].is_none() && inv[2].is_none());
    // The stone, above the hotbar, can't be dropped from here.
    assert!(inv[12].is_some());
    let out = lua(
        &mut st,
        &mut game,
        "print(workspace:FindFirstChild('Bread'), workspace:FindFirstChild('Diamond Pickaxe'))",
        0.1,
    );
    assert_eq!(out, ["nil    nil"]);
}

#[test]
fn a_new_character_holds_the_stack_in_hand_again() {
    let (mut game, mut st) = stocked("tools-respawn");
    let out = lua(
        &mut st,
        &mut game,
        "lp:LoadCharacter() task.wait(0.2) print(lp.Character ~= ch, lp.Character:FindFirstChildOfClass('Tool').Name)",
        0.4,
    );
    assert_eq!(out, ["true    Diamond Pickaxe"]);
}

#[test]
fn a_destroyed_stack_tool_leaves_the_backpack_until_the_stack_changes() {
    let (mut game, mut st) = stocked("tools-destroy");
    lua(&mut st, &mut game, "bp.Bread:Destroy()", 0.2);
    let out = lua(&mut st, &mut game, "print(bp:FindFirstChild('Bread'))", 0.1);
    assert_eq!(out, ["nil"]);
    game.inventory.as_mut().unwrap()[2] = Some(("minecraft:bread".into(), 6));
    run_for(&mut st, &mut game, 0.1);
    let out = lua(&mut st, &mut game, "print(bp.Bread:GetAttribute('Count'))", 0.1);
    assert_eq!(out, ["6"]);
}

/// A script's own Tool, held with 0: the mouse's left button is its.
#[test]
fn a_scripts_tool_is_an_extra_slot_that_takes_the_clicks() {
    let (mut game, mut st) = stocked("tools-own");
    lua(
        &mut st,
        &mut game,
        "local t = Instance.new('Tool') t.Name = 'Teleport' t.RequiresHandle = false t.Parent = bp
         t.Equipped:Connect(function(m) print('equipped', m.ClassName) end)
         t.Activated:Connect(function() print('activated') end)
         t.Deactivated:Connect(function() print('deactivated') end)
         lp:GetMouse().Button1Down:Connect(function() print('button1') end)",
        0.1,
    );
    game.drawn.clear();
    st.hud(&mut game);
    assert!(
        game.drawn.iter().any(|d| d.starts_with("text Tel ")),
        "{:?}",
        game.drawn
    );
    st.key("key.keyboard.0", true);
    st.key("key.keyboard.0", false);
    run_for(&mut st, &mut game, 0.1);
    let out = lua(
        &mut st,
        &mut game,
        &format!("{LIST} print(names(ch), '|', names(bp))"),
        0.1,
    );
    assert_eq!(out, ["Teleport    |    Bread,Diamond Pickaxe,Stone"]);
    assert!(horizon_studio::game::hstudio_rbx_tools::takes_clicks(&st.s.rbx));
    st.key("key.mouse.left", true);
    run_for(&mut st, &mut game, 0.1);
    st.key("key.mouse.left", false);
    run_for(&mut st, &mut game, 0.1);
    let all = log(&st);
    for line in ["equipped    PlayerMouse", "activated", "button1", "deactivated"] {
        assert!(all.contains(&line.to_owned()), "{line}: {all:?}");
    }
    game.drawn.clear();
    st.hud(&mut game);
    assert!(
        game.drawn.iter().any(|d| d.starts_with("text Teleport ")),
        "{:?}",
        game.drawn
    );
    // Back to the game's slot 1 with its key: the pickaxe in hand again.
    st.key("key.keyboard.1", true);
    run_for(&mut st, &mut game, 0.1);
    let out = lua(&mut st, &mut game, "print(names(ch))", 0.1);
    assert_eq!(out, ["Diamond Pickaxe"]);
    assert!(!horizon_studio::game::hstudio_rbx_tools::takes_clicks(&st.s.rbx));
}

#[test]
fn the_wheel_goes_past_the_hotbar_into_the_scripts_tools() {
    use horizon_studio::game::hstudio_rbx_tools::wheel;
    let (mut game, mut st) = stocked("tools-wheel");
    lua(
        &mut st,
        &mut game,
        "for _, n in ipairs({'A', 'B'}) do local t = Instance.new('Tool') t.Name = n t.Parent = bp end",
        0.1,
    );
    // Within the hotbar the wheel is the game's.
    assert_eq!(wheel(&mut st.s.rbx, -1.0, 0), None);
    // Slot 9, down: the first tool, then the second, then slot 1 again
    // (which the bridge asks the game for).
    game.player.as_mut().unwrap().slot = 8;
    run_for(&mut st, &mut game, 0.1);
    assert_eq!(wheel(&mut st.s.rbx, -1.0, 8), Some(None));
    run_for(&mut st, &mut game, 0.1);
    assert_eq!(
        lua(&mut st, &mut game, "print(ch:FindFirstChildOfClass('Tool').Name)", 0.0),
        ["A"]
    );
    assert_eq!(wheel(&mut st.s.rbx, -1.0, 8), Some(None));
    assert_eq!(wheel(&mut st.s.rbx, -1.0, 8), Some(Some(0)));
    game.player.as_mut().unwrap().slot = 0;
    run_for(&mut st, &mut game, 0.1);
    assert_eq!(
        lua(&mut st, &mut game, "print(ch:FindFirstChildOfClass('Tool').Name)", 0.0),
        ["Diamond Pickaxe"]
    );
    // Slot 1, up: the last tool.
    assert_eq!(wheel(&mut st.s.rbx, 1.0, 0), Some(None));
    run_for(&mut st, &mut game, 0.1);
    assert_eq!(
        lua(&mut st, &mut game, "print(ch:FindFirstChildOfClass('Tool').Name)", 0.0),
        ["B"]
    );
}

/// BTools: the Hammer, the Clone and the Grab on the Workspace's parts, the
/// world's blocks and the entities' mirrors.
fn btools(name: &str) -> (FakeGame, Studio) {
    let (mut game, mut st) = stocked(name);
    // A stone floor under a part, a block ahead at eye height.
    for x in 0..20 {
        for z in -20..10 {
            game.blocks.insert((x, 63, z), "minecraft:stone".into());
        }
    }
    lua(
        &mut st,
        &mut game,
        "for _, b in ipairs({'Hammer', 'Clone', 'Grab', 'GameTool'}) do
           local h = Instance.new('HopperBin') h.Name = b h.BinType = Enum.BinType[b] h.Parent = bp
         end
         p = Instance.new('Part') p.Name = 'Target' p.Anchored = true p.Size = Vector3.new(2, 2, 2)
         p.CFrame = CFrame.new(10.5, 65.6, -8) p.Parent = workspace",
        0.1,
    );
    (game, st)
}

fn hold(st: &mut Studio, game: &mut FakeGame, bin: &str) {
    lua(
        st,
        game,
        &format!("ch.Humanoid:EquipTool(bp:FindFirstChild('{bin}'))"),
        0.1,
    );
}

fn click(st: &mut Studio, game: &mut FakeGame) {
    st.key("key.mouse.left", true);
    run_for(st, game, 0.1);
    st.key("key.mouse.left", false);
    run_for(st, game, 0.1);
}

#[test]
fn the_hammer_deletes_the_part_clicked_and_breaks_a_block_client_side() {
    let (mut game, mut st) = btools("tools-hammer");
    hold(&mut st, &mut game, "Hammer");
    let out = lua(
        &mut st,
        &mut game,
        "print(bp:FindFirstChild('Hammer'), ch.Hammer.Active)",
        0.0,
    );
    assert_eq!(out, ["nil    true"]);
    click(&mut st, &mut game);
    let out = lua(&mut st, &mut game, "print(p.Parent)", 0.0);
    assert_eq!(out, ["nil"]);
    // Looking down at the floor: the block under the crosshair breaks.
    game.camera.as_mut().unwrap().pitch = 90.0;
    click(&mut st, &mut game);
    assert!(
        game.moves.contains(&"block 10 63 -4 minecraft:air".to_owned()),
        "{:?}",
        game.moves
    );
}

#[test]
fn the_hammer_takes_an_entity_out_of_the_clients_view() {
    let (mut game, mut st) = btools("tools-hammer-mob");
    lua(&mut st, &mut game, "p:Destroy()", 0.0);
    game.entities = vec![EntityInfo {
        id: 77,
        kind: "minecraft:zombie".into(),
        pos: [10.5, 64.0, -8.0],
        width: 0.6,
        height: 1.95,
        eye: 1.74,
        health: 20.0,
        max_health: 20.0,
        on_ground: true,
        ..EntityInfo::default()
    }];
    hold(&mut st, &mut game, "Hammer");
    click(&mut st, &mut game);
    assert!(game.moves.contains(&"hide 77".to_owned()), "{:?}", game.moves);
}

#[test]
fn the_clone_copies_a_part_on_top_and_a_block_onto_the_face_clicked() {
    let (mut game, mut st) = btools("tools-clone");
    hold(&mut st, &mut game, "Clone");
    click(&mut st, &mut game);
    let out = lua(
        &mut st,
        &mut game,
        "local n, top = 0, nil
         for _, c in ipairs(workspace:GetChildren()) do if c.Name == 'Target' then n += 1 top = c end end
         print(n, top.Position.Y)",
        0.0,
    );
    assert_eq!(out, ["2    67.6"]);
    game.camera.as_mut().unwrap().pitch = 90.0;
    lua(
        &mut st,
        &mut game,
        "for _, c in ipairs(workspace:GetChildren()) do if c.Name == 'Target' then c:Destroy() end end",
        0.0,
    );
    click(&mut st, &mut game);
    assert!(
        game.moves.contains(&"block 10 64 -4 minecraft:stone".to_owned()),
        "{:?}",
        game.moves
    );
}

#[test]
fn the_game_tool_drags_as_the_grab() {
    let (mut game, mut st) = btools("tools-gametool");
    hold(&mut st, &mut game, "GameTool");
    st.key("key.mouse.left", true);
    run_for(&mut st, &mut game, 0.1);
    game.camera.as_mut().unwrap().pitch = 60.0;
    run_for(&mut st, &mut game, 0.1);
    st.key("key.mouse.left", false);
    run_for(&mut st, &mut game, 0.1);
    let out = lua(
        &mut st,
        &mut game,
        "print(math.floor(p.Position.Y * 10 + 0.5) / 10)",
        0.0,
    );
    assert_eq!(out, ["65"]);
}

#[test]
fn the_grab_drags_a_part_and_moves_a_block() {
    let (mut game, mut st) = btools("tools-grab");
    hold(&mut st, &mut game, "Grab");
    st.key("key.mouse.left", true);
    run_for(&mut st, &mut game, 0.1);
    // The view goes down to the floor: the part follows, on it.
    game.camera.as_mut().unwrap().pitch = 60.0;
    run_for(&mut st, &mut game, 0.1);
    st.key("key.mouse.left", false);
    run_for(&mut st, &mut game, 0.1);
    let out = lua(
        &mut st,
        &mut game,
        "print(math.floor(p.Position.Y * 10 + 0.5) / 10)",
        0.0,
    );
    assert_eq!(out, ["65"]);
    // A block: lifted at the floor under the crosshair, put on the floor ahead.
    lua(&mut st, &mut game, "p:Destroy()", 0.0);
    game.camera.as_mut().unwrap().pitch = 90.0;
    st.key("key.mouse.left", true);
    run_for(&mut st, &mut game, 0.1);
    game.camera.as_mut().unwrap().pitch = 45.0;
    st.key("key.mouse.left", false);
    run_for(&mut st, &mut game, 0.1);
    assert!(
        game.moves.contains(&"block 10 63 -4 minecraft:air".to_owned()),
        "{:?}",
        game.moves
    );
    let put = game.moves.iter().rev().find(|m| m.starts_with("block ")).unwrap();
    assert!(put.ends_with("minecraft:stone") && put.contains(" 64 "), "{put}");
}

/// A part 2 studs wide whose centre is at `at`, and the console's `p`.
fn part_at(st: &mut Studio, game: &mut FakeGame, at: [f64; 3]) {
    lua(
        st,
        game,
        &format!(
            "p = Instance.new('Part') p.Anchored = true p.Size = Vector3.new(2, 2, 2)
             p.CFrame = CFrame.new({}, {}, {}) p.Parent = workspace",
            at[0], at[1], at[2]
        ),
        0.1,
    );
}

#[test]
fn handles_are_drawn_and_dragged_along_a_face() {
    let (mut game, mut st) = stocked("tools-handles");
    // The Top knob (a stud above the top face) is on the crosshair.
    part_at(&mut st, &mut game, [10.5, 63.62, -8.0]);
    lua(
        &mut st,
        &mut game,
        "h = Instance.new('Handles') h.Adornee = p h.Color = BrickColor.new('Deep orange') h.Parent = lp.PlayerGui
         h.MouseButton1Down:Connect(function(f) print('down', f) end)
         h.MouseDrag:Connect(function(f, d) print('drag', f, math.floor(d * 100 + 0.5) / 100) end)
         h.MouseButton1Up:Connect(function(f) print('up', f) end)",
        0.1,
    );
    game.drawn.clear();
    st.hud(&mut game);
    let knobs = game
        .drawn
        .iter()
        .filter(|d| d.starts_with("rect ") && d.contains(" r5 "))
        .count();
    assert_eq!(knobs, 6, "{:?}", game.drawn);
    st.key("key.mouse.left", true);
    run_for(&mut st, &mut game, 0.1);
    // The view goes up 10 degrees: the ray meets the knob's line 0.84 higher.
    game.camera.as_mut().unwrap().pitch = -10.0;
    run_for(&mut st, &mut game, 0.1);
    st.key("key.mouse.left", false);
    run_for(&mut st, &mut game, 0.1);
    let all = log(&st);
    let fired: Vec<&String> = all.iter().filter(|l| l.contains("Enum.NormalId")).collect();
    assert_eq!(
        fired,
        [
            "down    Enum.NormalId.Top",
            "drag    Enum.NormalId.Top    0.84",
            "up    Enum.NormalId.Top"
        ]
    );
}

#[test]
fn arc_handles_turn_about_an_axis() {
    let (mut game, mut st) = stocked("tools-arcs");
    // The Z ring's upper knob (its radius above the centre) is on the crosshair.
    let r = 3f64.sqrt() + 0.5;
    part_at(&mut st, &mut game, [10.5, 65.62 - r, -8.0]);
    lua(
        &mut st,
        &mut game,
        "h = Instance.new('ArcHandles') h.Adornee = p h.Parent = lp.PlayerGui
         h.MouseButton1Down:Connect(function(a) print('down', a) end)
         h.MouseDrag:Connect(function(a, angle, dr)
           print('drag', a, math.floor(angle * 1000 + 0.5) / 1000, math.floor(dr * 1000 + 0.5) / 1000)
         end)",
        0.1,
    );
    st.key("key.mouse.left", true);
    run_for(&mut st, &mut game, 0.1);
    game.camera.as_mut().unwrap().yaw = 185.0;
    run_for(&mut st, &mut game, 0.1);
    st.key("key.mouse.left", false);
    run_for(&mut st, &mut game, 0.1);
    let all = log(&st);
    let fired: Vec<&String> = all.iter().filter(|l| l.contains("Enum.Axis")).collect();
    assert_eq!(fired, ["down    Enum.Axis.Z", "drag    Enum.Axis.Z    -0.184    0.038"]);
}

#[test]
fn a_dragger_puts_the_parts_on_what_the_ray_meets() {
    let (mut game, mut st) = stocked("tools-dragger");
    for x in 0..20 {
        for z in -20..10 {
            game.blocks.insert((x, 63, z), "minecraft:stone".into());
        }
    }
    part_at(&mut st, &mut game, [10.5, 70.0, -8.0]);
    let out = lua(
        &mut st,
        &mut game,
        "local q = p:Clone() q.CFrame = p.CFrame * CFrame.new(2, 0, 0) q.Parent = workspace
         local d = Instance.new('Dragger')
         d:MouseDown(p, Vector3.new(0, 0, 0), {p, q})
         d:MouseMove(Ray.new(Vector3.new(13.2, 80, -10.7), Vector3.new(0, -1, 0)))
         print(p.Position, q.Position)
         d:AxisRotate(Enum.Axis.Y)
         print(math.floor(q.Position.X + 0.5), math.floor(q.Position.Z + 0.5))
         d:MouseUp()",
        0.1,
    );
    assert_eq!(out, ["13.5, 65, -11    15.5, 65, -11", "14    -13"]);
}

#[test]
fn a_click_on_a_part_fires_its_click_detector_within_reach() {
    let (mut game, mut st) = stocked("parts-click");
    part_at(&mut st, &mut game, [10.5, 65.62, -8.0]);
    lua(
        &mut st,
        &mut game,
        "local cd = Instance.new('ClickDetector', p)
         cd.MouseClick:Connect(function(who) print('click', who.Name) end)
         cd.RightMouseClick:Connect(function(who) print('right', who.Name) end)",
        0.1,
    );
    click(&mut st, &mut game);
    st.key("key.mouse.right", true);
    st.key("key.mouse.right", false);
    run_for(&mut st, &mut game, 0.1);
    let all = log(&st);
    assert!(all.contains(&"click    Player1".to_owned()), "{all:?}");
    assert!(all.contains(&"right    Player1".to_owned()), "{all:?}");
    // Out of its reach, nothing.
    lua(&mut st, &mut game, "p.ClickDetector.MaxActivationDistance = 2", 0.1);
    let before = log(&st).iter().filter(|l| l.starts_with("click")).count();
    click(&mut st, &mut game);
    assert_eq!(log(&st).iter().filter(|l| l.starts_with("click")).count(), before);
}

#[test]
fn a_proximity_prompt_in_reach_is_shown_and_its_key_triggers_it() {
    let (mut game, mut st) = stocked("parts-prompt");
    lua(
        &mut st,
        &mut game,
        "pps = game:GetService('ProximityPromptService')",
        0.0,
    );
    part_at(&mut st, &mut game, [12.0, 64.0, -6.0]);
    lua(
        &mut st,
        &mut game,
        "local pp = Instance.new('ProximityPrompt') pp.ActionText = 'Open' pp.ObjectText = 'Door' pp.HoldDuration = 0.3 pp.Parent = p
         pp.PromptShown:Connect(function() print('shown') end)
         pp.PromptButtonHoldBegan:Connect(function(who) print('hold', who.Name) end)
         pp.Triggered:Connect(function(who) print('triggered', who.Name) end)
         pps.PromptTriggered:Connect(function(prompt, who) print('service', prompt.ActionText) end)
         pp.PromptHidden:Connect(function() print('hidden') end)",
        0.5,
    );
    game.drawn.clear();
    st.hud(&mut game);
    assert!(
        game.drawn.iter().any(|d| d.starts_with("text [E] Open Door ")),
        "{:?}",
        game.drawn
    );
    assert_eq!(
        horizon_studio::game::hstudio_rbx_parts::prompt_key(&st.s.rbx).as_deref(),
        Some("key.keyboard.e")
    );
    st.key("key.keyboard.e", true);
    run_for(&mut st, &mut game, 0.5);
    st.key("key.keyboard.e", false);
    run_for(&mut st, &mut game, 0.1);
    // Walking away hides it.
    game.player.as_mut().unwrap().z = 30.0;
    run_for(&mut st, &mut game, 0.5);
    let all = log(&st);
    let seen: Vec<&String> = all
        .iter()
        .filter(|l| {
            ["shown", "hold", "triggered", "service", "hidden"]
                .iter()
                .any(|w| l.starts_with(w))
        })
        .collect();
    assert_eq!(
        seen,
        [
            "shown",
            "hold    Player1",
            "triggered    Player1",
            "service    Open",
            "hidden"
        ]
    );
}

#[test]
fn the_characters_touches_reach_a_parts_touched_and_its_touch_interest() {
    let (mut game, mut st) = stocked("parts-touch");
    part_at(&mut st, &mut game, [10.5, 64.5, -3.25]);
    let out = lua(
        &mut st,
        &mut game,
        "p.Touched:Connect(function(hit) print('touched', hit.Name) end)
         p.TouchEnded:Connect(function(hit) print('ended', hit.Name) end)
         print(p.TouchInterest.ClassName)",
        0.3,
    );
    assert_eq!(out[0], "TouchTransmitter");
    game.player.as_mut().unwrap().x = 20.0;
    run_for(&mut st, &mut game, 0.2);
    let all = log(&st);
    assert!(all.contains(&"touched    HumanoidRootPart".to_owned()), "{all:?}");
    assert!(all.contains(&"ended    HumanoidRootPart".to_owned()), "{all:?}");
}

#[test]
fn body_movers_move_a_scripts_unanchored_parts() {
    let (mut game, mut st) = stocked("parts-movers");
    part_at(&mut st, &mut game, [0.0, 70.0, 0.0]);
    let out = lua(
        &mut st,
        &mut game,
        "p.Anchored = false
         local bp = Instance.new('BodyPosition') bp.MaxForce = Vector3.new(math.huge, math.huge, math.huge)
         bp.Position = Vector3.new(10, 70, 0) bp.Parent = p
         task.wait(1.5)
         print(math.floor(p.Position.X + 0.5))
         bp:Destroy()
         local anchored = Instance.new('Part') anchored.Anchored = true anchored.Parent = workspace
         local bv = Instance.new('BodyVelocity') bv.Velocity = Vector3.new(0, 10, 0) bv.Parent = anchored
         task.wait(0.5)
         print(anchored.Position.Y)",
        2.5,
    );
    assert_eq!(out, ["10", "0"]);
}

#[test]
fn a_models_box_and_a_mirrors_destroy_and_another_players_backpack() {
    let (mut game, mut st) = stocked("parts-model");
    game.entities = vec![EntityInfo {
        id: 77,
        kind: "minecraft:zombie".into(),
        pos: [13.0, 64.0, -12.0],
        width: 0.6,
        height: 1.95,
        eye: 1.74,
        health: 20.0,
        max_health: 20.0,
        on_ground: true,
        ..EntityInfo::default()
    }];
    game.players = Some(vec![
        horizon_studio::game::hstudio::PlayerEntry {
            uuid: 1,
            name: "Steve".into(),
            local: true,
            ..Default::default()
        },
        horizon_studio::game::hstudio::PlayerEntry {
            uuid: 2,
            name: "Alex".into(),
            ..Default::default()
        },
    ]);
    let out = lua(
        &mut st,
        &mut game,
        "local m = Instance.new('Model', workspace)
         local a = Instance.new('Part', m) a.Size = Vector3.new(2, 2, 2) a.CFrame = CFrame.new(0, 0, 0)
         local b = Instance.new('Part', m) b.Size = Vector3.new(2, 2, 2) b.CFrame = CFrame.new(4, 0, 0)
         local cf, size = m:GetBoundingBox()
         print(cf.Position, size, m:GetModelCFrame().Position, m:GetExtentsSize())
         print(#game.Players.Alex.Backpack:GetChildren())
         workspace.Zombie.Head:Destroy()",
        0.3,
    );
    assert_eq!(out, ["2, 0, 0    6, 2, 2    2, 0, 0    6, 2, 2", "0"]);
    assert!(acts(&game).contains(&"hide 77".to_owned()), "{:?}", game.moves);
    let out = lua(&mut st, &mut game, "print(workspace:FindFirstChild('Zombie'))", 0.1);
    assert_eq!(out, ["nil"]);
}
