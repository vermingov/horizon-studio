// Horizon Studio's Lua standard library: the base functions, string (with
// Lua patterns and string.format), math, table, coroutine and
// os.clock/time/date, as in Lua 5.3, for the interpreter in hstudio_lua
// (pcall and xpcall are the VM's own). Nothing here reaches files, the
// network or other processes.

use super::hstudio_lua::{
    Host, Lua, MAX_STRING, Native, Res, Resume, Str, Table, TableRef, Thread, Throw, Value, arg, bad_arg, check_func,
    check_int, check_num, check_str, check_table, fmt_g, fmt_num, opt_int, opt_num, opt_str, str_to_num,
};
use std::cell::RefCell;
use std::rc::Rc;

/// A fresh set of globals with the standard library; also sets the
/// interpreter's string methods the first time.
pub fn new_env(lua: &mut Lua) -> TableRef {
    let env = Rc::new(RefCell::new(Table::new()));
    {
        let mut g = env.borrow_mut();
        let base: [(&str, super::hstudio_lua::Builtin); 21] = [
            ("print", print),
            ("type", type_),
            ("typeof", typeof_),
            ("tostring", tostring),
            ("tonumber", tonumber),
            ("pairs", pairs),
            ("ipairs", ipairs),
            ("next", next),
            ("select", select),
            ("rawget", rawget),
            ("rawset", rawset),
            ("rawequal", rawequal),
            ("rawlen", rawlen),
            ("setmetatable", setmetatable),
            ("getmetatable", getmetatable),
            ("error", error),
            ("assert", assert),
            ("unpack", unpack),
            ("dump", dump),
            ("inspect", dump),
            ("collectgarbage", collectgarbage),
        ];
        for (name, f) in base {
            g.set_str(name, Native::builtin(name, f));
        }
        g.set_str("pcall", Native::pcall("pcall", false));
        g.set_str("xpcall", Native::pcall("xpcall", true));
        g.set_str(
            "load",
            Native::with_state("load", load, vec![Value::Table(env.clone())]),
        );
        g.set_str("_G", Value::Table(env.clone()));
        g.set_str("_VERSION", Value::str("Lua 5.3 (Horizon Studio)"));
        g.set_str("string", Value::Table(string_lib()));
        g.set_str("math", Value::Table(math_lib()));
        g.set_str("table", Value::Table(table_lib()));
        g.set_str("os", Value::Table(os_lib()));
        g.set_str("utf8", Value::Table(utf8_lib()));
        g.set_str("coroutine", Value::Table(coroutine_lib()));
        g.set_str("bit32", Value::Table(bit32_lib()));
        g.set_str("buffer", Value::Table(buffer_lib()));
        g.set_str("gcinfo", Native::builtin("gcinfo", gcinfo));
    }
    if lua.string_lib.is_none()
        && let Value::Table(s) = env.borrow().get_str("string")
    {
        lua.string_lib = Some(s);
    }
    env
}

fn lib(entries: &[(&str, super::hstudio_lua::Builtin)]) -> TableRef {
    let mut t = Table::new();
    for (name, f) in entries {
        t.set_str(name, Native::builtin(name, *f));
    }
    Rc::new(RefCell::new(t))
}

fn one(v: Value) -> Res {
    Ok(vec![v])
}

// ----------------------------------------------------------------- base

fn print(lua: &mut Lua, h: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let mut parts = Vec::with_capacity(args.len());
    for a in &args {
        parts.push(lua.tostring(h, a)?);
    }
    h.print(&parts.join("\t"));
    Ok(Vec::new())
}

fn type_(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    if args.is_empty() {
        return Err(bad_arg(lua, 0, "type", "value expected"));
    }
    one(Value::str(args[0].type_name()))
}

/// Luau's `typeof`: a userdata's own type name (`Vector3`, `Instance`), else `type`.
fn typeof_(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    if args.is_empty() {
        return Err(bad_arg(lua, 0, "typeof", "value expected"));
    }
    one(Value::str(&lua.type_of(&args[0])))
}

fn tostring(lua: &mut Lua, h: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let s = lua.tostring(h, &arg(&args, 0))?;
    one(Value::str(&s))
}

fn tonumber(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let v = arg(&args, 0);
    match args.get(1) {
        None | Some(Value::Nil) => one(match &v {
            Value::Num(_) => v.clone(),
            Value::Str(s) => str_to_num(s).map_or(Value::Nil, Value::Num),
            _ => Value::Nil,
        }),
        Some(_) => {
            let base = check_int(lua, &args, 1, "tonumber")?;
            if !(2..=36).contains(&base) {
                return Err(bad_arg(lua, 1, "tonumber", "base out of range"));
            }
            let s = check_str(lua, &args, 0, "tonumber")?;
            let t = String::from_utf8_lossy(&s).trim().to_lowercase();
            let (neg, digits) = match t.strip_prefix('-') {
                Some(d) => (true, d.to_owned()),
                None => (false, t),
            };
            let mut n = 0f64;
            if digits.is_empty() {
                return one(Value::Nil);
            }
            for c in digits.chars() {
                match c.to_digit(base as u32) {
                    Some(d) => n = n * base as f64 + f64::from(d),
                    None => return one(Value::Nil),
                }
            }
            one(Value::Num(if neg { -n } else { n }))
        }
    }
}

fn next(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = check_table(lua, &args, 0, "next")?;
    let r = t.borrow().next(&arg(&args, 1));
    match r {
        Ok(Some((k, v))) => Ok(vec![k, v]),
        Ok(None) => one(Value::Nil),
        Err(()) => Err(lua.rt_error("invalid key to 'next'")),
    }
}

fn pairs(lua: &mut Lua, h: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let v = arg(&args, 0);
    if let Some(mm) = lua.metamethod(&v, "__pairs") {
        let mut r = lua.call(h, &mm, vec![v])?;
        r.resize(3, Value::Nil);
        return Ok(r);
    }
    check_table(lua, &args, 0, "pairs")?;
    Ok(vec![Native::builtin("next", next), v, Value::Nil])
}

fn ipairs_step(lua: &mut Lua, h: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let i = check_num(lua, &args, 1, "ipairs")? + 1.0;
    let t = arg(&args, 0);
    let v = match &t {
        Value::Table(tb) if tb.borrow().meta.is_none() => tb.borrow().get_int(i as i64),
        _ => lua.index(h, &t, &Value::Num(i))?,
    };
    if v.is_nil() {
        one(Value::Nil)
    } else {
        Ok(vec![Value::Num(i), v])
    }
}

fn ipairs(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    if args.is_empty() || args[0].is_nil() {
        return Err(bad_arg(lua, 0, "ipairs", "table expected, got nil"));
    }
    Ok(vec![
        Native::builtin("ipairs_step", ipairs_step),
        arg(&args, 0),
        Value::Num(0.0),
    ])
}

fn select(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let n = args.len() as i64 - 1;
    if matches!(args.first(), Some(Value::Str(s)) if &**s == b"#") {
        return one(Value::Num(n as f64));
    }
    let i = check_int(lua, &args, 0, "select")?;
    let start = if i < 0 { n + i } else { i - 1 };
    if i == 0 || start < 0 {
        return Err(bad_arg(lua, 0, "select", "index out of range"));
    }
    Ok(args.into_iter().skip(1 + start as usize).collect())
}

fn rawget(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = check_table(lua, &args, 0, "rawget")?;
    let v = t.borrow().get(&arg(&args, 1));
    one(v)
}

fn rawset(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = check_table(lua, &args, 0, "rawset")?;
    let r = t.borrow_mut().set(arg(&args, 1), arg(&args, 2));
    r.map_err(|e| lua.rt_error(e))?;
    one(args[0].clone())
}

fn rawequal(_: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    one(Value::Bool(arg(&args, 0).raw_eq(&arg(&args, 1))))
}

fn rawlen(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    match args.first() {
        Some(Value::Table(t)) => one(Value::Num(t.borrow().len() as f64)),
        Some(Value::Str(s)) => one(Value::Num(s.len() as f64)),
        _ => Err(bad_arg(lua, 0, "rawlen", "table or string expected")),
    }
}

fn setmetatable(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = check_table(lua, &args, 0, "setmetatable")?;
    let mt = match args.get(1) {
        Some(Value::Table(m)) => Some(m.clone()),
        None | Some(Value::Nil) => None,
        _ => return Err(bad_arg(lua, 1, "setmetatable", "nil or table expected")),
    };
    let protected = t
        .borrow()
        .meta
        .as_ref()
        .is_some_and(|m| !m.borrow().get_str("__metatable").is_nil());
    if protected {
        return Err(lua.rt_error("cannot change a protected metatable"));
    }
    readonly(lua, &t)?;
    t.borrow_mut().meta = mt;
    one(args[0].clone())
}

fn getmetatable(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let v = args.first().cloned().unwrap_or_default();
    let mt = match &v {
        // Strings share one, as in Luau: its __index the string library.
        Value::Str(_) => Some(
            lua.string_meta
                .get_or_insert_with(|| {
                    let mut t = Table::new();
                    if let Some(s) = &lua.string_lib {
                        t.set_str("__index", Value::Table(s.clone()));
                    }
                    t.set_readonly(true);
                    Rc::new(RefCell::new(t))
                })
                .clone(),
        ),
        other => other.metatable(),
    };
    let Some(mt) = mt else {
        return one(Value::Nil);
    };
    let guard = mt.borrow().get_str("__metatable");
    one(if guard.is_nil() { Value::Table(mt) } else { guard })
}

fn error(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let v = arg(&args, 0);
    let level = opt_int(lua, &args, 1, "error", 1)?;
    let v = match (&v, lua.position(level.max(0) as usize)) {
        (Value::Str(s), Some(at)) => Value::str(&format!("{at} {}", String::from_utf8_lossy(s))),
        _ => v,
    };
    Err(Throw::Error(v, lua.traceback()))
}

fn assert(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    if args.is_empty() {
        return Err(bad_arg(lua, 0, "assert", "value expected"));
    }
    if args[0].truthy() {
        return Ok(args);
    }
    match args.get(1) {
        Some(m) if !m.is_nil() => Err(Throw::Error(m.clone(), lua.traceback())),
        _ => Err(lua.rt_error("assertion failed!")),
    }
}

fn unpack(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = check_table(lua, &args, 0, "unpack")?;
    let n = t.borrow().len() as i64;
    let i = opt_int(lua, &args, 1, "unpack", 1)?;
    let j = opt_int(lua, &args, 2, "unpack", n)?;
    if i > j {
        return Ok(Vec::new());
    }
    if j - i >= 1_000_000 {
        return Err(lua.rt_error("too many results to unpack"));
    }
    lua.charge(((j - i) as usize) * 16)?;
    let t = t.borrow();
    Ok((i..=j).map(|k| t.get_int(k)).collect())
}

fn load(lua: &mut Lua, _: &mut dyn Host, me: &Native, args: Vec<Value>) -> Res {
    let src = check_str(lua, &args, 0, "load")?;
    let name = opt_str(lua, &args, 1, "load")?.map_or_else(
        || "load".to_owned(),
        |n| String::from_utf8_lossy(&n).trim_start_matches(['=', '@']).to_owned(),
    );
    // The chunk gets the globals of the code that loads it.
    let caller = lua.frame_info(1).map(|f| f.closure.env());
    let env = match (args.get(3), caller) {
        (Some(Value::Table(t)), _) => t.clone(),
        (_, Some(env)) => env,
        _ => match me.state.borrow().first() {
            Some(Value::Table(t)) => t.clone(),
            _ => return Err(lua.rt_error("load has no environment")),
        },
    };
    match lua.load_bytes(&src, &name, &env) {
        Ok(f) => one(f),
        Err(e) => Ok(vec![Value::Nil, Value::str(&e)]),
    }
}

fn dump(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let depth = opt_int(lua, &args, 1, "dump", 3)?.clamp(0, 8) as usize;
    one(Value::str(&inspect(&arg(&args, 0), depth)))
}

/// A value as the console shows it: strings quoted, tables expanded `depth`
/// levels deep (30 entries each), cycles marked.
pub fn inspect(v: &Value, depth: usize) -> String {
    let mut out = String::new();
    let mut seen = Vec::new();
    inspect_into(&mut out, v, depth, 0, &mut seen);
    out
}

fn inspect_into(out: &mut String, v: &Value, depth: usize, indent: usize, seen: &mut Vec<usize>) {
    match v {
        Value::Str(s) => out.push_str(&quote(s)),
        // A Roblox value (Vector3, CFrame, an EnumItem...): as tostring shows it.
        Value::Userdata(_) if super::hstudio_rbx_values::get(v).is_some() => {
            if let Some(d) = super::hstudio_rbx_values::get(v) {
                out.push_str(&super::hstudio_rbx_values::text(&d));
            }
        }
        Value::Table(t) => {
            let ptr = Rc::as_ptr(t) as *const u8 as usize;
            if seen.contains(&ptr) {
                out.push_str("<cycle>");
                return;
            }
            let entries = t.borrow().entries();
            if entries.is_empty() {
                out.push_str("{}");
                return;
            }
            if depth == 0 {
                out.push_str(&format!("{{...}} ({} entries)", entries.len()));
                return;
            }
            seen.push(ptr);
            let n = t.borrow().len();
            let inline = entries.len() <= 8 && entries.iter().all(|(_, v)| !matches!(v, Value::Table(_)));
            out.push('{');
            for (i, (k, val)) in entries.iter().enumerate() {
                if i == 30 {
                    if inline {
                        out.push_str(" ...");
                    } else {
                        out.push('\n');
                        out.push_str(&"  ".repeat(indent + 1));
                        out.push_str(&format!("... ({} more)", entries.len() - 30));
                    }
                    break;
                }
                if inline {
                    out.push_str(if i == 0 { " " } else { ", " });
                } else {
                    out.push('\n');
                    out.push_str(&"  ".repeat(indent + 1));
                }
                let positional = matches!(k, Value::Num(x) if *x == (i + 1) as f64 && i < n);
                if !positional {
                    match k {
                        Value::Str(s) if is_identifier(s) => out.push_str(&String::from_utf8_lossy(s)),
                        _ => {
                            out.push('[');
                            inspect_into(out, k, 0, indent + 1, seen);
                            out.push(']');
                        }
                    }
                    out.push_str(" = ");
                }
                inspect_into(out, val, depth - 1, indent + 1, seen);
                if !inline {
                    out.push(',');
                }
            }
            if inline {
                out.push_str(" }");
            } else {
                out.push('\n');
                out.push_str(&"  ".repeat(indent));
                out.push('}');
            }
            seen.pop();
        }
        other => out.push_str(&other.plain_text()),
    }
}

