// The executor library's built-ins (hstudio_exec): the functions of the
// sUNC standard that need only the interpreter. Closures (hookfunction,
// newcclosure, clonefunction, restorefunction, getfunctionhash, the closure
// checks, loadstring), the debug library (Luau's info and traceback, and
// sUNC's constants, upvalues, protos and stack), metatables (getrawmetatable,
// setrawmetatable, setreadonly, getnamecallmethod), Base64 and LZ4, the
// garbage collector's view (getgc, filtergc, getreg), thread identities and
// identifyexecutor. The ones that need the studio (files, the network,
// Drawing, Instances) are in hstudio_sunc.

use super::hstudio_lua::{
    Body, Closure, Host, Lua, MAX_STRING, Native, NativeKind, Proto, Res, Table, TableRef, Throw, Userdata, Value,
    bad_arg, check_int, check_str, check_table, opt_int, opt_str,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// What identifyexecutor says.
pub const EXECUTOR: &str = "Horizon Studio";

thread_local! {
    /// The natives that are the executor's own (isexecutorclosure).
    static EXECUTOR_NATIVES: RefCell<HashSet<usize>> = RefCell::new(HashSet::new());
    /// Each thread's identity (getthreadidentity), by its address.
    static IDENTITIES: RefCell<HashMap<usize, i64>> = RefCell::new(HashMap::new());
}

/// The identity a thread runs at unless set: an executor's, 8.
pub const DEFAULT_IDENTITY: i64 = 8;

/// Marks a native as the executor's.
pub fn mark(v: &Value) {
    if let Value::Native(n) = v {
        EXECUTOR_NATIVES.with(|s| s.borrow_mut().insert(Rc::as_ptr(n) as usize));
    }
}

fn is_marked(n: &Rc<Native>) -> bool {
    EXECUTOR_NATIVES.with(|s| s.borrow().contains(&(Rc::as_ptr(n) as usize)))
}

/// The running thread's identity.
pub fn identity(lua: &Lua) -> i64 {
    let key = Rc::as_ptr(&lua.current_thread()) as usize;
    IDENTITIES.with(|m| m.borrow().get(&key).copied().unwrap_or(DEFAULT_IDENTITY))
}

fn one(v: Value) -> Res {
    Ok(vec![v])
}

fn none() -> Res {
    Ok(Vec::new())
}

/// The executor's built-in globals, and its `debug` library.
pub fn globals() -> Vec<(&'static str, Value)> {
    let entries: &[(&str, super::hstudio_lua::Builtin)] = &[
        ("checkcaller", checkcaller),
        ("clonefunction", clonefunction),
        ("getfunctionhash", getfunctionhash),
        ("hookfunction", hookfunction),
        ("replaceclosure", hookfunction),
        ("hookmetamethod", hookmetamethod),
        ("iscclosure", iscclosure),
        ("islclosure", islclosure),
        ("isexecutorclosure", isexecutorclosure),
        ("checkclosure", isexecutorclosure),
        ("isourclosure", isexecutorclosure),
        ("loadstring", loadstring),
        ("newcclosure", newcclosure),
        ("restorefunction", restorefunction),
        ("getrawmetatable", getrawmetatable),
        ("setrawmetatable", setrawmetatable),
        ("setreadonly", setreadonly),
        ("isreadonly", isreadonly),
        ("getnamecallmethod", getnamecallmethod),
        ("base64encode", base64encode),
        ("base64decode", base64decode),
        ("lz4compress", lz4compress),
        ("lz4decompress", lz4decompress),
        ("getgc", getgc),
        ("filtergc", filtergc),
        ("getreg", getreg),
        ("identifyexecutor", identifyexecutor),
        ("getexecutorname", identifyexecutor),
        ("getthreadidentity", getthreadidentity),
        ("setthreadidentity", setthreadidentity),
        ("getidentity", getthreadidentity),
        ("setidentity", setthreadidentity),
        // UNC's names for the thread identity.
        ("getthreadcontext", getthreadidentity),
        ("setthreadcontext", setthreadidentity),
        ("newproxy", newproxy),
        ("getfenv", getfenv),
        ("setfenv", setfenv),
        // Base64 as plain globals (sUNC) and under the UNC names.
        ("base64_encode", base64encode),
        ("base64_decode", base64decode),
    ];
    let mut out: Vec<(&'static str, Value)> = Vec::new();
    for (name, f) in entries {
        let v = Native::builtin(name, *f);
        mark(&v);
        out.push((name, v));
    }
    out.push(("debug", Value::Table(debug_lib())));
    out.push(("crypt", Value::Table(crypt_lib())));
    out.push(("base64", Value::Table(base64_lib())));
    out
}

fn debug_lib() -> TableRef {
    let entries: &[(&str, super::hstudio_lua::Builtin)] = &[
        ("traceback", d_traceback),
        ("info", d_info),
        ("getinfo", d_getinfo),
        ("getconstant", d_getconstant),
        ("getconstants", d_getconstants),
        ("setconstant", d_setconstant),
        ("getproto", d_getproto),
        ("getprotos", d_getprotos),
        ("getstack", d_getstack),
        ("setstack", d_setstack),
        ("getupvalue", d_getupvalue),
        ("getupvalues", d_getupvalues),
        ("setupvalue", d_setupvalue),
        ("getmetatable", getrawmetatable),
        ("setmetatable", setrawmetatable),
        ("getregistry", getreg),
        ("profilebegin", d_nothing),
        ("profileend", d_nothing),
        ("setmemorycategory", d_nothing),
        ("resetmemorycategory", d_nothing),
    ];
    let mut t = Table::new();
    for (name, f) in entries {
        let v = Native::builtin(name, *f);
        mark(&v);
        t.set_str(name, v);
    }
    Rc::new(RefCell::new(t))
}

// ---------------------------------------------------------------- closures

fn func_arg(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<Value, Throw> {
    match args.get(i) {
        Some(f @ (Value::Func(_) | Value::Native(_))) => Ok(f.clone()),
        other => Err(bad_arg(
            lua,
            i,
            fname,
            &format!(
                "function expected, got {}",
                other.map_or("no value".to_owned(), |v| lua.type_of(v))
            ),
        )),
    }
}

/// Every call made from a script mod is the executor's own.
fn checkcaller(_: &mut Lua, _: &mut dyn Host, _: &Native, _: Vec<Value>) -> Res {
    one(Value::Bool(true))
}

/// A new function that runs as `f` does now, unaffected by later hooks of `f`.
pub fn clone_fn(f: &Value) -> Value {
    match f {
        Value::Func(c) => {
            let copy = Rc::new(Closure::from_body(c.body()));
            copy.set_redirect(c.redirect());
            Value::Func(copy)
        }
        Value::Native(n) => {
            let copy = Native::of(&n.name, n.kind.clone(), n.state.borrow().clone());
            if let Value::Native(c) = &copy {
                c.set_redirect(n.redirect());
            }
            if is_marked(n) {
                mark(&copy);
            }
            copy
        }
        other => other.clone(),
    }
}

fn clonefunction(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let f = func_arg(lua, &args, 0, "clonefunction")?;
    let copy = match &f {
        // A clone runs the function itself, not what it is hooked to.
        Value::Func(c) => Value::Func(Rc::new(Closure::from_body(c.body()))),
        other => clone_fn(other),
    };
    one(copy)
}

fn set_redirect(f: &Value, to: Option<Value>) -> Option<Value> {
    match f {
        Value::Func(c) => c.set_redirect(to),
        Value::Native(n) => n.set_redirect(to),
        _ => None,
    }
}

fn redirect_of(f: &Value) -> Option<Value> {
    match f {
        Value::Func(c) => c.redirect(),
        Value::Native(n) => n.redirect(),
        _ => None,
    }
}

/// hookfunction: calls of `target` run `hook` from now on; the function as
/// it was comes back.
pub fn hook(lua: &Lua, target: &Value, hook: Value) -> Result<Value, Throw> {
    if target.raw_eq(&hook) {
        return Err(lua.rt_error("hookfunction: a function can't hook itself"));
    }
    let original = clone_fn(target);
    set_redirect(target, Some(hook));
    Ok(original)
}

fn hookfunction(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let target = func_arg(lua, &args, 0, "hookfunction")?;
    let h = func_arg(lua, &args, 1, "hookfunction")?;
    one(hook(lua, &target, h)?)
}

fn restorefunction(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let f = func_arg(lua, &args, 0, "restorefunction")?;
    if redirect_of(&f).is_none() {
        return Err(lua.rt_error("restorefunction: function is not hooked"));
    }
    set_redirect(&f, None);
    none()
}

fn hookmetamethod(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let obj = args.first().cloned().unwrap_or_default();
    let name = check_str(lua, &args, 1, "hookmetamethod")?;
    let h = func_arg(lua, &args, 2, "hookmetamethod")?;
    let Some(mt) = obj.metatable() else {
        return Err(lua.rt_error("hookmetamethod: the object has no metatable"));
    };
    let key = Value::Str(name.clone());
    let mut f = mt.borrow().get(&key);
    if f.is_nil() && &*name == b"__namecall" {
        // Without one, method calls look the method up: that, as a native.
        f = Native::of("__namecall", NativeKind::Namecall, Vec::new());
        let mut m = mt.borrow_mut();
        let ro = m.is_readonly();
        m.set_readonly(false);
        let _ = m.set(key, f.clone());
        m.set_readonly(ro);
    }
    if !f.is_callable() {
        return Err(lua.rt_error(format!(
            "hookmetamethod: the metatable has no {} function",
            String::from_utf8_lossy(&name)
        )));
    }
    one(hook(lua, &f, h)?)
}

fn iscclosure(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let f = func_arg(lua, &args, 0, "iscclosure")?;
    one(Value::Bool(matches!(f, Value::Native(_))))
}

fn islclosure(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let f = func_arg(lua, &args, 0, "islclosure")?;
    one(Value::Bool(matches!(f, Value::Func(_))))
}

/// Lua functions are all a mod's; natives are the executor's when it made them.
fn isexecutorclosure(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let f = func_arg(lua, &args, 0, "isexecutorclosure")?;
    one(Value::Bool(match &f {
        Value::Func(_) => true,
        Value::Native(n) => is_marked(n),
        _ => false,
    }))
}

/// The globals of the code calling: its function's environment.
pub fn caller_env(lua: &Lua) -> Option<TableRef> {
    lua.frame_info(1).map(|f| f.closure.env())
}

fn loadstring(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let src = check_str(lua, &args, 0, "loadstring")?;
    let name = match opt_str(lua, &args, 1, "loadstring")? {
        Some(n) => format!("[string \"{}\"]", String::from_utf8_lossy(&n)),
        None => {
            let first = String::from_utf8_lossy(&src).lines().next().unwrap_or("").to_owned();
            let short: String = first.chars().take(40).collect();
            let more = if short.len() < first.len() || src.contains(&b'\n') {
                "..."
            } else {
                ""
            };
            format!("[string \"{short}{more}\"]")
        }
    };
    let Some(env) = caller_env(lua) else {
        return Err(lua.rt_error("loadstring: no environment to load into"));
    };
    match lua.load_bytes(&src, &name, &env) {
        Ok(f) => one(f),
        Err(e) => Ok(vec![Value::Nil, Value::str(&e)]),
    }
}

fn newcclosure(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let f = func_arg(lua, &args, 0, "newcclosure")?;
    let name = match &f {
        Value::Func(c) => c.proto().debug_name.as_deref().unwrap_or("").to_owned(),
        Value::Native(n) => n.name.to_string(),
        _ => String::new(),
    };
    let w = Native::of(&name, NativeKind::Wrap(f), Vec::new());
    mark(&w);
    one(w)
}

/// The SHA-384 of a Lua function's constants and code, in hex.
pub fn function_hash(p: &Proto) -> String {
    let mut data = Vec::new();
    for c in p.constant_slots() {
        match c {
            Some(v) => data.extend_from_slice(format!("{}:{};", v.type_name(), v.plain_text()).as_bytes()),
            None => data.extend_from_slice(b"special;"),
        }
    }
    data.extend_from_slice(format!("{}|{}|", p.nparams, p.vararg).as_bytes());
    for op in &p.code {
        data.extend_from_slice(format!("{op:?};").as_bytes());
    }
    for child in &p.protos {
        data.extend_from_slice(function_hash(child).as_bytes());
    }
    super::hstudio_crypt::hex(&super::hstudio_crypt::sha384(&data))
}

fn getfunctionhash(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    match func_arg(lua, &args, 0, "getfunctionhash")? {
        Value::Func(c) => one(Value::str(&function_hash(&c.proto()))),
        _ => Err(lua.rt_error("getfunctionhash: C closures have nothing to hash")),
    }
}

// ---------------------------------------------------------------- debug

/// A Lua function argument, or the function at a stack level.
fn lua_fn(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<Rc<Closure>, Throw> {
    match args.get(i) {
        Some(Value::Func(c)) => Ok(c.clone()),
        Some(Value::Num(n)) => {
            let level = *n as usize;
            lua.frame_info(level)
                .map(|f| f.closure)
                .ok_or_else(|| bad_arg(lua, i, fname, "level out of range"))
        }
        Some(Value::Native(_)) => Err(bad_arg(
            lua,
            i,
            fname,
            "Lua function expected (C closures are not supported)",
        )),
        _ => Err(bad_arg(lua, i, fname, "function or level expected")),
    }
}

fn d_nothing(_: &mut Lua, _: &mut dyn Host, _: &Native, _: Vec<Value>) -> Res {
    none()
}

fn d_traceback(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let msg = match args.first() {
        Some(Value::Str(s)) => format!("{}\n", String::from_utf8_lossy(s)),
        Some(Value::Thread(_)) | None | Some(Value::Nil) => String::new(),
        Some(other) => return one(other.clone()),
    };
    one(Value::str(&format!("{msg}{}", lua.traceback())))
}

/// What `debug.info` and `getinfo` know of a function or a level.
struct Info {
    source: String,
    line: f64,
    name: String,
    nparams: f64,
    vararg: bool,
    func: Value,
    native: bool,
    nups: f64,
}

fn info_of(lua: &Lua, args: &[Value], fname: &str) -> Result<Option<Info>, Throw> {
    let (closure, line, func) = match args.first() {
        Some(Value::Num(n)) => {
            let level = *n as usize;
            match lua.frame_info(level) {
                Some(f) => (f.closure.clone(), f64::from(f.line), Value::Func(f.closure)),
                None => return Ok(None),
            }
        }
        Some(Value::Func(c)) => (c.clone(), f64::from(c.proto().line.max(1)), Value::Func(c.clone())),
        Some(Value::Native(n)) => {
            return Ok(Some(Info {
                source: "[C]".into(),
                line: -1.0,
                name: n.name.to_string(),
                nparams: 0.0,
                vararg: true,
                func: Value::Native(n.clone()),
                native: true,
                nups: 0.0,
            }));
        }
        _ => return Err(bad_arg(lua, 0, fname, "function or level expected")),
    };
    let p = closure.proto();
    Ok(Some(Info {
        source: p.chunk.to_string(),
        line,
        name: p.debug_name.as_deref().unwrap_or("").to_owned(),
        nparams: p.nparams as f64,
        vararg: p.vararg,
        func,
        native: false,
        nups: p.upvals.len() as f64,
    }))
}

/// Luau's `debug.info(f or level, "slnaf")`.
fn d_info(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    // debug.info(thread, level, options) drops the thread.
    let args = match args.first() {
        Some(Value::Thread(_)) => args[1..].to_vec(),
        _ => args,
    };
    let what = check_str(lua, &args, 1, "info")?;
    let Some(i) = info_of(lua, &args, "info")? else {
        return none();
    };
    let mut out = Vec::new();
    for c in what.iter() {
        match c {
            b's' => out.push(Value::str(&i.source)),
            b'l' => out.push(Value::Num(i.line)),
            b'n' => out.push(if i.name.is_empty() {
                Value::Nil
            } else {
                Value::str(&i.name)
            }),
            b'a' => {
                out.push(Value::Num(i.nparams));
                out.push(Value::Bool(i.vararg));
            }
            b'f' => out.push(i.func.clone()),
            _ => return Err(bad_arg(lua, 1, "info", "invalid option")),
        }
    }
    Ok(out)
}

fn d_getinfo(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let Some(i) = info_of(lua, &args, "getinfo")? else {
        return one(Value::Nil);
    };
    let mut t = Table::new();
    t.set_str("source", Value::str(&i.source));
    t.set_str("short_src", Value::str(&i.source));
    t.set_str("currentline", Value::Num(i.line));
    t.set_str("linedefined", Value::Num(i.line));
    t.set_str("name", Value::str(&i.name));
    t.set_str("what", Value::str(if i.native { "C" } else { "Lua" }));
    t.set_str("nups", Value::Num(i.nups));
    t.set_str("numparams", Value::Num(i.nparams));
    t.set_str("is_vararg", Value::Num(if i.vararg { 1.0 } else { 0.0 }));
    t.set_str("func", i.func);
    one(Value::table(t))
}

fn index_arg(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<usize, Throw> {
    let n = check_int(lua, args, i, fname)?;
    if n < 1 {
        return Err(bad_arg(lua, i, fname, "index out of range"));
    }
    Ok(n as usize - 1)
}

/// A list that may hold nils, as 1..n keys.
fn sparse(items: Vec<Value>) -> Value {
    let mut t = Table::new();
    for (i, v) in items.into_iter().enumerate() {
        if !v.is_nil() {
            let _ = t.set(Value::Num(i as f64 + 1.0), v);
        }
    }
    Value::table(t)
}

fn d_getconstants(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let c = lua_fn(lua, &args, 0, "getconstants")?;
    let slots = c.proto().constant_slots();
    one(sparse(slots.into_iter().map(Option::unwrap_or_default).collect()))
}

fn d_getconstant(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let c = lua_fn(lua, &args, 0, "getconstant")?;
    let i = index_arg(lua, &args, 1, "getconstant")?;
    one(c.proto().constant_slots().get(i).cloned().flatten().unwrap_or_default())
}

fn d_setconstant(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let c = lua_fn(lua, &args, 0, "setconstant")?;
    let i = index_arg(lua, &args, 1, "setconstant")?;
    let v = args.get(2).cloned().unwrap_or_default();
    if !matches!(v, Value::Nil | Value::Bool(_) | Value::Num(_) | Value::Str(_)) {
        return Err(bad_arg(
            lua,
            2,
            "setconstant",
            "number, string, boolean or nil expected",
        ));
    }
    let body = c.body();
    let p = body
        .proto
        .with_constant(i, v)
        .ok_or_else(|| bad_arg(lua, 1, "setconstant", "index out of range"))?;
    c.replace_body(Rc::new(Body {
        proto: Rc::new(p),
        upvals: body.upvals.clone(),
        env: body.env.clone(),
    }));
    none()
}

/// A closure of a nested function that can't be called (an inactive proto).
fn inactive(lua: &Lua, p: &Rc<Proto>, env: TableRef) -> Value {
    let upvals = p.upvals.iter().map(|_| Rc::new(RefCell::new(Value::Nil))).collect();
    let c = Rc::new(Closure::new(p.clone(), upvals, env));
    c.set_redirect(Some(Native::builtin("inactive proto", inactive_call)));
    let _ = lua;
    Value::Func(c)
}

fn inactive_call(lua: &mut Lua, _: &mut dyn Host, _: &Native, _: Vec<Value>) -> Res {
    Err(lua.rt_error("attempt to call an inactive proto (debug.getproto with activated = true gives live ones)"))
}

fn d_getprotos(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let c = lua_fn(lua, &args, 0, "getprotos")?;
    let env = c.env();
    let list = c.proto().protos.iter().map(|p| inactive(lua, p, env.clone())).collect();
    one(Value::table(Table::from_list(list)))
}

fn d_getproto(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let c = lua_fn(lua, &args, 0, "getproto")?;
    let i = index_arg(lua, &args, 1, "getproto")?;
    let proto = c.proto();
    let p = proto
        .protos
        .get(i)
        .cloned()
        .ok_or_else(|| bad_arg(lua, 1, "getproto", "proto index out of range"))?;
    if args.get(2).is_some_and(Value::truthy) {
        let live: Vec<Value> = lua
            .live_closures()
            .into_iter()
            .filter(|f| Rc::ptr_eq(&f.proto(), &p) && f.redirect().is_none())
            .map(Value::Func)
            .collect();
        return one(Value::table(Table::from_list(live)));
    }
    one(inactive(lua, &p, c.env()))
}

fn level_arg(lua: &Lua, args: &[Value], fname: &str) -> Result<usize, Throw> {
    let level = check_int(lua, args, 0, fname)?;
    if level < 1 || lua.frame_info(level as usize).is_none() {
        return Err(bad_arg(lua, 0, fname, "level out of range (C functions have no stack)"));
    }
    Ok(level as usize)
}

fn d_getstack(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let level = level_arg(lua, &args, "getstack")?;
    match args.get(1) {
        None | Some(Value::Nil) => one(sparse(lua.stack_values(level).unwrap_or_default())),
        Some(_) => {
            let i = index_arg(lua, &args, 1, "getstack")?;
            one(lua.get_stack(level, i).unwrap_or_default())
        }
    }
}

fn d_setstack(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let level = level_arg(lua, &args, "setstack")?;
    let i = index_arg(lua, &args, 1, "setstack")?;
    let v = args.get(2).cloned().unwrap_or_default();
    if !lua.set_stack(level, i, v) {
        return Err(bad_arg(lua, 1, "setstack", "index out of range"));
    }
    none()
}

fn d_getupvalues(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let c = lua_fn(lua, &args, 0, "getupvalues")?;
    let n = c.body().upvals.len();
    one(sparse((0..n).map(|i| c.upvalue(i).unwrap_or_default()).collect()))
}

fn d_getupvalue(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let c = lua_fn(lua, &args, 0, "getupvalue")?;
    let i = index_arg(lua, &args, 1, "getupvalue")?;
    c.upvalue(i)
        .map(|v| vec![v])
        .ok_or_else(|| bad_arg(lua, 1, "getupvalue", "upvalue index out of range"))
}

fn d_setupvalue(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let c = lua_fn(lua, &args, 0, "setupvalue")?;
    let i = index_arg(lua, &args, 1, "setupvalue")?;
    let v = args.get(2).cloned().unwrap_or_default();
    if !c.set_upvalue(i, v) {
        return Err(bad_arg(lua, 1, "setupvalue", "upvalue index out of range"));
    }
    none()
}

// ---------------------------------------------------------------- metatables

fn getrawmetatable(_: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    one(args.first().and_then(Value::metatable).map_or(Value::Nil, Value::Table))
}

fn setrawmetatable(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let obj = args.first().cloned().unwrap_or_default();
    let mt = match args.get(1) {
        Some(Value::Table(t)) => Some(t.clone()),
        None | Some(Value::Nil) => None,
        _ => return Err(bad_arg(lua, 1, "setrawmetatable", "table or nil expected")),
    };
    match &obj {
        Value::Table(t) => t.borrow_mut().meta = mt,
        Value::Userdata(u) => *u.meta.borrow_mut() = mt,
        // Other types share one metatable each here, which scripts can't replace.
        _ => {}
    }
    one(obj)
}

fn setreadonly(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = check_table(lua, &args, 0, "setreadonly")?;
    let on = args.get(1).is_some_and(Value::truthy);
    t.borrow_mut().set_readonly(on);
    none()
}

fn isreadonly(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let t = check_table(lua, &args, 0, "isreadonly")?;
    one(Value::Bool(t.borrow().is_readonly()))
}

fn getnamecallmethod(lua: &mut Lua, _: &mut dyn Host, _: &Native, _: Vec<Value>) -> Res {
    one(lua.namecall_method().map_or(Value::Nil, Value::Str))
}

// ---------------------------------------------------------------- encoding

fn base64encode(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let data = check_str(lua, &args, 0, "base64encode")?;
    one(Value::str(&super::hstudio_crypt::base64_encode(&data)))
}

fn base64decode(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let data = check_str(lua, &args, 0, "base64decode")?;
    let out = super::hstudio_crypt::base64_decode(&data[..]).map_err(|e| lua.rt_error(format!("base64decode: {e}")))?;
    one(Value::bytes(&out))
}

fn lz4compress(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let data = check_str(lua, &args, 0, "lz4compress")?;
    lua.charge(data.len())?;
    one(Value::bytes(&super::hstudio_crypt::lz4_compress(&data)))
}

fn lz4decompress(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let data = check_str(lua, &args, 0, "lz4decompress")?;
    let size = opt_int(lua, &args, 1, "lz4decompress", MAX_STRING as i64)?.clamp(0, MAX_STRING as i64);
    let out = super::hstudio_crypt::lz4_decompress(&data, size as usize)
        .map_err(|e| lua.rt_error(format!("lz4decompress: {e}")))?;
    lua.charge(out.len())?;
    one(Value::bytes(&out))
}

// ---------------------------------------------------------------- crypt (UNC)

/// UNC's `crypt` library, on the crypt primitives in hstudio_crypt. sUNC
/// deprecates this table (it warns one exists); UNC's check needs it.
fn crypt_lib() -> TableRef {
    let entries: &[(&str, super::hstudio_lua::Builtin)] = &[
        ("base64encode", base64encode),
        ("base64decode", base64decode),
        ("base64_encode", base64encode),
        ("base64_decode", base64decode),
        ("generatebytes", c_generatebytes),
        ("generatekey", c_generatekey),
        ("hash", c_hash),
        ("encrypt", c_encrypt),
        ("decrypt", c_decrypt),
    ];
    let mut t = Table::new();
    for (name, f) in entries {
        let v = Native::builtin(name, *f);
        mark(&v);
        t.set_str(name, v);
    }
    // crypt.base64.encode / .decode (nested; the check doesn't descend here).
    t.set_str("base64", Value::Table(base64_lib()));
    Rc::new(RefCell::new(t))
}

/// `base64` and `crypt.base64`: `encode` and `decode`.
fn base64_lib() -> TableRef {
    let mut t = Table::new();
    let enc = Native::builtin("encode", base64encode);
    let dec = Native::builtin("decode", base64decode);
    mark(&enc);
    mark(&dec);
    t.set_str("encode", enc);
    t.set_str("decode", dec);
    Rc::new(RefCell::new(t))
}

/// `crypt.generatebytes(n)`: `n` random bytes, as Base64.
fn c_generatebytes(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let n = check_int(lua, &args, 0, "generatebytes")?;
    if !(0..=1 << 20).contains(&n) {
        return Err(bad_arg(lua, 0, "generatebytes", "a size from 0 to 1048576 expected"));
    }
    lua.charge(n as usize)?;
    one(Value::str(&super::hstudio_crypt::base64_encode(
        &super::hstudio_crypt::random_bytes(n as usize),
    )))
}

/// `crypt.generatekey()`: a 32-byte AES key, as Base64.
fn c_generatekey(_: &mut Lua, _: &mut dyn Host, _: &Native, _: Vec<Value>) -> Res {
    one(Value::str(&super::hstudio_crypt::base64_encode(
        &super::hstudio_crypt::random_bytes(32),
    )))
}

/// `crypt.hash(data, algorithm)`: the digest in hex. Algorithms: md5, sha1,
/// sha224/256/384/512 and sha3-224/256/384/512.
fn c_hash(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let data = check_str(lua, &args, 0, "hash")?;
    let algo = check_str(lua, &args, 1, "hash")?;
    let h = super::hstudio_crypt::Hash::from_name(&String::from_utf8_lossy(&algo))
        .ok_or_else(|| bad_arg(lua, 1, "hash", "unknown hash algorithm"))?;
    lua.charge(data.len())?;
    one(Value::str(&h.hex(&data)))
}

/// A Base64 key or IV, decoded.
fn b64_arg(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<Vec<u8>, Throw> {
    let s = check_str(lua, args, i, fname)?;
    super::hstudio_crypt::base64_decode(&s[..]).map_err(|e| bad_arg(lua, i, fname, &format!("not Base64: {e}")))
}

/// A given IV cut to the mode's length when it is longer (UNC hands CBC a
/// 32-byte key as the IV); a shorter one is left for the cipher to reject.
fn iv_sized(iv: Vec<u8>, mode: super::hstudio_crypt::Mode) -> Vec<u8> {
    let want = mode.iv_size();
    if want > 0 && iv.len() > want {
        iv[..want].to_vec()
    } else {
        iv
    }
}

fn mode_arg(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<super::hstudio_crypt::Mode, Throw> {
    match opt_str(lua, args, i, fname)? {
        None => Ok(super::hstudio_crypt::Mode::Cbc),
        Some(s) => super::hstudio_crypt::Mode::from_name(&String::from_utf8_lossy(&s))
            .ok_or_else(|| bad_arg(lua, i, fname, "unknown cipher mode")),
    }
}

/// `crypt.encrypt(data, key, iv?, mode?)`: AES. `key` and `iv` are Base64
/// (a random IV is made when none is given). Returns the Base64 ciphertext
/// and the Base64 IV.
fn c_encrypt(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let data = check_str(lua, &args, 0, "encrypt")?;
    let key = b64_arg(lua, &args, 1, "encrypt")?;
    let mode = mode_arg(lua, &args, 3, "encrypt")?;
    let iv = match args.get(2) {
        None | Some(Value::Nil) => super::hstudio_crypt::random_bytes(mode.iv_size().max(16)),
        // A longer IV (UNC passes a 32-byte generatekey()) is cut to size.
        Some(_) => iv_sized(b64_arg(lua, &args, 2, "encrypt")?, mode),
    };
    let out =
        super::hstudio_crypt::encrypt(mode, &key, &iv, &data).map_err(|e| lua.rt_error(format!("encrypt: {e}")))?;
    Ok(vec![
        Value::str(&super::hstudio_crypt::base64_encode(&out)),
        Value::str(&super::hstudio_crypt::base64_encode(&iv)),
    ])
}

/// `crypt.decrypt(data, key, iv, mode?)`: the plaintext from Base64 inputs.
fn c_decrypt(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let data = b64_arg(lua, &args, 0, "decrypt")?;
    let key = b64_arg(lua, &args, 1, "decrypt")?;
    let mode = mode_arg(lua, &args, 3, "decrypt")?;
    let iv = iv_sized(b64_arg(lua, &args, 2, "decrypt")?, mode);
    let out =
        super::hstudio_crypt::decrypt(mode, &key, &iv, &data).map_err(|e| lua.rt_error(format!("decrypt: {e}")))?;
    one(Value::bytes(&out))
}

// ---------------------------------------------------------------- the collector

fn getgc(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    lua.track_objects(true);
    let mut out: Vec<Value> = lua.live_closures().into_iter().map(Value::Func).collect();
    if args.first().is_some_and(Value::truthy) {
        out.extend(lua.live_tables().into_iter().map(Value::Table));
    }
    lua.charge(out.len() * 16)?;
    one(Value::table(Table::from_list(out)))
}

fn getreg(lua: &mut Lua, _: &mut dyn Host, _: &Native, _: Vec<Value>) -> Res {
    lua.track_objects(true);
    let mut out: Vec<Value> = lua.live_threads().into_iter().map(Value::Thread).collect();
    out.extend(lua.live_closures().into_iter().map(Value::Func));
    one(Value::table(Table::from_list(out)))
}

/// Whether `t` holds every item of `want` (`check` says how one matches).
fn all_in(want: &Value, check: impl Fn(&Value, &Value) -> bool) -> bool {
    match want {
        Value::Table(w) => w.borrow().entries().iter().all(|(k, v)| check(k, v)),
        _ => true,
    }
}

fn filtergc(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    lua.track_objects(true);
    let kind = check_str(lua, &args, 0, "filtergc")?;
    let opts = check_table(lua, &args, 1, "filtergc")?;
    let first = args.get(2).is_some_and(Value::truthy);
    let o = |k: &str| opts.borrow().get_str(k);
    let mut hits: Vec<Value> = Vec::new();
    match &*kind {
        b"function" => {
            let name = o("Name").as_text();
            let ignore_exec = !matches!(o("IgnoreExecutor"), Value::Bool(false));
            let hash = o("Hash").as_text();
            let consts = o("Constants");
            let ups = o("Upvalues");
            for c in lua.live_closures() {
                // Every Lua function here is a mod's: the executor's own.
                if ignore_exec {
                    break;
                }
                let p = c.proto();
                if let Some(n) = &name
                    && p.debug_name.as_deref() != Some(n.as_str())
                {
                    continue;
                }
                if let Some(h) = &hash
                    && !function_hash(&p).eq_ignore_ascii_case(h)
                {
                    continue;
                }
                let slots: Vec<Value> = p.constant_slots().into_iter().flatten().collect();
                if !all_in(&consts, |_, want| slots.iter().any(|s| s.raw_eq(want))) {
                    continue;
                }
                let values: Vec<Value> = (0..c.body().upvals.len()).filter_map(|i| c.upvalue(i)).collect();
                if !all_in(&ups, |_, want| values.iter().any(|s| s.raw_eq(want))) {
                    continue;
                }
                hits.push(Value::Func(c));
                if first {
                    break;
                }
            }
        }
        b"table" => {
            let keys = o("Keys");
            let vals = o("Values");
            let pairs = o("KeyValuePairs");
            let meta = o("Metatable");
            for t in lua.live_tables() {
                if Rc::ptr_eq(&t, &opts) {
                    continue;
                }
                let tb = t.borrow();
                if !all_in(&keys, |_, k| !tb.get(k).is_nil()) {
                    continue;
                }
                if let Value::Table(w) = &vals {
                    let have: Vec<Value> = tb.entries().into_iter().map(|e| e.1).collect();
                    if !w.borrow().list().iter().all(|v| have.iter().any(|h| h.raw_eq(v))) {
                        continue;
                    }
                }
                if !all_in(&pairs, |k, v| tb.get(k).raw_eq(v)) {
                    continue;
                }
                if let Value::Table(m) = &meta
                    && !tb.meta.as_ref().is_some_and(|tm| Rc::ptr_eq(tm, m))
                {
                    continue;
                }
                // The filter's own tables aren't what it looks for.
                if [&keys, &vals, &pairs]
                    .iter()
                    .any(|f| matches!(f, Value::Table(x) if Rc::ptr_eq(x, &t)))
                {
                    continue;
                }
                drop(tb);
                hits.push(Value::Table(t));
                if first {
                    break;
                }
            }
        }
        _ => return Err(bad_arg(lua, 0, "filtergc", "\"function\" or \"table\" expected")),
    }
    if first {
        return one(hits.into_iter().next().unwrap_or_default());
    }
    one(Value::table(Table::from_list(hits)))
}

// ---------------------------------------------------------------- the rest

fn identifyexecutor(_: &mut Lua, _: &mut dyn Host, _: &Native, _: Vec<Value>) -> Res {
    Ok(vec![Value::str(EXECUTOR), Value::str(super::hstudio::VERSION)])
}

fn getthreadidentity(lua: &mut Lua, _: &mut dyn Host, _: &Native, _: Vec<Value>) -> Res {
    one(Value::Num(identity(lua) as f64))
}

fn setthreadidentity(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let id = check_int(lua, &args, 0, "setthreadidentity")?.clamp(0, 8);
    let key = Rc::as_ptr(&lua.current_thread()) as usize;
    IDENTITIES.with(|m| {
        let mut m = m.borrow_mut();
        // Dead threads' entries go now and then.
        if m.len() > 4096 {
            m.clear();
        }
        m.insert(key, id);
    });
    none()
}

fn newproxy(_: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let meta = args
        .first()
        .is_some_and(Value::truthy)
        .then(|| Rc::new(RefCell::new(Table::new())));
    one(Userdata::value((), meta))
}

fn getfenv(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let env = match args.first() {
        None | Some(Value::Nil) => caller_env(lua),
        Some(Value::Num(n)) if *n == 0.0 => caller_env(lua),
        Some(Value::Func(c)) => Some(c.env()),
        Some(Value::Native(_)) => caller_env(lua),
        Some(Value::Num(n)) => lua.frame_info(*n as usize).map(|f| f.closure.env()),
        _ => return Err(bad_arg(lua, 0, "getfenv", "function or level expected")),
    };
    one(env.map_or(Value::Nil, Value::Table))
}

fn setfenv(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let env = check_table(lua, &args, 1, "setfenv")?;
    let c = match args.first() {
        Some(Value::Func(c)) => c.clone(),
        Some(Value::Num(n)) => lua
            .frame_info((*n as usize).max(1))
            .map(|f| f.closure)
            .ok_or_else(|| bad_arg(lua, 0, "setfenv", "level out of range"))?,
        _ => return Err(bad_arg(lua, 0, "setfenv", "Lua function or level expected")),
    };
    let body = c.body();
    c.replace_body(Rc::new(Body {
        proto: body.proto.clone(),
        upvals: body.upvals.clone(),
        env,
    }));
    one(Value::Func(c))
}
