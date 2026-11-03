// A mod run again (saved in the editor) keeps what other scripts still hold:
// its old instances are destroyed, as in Roblox, not gone.

mod common;
use common::{FakeGame, log, run_for, studio};

#[test]
fn an_instance_kept_in_getgenv_survives_its_mod_running_again() {
    // Hydroxide keeps its UI in getgenv().oh, and when run again its Exit
    // reads and destroys the old one: that must not be an unknown object.
    let mut game = FakeGame::new("mod-reload-kept");
    let mut st = studio(&mut game);
    game.write_mod(
        "a.lua",
        r#"
        local f = Instance.new("ScreenGui")
        f.Name = "Old"
        getgenv().kept = f
        print("made")
        "#,
    );
    run_for(&mut st, &mut game, 2.0);
    game.write_mod(
        "a.lua",
        r#"
        local f = getgenv().kept
        print("again", typeof(f), pcall(function() return f.ClassName, f.Name, f.Parent end))
        print("destroy", pcall(function() f:Destroy() end))
        "#,
    );
    run_for(&mut st, &mut game, 1.0);
    let p: Vec<String> = log(&st)
        .iter()
        .filter_map(|l| l.strip_prefix("[a] ").map(|s| s.replace("    ", "\t")))
        .collect();
    assert_eq!(
        p,
        ["made", "again\tInstance\ttrue\tScreenGui\tOld\tnil", "destroy\ttrue"],
        "{:?}",
        log(&st)
    );
}