fn is_identifier(s: &[u8]) -> bool {
    !s.is_empty()
        && (s[0].is_ascii_alphabetic() || s[0] == b'_')
        && s.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_')
}

/// `%q`: a string as a Lua literal.
pub fn quote(s: &[u8]) -> String {
    let mut out = String::from("\"");
    for (i, &c) in s.iter().enumerate() {
        match c {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0 => {
                if s.get(i + 1).is_some_and(u8::is_ascii_digit) {
                    out.push_str("\\000");
                } else {
                    out.push_str("\\0");
                }
            }
            c if c < 32 || c == 127 => out.push_str(&format!("\\{c}")),
            _ => {}
        }
        if c >= 32 && c != b'"' && c != b'\\' && c != 127 {
            // Bytes of UTF-8 text are kept; they are joined back below.
            out.push(char::from(c));
        }
    }
    out.push('"');
    // Pushing bytes one by one as chars turned UTF-8 into Latin-1: redo
    // the text part from the bytes when they are valid UTF-8.
    if let Ok(text) = std::str::from_utf8(s)
        && !text.is_ascii()
    {
        let mut fixed = String::from("\"");
        for ch in text.chars() {
            match ch {
                '"' => fixed.push_str("\\\""),
                '\\' => fixed.push_str("\\\\"),
                '\n' => fixed.push_str("\\n"),
                '\r' => fixed.push_str("\\r"),
                '\t' => fixed.push_str("\\t"),
                c if (c as u32) < 32 || c as u32 == 127 => fixed.push_str(&format!("\\{}", c as u32)),
                c => fixed.push(c),
            }
        }
        fixed.push('"');
        return fixed;
    }
    out
}

// ----------------------------------------------------------------- string

fn string_lib() -> TableRef {
    lib(&[
        ("len", s_len),
        ("sub", s_sub),
        ("upper", s_upper),
        ("lower", s_lower),
        ("rep", s_rep),
        ("reverse", s_reverse),
        ("byte", s_byte),
        ("char", s_char),
        ("format", s_format),
        ("find", s_find),
        ("match", s_match),
        ("gmatch", s_gmatch),
        ("gsub", s_gsub),
        ("split", s_split),
        ("trim", s_trim),
        ("startswith", s_startswith),
        ("endswith", s_endswith),
        ("pack", s_pack),
        ("unpack", s_unpack),
        ("packsize", s_packsize),
    ])
}

fn s_len(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    one(Value::Num(check_str(lua, &args, 0, "len")?.len() as f64))
}

/// A string position: negative counts from the end.
fn posrelat(pos: i64, len: usize) -> i64 {
    if pos >= 0 {
        pos
    } else if pos.unsigned_abs() as usize > len {
        0
    } else {
        len as i64 + pos + 1
    }
}

fn s_sub(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let s = check_str(lua, &args, 0, "sub")?;
    let l = s.len();
    let start = posrelat(opt_int(lua, &args, 1, "sub", 1)?, l).max(1);
    let end = posrelat(opt_int(lua, &args, 2, "sub", -1)?, l).min(l as i64);
    if start > end {
        return one(Value::str(""));
    }
    one(Value::bytes(&s[start as usize - 1..end as usize]))
}

fn s_upper(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    one(Value::bytes(&check_str(lua, &args, 0, "upper")?.to_ascii_uppercase()))
}

fn s_lower(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    one(Value::bytes(&check_str(lua, &args, 0, "lower")?.to_ascii_lowercase()))
}

fn s_rep(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let s = check_str(lua, &args, 0, "rep")?;
    let n = check_int(lua, &args, 1, "rep")?;
    let sep = opt_str(lua, &args, 2, "rep")?.unwrap_or_else(|| Rc::from(&b""[..]));
    if n <= 0 {
        return one(Value::str(""));
    }
    let total = (s.len() + sep.len()).saturating_mul(n as usize);
    if total > MAX_STRING {
        return Err(Throw::Fatal("string.rep: resulting string is too large".into()));
    }
    lua.charge(total)?;
    let mut out = Vec::with_capacity(total);
    for i in 0..n {
        if i > 0 {
            out.extend_from_slice(&sep);
        }
        out.extend_from_slice(&s);
    }
    one(Value::Str(out.into()))
}

fn s_reverse(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let mut v = check_str(lua, &args, 0, "reverse")?.to_vec();
    v.reverse();
    one(Value::Str(v.into()))
}

fn s_byte(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let s = check_str(lua, &args, 0, "byte")?;
    let i = posrelat(opt_int(lua, &args, 1, "byte", 1)?, s.len()).max(1);
    let j = posrelat(opt_int(lua, &args, 2, "byte", i)?, s.len()).min(s.len() as i64);
    if i > j {
        return Ok(Vec::new());
    }
    Ok(s[i as usize - 1..j as usize]
        .iter()
        .map(|&b| Value::Num(f64::from(b)))
        .collect())
}

fn s_char(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let mut out = Vec::with_capacity(args.len());
    for i in 0..args.len() {
        let c = check_int(lua, &args, i, "char")?;
        out.push(u8::try_from(c).map_err(|_| bad_arg(lua, i, "char", "value out of range"))?);
    }
    one(Value::Str(out.into()))
}

fn s_trim(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let s = check_str(lua, &args, 0, "trim")?;
    one(Value::bytes(s.trim_ascii()))
}

fn s_startswith(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let s = check_str(lua, &args, 0, "startswith")?;
    let p = check_str(lua, &args, 1, "startswith")?;
    one(Value::Bool(s.starts_with(&p)))
}

fn s_endswith(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let s = check_str(lua, &args, 0, "endswith")?;
    let p = check_str(lua, &args, 1, "endswith")?;
    one(Value::Bool(s.ends_with(&p)))
}

/// `string.split(s, sep = " ", plain = true)`: the pieces between `sep`s;
/// with `plain` false, `sep` is a pattern. A blank `sep` with the default
/// splits on runs of whitespace and drops empty pieces.
fn s_split(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let s = check_str(lua, &args, 0, "split")?;
    let sep = opt_str(lua, &args, 1, "split")?;
    let plain = !matches!(args.get(2), Some(Value::Bool(false)));
    let mut out = Table::new();
    lua.charge(s.len())?;
    match sep {
        None => {
            for w in s.split(|c| c.is_ascii_whitespace()).filter(|w| !w.is_empty()) {
                out.push(Value::bytes(w));
            }
        }
        Some(sep) if sep.is_empty() => {
            for c in s.iter() {
                out.push(Value::bytes(&[*c]));
            }
        }
        Some(sep) if plain => {
            let mut start = 0;
            let mut i = 0;
            while i + sep.len() <= s.len() {
                if s[i..].starts_with(&sep) {
                    out.push(Value::bytes(&s[start..i]));
                    i += sep.len();
                    start = i;
                } else {
                    i += 1;
                }
            }
            out.push(Value::bytes(&s[start..]));
        }
        Some(sep) => {
            let mut start = 0;
            let mut pos = 0;
            while pos <= s.len() {
                let mut ms = MatchState::new(&s, &sep, pattern_steps(lua));
                let found = ms.do_match(pos, 0).map_err(|e| lua.rt_error(e))?;
                lua.steps = ms.steps;
                match found {
                    Some(e) if e > pos => {
                        out.push(Value::bytes(&s[start..pos]));
                        start = e;
                        pos = e;
                    }
                    _ => pos += 1,
                }
            }
            out.push(Value::bytes(&s[start..]));
        }
    }
    one(Value::table(out))
}

// string.format

fn s_format(lua: &mut Lua, h: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let fmt = check_str(lua, &args, 0, "format")?;
    let mut out: Vec<u8> = Vec::new();
    let mut argi = 1;
    let mut i = 0;
    while i < fmt.len() {
        let c = fmt[i];
        i += 1;
        if c != b'%' {
            out.push(c);
            continue;
        }
        if fmt.get(i) == Some(&b'%') {
            out.push(b'%');
            i += 1;
            continue;
        }
        let spec_start = i;
        while i < fmt.len() && b"-+ #0".contains(&fmt[i]) {
            i += 1;
        }
        let flags = &fmt[spec_start..i];
        let mut width = 0usize;
        while i < fmt.len() && fmt[i].is_ascii_digit() {
            width = width * 10 + usize::from(fmt[i] - b'0');
            i += 1;
        }
        let mut precision = None;
        if fmt.get(i) == Some(&b'.') {
            i += 1;
            let mut p = 0usize;
            while i < fmt.len() && fmt[i].is_ascii_digit() {
                p = p * 10 + usize::from(fmt[i] - b'0');
                i += 1;
            }
            precision = Some(p);
        }
        if width > 99 || precision.is_some_and(|p| p > 99) {
            return Err(lua.rt_error("invalid conversion (width or precision too long)"));
        }
        let Some(&conv) = fmt.get(i) else {
            return Err(lua.rt_error("invalid conversion '%' to 'format'"));
        };
        i += 1;
        let left = flags.contains(&b'-');
        let zero = flags.contains(&b'0') && !left;
        let plus = flags.contains(&b'+');
        let space = flags.contains(&b' ');
        let alt = flags.contains(&b'#');
        if argi >= args.len() && conv != b'%' {
            return Err(bad_arg(lua, argi, "format", "no value"));
        }
        let sign = |neg: bool| -> &'static str {
            if neg {
                "-"
            } else if plus {
                "+"
            } else if space {
                " "
            } else {
                ""
            }
        };
        let (body, numeric): (Vec<u8>, bool) = match conv {
            b'd' | b'i' => {
                let n = check_int(lua, &args, argi, "format")?;
                let mut digits = n.unsigned_abs().to_string();
                if let Some(p) = precision {
                    while digits.len() < p {
                        digits.insert(0, '0');
                    }
                }
                (format!("{}{digits}", sign(n < 0)).into_bytes(), true)
            }
            b'u' => (check_int(lua, &args, argi, "format")?.to_string().into_bytes(), true),
            b'c' => (vec![check_int(lua, &args, argi, "format")? as u8], false),
            b'x' | b'X' | b'o' => {
                let n = check_int(lua, &args, argi, "format")? as u64;
                let s = match conv {
                    b'x' => format!("{}{n:x}", if alt && n != 0 { "0x" } else { "" }),
                    b'X' => format!("{}{n:X}", if alt && n != 0 { "0X" } else { "" }),
                    _ => format!("{}{n:o}", if alt { "0" } else { "" }),
                };
                (s.into_bytes(), true)
            }
            b'f' | b'F' | b'e' | b'E' | b'g' | b'G' => {
                let n = check_num(lua, &args, argi, "format")?;
                let p = precision.unwrap_or(6);
                let mag = n.abs();
                let mut s = if !n.is_finite() {
                    if n.is_nan() { "nan".to_owned() } else { "inf".to_owned() }
                } else {
                    match conv {
                        b'f' | b'F' => format!("{mag:.p$}"),
                        b'e' | b'E' => c_exp(mag, p),
                        _ => fmt_g(mag, if p == 0 { 1 } else { p }, alt),
                    }
                };
                if conv.is_ascii_uppercase() {
                    s = s.to_uppercase();
                }
                (
                    format!("{}{s}", sign(n.is_sign_negative() && !n.is_nan())).into_bytes(),
                    n.is_finite(),
                )
            }
            b'a' | b'A' => return Err(lua.rt_error("'%a' is not supported by 'format'")),
            b'q' => (quote(&check_str(lua, &args, argi, "format")?).into_bytes(), false),
            b's' => {
                let s = lua.tostring(h, &args[argi])?;
                let mut b = s.into_bytes();
                if let Some(p) = precision {
                    b.truncate(p);
                }
                (b, false)
            }
            other => {
                return Err(lua.rt_error(format!("invalid conversion '%{}' to 'format'", char::from(other))));
            }
        };
        argi += 1;
        if body.len() >= width {
            out.extend_from_slice(&body);
        } else if left {
            out.extend_from_slice(&body);
            out.resize(out.len() + width - body.len(), b' ');
        } else if zero && numeric {
            let sign_len = usize::from(matches!(body.first(), Some(b'-' | b'+' | b' ')));
            out.extend_from_slice(&body[..sign_len]);
            out.resize(out.len() + width - body.len(), b'0');
            out.extend_from_slice(&body[sign_len..]);
        } else {
            out.resize(out.len() + width - body.len(), b' ');
            out.extend_from_slice(&body);
        }
        if out.len() > MAX_STRING {
            return Err(Throw::Fatal("string.format: result is too large".into()));
        }
    }
    one(Value::Str(out.into()))
}

