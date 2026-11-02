// The Roblox layer's methods (hstudio_rbx_methods): Instance's
// (FindFirstChild, WaitForChild, Clone, Destroy, attributes and tags...)
// and the services' (game:GetService, TweenService:Create,
// UserInputService:IsKeyDown, workspace:Raycast, Camera:WorldToViewportPoint,
// HttpService:JSONEncode...), for hstudio_rbx's dispatch.

use super::hstudio::json_to_lua;
use super::hstudio_lua::{Host, Lua, Res, Table, Throw, Value, bad_arg, check_num, check_text, opt_num};
use super::hstudio_rbx::{
    Env, Id, Rbx, Tween, Wait, Waiter, class_def, is_a, legacy_info, lerp_value, tween_goal, tween_info_arg, udim2_arg,
};
use super::hstudio_rbx_types::{CFrame, Color3, EasingDirection, EasingStyle, TweenInfo, Vector2, Vector3, ease};
use super::hstudio_rbx_values::{
    Data, as_cf, as_item, as_v2, as_v3, cf, coerce_enum, enum_item, enum_value, get, v2, v3, wrap,
};

fn one(v: Value) -> Res {
    Ok(vec![v])
}

fn none() -> Res {
    Ok(Vec::new())
}

fn me(env: &mut dyn Env, lua: &Lua, args: &[Value], name: &str) -> Result<Id, Throw> {
    match args.first().and_then(|v| env.rbx().id_of(v)) {
        Some(id) => Ok(id),
        None => Err(lua.rt_error(format!("Expected ':' not '.' calling member function {name}"))),
    }
}

fn text_arg(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<String, Throw> {
    check_text(lua, args, i, fname)
}

fn bool_arg(args: &[Value], i: usize) -> bool {
    args.get(i).is_some_and(Value::truthy)
}

/// A method of an instance (`owner_class` says which class has it).
pub fn method(env: &mut dyn Env, lua: &mut Lua, name: &str, owner_class: &str, args: Vec<Value>) -> Res {
    let id = me(env, lua, &args, name)?;
    super::hstudio_rbx_world::before_call(env, id, name, &args);
    // CollectionService:AddTag(instance, tag) is the instance's own.
    if matches!(name, "AddTag" | "RemoveTag" | "HasTag" | "GetTags")
        && env.rbx().service("CollectionService") == Some(id)
        && let Some(target) = args.get(1).and_then(|v| env.rbx().id_of(v))
    {
        let rest: Vec<Value> = std::iter::once(env.rbx().value(target))
            .chain(args[2..].iter().cloned())
            .collect();
        return instance_method(env, lua, target, name, &rest);
    }
    match owner_class {
        "Instance" => instance_method(env, lua, id, name, &args),
        _ => service_method(env, lua, id, name, owner_class, &args),
    }
}

fn find_child(rbx: &Rbx, id: Id, f: &dyn Fn(Id) -> bool, recursive: bool) -> Option<Id> {
    if recursive {
        rbx.descendants(id).into_iter().find(|&d| f(d))
    } else {
        rbx.children(id).iter().copied().find(|&c| f(c))
    }
}

fn find_ancestor(rbx: &Rbx, id: Id, f: &dyn Fn(Id) -> bool) -> Option<Id> {
    let mut at = rbx.inst(id).parent;
    while let Some(p) = at {
        if f(p) {
            return Some(p);
        }
        at = rbx.inst(p).parent;
    }
    None
}

fn opt_value(rbx: &Rbx, id: Option<Id>) -> Value {
    id.map_or(Value::Nil, |i| rbx.value(i))
}

/// A value an attribute can hold.
fn attribute_ok(v: &Value) -> bool {
    match v {
        Value::Nil | Value::Bool(_) | Value::Num(_) | Value::Str(_) => true,
        other => matches!(
            get(other),
            Some(
                Data::V2(_)
                    | Data::V3(_)
                    | Data::CF(_)
                    | Data::C3(_)
                    | Data::UD(_)
                    | Data::UD2(_)
                    | Data::Brick(_)
                    | Data::Rect(_)
                    | Data::Range(_)
                    | Data::NumSeq(_)
                    | Data::ColorSeq(_)
                    | Data::Font(_)
                    | Data::Item(_)
            )
        ),
    }
}

fn instance_method(env: &mut dyn Env, lua: &mut Lua, id: Id, name: &str, args: &[Value]) -> Res {
    let owner = env.owner();
    let now = env.now();
    let rbx = env.rbx();
    match name {
        "FindFirstChild" => {
            let n = text_arg(lua, args, 1, name)?;
            let found = find_child(rbx, id, &|c| rbx.inst(c).name == n, bool_arg(args, 2));
            one(opt_value(rbx, found))
        }
        "FindFirstChildOfClass" | "FindFirstChildWhichIsA" => {
            let class = text_arg(lua, args, 1, name)?;
            let exact = name == "FindFirstChildOfClass";
            let found = find_child(
                rbx,
                id,
                &|c| {
                    let k = rbx.inst(c).class;
                    if exact { k.name == class } else { is_a(k, &class) }
                },
                bool_arg(args, 2),
            );
            one(opt_value(rbx, found))
        }
        "FindFirstDescendant" => {
            let n = text_arg(lua, args, 1, name)?;
            let found = find_child(rbx, id, &|c| rbx.inst(c).name == n, true);
            one(opt_value(rbx, found))
        }
        "FindFirstAncestor" => {
            let n = text_arg(lua, args, 1, name)?;
            let found = find_ancestor(rbx, id, &|c| rbx.inst(c).name == n);
            one(opt_value(rbx, found))
        }
        "FindFirstAncestorOfClass" | "FindFirstAncestorWhichIsA" => {
            let class = text_arg(lua, args, 1, name)?;
            let exact = name == "FindFirstAncestorOfClass";
            let found = find_ancestor(rbx, id, &|c| {
                let k = rbx.inst(c).class;
                if exact { k.name == class } else { is_a(k, &class) }
            });
            one(opt_value(rbx, found))
        }
        "GetChildren" => {
            let kids = rbx.children(id).to_vec();
            one(rbx.list(&kids))
        }
        "GetDescendants" => {
            let all = rbx.descendants(id);
            one(rbx.list(&all))
        }
        "IsA" => {
            let class = text_arg(lua, args, 1, name)?;
            one(Value::Bool(is_a(rbx.inst(id).class, &class)))
        }
        "IsDescendantOf" | "IsAncestorOf" => {
            let other = match args.get(1).and_then(|v| rbx.id_of(v)) {
                Some(o) => o,
                None if matches!(args.get(1), None | Some(Value::Nil)) => return one(Value::Bool(false)),
                None => return Err(bad_arg(lua, 1, name, "Instance expected")),
            };
            let r = if name == "IsDescendantOf" {
                rbx.is_descendant_of(id, other)
            } else {
                rbx.is_descendant_of(other, id)
            };
            one(Value::Bool(r))
        }
        "GetFullName" => one(Value::str(&rbx.full_name(id))),
        "Destroy" | "Remove" => {
            // Locked (a service, the mouse, settings()): never, parent or not.
            let parentless_ok = rbx.inst(id).parent.is_none() && !rbx.unparented_locked(id);
            if rbx.inst(id).locked && !parentless_ok && !rbx.inst(id).destroyed {
                return Err(lua.rt_error(format!("The Parent property of {} is locked", rbx.inst(id).name)));
            }
            if name == "Remove" {
                rbx.set_parent(id, None).map_err(|e| lua.rt_error(e))?;
            } else {
                rbx.destroy(id);
            }
            none()
        }
        "ClearAllChildren" => {
            for c in rbx.children(id).to_vec() {
                if !rbx.inst(c).locked {
                    rbx.destroy(c);
                }
            }
            none()
        }
        "Clone" => {
            if !rbx.raw_get(id, "Archivable").truthy() {
                return one(Value::Nil);
            }
            if rbx.inst(id).class.service || id == rbx.game || rbx.inst(id).class.name == "Player" {
                return Err(lua.rt_error(format!("{} cannot be cloned", rbx.inst(id).class.name)));
            }
            let copy = rbx.clone_tree(id, owner, true);
            one(rbx.value(copy))
        }
        "WaitForChild" => {
            let n = text_arg(lua, args, 1, name)?;
            if let Some(c) = rbx.child_named(id, &n) {
                return one(rbx.value(c));
            }
            let timeout = match args.get(2) {
                None | Some(Value::Nil) => None,
                Some(_) => Some(check_num(lua, args, 2, name)?),
            };
            if !lua.is_yieldable() {
                return Err(lua.rt_error(
                    "WaitForChild can't wait here: this code isn't in a thread that can wait (run it with task.spawn)",
                ));
            }
            rbx.waiters.push(Waiter {
                owner,
                thread: lua.current_thread(),
                what: Wait::Child {
                    parent: id,
                    name: n,
                    until: timeout.map(|t| now + t.max(0.0)),
                    since: now,
                    warned: false,
                },
            });
            Err(Throw::Yield(Vec::new()))
        }
        "GetPropertyChangedSignal" => {
            let prop = text_arg(lua, args, 1, name)?;
            one(rbx.signal(id, &format!("prop:{prop}")))
        }
        "GetAttribute" => {
            let n = text_arg(lua, args, 1, name)?;
            let v = rbx
                .inst(id)
                .attributes
                .iter()
                .find(|(k, _)| *k == n)
                .map(|(_, v)| v.clone());
            one(v.unwrap_or_default())
        }
        "SetAttribute" => {
            let n = text_arg(lua, args, 1, name)?;
            let v = args.get(2).cloned().unwrap_or_default();
            if !attribute_ok(&v) {
                return Err(lua.rt_error(format!("{} is not a supported attribute type", lua.type_of(&v))));
            }
            if n.is_empty() || n.len() > 100 || !n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return Err(bad_arg(
                    lua,
                    1,
                    name,
                    "attribute names are letters, digits and '_', up to 100",
                ));
            }
            let attrs = &mut rbx.inst_mut(id).attributes;
            let old = attrs.iter().position(|(k, _)| *k == n);
            let changed = match (old, &v) {
                (Some(i), Value::Nil) => {
                    attrs.remove(i);
                    true
                }
                (Some(i), _) => {
                    let differs = !super::hstudio_rbx::same_value(&attrs[i].1, &v);
                    attrs[i].1 = v;
                    differs
                }
                (None, Value::Nil) => false,
                (None, _) => {
                    attrs.push((n.clone(), v));
                    true
                }
            };
            if changed {
                rbx.fire(id, "AttributeChanged", vec![Value::str(&n)]);
                rbx.fire(id, &format!("attr:{n}"), Vec::new());
            }
            none()
        }
        "GetAttributes" => {
            let mut t = Table::new();
            for (k, v) in &rbx.inst(id).attributes {
                t.set_str(k, v.clone());
            }
            one(Value::table(t))
        }
        "GetAttributeChangedSignal" => {
            let n = text_arg(lua, args, 1, name)?;
            one(rbx.signal(id, &format!("attr:{n}")))
        }
        "AddTag" | "RemoveTag" => tag(rbx, lua, id, name, args),
        "HasTag" => {
            let t = text_arg(lua, args, 1, name)?;
            one(Value::Bool(rbx.inst(id).tags.contains(&t)))
        }
        "GetTags" => {
            let tags: Vec<Value> = rbx.inst(id).tags.iter().map(|t| Value::str(t)).collect();
            one(Value::table(Table::from_list(tags)))
        }
        "GetDebugId" => one(Value::str(&format!("hs_{id}"))),
        _ => one(Value::Nil),
    }
}

