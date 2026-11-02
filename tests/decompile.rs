// decompile as far as it goes here: a script's source, a function's own
// text (from the source every function compiled here keeps), the source
// getscriptbytecode's bytes carry, and a plain answer for what has no code.

mod common;

use common::{FakeGame, log, mod_index, run_for, studio};
use horizon_studio::game::hstudio::ModState;

fn run(name: &str, src: &str) -> Vec<String> {
    let mut game = FakeGame::new(name);
    let mut st = studio(&mut game);
    game.write_mod("t.lua", src);
    run_for(&mut st, &mut game, 0.5);
    let i = mod_index(&st, "t");
    if let ModState::Failed(m, t) = &st.s.mods[i].state {
        panic!("{m}\n{t}\n{:?}", log(&st));
    }
    log(&st)
        .iter()
        .filter_map(|l| l.strip_prefix("[t] ").map(str::to_owned))
        .collect()
}

#[test]
fn a_function_decompiles_to_its_own_text() {
    let p = run(
        "decompile-fn",
        r##"
        local function add(a, b)
            return a + b
        end
        local t = {}
        function t.greet(name) return "hi " .. name end
        local anon = function(...) return select("#", ...) end
        print(decompile(add) == "local function add(a, b)\n            return a + b\n        end")
        print(decompile(t.greet))
        print(decompile(anon))
        print(decompile(newcclosure(add)) == decompile(add))
        print(decompile(print))
        "##,
    );
    assert_eq!(
        p,
        [
            "true",
            "function t.greet(name) return \"hi \" .. name end",
            "function(...) return select(\"#\", ...) end",
            "true",
            "-- print: a built-in function, with no Lua source",
        ],
        "{p:?}"
    );
}

#[test]
fn loadstring_code_and_getscriptbytecode_give_the_source_back() {
    let p = run(
        "decompile-load",
        r#"
        local f = loadstring("local x = 1\nreturn function(y) return x + y end")
        local inner = f()
        print(decompile(inner))
        print(decompile(f) == "local x = 1\nreturn function(y) return x + y end")
        local bc = getscriptbytecode(script)
        print(decompile(bc) == decompile(script))
        print(decompile("HSBC1\nproto main 0 0 true\n"):sub(1, 24))
        print(pcall(decompile, "not bytecode"))
        "#,
    );
    assert_eq!(p[0], "function(y) return x + y end", "{p:?}");
    assert_eq!(p[1], "true", "{p:?}");
    assert_eq!(p[2], "true", "{p:?}");
    assert_eq!(p[3], "-- compiled code without", "{p:?}");
    assert!(
        p[4].starts_with("false") && p[4].contains("bytecode from getscriptbytecode expected"),
        "{p:?}"
    );
}

#[test]
fn something_that_is_not_a_script_says_so() {
    let p = run(
        "decompile-part",
        r#"
        local part = Instance.new("Part")
        part.Name = "Thing"
        part.Parent = workspace
        print(decompile(part))
        print(decompile(script):find("Instance.new", 1, true) ~= nil)
        "#,
    );
    assert_eq!(
        p,
        ["-- Workspace.Thing is a Part, not a script: it has no code", "true"],
        "{p:?}"
    );
}