/// C's `%.<p>e` of a non-negative number: `1.500000e+03`.
fn c_exp(n: f64, p: usize) -> String {
    let s = format!("{n:.p$e}");
    let (mant, exp) = s.split_once('e').unwrap_or((&s, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    format!("{mant}e{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs())
}

// Lua patterns, after lstrlib.c.

const CAP_UNFINISHED: isize = -1;
const CAP_POSITION: isize = -2;
const MAX_CAPTURES: usize = 32;
const MAX_MATCH_DEPTH: usize = 200;

/// The steps a pattern may take: the call's, or while its deadline lasts
/// (Roblox's script timeout) many more.
fn pattern_steps(lua: &Lua) -> u64 {
    if lua.deadline.is_some_and(|d| std::time::Instant::now() < d) {
        lua.steps.max(1_000_000_000)
    } else {
        lua.steps
    }
}

struct MatchState<'a> {
    src: &'a [u8],
    pat: &'a [u8],
    level: usize,
    capture: [(usize, isize); MAX_CAPTURES],
    depth: usize,
    steps: u64,
}

type MResult = Result<Option<usize>, String>;

impl<'a> MatchState<'a> {
    fn new(src: &'a [u8], pat: &'a [u8], steps: u64) -> MatchState<'a> {
        MatchState {
            src,
            pat,
            level: 0,
            capture: [(0, 0); MAX_CAPTURES],
            depth: 0,
            steps,
        }
    }
    fn class_end(&self, mut p: usize) -> Result<usize, String> {
        let c = self.pat[p];
        p += 1;
        if c == b'%' {
            if p >= self.pat.len() {
                return Err("malformed pattern (ends with '%')".into());
            }
            return Ok(p + 1);
        }
        if c == b'[' {
            if self.pat.get(p) == Some(&b'^') {
                p += 1;
            }
            loop {
                if p >= self.pat.len() {
                    return Err("malformed pattern (missing ']')".into());
                }
                let cc = self.pat[p];
                p += 1;
                if cc == b'%' && p < self.pat.len() {
                    p += 1;
                }
                if self.pat.get(p) == Some(&b']') {
                    break;
                }
            }
            return Ok(p + 1);
        }
        Ok(p)
    }
    fn single_match(&self, s: usize, p: usize, ep: usize) -> bool {
        let Some(&c) = self.src.get(s) else {
            return false;
        };
        match self.pat[p] {
            b'.' => true,
            b'%' => match_class(c, self.pat[p + 1]),
            b'[' => self.match_bracket(c, p, ep - 1),
            pc => pc == c,
        }
    }
    fn match_bracket(&self, c: u8, mut p: usize, ec: usize) -> bool {
        let mut sig = true;
        if self.pat[p + 1] == b'^' {
            sig = false;
            p += 1;
        }
        p += 1;
        while p < ec {
            if self.pat[p] == b'%' {
                p += 1;
                if match_class(c, self.pat[p]) {
                    return sig;
                }
                p += 1;
            } else if self.pat.get(p + 1) == Some(&b'-') && p + 2 < ec {
                if self.pat[p] <= c && c <= self.pat[p + 2] {
                    return sig;
                }
                p += 3;
            } else {
                if self.pat[p] == c {
                    return sig;
                }
                p += 1;
            }
        }
        !sig
    }
    fn do_match(&mut self, s: usize, p: usize) -> MResult {
        self.depth += 1;
        let r = self.do_match_inner(s, p);
        self.depth -= 1;
        r
    }
    fn do_match_inner(&mut self, mut s: usize, mut p: usize) -> MResult {
        if self.depth > MAX_MATCH_DEPTH {
            return Err("pattern too complex".into());
        }
        loop {
            if self.steps == 0 {
                return Err("pattern matching ran too long".into());
            }
            self.steps -= 1;
            if p == self.pat.len() {
                return Ok(Some(s));
            }
            match self.pat[p] {
                b'(' => {
                    return if self.pat.get(p + 1) == Some(&b')') {
                        self.start_capture(s, p + 2, CAP_POSITION)
                    } else {
                        self.start_capture(s, p + 1, CAP_UNFINISHED)
                    };
                }
                b')' => return self.end_capture(s, p + 1),
                b'$' if p + 1 == self.pat.len() => {
                    return Ok((s == self.src.len()).then_some(s));
                }
                b'%' if self.pat.get(p + 1) == Some(&b'b') => match self.match_balance(s, p + 2)? {
                    Some(e) => {
                        s = e;
                        p += 4;
                        continue;
                    }
                    None => return Ok(None),
                },
                b'%' if self.pat.get(p + 1) == Some(&b'f') => {
                    p += 2;
                    if self.pat.get(p) != Some(&b'[') {
                        return Err("missing '[' after '%f' in pattern".into());
                    }
                    let ep = self.class_end(p)?;
                    let prev = if s == 0 { 0 } else { self.src[s - 1] };
                    let cur = self.src.get(s).copied().unwrap_or(0);
                    if !self.match_bracket(prev, p, ep - 1) && self.match_bracket(cur, p, ep - 1) {
                        p = ep;
                        continue;
                    }
                    return Ok(None);
                }
                b'%' if self.pat.get(p + 1).is_some_and(u8::is_ascii_digit) => {
                    match self.match_capture(s, self.pat[p + 1])? {
                        Some(e) => {
                            s = e;
                            p += 2;
                            continue;
                        }
                        None => return Ok(None),
                    }
                }
                _ => {}
            }
            let ep = self.class_end(p)?;
            let m = self.single_match(s, p, ep);
            match self.pat.get(ep) {
                Some(b'?') => {
                    if m && let Some(r) = self.do_match(s + 1, ep + 1)? {
                        return Ok(Some(r));
                    }
                    p = ep + 1;
                }
                Some(b'+') => return if m { self.max_expand(s + 1, p, ep) } else { Ok(None) },
                Some(b'*') => return self.max_expand(s, p, ep),
                Some(b'-') => return self.min_expand(s, p, ep),
                _ => {
                    if !m {
                        return Ok(None);
                    }
                    s += 1;
                    p = ep;
                }
            }
        }
    }
    fn max_expand(&mut self, s: usize, p: usize, ep: usize) -> MResult {
        let mut i = 0;
        while self.single_match(s + i, p, ep) {
            i += 1;
        }
        loop {
            if let Some(r) = self.do_match(s + i, ep + 1)? {
                return Ok(Some(r));
            }
            if i == 0 {
                return Ok(None);
            }
            i -= 1;
        }
    }
    fn min_expand(&mut self, mut s: usize, p: usize, ep: usize) -> MResult {
        loop {
            if let Some(r) = self.do_match(s, ep + 1)? {
                return Ok(Some(r));
            }
            if self.single_match(s, p, ep) {
                s += 1;
            } else {
                return Ok(None);
            }
        }
    }
    fn start_capture(&mut self, s: usize, p: usize, what: isize) -> MResult {
        if self.level >= MAX_CAPTURES {
            return Err("too many captures".into());
        }
        self.capture[self.level] = (s, what);
        self.level += 1;
        let r = self.do_match(s, p)?;
        if r.is_none() {
            self.level -= 1;
        }
        Ok(r)
    }
    fn end_capture(&mut self, s: usize, p: usize) -> MResult {
        let l = (0..self.level)
            .rev()
            .find(|&i| self.capture[i].1 == CAP_UNFINISHED)
            .ok_or("invalid pattern capture")?;
        self.capture[l].1 = (s - self.capture[l].0) as isize;
        let r = self.do_match(s, p)?;
        if r.is_none() {
            self.capture[l].1 = CAP_UNFINISHED;
        }
        Ok(r)
    }
    fn match_balance(&self, s: usize, p: usize) -> MResult {
        if p + 1 >= self.pat.len() {
            return Err("missing arguments to '%b'".into());
        }
        if self.src.get(s) != Some(&self.pat[p]) {
            return Ok(None);
        }
        let (b, e) = (self.pat[p], self.pat[p + 1]);
        let mut cont = 1;
        let mut i = s + 1;
        while i < self.src.len() {
            let c = self.src[i];
            if c == e {
                cont -= 1;
                if cont == 0 {
                    return Ok(Some(i + 1));
                }
            } else if c == b {
                cont += 1;
            }
            i += 1;
        }
        Ok(None)
    }
    fn match_capture(&self, s: usize, digit: u8) -> MResult {
        let l = usize::from(digit - b'1');
        if digit < b'1' || l >= self.level || self.capture[l].1 == CAP_UNFINISHED {
            return Err(format!("invalid capture index %{}", char::from(digit)));
        }
        let (start, len) = self.capture[l];
        let len = len.max(0) as usize;
        let cap = &self.src[start..start + len];
        Ok(self.src[s..].starts_with(cap).then_some(s + len))
    }
    fn get_capture(&self, i: usize, s: usize, e: usize) -> Result<Value, String> {
        if i >= self.level {
            if i == 0 {
                return Ok(Value::bytes(&self.src[s..e]));
            }
            return Err(format!("invalid capture index %{}", i + 1));
        }
        let (start, len) = self.capture[i];
        match len {
            CAP_UNFINISHED => Err("unfinished capture".into()),
            CAP_POSITION => Ok(Value::Num(start as f64 + 1.0)),
            len => Ok(Value::bytes(&self.src[start..start + len as usize])),
        }
    }
    fn captures(&self, s: usize, e: usize, whole: bool) -> Result<Vec<Value>, String> {
        let n = if self.level == 0 && whole { 1 } else { self.level };
        (0..n).map(|i| self.get_capture(i, s, e)).collect()
    }
}

fn match_class(c: u8, cl: u8) -> bool {
    let res = match cl.to_ascii_lowercase() {
        b'a' => c.is_ascii_alphabetic(),
        b'c' => c.is_ascii_control(),
        b'd' => c.is_ascii_digit(),
        b'g' => c.is_ascii_graphic(),
        b'l' => c.is_ascii_lowercase(),
        b'p' => c.is_ascii_punctuation(),
        b's' => c.is_ascii_whitespace() || c == 11,
        b'u' => c.is_ascii_uppercase(),
        b'w' => c.is_ascii_alphanumeric(),
        b'x' => c.is_ascii_hexdigit(),
        _ => return cl == c,
    };
    if cl.is_ascii_uppercase() { !res } else { res }
}

fn has_specials(p: &[u8]) -> bool {
    p.iter().any(|c| b"^$*+?.([%-".contains(c))
}

fn find_aux(lua: &mut Lua, args: &[Value], find: bool) -> Res {
    let fname = if find { "find" } else { "match" };
    let s = check_str(lua, args, 0, fname)?;
    let p = check_str(lua, args, 1, fname)?;
    let init = posrelat(opt_int(lua, args, 2, fname, 1)?, s.len()).max(1) as usize;
    if init > s.len() + 1 {
        return one(Value::Nil);
    }
    let plain = args.get(3).is_some_and(Value::truthy);
    if find && (plain || !has_specials(&p)) {
        lua.charge(s.len())?;
        let hay = &s[init - 1..];
        let pos = if p.is_empty() {
            Some(0)
        } else {
            hay.windows(p.len()).position(|w| w == &p[..])
        };
        return Ok(match pos {
            Some(i) => vec![
                Value::Num((init + i) as f64),
                Value::Num((init + i + p.len() - 1) as f64),
            ],
            None => vec![Value::Nil],
        });
    }
    let (anchor, pstart) = if p.first() == Some(&b'^') {
        (true, 1)
    } else {
        (false, 0)
    };
    let mut ms = MatchState::new(&s, &p, pattern_steps(lua));
    let mut s1 = init - 1;
    let result = loop {
        ms.level = 0;
        match ms.do_match(s1, pstart) {
            Err(e) => {
                lua.steps = ms.steps;
                return Err(lua.rt_error(e));
            }
            Ok(Some(e)) => {
                let caps = ms.captures(s1, e, !find).map_err(|m| lua.rt_error(m));
                break Some((s1, e, caps));
            }
            Ok(None) => {}
        }
        s1 += 1;
        if anchor || s1 > s.len() {
            break None;
        }
    };
    lua.steps = ms.steps;
    match result {
        None => one(Value::Nil),
        Some((start, end, caps)) => {
            let caps = caps?;
            if find {
                let mut r = vec![Value::Num(start as f64 + 1.0), Value::Num(end as f64)];
                r.extend(caps);
                Ok(r)
            } else {
                Ok(caps)
            }
        }
    }
}

fn s_find(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    find_aux(lua, &args, true)
}

fn s_match(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    find_aux(lua, &args, false)
}

fn gmatch_step(lua: &mut Lua, _: &mut dyn Host, me: &Native, _: Vec<Value>) -> Res {
    let (s, p, mut pos) = {
        let st = me.state.borrow();
        let (Value::Str(s), Value::Str(p), Value::Num(pos)) = (&st[0], &st[1], &st[2]) else {
            return one(Value::Nil);
        };
        (s.clone(), p.clone(), *pos as usize)
    };
    let mut ms = MatchState::new(&s, &p, pattern_steps(lua));
    while pos <= s.len() {
        ms.level = 0;
        let r = ms.do_match(pos, 0);
        match r {
            Err(e) => {
                lua.steps = ms.steps;
                return Err(lua.rt_error(e));
            }
            Ok(Some(e)) => {
                let caps = ms.captures(pos, e, true);
                lua.steps = ms.steps;
                let next = if e == pos { e + 1 } else { e };
                me.state.borrow_mut()[2] = Value::Num(next as f64);
                return caps.map_err(|m| lua.rt_error(m));
            }
            Ok(None) => pos += 1,
        }
    }
    lua.steps = ms.steps;
    me.state.borrow_mut()[2] = Value::Num((s.len() + 1) as f64);
    one(Value::Nil)
}

fn s_gmatch(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let s = check_str(lua, &args, 0, "gmatch")?;
    let p = check_str(lua, &args, 1, "gmatch")?;
    one(Native::with_state(
        "gmatch_step",
        gmatch_step,
        vec![Value::Str(s), Value::Str(p), Value::Num(0.0)],
    ))
}

fn s_gsub(lua: &mut Lua, h: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let s = check_str(lua, &args, 0, "gsub")?;
    let p = check_str(lua, &args, 1, "gsub")?;
    let repl = arg(&args, 2);
    if !matches!(
        repl,
        Value::Str(_) | Value::Num(_) | Value::Table(_) | Value::Func(_) | Value::Native(_)
    ) {
        return Err(bad_arg(lua, 2, "gsub", "string/function/table expected"));
    }
    let max_n = opt_int(lua, &args, 3, "gsub", i64::MAX)?;
    let (anchor, pstart) = if p.first() == Some(&b'^') {
        (true, 1)
    } else {
        (false, 0)
    };
    let mut out: Vec<u8> = Vec::new();
    let mut src = 0usize;
    let mut n = 0i64;
    while n < max_n {
        let mut ms = MatchState::new(&s, &p, pattern_steps(lua));
        let r = ms.do_match(src, pstart);
        lua.steps = ms.steps;
        let e = r.map_err(|m| lua.rt_error(m))?;
        if let Some(e) = e {
            n += 1;
            // The replacement for the match s[src..e].
            let whole = &s[src..e];
            let value = match &repl {
                Value::Str(_) | Value::Num(_) => {
                    let r = match &repl {
                        Value::Str(r) => r.to_vec(),
                        Value::Num(x) => fmt_num(*x).into_bytes(),
                        _ => Vec::new(),
                    };
                    let mut piece = Vec::new();
                    let mut i = 0;
                    while i < r.len() {
                        if r[i] == b'%' {
                            i += 1;
                            match r.get(i) {
                                Some(b'0') => piece.extend_from_slice(whole),
                                Some(d) if d.is_ascii_digit() => {
                                    let v = ms
                                        .get_capture(usize::from(d - b'1'), src, e)
                                        .map_err(|m| lua.rt_error(m))?;
                                    match v {
                                        Value::Str(c) => piece.extend_from_slice(&c),
                                        other => piece.extend_from_slice(other.plain_text().as_bytes()),
                                    }
                                }
                                Some(b'%') => piece.push(b'%'),
                                _ => return Err(lua.rt_error("invalid use of '%' in replacement string")),
                            }
                        } else {
                            piece.push(r[i]);
                        }
                        i += 1;
                    }
                    Value::Str(piece.into())
                }
                Value::Table(t) => {
                    let key = ms.get_capture(0, src, e).map_err(|m| lua.rt_error(m))?;
                    let t = Value::Table(t.clone());
                    lua.index(h, &t, &key)?
                }
                f => {
                    let caps = ms.captures(src, e, true).map_err(|m| lua.rt_error(m))?;
                    lua.call1(h, f, caps)?
                }
            };
            match value {
                Value::Nil | Value::Bool(false) => out.extend_from_slice(whole),
                Value::Str(v) => out.extend_from_slice(&v),
                Value::Num(x) => out.extend_from_slice(fmt_num(x).as_bytes()),
                other => {
                    return Err(lua.rt_error(format!("invalid replacement value (a {})", other.type_name())));
                }
            }
            if e > src {
                src = e;
            } else {
                if src < s.len() {
                    out.push(s[src]);
                }
                src += 1;
            }
        } else if src < s.len() {
            out.push(s[src]);
            src += 1;
        } else {
            break;
        }
        if out.len() > MAX_STRING {
            return Err(Throw::Fatal("string.gsub: result is too large".into()));
        }
        if src > s.len() || anchor {
            break;
        }
    }
    if src < s.len() {
        out.extend_from_slice(&s[src..]);
    }
    Ok(vec![Value::Str(out.into()), Value::Num(n as f64)])
}

// ----------------------------------------------------------------- math

fn math_lib() -> TableRef {
    let t = lib(&[
        ("abs", m_abs),
        ("ceil", m_ceil),
        ("floor", m_floor),
        ("sqrt", m_sqrt),
        ("sin", m_sin),
        ("cos", m_cos),
        ("tan", m_tan),
        ("asin", m_asin),
        ("acos", m_acos),
        ("atan", m_atan),
        ("exp", m_exp),
        ("log", m_log),
        ("fmod", m_fmod),
        ("modf", m_modf),
        ("max", m_max),
        ("min", m_min),
        ("random", m_random),
        ("randomseed", m_randomseed),
        ("tointeger", m_tointeger),
        ("type", m_type),
        ("clamp", m_clamp),
        // Luau's.
        ("sign", m_sign),
        ("round", m_round),
        ("noise", m_noise),
        ("lerp", m_lerp),
        ("map", m_map),
        ("isnan", m_isnan),
        ("isinf", m_isinf),
        ("isfinite", m_isfinite),
        ("rad", m_rad),
        ("deg", m_deg),
        ("ult", m_ult),
        // Lua 5.1 and 5.2's, which code still uses.
        ("atan2", m_atan),
        ("pow", m_pow),
        ("log10", m_log10),
        ("sinh", m_sinh),
        ("cosh", m_cosh),
        ("tanh", m_tanh),
        ("frexp", m_frexp),
        ("ldexp", m_ldexp),
    ]);
    {
        let mut m = t.borrow_mut();
        m.set_str("pi", Value::Num(std::f64::consts::PI));
        m.set_str("huge", Value::Num(f64::INFINITY));
        m.set_str("maxinteger", Value::Num(9_007_199_254_740_991.0));
        m.set_str("mininteger", Value::Num(-9_007_199_254_740_991.0));
    }
    t
}

macro_rules! math1 {
    ($name:ident, $lua_name:literal, $f:expr) => {
        fn $name(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
            let x = check_num(lua, &args, 0, $lua_name)?;
            let f: fn(f64) -> f64 = $f;
            one(Value::Num(f(x)))
        }
    };
}

math1!(m_abs, "abs", f64::abs);
math1!(m_ceil, "ceil", f64::ceil);
math1!(m_floor, "floor", f64::floor);
math1!(m_sqrt, "sqrt", f64::sqrt);
math1!(m_sin, "sin", f64::sin);
math1!(m_cos, "cos", f64::cos);
math1!(m_tan, "tan", f64::tan);
math1!(m_asin, "asin", f64::asin);
math1!(m_acos, "acos", f64::acos);
math1!(m_exp, "exp", f64::exp);
math1!(m_rad, "rad", f64::to_radians);
math1!(m_deg, "deg", f64::to_degrees);
math1!(m_log10, "log10", f64::log10);
math1!(m_sinh, "sinh", f64::sinh);
math1!(m_cosh, "cosh", f64::cosh);
math1!(m_tanh, "tanh", f64::tanh);

fn m_pow(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let x = check_num(lua, &args, 0, "pow")?;
    let y = check_num(lua, &args, 1, "pow")?;
    one(Value::Num(x.powf(y)))
}

/// `math.ult(m, n)`: whether m < n as unsigned 64-bit integers.
fn m_ult(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let m = check_int(lua, &args, 0, "ult")?;
    let n = check_int(lua, &args, 1, "ult")?;
    one(Value::Bool((m as u64) < (n as u64)))
}

/// `math.frexp(x)`: m, e with x = m * 2^e and 0.5 <= |m| < 1.
fn m_frexp(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let x = check_num(lua, &args, 0, "frexp")?;
    if x == 0.0 || !x.is_finite() {
        return Ok(vec![Value::Num(x), Value::Num(0.0)]);
    }
    let e = x.abs().log2().floor() + 1.0;
    let mut m = x / 2f64.powf(e);
    let mut e = e;
    // Rounding at the edges.
    if m.abs() >= 1.0 {
        m /= 2.0;
        e += 1.0;
    } else if m.abs() < 0.5 {
        m *= 2.0;
        e -= 1.0;
    }
    Ok(vec![Value::Num(m), Value::Num(e)])
}

fn m_ldexp(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let m = check_num(lua, &args, 0, "ldexp")?;
    let e = check_num(lua, &args, 1, "ldexp")?;
    one(Value::Num(m * 2f64.powf(e)))
}

fn m_atan(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let y = check_num(lua, &args, 0, "atan")?;
    let x = opt_num(lua, &args, 1, "atan", 1.0)?;
    one(Value::Num(y.atan2(x)))
}

fn m_log(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let x = check_num(lua, &args, 0, "log")?;
    one(Value::Num(match args.get(1) {
        None | Some(Value::Nil) => x.ln(),
        _ => {
            let b = check_num(lua, &args, 1, "log")?;
            if b == 2.0 {
                x.log2()
            } else if b == 10.0 {
                x.log10()
            } else {
                x.ln() / b.ln()
            }
        }
    }))
}

fn m_fmod(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let a = check_num(lua, &args, 0, "fmod")?;
    let b = check_num(lua, &args, 1, "fmod")?;
    one(Value::Num(a % b))
}

fn m_modf(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let x = check_num(lua, &args, 0, "modf")?;
    let i = x.trunc();
    let frac = if x.is_infinite() { 0.0 } else { x - i };
    Ok(vec![Value::Num(i), Value::Num(frac)])
}

fn m_max(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let mut best = check_num(lua, &args, 0, "max")?;
    for i in 1..args.len() {
        let v = check_num(lua, &args, i, "max")?;
        if v > best {
            best = v;
        }
    }
    one(Value::Num(best))
}

fn m_min(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let mut best = check_num(lua, &args, 0, "min")?;
    for i in 1..args.len() {
        let v = check_num(lua, &args, i, "min")?;
        if v < best {
            best = v;
        }
    }
    one(Value::Num(best))
}

fn m_clamp(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let x = check_num(lua, &args, 0, "clamp")?;
    let lo = check_num(lua, &args, 1, "clamp")?;
    let hi = check_num(lua, &args, 2, "clamp")?;
    one(Value::Num(x.max(lo).min(hi)))
}

fn m_sign(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let x = check_num(lua, &args, 0, "sign")?;
    one(Value::Num(if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }))
}