fn tag(rbx: &mut Rbx, lua: &Lua, id: Id, name: &str, args: &[Value]) -> Res {
    let t = text_arg(lua, args, 1, name)?;
    let cs = rbx.service("CollectionService").unwrap_or(rbx.game);
    let has = rbx.inst(id).tags.contains(&t);
    let v = rbx.value(id);
    if name == "AddTag" && !has {
        rbx.inst_mut(id).tags.push(t.clone());
        rbx.fire(cs, &format!("tag+:{t}"), vec![v]);
        rbx.fire(cs, "TagAdded", vec![Value::str(&t)]);
    } else if name == "RemoveTag" && has {
        rbx.inst_mut(id).tags.retain(|x| *x != t);
        rbx.fire(cs, &format!("tag-:{t}"), vec![v]);
        rbx.fire(cs, "TagRemoved", vec![Value::str(&t)]);
    }
    none()
}

/// `Enum.KeyCode` and the like as an argument.
fn enum_arg(
    lua: &Lua,
    args: &[Value],
    i: usize,
    fname: &str,
    e: &str,
) -> Result<super::hstudio_rbx_values::EnumItem, Throw> {
    args.get(i)
        .and_then(|v| coerce_enum(e, v))
        .ok_or_else(|| bad_arg(lua, i, fname, &format!("Enum.{e} expected")))
}

