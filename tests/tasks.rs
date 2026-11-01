// Luau's task scheduler on a fake game: task.wait, spawn, defer, delay and
// cancel, the older wait/spawn/delay, a mod's top level waiting, and a
// failing thread stopping its mod.

mod common;

use common::{FakeGame, log, mod_index, run_for, studio};
use horizon_studio::game::hstudio::ModState;

fn printed(st: &horizon_studio::game::hstudio::Studio, id: &str) -> Vec<String> {
    let prefix = format!("[{id}] ");
    log(st)
        .iter()
        .filter_map(|l| l.strip_prefix(&prefix).map(str::to_owned))
        .collect()
}

#[test]
fn a_mods_top_level_can_wait() {
    let mut game = FakeGame::new("task-top");
    let mut st = studio(&mut game);
    game.write_mod(
        "waiter.lua",
        "print('start')\nlocal waited = task.wait(0.5)\nprint('after', waited >= 0.5)\nprint(typeof(task.wait()))\n",
    );
    run_for(&mut st, &mut game, 0.6);
    assert_eq!(printed(&st, "waiter"), ["start"]);
    run_for(&mut st, &mut game, 0.6);
    assert_eq!(printed(&st, "waiter"), ["start", "after    true", "number"]);
    assert_eq!(st.s.mods[mod_index(&st, "waiter")].state, ModState::Running);
}

#[test]
fn spawn_defer_delay_and_cancel() {
    let mut game = FakeGame::new("task-spawn");
    let mut st = studio(&mut game);
    game.write_mod(
        "t.lua",
        r#"
        local order = {}
        local function note(x) table.insert(order, x) end
        task.spawn(function(a) note("spawn " .. a); task.wait(0.2); note("spawn woke") end, "now")
        task.defer(function(a) note("defer " .. a) end, "later")
        task.delay(0.3, function(a) note("delay " .. a) end, "x")
        local victim = task.delay(0.3, function() note("cancelled ran!") end)
        task.cancel(victim)
        local co = coroutine.create(function() note("thread"); task.wait(0.1); note("thread woke") end)
        task.spawn(co)
        note("main")
        command("order", function() return table.concat(order, ", ") end)
        "#,
    );
    run_for(&mut st, &mut game, 1.2);
    assert!(st.command("order"));
    st.frame(&mut game, 0.016);
    let said = game.chats.last().cloned().unwrap_or_default();
    assert_eq!(
        said,
        "spawn now, thread, main, defer later, thread woke, spawn woke, delay x",
        "{:?}",
        log(&st)
    );
}

#[test]
fn the_older_globals() {
    let mut game = FakeGame::new("task-legacy");
    let mut st = studio(&mut game);
    game.write_mod(
        "old.lua",
        r#"
        spawn(function(dt, t) print("spawned", dt, type(t)) end)
        delay(0.2, function(waited) print("delayed", waited >= 0.2) end)
        local a, b = wait(0.1)
        print("waited", a >= 0.1, type(b))
        print(type(tick()), tick() > 1e9, type(time()))
        warn("careful", 1)
        "#,
    );
    run_for(&mut st, &mut game, 1.2);
    let p = printed(&st, "old");
    assert!(p.contains(&"spawned    0    number".to_owned()), "{p:?}");
    assert!(p.contains(&"delayed    true".to_owned()), "{p:?}");
    assert!(p.contains(&"waited    true    number".to_owned()), "{p:?}");
    assert!(p.contains(&"number    true    number".to_owned()), "{p:?}");
    assert!(
        log(&st).contains(&"[old] warning: careful 1".to_owned()),
        "{:?}",
        log(&st)
    );
}

#[test]
fn commands_can_wait_and_handlers_say_why_they_cannot() {
    let mut game = FakeGame::new("task-main");
    let mut st = studio(&mut game);
    game.write_mod(
        "h.lua",
        "command('slow', function() task.wait(0.1) chat.show('slow done') end)\non('tick', function() task.wait(1) end)\n",
    );
    run_for(&mut st, &mut game, 0.6);
    let i = mod_index(&st, "h");
    match &st.s.mods[i].state {
        ModState::Failed(msg, _) => assert!(msg.contains("can't wait here"), "{msg}"),
        other => panic!("{other:?}"),
    }
    game.write_mod(
        "h.lua",
        "command('slow', function() task.wait(0.1) chat.show('slow done') end)\n",
    );
    run_for(&mut st, &mut game, 0.6);
    assert!(
        st.command("slow"),
        "{:?} {:?}",
        log(&st),
        st.s.mods[mod_index(&st, "h")].state
    );
    run_for(&mut st, &mut game, 0.3);
    assert!(game.chats.contains(&"[h] slow done".to_owned()), "{:?}", game.chats);
}

#[test]
fn a_failing_thread_is_printed_and_its_mod_goes_on() {
    // As in Roblox: an error in a thread (task.delay, task.spawn, an event's
    // handler) is printed with its traceback, and the script goes on.
    let mut game = FakeGame::new("task-fail");
    let mut st = studio(&mut game);
    game.write_mod(
        "bad.lua",
        "task.delay(0.1, function() error('boom') end)\ntask.delay(0.3, function() print('still here') end)\nprint('loaded')\n",
    );
    run_for(&mut st, &mut game, 1.2);
    let i = mod_index(&st, "bad");
    assert_eq!(st.s.mods[i].state, ModState::Running);
    assert!(
        log(&st).iter().any(|l| l.starts_with("[bad] ") && l.contains("boom")),
        "{:?}",
        log(&st)
    );
    assert!(printed(&st, "bad").contains(&"still here".to_owned()), "{:?}", log(&st));
    // An error in its main chunk still stops it.
    game.write_mod("bad.lua", "error('main boom')\n");
    run_for(&mut st, &mut game, 1.2);
    match &st.s.mods[mod_index(&st, "bad")].state {
        ModState::Failed(msg, _) => assert!(msg.contains("main boom"), "{msg}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_console_can_wait() {
    let mut game = FakeGame::new("task-console");
    let mut st = studio(&mut game);
    st.run_console(
        &mut game,
        "task.spawn(function() task.wait(0.1) print('console woke') end)",
    );
    run_for(&mut st, &mut game, 0.3);
    assert!(log(&st).contains(&"console woke".to_owned()), "{:?}", log(&st));
}