/// Rounds half away from zero, as Luau does.
fn m_round(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    one(Value::Num(check_num(lua, &args, 0, "round")?.round()))
}

fn m_lerp(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let a = check_num(lua, &args, 0, "lerp")?;
    let b = check_num(lua, &args, 1, "lerp")?;
    let t = check_num(lua, &args, 2, "lerp")?;
    one(Value::Num(if t == 1.0 { b } else { a + (b - a) * t }))
}

/// `math.map(x, inmin, inmax, outmin, outmax)`.
fn m_map(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let x = check_num(lua, &args, 0, "map")?;
    let a = check_num(lua, &args, 1, "map")?;
    let b = check_num(lua, &args, 2, "map")?;
    let c = check_num(lua, &args, 3, "map")?;
    let d = check_num(lua, &args, 4, "map")?;
    one(Value::Num(c + (x - a) * (d - c) / (b - a)))
}

fn m_isnan(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    one(Value::Bool(check_num(lua, &args, 0, "isnan")?.is_nan()))
}

fn m_isinf(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    one(Value::Bool(check_num(lua, &args, 0, "isinf")?.is_infinite()))
}

fn m_isfinite(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    one(Value::Bool(check_num(lua, &args, 0, "isfinite")?.is_finite()))
}

/// Luau's `math.noise(x[, y[, z]])`: Perlin's improved noise, 0 at whole
/// numbers, about -1 to 1.
fn m_noise(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let x = check_num(lua, &args, 0, "noise")?;
    let y = opt_num(lua, &args, 1, "noise", 0.0)?;
    let z = opt_num(lua, &args, 2, "noise", 0.0)?;
    one(Value::Num(perlin(x, y, z)))
}

/// Ken Perlin's permutation.
const PERM: [u8; 256] = [
    151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225, 140, 36, 103, 30, 69, 142, 8, 99, 37, 240,
    21, 10, 23, 190, 6, 148, 247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219, 203, 117, 35, 11, 32, 57, 177, 33, 88,
    237, 149, 56, 87, 174, 20, 125, 136, 171, 168, 68, 175, 74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83,
    111, 229, 122, 60, 211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54, 65, 25, 63, 161, 1, 216,
    80, 73, 209, 76, 132, 187, 208, 89, 18, 169, 200, 196, 135, 130, 116, 188, 159, 86, 164, 100, 109, 198, 173, 186,
    3, 64, 52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126, 255, 82, 85, 212, 207, 206, 59, 227, 47, 16, 58, 17,
    182, 189, 28, 42, 223, 183, 170, 213, 119, 248, 152, 2, 44, 154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9, 129,
    22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104, 218, 246, 97, 228, 251, 34, 242, 193, 238,
    210, 144, 12, 191, 179, 162, 241, 81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31, 181, 199, 106, 157, 184,
    84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93, 222, 114, 67, 29, 24, 72, 243, 141, 128, 195,
    78, 66, 215, 61, 156, 180,
];