fn service_method(env: &mut dyn Env, lua: &mut Lua, id: Id, name: &str, owner_class: &str, args: &[Value]) -> Res {
    let owner = env.owner();
    let now = env.now();
    match (owner_class, name) {
        ("DataModel", "GetService" | "FindService") => {
            let n = text_arg(lua, args, 1, name)?;
            // CoreGui is for identity 3 and up (setthreadidentity).
            if n == "CoreGui" && super::hstudio_exec::identity(lua) < 3 {
                return one(Value::Nil);
            }
            let rbx = env.rbx();
            match rbx.service(&n).or_else(|| rbx.api_service(&n)) {
                Some(s) => one(rbx.value(s)),
                None if name == "FindService" => one(Value::Nil),
                None => Err(lua.rt_error(format!("'{n}' is not a valid Service name"))),
            }
        }
        ("DataModel", "IsLoaded") => one(Value::Bool(true)),
        // TestService's output goes to the console, as Roblox's Output.
        ("TestService", "Message" | "Warn" | "Error" | "Checkpoint" | "Fail" | "Done") => {
            let text = args.get(1).and_then(Value::as_text).unwrap_or_default();
            let h: &mut dyn Host = env;
            match name {
                "Warn" | "Error" | "Fail" => h.print(&format!("warning: {text}")),
                _ => h.print(&text),
            }
            none()
        }
        ("TestService", "Check" | "Require") => {
            let ok = args.get(1).is_some_and(Value::truthy);
            if !ok {
                let text = args.get(2).and_then(Value::as_text).unwrap_or_default();
                let h: &mut dyn Host = env;
                h.print(&format!("warning: {text}"));
            }
            none()
        }
        // Assets load as they are drawn here: nothing to wait for.
        ("ContentProvider", "Preload" | "PreloadAsync") => none(),
        ("DataModel", "BindToClose") => none(),
        (
            "Players",
            "GetPlayers"
            | "GetPlayerFromCharacter"
            | "GetPlayerByUserId"
            | "GetNameFromUserIdAsync"
            | "GetUserIdFromNameAsync",
        )
        | ("Player", "GetMouse" | "Kick" | "LoadCharacter" | "DistanceFromCharacter" | "GetNetworkPing") => {
            super::hstudio_rbx_world::player_method(env, lua, id, name, args)
        }
        // The user's picture, as Roblox's rbxthumb:// content, which the
        // GUI's images fetch from the thumbnails API.
        ("Players", "GetUserThumbnailAsync") => {
            let user = check_num(lua, args, 1, name)? as u64;
            let kind = match args.get(2).map(super::hstudio_rbx_draw::enum_name) {
                Some("AvatarBust") => "AvatarBust",
                Some("AvatarThumbnail") => "Avatar",
                _ => "AvatarHeadShot",
            };
            let size = args
                .get(3)
                .map(super::hstudio_rbx_draw::enum_name)
                .and_then(|n| {
                    n.strip_prefix("Size")
                        .and_then(|s| s.split('x').next())
                        .and_then(|w| w.parse::<u32>().ok())
                })
                .unwrap_or(150);
            let content = format!("rbxthumb://type={kind}&id={user}&w={size}&h={size}");
            Ok(vec![Value::str(&content), Value::Bool(true)])
        }
        ("Player", "IsFriendsWith" | "IsInGroup") => one(Value::Bool(false)),
        ("Player", "GetRankInGroup") => one(Value::Num(0.0)),
        ("RunService", "IsClient" | "IsRunning" | "IsRunMode") => one(Value::Bool(true)),
        ("RunService", "IsServer" | "IsStudio") => one(Value::Bool(false)),
        ("RunService", "BindToRenderStep") => {
            let n = text_arg(lua, args, 1, name)?;
            let prio = check_num(lua, args, 2, name)?;
            let f = args
                .get(3)
                .filter(|f| f.is_callable())
                .cloned()
                .ok_or_else(|| bad_arg(lua, 3, name, "function expected"))?;
            let rbx = env.rbx();
            rbx.render_steps.retain(|r| r.0 != n);
            rbx.render_steps.push((n, prio, owner, f));
            rbx.render_steps.sort_by(|a, b| a.1.total_cmp(&b.1));
            none()
        }
        ("RunService", "UnbindFromRenderStep") => {
            let n = text_arg(lua, args, 1, name)?;
            env.rbx().render_steps.retain(|r| r.0 != n);
            none()
        }
        ("UserInputService", "IsKeyDown") => {
            let k = enum_arg(lua, args, 1, name, "KeyCode")?;
            one(Value::Bool(env.rbx().pointer.held.contains(k.name)))
        }
        ("UserInputService", "IsMouseButtonPressed") => {
            let t = enum_arg(lua, args, 1, name, "UserInputType")?;
            let b = env.rbx().pointer.buttons;
            one(Value::Bool(match t.name {
                "MouseButton1" => b[0],
                "MouseButton2" => b[1],
                "MouseButton3" => b[2],
                _ => false,
            }))
        }
        ("UserInputService", "GetMouseLocation") => {
            let p = &env.rbx().pointer;
            one(v2(Vector2::new(p.x, p.y)))
        }
        ("UserInputService", "GetMouseDelta") => one(v2(Vector2::ZERO)),
        ("UserInputService", "GetKeysPressed") => {
            let mut held: Vec<&'static str> = env.rbx().pointer.held.iter().copied().collect();
            held.sort_unstable();
            let items = held
                .into_iter()
                .map(|k| super::hstudio_rbx_draw::input_object(env.rbx(), Some(k), "Keyboard", "Begin", None))
                .collect();
            one(Value::table(Table::from_list(items)))
        }
        ("UserInputService", "GetMouseButtonsPressed") => {
            let b = env.rbx().pointer.buttons;
            let mut items = Vec::new();
            for (i, n) in ["MouseButton1", "MouseButton2", "MouseButton3"].into_iter().enumerate() {
                if b[i] {
                    items.push(super::hstudio_rbx_draw::input_object(env.rbx(), None, n, "Begin", None));
                }
            }
            one(Value::table(Table::from_list(items)))
        }
        ("UserInputService", "GetFocusedTextBox") => {
            let rbx = env.rbx();
            let f = rbx.pointer.focus;
            one(opt_value(rbx, f))
        }
        ("UserInputService", "GetLastInputType") => one(enum_value(enum_item("UserInputType", "Keyboard"))),
        ("UserInputService", "GetStringForKeyCode") => {
            let k = enum_arg(lua, args, 1, name, "KeyCode")?;
            one(Value::str(if k.name.len() == 1 { k.name } else { "" }))
        }
        ("TweenService", "Create") => {
            let target = args
                .get(1)
                .and_then(|v| env.rbx().id_of(v))
                .ok_or_else(|| bad_arg(lua, 1, name, "Instance expected"))?;
            let info = tween_info_arg(lua, args, 2, name)?;
            let Some(Value::Table(goals)) = args.get(3) else {
                return Err(bad_arg(lua, 3, name, "a table of property goals expected"));
            };
            let rbx = env.rbx();
            let mut list = Vec::new();
            for (k, v) in goals.borrow().entries() {
                let k = k
                    .as_text()
                    .ok_or_else(|| lua.rt_error("TweenService:Create: goal names are strings"))?;
                let p = tween_goal(rbx, target, &k, &v).map_err(|e| lua.rt_error(e))?;
                list.push((p, v));
            }
            let tid = make_tween(rbx, target, info, list, owner, None);
            one(rbx.value(tid))
        }
        ("TweenService", "GetValue") => {
            let a = check_num(lua, args, 1, name)?;
            let style = enum_arg(lua, args, 2, name, "EasingStyle")?;
            let dir = enum_arg(lua, args, 3, name, "EasingDirection")?;
            let style = EasingStyle::from_name(style.name).ok_or_else(|| lua.rt_error("unsupported EasingStyle"))?;
            let dir = EasingDirection::from_name(dir.name).unwrap_or(EasingDirection::Out);
            one(Value::Num(ease(style, dir, a)))
        }
        ("Tween", "Play") => {
            let rbx = env.rbx();
            let target = rbx.tween_value(id).map(|t| t.target);
            if let Some(target) = target {
                // A tween that starts takes over the properties from the others.
                let props: Vec<&'static str> = rbx
                    .tween_value(id)
                    .map(|t| t.goals.iter().map(|g| g.0).collect())
                    .unwrap_or_default();
                for other in rbx
                    .tweens
                    .iter_mut()
                    .filter(|t| t.id != id && t.target == target && t.playing)
                {
                    if other.goals.iter().any(|g| props.contains(&g.0)) {
                        other.playing = false;
                    }
                }
                let starts: Vec<Value> = props.iter().map(|p| rbx.raw_get(target, p)).collect();
                if let Some(t) = rbx.tweens.iter_mut().find(|t| t.id == id) {
                    let state = rbx_state(&rbx_state_of(t));
                    if state != "Paused" {
                        t.elapsed = 0.0;
                        t.starts = starts;
                    }
                    t.playing = true;
                }
                rbx.raw_set(id, "PlaybackState", enum_value(enum_item("PlaybackState", "Playing")));
            }
            none()
        }
        ("Tween", "Pause") => {
            let rbx = env.rbx();
            if let Some(t) = rbx.tweens.iter_mut().find(|t| t.id == id) {
                t.playing = false;
            }
            rbx.raw_set(id, "PlaybackState", enum_value(enum_item("PlaybackState", "Paused")));
            none()
        }
        ("Tween", "Cancel") => {
            let rbx = env.rbx();
            if let Some(t) = rbx.tweens.iter_mut().find(|t| t.id == id) {
                t.playing = false;
                t.elapsed = 0.0;
            }
            let cancelled = enum_value(enum_item("PlaybackState", "Cancelled"));
            rbx.raw_set(id, "PlaybackState", cancelled.clone());
            rbx.fire(id, "Completed", vec![cancelled]);
            none()
        }
        ("Debris", "AddItem") => {
            let target = args
                .get(1)
                .and_then(|v| env.rbx().id_of(v))
                .ok_or_else(|| bad_arg(lua, 1, name, "Instance expected"))?;
            let secs = opt_num(lua, args, 2, name, 10.0)?;
            env.rbx().debris.push((target, now + secs.max(0.0)));
            none()
        }
        ("HttpService", "JSONEncode") => {
            let v = args.get(1).cloned().unwrap_or_default();
            let j = super::hstudio::lua_to_json_roblox(&v)
                .map_err(|e| lua.rt_error(format!("Can't convert to JSON: {e}")))?;
            one(Value::str(&j.to_string()))
        }
        ("HttpService", "JSONDecode") => {
            let t = text_arg(lua, args, 1, name)?;
            let j: serde_json::Value =
                serde_json::from_str(&t).map_err(|e| lua.rt_error(format!("Can't parse JSON: {e}")))?;
            one(json_to_lua(&j))
        }
        ("HttpService", "GenerateGUID") => {
            let braces = !matches!(args.get(1), Some(Value::Bool(false)));
            let mut rng = super::hstudio_rbx_types::Random::from_entropy();
            let mut hex = String::new();
            for i in 0..16 {
                let mut b = (rng.next_u32() & 0xFF) as u8;
                if i == 6 {
                    b = (b & 0x0F) | 0x40;
                }
                if i == 8 {
                    b = (b & 0x3F) | 0x80;
                }
                hex.push_str(&format!("{b:02X}"));
                if matches!(i, 3 | 5 | 7 | 9) {
                    hex.push('-');
                }
            }
            one(Value::str(&if braces { format!("{{{hex}}}") } else { hex }))
        }
        ("HttpService", "UrlEncode") => {
            let t = text_arg(lua, args, 1, name)?;
            let mut out = String::new();
            for b in t.bytes() {
                if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                    out.push(b as char);
                } else {
                    out.push_str(&format!("%{b:02X}"));
                }
            }
            one(Value::str(&out))
        }
        ("HttpService", _) => Err(lua.rt_error(format!("HttpService:{name} is not available here"))),
        ("WebSocket", "Send" | "Close") => {
            let data = args.get(1).and_then(|v| v.as_bytes().map(<[u8]>::to_vec));
            super::hstudio_sunc::socket_method(env.rbx(), id, name, data);
            none()
        }
        ("TextService", "GetTextSize") => {
            let text = text_arg(lua, args, 1, name)?;
            let size = check_num(lua, args, 2, name)?;
            let font = args.get(3).and_then(|v| coerce_enum("Font", v));
            let frame = args
                .get(4)
                .and_then(as_v2)
                .unwrap_or(Vector2::new(f64::INFINITY, f64::INFINITY));
            let face = super::hstudio_rbx_draw::font_of_enum(font);
            let (w, h) = super::hstudio_rbx_draw::text_size(env.game(), &text, size, face, frame.x);
            one(v2(Vector2::new(w, h)))
        }
        // As GetTextSize, from a GetTextBoundsParams; its Width wraps the
        // text (0 doesn't), and rich text is measured run by run.
        ("TextService", "GetTextBoundsAsync") => {
            let rbx = env.rbx();
            let p = args
                .get(1)
                .and_then(|v| rbx.id_of(v))
                .filter(|&p| rbx.inst(p).class.name == "GetTextBoundsParams")
                .ok_or_else(|| bad_arg(lua, 1, name, "GetTextBoundsParams expected"))?;
            let text = rbx.raw_get(p, "Text").as_text().unwrap_or_default();
            let size = rbx.raw_get(p, "Size").to_num().unwrap_or(8.0);
            let font = super::hstudio_rbx_values::as_font(&rbx.raw_get(p, "Font")).unwrap_or_default();
            let width = rbx.raw_get(p, "Width").to_num().filter(|w| *w > 0.0);
            let rich = rbx.raw_get(p, "RichText").truthy();
            let game = env.game();
            let (w, h) = if rich {
                let read = super::hstudio_rbx_layout::parse_rich(&text);
                let mut measure = |t: &str, s: f64, f: &super::hstudio_rbx_types::Font| -> f64 {
                    super::hstudio_rbx_draw::measure_rbx(game, t, s, f)
                };
                let b =
                    super::hstudio_rbx_layout::text_bounds(&read.text, &read.spans, size, &font, width, &mut measure);
                (b.x.ceil(), b.y.ceil())
            } else {
                super::hstudio_rbx_draw::text_size(game, &text, size, font, width.unwrap_or(f64::INFINITY))
            };
            one(v2(Vector2::new(w, h)))
        }
        ("GuiService", "GetGuiInset") => Ok(vec![v2(Vector2::ZERO), v2(Vector2::ZERO)]),
        ("GuiService", "IsTenFootInterface") => one(Value::Bool(false)),
        ("StarterGui", "SetCore") => {
            let what = text_arg(lua, args, 1, name)?;
            if what == "SendNotification"
                && let Some(Value::Table(t)) = args.get(2)
            {
                let title = t.borrow().get_str("Title").as_text().unwrap_or_default();
                let text = t.borrow().get_str("Text").as_text().unwrap_or_default();
                let line = match (title.is_empty(), text.is_empty()) {
                    (false, false) => format!("{title}: {text}"),
                    (false, true) => title,
                    _ => text,
                };
                env.rbx().warnings.push(format!("\u{1}{line}"));
            }
            none()
        }
        ("StarterGui", "GetCore") => one(Value::Nil),
        ("StarterGui", "SetCoreGuiEnabled") => {
            let t = enum_arg(lua, args, 1, name, "CoreGuiType")?;
            let on = bool_arg(args, 2);
            let rbx = env.rbx();
            if on {
                rbx.core_gui_off.remove(t.name);
            } else {
                rbx.core_gui_off.insert(t.name);
            }
            none()
        }
        ("StarterGui", "GetCoreGuiEnabled") => {
            let t = enum_arg(lua, args, 1, name, "CoreGuiType")?;
            one(Value::Bool(!env.rbx().core_gui_off.contains(t.name)))
        }
        ("CollectionService", "GetTagged") => {
            let t = text_arg(lua, args, 1, name)?;
            let rbx = env.rbx();
            let ids = rbx.tagged(&t);
            one(rbx.list(&ids))
        }
        ("CollectionService", "GetInstanceAddedSignal" | "GetInstanceRemovedSignal") => {
            let t = text_arg(lua, args, 1, name)?;
            let sign = if name == "GetInstanceAddedSignal" { "+" } else { "-" };
            let rbx = env.rbx();
            one(rbx.signal(id, &format!("tag{sign}:{t}")))
        }
        ("CollectionService", _) => {
            // CollectionService:AddTag(inst, tag) and the like.
            let target = args
                .get(1)
                .and_then(|v| env.rbx().id_of(v))
                .ok_or_else(|| bad_arg(lua, 1, name, "Instance expected"))?;
            let rest: Vec<Value> = std::iter::once(env.rbx().value(target))
                .chain(args[2..].iter().cloned())
                .collect();
            instance_method(env, lua, target, name, &rest)
        }
        ("ContextActionService", "BindAction" | "BindActionAtPriority") => {
            let n = text_arg(lua, args, 1, name)?;
            let f = args
                .get(2)
                .filter(|f| f.is_callable())
                .cloned()
                .ok_or_else(|| bad_arg(lua, 2, name, "function expected"))?;
            let skip = if name == "BindActionAtPriority" { 5 } else { 4 };
            let inputs: Vec<_> = args.iter().skip(skip).filter_map(as_item).collect();
            let rbx = env.rbx();
            rbx.actions.retain(|a| a.0 != n);
            rbx.actions.insert(0, (n, owner, f, inputs));
            none()
        }
        ("ContextActionService", "UnbindAction") => {
            let n = text_arg(lua, args, 1, name)?;
            env.rbx().actions.retain(|a| a.0 != n);
            none()
        }
        ("ContextActionService", "UnbindAllActions") => {
            env.rbx().actions.retain(|a| a.1 != owner);
            none()
        }
        ("Workspace", "Raycast") => {
            let origin = args
                .get(1)
                .and_then(as_v3)
                .ok_or_else(|| bad_arg(lua, 1, name, "Vector3 expected"))?;
            let dir = args
                .get(2)
                .and_then(as_v3)
                .ok_or_else(|| bad_arg(lua, 2, name, "Vector3 expected"))?;
            let params = match args.get(3) {
                Some(Value::Table(t)) => Some(t.clone()),
                _ => None,
            };
            one(super::hstudio_rbx_query::raycast(env, origin, dir, params.as_ref()))
        }
        (
            "Workspace",
            "GetPartBoundsInRadius"
            | "GetPartBoundsInBox"
            | "GetPartsInPart"
            | "FindPartOnRay"
            | "FindPartOnRayWithIgnoreList"
            | "FindPartOnRayWithWhitelist",
        ) => super::hstudio_rbx_query::workspace_method(env, lua, name, args),
        // A copy of the terrain's region: Minecraft's blocks aren't Roblox's
        // voxels, so it holds none (its size says the region's).
        ("Terrain", "CopyRegion") => {
            let owner = env.owner();
            let rbx = env.rbx();
            let region = rbx.make("TerrainRegion", Some(owner));
            rbx.inst_mut(region).name = "TerrainRegion".into();
            one(rbx.value(region))
        }
        ("Terrain", _) => super::hstudio_rbx_query::terrain_method(env, lua, name, args),
        ("Workspace", "GetServerTimeNow") => one(Value::Num(now)),
        ("Camera", "WorldToViewportPoint" | "WorldToScreenPoint") => {
            let p = args
                .get(1)
                .and_then(as_v3)
                .ok_or_else(|| bad_arg(lua, 1, name, "Vector3 expected"))?;
            let (at, on) = super::hstudio_rbx_draw::to_screen(env, p);
            Ok(vec![v3(at), Value::Bool(on)])
        }
        ("Camera", "ViewportPointToRay" | "ScreenPointToRay") => {
            let x = check_num(lua, args, 1, name)?;
            let y = check_num(lua, args, 2, name)?;
            let depth = opt_num(lua, args, 3, name, 0.0)?;
            let (o, d) = super::hstudio_rbx_draw::screen_ray(env, x, y);
            one(wrap(Data::Ray(o + d * depth, d)))
        }
        ("PVInstance", "GetPivot") => {
            let rbx = env.rbx();
            one(cf(pivot(rbx, id)))
        }
        ("PVInstance", "PivotTo") => {
            let target = args
                .get(1)
                .and_then(as_cf)
                .ok_or_else(|| bad_arg(lua, 1, name, "CFrame expected"))?;
            move_to(env.rbx(), id, target);
            none()
        }
        ("Model", "SetPrimaryPartCFrame") => {
            let target = args
                .get(1)
                .and_then(as_cf)
                .ok_or_else(|| bad_arg(lua, 1, name, "CFrame expected"))?;
            move_to(env.rbx(), id, target);
            none()
        }
        ("Model", "GetPrimaryPartCFrame") => {
            let rbx = env.rbx();
            one(cf(pivot(rbx, id)))
        }
        ("Model", "MoveTo") => {
            let p = args
                .get(1)
                .and_then(as_v3)
                .ok_or_else(|| bad_arg(lua, 1, name, "Vector3 expected"))?;
            // The player's character is moved up out of the blocks in its way,
            // as Roblox moves a model up out of what is in its way.
            let p = if id == env.rbx().character {
                let (rbx, game) = env.rbx_and_game();
                super::hstudio_rbx_world::room_for_player(game, rbx.world.root_y, p)
            } else {
                p
            };
            let rbx = env.rbx();
            let now_at = pivot(rbx, id);
            move_to(rbx, id, CFrame::from_position(p) * now_at.rotation());
            none()
        }
        ("BasePart", "GetMass") => {
            let s = as_v3(&env.rbx().raw_get(id, "Size")).unwrap_or_default();
            one(Value::Num(s.x * s.y * s.z * 0.7))
        }
        ("BasePart", "GetTouchingParts") => one(Value::table(Table::new())),
        ("Humanoid", "MoveTo") => {
            let p = args
                .get(1)
                .and_then(as_v3)
                .ok_or_else(|| bad_arg(lua, 1, name, "Vector3 expected"))?;
            super::hstudio_rbx_world::move_to(env.rbx(), id, p);
            none()
        }
        ("Humanoid", "TakeDamage") => {
            let n = check_num(lua, args, 1, name)?;
            let rbx = env.rbx();
            let h = rbx.raw_get(id, "Health").to_num().unwrap_or(100.0);
            rbx.raw_set(id, "Health", Value::Num((h - n.max(0.0)).max(0.0)));
            none()
        }
        ("Humanoid", "ChangeState") => none(),
        // Which states the Humanoid may enter: all, until a script says.
        ("Humanoid", "SetStateEnabled" | "GetStateEnabled") => {
            let state = match args.get(1) {
                Some(v) => match super::hstudio_rbx_values::get(v) {
                    Some(Data::Item(it)) => it.name.to_owned(),
                    _ => match v.to_num() {
                        Some(n) => super::hstudio_rbx_values::enum_def("HumanoidStateType")
                            .and_then(|d| d.items.iter().find(|i| f64::from(i.1) == n))
                            .map_or_else(|| n.to_string(), |i| i.0.to_owned()),
                        None => return Err(bad_arg(lua, 1, name, "HumanoidStateType expected")),
                    },
                },
                None => return Err(bad_arg(lua, 1, name, "HumanoidStateType expected")),
            };
            let key = format!("StateEnabled:{state}");
            let rbx = env.rbx();
            if name == "GetStateEnabled" {
                return one(Value::Bool(rbx.inst(id).extra.get(&key).is_none_or(Value::truthy)));
            }
            let on = args.get(2).is_some_and(Value::truthy);
            rbx.inst_mut(id).extra.insert(key, Value::Bool(on));
            none()
        }
        // A message to the chat: the game's chat (its commands aside).
        ("TextChannel", "SendAsync") => {
            let text = args.get(1).and_then(Value::as_text).unwrap_or_default();
            send_chat(env, &text);
            none()
        }
        ("RemoteEvent", "FireServer") => {
            let rbx = env.rbx();
            let say = rbx.inst(id).name == "SayMessageRequest"
                && rbx
                    .inst(id)
                    .parent
                    .is_some_and(|p| rbx.inst(p).name == "DefaultChatSystemChatEvents");
            if say {
                let text = args.get(1).and_then(Value::as_text).unwrap_or_default();
                send_chat(env, &text);
            }
            none()
        }
        ("MarketplaceService", "GetProductInfo") => {
            let asset = args.get(1).and_then(Value::to_num).unwrap_or(0.0);
            let mut t = Table::new();
            let rbx = env.rbx();
            let place = rbx.raw_get(rbx.game, "PlaceId").to_num().unwrap_or(0.0);
            let title = if asset == place { "Minecraft" } else { "" };
            t.set_str("Name", Value::str(title));
            t.set_str("Description", Value::str(""));
            t.set_str("AssetId", Value::Num(asset));
            t.set_str("ProductId", Value::Num(asset));
            t.set_str("AssetTypeId", Value::Num(9.0));
            t.set_str("PriceInRobux", Value::Num(0.0));
            t.set_str("IsForSale", Value::Bool(false));
            t.set_str("IsPublicDomain", Value::Bool(false));
            t.set_str("IsLimited", Value::Bool(false));
            t.set_str("Created", Value::str("2006-01-01T00:00:00Z"));
            t.set_str("Updated", Value::str("2006-01-01T00:00:00Z"));
            let mut creator = Table::new();
            creator.set_str("Name", Value::str("Roblox"));
            creator.set_str("Id", Value::Num(1.0));
            creator.set_str("CreatorType", Value::str("User"));
            creator.set_str("CreatorTargetId", Value::Num(1.0));
            t.set_str("Creator", Value::table(creator));
            one(Value::table(t))
        }
        // settings():GetService and UserSettings():GetService: a settings
        // object of theirs by its class.
        ("GlobalSettings" | "UserSettings", "GetService") => {
            let n = text_arg(lua, args, 1, name)?;
            let rbx = env.rbx();
            let found = rbx
                .inst(id)
                .children
                .iter()
                .copied()
                .find(|&c| rbx.inst(c).class.name == n);
            match found {
                Some(c) => one(rbx.value(c)),
                None => Err(lua.rt_error(format!("'{n}' is not a valid Service name"))),
            }
        }
        ("GlobalSettings", "GetFFlag") => one(Value::Bool(false)),
        ("GlobalSettings", "GetFVariable") => one(Value::str("")),
        ("Humanoid", "GetState") => {
            let state = super::hstudio_rbx_world::state_of(env.rbx(), id);
            one(enum_value(enum_item("HumanoidStateType", state)))
        }
        ("Humanoid", "UnequipTools") => none(),
        ("Humanoid" | "Animator", "GetPlayingAnimationTracks") => {
            let rbx = env.rbx();
            // A Humanoid's tracks and its Animator's are the same tracks.
            let mine =
                |owner: Id| owner == id || rbx.inst(owner).parent == Some(id) || rbx.inst(id).parent == Some(owner);
            let tracks: Vec<Id> = rbx
                .anim_tracks
                .iter()
                .filter(|(o, t)| {
                    rbx.try_inst(*t).is_some_and(|i| !i.destroyed)
                        && rbx.try_inst(*o).is_some()
                        && mine(*o)
                        && rbx.inst(*t).extra.get("IsPlaying").is_some_and(Value::truthy)
                })
                .map(|(_, t)| *t)
                .collect();
            one(rbx.list(&tracks))
        }
        ("Humanoid" | "Animator" | "AnimationController", "LoadAnimation") => {
            let owner = env.owner();
            let rbx = env.rbx();
            let anim = args
                .get(1)
                .and_then(|v| rbx.id_of(v))
                .filter(|&a| rbx.inst(a).class.name == "Animation")
                .ok_or_else(|| bad_arg(lua, 1, name, "Animation expected"))?;
            let track = rbx.make("AnimationTrack", Some(owner));
            let anim_name = rbx.inst(anim).name.clone();
            rbx.inst_mut(track).name = anim_name;
            let av = rbx.value(anim);
            let extra = &mut rbx.inst_mut(track).extra;
            extra.insert("Animation".into(), av);
            extra.insert("IsPlaying".into(), Value::Bool(false));
            extra.insert("Speed".into(), Value::Num(1.0));
            extra.insert("Length".into(), Value::Num(1.0));
            rbx.anim_tracks.push((id, track));
            one(rbx.value(track))
        }
        ("AnimationTrack", "Play") => {
            let rbx = env.rbx();
            if let Some(speed) = args.get(3).and_then(Value::to_num) {
                rbx.inst_mut(id).extra.insert("Speed".into(), Value::Num(speed));
            }
            let was = rbx.inst(id).extra.get("IsPlaying").is_some_and(Value::truthy);
            rbx.inst_mut(id).extra.insert("IsPlaying".into(), Value::Bool(true));
            if !was {
                rbx.fire(id, "Changed", vec![Value::str("IsPlaying")]);
            }
            none()
        }
        ("AnimationTrack", "Stop") => {
            let rbx = env.rbx();
            let was = rbx.inst(id).extra.get("IsPlaying").is_some_and(Value::truthy);
            rbx.inst_mut(id).extra.insert("IsPlaying".into(), Value::Bool(false));
            if was {
                rbx.fire(id, "Changed", vec![Value::str("IsPlaying")]);
                rbx.fire(id, "Stopped", Vec::new());
                rbx.fire(id, "Ended", Vec::new());
            }
            none()
        }
        ("AnimationTrack", "AdjustSpeed" | "AdjustWeight") => {
            let n = args
                .get(1)
                .map(|v| v.to_num().or_else(|| v.as_text().and_then(|t| t.trim().parse().ok())))
                .unwrap_or(Some(1.0))
                .ok_or_else(|| bad_arg(lua, 1, name, "number expected"))?;
            let key = if name == "AdjustSpeed" { "Speed" } else { "WeightTarget" };
            env.rbx().inst_mut(id).extra.insert(key.into(), Value::Num(n));
            none()
        }
        ("AnimationTrack", "GetMarkerReachedSignal") => {
            let marker = text_arg(lua, args, 1, name)?;
            one(env.rbx().signal(id, &format!("Marker:{marker}")))
        }
        // Emotes are the game's own: none plays here.
        ("Humanoid", "PlayEmote") => one(Value::Bool(false)),
        ("BindableEvent", "Fire") => {
            env.rbx().fire(id, "Event", args[1..].to_vec());
            none()
        }
        ("BindableFunction", "Invoke") => {
            let f = env.rbx().raw_get(id, "OnInvoke");
            if !f.is_callable() {
                return Err(lua.rt_error("BindableFunction:Invoke: OnInvoke is not set"));
            }
            let h: &mut dyn Host = env;
            lua.call(h, &f, args[1..].to_vec())
        }
        // A video doesn't load here: playing it only says so.
        ("VideoFrame", "Play" | "Pause") => {
            let play = name == "Play";
            let rbx = env.rbx();
            rbx.raw_set(id, "Playing", Value::Bool(play));
            let video = rbx.raw_get(id, "Video");
            rbx.fire(id, if play { "Played" } else { "Paused" }, vec![video]);
            none()
        }
        ("Sound", "Play" | "Resume") => {
            let rbx = env.rbx();
            rbx.quiet_set(id, "Playing", Value::Bool(true));
            rbx.play_sound(id);
            none()
        }
        ("Sound", "Stop" | "Pause") => {
            let rbx = env.rbx();
            rbx.raw_set(id, "Playing", Value::Bool(false));
            rbx.quiet_set(id, "IsPlaying", Value::Bool(false));
            rbx.fire(
                id,
                if name == "Stop" { "Stopped" } else { "Paused" },
                vec![Value::str("")],
            );
            none()
        }
        ("TextBox", "CaptureFocus") => {
            super::hstudio_rbx_draw::focus(env.rbx(), Some(id));
            none()
        }
        ("TextBox", "ReleaseFocus") => {
            let rbx = env.rbx();
            if rbx.pointer.focus == Some(id) {
                super::hstudio_rbx_draw::unfocus(rbx, bool_arg(args, 1));
            }
            none()
        }
        ("TextBox", "IsFocused") => one(Value::Bool(env.rbx().pointer.focus == Some(id))),
        ("GuiObject", "TweenPosition" | "TweenSize" | "TweenSizeAndPosition") => {
            let both = name == "TweenSizeAndPosition";
            let first = udim2_arg(lua, args, 1, name)?;
            let second = if both {
                Some(udim2_arg(lua, args, 2, name)?)
            } else {
                None
            };
            let at = if both { 3 } else { 2 };
            let info = legacy_info(lua, args, at, name)?;
            let override_ = bool_arg(args, at + 3);
            let callback = args.get(at + 4).filter(|f| f.is_callable()).cloned();
            let rbx = env.rbx();
            let busy = rbx
                .tweens
                .iter()
                .any(|t| t.target == id && t.playing && t.callback.is_some());
            if busy && !override_ {
                return one(Value::Bool(false));
            }
            let mut goals = Vec::new();
            match name {
                "TweenPosition" => goals.push(("Position", super::hstudio_rbx_values::udim2(first))),
                "TweenSize" => goals.push(("Size", super::hstudio_rbx_values::udim2(first))),
                _ => {
                    goals.push(("Size", super::hstudio_rbx_values::udim2(first)));
                    goals.push(("Position", super::hstudio_rbx_values::udim2(second.unwrap_or(first))));
                }
            }
            let tid = make_tween(rbx, id, info, goals, owner, Some(callback.unwrap_or_default()));
            start_tween(rbx, tid);
            one(Value::Bool(true))
        }
        ("UIPageLayout", _) => {
            let rbx = env.rbx();
            let pages = super::hstudio_rbx_draw::pages(rbx, id);
            let page = match name {
                "JumpTo" => Some(
                    args.get(1)
                        .and_then(|v| rbx.id_of(v))
                        .ok_or_else(|| bad_arg(lua, 1, name, "Instance expected"))?,
                ),
                "JumpToIndex" => {
                    let i = check_num(lua, args, 1, name)?;
                    (i >= 0.0).then(|| pages.get(i as usize).copied()).flatten()
                }
                _ => super::hstudio_rbx_draw::page_beside(rbx, id, &pages, name == "Next"),
            };
            if let Some(p) = page {
                let forward = match name {
                    "Next" => Some(true),
                    "Previous" => Some(false),
                    _ => None,
                };
                super::hstudio_rbx_draw::turn_page(rbx, id, p, forward);
            }
            none()
        }
        ("PlayerGui" | "LayerCollector", "GetGuiObjectsAtPosition") => {
            let x = check_num(lua, args, 1, name)?;
            let y = check_num(lua, args, 2, name)?;
            let rbx = env.rbx();
            let ids = super::hstudio_rbx_draw::objects_at(rbx, x, y);
            one(rbx.list(&ids))
        }
        _ => Err(lua.rt_error(format!("{name} is not available here"))),
    }
}

