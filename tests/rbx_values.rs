// hstudio_rbx_values from scripts: Roblox's data types and Enum as a Luau
// script sees them (constructors, properties, methods, operators, typeof
// and tostring).

use horizon_studio::game::hstudio_lua::{Lua, NoHost};
use horizon_studio::game::hstudio_lualib::new_env;
use horizon_studio::game::hstudio_rbx_values::globals;

fn try_run(src: &str) -> Result<Vec<String>, String> {
    let mut lua = Lua::new();
    lua.steps = 5_000_000;
    let env = new_env(&mut lua);
    for (name, v) in globals() {
        env.borrow_mut().set_str(name, v);
    }
    let f = lua.load(src, "test.lua", &env)?;
    let mut host = NoHost::default();
    match lua.call(&mut host, &f, Vec::new()) {
        Ok(_) => Ok(host.printed),
        Err(e) => Err(e.message()),
    }
}

fn run(src: &str) -> Vec<String> {
    try_run(src).unwrap_or_else(|e| panic!("script failed: {e}\n--- script ---\n{src}"))
}

fn error_of(src: &str) -> String {
    try_run(src).expect_err("the script should fail")
}

#[test]
fn vectors() {
    let src = r#"
        local a = Vector3.new(1, 2, 3)
        local b = Vector3.new(4, 5, 6)
        print(a + b, a - b, a * 2, 2 * a, a * b, b / 2, -a, b // Vector3.new(4, 4, 4))
        print(a.X, a.y, Vector3.new(3, 4, 0).Magnitude, Vector3.new(0, 0, 5).Unit)
        print(a:Dot(b), a:Cross(b), a:Lerp(b, 0.5), a == Vector3.new(1, 2, 3), a ~= b)
        print(typeof(a), Vector3.zero, Vector3.one, Vector3.xAxis, Vector3.new())
        print(Vector3.FromNormalId(Enum.NormalId.Top), Vector3.new(-1.5, 2.5, 0):Floor())
        local v = Vector2.new(3, 4)
        print(v.Magnitude, v + Vector2.one, v:Dot(Vector2.new(1, 0)), typeof(v))
    "#;
    assert_eq!(
        run(src),
        [
            "5, 7, 9\t-3, -3, -3\t2, 4, 6\t2, 4, 6\t4, 10, 18\t2, 2.5, 3\t-1, -2, -3\t1, 1, 1",
            "1\t2\t5\t0, 0, 1",
            "32\t-3, 6, -3\t2.5, 3.5, 4.5\ttrue\ttrue",
            "Vector3\t0, 0, 0\t1, 1, 1\t1, 0, 0\t0, 0, 0",
            "0, 1, 0\t-2, 2, 0",
            "5\t4, 5\t3\tVector2",
        ]
    );
}

#[test]
fn cframes() {
    let src = r#"
        local c = CFrame.new(1, 2, 3)
        print(c.Position, c.X, c * Vector3.new(1, 1, 1), (c * CFrame.new(1, 0, 0)).Position)
        local r = CFrame.Angles(0, math.rad(90), 0)
        local p = r * Vector3.new(1, 0, 0)
        print(math.round(p.X), math.round(p.Z), typeof(r))
        print(c:Inverse().Position, (c + Vector3.new(1, 1, 1)).Position, c:ToObjectSpace(c).Position)
        local look = CFrame.lookAt(Vector3.zero, Vector3.new(0, 0, -10))
        print(look.LookVector, CFrame.new(Vector3.new(5, 0, 0)).X)
    "#;
    assert_eq!(
        run(src),
        [
            "1, 2, 3\t1\t2, 3, 4\t2, 2, 3",
            "0\t-1\tCFrame",
            "-1, -2, -3\t2, 3, 4\t0, 0, 0",
            "0, 0, -1\t5",
        ]
    );
}

#[test]
fn colors_udims_and_more() {
    let src = r#"
        local c = Color3.fromRGB(255, 128, 0)
        print(c.R, math.floor(c.G * 255 + 0.5), c:ToHex(), Color3.new(1, 0, 0):Lerp(Color3.new(0, 0, 1), 0.5))
        print(UDim2.new(0.5, 10, 1, -20), UDim2.fromScale(1, 1), UDim2.fromOffset(100, 50).X.Offset)
        print(UDim.new(0, 8) + UDim.new(0.5, 2), UDim2.new(0, 0, 0, 0):Lerp(UDim2.new(1, 100, 1, 100), 0.5))
        local t = TweenInfo.new(2, Enum.EasingStyle.Sine, Enum.EasingDirection.InOut, 1, true, 0.5)
        print(t.Time, t.EasingStyle, t.EasingDirection, t.RepeatCount, t.Reverses, t.DelayTime)
        print(BrickColor.new("Bright red").Name, BrickColor.Red().Number, typeof(BrickColor.random()))
        print(NumberRange.new(1, 5).Max, NumberSequence.new(0, 1).Keypoints[2].Value, typeof(ColorSequence.new(c)))
        local rng = Random.new(42)
        local n = rng:NextInteger(1, 10)
        print(n >= 1 and n <= 10, Random.new(42):NextInteger(1, 10) == n, rng:NextNumber() < 1)
        print(Font.fromEnum(Enum.Font.GothamBold).Weight, Ray.new(Vector3.zero, Vector3.new(0, 0, 10)):Distance(Vector3.new(3, 0, 5)))
    "#;
    assert_eq!(
        run(src),
        [
            "1\t128\tff8000\t0.5, 0, 0.5",
            "{0.5, 10}, {1, -20}\t{1, 0}, {1, 0}\t100",
            "0.5, 10\t{0.5, 50}, {0.5, 50}",
            "2\tEnum.EasingStyle.Sine\tEnum.EasingDirection.InOut\t1\ttrue\t0.5",
            "Bright red\t21\tBrickColor",
            "5\t1\tColorSequence",
            "true\ttrue\ttrue",
            "Enum.FontWeight.Bold\t3",
        ]
    );
}

#[test]
fn enums() {
    let src = r#"
        local e = Enum.KeyCode.E
        print(e, e.Name, e.Value, e.EnumType, typeof(e), e == Enum.KeyCode.E, e == Enum.KeyCode.Q)
        print(#Enum.EasingDirection:GetEnumItems(), Enum.Material.Neon.Value, e:IsA("KeyCode"))
        print(Enum.KeyCode:FromName("Space"), Enum.UserInputType:FromValue(8), typeof(Enum), typeof(Enum.KeyCode))
        local seen = {}
        seen[Enum.KeyCode.A] = true
        print(seen[Enum.KeyCode.A])
    "#;
    assert_eq!(
        run(src),
        [
            "Enum.KeyCode.E\tE\t101\tKeyCode\tEnumItem\ttrue\tfalse",
            "3\t288\ttrue",
            "Enum.KeyCode.Space\tEnum.UserInputType.Keyboard\tEnums\tEnum",
            "true",
        ]
    );
    assert!(error_of("print(Enum.KeyCode.Nope)").contains("Nope is not a valid member of \"Enum.KeyCode\""));
    assert!(error_of("print(Enum.Nope)").contains("Nope is not a valid EnumItem"));
}

#[test]
fn values_cannot_change_and_errors_name_the_types() {
    assert!(error_of("local v = Vector3.new() v.X = 1").contains("X cannot be assigned to"));
    assert!(error_of("print(Vector3.new().Nope)").contains("Nope is not a valid member of Vector3"));
    assert!(error_of("print(Vector3.new() + 1)").contains("arithmetic (add) on Vector3 and number"));
    assert!(error_of("print(CFrame.lookAt(1))").contains("Vector3 expected, got number"));
    assert!(error_of("Vector3.new = nil").contains("readonly"));
}

#[test]
fn region3s() {
    let src = r#"
        local r = Region3.new(Vector3.new(4, 64, 3), Vector3.new(0, 60, 0))
        print(r.CFrame.Position, r.Size, typeof(r), r == Region3.new(Vector3.new(0, 60, 0), Vector3.new(4, 64, 3)))
        local g = r:ExpandToGrid(4)
        print(g.Size, g.CFrame.Position, Region3.new(Vector3.zero, Vector3.one * 2))
    "#;
    assert_eq!(
        run(src),
        [
            "2, 62, 1.5\t4, 4, 3\tRegion3\ttrue",
            "4, 4, 4\t2, 62, 2\t1, 1, 1, 1, 0, 0, 0, 1, 0, 0, 0, 1; 2, 2, 2",
        ]
    );
    assert!(error_of("local r = Region3.new() r.Size = Vector3.one").contains("Size cannot be assigned to"));
    assert!(error_of("Region3.new():ExpandToGrid(0)").contains("a resolution above 0"));
}