fn perlin(x: f64, y: f64, z: f64) -> f64 {
    if !(x.is_finite() && y.is_finite() && z.is_finite()) {
        return 0.0;
    }
    let p = |i: i64| i64::from(PERM[(i & 255) as usize]);
    let fade = |t: f64| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let lerp = |t: f64, a: f64, b: f64| a + t * (b - a);
    let grad = |hash: i64, x: f64, y: f64, z: f64| {
        let h = hash & 15;
        let u = if h < 8 { x } else { y };
        let v = if h < 4 {
            y
        } else if h == 12 || h == 14 {
            x
        } else {
            z
        };
        (if h & 1 == 0 { u } else { -u }) + (if h & 2 == 0 { v } else { -v })
    };
    let (fx, fy, fz) = (x.floor(), y.floor(), z.floor());
    let (xi, yi, zi) = (fx as i64, fy as i64, fz as i64);
    let (x, y, z) = (x - fx, y - fy, z - fz);
    let (u, v, w) = (fade(x), fade(y), fade(z));
    let a = p(xi) + yi;
    let (aa, ab) = (p(a) + zi, p(a + 1) + zi);
    let b = p(xi + 1) + yi;
    let (ba, bb) = (p(b) + zi, p(b + 1) + zi);
    lerp(
        w,
        lerp(
            v,
            lerp(u, grad(p(aa), x, y, z), grad(p(ba), x - 1.0, y, z)),
            lerp(u, grad(p(ab), x, y - 1.0, z), grad(p(bb), x - 1.0, y - 1.0, z)),
        ),
        lerp(
            v,
            lerp(u, grad(p(aa + 1), x, y, z - 1.0), grad(p(ba + 1), x - 1.0, y, z - 1.0)),
            lerp(
                u,
                grad(p(ab + 1), x, y - 1.0, z - 1.0),
                grad(p(bb + 1), x - 1.0, y - 1.0, z - 1.0),
            ),
        ),
    )
}