/// A tween instance of `target`, not playing yet.
pub fn make_tween(
    rbx: &mut Rbx,
    target: Id,
    info: TweenInfo,
    goals: Vec<(&'static str, Value)>,
    owner: super::hstudio::Owner,
    callback: Option<Value>,
) -> Id {
    let tid = rbx.make("Tween", Some(owner));
    let tv = rbx.value(target);
    rbx.quiet_set(tid, "Instance", tv);
    rbx.tweens.push(Tween {
        id: tid,
        target,
        info,
        goals,
        starts: Vec::new(),
        elapsed: 0.0,
        playing: false,
        owner,
        callback,
    });
    tid
}

fn start_tween(rbx: &mut Rbx, tid: Id) {
    let (target, props) = match rbx.tween_value(tid) {
        Some(t) => (t.target, t.goals.iter().map(|g| g.0).collect::<Vec<_>>()),
        None => return,
    };
    let starts: Vec<Value> = props.iter().map(|p| rbx.raw_get(target, p)).collect();
    for other in rbx
        .tweens
        .iter_mut()
        .filter(|t| t.id != tid && t.target == target && t.playing)
    {
        if other.goals.iter().any(|g| props.contains(&g.0)) {
            other.playing = false;
        }
    }
    if let Some(t) = rbx.tweens.iter_mut().find(|t| t.id == tid) {
        t.starts = starts;
        t.elapsed = 0.0;
        t.playing = true;
    }
    rbx.raw_set(tid, "PlaybackState", enum_value(enum_item("PlaybackState", "Playing")));
}

fn rbx_state_of(t: &Tween) -> (bool, f64) {
    (t.playing, t.elapsed)
}

fn rbx_state(s: &(bool, f64)) -> &'static str {
    if !s.0 && s.1 > 0.0 { "Paused" } else { "Other" }
}