/// xorshift64*: the next draw in [0, 1).
pub fn random01(lua: &mut Lua) -> f64 {
    let mut x = lua.rng;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    lua.rng = x;
    (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
}

fn m_random(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let r = random01(lua);
    let (lo, hi) = match args.len() {
        0 => return one(Value::Num(r)),
        1 => (1, check_int(lua, &args, 0, "random")?),
        _ => (check_int(lua, &args, 0, "random")?, check_int(lua, &args, 1, "random")?),
    };
    if lo > hi {
        return Err(bad_arg(lua, args.len() - 1, "random", "interval is empty"));
    }
    let span = (hi - lo) as f64 + 1.0;
    one(Value::Num(lo as f64 + (r * span).floor()))
}

fn m_randomseed(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let seed = opt_num(lua, &args, 0, "randomseed", 0.0)?;
    lua.rng = (seed.to_bits() ^ 0x9E37_79B9_7F4A_7C15) | 1;
    for _ in 0..16 {
        random01(lua);
    }
    Ok(Vec::new())
}

fn m_tointeger(_: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    one(match args.first() {
        Some(Value::Num(n)) if n.fract() == 0.0 => Value::Num(*n),
        _ => Value::Nil,
    })
}

fn m_type(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    if args.is_empty() {
        return Err(bad_arg(lua, 0, "type", "value expected"));
    }
    one(match &args[0] {
        Value::Num(n) if n.fract() == 0.0 => Value::str("integer"),
        Value::Num(_) => Value::str("float"),
        _ => Value::Nil,
    })
}

// ----------------------------------------------------------------- table

fn table_lib() -> TableRef {
    lib(&[
        ("insert", t_insert),
        ("remove", t_remove),
        ("concat", t_concat),
        ("sort", t_sort),
        ("unpack", unpack),
        ("pack", t_pack),
        ("move", t_move),
        // Luau's.
        ("find", t_find),
        ("create", t_create),
        ("clear", t_clear),
        ("clone", t_clone),
        ("freeze", t_freeze),
        ("isfrozen", t_isfrozen),
        ("maxn", t_maxn),
        ("getn", t_getn),
        ("foreach", t_foreach),
        ("foreachi", t_foreachi),
    ])
}

/// `table.find(t, value[, init])`: the first index from `init` holding it.
fn t_find(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = check_table(lua, &args, 0, "find")?;
    let want = arg(&args, 1);
    let init = opt_int(lua, &args, 2, "find", 1)?;
    if init < 1 {
        return Err(bad_arg(lua, 2, "find", "index out of range"));
    }
    let t = t.borrow();
    let mut i = init;
    loop {
        let v = t.get_int(i);
        if v.is_nil() {
            return one(Value::Nil);
        }
        if v.raw_eq(&want) {
            return one(Value::Num(i as f64));
        }
        i += 1;
        lua.charge(1)?;
    }
}

/// `table.create(n[, value])`: a list of `n` copies.
fn t_create(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let n = check_int(lua, &args, 0, "create")?;
    if !(0..=super::hstudio_lua::MAX_TABLE as i64).contains(&n) {
        return Err(bad_arg(lua, 0, "create", "size out of range"));
    }
    lua.charge(n as usize)?;
    let v = arg(&args, 1);
    let mut t = Table::with_capacity(n as usize, 0);
    if !v.is_nil() {
        t.set_list(vec![v; n as usize]);
    }
    one(Value::table(t))
}

fn t_clear(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = check_table(lua, &args, 0, "clear")?;
    readonly(lua, &t)?;
    let mut t = t.borrow_mut();
    let meta = t.meta.take();
    *t = Table::new();
    t.meta = meta;
    Ok(Vec::new())
}

/// A shallow copy with the same metatable, not frozen.
fn t_clone(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = check_table(lua, &args, 0, "clone")?;
    let src = t.borrow();
    lua.charge(src.len())?;
    let mut out = Table::new();
    for (k, v) in src.entries() {
        out.set(k, v).map_err(|m| lua.rt_error(m))?;
    }
    out.meta = src.meta.clone();
    one(Value::table(out))
}

fn t_freeze(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = check_table(lua, &args, 0, "freeze")?;
    t.borrow_mut().set_readonly(true);
    one(Value::Table(t))
}

fn t_isfrozen(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = check_table(lua, &args, 0, "isfrozen")?;
    one(Value::Bool(t.borrow().is_readonly()))
}

fn t_maxn(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = check_table(lua, &args, 0, "maxn")?;
    let max = t
        .borrow()
        .entries()
        .iter()
        .filter_map(|(k, _)| k.as_num())
        .fold(0.0, f64::max);
    one(Value::Num(max))
}

fn t_getn(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = check_table(lua, &args, 0, "getn")?;
    one(Value::Num(t.borrow().len() as f64))
}

/// `table.foreach(t, f)`: f(k, v) for each pair until one returns non-nil.
fn t_foreach(lua: &mut Lua, h: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = check_table(lua, &args, 0, "foreach")?;
    let f = check_func(lua, &args, 1, "foreach")?;
    let entries = t.borrow().entries();
    for (k, v) in entries {
        let r = lua.call1(h, &f, vec![k, v])?;
        if !r.is_nil() {
            return one(r);
        }
    }
    Ok(Vec::new())
}

fn t_foreachi(lua: &mut Lua, h: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = check_table(lua, &args, 0, "foreachi")?;
    let f = check_func(lua, &args, 1, "foreachi")?;
    let items = t.borrow().list();
    for (i, v) in items.into_iter().enumerate() {
        let r = lua.call1(h, &f, vec![Value::Num(i as f64 + 1.0), v])?;
        if !r.is_nil() {
            return one(r);
        }
    }
    Ok(Vec::new())
}

// ----------------------------------------------------------------- bit32

fn bit32_lib() -> TableRef {
    lib(&[
        ("band", b_band),
        ("bor", b_bor),
        ("bxor", b_bxor),
        ("btest", b_btest),
        ("bnot", b_bnot),
        ("lshift", b_lshift),
        ("rshift", b_rshift),
        ("arshift", b_arshift),
        ("lrotate", b_lrotate),
        ("rrotate", b_rrotate),
        ("extract", b_extract),
        ("replace", b_replace),
        ("countlz", b_countlz),
        ("countrz", b_countrz),
        ("byteswap", b_byteswap),
    ])
}

/// A number as bit32 takes it: modulo 2^32.
fn u32_arg(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<u32, Throw> {
    let n = check_num(lua, args, i, fname)?;
    if !n.is_finite() {
        return Ok(0);
    }
    Ok(n.trunc().rem_euclid(4_294_967_296.0) as u32)
}

fn u32_res(n: u32) -> Res {
    one(Value::Num(f64::from(n)))
}

fn fold_bits(lua: &Lua, args: &[Value], fname: &str, start: u32, f: fn(u32, u32) -> u32) -> Result<u32, Throw> {
    let mut acc = start;
    for i in 0..args.len() {
        acc = f(acc, u32_arg(lua, args, i, fname)?);
    }
    Ok(acc)
}

fn b_band(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    u32_res(fold_bits(lua, &args, "band", u32::MAX, |a, b| a & b)?)
}

fn b_bor(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    u32_res(fold_bits(lua, &args, "bor", 0, |a, b| a | b)?)
}

fn b_bxor(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    u32_res(fold_bits(lua, &args, "bxor", 0, |a, b| a ^ b)?)
}

fn b_btest(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    one(Value::Bool(
        fold_bits(lua, &args, "btest", u32::MAX, |a, b| a & b)? != 0,
    ))
}

fn b_bnot(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    u32_res(!u32_arg(lua, &args, 0, "bnot")?)
}

/// The value and a shift count, which may be negative (the other way).
fn shift_args(lua: &Lua, args: &[Value], fname: &str) -> Result<(u32, i64), Throw> {
    Ok((u32_arg(lua, args, 0, fname)?, check_int(lua, args, 1, fname)?))
}

fn shl(x: u32, n: i64) -> u32 {
    match n {
        n if n <= -32 || n >= 32 => 0,
        n if n >= 0 => x << n,
        n => x >> -n,
    }
}

fn b_lshift(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let (x, n) = shift_args(lua, &args, "lshift")?;
    u32_res(shl(x, n))
}

fn b_rshift(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let (x, n) = shift_args(lua, &args, "rshift")?;
    u32_res(shl(x, -n))
}

fn b_arshift(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let (x, n) = shift_args(lua, &args, "arshift")?;
    if n < 0 {
        return u32_res(shl(x, -n));
    }
    u32_res(((x as i32) >> n.min(31)) as u32)
}

fn b_lrotate(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let (x, n) = shift_args(lua, &args, "lrotate")?;
    u32_res(x.rotate_left(n.rem_euclid(32) as u32))
}

fn b_rrotate(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let (x, n) = shift_args(lua, &args, "rrotate")?;
    u32_res(x.rotate_right(n.rem_euclid(32) as u32))
}

/// The field and width bit32.extract and replace take, checked.
fn field_args(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<(u32, u32), Throw> {
    let f = check_int(lua, args, i, fname)?;
    let w = opt_int(lua, args, i + 1, fname, 1)?;
    if f < 0 {
        return Err(bad_arg(lua, i, fname, "field cannot be negative"));
    }
    if w <= 0 {
        return Err(bad_arg(lua, i + 1, fname, "width must be positive"));
    }
    if f + w > 32 {
        return Err(lua.rt_error("trying to access non-existent bits"));
    }
    Ok((f as u32, w as u32))
}

fn mask(w: u32) -> u32 {
    if w >= 32 { u32::MAX } else { (1 << w) - 1 }
}

fn b_extract(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let x = u32_arg(lua, &args, 0, "extract")?;
    let (f, w) = field_args(lua, &args, 1, "extract")?;
    u32_res((x >> f) & mask(w))
}

fn b_replace(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let x = u32_arg(lua, &args, 0, "replace")?;
    let v = u32_arg(lua, &args, 1, "replace")?;
    let (f, w) = field_args(lua, &args, 2, "replace")?;
    let m = mask(w) << f;
    u32_res((x & !m) | ((v << f) & m))
}

fn b_countlz(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    u32_res(u32_arg(lua, &args, 0, "countlz")?.leading_zeros())
}

fn b_countrz(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    u32_res(u32_arg(lua, &args, 0, "countrz")?.trailing_zeros())
}

fn b_byteswap(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    u32_res(u32_arg(lua, &args, 0, "byteswap")?.swap_bytes())
}

/// `table.move(a1, f, e, t[, a2])`: a1[f..e] into a2 from t (a2 is a1 by
/// default), overlapping ranges kept right; returns a2.
fn t_move(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let a1 = check_table(lua, &args, 0, "move")?;
    let f = check_int(lua, &args, 1, "move")?;
    let e = check_int(lua, &args, 2, "move")?;
    let t = check_int(lua, &args, 3, "move")?;
    let a2 = match args.get(4) {
        None | Some(Value::Nil) => a1.clone(),
        Some(_) => check_table(lua, &args, 4, "move")?,
    };
    if e >= f {
        if e - f >= 1 << 24 {
            return Err(bad_arg(lua, 2, "move", "too many elements to move"));
        }
        lua.charge((e - f + 1) as usize)?;
        let values: Vec<Value> = {
            let src = a1.borrow();
            (f..=e).map(|i| src.get_int(i)).collect()
        };
        let mut dst = a2.borrow_mut();
        for (k, v) in values.into_iter().enumerate() {
            dst.set(Value::Num((t + k as i64) as f64), v)
                .map_err(|m| lua.rt_error(m))?;
        }
    }
    one(Value::Table(a2))
}

/// Writes to a frozen table fail.
fn readonly(lua: &Lua, t: &TableRef) -> Result<(), Throw> {
    if t.borrow().is_readonly() {
        return Err(lua.rt_error("attempt to modify a readonly table"));
    }
    Ok(())
}

fn t_insert(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = check_table(lua, &args, 0, "insert")?;
    let n = t.borrow().len();
    let r = match args.len() {
        2 => t.borrow_mut().insert(n + 1, args[1].clone()),
        3 => {
            let pos = check_int(lua, &args, 1, "insert")?;
            if pos < 1 || pos as usize > n + 1 {
                return Err(bad_arg(lua, 1, "insert", "position out of bounds"));
            }
            lua.charge(n * 2)?;
            t.borrow_mut().insert(pos as usize, args[2].clone())
        }
        _ => return Err(lua.rt_error("wrong number of arguments to 'insert'")),
    };
    r.map_err(|e| match e {
        "table is too large" => Throw::Fatal(e.into()),
        e => lua.rt_error(e),
    })?;
    Ok(Vec::new())
}

fn t_remove(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = check_table(lua, &args, 0, "remove")?;
    readonly(lua, &t)?;
    let n = t.borrow().len();
    let pos = opt_int(lua, &args, 1, "remove", n as i64)?;
    if args.len() > 1 && n + 1 != pos as usize && (pos < 1 || pos as usize > n) && !(n == 0 && pos == 0) {
        return Err(bad_arg(lua, 1, "remove", "position out of bounds"));
    }
    if pos as usize == n + 1 || pos == 0 {
        let v = t.borrow().get_int(pos);
        let _ = t.borrow_mut().set(Value::Num(pos as f64), Value::Nil);
        return one(v);
    }
    lua.charge(n * 2)?;
    let v = t.borrow_mut().remove(pos as usize);
    one(v)
}

fn t_concat(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = check_table(lua, &args, 0, "concat")?;
    let sep = opt_str(lua, &args, 1, "concat")?.unwrap_or_else(|| Rc::from(&b""[..]));
    let n = t.borrow().len() as i64;
    let i = opt_int(lua, &args, 2, "concat", 1)?;
    let j = opt_int(lua, &args, 3, "concat", n)?;
    let mut out = Vec::new();
    let mut k = i;
    while k <= j {
        let v = t.borrow().get_int(k);
        match v {
            Value::Str(s) => out.extend_from_slice(&s),
            Value::Num(x) => out.extend_from_slice(fmt_num(x).as_bytes()),
            other => {
                return Err(lua.rt_error(format!(
                    "invalid value (at index {k}) in table for 'concat' (a {} value)",
                    other.type_name()
                )));
            }
        }
        if k < j {
            out.extend_from_slice(&sep);
        }
        if out.len() > MAX_STRING {
            return Err(Throw::Fatal("table.concat: result is too large".into()));
        }
        k += 1;
    }
    lua.charge(out.len())?;
    one(Value::Str(out.into()))
}

fn t_pack(_: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let n = args.len();
    let mut t = Table::from_list(args);
    t.set_str("n", Value::Num(n as f64));
    one(Value::table(t))
}

fn t_sort(lua: &mut Lua, h: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = check_table(lua, &args, 0, "sort")?;
    readonly(lua, &t)?;
    let cmp = match args.get(1) {
        None | Some(Value::Nil) => None,
        Some(_) => Some(check_func(lua, &args, 1, "sort")?),
    };
    let items = t.borrow().list();
    let n = items.len();
    lua.charge(n * 16)?;
    let sorted = merge_sort(lua, h, items, cmp.as_ref())?;
    t.borrow_mut().set_list(sorted);
    let _ = n;
    Ok(Vec::new())
}

/// A stable merge sort with a comparator that can fail (Rust's own sorts may
/// panic on an inconsistent order, which a script can give).
fn merge_sort(lua: &mut Lua, h: &mut dyn Host, mut src: Vec<Value>, cmp: Option<&Value>) -> Result<Vec<Value>, Throw> {
    let n = src.len();
    let mut buf: Vec<Value> = Vec::with_capacity(n);
    let mut width = 1;
    while width < n {
        buf.clear();
        let mut i = 0;
        while i < n {
            let mid = (i + width).min(n);
            let end = (i + 2 * width).min(n);
            let (mut a, mut b) = (i, mid);
            while a < mid && b < end {
                let less = match cmp {
                    Some(f) => lua.call1(h, f, vec![src[b].clone(), src[a].clone()])?.truthy(),
                    None => lua.less(h, &src[b], &src[a], false)?,
                };
                if less {
                    buf.push(src[b].clone());
                    b += 1;
                } else {
                    buf.push(src[a].clone());
                    a += 1;
                }
            }
            buf.extend_from_slice(&src[a..mid]);
            buf.extend_from_slice(&src[b..end]);
            i = end;
        }
        std::mem::swap(&mut src, &mut buf);
        width *= 2;
    }
    Ok(src)
}

// ----------------------------------------------------------------- coroutine

fn coroutine_lib() -> TableRef {
    lib(&[
        ("create", co_create),
        ("resume", co_resume),
        ("yield", co_yield),
        ("status", co_status),
        ("wrap", co_wrap),
        ("isyieldable", co_isyieldable),
        ("running", co_running),
        ("close", co_close),
    ])
}

fn check_thread(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<Rc<Thread>, Throw> {
    match args.get(i) {
        Some(Value::Thread(t)) => Ok(t.clone()),
        _ => Err(bad_arg(lua, i, fname, "coroutine expected")),
    }
}

fn check_function(lua: &Lua, args: &[Value], fname: &str) -> Result<Value, Throw> {
    match args.first() {
        Some(f) if f.is_callable() => Ok(f.clone()),
        other => {
            let got = other.map_or("no value", Value::type_name);
            Err(bad_arg(lua, 0, fname, &format!("function expected, got {got}")))
        }
    }
}

fn co_create(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let f = check_function(lua, &args, "create")?;
    one(Value::Thread(lua.new_thread(f)))
}

fn co_resume(lua: &mut Lua, h: &mut dyn Host, _: &Native, mut args: Vec<Value>) -> Res {
    let co = check_thread(lua, &args, 0, "resume")?;
    args.remove(0);
    match lua.resume(h, &co, args) {
        Resume::Done(mut v) | Resume::Yielded(mut v) => {
            v.insert(0, Value::Bool(true));
            Ok(v)
        }
        Resume::Error(Throw::Error(e, _)) => Ok(vec![Value::Bool(false), e]),
        Resume::Error(fatal) => Err(fatal),
    }
}

fn co_yield(_: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    Err(Throw::Yield(args))
}

fn co_status(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let co = check_thread(lua, &args, 0, "status")?;
    one(Value::str(co.status().name()))
}

fn co_wrap(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let f = check_function(lua, &args, "wrap")?;
    let co = lua.new_thread(f);
    one(Native::with_state("wrap", wrap_step, vec![Value::Thread(co)]))
}

/// A `coroutine.wrap` function: resumes its coroutine; an error goes on
/// with where it was called from in front, as in Lua.
fn wrap_step(lua: &mut Lua, h: &mut dyn Host, me: &Native, args: Vec<Value>) -> Res {
    let co = match me.state.borrow().first() {
        Some(Value::Thread(t)) => t.clone(),
        _ => return Err(lua.rt_error("cannot resume dead coroutine")),
    };
    match lua.resume(h, &co, args) {
        Resume::Done(v) | Resume::Yielded(v) => Ok(v),
        Resume::Error(Throw::Error(Value::Str(s), tb)) => {
            let msg = match lua.position(1) {
                Some(at) => format!("{at} {}", String::from_utf8_lossy(&s)),
                None => String::from_utf8_lossy(&s).into_owned(),
            };
            Err(Throw::Error(Value::str(&msg), tb))
        }
        Resume::Error(e) => Err(e),
    }
}

fn co_isyieldable(lua: &mut Lua, _: &mut dyn Host, _: &Native, _: Vec<Value>) -> Res {
    one(Value::Bool(lua.is_yieldable()))
}

fn co_running(lua: &mut Lua, _: &mut dyn Host, _: &Native, _: Vec<Value>) -> Res {
    let t = lua.current_thread();
    let main = t.is_main();
    Ok(vec![Value::Thread(t), Value::Bool(main)])
}

fn co_close(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let co = check_thread(lua, &args, 0, "close")?;
    match lua.close_thread(&co) {
        Ok(None) => one(Value::Bool(true)),
        Ok(Some(e)) => Ok(vec![Value::Bool(false), e]),
        Err(msg) => Err(lua.rt_error(msg)),
    }
}

// ----------------------------------------------------------------- os

fn os_lib() -> TableRef {
    lib(&[
        ("clock", os_clock),
        ("time", os_time),
        ("date", os_date),
        ("difftime", os_difftime),
    ])
}

fn os_difftime(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t2 = check_num(lua, &args, 0, "difftime")?;
    let t1 = opt_num(lua, &args, 1, "difftime", 0.0)?;
    one(Value::Num(t2 - t1))
}

/// `collectgarbage([opt])`: memory is managed for scripts; "count" is 0 and
/// the rest do nothing.
fn collectgarbage(_: &mut Lua, _: &mut dyn Host, _: &Native, _: Vec<Value>) -> Res {
    one(Value::Num(0.0))
}

// ----------------------------------------------------------------- utf8

fn utf8_lib() -> TableRef {
    let t = lib(&[
        ("char", u_char),
        ("codepoint", u_codepoint),
        ("len", u_len),
        ("offset", u_offset),
        ("codes", u_codes),
        ("graphemes", u_graphemes),
        ("nfcnormalize", u_normalize),
        ("nfdnormalize", u_normalize),
    ]);
    t.borrow_mut()
        .set_str("charpattern", Value::bytes(b"[\x00-\x7F\xC2-\xFD][\x80-\xBF]*"));
    t
}

/// The code point at byte `i` and its length in bytes, if a whole one starts there.
fn utf8_at(s: &[u8], i: usize) -> Option<(u32, usize)> {
    let b0 = *s.get(i)?;
    let (len, first) = match b0 {
        0x00..=0x7F => return Some((u32::from(b0), 1)),
        0xC2..=0xDF => (2, u32::from(b0 & 0x1F)),
        0xE0..=0xEF => (3, u32::from(b0 & 0x0F)),
        0xF0..=0xF4 => (4, u32::from(b0 & 0x07)),
        _ => return None,
    };
    let mut code = first;
    for k in 1..len {
        let b = *s.get(i + k)?;
        if b & 0xC0 != 0x80 {
            return None;
        }
        code = (code << 6) | u32::from(b & 0x3F);
    }
    let shortest = match len {
        2 => code >= 0x80,
        3 => code >= 0x800,
        _ => (0x1_0000..=0x10_FFFF).contains(&code),
    };
    shortest.then_some((code, len))
}

/// A 1-based (or negative, from the end) position as a 0-based index.
fn byte_pos(i: i64, len: usize) -> i64 {
    if i >= 0 { i - 1 } else { len as i64 + i }
}

fn u_char(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let mut out = String::new();
    for i in 0..args.len() {
        let c = check_int(lua, &args, i, "char")?;
        let ch = u32::try_from(c)
            .ok()
            .and_then(char::from_u32)
            .ok_or_else(|| bad_arg(lua, i, "char", "value out of range"))?;
        out.push(ch);
    }
    one(Value::str(&out))
}

fn u_codepoint(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let s = check_str(lua, &args, 0, "codepoint")?;
    let i = opt_int(lua, &args, 1, "codepoint", 1)?;
    let j = opt_int(lua, &args, 2, "codepoint", i)?;
    let (from, to) = (byte_pos(i, s.len()), byte_pos(j, s.len()));
    if from < 0 || to >= s.len() as i64 {
        return Err(bad_arg(lua, 1, "codepoint", "out of range"));
    }
    let mut out = Vec::new();
    let mut at = from as usize;
    while (at as i64) <= to {
        let (code, n) = utf8_at(&s, at).ok_or_else(|| lua.rt_error("invalid UTF-8 code"))?;
        out.push(Value::Num(f64::from(code)));
        at += n;
    }
    Ok(out)
}

fn u_len(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let s = check_str(lua, &args, 0, "len")?;
    let i = opt_int(lua, &args, 1, "len", 1)?;
    let j = opt_int(lua, &args, 2, "len", -1)?;
    let (mut at, to) = (byte_pos(i, s.len()), byte_pos(j, s.len()));
    if at < 0 || at > s.len() as i64 {
        return Err(bad_arg(lua, 1, "len", "initial position out of string"));
    }
    let mut n = 0;
    while at <= to && (at as usize) < s.len() {
        match utf8_at(&s, at as usize) {
            Some((_, len)) => {
                at += len as i64;
                n += 1;
            }
            None => return Ok(vec![Value::Nil, Value::Num((at + 1) as f64)]),
        }
    }
    one(Value::Num(f64::from(n)))
}

fn u_offset(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let s = check_str(lua, &args, 0, "offset")?;
    let mut n = check_int(lua, &args, 1, "offset")?;
    let len = s.len() as i64;
    let default = if n >= 0 { 1 } else { len + 1 };
    let mut at = byte_pos(opt_int(lua, &args, 2, "offset", default)?, s.len());
    if at < 0 || at > len {
        return Err(bad_arg(lua, 2, "offset", "position out of range"));
    }
    let cont = |p: i64| p < len && s[p as usize] & 0xC0 == 0x80;
    if n == 0 {
        while at > 0 && cont(at) {
            at -= 1;
        }
        return one(Value::Num((at + 1) as f64));
    }
    if cont(at) {
        return Err(lua.rt_error("initial position is a continuation byte"));
    }
    if n < 0 {
        while n < 0 && at > 0 {
            at -= 1;
            while at > 0 && cont(at) {
                at -= 1;
            }
            n += 1;
        }
    } else {
        n -= 1;
        while n > 0 && at < len {
            at += 1;
            while cont(at) {
                at += 1;
            }
            n -= 1;
        }
    }
    if n == 0 {
        one(Value::Num((at + 1) as f64))
    } else {
        one(Value::Nil)
    }
}

/// `for pos, code in utf8.codes(s)`.
fn u_codes(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let s = check_str(lua, &args, 0, "codes")?;
    Ok(vec![
        Native::builtin("codes", u_codes_next),
        Value::Str(s),
        Value::Num(0.0),
    ])
}

fn u_codes_next(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let s = check_str(lua, &args, 0, "codes")?;
    let mut at = check_int(lua, &args, 1, "codes")? - 1;
    let len = s.len() as i64;
    if at < 0 {
        at = 0;
    } else if at < len {
        at += 1;
        while at < len && s[at as usize] & 0xC0 == 0x80 {
            at += 1;
        }
    }
    if at >= len {
        return Ok(Vec::new());
    }
    let (code, _) = utf8_at(&s, at as usize).ok_or_else(|| lua.rt_error("invalid UTF-8 code"))?;
    Ok(vec![Value::Num((at + 1) as f64), Value::Num(f64::from(code))])
}

fn os_clock(lua: &mut Lua, _: &mut dyn Host, _: &Native, _: Vec<Value>) -> Res {
    one(Value::Num(lua.started.elapsed().as_secs_f64()))
}

fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs() as f64)
}

fn os_time(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let Some(Value::Table(t)) = args.first() else {
        return one(Value::Num(now_secs()));
    };
    let field = |name: &str, default: Option<i64>| -> Result<i64, Throw> {
        match t.borrow().get_str(name) {
            Value::Num(n) => Ok(n as i64),
            _ => default.ok_or_else(|| lua.rt_error(format!("field '{name}' missing in date table"))),
        }
    };
    let (y, m, d) = (field("year", None)?, field("month", None)?, field("day", None)?);
    let (hh, mm, ss) = (field("hour", Some(12))?, field("min", Some(0))?, field("sec", Some(0))?);
    let days = days_from_civil(y, m, d);
    one(Value::Num((days * 86400 + hh * 3600 + mm * 60 + ss) as f64))
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `os.date(format, time)` in UTC: `*t` for a table, else strftime's
/// %Y %y %m %d %H %I %M %S %p %A %a %B %b %j %c %x %X %%.
fn os_date(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let fmt =
        opt_str(lua, &args, 0, "date")?.map_or_else(|| "%c".to_owned(), |s| String::from_utf8_lossy(&s).into_owned());
    let fmt = fmt.strip_prefix('!').unwrap_or(&fmt).to_owned();
    let t = opt_num(lua, &args, 1, "date", now_secs())? as i64;
    let days = t.div_euclid(86400);
    let secs = t.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mi, ss) = (secs / 3600, secs / 60 % 60, secs % 60);
    let wday = (days + 4).rem_euclid(7); // 0 = Sunday
    let yday = days - days_from_civil(y, 1, 1) + 1;
    if fmt.starts_with("*t") {
        let mut tb = Table::new();
        for (k, v) in [
            ("year", y),
            ("month", m),
            ("day", d),
            ("hour", hh),
            ("min", mi),
            ("sec", ss),
            ("wday", wday + 1),
            ("yday", yday),
        ] {
            tb.set_str(k, Value::Num(v as f64));
        }
        tb.set_str("isdst", Value::Bool(false));
        return one(Value::table(tb));
    }
    const DAYS: [&str; 7] = [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ];
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let mut out = String::new();
    let mut chars = fmt.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let h12 = if hh % 12 == 0 { 12 } else { hh % 12 };
        let piece = match chars.next() {
            Some('Y') => y.to_string(),
            Some('y') => format!("{:02}", y.rem_euclid(100)),
            Some('m') => format!("{m:02}"),
            Some('d') => format!("{d:02}"),
            Some('H') => format!("{hh:02}"),
            Some('I') => format!("{h12:02}"),
            Some('M') => format!("{mi:02}"),
            Some('S') => format!("{ss:02}"),
            Some('p') => (if hh < 12 { "AM" } else { "PM" }).to_owned(),
            Some('A') => DAYS[wday as usize].to_owned(),
            Some('a') => DAYS[wday as usize][..3].to_owned(),
            Some('B') => MONTHS[m as usize - 1].to_owned(),
            Some('b') => MONTHS[m as usize - 1][..3].to_owned(),
            Some('j') => format!("{yday:03}"),
            Some('c') => format!(
                "{} {} {d:2} {hh:02}:{mi:02}:{ss:02} {y}",
                &DAYS[wday as usize][..3],
                &MONTHS[m as usize - 1][..3]
            ),
            Some('x') => format!("{m:02}/{d:02}/{:02}", y.rem_euclid(100)),
            Some('X') => format!("{hh:02}:{mi:02}:{ss:02}"),
            Some('%') => "%".to_owned(),
            Some(other) => {
                return Err(bad_arg(
                    lua,
                    0,
                    "date",
                    &format!("invalid conversion specifier '%{other}'"),
                ));
            }
            None => "%".to_owned(),
        };
        out.push_str(&piece);
    }
    one(Value::str(&out))
}

/// A string argument as text, for the studio's API functions.
pub fn text_of(v: &Str) -> String {
    String::from_utf8_lossy(v).into_owned()
}

// ----------------------------------------------------------------- buffer

/// Luau's buffer library: fixed-size byte arrays (`type` "buffer").
fn buffer_lib() -> TableRef {
    lib(&[
        ("create", b_create),
        ("fromstring", b_fromstring),
        ("tostring", b_tostring),
        ("len", b_len),
        ("readi8", b_read),
        ("readu8", b_read),
        ("readi16", b_read),
        ("readu16", b_read),
        ("readi32", b_read),
        ("readu32", b_read),
        ("readf32", b_read),
        ("readf64", b_read),
        ("writei8", b_write),
        ("writeu8", b_write),
        ("writei16", b_write),
        ("writeu16", b_write),
        ("writei32", b_write),
        ("writeu32", b_write),
        ("writef32", b_write),
        ("writef64", b_write),
        ("readstring", b_readstring),
        ("writestring", b_writestring),
        ("copy", b_copy),
        ("fill", b_fill),
        ("readbits", b_readbits),
        ("writebits", b_writebits),
    ])
}

/// The largest buffer: Luau's 1 GiB.
const MAX_BUFFER: i64 = 1 << 30;

fn new_buffer(bytes: Vec<u8>) -> Value {
    super::hstudio_lua::Userdata::value(super::hstudio_lua::LuaBuffer(bytes), None)
}

/// Runs `f` on buffer argument `i`'s bytes.
fn with_buffer<R>(
    lua: &Lua,
    args: &[Value],
    i: usize,
    fname: &str,
    f: impl FnOnce(&mut Vec<u8>) -> R,
) -> Result<R, Throw> {
    if let Some(Value::Userdata(u)) = args.get(i) {
        let mut d = u.data.borrow_mut();
        if let Some(b) = d.downcast_mut::<super::hstudio_lua::LuaBuffer>() {
            return Ok(f(&mut b.0));
        }
    }
    Err(bad_arg(lua, i, fname, "buffer expected"))
}

fn oob(lua: &Lua) -> Throw {
    lua.rt_error("buffer access out of bounds")
}

/// A range of `n` bytes at `offset` in a buffer of `len`, if inside it.
fn range(offset: i64, n: i64, len: usize) -> Option<std::ops::Range<usize>> {
    (offset >= 0 && n >= 0 && offset.checked_add(n)? as u64 <= len as u64)
        .then(|| offset as usize..(offset + n) as usize)
}

fn b_create(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let n = check_int(lua, &args, 0, "create")?;
    if !(0..=MAX_BUFFER).contains(&n) {
        return Err(bad_arg(lua, 0, "create", "size out of range"));
    }
    lua.charge(n as usize)?;
    one(new_buffer(vec![0; n as usize]))
}

fn b_fromstring(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let s = check_str(lua, &args, 0, "fromstring")?;
    one(new_buffer(s.to_vec()))
}

fn b_tostring(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let s = with_buffer(lua, &args, 0, "tostring", |b| b.clone())?;
    one(Value::bytes(&s))
}

fn b_len(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let n = with_buffer(lua, &args, 0, "len", |b| b.len())?;
    one(Value::Num(n as f64))
}

/// A read or write function's width and kind, by its name.
fn b_kind(name: &str) -> (usize, char) {
    let kind = name
        .chars()
        .nth(if name.starts_with("read") { 4 } else { 5 })
        .unwrap_or('u');
    let width: usize = name
        .trim_start_matches(|c: char| c.is_ascii_alphabetic())
        .parse()
        .unwrap_or(8);
    (width / 8, kind)
}

fn b_read(lua: &mut Lua, _: &mut dyn Host, me: &Native, args: Vec<Value>) -> Res {
    let (n, kind) = b_kind(&me.name);
    let offset = check_int(lua, &args, 1, &me.name)?;
    let bytes = with_buffer(lua, &args, 0, &me.name, |b| {
        range(offset, n as i64, b.len()).map(|r| b[r].to_vec())
    })?
    .ok_or_else(|| oob(lua))?;
    let mut raw = [0u8; 8];
    raw[..n].copy_from_slice(&bytes);
    let u = u64::from_le_bytes(raw);
    let v = match (kind, n) {
        ('f', 4) => f64::from(f32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]])),
        ('f', _) => f64::from_le_bytes(raw),
        ('i', _) => {
            let shift = 64 - 8 * n as u32;
            (((u << shift) as i64) >> shift) as f64
        }
        _ => u as f64,
    };
    one(Value::Num(v))
}

fn b_write(lua: &mut Lua, _: &mut dyn Host, me: &Native, args: Vec<Value>) -> Res {
    let (n, kind) = b_kind(&me.name);
    let offset = check_int(lua, &args, 1, &me.name)?;
    let v = check_num(lua, &args, 2, &me.name)?;
    let raw: [u8; 8] = match (kind, n) {
        ('f', 4) => {
            let b = (v as f32).to_le_bytes();
            [b[0], b[1], b[2], b[3], 0, 0, 0, 0]
        }
        ('f', _) => v.to_le_bytes(),
        // Integers wrap, as Luau's conversion does.
        _ => (if v.is_finite() { v.trunc() as i128 as u64 } else { 0 }).to_le_bytes(),
    };
    with_buffer(lua, &args, 0, &me.name, |b| {
        let r = range(offset, n as i64, b.len())?;
        b[r].copy_from_slice(&raw[..n]);
        Some(())
    })?
    .ok_or_else(|| oob(lua))?;
    Ok(Vec::new())
}

fn b_readstring(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let offset = check_int(lua, &args, 1, "readstring")?;
    let count = check_int(lua, &args, 2, "readstring")?;
    let s = with_buffer(lua, &args, 0, "readstring", |b| {
        range(offset, count, b.len()).map(|r| b[r].to_vec())
    })?
    .ok_or_else(|| oob(lua))?;
    one(Value::bytes(&s))
}

fn b_writestring(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let offset = check_int(lua, &args, 1, "writestring")?;
    let s = check_str(lua, &args, 2, "writestring")?;
    let count = opt_int(lua, &args, 3, "writestring", s.len() as i64)?;
    if count < 0 || count as usize > s.len() {
        return Err(bad_arg(lua, 3, "writestring", "count out of range"));
    }
    with_buffer(lua, &args, 0, "writestring", |b| {
        let r = range(offset, count, b.len())?;
        b[r].copy_from_slice(&s[..count as usize]);
        Some(())
    })?
    .ok_or_else(|| oob(lua))?;
    Ok(Vec::new())
}

fn b_copy(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t_off = check_int(lua, &args, 1, "copy")?;
    let source = with_buffer(lua, &args, 2, "copy", |b| b.clone())?;
    let s_off = opt_int(lua, &args, 3, "copy", 0)?;
    let count = opt_int(lua, &args, 4, "copy", source.len() as i64 - s_off)?;
    let src = range(s_off, count, source.len()).ok_or_else(|| oob(lua))?;
    with_buffer(lua, &args, 0, "copy", |b| {
        let r = range(t_off, count, b.len())?;
        b[r].copy_from_slice(&source[src]);
        Some(())
    })?
    .ok_or_else(|| oob(lua))?;
    Ok(Vec::new())
}

fn b_fill(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let offset = check_int(lua, &args, 1, "fill")?;
    let value = check_int(lua, &args, 2, "fill")? as u8;
    let len = with_buffer(lua, &args, 0, "fill", |b| b.len())?;
    let count = opt_int(lua, &args, 3, "fill", len as i64 - offset)?;
    with_buffer(lua, &args, 0, "fill", |b| {
        let r = range(offset, count, b.len())?;
        b[r].fill(value);
        Some(())
    })?
    .ok_or_else(|| oob(lua))?;
    Ok(Vec::new())
}