/// Moves a tween one frame on: its target's properties and its end.
pub fn step_tweens(rbx: &mut Rbx, dt: f64) {
    let mut done: Vec<(Id, Option<Value>)> = Vec::new();
    let mut sets: Vec<(Id, &'static str, Value)> = Vec::new();
    for t in rbx.tweens.iter_mut().filter(|t| t.playing) {
        t.elapsed += dt;
        let (alpha, over) = super::hstudio_rbx::tween_alpha(&t.info, t.elapsed);
        for (i, (prop, goal)) in t.goals.iter().enumerate() {
            let start = t.starts.get(i).cloned().unwrap_or_default();
            sets.push((t.target, prop, lerp_value(&start, goal, alpha)));
        }
        if over {
            t.playing = false;
            done.push((t.id, t.callback.clone()));
        }
    }
    for (id, prop, v) in sets {
        if rbx.try_inst(id).is_some_and(|i| !i.destroyed) {
            rbx.raw_set(id, prop, v);
            // A tweened Lighting, camera or gravity reaches the game too.
            super::hstudio_rbx_world::effect_set(rbx, id, prop);
            // A tween of the player's root part moves the player.
            if id == rbx.root_part && prop == "CFrame" {
                super::hstudio_rbx_world::root_moved(rbx);
            }
        }
    }
    for (tid, callback) in done {
        let completed = enum_value(enum_item("PlaybackState", "Completed"));
        rbx.raw_set(tid, "PlaybackState", completed.clone());
        rbx.fire(tid, "Completed", vec![completed]);
        if let Some(f) = callback {
            let owner = rbx.tweens.iter().find(|t| t.id == tid).map(|t| t.owner);
            if f.is_callable()
                && let Some(owner) = owner
            {
                rbx.deferred
                    .push((owner, f, vec![enum_value(enum_item("TweenStatus", "Completed"))]));
            }
            // TweenPosition's tweens are the studio's own: they go when done.
            rbx.tweens.retain(|t| t.id != tid);
            rbx.destroy(tid);
        }
    }
}

/// A model's or part's pivot.
pub fn pivot(rbx: &Rbx, id: Id) -> CFrame {
    let inst = rbx.inst(id);
    if is_a(inst.class, "BasePart") || inst.class.name == "Camera" {
        return as_cf(&inst.get("CFrame")).unwrap_or_default();
    }
    if let Some(pp) = rbx.id_of(&inst.get("PrimaryPart")) {
        return as_cf(&rbx.raw_get(pp, "CFrame")).unwrap_or_default();
    }
    let parts: Vec<Vector3> = rbx
        .descendants(id)
        .into_iter()
        .filter(|&d| is_a(rbx.inst(d).class, "BasePart"))
        .filter_map(|d| as_cf(&rbx.raw_get(d, "CFrame")).map(|c| c.position()))
        .collect();
    if parts.is_empty() {
        return as_cf(&inst.get("WorldPivot")).unwrap_or_default();
    }
    let sum = parts.iter().fold(Vector3::ZERO, |a, b| a + *b);
    CFrame::from_position(sum / parts.len() as f64)
}

/// Moves a model (every part in it) or a part to `target`, keeping their
/// offsets. Moving the player's root part moves the player.
fn move_to(rbx: &mut Rbx, id: Id, target: CFrame) {
    let from = pivot(rbx, id);
    let delta = target * from.inverse();
    let mut ids = vec![id];
    ids.extend(rbx.descendants(id));
    for d in ids {
        if is_a(rbx.inst(d).class, "BasePart")
            && let Some(c) = as_cf(&rbx.raw_get(d, "CFrame"))
        {
            rbx.raw_set(d, "CFrame", cf(delta * c));
            if d == rbx.root_part {
                super::hstudio_rbx_world::root_moved(rbx);
            }
        }
    }
    if rbx.inst(id).class.name == "Model" {
        rbx.quiet_set(id, "WorldPivot", cf(target));
    }
}

/// `Color3` white, for defaults.
pub fn white() -> Color3 {
    Color3::new(1.0, 1.0, 1.0)
}

/// Whether `class` can be made with Instance.new.
pub fn creatable(class: &str) -> bool {
    class_def(class).is_some_and(|c| c.creatable)
}

/// A Roblox script's chat message, said in the game's chat. Roblox's chat
/// commands (/e, /w) aren't the game's, so a line starting with / isn't sent.
fn send_chat(env: &mut dyn Env, text: &str) {
    let text = text.trim();
    if text.is_empty() || text.starts_with('/') {
        return;
    }
    let line: String = text.chars().filter(|c| !c.is_control()).take(256).collect();
    env.game().send(&line);
}