fn b_readbits(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let bit = check_int(lua, &args, 1, "readbits")?;
    let count = check_int(lua, &args, 2, "readbits")?;
    if !(0..=32).contains(&count) {
        return Err(bad_arg(lua, 2, "readbits", "bit count is out of range of [0; 32]"));
    }
    let v = with_buffer(lua, &args, 0, "readbits", |b| {
        if bit < 0 || (bit + count) as u64 > b.len() as u64 * 8 {
            return None;
        }
        let mut v: u64 = 0;
        for k in 0..count {
            let p = (bit + k) as usize;
            v |= u64::from((b[p / 8] >> (p % 8)) & 1) << k;
        }
        Some(v)
    })?
    .ok_or_else(|| oob(lua))?;
    one(Value::Num(v as f64))
}

fn b_writebits(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let bit = check_int(lua, &args, 1, "writebits")?;
    let count = check_int(lua, &args, 2, "writebits")?;
    let value = check_num(lua, &args, 3, "writebits")? as i128 as u64;
    if !(0..=32).contains(&count) {
        return Err(bad_arg(lua, 2, "writebits", "bit count is out of range of [0; 32]"));
    }
    with_buffer(lua, &args, 0, "writebits", |b| {
        if bit < 0 || (bit + count) as u64 > b.len() as u64 * 8 {
            return None;
        }
        for k in 0..count {
            let p = (bit + k) as usize;
            let on = (value >> k) & 1 == 1;
            if on {
                b[p / 8] |= 1 << (p % 8);
            } else {
                b[p / 8] &= !(1 << (p % 8));
            }
        }
        Some(())
    })?
    .ok_or_else(|| oob(lua))?;
    Ok(Vec::new())
}

// ----------------------------------------------------------------- string.pack

/// One item of a pack format: its kind and size, read from `fmt` at `i`.
enum PackItem {
    Int {
        size: usize,
        signed: bool,
    },
    Float(usize),
    /// A string with its length before it, in `size` bytes.
    Prefixed(usize),
    Fixed(usize),
    Zero,
    Pad,
    Align(usize),
}

/// Lua 5.3's string.pack formats: endianness (`<`, `>`, `=`), `!` alignment
/// (taken, not applied but for `X`), and the items.
fn pack_items(lua: &Lua, fmt: &[u8], fname: &str) -> Result<Vec<(PackItem, bool)>, Throw> {
    let mut out = Vec::new();
    let mut little = true;
    let mut i = 0;
    let num = |i: &mut usize| -> Option<usize> {
        let start = *i;
        while *i < fmt.len() && fmt[*i].is_ascii_digit() {
            *i += 1;
        }
        std::str::from_utf8(&fmt[start..*i]).ok()?.parse().ok()
    };
    while i < fmt.len() {
        let c = fmt[i];
        i += 1;
        let item = match c {
            b'<' | b'=' => {
                little = true;
                continue;
            }
            b'>' => {
                little = false;
                continue;
            }
            b'!' | b' ' => {
                let _ = num(&mut i);
                continue;
            }
            b'b' => PackItem::Int { size: 1, signed: true },
            b'B' => PackItem::Int { size: 1, signed: false },
            b'h' => PackItem::Int { size: 2, signed: true },
            b'H' => PackItem::Int { size: 2, signed: false },
            b'i' => PackItem::Int {
                size: num(&mut i).unwrap_or(4),
                signed: true,
            },
            b'I' => PackItem::Int {
                size: num(&mut i).unwrap_or(4),
                signed: false,
            },
            b'l' | b'j' => PackItem::Int { size: 8, signed: true },
            b'L' | b'J' | b'T' => PackItem::Int { size: 8, signed: false },
            b'f' => PackItem::Float(4),
            b'd' | b'n' => PackItem::Float(8),
            b's' => PackItem::Prefixed(num(&mut i).unwrap_or(8)),
            b'c' => PackItem::Fixed(
                num(&mut i).ok_or_else(|| lua.rt_error(format!("{fname}: missing size for format option 'c'")))?,
            ),
            b'z' => PackItem::Zero,
            b'x' => PackItem::Pad,
            b'X' => {
                let size = match fmt.get(i) {
                    Some(b'i' | b'I') => {
                        i += 1;
                        num(&mut i).unwrap_or(4)
                    }
                    Some(b'b' | b'B') => 1,
                    Some(b'h' | b'H') => 2,
                    Some(b'f') => 4,
                    Some(_) => 8,
                    None => return Err(lua.rt_error(format!("{fname}: invalid next option for option 'X'"))),
                };
                if fmt.get(i).is_some_and(|c| !c.is_ascii_digit()) && !matches!(fmt.get(i - 1), Some(b'i' | b'I')) {
                    i += 1;
                }
                PackItem::Align(size)
            }
            _ => return Err(lua.rt_error(format!("{fname}: invalid format option '{}'", c as char))),
        };
        if let PackItem::Int { size, .. } = item
            && !(1..=16).contains(&size)
        {
            return Err(lua.rt_error(format!("{fname}: integral size ({size}) out of limits [1,16]")));
        }
        out.push((item, little));
    }
    Ok(out)
}

fn put_int(out: &mut Vec<u8>, v: i128, size: usize, little: bool) {
    let bytes = v.to_le_bytes();
    let mut b: Vec<u8> = (0..size)
        .map(|k| {
            if k < 16 {
                bytes[k]
            } else if v < 0 {
                0xFF
            } else {
                0
            }
        })
        .collect();
    if !little {
        b.reverse();
    }
    out.extend_from_slice(&b);
}

fn s_pack(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let fmt = check_str(lua, &args, 0, "pack")?;
    let mut out = Vec::new();
    let mut a = 1;
    for (item, little) in pack_items(lua, &fmt, "pack")? {
        match item {
            PackItem::Int { size, .. } => {
                let v = check_num(lua, &args, a, "pack")?;
                a += 1;
                put_int(&mut out, v.trunc() as i128, size, little);
            }
            PackItem::Float(4) => {
                let v = check_num(lua, &args, a, "pack")? as f32;
                a += 1;
                let mut b = v.to_le_bytes().to_vec();
                if !little {
                    b.reverse();
                }
                out.extend_from_slice(&b);
            }
            PackItem::Float(_) => {
                let v = check_num(lua, &args, a, "pack")?;
                a += 1;
                let mut b = v.to_le_bytes().to_vec();
                if !little {
                    b.reverse();
                }
                out.extend_from_slice(&b);
            }
            PackItem::Prefixed(size) => {
                let s = check_str(lua, &args, a, "pack")?;
                a += 1;
                put_int(&mut out, s.len() as i128, size, little);
                out.extend_from_slice(&s);
            }
            PackItem::Fixed(n) => {
                let s = check_str(lua, &args, a, "pack")?;
                a += 1;
                if s.len() > n {
                    return Err(bad_arg(lua, a - 1, "pack", "string longer than given size"));
                }
                out.extend_from_slice(&s);
                out.resize(out.len() + n - s.len(), 0);
            }
            PackItem::Zero => {
                let s = check_str(lua, &args, a, "pack")?;
                a += 1;
                if s.contains(&0) {
                    return Err(bad_arg(lua, a - 1, "pack", "string contains zeros"));
                }
                out.extend_from_slice(&s);
                out.push(0);
            }
            PackItem::Pad => out.push(0),
            PackItem::Align(size) => {
                while out.len() % size != 0 {
                    out.push(0);
                }
            }
        }
        if out.len() > MAX_STRING {
            return Err(lua.rt_error("pack: result too large"));
        }
    }
    one(Value::bytes(&out))
}

fn s_packsize(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let fmt = check_str(lua, &args, 0, "packsize")?;
    let mut n = 0usize;
    for (item, _) in pack_items(lua, &fmt, "packsize")? {
        n += match item {
            PackItem::Int { size, .. } => size,
            PackItem::Float(size) | PackItem::Fixed(size) => size,
            PackItem::Pad => 1,
            PackItem::Align(size) => (size - n % size) % size,
            PackItem::Prefixed(_) | PackItem::Zero => {
                return Err(bad_arg(lua, 0, "packsize", "variable-size format in packsize"));
            }
        };
    }
    one(Value::Num(n as f64))
}

fn s_unpack(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let fmt = check_str(lua, &args, 0, "unpack")?;
    let data = check_str(lua, &args, 1, "unpack")?;
    let start = opt_int(lua, &args, 2, "unpack", 1)?;
    let mut pos = if start < 0 {
        (data.len() as i64 + start).max(0) as usize
    } else {
        (start.max(1) - 1) as usize
    };
    if pos > data.len() {
        return Err(bad_arg(lua, 2, "unpack", "initial position out of string"));
    }
    let short = |lua: &Lua| bad_arg(lua, 1, "unpack", "data string too short");
    let take = |pos: &mut usize, n: usize, lua: &Lua| -> Result<Vec<u8>, Throw> {
        let end = pos
            .checked_add(n)
            .filter(|&e| e <= data.len())
            .ok_or_else(|| short(lua))?;
        let b = data[*pos..end].to_vec();
        *pos = end;
        Ok(b)
    };
    let mut out = Vec::new();
    for (item, little) in pack_items(lua, &fmt, "unpack")? {
        let int_of = |mut b: Vec<u8>, signed: bool| -> f64 {
            if !little {
                b.reverse();
            }
            let size = b.len().min(16);
            let mut raw = [0u8; 16];
            raw[..size].copy_from_slice(&b[..size]);
            let u = u128::from_le_bytes(raw);
            if signed && size < 16 {
                let shift = 128 - 8 * size as u32;
                (((u << shift) as i128) >> shift) as f64
            } else {
                u as f64
            }
        };
        match item {
            PackItem::Int { size, signed } => out.push(Value::Num(int_of(take(&mut pos, size, lua)?, signed))),
            PackItem::Float(size) => {
                let mut b = take(&mut pos, size, lua)?;
                if !little {
                    b.reverse();
                }
                let v = if size == 4 {
                    f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                } else {
                    f64::from_le_bytes(b.try_into().unwrap_or([0; 8]))
                };
                out.push(Value::Num(v));
            }
            PackItem::Prefixed(size) => {
                let n = int_of(take(&mut pos, size, lua)?, false) as usize;
                out.push(Value::bytes(&take(&mut pos, n, lua)?));
            }
            PackItem::Fixed(n) => out.push(Value::bytes(&take(&mut pos, n, lua)?)),
            PackItem::Zero => {
                let end = data[pos..]
                    .iter()
                    .position(|&c| c == 0)
                    .ok_or_else(|| lua.rt_error("unpack: unfinished string for format 'z'"))?;
                out.push(Value::bytes(&data[pos..pos + end]));
                pos += end + 1;
            }
            PackItem::Pad => pos += 1,
            PackItem::Align(size) => {
                while pos % size != 0 {
                    pos += 1;
                }
            }
        }
    }
    out.push(Value::Num((pos + 1) as f64));
    Ok(out)
}

fn gcinfo(_: &mut Lua, _: &mut dyn Host, _: &Native, _: Vec<Value>) -> Res {
    // Kilobytes in use, as Luau's gcinfo: an estimate (the studio's own
    // allocator doesn't count per script).
    one(Value::Num(2048.0))
}

/// Roblox's utf8.graphemes(s[, i[, j]]): the start and end of each grapheme
/// cluster (a code point and the combining marks, variation selectors and
/// zero-width joins after it).
fn u_graphemes(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let s = check_str(lua, &args, 0, "graphemes")?;
    let i = opt_int(lua, &args, 1, "graphemes", 1)?.max(1) as usize - 1;
    let j = (opt_int(lua, &args, 2, "graphemes", s.len() as i64)?.max(0) as usize).min(s.len());
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut p = i;
    let mut join = false;
    while p < j {
        let (cp, n) = utf8_at(&s, p).unwrap_or((u32::from(s[p]), 1));
        let extends = matches!(cp, 0x300..=0x36F | 0x1AB0..=0x1AFF | 0x20D0..=0x20FF | 0xFE00..=0xFE0F | 0x1F3FB..=0x1F3FF | 0x200D | 0xE0020..=0xE007F);
        match spans.last_mut() {
            Some(last) if extends || join => last.1 = p + n,
            _ => spans.push((p, p + n)),
        }
        join = cp == 0x200D;
        p += n;
    }
    let mut t = Table::new();
    for (k, (a, b)) in spans.iter().enumerate() {
        let _ = t.set(Value::Num((2 * k + 1) as f64), Value::Num((a + 1) as f64));
        let _ = t.set(Value::Num((2 * k + 2) as f64), Value::Num(*b as f64));
    }
    let state = Value::table(t);
    let next = Native::with_state("graphemes_next", graphemes_next, vec![state, Value::Num(0.0)]);
    one(next)
}

fn graphemes_next(_: &mut Lua, _: &mut dyn Host, me: &Native, _: Vec<Value>) -> Res {
    let st = me.state.borrow();
    let (Some(Value::Table(t)), Some(Value::Num(k))) = (st.first(), st.get(1)) else {
        return Ok(Vec::new());
    };
    let k = *k as usize;
    let a = t.borrow().get(&Value::Num((2 * k + 1) as f64));
    let b = t.borrow().get(&Value::Num((2 * k + 2) as f64));
    drop(st);
    if a.is_nil() {
        return Ok(Vec::new());
    }
    me.state.borrow_mut()[1] = Value::Num((k + 1) as f64);
    Ok(vec![a, b])
}

/// utf8.nfcnormalize and nfdnormalize: the text as it is (the studio has no
/// Unicode tables to compose or decompose with).
fn u_normalize(lua: &mut Lua, _: &mut dyn Host, me: &Native, args: Vec<Value>) -> Res {
    let s = check_str(lua, &args, 0, &me.name)?;
    one(Value::bytes(&s))
}
