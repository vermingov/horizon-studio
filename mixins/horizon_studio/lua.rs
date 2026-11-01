// Horizon Studio's Lua: an interpreter for Lua 5.3 in plain Rust, with no
// dependencies, so the bundle can add it to the game as one module. A chunk
// compiles to register bytecode (a `Proto` for each function), which a VM
// runs with explicit call frames: a Lua function calling another pushes a
// frame instead of recursing on the game's stack, so each coroutine has a
// stack of its own and can stop anywhere and carry on later. Scripts get no
// file, network or process access: only the functions the studio hands
// them. Each call into a script runs under a step budget and a frame limit,
// so an endless loop or runaway recursion stops that script with an error
// instead of freezing the game.
//
// What is here: nil, booleans, numbers (64-bit floats, printed as integers
// when they are whole), byte strings, tables with metatables, closures with
// upvalues, coroutines, userdata, varargs and multiple results, every
// statement but goto, and every operator (the bitwise ones on whole
// numbers). The standard library is in hstudio_lualib. Not here: goto,
// integer subtypes, io, os beyond the clock, load of binary chunks,
// string.pack.

use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};
use std::rc::{Rc, Weak};

pub type Str = Rc<[u8]>;
pub type TableRef = Rc<RefCell<Table>>;
pub type Cell = Rc<RefCell<Value>>;
pub type Res = Result<Vec<Value>, Throw>;
/// A built-in function: the interpreter, the host, the function itself (its
/// `state` holds what an iterator keeps between calls) and the arguments.
pub type Builtin = fn(&mut Lua, &mut dyn Host, &Native, Vec<Value>) -> Res;

/// The longest string a script can make.
pub const MAX_STRING: usize = 4 << 20;
/// The most entries one table can hold: Luau's limit (2^26).
pub const MAX_TABLE: usize = 1 << 26;
/// Lua frames one coroutine may have before "stack overflow", as many as
/// Luau allows (they live on the heap, not the game's stack).
pub const MAX_DEPTH: usize = 20_000;
/// Natives calling back into Lua (metamethods, sort's comparator) and
/// coroutines resuming others, nested: each of those still recurses on the
/// game's stack.
pub const MAX_CCALLS: usize = 200;
/// The game's stack those nested calls may use below where the host called
/// in (Windows gives the main thread 1 MiB in all; unoptimised builds, the
/// tests, have much bigger frames).
pub const STACK_LIMIT: usize = if cfg!(debug_assertions) { 1536 << 10 } else { 256 << 10 };
/// Nested expressions and blocks the parser accepts.
const MAX_NESTING: u32 = 80;

/// Where the stack is now (it grows down on every platform the game runs on).
#[inline(always)]
fn stack_addr() -> usize {
    let marker = 0u8;
    std::hint::black_box(&marker) as *const u8 as usize
}

#[derive(Clone, Default)]
pub enum Value {
    #[default]
    Nil,
    Bool(bool),
    Num(f64),
    Str(Str),
    Table(TableRef),
    Func(Rc<Closure>),
    Native(Rc<Native>),
    Thread(Rc<Thread>),
    #[allow(dead_code)]
    Userdata(Rc<Userdata>),
}

impl Value {
    pub fn str(s: &str) -> Value {
        Value::Str(Rc::from(s.as_bytes()))
    }
    pub fn bytes(b: &[u8]) -> Value {
        Value::Str(Rc::from(b))
    }
    pub fn table(t: Table) -> Value {
        Value::Table(Rc::new(RefCell::new(t)))
    }
    pub fn is_nil(&self) -> bool {
        matches!(self, Value::Nil)
    }
    pub fn truthy(&self) -> bool {
        !matches!(self, Value::Nil | Value::Bool(false))
    }
    pub fn is_callable(&self) -> bool {
        matches!(self, Value::Func(_) | Value::Native(_))
    }
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Nil => "nil",
            Value::Bool(_) => "boolean",
            Value::Num(_) => "number",
            Value::Str(_) => "string",
            Value::Table(_) => "table",
            Value::Func(_) | Value::Native(_) => "function",
            Value::Thread(_) => "thread",
            Value::Userdata(u) if u.data.borrow().is::<LuaBuffer>() => "buffer",
            Value::Userdata(_) => "userdata",
        }
    }
    #[allow(dead_code)]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
    #[allow(dead_code)]
    pub fn as_num(&self) -> Option<f64> {
        match self {
            Value::Num(n) => Some(*n),
            _ => None,
        }
    }
    /// The string as text (invalid UTF-8 replaced), or None.
    #[allow(dead_code)]
    pub fn as_text(&self) -> Option<String> {
        self.as_bytes().map(|b| String::from_utf8_lossy(b).into_owned())
    }
    /// Numbers coerce from strings, as in arithmetic.
    pub fn to_num(&self) -> Option<f64> {
        match self {
            Value::Num(n) => Some(*n),
            Value::Str(s) => str_to_num(s),
            _ => None,
        }
    }
    pub fn raw_eq(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Num(a), Value::Num(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Table(a), Value::Table(b)) => Rc::ptr_eq(a, b),
            (Value::Func(a), Value::Func(b)) => Rc::ptr_eq(a, b),
            (Value::Native(a), Value::Native(b)) => Rc::ptr_eq(a, b),
            (Value::Thread(a), Value::Thread(b)) => Rc::ptr_eq(a, b),
            (Value::Userdata(a), Value::Userdata(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
    /// `tostring` without metamethods.
    pub fn plain_text(&self) -> String {
        match self {
            Value::Nil => "nil".into(),
            Value::Bool(b) => b.to_string(),
            Value::Num(n) => fmt_num(*n),
            Value::Str(s) => String::from_utf8_lossy(s).into_owned(),
            Value::Table(t) => format!("table: {:p}", Rc::as_ptr(t)),
            Value::Func(f) => format!("function: {:p}", Rc::as_ptr(f)),
            Value::Native(f) => format!("function: builtin '{}': {:p}", f.name, Rc::as_ptr(f)),
            Value::Thread(t) => format!("thread: {:p}", Rc::as_ptr(t)),
            Value::Userdata(u) => format!("userdata: {:p}", Rc::as_ptr(u)),
        }
    }
    /// The metatable of a table or a userdata.
    pub fn metatable(&self) -> Option<TableRef> {
        match self {
            Value::Table(t) => t.borrow().meta.clone(),
            Value::Userdata(u) => u.meta.borrow().clone(),
            _ => None,
        }
    }
}

/// A function the interpreter does not run itself: a built-in, or one the
/// host answers by its id.
pub struct Native {
    pub name: Rc<str>,
    pub kind: NativeKind,
    pub state: RefCell<Vec<Value>>,
    /// What a call runs instead (`hookfunction`).
    redirect: RefCell<Option<Value>>,
}

#[derive(Clone)]
pub enum NativeKind {
    Builtin(Builtin),
    Host(u32),
    /// `pcall` (false) or `xpcall` (true): the VM runs these itself, so a
    /// coroutine can yield inside them.
    Pcall(bool),
    /// A Lua function wrapped as a native (`newcclosure`): the VM runs the
    /// function in its place, so it can yield.
    Wrap(Value),
    /// The method call in progress (`getnamecallmethod`), looked up on the
    /// first argument and called with the rest: `__namecall` as it is
    /// without a hook.
    Namecall,
}

impl Native {
    pub fn builtin(name: &str, f: Builtin) -> Value {
        Native::with_state(name, f, Vec::new())
    }
    pub fn with_state(name: &str, f: Builtin, state: Vec<Value>) -> Value {
        Native::of(name, NativeKind::Builtin(f), state)
    }
    pub fn host(name: &str, id: u32) -> Value {
        Native::of(name, NativeKind::Host(id), Vec::new())
    }
    /// `pcall`, or `xpcall` with `handler`.
    pub fn pcall(name: &str, handler: bool) -> Value {
        Native::of(name, NativeKind::Pcall(handler), Vec::new())
    }
    /// A native of any kind.
    pub fn of(name: &str, kind: NativeKind, state: Vec<Value>) -> Value {
        Value::Native(Rc::new(Native {
            name: name.into(),
            kind,
            state: RefCell::new(state),
            redirect: RefCell::new(None),
        }))
    }
    /// What a call to it runs instead, if it is hooked.
    pub fn redirect(&self) -> Option<Value> {
        self.redirect.borrow().clone()
    }
    /// Hooks it (Some) or unhooks it (None); the hook before.
    pub fn set_redirect(&self, to: Option<Value>) -> Option<Value> {
        std::mem::replace(&mut *self.redirect.borrow_mut(), to)
    }
}

/// What the interpreter asks of the program that runs it.
pub trait Host {
    /// A function made with `Native::host`.
    fn call(&mut self, lua: &mut Lua, id: u32, args: Vec<Value>) -> Res;
    /// A line from `print`.
    fn print(&mut self, text: &str);
}

/// A host that offers nothing: `print` collects lines (tests, `load`).
#[derive(Default)]
pub struct NoHost {
    pub printed: Vec<String>,
}

impl Host for NoHost {
    fn call(&mut self, lua: &mut Lua, _id: u32, _args: Vec<Value>) -> Res {
        Err(lua.rt_error("this function is not available here"))
    }
    fn print(&mut self, text: &str) {
        self.printed.push(text.to_owned());
    }
}

/// How a call ended other than by returning.
pub enum Throw {
    /// An error a script can catch with pcall: its value and where it came from.
    Error(Value, String),
    /// Out of budget or memory: stops the script, pcall or not.
    Fatal(String),
    /// A native's way to yield its coroutine (`coroutine.yield`): the values
    /// go to the resumer, and what the coroutine is resumed with become the
    /// call's results. Only from a native that Lua code called directly.
    Yield(Vec<Value>),
}

impl Throw {
    /// The message, as the console shows it.
    pub fn message(&self) -> String {
        match self {
            Throw::Error(v, _) => match v {
                Value::Str(s) => String::from_utf8_lossy(s).into_owned(),
                Value::Nil => "nil".into(),
                other => format!("(error object is a {} value)", other.type_name()),
            },
            Throw::Fatal(m) => m.clone(),
            Throw::Yield(_) => "attempt to yield across a C-call boundary".into(),
        }
    }
    pub fn traceback(&self) -> &str {
        match self {
            Throw::Error(_, t) => t,
            Throw::Fatal(_) | Throw::Yield(_) => "",
        }
    }
}

/// What an index error says of what was indexed: its name where the code
/// names it (a local, a global, a field), else the key it was indexed with,
/// as Luau says (`with 'Name'`).
fn index_desc(p: &Proto, pc: usize, key: &Value) -> String {
    let d = p.desc(pc, 0);
    if !d.is_empty() {
        return d;
    }
    match key {
        Value::Str(_) => format!(" with '{}'", key.plain_text()),
        Value::Num(n) => format!(" with {}", fmt_num(*n)),
        _ => String::new(),
    }
}

/// A Luau buffer's bytes (`type` "buffer"), in a Userdata.
pub struct LuaBuffer(pub Vec<u8>);

/// A value a host makes for scripts: Rust data, a metatable (for its
/// methods and operators) and identity by pointer.
#[allow(dead_code)]
pub struct Userdata {
    pub data: RefCell<Box<dyn Any>>,
    pub meta: RefCell<Option<TableRef>>,
}

#[allow(dead_code)]
impl Userdata {
    /// A userdata value holding `data`.
    pub fn value<T: Any>(data: T, meta: Option<TableRef>) -> Value {
        Value::Userdata(Rc::new(Userdata {
            data: RefCell::new(Box::new(data)),
            meta: RefCell::new(meta),
        }))
    }
    /// Its metatable's `__type` (what `typeof` names it), if a string.
    pub fn type_name(&self) -> Option<String> {
        let mt = self.meta.borrow().clone()?;
        let t = mt.borrow().get_str("__type");
        t.as_text()
    }
}

/// What a coroutine is doing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    /// Made and not started, or yielded.
    Suspended,
    Running,
    /// It resumed another, which runs.
    Normal,
    Dead,
}

impl Status {
    pub fn name(self) -> &'static str {
        match self {
            Status::Suspended => "suspended",
            Status::Running => "running",
            Status::Normal => "normal",
            Status::Dead => "dead",
        }
    }
}

/// A coroutine (or the main thread): its stack and frames while it is not
/// the one running, and its status.
pub struct Thread {
    state: RefCell<Option<ThreadState>>,
    status: std::cell::Cell<Status>,
    main: bool,
    /// The error it died of, for `coroutine.close`.
    error: RefCell<Option<Value>>,
}

impl Thread {
    pub fn status(&self) -> Status {
        self.status.get()
    }
    #[allow(dead_code)]
    pub fn is_dead(&self) -> bool {
        self.status.get() == Status::Dead
    }
    #[allow(dead_code)]
    pub fn is_suspended(&self) -> bool {
        self.status.get() == Status::Suspended
    }
    pub fn is_main(&self) -> bool {
        self.main
    }
}

/// How `Lua::resume` came back.
pub enum Resume {
    /// The coroutine's function returned these.
    Done(Vec<Value>),
    /// It yielded these, and can be resumed.
    Yielded(Vec<Value>),
    /// It failed (and is dead), or could not be resumed.
    Error(Throw),
}

// ----------------------------------------------------------------- tables

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum Key {
    Bool(bool),
    Int(i64),
    Float(u64),
    Str(Str),
    Ptr(usize),
}

fn num_key(n: f64) -> Key {
    if n.fract() == 0.0 && (-9.223_372_036_854_776e18..9.223_372_036_854_776e18).contains(&n) {
        Key::Int(n as i64)
    } else {
        Key::Float(n.to_bits())
    }
}

fn key_of(v: &Value) -> Option<Key> {
    Some(match v {
        Value::Nil => return None,
        Value::Bool(b) => Key::Bool(*b),
        Value::Num(n) if n.is_nan() => return None,
        Value::Num(n) => num_key(*n),
        Value::Str(s) => Key::Str(s.clone()),
        Value::Table(t) => Key::Ptr(Rc::as_ptr(t) as *const u8 as usize),
        Value::Func(f) => Key::Ptr(Rc::as_ptr(f) as *const u8 as usize),
        Value::Native(f) => Key::Ptr(Rc::as_ptr(f) as *const u8 as usize),
        Value::Thread(t) => Key::Ptr(Rc::as_ptr(t) as *const u8 as usize),
        Value::Userdata(u) => Key::Ptr(Rc::as_ptr(u) as *const u8 as usize),
    })
}

/// A quick hash for table keys (FxHash's step). Tables only need keys spread
/// over buckets, and SipHash costs more than the rest of a lookup.
#[derive(Default, Clone, Copy)]
struct KeyHasher(u64);

impl KeyHasher {
    #[inline]
    fn add(&mut self, w: u64) {
        self.0 = (self.0.rotate_left(5) ^ w).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

impl Hasher for KeyHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0.rotate_left(26)
    }
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let (chunks, rest) = bytes.as_chunks::<8>();
        for c in chunks {
            self.add(u64::from_le_bytes(*c));
        }
        if !rest.is_empty() {
            let mut w = 0u64;
            for (i, b) in rest.iter().enumerate() {
                w |= u64::from(*b) << (8 * i);
            }
            self.add(w);
        }
    }
    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add(u64::from(i));
    }
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add(i);
    }
    #[inline]
    fn write_i64(&mut self, i: i64) {
        self.add(i as u64);
    }
    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add(i as u64);
    }
    #[inline]
    fn write_isize(&mut self, i: isize) {
        self.add(i as u64);
    }
}

type KeyMap = HashMap<Key, usize, BuildHasherDefault<KeyHasher>>;

const READONLY: &str = "attempt to modify a readonly table";
/// Keys a table's hash part holds before it gets an index: up to this
/// many, a lookup scans them (most tables are small objects).
const SMALL: usize = 8;

/// A table: an array part for 1..n and an insertion-ordered hash part, so
/// `pairs` visits entries in a stable order. The hash part has an index once
/// it holds more than SMALL entries.
#[derive(Default)]
pub struct Table {
    arr: Vec<Value>,
    hash: Vec<(Key, Value, Value)>,
    index: KeyMap,
    dead: usize,
    readonly: bool,
    pub meta: Option<TableRef>,
}

impl Table {
    pub fn new() -> Table {
        Table::default()
    }
    /// A table with room for `narr` items and `nhash` keys.
    pub fn with_capacity(narr: usize, nhash: usize) -> Table {
        Table {
            arr: Vec::with_capacity(narr.min(1024)),
            hash: Vec::with_capacity(nhash.min(1024)),
            index: if nhash > SMALL {
                KeyMap::with_capacity_and_hasher(nhash.min(1024), BuildHasherDefault::default())
            } else {
                KeyMap::default()
            },
            ..Table::default()
        }
    }
    pub fn from_list(items: Vec<Value>) -> Table {
        let mut t = Table::new();
        for v in items {
            t.push(v);
        }
        t
    }
    /// Frozen: every write fails with "attempt to modify a readonly table"
    /// (`table.freeze`, `setreadonly`).
    #[allow(dead_code)]
    pub fn set_readonly(&mut self, on: bool) {
        self.readonly = on;
    }
    pub fn is_readonly(&self) -> bool {
        self.readonly
    }
    /// Names the values this is the metatable of, for `typeof` (`__type`).
    #[allow(dead_code)]
    pub fn set_type_name(&mut self, name: &str) {
        self.set_str("__type", Value::str(name));
    }
    /// Where `k` is in the hash part.
    #[inline]
    fn find(&self, k: &Key) -> Option<usize> {
        if self.hash.len() > SMALL {
            self.index.get(k).copied()
        } else {
            self.hash.iter().position(|e| e.0 == *k)
        }
    }
    #[inline]
    fn get_key(&self, k: &Key) -> Value {
        if let Key::Int(i) = k
            && *i >= 1
            && (*i as usize) <= self.arr.len()
        {
            return self.arr[*i as usize - 1].clone();
        }
        match self.find(k) {
            Some(p) => self.hash[p].2.clone(),
            None => Value::Nil,
        }
    }
    #[inline]
    pub fn get(&self, k: &Value) -> Value {
        if let Value::Num(n) = k
            && *n >= 1.0
            && *n <= self.arr.len() as f64
            && n.fract() == 0.0
        {
            return self.arr[*n as usize - 1].clone();
        }
        match key_of(k) {
            Some(k) => self.get_key(&k),
            None => Value::Nil,
        }
    }
    pub fn get_str(&self, k: &str) -> Value {
        self.get_key(&Key::Str(Rc::from(k.as_bytes())))
    }
    pub fn get_int(&self, i: i64) -> Value {
        self.get_key(&Key::Int(i))
    }
    pub fn set_str(&mut self, k: &str, v: Value) {
        let _ = self.set(Value::str(k), v);
    }
    /// Appends at `#t + 1`.
    pub fn push(&mut self, v: Value) {
        let n = self.len() as f64 + 1.0;
        let _ = self.set(Value::Num(n), v);
    }
    fn size(&self) -> usize {
        self.arr.len() + self.hash.len() - self.dead
    }
    pub fn set(&mut self, k: Value, v: Value) -> Result<(), &'static str> {
        let key = match key_of(&k) {
            Some(key) => key,
            None if k.is_nil() => return Err("index is nil"),
            None => return Err("index is NaN"),
        };
        self.store(key, k, v)
    }
    /// `set` with the key made already.
    fn store(&mut self, key: Key, k: Value, v: Value) -> Result<(), &'static str> {
        if self.readonly {
            return Err(READONLY);
        }
        if let Key::Int(i) = key {
            if i >= 1 && (i as usize) <= self.arr.len() {
                self.arr[i as usize - 1] = v;
                return Ok(());
            }
            if i >= 1 && i as usize == self.arr.len() + 1 && !v.is_nil() {
                if self.size() >= MAX_TABLE {
                    return Err("table is too large");
                }
                self.kill(&key);
                self.arr.push(v);
                // Keys stored in the hash part while the array was shorter.
                loop {
                    let next = Key::Int(self.arr.len() as i64 + 1);
                    match self.find(&next) {
                        Some(p) if !self.hash[p].2.is_nil() => {
                            let moved = std::mem::take(&mut self.hash[p].2);
                            self.dead += 1;
                            self.arr.push(moved);
                        }
                        _ => break,
                    }
                }
                return Ok(());
            }
        }
        if let Some(p) = self.find(&key) {
            let was_nil = self.hash[p].2.is_nil();
            match (was_nil, v.is_nil()) {
                (false, true) => self.dead += 1,
                (true, false) => self.dead -= 1,
                _ => {}
            }
            self.hash[p].2 = v;
            return Ok(());
        }
        if v.is_nil() {
            return Ok(());
        }
        if self.size() >= MAX_TABLE {
            return Err("table is too large");
        }
        if self.dead > 32 && self.dead * 2 > self.hash.len() {
            self.compact();
        }
        let at = self.hash.len();
        if at >= SMALL {
            if at == SMALL {
                for (i, e) in self.hash.iter().enumerate() {
                    self.index.insert(e.0.clone(), i);
                }
            }
            self.index.insert(key.clone(), at);
        }
        self.hash.push((key, k, v));
        Ok(())
    }
    fn kill(&mut self, key: &Key) {
        if let Some(p) = self.find(key)
            && !self.hash[p].2.is_nil()
        {
            self.hash[p].2 = Value::Nil;
            self.dead += 1;
        }
    }
    fn compact(&mut self) {
        self.hash.retain(|e| !e.2.is_nil());
        self.index.clear();
        if self.hash.len() > SMALL {
            for (i, e) in self.hash.iter().enumerate() {
                self.index.insert(e.0.clone(), i);
            }
        }
        self.dead = 0;
    }
    /// A border: `t[n]` is not nil and `t[n + 1]` is (0 for an empty array).
    pub fn len(&self) -> usize {
        let mut n = self.arr.len();
        while n > 0 && self.arr[n - 1].is_nil() {
            n -= 1;
        }
        n
    }
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.size() == 0
    }
    /// `next`: the entry after `k` (nil: the first), None at the end, Err for
    /// a key the table doesn't hold.
    #[allow(clippy::result_unit_err)]
    pub fn next(&self, k: &Value) -> Result<Option<(Value, Value)>, ()> {
        let mut from_arr = 0;
        let mut from_hash = 0;
        if !k.is_nil() {
            let key = key_of(k).ok_or(())?;
            match key {
                Key::Int(i) if i >= 1 && (i as usize) <= self.arr.len() => from_arr = i as usize,
                _ => {
                    from_arr = self.arr.len();
                    from_hash = self.find(&key).ok_or(())? + 1;
                }
            }
        }
        for j in from_arr..self.arr.len() {
            if !self.arr[j].is_nil() {
                return Ok(Some((Value::Num(j as f64 + 1.0), self.arr[j].clone())));
            }
        }
        for e in &self.hash[from_hash.min(self.hash.len())..] {
            if !e.2.is_nil() {
                return Ok(Some((e.1.clone(), e.2.clone())));
            }
        }
        Ok(None)
    }
    /// Every entry, array part first, as `pairs` visits them.
    pub fn entries(&self) -> Vec<(Value, Value)> {
        let mut out = Vec::new();
        for (j, v) in self.arr.iter().enumerate() {
            if !v.is_nil() {
                out.push((Value::Num(j as f64 + 1.0), v.clone()));
            }
        }
        for e in &self.hash {
            if !e.2.is_nil() {
                out.push((e.1.clone(), e.2.clone()));
            }
        }
        out
    }
    /// `t[1..=#t]`.
    pub fn list(&self) -> Vec<Value> {
        self.arr[..self.len()].to_vec()
    }
    /// Replaces `t[1..=n]` with `items`, as table.sort writes back.
    pub fn set_list(&mut self, items: Vec<Value>) {
        for (i, v) in items.into_iter().enumerate() {
            let _ = self.set(Value::Num(i as f64 + 1.0), v);
        }
    }
    /// Inserts at position `pos` (1-based), shifting the rest up.
    pub fn insert(&mut self, pos: usize, v: Value) -> Result<(), &'static str> {
        if self.readonly {
            return Err(READONLY);
        }
        let n = self.len();
        if pos == n + 1 {
            return self.set(Value::Num(pos as f64), v);
        }
        if self.size() >= MAX_TABLE {
            return Err("table is too large");
        }
        if n == self.arr.len() {
            self.arr.insert(pos - 1, v);
        } else {
            self.arr.truncate(n);
            self.arr.insert(pos - 1, v);
        }
        Ok(())
    }
    /// Removes position `pos` (1-based) of the array part, shifting the rest
    /// down (nothing, on a readonly table).
    pub fn remove(&mut self, pos: usize) -> Value {
        let n = self.len();
        if pos == 0 || pos > n || self.readonly {
            return Value::Nil;
        }
        self.arr.truncate(n);
        self.arr.remove(pos - 1)
    }
}

// ----------------------------------------------------------------- lexer

#[derive(Clone, PartialEq, Debug)]
enum Tok {
    Name(Rc<str>),
    Num(f64),
    Str(Str),
    /// A keyword or a symbol.
    Op(&'static str),
    /// Luau's interpolated strings, `` `a{x}b{y}c` ``: the text before the
    /// first expression, between two, and after the last.
    InterpBegin(Str),
    InterpMid(Str),
    InterpEnd(Str),
    Eof,
}

const KEYWORDS: [&str; 22] = [
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in", "local", "nil",
    "not", "or", "repeat", "return", "then", "true", "until", "while",
];

// Longest first, so `...` wins over `..` and `.`. Luau adds the compound
// assignments, `->` and `?` (in types) and `@` (attributes).
const SYMBOLS: [&str; 44] = [
    "...", "..=", "//=", "..", "==", "~=", "<=", ">=", "//", "::", "<<", ">>", "+=", "-=", "*=", "/=", "%=", "^=",
    "->", "+", "-", "*", "/", "%", "^", "#", "&", "~", "|", "<", ">", "=", "(", ")", "{", "}", "[", "]", ";", ":", ",",
    ".", "@", "?",
];

fn tok_text(t: &Tok) -> String {
    match t {
        Tok::Name(n) => n.to_string(),
        Tok::Num(n) => fmt_num(*n),
        Tok::Str(s) => String::from_utf8_lossy(s).into_owned(),
        Tok::Op(o) => (*o).to_owned(),
        Tok::InterpBegin(s) | Tok::InterpMid(s) | Tok::InterpEnd(s) => format!("`{}", String::from_utf8_lossy(s)),
        Tok::Eof => "<eof>".into(),
    }
}

/// The tokens with their lines, and where each starts (a byte of the source).
type Lexed = (Vec<(Tok, u32)>, Vec<usize>);

fn lex(src: &[u8], chunk: &str) -> Result<Lexed, String> {
    let mut out = Vec::new();
    // Where each token starts, for the editor's marks.
    let mut starts: Vec<usize> = Vec::new();
    let mut i = 0;
    let mut line = 1u32;
    let err = |line: u32, msg: &str| format!("{chunk}:{line}: {msg}");
    let mark = |at: usize| SYNTAX_AT.with(|s| s.set(Some(at)));
    // The interpolated strings open around here: the braces open inside the
    // expression of each (a `}` at 0 goes back to its string).
    let mut braces: Vec<u32> = Vec::new();
    // A `#!` first line, as lua allows.
    if src.starts_with(b"#!") {
        while i < src.len() && src[i] != b'\n' {
            i += 1;
        }
    }
    while i < src.len() {
        let c = src[i];
        if c == b'\n' {
            line += 1;
            i += 1;
            continue;
        }
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if c == b'-' && src.get(i + 1) == Some(&b'-') {
            i += 2;
            if src.get(i) == Some(&b'[')
                && let Some(level) = long_bracket(src, i)
            {
                let (_, end, lines) = long_string(src, i, level).ok_or_else(|| err(line, "unfinished long comment"))?;
                line += lines;
                i = end;
                continue;
            }
            while i < src.len() && src[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < src.len() && (src[i].is_ascii_alphanumeric() || src[i] == b'_') {
                i += 1;
            }
            let word = std::str::from_utf8(&src[start..i]).unwrap_or("");
            starts.push(start);
            match KEYWORDS.iter().find(|k| **k == word) {
                Some(k) => out.push((Tok::Op(k), line)),
                None => out.push((Tok::Name(word.into()), line)),
            }
            continue;
        }
        if c.is_ascii_digit() || (c == b'.' && src.get(i + 1).is_some_and(u8::is_ascii_digit)) {
            let start = i;
            if c == b'0' && matches!(src.get(i + 1), Some(b'x' | b'X' | b'b' | b'B')) {
                i += 2;
                while i < src.len() && (src[i].is_ascii_hexdigit() || src[i] == b'_') {
                    i += 1;
                }
            } else {
                while i < src.len() {
                    let d = src[i];
                    let exp_sign = matches!(d, b'+' | b'-') && matches!(src[i - 1], b'e' | b'E');
                    if d.is_ascii_alphanumeric() || d == b'.' || d == b'_' || exp_sign {
                        i += 1;
                    } else {
                        break;
                    }
                }
            }
            let text = &src[start..i];
            // Luau's `1_000_000` and `0b1010`.
            let plain: Vec<u8> = text.iter().copied().filter(|&b| b != b'_').collect();
            let n = match plain.get(..2) {
                Some(b"0b" | b"0B") => bin_to_num(&plain[2..]),
                _ => str_to_num(&plain),
            };
            let n = n.ok_or_else(|| {
                mark(start);
                err(
                    line,
                    &format!("malformed number near '{}'", String::from_utf8_lossy(text)),
                )
            })?;
            starts.push(start);
            out.push((Tok::Num(n), line));
            continue;
        }
        if c == b'"' || c == b'\'' {
            let (s, end, lines) = short_string(src, i).map_err(|m| {
                mark(i);
                err(line, m)
            })?;
            starts.push(i);
            out.push((Tok::Str(s.into()), line));
            line += lines;
            i = end;
            continue;
        }
        // A backtick string, or the rest of one after an expression in it.
        let resume = c == b'}' && braces.last() == Some(&0);
        if c == b'`' || resume {
            if resume {
                braces.pop();
            }
            let (s, end, lines) = string_body(src, i + 1, b"`{").map_err(|m| {
                mark(i);
                err(line, m)
            })?;
            starts.push(i);
            let open = src[end] == b'{';
            if open && src.get(end + 1) == Some(&b'{') {
                mark(end);
                return Err(err(
                    line,
                    "double braces are not permitted within interpolated strings; did you mean '\\{'?",
                ));
            }
            let s: Str = s.into();
            out.push((
                match (resume, open) {
                    (false, false) => Tok::Str(s),
                    (false, true) => Tok::InterpBegin(s),
                    (true, true) => Tok::InterpMid(s),
                    (true, false) => Tok::InterpEnd(s),
                },
                line,
            ));
            if open {
                braces.push(0);
            }
            line += lines;
            i = end + 1;
            continue;
        }
        if c == b'{'
            && let Some(depth) = braces.last_mut()
        {
            *depth += 1;
        }
        if c == b'}'
            && let Some(depth) = braces.last_mut()
        {
            *depth -= 1;
        }
        if c == b'['
            && let Some(level) = long_bracket(src, i)
        {
            let (s, end, lines) = long_string(src, i, level).ok_or_else(|| {
                mark(i);
                err(line, "unfinished long string")
            })?;
            starts.push(i);
            out.push((Tok::Str(s.into()), line));
            line += lines;
            i = end;
            continue;
        }
        match SYMBOLS.iter().find(|s| src[i..].starts_with(s.as_bytes())) {
            Some(s) => {
                starts.push(i);
                out.push((Tok::Op(s), line));
                i += s.len();
            }
            None => {
                mark(i);
                return Err(err(
                    line,
                    &format!("unexpected symbol near '{}'", String::from_utf8_lossy(&src[i..i + 1])),
                ));
            }
        }
    }
    if !braces.is_empty() {
        return Err(err(line, "unfinished interpolated string"));
    }
    starts.push(src.len());
    out.push((Tok::Eof, line));
    Ok((out, starts))
}

thread_local! {
    /// Where the last syntax error was, a byte of the source.
    static SYNTAX_AT: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

/// Where the last chunk that failed to load on this thread failed: a byte of
/// its source (the token the parser stopped at), for the editor's marks.
pub fn syntax_error_at() -> Option<usize> {
    SYNTAX_AT.with(std::cell::Cell::get)
}

/// `[[`, `[=[`...: the level, if `src[i..]` opens a long bracket.
fn long_bracket(src: &[u8], i: usize) -> Option<usize> {
    let mut j = i + 1;
    while src.get(j) == Some(&b'=') {
        j += 1;
    }
    (src.get(j) == Some(&b'[')).then_some(j - i - 1)
}

/// A long string from its opening bracket: its text, the index after it and
/// the newlines in it. The first newline right after the bracket is dropped.
fn long_string(src: &[u8], i: usize, level: usize) -> Option<(Vec<u8>, usize, u32)> {
    let mut j = i + level + 2;
    let mut lines = 0;
    if src.get(j) == Some(&b'\r') {
        j += 1;
    }
    if src.get(j) == Some(&b'\n') {
        j += 1;
        lines += 1;
    }
    let start = j;
    while j < src.len() {
        if src[j] == b']' {
            let mut k = j + 1;
            while src.get(k) == Some(&b'=') {
                k += 1;
            }
            if k - j - 1 == level && src.get(k) == Some(&b']') {
                return Some((src[start..j].to_vec(), k + 1, lines));
            }
        }
        if src[j] == b'\n' {
            lines += 1;
        }
        j += 1;
    }
    None
}

fn short_string(src: &[u8], i: usize) -> Result<(Vec<u8>, usize, u32), &'static str> {
    let (out, end, lines) = string_body(src, i + 1, &[src[i]])?;
    Ok((out, end + 1, lines))
}

/// A quoted string's text from `j` up to the first of `stop` (not escaped):
/// the text, where that stop is, and the newlines in it.
fn string_body(src: &[u8], mut j: usize, stop: &[u8]) -> Result<(Vec<u8>, usize, u32), &'static str> {
    let mut out = Vec::new();
    let mut lines = 0;
    loop {
        let Some(&c) = src.get(j) else {
            return Err("unfinished string");
        };
        if stop.contains(&c) {
            return Ok((out, j, lines));
        }
        if c == b'\n' {
            return Err("unfinished string");
        }
        if c != b'\\' {
            out.push(c);
            j += 1;
            continue;
        }
        j += 1;
        let Some(&e) = src.get(j) else {
            return Err("unfinished string");
        };
        j += 1;
        match e {
            b'n' => out.push(b'\n'),
            b't' => out.push(b'\t'),
            b'r' => out.push(b'\r'),
            b'a' => out.push(7),
            b'b' => out.push(8),
            b'f' => out.push(12),
            b'v' => out.push(11),
            b'\\' => out.push(b'\\'),
            b'"' => out.push(b'"'),
            b'\'' => out.push(b'\''),
            b'`' => out.push(b'`'),
            b'{' => out.push(b'{'),
            b'\n' => {
                out.push(b'\n');
                lines += 1;
            }
            b'x' => {
                let hex = src.get(j..j + 2).ok_or("hexadecimal digit expected")?;
                let s = std::str::from_utf8(hex).map_err(|_| "hexadecimal digit expected")?;
                out.push(u8::from_str_radix(s, 16).map_err(|_| "hexadecimal digit expected")?);
                j += 2;
            }
            b'z' => {
                while j < src.len() && src[j].is_ascii_whitespace() {
                    if src[j] == b'\n' {
                        lines += 1;
                    }
                    j += 1;
                }
            }
            b'u' => {
                if src.get(j) != Some(&b'{') {
                    return Err("missing '{' in \\u{xxxx}");
                }
                let end = src[j..]
                    .iter()
                    .position(|&b| b == b'}')
                    .ok_or("missing '}' in \\u{xxxx}")?;
                let s = std::str::from_utf8(&src[j + 1..j + end]).map_err(|_| "bad \\u escape")?;
                let code = u32::from_str_radix(s, 16).map_err(|_| "hexadecimal digit expected")?;
                let ch = char::from_u32(code).ok_or("UTF-8 value too large")?;
                let mut buf = [0; 4];
                out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                j += end + 1;
            }
            d if d.is_ascii_digit() => {
                let mut v = u32::from(d - b'0');
                for _ in 0..2 {
                    match src.get(j) {
                        Some(&d) if d.is_ascii_digit() => {
                            v = v * 10 + u32::from(d - b'0');
                            j += 1;
                        }
                        _ => break,
                    }
                }
                out.push(u8::try_from(v).map_err(|_| "decimal escape too large")?);
            }
            _ => return Err("invalid escape sequence"),
        }
    }
}

/// Luau's binary literal's digits (after `0b`).
fn bin_to_num(digits: &[u8]) -> Option<f64> {
    if digits.is_empty() || !digits.iter().all(|b| matches!(b, b'0' | b'1')) {
        return None;
    }
    Some(digits.iter().fold(0f64, |v, b| v * 2.0 + f64::from(b - b'0')))
}

/// Lua's `tonumber` of a string: decimal, exponent, hexadecimal integers.
pub fn str_to_num(s: &[u8]) -> Option<f64> {
    let t = std::str::from_utf8(s).ok()?.trim();
    let (neg, body) = match t.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    if let Some(hex) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let mut v = 0f64;
        for b in hex.bytes() {
            v = v * 16.0 + f64::from((b as char).to_digit(16)?);
        }
        return Some(if neg { -v } else { v });
    }
    if body.is_empty()
        || !body
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-'))
        || !body.bytes().any(|b| b.is_ascii_digit())
    {
        return None;
    }
    let v: f64 = body.parse().ok()?;
    Some(if neg { -v } else { v })
}

/// A number as Lua prints it: whole numbers without a fraction, others with
/// 14 significant digits.
pub fn fmt_num(n: f64) -> String {
    if n.is_nan() {
        return if n.is_sign_negative() {
            "-nan".into()
        } else {
            "nan".into()
        };
    }
    if n.is_infinite() {
        return if n > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if n.fract() == 0.0 && n.abs() < 1e15 {
        return format!("{}", n as i64);
    }
    fmt_g(n, 14, false)
}

/// C's `%.<prec>g`.
pub fn fmt_g(n: f64, prec: usize, alt: bool) -> String {
    if n == 0.0 {
        return if n.is_sign_negative() { "-0".into() } else { "0".into() };
    }
    if !n.is_finite() {
        return fmt_num(n);
    }
    let p = prec.max(1);
    let sci = format!("{:.*e}", p - 1, n);
    let (mant, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let trim = |s: &str| -> String {
        if alt || !s.contains('.') {
            s.to_owned()
        } else {
            s.trim_end_matches('0').trim_end_matches('.').to_owned()
        }
    };
    if exp < -4 || exp >= p as i32 {
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{}e{sign}{:02}", trim(mant), exp.abs())
    } else {
        let decimals = (p as i32 - 1 - exp).max(0) as usize;
        trim(&format!("{n:.decimals$}"))
    }
}

// ----------------------------------------------------------------- syntax tree

/// A local's number in its function, in declaration order.
type VarId = usize;

/// A local, as the parser saw it.
struct VarAst {
    name: Rc<str>,
    /// A closure uses it.
    captured: bool,
    /// Assigned to after its declaration.
    written: bool,
}

/// An upvalue as the parser resolved it: `index` is a local of the
/// enclosing function (`from_local`) or one of its upvalues.
#[derive(Clone)]
pub struct UpDesc {
    pub name: Rc<str>,
    pub from_local: bool,
    pub index: usize,
}

/// A function's syntax tree.
struct FuncAst {
    /// `function 'name'`, `function <main.lua:12>` or `main chunk`.
    name: Rc<str>,
    /// The name it was declared with (`local function f`, `function t.f`: f).
    debug_name: Option<Rc<str>>,
    line: u32,
    nparams: usize,
    vararg: bool,
    /// Every local, by id; the parameters first.
    vars: Vec<VarAst>,
    upvals: Vec<UpDesc>,
    body: Vec<Stmt>,
}

struct Stmt {
    line: u32,
    kind: StmtKind,
}

enum StmtKind {
    Local(Vec<VarId>, Vec<Expr>),
    LocalFunction(VarId, Box<FuncAst>),
    /// `function name.path()`: the target and the function.
    Function(Expr, Box<FuncAst>),
    Assign(Vec<Expr>, Vec<Expr>),
    Call(Expr),
    Do(Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    Repeat(Vec<Stmt>, Expr),
    If(Vec<(Expr, Vec<Stmt>)>, Option<Vec<Stmt>>),
    /// The first of four hidden locals (start, limit, step, count) and the variable.
    NumFor {
        base: VarId,
        var: VarId,
        start: Expr,
        limit: Expr,
        step: Option<Expr>,
        body: Vec<Stmt>,
    },
    /// The first of three hidden locals (generator, state, control) and the variables.
    GenFor {
        base: VarId,
        vars: Vec<VarId>,
        exprs: Vec<Expr>,
        body: Vec<Stmt>,
    },
    Return(Vec<Expr>),
    Break,
    /// Luau's `continue`.
    Continue,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    IDiv,
    Mod,
    Pow,
    Concat,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    BAnd,
    BOr,
    BXor,
    Shl,
    Shr,
}

#[derive(Clone, Copy)]
enum UnOp {
    Neg,
    Not,
    Len,
    BNot,
}

/// A table constructor's item: `[k] = v`, `name = v` (a record) or `v`.
struct Item {
    key: Option<Expr>,
    value: Expr,
    record: bool,
}

enum Expr {
    Nil,
    True,
    False,
    Num(f64),
    Str(Str),
    Vararg,
    Function(Box<FuncAst>),
    Local(VarId, Rc<str>),
    Upval(usize, Rc<str>),
    Global(Str),
    Index(Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Method(Box<Expr>, Str, Vec<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Un(UnOp, Box<Expr>),
    Table(Vec<Item>),
    Paren(Box<Expr>),
    /// Luau's `if c then a else b` (an `elseif` is another one in the else).
    IfElse(Box<Expr>, Box<Expr>, Box<Expr>),
    /// One of the interpreter's own functions (`builtin_value`), as a constant.
    Builtin(&'static str),
}

impl Expr {
    fn is_multi(&self) -> bool {
        matches!(self, Expr::Call(..) | Expr::Method(..) | Expr::Vararg)
    }
    /// What error messages call it: ` (global 'x')` and the like.
    fn desc(&self) -> Desc {
        match self {
            Expr::Local(_, n) => Desc::Local(n.clone()),
            Expr::Upval(_, n) => Desc::Upval(n.clone()),
            Expr::Global(n) => Desc::Global(n.clone()),
            Expr::Index(_, k) => match &**k {
                Expr::Str(s) => Desc::Field(s.clone()),
                _ => Desc::None,
            },
            Expr::Str(_) | Expr::Num(_) => Desc::Constant,
            _ => Desc::None,
        }
    }
}

/// An operand's name in an error message.
#[derive(Clone, Debug, Default)]
enum Desc {
    #[default]
    None,
    Local(Rc<str>),
    Upval(Rc<str>),
    Global(Str),
    Field(Str),
    Constant,
}

impl Desc {
    fn text(&self) -> String {
        match self {
            Desc::None => String::new(),
            Desc::Local(n) => format!(" (local '{n}')"),
            Desc::Upval(n) => format!(" (upvalue '{n}')"),
            Desc::Global(n) => format!(" (global '{}')", String::from_utf8_lossy(n)),
            Desc::Field(n) => format!(" (field '{}')", String::from_utf8_lossy(n)),
            Desc::Constant => " (constant)".into(),
        }
    }
    fn is_none(&self) -> bool {
        matches!(self, Desc::None)
    }
}

// ----------------------------------------------------------------- parser

struct FuncState {
    actives: Vec<(Rc<str>, VarId)>,
    vars: Vec<VarAst>,
    blocks: Vec<usize>,
    upvals: Vec<UpDesc>,
    vararg: bool,
    loops: u32,
}

impl FuncState {
    fn new(vararg: bool) -> FuncState {
        FuncState {
            actives: Vec::new(),
            vars: Vec::new(),
            blocks: Vec::new(),
            upvals: Vec::new(),
            vararg,
            loops: 0,
        }
    }
}

enum Var {
    Local(VarId),
    Up(usize),
}

struct Parser {
    toks: Vec<(Tok, u32)>,
    /// Where each token starts in the source.
    starts: Vec<usize>,
    pos: usize,
    chunk: Rc<str>,
    fs: Vec<FuncState>,
    depth: u32,
    stack_base: usize,
    /// Globals the chunk assigns to (they are read with GetGlobal, not as imports).
    written: HashSet<Str>,
    /// The chunk names getfenv or setfenv: no imports at all.
    fenv: bool,
}

type PResult<T> = Result<T, String>;

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos].0
    }
    fn peek2(&self) -> &Tok {
        &self.toks[(self.pos + 1).min(self.toks.len() - 1)].0
    }
    fn line(&self) -> u32 {
        self.toks[self.pos].1
    }
    fn advance(&mut self) -> Tok {
        let t = self.toks[self.pos].0.clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        t
    }
    fn check(&self, op: &str) -> bool {
        matches!(self.peek(), Tok::Op(o) if *o == op)
    }
    fn accept(&mut self, op: &str) -> bool {
        if self.check(op) {
            self.advance();
            true
        } else {
            false
        }
    }
    fn error<T>(&self, msg: &str) -> PResult<T> {
        let at = self.starts.get(self.pos).copied();
        SYNTAX_AT.with(|s| s.set(at));
        Err(format!(
            "{}:{}: {} near '{}'",
            self.chunk,
            self.line(),
            msg,
            tok_text(self.peek())
        ))
    }
    fn expect(&mut self, op: &str, what: &str) -> PResult<()> {
        if self.accept(op) {
            Ok(())
        } else {
            self.error(&format!("'{op}' expected{what}"))
        }
    }
    /// `end` closing what opened at `line`.
    fn expect_match(&mut self, op: &str, opener: &str, line: u32) -> PResult<()> {
        if self.line() == line {
            self.expect(op, "")
        } else {
            self.expect(op, &format!(" (to close '{opener}' at line {line})"))
        }
    }
    fn name(&mut self) -> PResult<Rc<str>> {
        match self.peek().clone() {
            Tok::Name(n) => {
                self.advance();
                Ok(n)
            }
            _ => self.error("<name> expected"),
        }
    }
    fn enter(&mut self) -> PResult<()> {
        self.depth += 1;
        if self.depth > MAX_NESTING || self.stack_base.saturating_sub(stack_addr()) > STACK_LIMIT {
            return self.error("chunk has too many syntax levels");
        }
        Ok(())
    }
    fn fs(&mut self) -> &mut FuncState {
        self.fs.last_mut().expect("a function is being parsed")
    }
    fn open_scope(&mut self) {
        let n = self.fs().actives.len();
        self.fs().blocks.push(n);
    }
    fn close_scope(&mut self) {
        let fs = self.fs();
        let n = fs.blocks.pop().unwrap_or(0);
        fs.actives.truncate(n);
    }
    fn declare(&mut self, name: Rc<str>) -> VarId {
        let fs = self.fs();
        let id = fs.vars.len();
        fs.vars.push(VarAst {
            name: name.clone(),
            captured: false,
            written: false,
        });
        fs.actives.push((name, id));
        id
    }
    fn resolve(&mut self, level: usize, name: &str) -> Option<Var> {
        let fs = &self.fs[level];
        if let Some((_, v)) = fs.actives.iter().rev().find(|(n, _)| &**n == name) {
            return Some(Var::Local(*v));
        }
        if let Some(i) = fs.upvals.iter().position(|u| &*u.name == name) {
            return Some(Var::Up(i));
        }
        if level == 0 {
            return None;
        }
        let (from_local, index) = match self.resolve(level - 1, name)? {
            Var::Local(v) => {
                self.fs[level - 1].vars[v].captured = true;
                (true, v)
            }
            Var::Up(i) => (false, i),
        };
        let fs = &mut self.fs[level];
        fs.upvals.push(UpDesc {
            name: name.into(),
            from_local,
            index,
        });
        Some(Var::Up(fs.upvals.len() - 1))
    }
    fn var(&mut self, name: Rc<str>) -> Expr {
        let top = self.fs.len() - 1;
        match self.resolve(top, &name) {
            Some(Var::Local(s)) => Expr::Local(s, name),
            Some(Var::Up(i)) => Expr::Upval(i, name),
            None => {
                if &*name == "getfenv" || &*name == "setfenv" {
                    self.fenv = true;
                }
                Expr::Global(Rc::from(name.as_bytes()))
            }
        }
    }
    /// An assignment's target: the local it names is no longer a constant.
    fn written(&mut self, e: &Expr) {
        let mut level = self.fs.len() - 1;
        match e {
            Expr::Local(v, _) => self.fs[level].vars[*v].written = true,
            Expr::Upval(i, _) => {
                let mut i = *i;
                while level > 0 {
                    let (from_local, index) = {
                        let u = &self.fs[level].upvals[i];
                        (u.from_local, u.index)
                    };
                    level -= 1;
                    if from_local {
                        self.fs[level].vars[index].written = true;
                        return;
                    }
                    i = index;
                }
            }
            Expr::Global(n) => {
                self.written.insert(n.clone());
            }
            _ => {}
        }
    }

    fn chunk(&mut self) -> PResult<FuncAst> {
        self.fs.push(FuncState::new(true));
        let body = self.block()?;
        if *self.peek() != Tok::Eof {
            return self.error("'<eof>' expected");
        }
        let fs = self.fs.pop().expect("the chunk's function");
        Ok(FuncAst {
            name: "main chunk".into(),
            debug_name: None,
            line: 0,
            nparams: 0,
            vararg: true,
            vars: fs.vars,
            upvals: Vec::new(),
            body,
        })
    }
    fn block_follow(&self) -> bool {
        matches!(self.peek(), Tok::Eof | Tok::Op("end" | "else" | "elseif" | "until"))
    }
    fn block(&mut self) -> PResult<Vec<Stmt>> {
        self.open_scope();
        let b = self.statements()?;
        self.close_scope();
        Ok(b)
    }
    fn statements(&mut self) -> PResult<Vec<Stmt>> {
        self.enter()?;
        let mut out = Vec::new();
        while !self.block_follow() {
            if self.check("return") {
                let line = self.line();
                self.advance();
                let exprs = if self.block_follow() || self.check(";") {
                    Vec::new()
                } else {
                    self.exprlist()?
                };
                self.accept(";");
                if !self.block_follow() {
                    return self.error("'<eof>' expected");
                }
                out.push(Stmt {
                    line,
                    kind: StmtKind::Return(exprs),
                });
                break;
            }
            if let Some(s) = self.statement()? {
                out.push(s);
            }
        }
        self.depth -= 1;
        Ok(out)
    }
    fn loop_block(&mut self) -> PResult<Vec<Stmt>> {
        let depth = self.fs.len();
        self.fs().loops += 1;
        let b = self.block();
        // A function in it that failed to parse left its own state pushed.
        self.fs.truncate(depth);
        self.fs().loops -= 1;
        b
    }
    fn statement(&mut self) -> PResult<Option<Stmt>> {
        let line = self.line();
        let kind = match self.peek() {
            Tok::Op(";") => {
                self.advance();
                return Ok(None);
            }
            Tok::Op("if") => self.if_stat(line)?,
            Tok::Op("while") => {
                self.advance();
                let cond = self.expr()?;
                self.expect("do", "")?;
                let body = self.loop_block()?;
                self.expect_match("end", "while", line)?;
                StmtKind::While(cond, body)
            }
            Tok::Op("do") => {
                self.advance();
                let body = self.block()?;
                self.expect_match("end", "do", line)?;
                StmtKind::Do(body)
            }
            Tok::Op("for") => self.for_stat(line)?,
            Tok::Op("repeat") => {
                self.advance();
                self.fs().loops += 1;
                self.open_scope();
                let body = self.statements()?;
                self.expect_match("until", "repeat", line)?;
                let cond = self.expr()?;
                self.close_scope();
                self.fs().loops -= 1;
                StmtKind::Repeat(body, cond)
            }
            Tok::Op("function") => self.func_stat(line)?,
            Tok::Op("local") => {
                self.advance();
                if self.accept("function") {
                    let name = self.name()?;
                    let var = self.declare(name.clone());
                    let f = self.body(format!("function '{name}'"), Some(name), false, line)?;
                    StmtKind::LocalFunction(var, f)
                } else {
                    self.local_stat()?
                }
            }
            Tok::Op("break") => {
                self.advance();
                if self.fs().loops == 0 {
                    return Err(format!("{}:{line}: break outside a loop", self.chunk));
                }
                StmtKind::Break
            }
            Tok::Op("goto" | "::") => return self.error("goto and labels are not supported"),
            // Luau's attributes (`@native`, `@checked`) change nothing here.
            Tok::Op("@") => {
                self.advance();
                self.name()?;
                return Ok(None);
            }
            Tok::Name(n) if &**n == "continue" && self.ends_word() => {
                self.advance();
                if self.fs().loops == 0 {
                    return Err(format!("{}:{line}: continue outside a loop", self.chunk));
                }
                StmtKind::Continue
            }
            Tok::Name(n) if &**n == "type" && matches!(self.peek2(), Tok::Name(_)) => {
                self.advance();
                self.type_alias()?;
                return Ok(None);
            }
            Tok::Name(n) if &**n == "export" && matches!(self.peek2(), Tok::Name(t) if &**t == "type") => {
                self.advance();
                self.advance();
                self.type_alias()?;
                return Ok(None);
            }
            _ => self.expr_stat()?,
        };
        Ok(Some(Stmt { line, kind }))
    }
    /// Whether a contextual keyword (`continue`) stands alone: what follows
    /// can't go on an expression that starts with it.
    fn ends_word(&self) -> bool {
        !matches!(
            self.peek2(),
            Tok::Op("(" | "=" | "." | "[" | ":" | "," | "{" | "+=" | "-=" | "*=" | "/=" | "//=" | "%=" | "^=" | "..=")
                | Tok::Str(_)
                | Tok::InterpBegin(_)
        )
    }

    // --------------------------------------------------------- Luau's types
    // Types are read and dropped: they change nothing at run time.

    /// `type Name<T> = Type` (after `type`), or a `type function`.
    fn type_alias(&mut self) -> PResult<()> {
        if matches!(self.peek(), Tok::Op("function")) {
            return self.error("type functions are not supported");
        }
        self.name()?;
        if self.check("<") {
            self.generics()?;
        }
        self.expect("=", "")?;
        self.ty()
    }
    /// `<T, U..., V = number>`.
    fn generics(&mut self) -> PResult<()> {
        self.expect("<", "")?;
        loop {
            if self.close_angle() {
                return Ok(());
            }
            if self.check("...") {
                self.advance();
                self.ty()?;
            } else {
                self.name()?;
                self.accept("...");
                if self.accept("=") {
                    self.type_or_pack()?;
                }
            }
            if !self.accept(",") {
                if self.close_angle() {
                    return Ok(());
                }
                return self.error("'>' expected");
            }
        }
    }
    /// A `>` closing type arguments, also the first half of `>>` or `>=`.
    fn close_angle(&mut self) -> bool {
        match self.peek() {
            Tok::Op(">") => {
                self.advance();
                true
            }
            Tok::Op(">>") => {
                self.toks[self.pos].0 = Tok::Op(">");
                true
            }
            Tok::Op(">=") => {
                self.toks[self.pos].0 = Tok::Op("=");
                true
            }
            _ => false,
        }
    }
    /// A `: Type` annotation, if there is one.
    fn annotation(&mut self) -> PResult<()> {
        if self.accept(":") {
            self.type_or_pack()?;
        }
        Ok(())
    }
    /// A type, or a pack: `...T`, `T...`, `(A, B)`.
    fn type_or_pack(&mut self) -> PResult<()> {
        if self.accept("...") {
            return self.ty();
        }
        self.ty()?;
        self.accept("...");
        Ok(())
    }
    /// A type: unions and intersections of simple types, `?` for optional.
    fn ty(&mut self) -> PResult<()> {
        self.enter()?;
        if !self.accept("|") {
            self.accept("&");
        }
        loop {
            self.simple_type()?;
            while self.accept("?") {}
            if !self.accept("|") && !self.accept("&") {
                break;
            }
        }
        self.depth -= 1;
        Ok(())
    }
    fn simple_type(&mut self) -> PResult<()> {
        match self.peek().clone() {
            Tok::Op("nil" | "true" | "false") | Tok::Str(_) => {
                self.advance();
            }
            Tok::Name(n) if &*n == "typeof" && matches!(self.peek2(), Tok::Op("(")) => {
                self.advance();
                self.skip_balanced("(", ")")?;
            }
            Tok::Name(_) => {
                self.advance();
                while self.accept(".") {
                    self.name()?;
                }
                if self.check("<") {
                    self.advance();
                    loop {
                        if self.close_angle() {
                            break;
                        }
                        self.type_or_pack()?;
                        if !self.accept(",") {
                            if self.close_angle() {
                                break;
                            }
                            return self.error("'>' expected");
                        }
                    }
                }
            }
            Tok::Op("{") => {
                let line = self.line();
                self.advance();
                while !self.check("}") {
                    if self.accept("[") {
                        self.ty()?;
                        self.expect("]", "")?;
                        self.expect(":", "")?;
                        self.ty()?;
                    } else if matches!(self.peek(), Tok::Name(_)) && matches!(self.peek2(), Tok::Op(":")) {
                        self.advance();
                        self.advance();
                        self.ty()?;
                    } else if matches!(self.peek(), Tok::Name(m) if &**m == "read" || &**m == "write")
                        && matches!(self.peek2(), Tok::Name(_) | Tok::Op("["))
                    {
                        self.advance();
                        continue;
                    } else {
                        self.ty()?;
                    }
                    if !self.accept(",") && !self.accept(";") {
                        break;
                    }
                }
                self.expect_match("}", "{", line)?;
            }
            Tok::Op("<") => {
                self.generics()?;
                self.function_type()?;
            }
            Tok::Op("(") => self.function_type()?,
            Tok::Op("...") => {
                self.advance();
                self.ty()?;
            }
            _ => return self.error("type expected"),
        }
        Ok(())
    }
    /// `(A, b: B, ...C) -> R`, or a type in brackets (a pack without `->`).
    fn function_type(&mut self) -> PResult<()> {
        let line = self.line();
        self.expect("(", "")?;
        while !self.check(")") {
            if matches!(self.peek(), Tok::Name(_)) && matches!(self.peek2(), Tok::Op(":")) {
                self.advance();
                self.advance();
            }
            self.type_or_pack()?;
            if !self.accept(",") {
                break;
            }
        }
        self.expect_match(")", "(", line)?;
        if self.accept("->") {
            self.type_or_pack()?;
        }
        Ok(())
    }
    /// Tokens from an `open` to its matching `close`.
    fn skip_balanced(&mut self, open: &str, close: &str) -> PResult<()> {
        self.expect(open, "")?;
        let mut depth = 1;
        while depth > 0 {
            match self.peek() {
                Tok::Eof => return self.error(&format!("'{close}' expected")),
                Tok::Op(o) if *o == open => depth += 1,
                Tok::Op(o) if *o == close => depth -= 1,
                _ => {}
            }
            self.advance();
        }
        Ok(())
    }

    fn if_stat(&mut self, line: u32) -> PResult<StmtKind> {
        self.advance();
        let mut arms = Vec::new();
        let cond = self.expr()?;
        self.expect("then", "")?;
        arms.push((cond, self.block()?));
        let mut otherwise = None;
        loop {
            if self.accept("elseif") {
                let cond = self.expr()?;
                self.expect("then", "")?;
                arms.push((cond, self.block()?));
            } else if self.accept("else") {
                otherwise = Some(self.block()?);
                self.expect_match("end", "if", line)?;
                break;
            } else {
                self.expect_match("end", "if", line)?;
                break;
            }
        }
        Ok(StmtKind::If(arms, otherwise))
    }
    fn for_stat(&mut self, line: u32) -> PResult<StmtKind> {
        self.advance();
        let first = self.name()?;
        self.annotation()?;
        if self.accept("=") {
            let start = self.expr()?;
            self.expect(",", "")?;
            let limit = self.expr()?;
            let step = if self.accept(",") { Some(self.expr()?) } else { None };
            self.expect("do", "")?;
            self.open_scope();
            let base = self.declare("(for start)".into());
            for hidden in ["(for limit)", "(for step)", "(for count)"] {
                self.declare(hidden.into());
            }
            let var = self.declare(first);
            let body = self.loop_block()?;
            self.close_scope();
            self.expect_match("end", "for", line)?;
            return Ok(StmtKind::NumFor {
                base,
                var,
                start,
                limit,
                step,
                body,
            });
        }
        let mut names = vec![first];
        while self.accept(",") {
            names.push(self.name()?);
            self.annotation()?;
        }
        self.expect("in", "")?;
        let exprs = self.exprlist()?;
        self.expect("do", "")?;
        self.open_scope();
        let base = self.declare("(for generator)".into());
        self.declare("(for state)".into());
        self.declare("(for control)".into());
        let vars = names.into_iter().map(|n| self.declare(n)).collect();
        let body = self.loop_block()?;
        self.close_scope();
        self.expect_match("end", "for", line)?;
        Ok(StmtKind::GenFor {
            base,
            vars,
            exprs,
            body,
        })
    }
    fn func_stat(&mut self, line: u32) -> PResult<StmtKind> {
        self.advance();
        let first = self.name()?;
        let mut full = first.to_string();
        let mut last = first.clone();
        let mut target = self.var(first);
        let mut method = false;
        let mut path = false;
        loop {
            if self.accept(".") {
                let n = self.name()?;
                full.push('.');
                full.push_str(&n);
                target = Expr::Index(Box::new(target), Box::new(Expr::Str(Rc::from(n.as_bytes()))));
                last = n;
                path = true;
            } else if self.accept(":") {
                let n = self.name()?;
                full.push(':');
                full.push_str(&n);
                target = Expr::Index(Box::new(target), Box::new(Expr::Str(Rc::from(n.as_bytes()))));
                last = n;
                method = true;
                path = true;
                break;
            } else {
                break;
            }
        }
        if !path {
            self.written(&target);
        }
        let f = self.body(format!("function '{full}'"), Some(last), method, line)?;
        Ok(StmtKind::Function(target, f))
    }
    fn local_stat(&mut self) -> PResult<StmtKind> {
        let mut names = vec![self.name()?];
        self.annotation()?;
        self.attrib()?;
        while self.accept(",") {
            names.push(self.name()?);
            self.annotation()?;
            self.attrib()?;
        }
        let exprs = if self.accept("=") { self.exprlist()? } else { Vec::new() };
        let vars = names.into_iter().map(|n| self.declare(n)).collect();
        Ok(StmtKind::Local(vars, exprs))
    }
    /// `<const>` is accepted (and not enforced); `<close>` is not supported.
    fn attrib(&mut self) -> PResult<()> {
        if self.accept("<") {
            let a = self.name()?;
            if &*a != "const" {
                return self.error(&format!("unknown attribute '{a}'"));
            }
            self.expect(">", "")?;
        }
        Ok(())
    }
    fn expr_stat(&mut self) -> PResult<StmtKind> {
        let e = self.suffixed()?;
        let compound = match self.peek() {
            Tok::Op("+=") => Some(BinOp::Add),
            Tok::Op("-=") => Some(BinOp::Sub),
            Tok::Op("*=") => Some(BinOp::Mul),
            Tok::Op("/=") => Some(BinOp::Div),
            Tok::Op("//=") => Some(BinOp::IDiv),
            Tok::Op("%=") => Some(BinOp::Mod),
            Tok::Op("^=") => Some(BinOp::Pow),
            Tok::Op("..=") => Some(BinOp::Concat),
            _ => None,
        };
        if let Some(op) = compound {
            self.advance();
            return self.compound(e, op);
        }
        if self.check("=") || self.check(",") {
            let mut targets = vec![e];
            while self.accept(",") {
                targets.push(self.suffixed()?);
            }
            self.expect("=", "")?;
            let values = self.exprlist()?;
            for t in &targets {
                if !matches!(t, Expr::Local(..) | Expr::Upval(..) | Expr::Global(_) | Expr::Index(..)) {
                    return Err(format!(
                        "{}:{}: syntax error (cannot assign to this)",
                        self.chunk,
                        self.line()
                    ));
                }
            }
            for t in &targets {
                self.written(t);
            }
            return Ok(StmtKind::Assign(targets, values));
        }
        if !matches!(e, Expr::Call(..) | Expr::Method(..)) {
            return self.error("syntax error (expected a call or an assignment)");
        }
        Ok(StmtKind::Call(e))
    }
    /// `target op= value`, as Luau runs it: the target's table and key are
    /// worked out once, then read, combined with the value and stored.
    fn compound(&mut self, target: Expr, op: BinOp) -> PResult<StmtKind> {
        let line = self.line();
        let value = self.expr()?;
        let assign = |target: Expr, read: Expr| {
            StmtKind::Assign(vec![target], vec![Expr::Bin(op, Box::new(read), Box::new(value))])
        };
        let plain = |e: &Expr| -> Option<Expr> {
            Some(match e {
                Expr::Local(v, n) => Expr::Local(*v, n.clone()),
                Expr::Upval(i, n) => Expr::Upval(*i, n.clone()),
                Expr::Global(n) => Expr::Global(n.clone()),
                Expr::Str(s) => Expr::Str(s.clone()),
                Expr::Num(n) => Expr::Num(*n),
                _ => return None,
            })
        };
        match target {
            Expr::Local(..) | Expr::Upval(..) | Expr::Global(_) => {
                self.written(&target);
                let read = plain(&target).expect("a name");
                Ok(assign(target, read))
            }
            Expr::Index(o, k) => {
                if let (Some(o2), Some(k2)) = (plain(&o), plain(&k)) {
                    let read = Expr::Index(Box::new(o2), Box::new(k2));
                    return Ok(assign(Expr::Index(o, k), read));
                }
                // `t[f()] += 1`: the table and the key in hidden locals.
                self.open_scope();
                let t = self.declare("(compound table)".into());
                let key = self.declare("(compound key)".into());
                self.close_scope();
                let at = |v: VarId, n: &str| Expr::Local(v, n.into());
                let place = || Expr::Index(Box::new(at(t, "(compound table)")), Box::new(at(key, "(compound key)")));
                Ok(StmtKind::Do(vec![
                    Stmt {
                        line,
                        kind: StmtKind::Local(vec![t, key], vec![*o, *k]),
                    },
                    Stmt {
                        line,
                        kind: assign(place(), place()),
                    },
                ]))
            }
            _ => Err(format!("{}:{line}: syntax error (cannot assign to this)", self.chunk)),
        }
    }
    fn body(&mut self, name: String, debug_name: Option<Rc<str>>, method: bool, line: u32) -> PResult<Box<FuncAst>> {
        self.fs.push(FuncState::new(false));
        self.open_scope();
        if method {
            self.declare("self".into());
        }
        if self.check("<") {
            self.generics()?;
        }
        self.expect("(", "")?;
        if !self.check(")") {
            loop {
                if self.accept("...") {
                    self.fs().vararg = true;
                    self.annotation()?;
                    break;
                }
                let n = self.name()?;
                self.annotation()?;
                self.declare(n);
                if !self.accept(",") {
                    break;
                }
            }
        }
        self.expect(")", "")?;
        self.annotation()?;
        let nparams = self.fs().actives.len();
        let body = self.statements()?;
        self.expect_match("end", "function", line)?;
        self.close_scope();
        let fs = self.fs.pop().expect("the function being parsed");
        let name = if name.is_empty() {
            format!("function <{}:{line}>", self.chunk)
        } else {
            name
        };
        Ok(Box::new(FuncAst {
            name: name.into(),
            debug_name,
            line,
            nparams,
            vararg: fs.vararg,
            vars: fs.vars,
            upvals: fs.upvals,
            body,
        }))
    }
    fn exprlist(&mut self) -> PResult<Vec<Expr>> {
        let mut v = vec![self.expr()?];
        while self.accept(",") {
            v.push(self.expr()?);
        }
        Ok(v)
    }
    fn expr(&mut self) -> PResult<Expr> {
        self.subexpr(0)
    }
    fn subexpr(&mut self, limit: u8) -> PResult<Expr> {
        self.enter()?;
        let mut e = match self.peek() {
            Tok::Op("not") => {
                self.advance();
                Expr::Un(UnOp::Not, Box::new(self.subexpr(12)?))
            }
            Tok::Op("-") => {
                self.advance();
                match self.subexpr(12)? {
                    Expr::Num(n) => Expr::Num(-n),
                    inner => Expr::Un(UnOp::Neg, Box::new(inner)),
                }
            }
            Tok::Op("#") => {
                self.advance();
                Expr::Un(UnOp::Len, Box::new(self.subexpr(12)?))
            }
            Tok::Op("~") => {
                self.advance();
                Expr::Un(UnOp::BNot, Box::new(self.subexpr(12)?))
            }
            _ => self.simple()?,
        };
        // Luau's type assertion, `x :: T`.
        while self.accept("::") {
            self.ty()?;
        }
        loop {
            let (op, left, right) = match self.peek() {
                Tok::Op("or") => (None, 1, 1),
                Tok::Op("and") => (None, 2, 2),
                Tok::Op("<") => (Some(BinOp::Lt), 3, 3),
                Tok::Op(">") => (Some(BinOp::Gt), 3, 3),
                Tok::Op("<=") => (Some(BinOp::Le), 3, 3),
                Tok::Op(">=") => (Some(BinOp::Ge), 3, 3),
                Tok::Op("~=") => (Some(BinOp::Ne), 3, 3),
                Tok::Op("==") => (Some(BinOp::Eq), 3, 3),
                Tok::Op("..") => (Some(BinOp::Concat), 9, 8),
                Tok::Op("+") => (Some(BinOp::Add), 10, 10),
                Tok::Op("-") => (Some(BinOp::Sub), 10, 10),
                Tok::Op("*") => (Some(BinOp::Mul), 11, 11),
                Tok::Op("/") => (Some(BinOp::Div), 11, 11),
                Tok::Op("//") => (Some(BinOp::IDiv), 11, 11),
                Tok::Op("%") => (Some(BinOp::Mod), 11, 11),
                Tok::Op("^") => (Some(BinOp::Pow), 14, 13),
                // Lua 5.3: | then ~ then & then the shifts, between comparison and `..`.
                Tok::Op("|") => (Some(BinOp::BOr), 4, 4),
                Tok::Op("~") => (Some(BinOp::BXor), 5, 5),
                Tok::Op("&") => (Some(BinOp::BAnd), 6, 6),
                Tok::Op("<<") => (Some(BinOp::Shl), 7, 7),
                Tok::Op(">>") => (Some(BinOp::Shr), 7, 7),
                _ => break,
            };
            if left <= limit {
                break;
            }
            let word = self.advance();
            let rhs = self.subexpr(right)?;
            e = match (op, word) {
                (Some(op), _) => Expr::Bin(op, Box::new(e), Box::new(rhs)),
                (None, Tok::Op("and")) => Expr::And(Box::new(e), Box::new(rhs)),
                (None, _) => Expr::Or(Box::new(e), Box::new(rhs)),
            };
        }
        self.depth -= 1;
        Ok(e)
    }
    fn simple(&mut self) -> PResult<Expr> {
        let line = self.line();
        Ok(match self.peek().clone() {
            Tok::Num(n) => {
                self.advance();
                Expr::Num(n)
            }
            Tok::Str(s) => {
                self.advance();
                Expr::Str(s)
            }
            Tok::Op("nil") => {
                self.advance();
                Expr::Nil
            }
            Tok::Op("true") => {
                self.advance();
                Expr::True
            }
            Tok::Op("false") => {
                self.advance();
                Expr::False
            }
            Tok::Op("...") => {
                if !self.fs().vararg {
                    return self.error("cannot use '...' outside a vararg function");
                }
                self.advance();
                Expr::Vararg
            }
            Tok::Op("{") => self.table()?,
            Tok::Op("function") => {
                self.advance();
                Expr::Function(self.body(String::new(), None, false, line)?)
            }
            Tok::Op("if") => {
                self.advance();
                self.if_else()?
            }
            Tok::InterpBegin(first) => {
                self.advance();
                let mut args = Vec::new();
                if !first.is_empty() {
                    args.push(Expr::Str(first));
                }
                loop {
                    args.push(Expr::Paren(Box::new(self.expr()?)));
                    match self.peek().clone() {
                        Tok::InterpMid(s) | Tok::InterpEnd(s) => {
                            let end = matches!(self.peek(), Tok::InterpEnd(_));
                            self.advance();
                            if !s.is_empty() {
                                args.push(Expr::Str(s));
                            }
                            if end {
                                break;
                            }
                        }
                        _ => return self.error("'}' expected in the interpolated string"),
                    }
                }
                Expr::Call(Box::new(Expr::Builtin("interpolate")), args)
            }
            _ => self.suffixed()?,
        })
    }
    /// An if-expression after its `if`: `c then a [elseif ...] else b`.
    fn if_else(&mut self) -> PResult<Expr> {
        self.enter()?;
        let cond = self.expr()?;
        self.expect("then", "")?;
        let yes = self.expr()?;
        let no = if self.accept("elseif") {
            self.if_else()?
        } else {
            self.expect("else", " (an if-expression needs an else)")?;
            self.expr()?
        };
        self.depth -= 1;
        Ok(Expr::IfElse(Box::new(cond), Box::new(yes), Box::new(no)))
    }
    fn primary(&mut self) -> PResult<Expr> {
        match self.peek().clone() {
            Tok::Name(n) => {
                self.advance();
                Ok(self.var(n))
            }
            Tok::Op("(") => {
                let line = self.line();
                self.advance();
                let e = self.expr()?;
                self.expect_match(")", "(", line)?;
                Ok(Expr::Paren(Box::new(e)))
            }
            _ => self.error("unexpected symbol"),
        }
    }
    fn suffixed(&mut self) -> PResult<Expr> {
        let mut e = self.primary()?;
        loop {
            match self.peek().clone() {
                Tok::Op(".") => {
                    self.advance();
                    let n = self.name()?;
                    e = Expr::Index(Box::new(e), Box::new(Expr::Str(Rc::from(n.as_bytes()))));
                }
                Tok::Op("[") => {
                    self.advance();
                    let k = self.expr()?;
                    self.expect("]", "")?;
                    e = Expr::Index(Box::new(e), Box::new(k));
                }
                Tok::Op(":") => {
                    self.advance();
                    let n = self.name()?;
                    let args = self.args()?;
                    e = Expr::Method(Box::new(e), Rc::from(n.as_bytes()), args);
                }
                Tok::Op("(" | "{") | Tok::Str(_) => {
                    let args = self.args()?;
                    e = Expr::Call(Box::new(e), args);
                }
                _ => return Ok(e),
            }
        }
    }
    fn args(&mut self) -> PResult<Vec<Expr>> {
        match self.peek().clone() {
            Tok::Str(s) => {
                self.advance();
                Ok(vec![Expr::Str(s)])
            }
            Tok::Op("{") => Ok(vec![self.table()?]),
            Tok::Op("(") => {
                let line = self.line();
                self.advance();
                if self.accept(")") {
                    return Ok(Vec::new());
                }
                let v = self.exprlist()?;
                self.expect_match(")", "(", line)?;
                Ok(v)
            }
            _ => self.error("function arguments expected"),
        }
    }
    fn table(&mut self) -> PResult<Expr> {
        let line = self.line();
        self.expect("{", "")?;
        let mut items = Vec::new();
        while !self.check("}") {
            if self.check("[") {
                self.advance();
                let k = self.expr()?;
                self.expect("]", "")?;
                self.expect("=", "")?;
                items.push(Item {
                    key: Some(k),
                    value: self.expr()?,
                    record: false,
                });
            } else if matches!(self.peek(), Tok::Name(_)) && matches!(self.peek2(), Tok::Op("=")) {
                let n = self.name()?;
                self.advance();
                items.push(Item {
                    key: Some(Expr::Str(Rc::from(n.as_bytes()))),
                    value: self.expr()?,
                    record: true,
                });
            } else {
                items.push(Item {
                    key: None,
                    value: self.expr()?,
                    record: false,
                });
            }
            if !self.accept(",") && !self.accept(";") {
                break;
            }
        }
        self.expect_match("}", "{", line)?;
        Ok(Expr::Table(items))
    }
}

// ----------------------------------------------------------------- bytecode

/// As many results or arguments as there are, up to the top of the stack.
const MULTI: u16 = u16::MAX;
/// SetList's count for "up to the top".
const MULTI8: u8 = u8::MAX;
/// NewCell's register for a cell that starts as nil.
const NO_REG: u16 = u16::MAX;
/// Positional items a table constructor stores at once.
const LIST_BATCH: usize = 50;

/// An instruction. Registers count from the frame's base; `K` is the
/// function's constants, `U` its closure's upvalues and `C` the frame's
/// cells (its locals that closures captured).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    /// A statement starts on this line: one step of the budget.
    Step(u32),
    /// A loop goes round: one step.
    Tick,
    /// Steps for a table constructor's items.
    Charge(u32),
    Move(u16, u16),
    LoadK(u16, u32),
    /// A whole number small enough to need no constant.
    LoadN(u16, i16),
    LoadNil(u16, u16),
    LoadBool(u16, bool),
    GetUpval(u16, u16),
    SetUpval(u16, u16),
    GetCell(u16, u16),
    SetCell(u16, u16),
    /// C[c] = a new cell holding R[a] (nil for NO_REG).
    NewCell(u16, u16),
    GetGlobal(u16, u32),
    SetGlobal(u16, u32),
    /// R[a] = a global, or a field path from one (`string.format`).
    GetImport(u16, u16),
    GetTable(u16, u16, u16),
    /// R[a] = R[b][K[c]], a string key.
    GetField(u16, u16, u16),
    SetTable(u16, u16, u16),
    /// R[a][K[b]] = R[c].
    SetField(u16, u16, u16),
    /// R[a] = {} with room for b items and c keys.
    NewTable(u16, u16, u16),
    /// R[a][start + i] = R[a + 1 + i] for the next n registers: `start << 8 | n`.
    SetList(u16, u32),
    /// R[a + 1] = R[b]; R[a] = R[b][K[c]], a method to call.
    Method(u16, u16, u16),
    /// Before a method call: K[k] is its name, for `__namecall`.
    Namecall(u16, u16),
    /// Calls R[a] with b arguments; c results in R[a..].
    Call(u16, u16, u16),
    Return(u16, u16),
    Vararg(u16, u16),
    Closure(u16, u16),
    Add(u16, u16, u16),
    Sub(u16, u16, u16),
    Mul(u16, u16, u16),
    Div(u16, u16, u16),
    Mod(u16, u16, u16),
    AddK(u16, u16, u16),
    SubK(u16, u16, u16),
    MulK(u16, u16, u16),
    DivK(u16, u16, u16),
    ModK(u16, u16, u16),
    /// The rarer operators: `//`, `^` and the bitwise ones.
    Arith(BinOp, u16, u16, u16),
    ArithK(BinOp, u16, u16, u16),
    Concat(u16, u16, u16),
    Eq(u16, u16, u16),
    Ne(u16, u16, u16),
    Lt(u16, u16, u16),
    Le(u16, u16, u16),
    EqK(u16, u16, u16),
    NeK(u16, u16, u16),
    Unm(u16, u16),
    Not(u16, u16),
    Len(u16, u16),
    BNot(u16, u16),
    Jmp(i32),
    JmpIf(u16, i32),
    JmpIfNot(u16, i32),
    /// A numeric for's bound, made a number (0 initial, 1 limit, 2 step).
    ForCheck(u16, u8),
    ForPrep(u16, i32),
    ForLoop(u16, i32),
    TForPrep(u16),
    TForCall(u16, u16),
    TForLoop(u16, i32),
}

const _: () = assert!(std::mem::size_of::<Op>() == 8);

/// A constant that is not a value of its own, as Luau's compiler makes
/// them: a global import, a table constructor's keys, a closure it can
/// share. They keep the constants numbered as Luau numbers them.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum Special {
    /// The constants of the names, `string` then `format`.
    Import(Box<[u32]>),
    /// The constants of the keys.
    Table(Box<[u32]>),
    /// The child function.
    Closure(u16),
}

/// A local of a compiled function: its register, its cell if a closure
/// captured it, and the instructions it is in scope for.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct LocalVar {
    pub name: Rc<str>,
    pub reg: u16,
    pub cell: Option<u16>,
    pub start: u32,
    pub end: u32,
}

/// A compiled function.
#[allow(dead_code)]
#[derive(Clone)]
pub struct Proto {
    /// `function 'name'`, `function <main.lua:12>` or `main chunk`.
    pub name: Rc<str>,
    /// The name it was declared with, as Luau's `debug.info(f, "n")` gives it.
    pub debug_name: Option<Rc<str>>,
    pub chunk: Rc<str>,
    /// Where it was defined (0: a main chunk).
    pub line: u32,
    pub nparams: usize,
    pub vararg: bool,
    pub maxstack: usize,
    pub ncells: usize,
    pub code: Vec<Op>,
    /// The line of each instruction.
    pub lines: Vec<u32>,
    /// Numbers and strings in the order the compiler met them; specials are nil.
    pub constants: Vec<Value>,
    pub specials: Vec<(u32, Special)>,
    /// The functions defined in it, in order.
    pub protos: Vec<Rc<Proto>>,
    /// `index` is a cell of the enclosing frame (`from_local`) or one of the
    /// enclosing closure's upvalues.
    pub upvals: Vec<UpDesc>,
    pub locals: Vec<LocalVar>,
    /// GetImport's paths: constants of the names.
    imports: Vec<Box<[u32]>>,
    /// The constants as table keys, for string fields.
    keys: Vec<Option<Key>>,
    /// Names of the operands of instructions that can fail, for messages.
    descs: Vec<(u32, [Desc; 2])>,
}

#[allow(dead_code)]
impl Proto {
    /// A copy with constant `i` (from 0) set to `v`: a number, a string,
    /// a boolean or nil (`debug.setconstant`); None for a slot that holds
    /// no plain value.
    pub fn with_constant(&self, i: usize, v: Value) -> Option<Proto> {
        if i >= self.constants.len() || self.specials.iter().any(|s| s.0 as usize == i) {
            return None;
        }
        let mut p = self.clone();
        p.keys[i] = match &v {
            Value::Str(_) => key_of(&v),
            _ => None,
        };
        p.constants[i] = v;
        Some(p)
    }
    /// The constants as Luau numbers them: None for an import, a table
    /// template or a shared closure.
    pub fn constant_slots(&self) -> Vec<Option<Value>> {
        let mut out: Vec<Option<Value>> = self.constants.iter().cloned().map(Some).collect();
        for (i, _) in &self.specials {
            out[*i as usize] = None;
        }
        out
    }
    fn desc(&self, pc: usize, i: usize) -> String {
        match self.descs.binary_search_by_key(&(pc as u32), |d| d.0) {
            Ok(at) => self.descs[at].1[i].text(),
            Err(_) => String::new(),
        }
    }
    /// The local in register `reg` at instruction `pc`.
    fn local_at(&self, reg: usize, pc: usize) -> Option<&LocalVar> {
        self.locals
            .iter()
            .rev()
            .find(|l| l.reg as usize == reg && (l.start as usize) <= pc && pc < l.end as usize)
    }
}

// ----------------------------------------------------------------- compiler

/// Where a local lives once compiled.
#[derive(Clone)]
enum Loc {
    /// Not declared yet.
    None,
    Reg(u16),
    /// A closure captured it: a cell.
    Cell(u16),
    /// Never assigned after a constant initial value: folded into its uses.
    Const(Value),
}

/// An upvalue as the parser numbered it, compiled.
#[derive(Clone)]
enum UpLoc {
    Unresolved,
    Index(u16),
    Const(Value),
}

#[derive(PartialEq, Eq, Hash)]
enum KKey {
    Num(u64),
    Str(Str),
    Import(Vec<u32>),
    Table(Vec<u32>),
    Closure(usize),
    Builtin(&'static str),
}

/// A place a multiple assignment stores to.
enum Place {
    None,
    Reg(u16),
    Cell(u16),
    Up(u16),
    Global(Str),
    Field(u16, Str, Desc),
    Table(u16, u16, Desc),
}

/// A function being compiled.
struct Gen<'a> {
    f: &'a FuncAst,
    code: Vec<Op>,
    lines: Vec<u32>,
    consts: Vec<Value>,
    keys: Vec<Option<Key>>,
    kmap: HashMap<KKey, u32>,
    specials: Vec<(u32, Special)>,
    imports: Vec<Box<[u32]>>,
    protos: Vec<Rc<Proto>>,
    descs: Vec<(u32, [Desc; 2])>,
    locals: Vec<LocalVar>,
    /// Indices in `locals` of those in scope.
    scope: Vec<usize>,
    locs: Vec<Loc>,
    /// The loop depth each local was declared at.
    var_loops: Vec<u32>,
    ups: Vec<UpDesc>,
    /// Whether each upvalue lets a closure be shared (Luau's DUPCLOSURE).
    ups_share: Vec<bool>,
    up_locs: Vec<UpLoc>,
    ncells: usize,
    nactive: usize,
    free: usize,
    maxstack: usize,
    line: u32,
    loops: u32,
    breaks: Vec<Vec<usize>>,
    /// The `continue`s of each loop being compiled, to patch.
    continues: Vec<Vec<usize>>,
}

impl<'a> Gen<'a> {
    fn new(f: &'a FuncAst) -> Gen<'a> {
        Gen {
            f,
            code: Vec::new(),
            lines: Vec::new(),
            consts: Vec::new(),
            keys: Vec::new(),
            kmap: HashMap::new(),
            specials: Vec::new(),
            imports: Vec::new(),
            protos: Vec::new(),
            descs: Vec::new(),
            locals: Vec::new(),
            scope: Vec::new(),
            locs: vec![Loc::None; f.vars.len()],
            var_loops: vec![0; f.vars.len()],
            ups: Vec::new(),
            ups_share: Vec::new(),
            up_locs: vec![UpLoc::Unresolved; f.upvals.len()],
            ncells: 0,
            nactive: 0,
            free: 0,
            maxstack: 0,
            line: f.line,
            loops: 0,
            breaks: Vec::new(),
            continues: Vec::new(),
        }
    }
    fn finish(mut self, chunk: &Rc<str>) -> Proto {
        self.descs.sort_by_key(|d| d.0);
        Proto {
            name: self.f.name.clone(),
            debug_name: self.f.debug_name.clone(),
            chunk: chunk.clone(),
            line: self.f.line,
            nparams: self.f.nparams,
            vararg: self.f.vararg,
            maxstack: self.maxstack,
            ncells: self.ncells,
            code: self.code,
            lines: self.lines,
            constants: self.consts,
            specials: self.specials,
            protos: self.protos,
            upvals: self.ups,
            locals: self.locals,
            imports: self.imports,
            keys: self.keys,
            descs: self.descs,
        }
    }
}

/// The compiler: the function being compiled and those it is inside.
struct Compiler<'a> {
    chunk: Rc<str>,
    /// Globals the chunk assigns to.
    written: &'a HashSet<Str>,
    fenv: bool,
    g: Gen<'a>,
    outer: Vec<Gen<'a>>,
    error: Option<String>,
}

fn compile(ast: &FuncAst, chunk: &Rc<str>, written: &HashSet<Str>, fenv: bool) -> Result<Proto, String> {
    let mut c = Compiler {
        chunk: chunk.clone(),
        written,
        fenv,
        g: Gen::new(ast),
        outer: Vec::new(),
        error: None,
    };
    c.body();
    match c.error {
        Some(e) => Err(e),
        None => Ok(c.g.finish(chunk)),
    }
}

impl<'a> Compiler<'a> {
    fn fail(&mut self, msg: &str) {
        if self.error.is_none() {
            self.error = Some(format!("{}:{}: {msg}", self.chunk, self.g.line));
        }
    }
    fn r16(&mut self, r: usize) -> u16 {
        if r >= u16::MAX as usize - 1 {
            self.fail("function or expression needs too many registers");
            return 0;
        }
        r as u16
    }
    fn emit(&mut self, op: Op) -> usize {
        self.g.code.push(op);
        self.g.lines.push(self.g.line);
        self.g.code.len() - 1
    }
    fn pc(&self) -> usize {
        self.g.code.len()
    }
    fn alloc(&mut self, n: usize) -> usize {
        let r = self.g.free;
        self.g.free += n;
        self.g.maxstack = self.g.maxstack.max(self.g.free);
        if self.g.free >= u16::MAX as usize - 1 {
            self.fail("function or expression needs too many registers");
        }
        r
    }
    /// Points the jump at `at` to `target`.
    fn patch(&mut self, at: usize, target: usize) {
        let off = target as i64 - at as i64 - 1;
        let Ok(off) = i32::try_from(off) else {
            self.fail("control structure too long");
            return;
        };
        self.g.code[at] = match self.g.code[at] {
            Op::Jmp(_) => Op::Jmp(off),
            Op::JmpIf(a, _) => Op::JmpIf(a, off),
            Op::JmpIfNot(a, _) => Op::JmpIfNot(a, off),
            Op::ForPrep(a, _) => Op::ForPrep(a, off),
            Op::ForLoop(a, _) => Op::ForLoop(a, off),
            Op::TForLoop(a, _) => Op::TForLoop(a, off),
            other => other,
        };
    }
    fn patch_here(&mut self, jumps: &[usize]) {
        let here = self.pc();
        for &j in jumps {
            self.patch(j, here);
        }
    }
    fn note(&mut self, pc: usize, a: Desc, b: Desc) {
        if !a.is_none() || !b.is_none() {
            self.g.descs.push((pc as u32, [a, b]));
        }
    }

    // ------------------------------------------------------------- constants

    fn konst(&mut self, key: KKey, v: Value, special: Option<Special>) -> u32 {
        if let Some(&i) = self.g.kmap.get(&key) {
            return i;
        }
        let i = self.g.consts.len() as u32;
        self.g.keys.push(match &v {
            Value::Str(_) => key_of(&v),
            _ => None,
        });
        self.g.consts.push(v);
        self.g.kmap.insert(key, i);
        if let Some(s) = special {
            self.g.specials.push((i, s));
        }
        i
    }
    fn k_str(&mut self, s: &Str) -> u32 {
        self.konst(KKey::Str(s.clone()), Value::Str(s.clone()), None)
    }
    fn k_num(&mut self, n: f64) -> u32 {
        self.konst(KKey::Num(n.to_bits()), Value::Num(n), None)
    }
    /// A constant for an operand that has 16 bits.
    fn k16(&mut self, k: u32) -> u16 {
        match u16::try_from(k) {
            Ok(k) if k != u16::MAX => k,
            _ => {
                self.fail("function has too many constants");
                0
            }
        }
    }
    fn load_const(&mut self, v: &Value, dest: usize) {
        let d = self.r16(dest);
        match v {
            Value::Bool(b) => {
                self.emit(Op::LoadBool(d, *b));
            }
            Value::Num(n)
                if n.fract() == 0.0 && (-32768.0..=32767.0).contains(n) && !(*n == 0.0 && n.is_sign_negative()) =>
            {
                self.emit(Op::LoadN(d, *n as i16));
            }
            Value::Num(n) => {
                let k = self.k_num(*n);
                self.emit(Op::LoadK(d, k));
            }
            Value::Str(s) => {
                let k = self.k_str(s);
                self.emit(Op::LoadK(d, k));
            }
            _ => {
                self.emit(Op::LoadNil(d, 1));
            }
        }
    }

    // ------------------------------------------------------------- names

    fn level(&mut self, level: usize) -> &mut Gen<'a> {
        if level == self.outer.len() {
            &mut self.g
        } else {
            &mut self.outer[level]
        }
    }
    fn level_ref(&self, level: usize) -> &Gen<'a> {
        if level == self.outer.len() {
            &self.g
        } else {
            &self.outer[level]
        }
    }
    /// The parser's upvalue `i` of the function being compiled.
    fn up(&mut self, i: usize) -> UpLoc {
        let level = self.outer.len();
        self.resolve_up(level, i)
    }
    fn resolve_up(&mut self, level: usize, i: usize) -> UpLoc {
        match &self.level_ref(level).up_locs[i] {
            UpLoc::Unresolved => {}
            known => return known.clone(),
        }
        if level == 0 {
            return UpLoc::Const(Value::Nil);
        }
        let (name, from_local, index) = {
            let d = &self.level_ref(level).f.upvals[i];
            (d.name.clone(), d.from_local, d.index)
        };
        let parent = level - 1;
        let (src, share) = if from_local {
            let p = self.level_ref(parent);
            let share = parent == 0 && !p.f.vars[index].written && p.var_loops[index] == 0;
            match &p.locs[index] {
                Loc::Const(v) => {
                    let v = v.clone();
                    self.level(level).up_locs[i] = UpLoc::Const(v.clone());
                    return UpLoc::Const(v);
                }
                Loc::Cell(c) => (*c, share),
                // A captured local always has a cell.
                Loc::Reg(_) | Loc::None => (0, false),
            }
        } else {
            match self.resolve_up(parent, index) {
                UpLoc::Index(k) => (k, self.level_ref(parent).ups_share[k as usize]),
                other => {
                    self.level(level).up_locs[i] = other.clone();
                    return other;
                }
            }
        };
        let g = self.level(level);
        let k = g.ups.len() as u16;
        g.ups.push(UpDesc {
            name,
            from_local,
            index: src as usize,
        });
        g.ups_share.push(share);
        g.up_locs[i] = UpLoc::Index(k);
        UpLoc::Index(k)
    }
    /// What the parser's upvalue `i` at `level` holds, if a constant (without
    /// making it an upvalue).
    fn peek_up(&self, level: usize, i: usize) -> Option<Value> {
        let g = self.level_ref(level);
        match &g.up_locs[i] {
            UpLoc::Const(v) => return Some(v.clone()),
            UpLoc::Index(_) => return None,
            UpLoc::Unresolved => {}
        }
        if level == 0 {
            return None;
        }
        let d = &g.f.upvals[i];
        if d.from_local {
            match &self.level_ref(level - 1).locs[d.index] {
                Loc::Const(v) => Some(v.clone()),
                _ => None,
            }
        } else {
            self.peek_up(level - 1, d.index)
        }
    }
    fn can_import(&self, root: &Str) -> bool {
        !self.fenv && !self.written.contains(root)
    }
    /// A global path of up to three names (`a`, `a.b`, `a.b.c`) in R[dest].
    fn import(&mut self, names: &[&Str], dest: usize) {
        let ids: Vec<u32> = names.iter().map(|n| self.k_str(n)).collect();
        let before = self.g.consts.len();
        let slot = self.konst(
            KKey::Import(ids.clone()),
            Value::Nil,
            Some(Special::Import(ids.clone().into())),
        );
        let at = if self.g.consts.len() > before {
            self.g.imports.push(ids.into());
            self.g.imports.len() - 1
        } else {
            let found = self.g.specials.iter().find(|s| s.0 == slot).and_then(|s| match &s.1 {
                Special::Import(want) => self.g.imports.iter().position(|i| i == want),
                _ => None,
            });
            found.unwrap_or(0)
        };
        let d = self.r16(dest);
        let at = self.k16(at as u32);
        self.emit(Op::GetImport(d, at));
    }

    // ------------------------------------------------------------- folding

    /// `e`'s value if the compiler knows it: literals, constant locals, and
    /// arithmetic, comparisons and logic on those (Luau folds the same).
    fn fold(&self, e: &Expr) -> Option<Value> {
        Some(match e {
            Expr::Nil => Value::Nil,
            Expr::True => Value::Bool(true),
            Expr::False => Value::Bool(false),
            Expr::Num(n) => Value::Num(*n),
            Expr::Str(s) => Value::Str(s.clone()),
            Expr::Local(v, _) => match &self.g.locs[*v] {
                Loc::Const(c) => c.clone(),
                _ => return None,
            },
            Expr::Upval(i, _) => return self.peek_up(self.outer.len(), *i),
            Expr::Paren(inner) => self.fold(inner)?,
            Expr::Un(UnOp::Neg, a) => match self.fold(a)? {
                Value::Num(n) => Value::Num(-n),
                _ => return None,
            },
            Expr::Un(UnOp::Not, a) => Value::Bool(!self.fold(a)?.truthy()),
            Expr::Un(UnOp::Len, a) => match self.fold(a)? {
                Value::Str(s) => Value::Num(s.len() as f64),
                _ => return None,
            },
            Expr::Bin(op, a, b) => {
                let x = self.fold(a)?;
                let y = self.fold(b)?;
                match (op, &x, &y) {
                    (BinOp::Eq, ..) => Value::Bool(x.raw_eq(&y)),
                    (BinOp::Ne, ..) => Value::Bool(!x.raw_eq(&y)),
                    (BinOp::Lt, Value::Num(p), Value::Num(q)) => Value::Bool(p < q),
                    (BinOp::Le, Value::Num(p), Value::Num(q)) => Value::Bool(p <= q),
                    (BinOp::Gt, Value::Num(p), Value::Num(q)) => Value::Bool(p > q),
                    (BinOp::Ge, Value::Num(p), Value::Num(q)) => Value::Bool(p >= q),
                    (
                        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::IDiv | BinOp::Mod | BinOp::Pow,
                        Value::Num(p),
                        Value::Num(q),
                    ) => Value::Num(arith(*op, *p, *q)),
                    _ => return None,
                }
            }
            Expr::And(a, b) => {
                let x = self.fold(a)?;
                if x.truthy() { self.fold(b)? } else { x }
            }
            Expr::Or(a, b) => {
                let x = self.fold(a)?;
                if x.truthy() { x } else { self.fold(b)? }
            }
            Expr::IfElse(c, a, b) => {
                if self.fold(c)?.truthy() {
                    self.fold(a)?
                } else {
                    self.fold(b)?
                }
            }
            _ => return None,
        })
    }

    // ------------------------------------------------------------- expressions

    /// `e` in a register: a local's own, or a new temporary.
    fn exp_any(&mut self, e: &'a Expr) -> usize {
        if let Expr::Local(v, _) = e
            && let Loc::Reg(r) = self.g.locs[*v]
        {
            return r as usize;
        }
        let r = self.alloc(1);
        self.exp_to(e, r);
        r
    }
    /// `e` through a new temporary into `dest`.
    fn exp_via_temp(&mut self, e: &'a Expr, dest: usize) {
        let save = self.g.free;
        let t = self.alloc(1);
        self.exp_to(e, t);
        let (d, t) = (self.r16(dest), self.r16(t));
        self.emit(Op::Move(d, t));
        self.g.free = save;
    }
    /// `e`'s value into register `dest`.
    fn exp_to(&mut self, e: &'a Expr, dest: usize) {
        if let Some(v) = self.fold(e) {
            self.load_const(&v, dest);
            return;
        }
        let d = self.r16(dest);
        // Is `dest` the newest temporary, with nothing above it?
        let top = dest >= self.g.nactive && dest + 1 == self.g.free;
        match e {
            Expr::Vararg => {
                self.emit(Op::Vararg(d, 1));
            }
            Expr::Function(f) => self.closure(f, dest),
            Expr::Local(v, _) => match self.g.locs[*v].clone() {
                Loc::Reg(r) => {
                    if r != d {
                        self.emit(Op::Move(d, r));
                    }
                }
                Loc::Cell(c) => {
                    self.emit(Op::GetCell(d, c));
                }
                Loc::Const(c) => self.load_const(&c, dest),
                Loc::None => {
                    self.emit(Op::LoadNil(d, 1));
                }
            },
            Expr::Upval(i, _) => match self.up(*i) {
                UpLoc::Index(u) => {
                    self.emit(Op::GetUpval(d, u));
                }
                UpLoc::Const(c) => self.load_const(&c, dest),
                UpLoc::Unresolved => {
                    self.emit(Op::LoadNil(d, 1));
                }
            },
            Expr::Global(n) => {
                if self.can_import(n) {
                    self.import(&[n], dest);
                } else {
                    let k = self.k_str(n);
                    self.emit(Op::GetGlobal(d, k));
                }
            }
            Expr::Index(o, k) => self.index(e, o, k, dest),
            Expr::Call(..) | Expr::Method(..) => {
                if top {
                    self.g.free = dest;
                    self.call(e, 1);
                } else {
                    let save = self.g.free;
                    let b = self.call(e, 1);
                    let b = self.r16(b);
                    self.emit(Op::Move(d, b));
                    self.g.free = save;
                }
            }
            Expr::And(a, b) | Expr::Or(a, b) => {
                if dest < self.g.nactive {
                    return self.exp_via_temp(e, dest);
                }
                let and = matches!(e, Expr::And(..));
                if let Some(x) = self.fold(a) {
                    // The left side decides alone whether the right one runs.
                    if x.truthy() == and {
                        self.exp_to(b, dest);
                    } else {
                        self.load_const(&x, dest);
                    }
                    return;
                }
                self.exp_to(a, dest);
                let j = if and {
                    self.emit(Op::JmpIfNot(d, 0))
                } else {
                    self.emit(Op::JmpIf(d, 0))
                };
                self.exp_to(b, dest);
                self.patch_here(&[j]);
            }
            Expr::Bin(op, a, b) => self.binary(*op, a, b, dest),
            Expr::Un(op, a) => {
                let save = self.g.free;
                let r = self.exp_any(a);
                let r = self.r16(r);
                let pc = self.emit(match op {
                    UnOp::Neg => Op::Unm(d, r),
                    UnOp::Not => Op::Not(d, r),
                    UnOp::Len => Op::Len(d, r),
                    UnOp::BNot => Op::BNot(d, r),
                });
                self.note(pc, a.desc(), Desc::None);
                self.g.free = save;
            }
            Expr::Table(items) => {
                if top {
                    self.table(items, dest);
                } else {
                    self.exp_via_temp(e, dest);
                }
            }
            Expr::Paren(inner) => self.exp_to(inner, dest),
            Expr::IfElse(c, a, b) => {
                if dest < self.g.nactive {
                    return self.exp_via_temp(e, dest);
                }
                if let Some(x) = self.fold(c) {
                    return self.exp_to(if x.truthy() { a } else { b }, dest);
                }
                let skip = self.jump_if_false(c);
                self.exp_to(a, dest);
                let end = self.emit(Op::Jmp(0));
                self.patch_here(&skip);
                self.exp_to(b, dest);
                self.patch_here(&[end]);
            }
            Expr::Builtin(name) => {
                let k = self.konst(KKey::Builtin(name), builtin_value(name), None);
                self.emit(Op::LoadK(d, k));
            }
            Expr::Nil | Expr::True | Expr::False | Expr::Num(_) | Expr::Str(_) => {
                self.emit(Op::LoadNil(d, 1));
            }
        }
    }
    /// A global path (`a.b.c`, `a` a global): its names.
    fn path(e: &'a Expr, out: &mut Vec<&'a Str>) -> bool {
        match e {
            Expr::Global(n) => {
                out.push(n);
                true
            }
            Expr::Index(o, k) => match &**k {
                Expr::Str(s) if Self::path(o, out) => {
                    out.push(s);
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }
    fn index(&mut self, e: &'a Expr, o: &'a Expr, k: &'a Expr, dest: usize) {
        let mut names = Vec::new();
        if Self::path(e, &mut names) && names.len() <= 3 && self.can_import(names[0]) {
            return self.import(&names, dest);
        }
        let save = self.g.free;
        let ro = self.exp_any(o);
        let (d, ro) = (self.r16(dest), self.r16(ro));
        let pc = match self.fold(k) {
            Some(Value::Str(s)) => {
                let kk = self.k_str(&s);
                let kk = self.k16(kk);
                self.emit(Op::GetField(d, ro, kk))
            }
            _ => {
                let rk = self.exp_any(k);
                let rk = self.r16(rk);
                self.emit(Op::GetTable(d, ro, rk))
            }
        };
        self.note(pc, o.desc(), Desc::None);
        self.g.free = save;
    }
    fn binary(&mut self, op: BinOp, a: &'a Expr, b: &'a Expr, dest: usize) {
        let save = self.g.free;
        let d = self.r16(dest);
        let rb = self.exp_any(a);
        let rb = self.r16(rb);
        let pc = match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod | BinOp::IDiv | BinOp::Pow => {
                if let Some(Value::Num(n)) = self.fold(b) {
                    let k = self.k_num(n);
                    let k = self.k16(k);
                    self.emit(match op {
                        BinOp::Add => Op::AddK(d, rb, k),
                        BinOp::Sub => Op::SubK(d, rb, k),
                        BinOp::Mul => Op::MulK(d, rb, k),
                        BinOp::Div => Op::DivK(d, rb, k),
                        BinOp::Mod => Op::ModK(d, rb, k),
                        _ => Op::ArithK(op, d, rb, k),
                    })
                } else {
                    let rc = self.exp_any(b);
                    let rc = self.r16(rc);
                    self.emit(match op {
                        BinOp::Add => Op::Add(d, rb, rc),
                        BinOp::Sub => Op::Sub(d, rb, rc),
                        BinOp::Mul => Op::Mul(d, rb, rc),
                        BinOp::Div => Op::Div(d, rb, rc),
                        BinOp::Mod => Op::Mod(d, rb, rc),
                        _ => Op::Arith(op, d, rb, rc),
                    })
                }
            }
            BinOp::Eq | BinOp::Ne => match self.fold(b) {
                Some(Value::Num(n)) => {
                    let k = self.k_num(n);
                    let k = self.k16(k);
                    self.emit(if op == BinOp::Eq {
                        Op::EqK(d, rb, k)
                    } else {
                        Op::NeK(d, rb, k)
                    })
                }
                Some(Value::Str(s)) => {
                    let k = self.k_str(&s);
                    let k = self.k16(k);
                    self.emit(if op == BinOp::Eq {
                        Op::EqK(d, rb, k)
                    } else {
                        Op::NeK(d, rb, k)
                    })
                }
                _ => {
                    let rc = self.exp_any(b);
                    let rc = self.r16(rc);
                    self.emit(if op == BinOp::Eq {
                        Op::Eq(d, rb, rc)
                    } else {
                        Op::Ne(d, rb, rc)
                    })
                }
            },
            _ => {
                let rc = self.exp_any(b);
                let rc = self.r16(rc);
                self.emit(match op {
                    BinOp::Concat => Op::Concat(d, rb, rc),
                    BinOp::Lt => Op::Lt(d, rb, rc),
                    BinOp::Le => Op::Le(d, rb, rc),
                    // `a > b` is `b < a`, as Lua does it.
                    BinOp::Gt => Op::Lt(d, rc, rb),
                    BinOp::Ge => Op::Le(d, rc, rb),
                    _ => Op::Arith(op, d, rb, rc),
                })
            }
        };
        self.note(pc, a.desc(), b.desc());
        self.g.free = save;
    }
    fn table(&mut self, items: &'a [Item], dest: usize) {
        let d = self.r16(dest);
        let narr = items.iter().filter(|i| i.key.is_none()).count();
        let nhash = items.len() - narr;
        // Luau's DUPTABLE: a constructor of `name = value` records only
        // gets a template, made of its keys.
        if narr == 0 && (1..=32).contains(&items.len()) && items.iter().all(|i| i.record) {
            let ids: Vec<u32> = items
                .iter()
                .map(|i| match &i.key {
                    Some(Expr::Str(s)) => self.k_str(s),
                    _ => 0,
                })
                .collect();
            self.konst(KKey::Table(ids.clone()), Value::Nil, Some(Special::Table(ids.into())));
        }
        self.emit(Op::NewTable(
            d,
            narr.min(u16::MAX as usize) as u16,
            nhash.min(u16::MAX as usize) as u16,
        ));
        let mut pending = 0usize;
        let mut first = 1u32;
        for (i, item) in items.iter().enumerate() {
            match &item.key {
                None if i + 1 == items.len() && item.value.is_multi() => {
                    self.multi(&item.value);
                    self.emit(Op::SetList(d, first << 8 | u32::from(MULTI8)));
                    pending = 0;
                    self.g.free = dest + 1;
                }
                None => {
                    let r = self.alloc(1);
                    self.exp_to(&item.value, r);
                    pending += 1;
                    if pending == LIST_BATCH {
                        self.emit(Op::SetList(d, first << 8 | pending as u32));
                        first += pending as u32;
                        pending = 0;
                        self.g.free = dest + 1;
                    }
                }
                Some(k) => {
                    if pending > 0 {
                        self.emit(Op::SetList(d, first << 8 | pending as u32));
                        first += pending as u32;
                        pending = 0;
                        self.g.free = dest + 1;
                    }
                    let save = self.g.free;
                    match self.fold(k) {
                        Some(Value::Str(s)) => {
                            let rv = self.exp_any(&item.value);
                            let rv = self.r16(rv);
                            let kk = self.k_str(&s);
                            let kk = self.k16(kk);
                            self.emit(Op::SetField(d, kk, rv));
                        }
                        _ => {
                            let rk = self.exp_any(k);
                            let rv = self.exp_any(&item.value);
                            let (rk, rv) = (self.r16(rk), self.r16(rv));
                            self.emit(Op::SetTable(d, rk, rv));
                        }
                    }
                    self.g.free = save;
                }
            }
        }
        if pending > 0 {
            self.emit(Op::SetList(d, first << 8 | pending as u32));
            self.g.free = dest + 1;
        }
        if first >= 1 << 24 {
            self.fail("table constructor has too many items");
        }
        if !items.is_empty() {
            self.emit(Op::Charge(items.len() as u32));
        }
    }
    /// A call; its results start at the register returned (`nres` of them,
    /// or as many as there are).
    fn call(&mut self, e: &'a Expr, nres: u16) -> usize {
        let base = self.alloc(1);
        let b = self.r16(base);
        match e {
            Expr::Call(f, args) => {
                self.exp_to(f, base);
                let n = self.args(args);
                let pc = self.emit(Op::Call(b, n, nres));
                self.note(pc, f.desc(), Desc::None);
            }
            Expr::Method(o, name, args) => {
                let ro = self.exp_any(o);
                let ro = self.r16(ro);
                let k = self.k_str(name);
                let k = self.k16(k);
                let pc = self.emit(Op::Method(b, ro, k));
                self.note(pc, o.desc(), Desc::None);
                self.g.free = base + 1;
                self.alloc(1);
                let n = self.args(args);
                self.emit(Op::Namecall(b, k));
                self.emit(Op::Call(b, if n == MULTI { MULTI } else { n + 1 }, nres));
            }
            other => self.exp_to(other, base),
        }
        let nres = if nres == MULTI { 0 } else { nres as usize };
        self.g.free = base + nres;
        self.g.maxstack = self.g.maxstack.max(base + nres);
        base
    }
    /// Arguments from the next register: their count, or MULTI.
    fn args(&mut self, args: &'a [Expr]) -> u16 {
        for (i, a) in args.iter().enumerate() {
            if i + 1 == args.len() && a.is_multi() {
                self.multi(a);
                return MULTI;
            }
            let r = self.alloc(1);
            self.exp_to(a, r);
        }
        if args.len() >= MULTI as usize {
            self.fail("function call has too many arguments");
        }
        args.len() as u16
    }
    /// All of a call's or `...`'s values, from the next register up.
    fn multi(&mut self, e: &'a Expr) {
        match e {
            Expr::Call(..) | Expr::Method(..) => {
                self.call(e, MULTI);
            }
            Expr::Vararg => {
                let r = self.alloc(1);
                let r = self.r16(r);
                self.emit(Op::Vararg(r, MULTI));
            }
            other => {
                let r = self.alloc(1);
                self.exp_to(other, r);
            }
        }
    }
    /// Values into the next registers: exactly `want` of them, or all
    /// (then the count, or MULTI).
    fn explist(&mut self, exprs: &'a [Expr], want: Option<usize>) -> u16 {
        let start = self.g.free;
        let mut have = 0;
        for (i, e) in exprs.iter().enumerate() {
            if i + 1 == exprs.len() && e.is_multi() {
                let Some(n) = want else {
                    self.multi(e);
                    return MULTI;
                };
                let need = n.saturating_sub(i);
                if let Expr::Vararg = e {
                    if need > 0 {
                        let r = self.alloc(need);
                        let (r, need16) = (self.r16(r), self.r16(need));
                        self.emit(Op::Vararg(r, need16));
                    }
                } else {
                    let need16 = self.r16(need);
                    self.call(e, need16);
                }
                have = i + need;
            } else {
                let r = self.alloc(1);
                self.exp_to(e, r);
                have = i + 1;
            }
        }
        match want {
            None => self.r16(have),
            Some(n) => {
                if have < n {
                    self.g.free = start + have;
                    let r = self.alloc(n - have);
                    let (r, k) = (self.r16(r), self.r16(n - have));
                    self.emit(Op::LoadNil(r, k));
                }
                self.g.free = start + n;
                self.g.maxstack = self.g.maxstack.max(self.g.free);
                self.r16(n)
            }
        }
    }
    /// Compiles a function inside this one.
    fn function(&mut self, f: &'a FuncAst) -> Proto {
        let child = Gen::new(f);
        let parent = std::mem::replace(&mut self.g, child);
        self.outer.push(parent);
        self.body();
        let parent = self.outer.pop().expect("the enclosing function");
        let child = std::mem::replace(&mut self.g, parent);
        child.finish(&self.chunk)
    }
    fn closure(&mut self, f: &'a FuncAst, dest: usize) {
        let proto = self.function(f);
        let id = self.g.protos.len();
        // Luau shares a closure whose upvalues cannot change (DUPCLOSURE),
        // which takes a constant; here closures are always new, but the
        // constant keeps the numbering.
        let share = !self.fenv && self.closure_shares(&proto);
        if share && id < u16::MAX as usize {
            self.konst(KKey::Closure(id), Value::Nil, Some(Special::Closure(id as u16)));
        }
        self.g.protos.push(Rc::new(proto));
        let (d, p) = (self.r16(dest), self.r16(id));
        self.emit(Op::Closure(d, p));
    }
    fn closure_shares(&self, p: &Proto) -> bool {
        p.upvals.iter().all(|u| {
            if u.from_local {
                // The cell of a local of this function.
                self.g
                    .locs
                    .iter()
                    .zip(&self.g.f.vars)
                    .zip(&self.g.var_loops)
                    .any(|((l, v), loops)| {
                        matches!(l, Loc::Cell(c) if *c as usize == u.index)
                            && !v.written
                            && *loops == 0
                            && self.outer.is_empty()
                    })
            } else {
                self.g.ups_share.get(u.index).copied().unwrap_or(false)
            }
        })
    }

    // ------------------------------------------------------------- statements

    fn body(&mut self) {
        let f = self.g.f;
        self.g.line = f.line;
        for v in 0..f.nparams {
            let r = self.alloc(1);
            let r16 = self.r16(r);
            let cell = if f.vars[v].captured {
                let c = self.g.ncells as u16;
                self.g.ncells += 1;
                self.emit(Op::NewCell(c, r16));
                Some(c)
            } else {
                None
            };
            self.g.locs[v] = match cell {
                Some(c) => Loc::Cell(c),
                None => Loc::Reg(r16),
            };
            self.open_local(v, r16, cell);
        }
        self.g.nactive = f.nparams;
        self.block(&f.body);
        self.emit(Op::Return(0, 0));
        let end = self.pc() as u32;
        for l in &mut self.g.locals {
            l.end = l.end.min(end);
        }
    }
    fn open_local(&mut self, v: VarId, reg: u16, cell: Option<u16>) {
        let start = self.pc() as u32;
        self.g.var_loops[v] = self.g.loops;
        self.g.scope.push(self.g.locals.len());
        self.g.locals.push(LocalVar {
            name: self.g.f.vars[v].name.clone(),
            reg,
            cell,
            start,
            end: u32::MAX,
        });
    }
    fn close_locals(&mut self, keep: usize) {
        let end = self.pc() as u32;
        while self.g.scope.len() > keep {
            if let Some(i) = self.g.scope.pop() {
                self.g.locals[i].end = end;
            }
        }
    }
    fn block(&mut self, stmts: &'a [Stmt]) {
        let (nactive, scope) = (self.g.nactive, self.g.scope.len());
        for s in stmts {
            self.stmt(s);
        }
        self.close_locals(scope);
        self.g.nactive = nactive;
        self.g.free = nactive;
    }
    /// A loop's body: the breaks and the continues in it.
    fn loop_body(&mut self, stmts: &'a [Stmt]) -> (Vec<usize>, Vec<usize>) {
        self.g.breaks.push(Vec::new());
        self.g.continues.push(Vec::new());
        self.g.loops += 1;
        self.block(stmts);
        self.g.loops -= 1;
        (
            self.g.breaks.pop().unwrap_or_default(),
            self.g.continues.pop().unwrap_or_default(),
        )
    }
    /// Declares `var` in register `reg` (a cell if a closure captured it).
    fn declare(&mut self, var: VarId, reg: usize) {
        let r = self.r16(reg);
        let cell = if self.g.f.vars[var].captured {
            let c = self.g.ncells as u16;
            self.g.ncells += 1;
            self.emit(Op::NewCell(c, r));
            Some(c)
        } else {
            None
        };
        self.g.locs[var] = match cell {
            Some(c) => Loc::Cell(c),
            None => Loc::Reg(r),
        };
        self.open_local(var, r, cell);
    }
    fn stmt(&mut self, s: &'a Stmt) {
        self.g.line = s.line;
        self.emit(Op::Step(s.line));
        match &s.kind {
            StmtKind::Local(vars, exprs) => self.local(vars, exprs),
            StmtKind::LocalFunction(var, f) => {
                let reg = self.alloc(1);
                let r = self.r16(reg);
                let cell = if self.g.f.vars[*var].captured {
                    let c = self.g.ncells as u16;
                    self.g.ncells += 1;
                    self.emit(Op::NewCell(c, NO_REG));
                    Some(c)
                } else {
                    None
                };
                self.g.locs[*var] = match cell {
                    Some(c) => Loc::Cell(c),
                    None => Loc::Reg(r),
                };
                self.open_local(*var, r, cell);
                self.g.nactive = reg + 1;
                self.closure(f, reg);
                if let Some(c) = cell {
                    self.emit(Op::SetCell(r, c));
                }
            }
            StmtKind::Function(target, f) => {
                if let Expr::Local(v, _) = target
                    && let Loc::Reg(r) = self.g.locs[*v]
                {
                    self.closure(f, r as usize);
                } else {
                    let t = self.alloc(1);
                    self.closure(f, t);
                    self.store_one(target, t);
                }
            }
            StmtKind::Assign(targets, values) => self.assign(targets, values),
            StmtKind::Call(e) => {
                self.call(e, 0);
            }
            StmtKind::Do(body) => self.block(body),
            StmtKind::While(cond, body) => {
                let top = self.pc();
                self.emit(Op::Tick);
                let exits = match self.fold(cond) {
                    Some(v) if !v.truthy() => return,
                    Some(_) => Vec::new(),
                    None => self.jump_if_false(cond),
                };
                let (breaks, continues) = self.loop_body(body);
                let j = self.emit(Op::Jmp(0));
                self.patch(j, top);
                for c in continues {
                    self.patch(c, top);
                }
                self.patch_here(&exits);
                self.patch_here(&breaks);
            }
            StmtKind::Repeat(body, cond) => {
                let top = self.pc();
                let (nactive, scope) = (self.g.nactive, self.g.scope.len());
                self.g.breaks.push(Vec::new());
                self.g.continues.push(Vec::new());
                self.g.loops += 1;
                for s in body {
                    self.stmt(s);
                }
                let continues = self.g.continues.pop().unwrap_or_default();
                self.patch_here(&continues);
                match self.fold(cond) {
                    Some(v) if v.truthy() => {}
                    Some(_) => {
                        let j = self.emit(Op::Jmp(0));
                        self.patch(j, top);
                    }
                    None => {
                        let r = self.exp_any(cond);
                        let r = self.r16(r);
                        let j = self.emit(Op::JmpIfNot(r, 0));
                        self.patch(j, top);
                    }
                }
                self.g.loops -= 1;
                let breaks = self.g.breaks.pop().unwrap_or_default();
                self.close_locals(scope);
                self.g.nactive = nactive;
                self.patch_here(&breaks);
            }
            StmtKind::If(arms, otherwise) => {
                let mut ends = Vec::new();
                let mut done = false;
                for (i, (cond, body)) in arms.iter().enumerate() {
                    match self.fold(cond) {
                        Some(v) if v.truthy() => {
                            self.block(body);
                            done = true;
                            break;
                        }
                        Some(_) => {}
                        None => {
                            let skip = self.jump_if_false(cond);
                            self.block(body);
                            if i + 1 < arms.len() || otherwise.is_some() {
                                ends.push(self.emit(Op::Jmp(0)));
                            }
                            self.patch_here(&skip);
                        }
                    }
                }
                if !done && let Some(b) = otherwise {
                    self.block(b);
                }
                self.patch_here(&ends);
            }
            StmtKind::NumFor {
                base,
                var,
                start,
                limit,
                step,
                body,
            } => self.num_for(*base, *var, [start, limit], step.as_ref(), body),
            StmtKind::GenFor {
                base,
                vars,
                exprs,
                body,
            } => self.gen_for(*base, vars, exprs, body),
            StmtKind::Return(exprs) => self.ret(exprs),
            StmtKind::Break => {
                let j = self.emit(Op::Jmp(0));
                if let Some(b) = self.g.breaks.last_mut() {
                    b.push(j);
                }
            }
            StmtKind::Continue => {
                let j = self.emit(Op::Jmp(0));
                if let Some(c) = self.g.continues.last_mut() {
                    c.push(j);
                }
            }
        }
        self.g.free = self.g.nactive;
    }
    /// Jumps (to patch) taken when `cond` is false.
    fn jump_if_false(&mut self, cond: &'a Expr) -> Vec<usize> {
        let save = self.g.free;
        let r = self.exp_any(cond);
        let r = self.r16(r);
        self.g.free = save;
        vec![self.emit(Op::JmpIfNot(r, 0))]
    }
    fn local(&mut self, vars: &'a [VarId], exprs: &'a [Expr]) {
        let multi = exprs.last().is_some_and(Expr::is_multi);
        let consts: Vec<Option<Value>> = vars
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                if self.g.f.vars[v].written {
                    return None;
                }
                match exprs.get(i) {
                    Some(e) if i + 1 == exprs.len() && multi => {
                        let _ = e;
                        None
                    }
                    Some(e) => self.fold(e),
                    None if multi => None,
                    None => Some(Value::Nil),
                }
            })
            .collect();
        // Luau leaves out a statement of constant locals: their uses get the values.
        if exprs.len() <= vars.len() && consts.iter().all(Option::is_some) {
            for (&v, c) in vars.iter().zip(consts) {
                self.g.locs[v] = Loc::Const(c.unwrap_or_default());
                self.g.var_loops[v] = self.g.loops;
            }
            return;
        }
        let base = self.g.free;
        self.explist(exprs, Some(vars.len()));
        for (i, (&v, c)) in vars.iter().zip(consts).enumerate() {
            match c {
                Some(c) => {
                    let r = self.r16(base + i);
                    self.open_local(v, r, None);
                    self.g.locs[v] = Loc::Const(c);
                }
                None => self.declare(v, base + i),
            }
        }
        self.g.nactive = base + vars.len();
        self.g.free = self.g.nactive;
    }
    /// Stores register `src` into one assignment target.
    fn store_one(&mut self, target: &'a Expr, src: usize) {
        let s = self.r16(src);
        match target {
            Expr::Local(v, _) => match self.g.locs[*v].clone() {
                Loc::Reg(r) => {
                    self.emit(Op::Move(r, s));
                }
                Loc::Cell(c) => {
                    self.emit(Op::SetCell(s, c));
                }
                Loc::Const(_) | Loc::None => {}
            },
            Expr::Upval(i, _) => {
                if let UpLoc::Index(u) = self.up(*i) {
                    self.emit(Op::SetUpval(s, u));
                }
            }
            Expr::Global(n) => {
                let k = self.k_str(n);
                self.emit(Op::SetGlobal(s, k));
            }
            Expr::Index(o, k) => {
                let ro = self.exp_any(o);
                let ro = self.r16(ro);
                let pc = match self.fold(k) {
                    Some(Value::Str(name)) => {
                        let kk = self.k_str(&name);
                        let kk = self.k16(kk);
                        self.emit(Op::SetField(ro, kk, s))
                    }
                    _ => {
                        let rk = self.exp_any(k);
                        let rk = self.r16(rk);
                        self.emit(Op::SetTable(ro, rk, s))
                    }
                };
                self.note(pc, o.desc(), Desc::None);
            }
            _ => {}
        }
    }
    fn assign(&mut self, targets: &'a [Expr], values: &'a [Expr]) {
        if let ([t], [v]) = (targets, values) {
            match t {
                Expr::Local(var, _) => match self.g.locs[*var].clone() {
                    Loc::Reg(r) => self.exp_to(v, r as usize),
                    Loc::Cell(c) => {
                        let r = self.exp_any(v);
                        let r = self.r16(r);
                        self.emit(Op::SetCell(r, c));
                    }
                    Loc::Const(_) | Loc::None => {
                        self.exp_any(v);
                    }
                },
                Expr::Upval(i, _) => {
                    let u = self.up(*i);
                    let r = self.exp_any(v);
                    let r = self.r16(r);
                    if let UpLoc::Index(u) = u {
                        self.emit(Op::SetUpval(r, u));
                    }
                }
                Expr::Global(n) => {
                    let r = self.exp_any(v);
                    let r = self.r16(r);
                    let k = self.k_str(n);
                    self.emit(Op::SetGlobal(r, k));
                }
                Expr::Index(o, k) => {
                    let ro = self.exp_any(o);
                    let ro = self.r16(ro);
                    let field = match self.fold(k) {
                        Some(Value::Str(s)) => Err(s),
                        _ => {
                            let rk = self.exp_any(k);
                            Ok(self.r16(rk))
                        }
                    };
                    let rv = self.exp_any(v);
                    let rv = self.r16(rv);
                    let pc = match field {
                        Err(name) => {
                            let kk = self.k_str(&name);
                            let kk = self.k16(kk);
                            self.emit(Op::SetField(ro, kk, rv))
                        }
                        Ok(rk) => self.emit(Op::SetTable(ro, rk, rv)),
                    };
                    self.note(pc, o.desc(), Desc::None);
                }
                _ => {}
            }
            return;
        }
        // Several: the places first (copied, as later stores may change the
        // locals they use), then the values, then the stores in order.
        let mut places = Vec::with_capacity(targets.len());
        for t in targets {
            let p = match t {
                Expr::Local(v, _) => match self.g.locs[*v] {
                    Loc::Reg(r) => Place::Reg(r),
                    Loc::Cell(c) => Place::Cell(c),
                    _ => Place::None,
                },
                Expr::Upval(i, _) => match self.up(*i) {
                    UpLoc::Index(u) => Place::Up(u),
                    _ => Place::None,
                },
                Expr::Global(n) => Place::Global(n.clone()),
                Expr::Index(o, k) => {
                    let ro = self.alloc(1);
                    self.exp_to(o, ro);
                    let ro = self.r16(ro);
                    match self.fold(k) {
                        Some(Value::Str(s)) => Place::Field(ro, s, o.desc()),
                        _ => {
                            let rk = self.alloc(1);
                            self.exp_to(k, rk);
                            let rk = self.r16(rk);
                            Place::Table(ro, rk, o.desc())
                        }
                    }
                }
                _ => Place::None,
            };
            places.push(p);
        }
        let start = self.g.free;
        self.explist(values, Some(targets.len()));
        for (i, p) in places.into_iter().enumerate() {
            let s = self.r16(start + i);
            match p {
                Place::None => {}
                Place::Reg(r) => {
                    self.emit(Op::Move(r, s));
                }
                Place::Cell(c) => {
                    self.emit(Op::SetCell(s, c));
                }
                Place::Up(u) => {
                    self.emit(Op::SetUpval(s, u));
                }
                Place::Global(n) => {
                    let k = self.k_str(&n);
                    self.emit(Op::SetGlobal(s, k));
                }
                Place::Field(o, name, desc) => {
                    let kk = self.k_str(&name);
                    let kk = self.k16(kk);
                    let pc = self.emit(Op::SetField(o, kk, s));
                    self.note(pc, desc, Desc::None);
                }
                Place::Table(o, k, desc) => {
                    let pc = self.emit(Op::SetTable(o, k, s));
                    self.note(pc, desc, Desc::None);
                }
            }
        }
    }
    fn ret(&mut self, exprs: &'a [Expr]) {
        match exprs {
            [] => {
                self.emit(Op::Return(0, 0));
            }
            [e @ (Expr::Call(..) | Expr::Method(..))] => {
                let b = self.call(e, MULTI);
                let b = self.r16(b);
                self.emit(Op::Return(b, MULTI));
            }
            [Expr::Local(v, _)] if matches!(self.g.locs[*v], Loc::Reg(_)) => {
                if let Loc::Reg(r) = self.g.locs[*v] {
                    self.emit(Op::Return(r, 1));
                }
            }
            _ => {
                let start = self.g.free;
                let n = self.explist(exprs, None);
                let s = self.r16(start);
                self.emit(Op::Return(s, n));
            }
        }
    }
    fn num_for(
        &mut self,
        base: VarId,
        var: VarId,
        [start, limit]: [&'a Expr; 2],
        step: Option<&'a Expr>,
        body: &'a [Stmt],
    ) {
        let a = self.g.nactive;
        self.g.free = a;
        self.alloc(5);
        let a16 = self.r16(a);
        for (i, (e, what)) in [(Some(start), 0u8), (Some(limit), 1), (step, 2)]
            .into_iter()
            .enumerate()
        {
            match e {
                Some(e) => {
                    let known = matches!(self.fold(e), Some(Value::Num(_)));
                    self.exp_to(e, a + i);
                    if !known {
                        let r = self.r16(a + i);
                        self.emit(Op::ForCheck(r, what));
                    }
                }
                None => self.load_const(&Value::Num(1.0), a + i),
            }
        }
        let scope = self.g.scope.len();
        for i in 0..4 {
            let r = self.r16(a + i);
            self.g.locs[base + i] = Loc::Reg(r);
            self.open_local(base + i, r, None);
        }
        self.g.nactive = a + 5;
        self.g.free = a + 5;
        let prep = self.emit(Op::ForPrep(a16, 0));
        let top = self.pc();
        self.g.loops += 1;
        self.declare(var, a + 4);
        self.g.loops -= 1;
        let (breaks, continues) = self.loop_body(body);
        self.patch_here(&continues);
        let lp = self.emit(Op::ForLoop(a16, 0));
        self.patch(lp, top);
        self.patch_here(&[prep]);
        self.patch_here(&breaks);
        self.close_locals(scope);
        self.g.nactive = a;
    }
    fn gen_for(&mut self, base: VarId, vars: &'a [VarId], exprs: &'a [Expr], body: &'a [Stmt]) {
        let a = self.g.nactive;
        self.g.free = a;
        self.explist(exprs, Some(3));
        let a16 = self.r16(a);
        let scope = self.g.scope.len();
        for i in 0..3 {
            let r = self.r16(a + i);
            self.g.locs[base + i] = Loc::Reg(r);
            self.open_local(base + i, r, None);
        }
        self.emit(Op::TForPrep(a16));
        let n = vars.len();
        self.g.free = a + 3;
        self.alloc(n.max(3));
        self.g.nactive = a + 3 + n;
        self.g.free = a + 3 + n;
        let top = self.pc();
        let n16 = self.r16(n);
        self.emit(Op::TForCall(a16, n16));
        let exit = self.emit(Op::TForLoop(a16, 0));
        self.g.loops += 1;
        for (i, &v) in vars.iter().enumerate() {
            self.declare(v, a + 3 + i);
        }
        self.g.loops -= 1;
        let (breaks, continues) = self.loop_body(body);
        let j = self.emit(Op::Jmp(0));
        self.patch(j, top);
        for c in continues {
            self.patch(c, top);
        }
        self.patch_here(&[exit]);
        self.patch_here(&breaks);
        self.close_locals(scope);
        self.g.nactive = a;
    }
}

/// The interpreter's own functions that compiled code calls as constants.
fn builtin_value(name: &'static str) -> Value {
    match name {
        "interpolate" => Native::builtin("interpolate", interpolate),
        "next" => Native::builtin("next", raw_next),
        _ => Value::Nil,
    }
}

/// `next` without `__pairs`, for generalized iteration.
fn raw_next(lua: &mut Lua, _: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let Some(Value::Table(t)) = args.first() else {
        return Err(lua.rt_error("bad argument #1 to 'next' (table expected)"));
    };
    let k = args.get(1).cloned().unwrap_or_default();
    match t.borrow().next(&k) {
        Ok(Some((k, v))) => Ok(vec![k, v]),
        Ok(None) => Ok(vec![Value::Nil]),
        Err(()) => Err(lua.rt_error("invalid key to 'next'")),
    }
}

/// An interpolated string: its pieces, each as `tostring` gives it, joined.
fn interpolate(lua: &mut Lua, h: &mut dyn Host, _: &Native, args: Vec<Value>) -> Res {
    let mut out: Vec<u8> = Vec::new();
    for a in &args {
        match a {
            Value::Str(s) => out.extend_from_slice(s),
            other => out.extend_from_slice(lua.tostring(h, other)?.as_bytes()),
        }
        if out.len() > MAX_STRING {
            return Err(lua.rt_error("string length overflow"));
        }
    }
    Ok(vec![Value::Str(out.into())])
}

// ----------------------------------------------------------------- closures

/// A Lua function: a body behind a cell, so `hookfunction` can change
/// what an existing closure runs and keep its identity.
pub struct Closure {
    body: RefCell<Rc<Body>>,
    /// What a call runs instead (`hookfunction`).
    redirect: RefCell<Option<Value>>,
}

/// What a closure runs: its function, its upvalues and its globals.
pub struct Body {
    pub proto: Rc<Proto>,
    pub upvals: Vec<Cell>,
    pub env: TableRef,
}

#[allow(dead_code)]
impl Closure {
    pub fn new(proto: Rc<Proto>, upvals: Vec<Cell>, env: TableRef) -> Closure {
        Closure::from_body(Rc::new(Body { proto, upvals, env }))
    }
    pub fn from_body(body: Rc<Body>) -> Closure {
        Closure {
            body: RefCell::new(body),
            redirect: RefCell::new(None),
        }
    }
    /// What a call to it runs instead, if it is hooked.
    pub fn redirect(&self) -> Option<Value> {
        self.redirect.borrow().clone()
    }
    /// Hooks it (Some) or unhooks it (None); the hook before.
    pub fn set_redirect(&self, to: Option<Value>) -> Option<Value> {
        std::mem::replace(&mut *self.redirect.borrow_mut(), to)
    }
    pub fn body(&self) -> Rc<Body> {
        self.body.borrow().clone()
    }
    pub fn proto(&self) -> Rc<Proto> {
        self.body.borrow().proto.clone()
    }
    pub fn env(&self) -> TableRef {
        self.body.borrow().env.clone()
    }
    /// Runs `body` from now on (calls already running keep theirs); the
    /// old body back.
    pub fn replace_body(&self, body: Rc<Body>) -> Rc<Body> {
        std::mem::replace(&mut *self.body.borrow_mut(), body)
    }
    pub fn upvalue(&self, i: usize) -> Option<Value> {
        self.body.borrow().upvals.get(i).map(|c| c.borrow().clone())
    }
    pub fn set_upvalue(&self, i: usize, v: Value) -> bool {
        match self.body.borrow().upvals.get(i) {
            Some(c) => {
                *c.borrow_mut() = v;
                true
            }
            None => false,
        }
    }
}

// ----------------------------------------------------------------- runtime

/// What to put before a call's results: nothing, or what the innermost
/// protected-call record says (`true` for pcall, `false` for an xpcall
/// handler's results), and so on outwards.
#[derive(Clone, Copy)]
enum Ret {
    Plain,
    Wrapped,
}

/// A Lua function running.
struct Frame {
    #[allow(dead_code)]
    closure: Rc<Closure>,
    body: Rc<Body>,
    /// The next instruction, while another frame runs.
    pc: usize,
    base: usize,
    /// Where its results go, in the caller's registers.
    func: usize,
    nres: u16,
    varargs: usize,
    nvarargs: usize,
    /// Where its cells start.
    cells: usize,
    line: u32,
    ret: Ret,
}

/// A pcall or xpcall in progress (`catches`), or an xpcall handler running
/// (whose results get `false` in front).
struct Protect {
    /// The frame count when it began: an error unwinds to it.
    frames: usize,
    func: usize,
    nres: u16,
    cells: usize,
    handler: Option<Value>,
    catches: bool,
    ret: Ret,
}

/// The call a coroutine yielded from: where its results go once resumed.
struct Pending {
    func: usize,
    nres: u16,
    ret: Ret,
}

/// A thread's own stack: registers, frames, cells and protected calls.
#[derive(Default)]
struct ThreadState {
    stack: Vec<Value>,
    frames: Vec<Frame>,
    cells: Vec<Cell>,
    pcalls: Vec<Protect>,
    /// The end of the last multiple results.
    top: usize,
    /// VM loops running on this thread: past one, a yield would cross a
    /// native's call into Lua.
    runs: u32,
    pending: Option<Pending>,
    /// A coroutine's function, before its first resume.
    start: Option<Value>,
}

/// The live objects, when tracked (`getgc`).
#[derive(Default)]
struct Registry {
    closures: Vec<Weak<Closure>>,
    tables: Vec<Weak<RefCell<Table>>>,
    threads: Vec<Weak<Thread>>,
    prune_at: usize,
}

/// Debug information about a running Lua function.
#[allow(dead_code)]
pub struct FrameInfo {
    pub closure: Rc<Closure>,
    pub proto: Rc<Proto>,
    pub line: u32,
    pub pc: usize,
}

#[derive(Clone, Copy)]
enum Ev {
    Index,
    NewIndex,
    Call,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Unm,
    IDiv,
    BAnd,
    BOr,
    BXor,
    Shl,
    Shr,
    BNot,
    Concat,
    Len,
    Eq,
    Lt,
    Le,
    ToString,
    Namecall,
}

const EV_NAMES: [&str; 24] = [
    "__index",
    "__newindex",
    "__call",
    "__add",
    "__sub",
    "__mul",
    "__div",
    "__mod",
    "__pow",
    "__unm",
    "__idiv",
    "__band",
    "__bor",
    "__bxor",
    "__shl",
    "__shr",
    "__bnot",
    "__concat",
    "__len",
    "__eq",
    "__lt",
    "__le",
    "__tostring",
    "__namecall",
];

thread_local! {
    /// The metamethods' names as table keys, made once.
    static EVENTS: Rc<[Key]> = EV_NAMES.iter().map(|n| Key::Str(Rc::from(n.as_bytes()))).collect();
}

fn arith_event(op: BinOp) -> Ev {
    match op {
        BinOp::Add => Ev::Add,
        BinOp::Sub => Ev::Sub,
        BinOp::Mul => Ev::Mul,
        BinOp::Div => Ev::Div,
        BinOp::IDiv => Ev::IDiv,
        BinOp::Mod => Ev::Mod,
        BinOp::Pow => Ev::Pow,
        BinOp::BAnd => Ev::BAnd,
        BinOp::BOr => Ev::BOr,
        BinOp::BXor => Ev::BXor,
        BinOp::Shl => Ev::Shl,
        _ => Ev::Shr,
    }
}

/// Steps a call is given again, while its deadline lasts.
const REFILL_STEPS: u64 = 1_000_000;

pub struct Lua {
    /// Steps left before the running script is stopped.
    pub steps: u64,
    /// Until when a call may go on once its steps run out (Roblox's script
    /// timeout): they are given again, a million at a time, until then.
    pub deadline: Option<std::time::Instant>,
    /// The `string` table: methods of strings (`s:upper()`).
    pub string_lib: Option<TableRef>,
    /// The strings' metatable, as getmetatable("") gives it: read-only,
    /// its `__index` the string library.
    pub string_meta: Option<TableRef>,
    /// The random generator's state (math.random).
    pub rng: u64,
    pub started: std::time::Instant,
    /// The running thread's stack.
    cur: ThreadState,
    thread: Rc<Thread>,
    /// Natives calling into Lua and resumes, nested.
    ccalls: usize,
    stack_base: usize,
    /// The method name of the running `obj:Name()` call on a userdata.
    namecall: Option<Str>,
    registry: Option<Registry>,
    events: Rc<[Key]>,
    nil_cell: Cell,
}

impl Default for Lua {
    fn default() -> Self {
        Lua::new()
    }
}

const HOST_CHUNK: &str = "[host]";

impl Lua {
    pub fn new() -> Lua {
        Lua {
            steps: 1_000_000,
            deadline: None,
            string_lib: None,
            string_meta: None,
            rng: 0x2545_F491_4F6C_DD1D,
            started: std::time::Instant::now(),
            cur: ThreadState::default(),
            thread: Rc::new(Thread {
                state: RefCell::new(None),
                status: std::cell::Cell::new(Status::Running),
                main: true,
                error: RefCell::new(None),
            }),
            ccalls: 0,
            stack_base: 0,
            namecall: None,
            registry: None,
            events: EVENTS.with(Rc::clone),
            nil_cell: Rc::new(RefCell::new(Value::Nil)),
        }
    }

    /// Compiles `src` into a function whose globals are `env`.
    pub fn load(&mut self, src: &str, chunk: &str, env: &TableRef) -> Result<Value, String> {
        self.load_bytes(src.as_bytes(), chunk, env)
    }
    pub fn load_bytes(&mut self, src: &[u8], chunk: &str, env: &TableRef) -> Result<Value, String> {
        SYNTAX_AT.with(|s| s.set(None));
        let (toks, starts) = lex(src, chunk)?;
        let mut parser = Parser {
            toks,
            starts,
            pos: 0,
            chunk: chunk.into(),
            fs: Vec::new(),
            depth: 0,
            stack_base: stack_addr(),
            written: HashSet::new(),
            fenv: false,
        };
        let ast = parser.chunk()?;
        let proto = compile(&ast, &parser.chunk, &parser.written, parser.fenv)?;
        let c = Rc::new(Closure::new(Rc::new(proto), Vec::new(), env.clone()));
        self.track_closure(&c);
        Ok(Value::Func(c))
    }

    // ------------------------------------------------------------- positions

    /// The running Lua function's chunk and line.
    fn here(&self) -> Option<(Rc<str>, u32)> {
        self.cur.frames.last().map(|f| (f.body.proto.chunk.clone(), f.line))
    }
    /// An error at the running statement: `chunk:line: msg`, with a traceback.
    pub fn rt_error(&self, msg: impl AsRef<str>) -> Throw {
        let text = match self.here() {
            None => msg.as_ref().to_owned(),
            Some((chunk, line)) => format!("{chunk}:{line}: {}", msg.as_ref()),
        };
        Throw::Error(Value::str(&text), self.traceback())
    }
    /// A fatal error at the running statement (`[host]:0:` outside Lua).
    fn fatal_here(&self, msg: &str) -> Throw {
        let (chunk, line) = self.here().unwrap_or_else(|| (HOST_CHUNK.into(), 0));
        Throw::Fatal(format!("{chunk}:{line}: {msg}"))
    }
    /// Where `error(msg, level)` puts its message: level 1 is the running
    /// function, 2 its caller.
    pub fn position(&self, level: usize) -> Option<String> {
        if level == 0 {
            return None;
        }
        let i = self.cur.frames.len().checked_sub(level)?;
        let f = &self.cur.frames[i];
        Some(format!("{}:{}:", f.body.proto.chunk, f.line))
    }
    pub fn traceback(&self) -> String {
        let mut lines = Vec::new();
        for f in self.cur.frames.iter().rev() {
            let p = &f.body.proto;
            if &*p.name == "main chunk" {
                lines.push(format!("{}:{}: in main chunk", p.chunk, f.line));
            } else {
                lines.push(format!("{}:{}: in {}", p.chunk, f.line, p.name));
            }
        }
        if lines.len() > 12 {
            let tail = lines.split_off(lines.len() - 5);
            lines.truncate(6);
            lines.push("...".into());
            lines.extend(tail);
        }
        lines.join("\n")
    }
    /// Out of steps: more until the deadline (REFILL_STEPS at a time), else the error.
    #[cold]
    fn out_of_steps(&mut self) -> Option<Throw> {
        if self.deadline.is_some_and(|d| std::time::Instant::now() < d) {
            self.steps = REFILL_STEPS;
            return None;
        }
        Some(self.steps_error())
    }
    #[cold]
    fn steps_error(&self) -> Throw {
        let at = match self.here() {
            None => String::new(),
            Some((chunk, line)) => format!("{chunk}:{line}: "),
        };
        Throw::Fatal(format!("{at}script ran too long and was stopped (an endless loop?)"))
    }
    #[inline]
    fn step(&mut self) -> Result<(), Throw> {
        if self.steps == 0
            && let Some(e) = self.out_of_steps()
        {
            return Err(e);
        }
        self.steps -= 1;
        Ok(())
    }
    /// Spends `n` steps at once (string and table built-ins that loop).
    pub fn charge(&mut self, n: usize) -> Result<(), Throw> {
        let n = n as u64 / 16;
        if n > self.steps {
            self.steps = 0;
            return self.step();
        }
        self.steps -= n;
        Ok(())
    }

    // ------------------------------------------------------------- calls

    /// Calls `f` with `args`.
    pub fn call(&mut self, h: &mut dyn Host, f: &Value, args: Vec<Value>) -> Res {
        if self.ccalls == 0 {
            self.stack_base = stack_addr();
        }
        match self.call_value(h, f, args) {
            Err(Throw::Yield(_)) => Err(self.rt_error(self.yield_error())),
            r => r,
        }
    }
    /// The first result of a call, or nil.
    pub fn call1(&mut self, h: &mut dyn Host, f: &Value, args: Vec<Value>) -> Result<Value, Throw> {
        Ok(self.call(h, f, args)?.into_iter().next().unwrap_or_default())
    }
    fn yield_error(&self) -> &'static str {
        if self.thread.main {
            "attempt to yield from outside a coroutine"
        } else {
            "attempt to yield across a C-call boundary"
        }
    }
    fn call_value(&mut self, h: &mut dyn Host, f: &Value, mut args: Vec<Value>) -> Res {
        let mut f = f.clone();
        for _ in 0..64 {
            match f {
                Value::Func(c) => match c.redirect() {
                    Some(to) => f = to,
                    None => return self.call_lua(h, c, args),
                },
                Value::Native(n) => {
                    if let Some(to) = n.redirect() {
                        f = to;
                        continue;
                    }
                    match &n.kind {
                        NativeKind::Wrap(inner) => {
                            f = inner.clone();
                            continue;
                        }
                        NativeKind::Namecall => {
                            f = self.namecall_target(h, args.first().cloned().unwrap_or_default())?;
                            continue;
                        }
                        _ => {}
                    }
                    self.step()?;
                    return self.call_native(h, &n, args);
                }
                other => match self.meta(&other, Ev::Call) {
                    Some(mm) if !matches!(mm, Value::Table(_)) => {
                        args.insert(0, other);
                        f = mm;
                    }
                    _ => return Err(self.rt_error(format!("attempt to call a {} value", other.type_name()))),
                },
            }
        }
        Err(self.rt_error("'__call' chain too long; possible loop"))
    }
    /// A Lua function called from Rust: a VM loop of its own, until it returns.
    fn call_lua(&mut self, h: &mut dyn Host, c: Rc<Closure>, args: Vec<Value>) -> Res {
        if self.cur.frames.len() >= MAX_DEPTH
            || self.ccalls >= MAX_CCALLS
            || self.stack_base.saturating_sub(stack_addr()) > STACK_LIMIT
        {
            return Err(self.rt_error("stack overflow"));
        }
        self.step()?;
        let func = self.cur.stack.len();
        let n = args.len();
        self.cur.stack.push(Value::Func(c.clone()));
        self.cur.stack.extend(args);
        self.enter(c, func, n, MULTI, Ret::Plain);
        let entry = self.cur.frames.len() - 1;
        self.ccalls += 1;
        self.cur.runs += 1;
        let r = self.run(h, entry);
        self.ccalls -= 1;
        self.cur.runs -= 1;
        r
    }
    fn call_native(&mut self, h: &mut dyn Host, n: &Rc<Native>, args: Vec<Value>) -> Res {
        match &n.kind {
            NativeKind::Builtin(f) => f(self, h, n, args),
            NativeKind::Host(id) => h.call(self, *id, args),
            NativeKind::Pcall(handler) => self.pcall_nested(h, *handler, args),
            NativeKind::Wrap(_) | NativeKind::Namecall => self.call_value(h, &Value::Native(n.clone()), args),
        }
    }
    /// The method `obj:Name(...)` calls, for `__namecall` without a hook.
    fn namecall_target(&mut self, h: &mut dyn Host, obj: Value) -> Result<Value, Throw> {
        let Some(name) = self.namecall.clone() else {
            return Err(self.rt_error("__namecall called without a method call"));
        };
        let f = self.index_raw_meta(h, &obj, &Value::Str(name.clone()))?;
        if !f.is_callable() && self.meta(&f, Ev::Call).is_none() {
            return Err(self.rt_error(format!(
                "attempt to call a {} value (method '{}')",
                f.type_name(),
                String::from_utf8_lossy(&name)
            )));
        }
        Ok(f)
    }
    /// `obj[key]` through `__index`, never `__namecall`.
    fn index_raw_meta(&mut self, h: &mut dyn Host, obj: &Value, key: &Value) -> Result<Value, Throw> {
        self.index(h, obj, key)
    }
    /// pcall and xpcall called from Rust (not from Lua code, where the VM
    /// runs them): protected calls of their own.
    fn pcall_nested(&mut self, h: &mut dyn Host, handler: bool, mut args: Vec<Value>) -> Res {
        if !handler && args.is_empty() {
            return Err(bad_arg(self, 0, "pcall", "value expected"));
        }
        if handler && args.len() < 2 {
            return Err(bad_arg(self, 1, "xpcall", "value expected"));
        }
        let f = args.remove(0);
        let handler = handler.then(|| args.remove(0));
        let r = match self.call_value(h, &f, args) {
            Err(Throw::Yield(_)) => Err(self.rt_error(self.yield_error())),
            r => r,
        };
        match r {
            Ok(mut r) => {
                r.insert(0, Value::Bool(true));
                Ok(r)
            }
            Err(Throw::Error(v, _)) => match handler {
                None => Ok(vec![Value::Bool(false), v]),
                Some(hf) => {
                    let mut r = self.call_value(h, &hf, vec![v])?;
                    r.insert(0, Value::Bool(false));
                    Ok(r)
                }
            },
            Err(other) => Err(other),
        }
    }

    // ------------------------------------------------------------- the VM

    /// Pushes a frame for `c`, whose `n` arguments follow it at `func`.
    fn enter(&mut self, c: Rc<Closure>, func: usize, n: usize, nres: u16, ret: Ret) {
        let body = c.body();
        let p = &body.proto;
        let st = &mut self.cur.stack;
        let (base, varargs, nvarargs) = if p.vararg {
            let nfix = p.nparams;
            let base = func + 1 + n;
            let need = base + p.maxstack.max(nfix);
            if st.len() < need {
                st.resize(need, Value::Nil);
            }
            for i in 0..nfix {
                st[base + i] = if i < n {
                    std::mem::take(&mut st[func + 1 + i])
                } else {
                    Value::Nil
                };
            }
            (base, func + 1 + nfix.min(n), n.saturating_sub(nfix))
        } else {
            let base = func + 1;
            let need = base + p.maxstack.max(p.nparams);
            if st.len() < need {
                st.resize(need, Value::Nil);
            }
            for v in st.iter_mut().skip(base + n).take(p.nparams.saturating_sub(n)) {
                *v = Value::Nil;
            }
            (base, base, 0)
        };
        let cells = self.cur.cells.len();
        if p.ncells > 0 {
            self.cur.cells.resize(cells + p.ncells, self.nil_cell.clone());
        }
        let line = p.line;
        self.cur.frames.push(Frame {
            closure: c,
            body,
            pc: 0,
            base,
            func,
            nres,
            varargs,
            nvarargs,
            cells,
            line,
            ret,
        });
    }

    /// Runs frames until the one at `entry` returns: its results.
    fn run(&mut self, h: &mut dyn Host, entry: usize) -> Res {
        let (entry_func, entry_cells) = match self.cur.frames.get(entry) {
            Some(f) => (f.func, f.cells),
            None => (self.cur.stack.len(), self.cur.cells.len()),
        };
        'frames: loop {
            let Some(fi) = self.cur.frames.len().checked_sub(1) else {
                return Ok(Vec::new());
            };
            let (body, base, cb, mut pc) = {
                let f = &self.cur.frames[fi];
                (f.body.clone(), f.base, f.cells, f.pc)
            };
            let p: &Proto = &body.proto;
            let code = &p.code[..];
            let k = &p.constants[..];
            let err: Throw = 'inner: loop {
                let op = code[pc];
                pc += 1;
                match op {
                    Op::Step(line) => {
                        self.cur.frames[fi].line = line;
                        if self.steps == 0
                            && let Some(e) = self.out_of_steps()
                        {
                            break 'inner e;
                        }
                        self.steps -= 1;
                    }
                    Op::Tick => {
                        if self.steps == 0
                            && let Some(e) = self.out_of_steps()
                        {
                            break 'inner e;
                        }
                        self.steps -= 1;
                    }
                    Op::Charge(n) => {
                        if let Err(e) = self.charge(n as usize * 16) {
                            break 'inner e;
                        }
                    }
                    Op::Move(a, b) => {
                        let v = self.cur.stack[base + b as usize].clone();
                        self.cur.stack[base + a as usize] = v;
                    }
                    Op::LoadK(a, kk) => self.cur.stack[base + a as usize] = k[kk as usize].clone(),
                    Op::LoadN(a, n) => self.set_num(base + a as usize, f64::from(n)),
                    Op::LoadNil(a, n) => {
                        let a = base + a as usize;
                        for v in &mut self.cur.stack[a..a + n as usize] {
                            *v = Value::Nil;
                        }
                    }
                    Op::LoadBool(a, b) => self.cur.stack[base + a as usize] = Value::Bool(b),
                    Op::GetUpval(a, u) => {
                        let v = body.upvals[u as usize].borrow().clone();
                        self.cur.stack[base + a as usize] = v;
                    }
                    Op::SetUpval(a, u) => {
                        let v = self.cur.stack[base + a as usize].clone();
                        *body.upvals[u as usize].borrow_mut() = v;
                    }
                    Op::GetCell(a, c) => {
                        let v = self.cur.cells[cb + c as usize].borrow().clone();
                        self.cur.stack[base + a as usize] = v;
                    }
                    Op::SetCell(a, c) => {
                        let v = self.cur.stack[base + a as usize].clone();
                        *self.cur.cells[cb + c as usize].borrow_mut() = v;
                    }
                    Op::NewCell(c, a) => {
                        let v = if a == NO_REG {
                            Value::Nil
                        } else {
                            self.cur.stack[base + a as usize].clone()
                        };
                        self.cur.cells[cb + c as usize] = Rc::new(RefCell::new(v));
                    }
                    Op::GetGlobal(a, kk) => {
                        let found = {
                            let env = body.env.borrow();
                            let v = match &p.keys[kk as usize] {
                                Some(key) => env.get_key(key),
                                None => env.get(&k[kk as usize]),
                            };
                            (!v.is_nil() || env.meta.is_none()).then_some(v)
                        };
                        let v = match found {
                            Some(v) => v,
                            None => {
                                self.cur.frames[fi].pc = pc;
                                match self.index(h, &Value::Table(body.env.clone()), &k[kk as usize]) {
                                    Ok(v) => v,
                                    Err(e) => break 'inner e,
                                }
                            }
                        };
                        self.cur.stack[base + a as usize] = v;
                    }
                    Op::SetGlobal(a, kk) => {
                        let v = self.cur.stack[base + a as usize].clone();
                        self.cur.frames[fi].pc = pc;
                        let env = Value::Table(body.env.clone());
                        if let Err(e) = self.store(h, &env, k[kk as usize].clone(), p.keys[kk as usize].as_ref(), v) {
                            break 'inner e;
                        }
                    }
                    Op::GetImport(a, i) => {
                        self.cur.frames[fi].pc = pc;
                        match self.import(h, p, &body.env, i as usize) {
                            Ok(v) => self.cur.stack[base + a as usize] = v,
                            Err(e) => break 'inner e,
                        }
                    }
                    Op::GetTable(a, b, c) => {
                        let found = match &self.cur.stack[base + b as usize] {
                            Value::Table(t) => {
                                let t = t.borrow();
                                let v = t.get(&self.cur.stack[base + c as usize]);
                                (!v.is_nil() || t.meta.is_none()).then_some(v)
                            }
                            _ => None,
                        };
                        let v = match found {
                            Some(v) => v,
                            None => {
                                self.cur.frames[fi].pc = pc;
                                let o = self.cur.stack[base + b as usize].clone();
                                let key = self.cur.stack[base + c as usize].clone();
                                match self.get_slow(h, &o, &key, p, pc - 1) {
                                    Ok(v) => v,
                                    Err(e) => break 'inner e,
                                }
                            }
                        };
                        self.cur.stack[base + a as usize] = v;
                    }
                    Op::GetField(a, b, kk) => {
                        let found = match (&self.cur.stack[base + b as usize], &p.keys[kk as usize]) {
                            (Value::Table(t), Some(key)) => {
                                let t = t.borrow();
                                let v = t.get_key(key);
                                (!v.is_nil() || t.meta.is_none()).then_some(v)
                            }
                            _ => None,
                        };
                        let v = match found {
                            Some(v) => v,
                            None => {
                                self.cur.frames[fi].pc = pc;
                                let o = self.cur.stack[base + b as usize].clone();
                                match self.get_slow(h, &o, &k[kk as usize], p, pc - 1) {
                                    Ok(v) => v,
                                    Err(e) => break 'inner e,
                                }
                            }
                        };
                        self.cur.stack[base + a as usize] = v;
                    }
                    Op::SetTable(a, b, c) => {
                        let done = match &self.cur.stack[base + a as usize] {
                            Value::Table(t) => {
                                let mut t = t.borrow_mut();
                                if t.meta.is_none() {
                                    let key = self.cur.stack[base + b as usize].clone();
                                    let v = self.cur.stack[base + c as usize].clone();
                                    Some(t.set(key, v))
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        };
                        match done {
                            Some(Ok(())) => {}
                            Some(Err(e)) => break 'inner self.set_error(e),
                            None => {
                                self.cur.frames[fi].pc = pc;
                                let o = self.cur.stack[base + a as usize].clone();
                                let key = self.cur.stack[base + b as usize].clone();
                                let v = self.cur.stack[base + c as usize].clone();
                                if let Err(e) = self.set_slow(h, &o, key, None, v, p, pc - 1) {
                                    break 'inner e;
                                }
                            }
                        }
                    }
                    Op::SetField(a, kk, c) => {
                        let done = match (&self.cur.stack[base + a as usize], &p.keys[kk as usize]) {
                            (Value::Table(t), Some(key)) => {
                                let mut t = t.borrow_mut();
                                if t.meta.is_none() {
                                    let v = self.cur.stack[base + c as usize].clone();
                                    Some(t.store(key.clone(), k[kk as usize].clone(), v))
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        };
                        match done {
                            Some(Ok(())) => {}
                            Some(Err(e)) => break 'inner self.set_error(e),
                            None => {
                                self.cur.frames[fi].pc = pc;
                                let o = self.cur.stack[base + a as usize].clone();
                                let v = self.cur.stack[base + c as usize].clone();
                                let key = p.keys[kk as usize].as_ref();
                                if let Err(e) = self.set_slow(h, &o, k[kk as usize].clone(), key, v, p, pc - 1) {
                                    break 'inner e;
                                }
                            }
                        }
                    }
                    Op::NewTable(a, narr, nhash) => {
                        let t = Rc::new(RefCell::new(Table::with_capacity(narr as usize, nhash as usize)));
                        self.track_table(&t);
                        self.cur.stack[base + a as usize] = Value::Table(t);
                    }
                    Op::SetList(a, packed) => {
                        if let Err(e) = self.set_list(base + a as usize, packed as u8, packed >> 8) {
                            break 'inner e;
                        }
                    }
                    Op::Method(a, b, kk) => {
                        self.cur.frames[fi].pc = pc;
                        let made = p.keys[kk as usize].as_ref();
                        if let Err(e) = self.method(h, base, [a as usize, b as usize], &k[kk as usize], made, p, pc - 1)
                        {
                            break 'inner e;
                        }
                    }
                    Op::Namecall(a, kk) => {
                        if let (Value::Userdata(_) | Value::Table(_), Value::Str(name)) =
                            (&self.cur.stack[base + a as usize + 1], &k[kk as usize])
                        {
                            self.namecall = Some(name.clone());
                        }
                    }
                    Op::Call(a, nargs, nres) => {
                        self.cur.frames[fi].pc = pc;
                        let func = base + a as usize;
                        let n = if nargs == MULTI {
                            self.cur.top.saturating_sub(func + 1)
                        } else {
                            nargs as usize
                        };
                        match self.op_call(h, func, n, nres, Some((p, pc - 1)), Ret::Plain) {
                            Ok(true) => continue 'frames,
                            Ok(false) => {}
                            Err(e) => break 'inner e,
                        }
                    }
                    Op::Return(a, n) => {
                        let start = base + a as usize;
                        let n = if n == MULTI {
                            self.cur.top.saturating_sub(start)
                        } else {
                            n as usize
                        };
                        if let Some(vals) = self.op_return(start, n, entry) {
                            return Ok(vals);
                        }
                        continue 'frames;
                    }
                    Op::Vararg(a, n) => self.vararg(fi, base + a as usize, n),
                    Op::Closure(a, i) => {
                        let f = self.closure(&body, cb, i as usize);
                        self.cur.stack[base + a as usize] = f;
                    }
                    Op::Add(a, b, c) => match self.nums(base + b as usize, base + c as usize) {
                        Some((x, y)) => self.set_num(base + a as usize, x + y),
                        None => {
                            if let Err(e) = self.arith_regs(h, BinOp::Add, base, a, b, c, p, pc - 1) {
                                break 'inner e;
                            }
                        }
                    },
                    Op::Sub(a, b, c) => match self.nums(base + b as usize, base + c as usize) {
                        Some((x, y)) => self.set_num(base + a as usize, x - y),
                        None => {
                            if let Err(e) = self.arith_regs(h, BinOp::Sub, base, a, b, c, p, pc - 1) {
                                break 'inner e;
                            }
                        }
                    },
                    Op::Mul(a, b, c) => match self.nums(base + b as usize, base + c as usize) {
                        Some((x, y)) => self.set_num(base + a as usize, x * y),
                        None => {
                            if let Err(e) = self.arith_regs(h, BinOp::Mul, base, a, b, c, p, pc - 1) {
                                break 'inner e;
                            }
                        }
                    },
                    Op::Div(a, b, c) => match self.nums(base + b as usize, base + c as usize) {
                        Some((x, y)) => self.set_num(base + a as usize, x / y),
                        None => {
                            if let Err(e) = self.arith_regs(h, BinOp::Div, base, a, b, c, p, pc - 1) {
                                break 'inner e;
                            }
                        }
                    },
                    Op::Mod(a, b, c) => match self.nums(base + b as usize, base + c as usize) {
                        Some((x, y)) => self.set_num(base + a as usize, arith(BinOp::Mod, x, y)),
                        None => {
                            if let Err(e) = self.arith_regs(h, BinOp::Mod, base, a, b, c, p, pc - 1) {
                                break 'inner e;
                            }
                        }
                    },
                    Op::AddK(a, b, kk) => match (&self.cur.stack[base + b as usize], &k[kk as usize]) {
                        (Value::Num(x), Value::Num(y)) => {
                            let v = x + y;
                            self.set_num(base + a as usize, v)
                        }
                        _ => {
                            if let Err(e) = self.arith_k(h, BinOp::Add, base, a, b, &k[kk as usize], p, pc - 1) {
                                break 'inner e;
                            }
                        }
                    },
                    Op::SubK(a, b, kk) => match (&self.cur.stack[base + b as usize], &k[kk as usize]) {
                        (Value::Num(x), Value::Num(y)) => {
                            let v = x - y;
                            self.set_num(base + a as usize, v)
                        }
                        _ => {
                            if let Err(e) = self.arith_k(h, BinOp::Sub, base, a, b, &k[kk as usize], p, pc - 1) {
                                break 'inner e;
                            }
                        }
                    },
                    Op::MulK(a, b, kk) => match (&self.cur.stack[base + b as usize], &k[kk as usize]) {
                        (Value::Num(x), Value::Num(y)) => {
                            let v = x * y;
                            self.set_num(base + a as usize, v)
                        }
                        _ => {
                            if let Err(e) = self.arith_k(h, BinOp::Mul, base, a, b, &k[kk as usize], p, pc - 1) {
                                break 'inner e;
                            }
                        }
                    },
                    Op::DivK(a, b, kk) => match (&self.cur.stack[base + b as usize], &k[kk as usize]) {
                        (Value::Num(x), Value::Num(y)) => {
                            let v = x / y;
                            self.set_num(base + a as usize, v)
                        }
                        _ => {
                            if let Err(e) = self.arith_k(h, BinOp::Div, base, a, b, &k[kk as usize], p, pc - 1) {
                                break 'inner e;
                            }
                        }
                    },
                    Op::ModK(a, b, kk) => match (&self.cur.stack[base + b as usize], &k[kk as usize]) {
                        (Value::Num(x), Value::Num(y)) => {
                            let v = arith(BinOp::Mod, *x, *y);
                            self.set_num(base + a as usize, v)
                        }
                        _ => {
                            if let Err(e) = self.arith_k(h, BinOp::Mod, base, a, b, &k[kk as usize], p, pc - 1) {
                                break 'inner e;
                            }
                        }
                    },
                    Op::Arith(op, a, b, c) => {
                        if let Err(e) = self.arith_regs(h, op, base, a, b, c, p, pc - 1) {
                            break 'inner e;
                        }
                    }
                    Op::ArithK(op, a, b, kk) => {
                        if let Err(e) = self.arith_k(h, op, base, a, b, &k[kk as usize], p, pc - 1) {
                            break 'inner e;
                        }
                    }
                    Op::Concat(a, b, c) => {
                        let x = self.cur.stack[base + b as usize].clone();
                        let y = self.cur.stack[base + c as usize].clone();
                        match self.concat(h, x, y, p, pc - 1) {
                            Ok(v) => self.cur.stack[base + a as usize] = v,
                            Err(e) => break 'inner e,
                        }
                    }
                    Op::Eq(a, b, c) | Op::Ne(a, b, c) => {
                        let ne = matches!(op, Op::Ne(..));
                        let fast = {
                            let (x, y) = (&self.cur.stack[base + b as usize], &self.cur.stack[base + c as usize]);
                            match (x, y) {
                                (Value::Num(x), Value::Num(y)) => Some(x == y),
                                (Value::Table(_), Value::Table(_)) | (Value::Userdata(_), Value::Userdata(_)) => {
                                    x.raw_eq(y).then_some(true)
                                }
                                _ => Some(x.raw_eq(y)),
                            }
                        };
                        let r = match fast {
                            Some(r) => r,
                            None => {
                                self.cur.frames[fi].pc = pc;
                                let x = self.cur.stack[base + b as usize].clone();
                                let y = self.cur.stack[base + c as usize].clone();
                                match self.equals(h, &x, &y) {
                                    Ok(r) => r,
                                    Err(e) => break 'inner e,
                                }
                            }
                        };
                        self.set_bool(base + a as usize, r != ne);
                    }
                    Op::EqK(a, b, kk) => {
                        let r = self.cur.stack[base + b as usize].raw_eq(&k[kk as usize]);
                        self.set_bool(base + a as usize, r);
                    }
                    Op::NeK(a, b, kk) => {
                        let r = self.cur.stack[base + b as usize].raw_eq(&k[kk as usize]);
                        self.set_bool(base + a as usize, !r);
                    }
                    Op::Lt(a, b, c) | Op::Le(a, b, c) => {
                        let le = matches!(op, Op::Le(..));
                        let r = match (&self.cur.stack[base + b as usize], &self.cur.stack[base + c as usize]) {
                            (Value::Num(x), Value::Num(y)) => {
                                if le {
                                    x <= y
                                } else {
                                    x < y
                                }
                            }
                            _ => {
                                self.cur.frames[fi].pc = pc;
                                let x = self.cur.stack[base + b as usize].clone();
                                let y = self.cur.stack[base + c as usize].clone();
                                match self.less(h, &x, &y, le) {
                                    Ok(r) => r,
                                    Err(e) => break 'inner e,
                                }
                            }
                        };
                        self.set_bool(base + a as usize, r);
                    }
                    Op::Unm(a, b) => match &self.cur.stack[base + b as usize] {
                        Value::Num(n) => {
                            let v = -n;
                            self.set_num(base + a as usize, v)
                        }
                        _ => {
                            if let Err(e) = self.unary(h, UnOp::Neg, base, a, b, p, pc - 1) {
                                break 'inner e;
                            }
                        }
                    },
                    Op::Not(a, b) => {
                        let r = !self.cur.stack[base + b as usize].truthy();
                        self.set_bool(base + a as usize, r);
                    }
                    Op::Len(a, b) => {
                        let fast = match &self.cur.stack[base + b as usize] {
                            Value::Str(s) => Some(s.len()),
                            Value::Table(t) => {
                                let t = t.borrow();
                                t.meta.is_none().then(|| t.len())
                            }
                            _ => None,
                        };
                        match fast {
                            Some(n) => self.set_num(base + a as usize, n as f64),
                            None => {
                                if let Err(e) = self.unary(h, UnOp::Len, base, a, b, p, pc - 1) {
                                    break 'inner e;
                                }
                            }
                        }
                    }
                    Op::BNot(a, b) => {
                        if let Err(e) = self.unary(h, UnOp::BNot, base, a, b, p, pc - 1) {
                            break 'inner e;
                        }
                    }
                    Op::Jmp(off) => pc = pc.wrapping_add_signed(off as isize),
                    Op::JmpIf(a, off) => {
                        if self.cur.stack[base + a as usize].truthy() {
                            pc = pc.wrapping_add_signed(off as isize);
                        }
                    }
                    Op::JmpIfNot(a, off) => {
                        if !self.cur.stack[base + a as usize].truthy() {
                            pc = pc.wrapping_add_signed(off as isize);
                        }
                    }
                    Op::ForCheck(a, what) => {
                        let i = base + a as usize;
                        match self.cur.stack[i].to_num() {
                            Some(n) => self.cur.stack[i] = Value::Num(n),
                            None => {
                                let what = ["initial", "limit", "step"][what as usize % 3];
                                break 'inner self.rt_error(format!("'for' {what} value must be a number"));
                            }
                        }
                    }
                    Op::ForPrep(a, off) => {
                        let i = base + a as usize;
                        let r = &mut self.cur.stack[i..i + 5];
                        let (start, limit, step) = (num(&r[0]), num(&r[1]), num(&r[2]));
                        if step == 0.0 {
                            break 'inner self.rt_error("'for' step is zero");
                        }
                        let x = start + 0.0 * step;
                        if (step > 0.0 && x > limit) || (step < 0.0 && x < limit) {
                            pc = pc.wrapping_add_signed(off as isize);
                        } else {
                            put_num(&mut r[3], 0.0);
                            put_num(&mut r[4], x);
                            if self.steps == 0
                                && let Some(e) = self.out_of_steps()
                            {
                                break 'inner e;
                            }
                            self.steps -= 1;
                        }
                    }
                    Op::ForLoop(a, off) => {
                        let i = base + a as usize;
                        let r = &mut self.cur.stack[i..i + 5];
                        let (start, limit, step) = (num(&r[0]), num(&r[1]), num(&r[2]));
                        let count = num(&r[3]) + 1.0;
                        let x = start + count * step;
                        if !((step > 0.0 && x > limit) || (step < 0.0 && x < limit)) {
                            put_num(&mut r[3], count);
                            put_num(&mut r[4], x);
                            if self.steps == 0
                                && let Some(e) = self.out_of_steps()
                            {
                                break 'inner e;
                            }
                            self.steps -= 1;
                            pc = pc.wrapping_add_signed(off as isize);
                        }
                    }
                    Op::TForPrep(a) => {
                        let i = base + a as usize;
                        let it = self.cur.stack[i].clone();
                        if !it.is_callable() && !matches!(it, Value::Table(_) | Value::Userdata(_)) {
                            let t = it.type_name();
                            break 'inner self.rt_error(format!("attempt to call a {t} value (for iterator)"));
                        }
                        // Luau's generalized iteration: `for k, v in t` runs
                        // the table's __iter, or goes through it with next.
                        if matches!(it, Value::Table(_) | Value::Userdata(_)) && self.meta(&it, Ev::Call).is_none() {
                            match self.metamethod(&it, "__iter") {
                                Some(f) => {
                                    self.cur.frames[fi].pc = pc;
                                    match self.call(h, &f, vec![it]) {
                                        Ok(r) => {
                                            for j in 0..3 {
                                                self.cur.stack[i + j] = r.get(j).cloned().unwrap_or_default();
                                            }
                                        }
                                        Err(e) => break 'inner e,
                                    }
                                }
                                None if matches!(it, Value::Table(_)) => {
                                    self.cur.stack[i] = builtin_value("next");
                                    self.cur.stack[i + 1] = it;
                                    self.cur.stack[i + 2] = Value::Nil;
                                }
                                None => {}
                            }
                        }
                    }
                    Op::TForCall(a, n) => {
                        if self.steps == 0
                            && let Some(e) = self.out_of_steps()
                        {
                            break 'inner e;
                        }
                        self.steps -= 1;
                        let i = base + a as usize;
                        for j in 0..3 {
                            let v = self.cur.stack[i + j].clone();
                            self.cur.stack[i + 3 + j] = v;
                        }
                        self.cur.frames[fi].pc = pc;
                        match self.op_call(h, i + 3, 2, n, None, Ret::Plain) {
                            Ok(true) => continue 'frames,
                            Ok(false) => {}
                            Err(e) => break 'inner e,
                        }
                    }
                    Op::TForLoop(a, off) => {
                        let i = base + a as usize;
                        if self.cur.stack[i + 3].is_nil() {
                            pc = pc.wrapping_add_signed(off as isize);
                        } else {
                            let v = self.cur.stack[i + 3].clone();
                            self.cur.stack[i + 2] = v;
                        }
                    }
                }
            };
            if let Some(f) = self.cur.frames.get_mut(fi) {
                f.pc = pc;
            }
            match self.catch(h, err, entry) {
                Ok(()) => continue 'frames,
                Err(e) => {
                    if !matches!(e, Throw::Yield(_)) {
                        self.unwind_run(entry, entry_func, entry_cells);
                    }
                    return Err(e);
                }
            }
        }
    }

    /// A number into a register, without dropping the number already there.
    #[inline(always)]
    fn set_num(&mut self, i: usize, x: f64) {
        put_num(&mut self.cur.stack[i], x);
    }
    #[inline(always)]
    fn set_bool(&mut self, i: usize, b: bool) {
        match &mut self.cur.stack[i] {
            Value::Bool(v) => *v = b,
            slot => *slot = Value::Bool(b),
        }
    }
    #[inline(always)]
    fn nums(&self, b: usize, c: usize) -> Option<(f64, f64)> {
        match (&self.cur.stack[b], &self.cur.stack[c]) {
            (Value::Num(x), Value::Num(y)) => Some((*x, *y)),
            _ => None,
        }
    }

    /// Calls the value at `func` with the `n` after it. A Lua function gets
    /// a frame (true: the loop goes to it); anything else runs now, its
    /// results in place.
    fn op_call(
        &mut self,
        h: &mut dyn Host,
        func: usize,
        mut n: usize,
        nres: u16,
        site: Option<(&Proto, usize)>,
        ret: Ret,
    ) -> Result<bool, Throw> {
        let mut first = true;
        for _ in 0..64 {
            let f = self.cur.stack[func].clone();
            match f {
                Value::Func(c) => {
                    if let Some(to) = c.redirect() {
                        self.cur.stack[func] = to;
                        continue;
                    }
                    if self.cur.frames.len() >= MAX_DEPTH {
                        return Err(self.rt_error("stack overflow"));
                    }
                    self.step()?;
                    self.enter(c, func, n, nres, ret);
                    return Ok(true);
                }
                Value::Native(nat) => {
                    if let Some(to) = nat.redirect() {
                        self.cur.stack[func] = to;
                        continue;
                    }
                    match &nat.kind {
                        NativeKind::Wrap(inner) => {
                            self.cur.stack[func] = inner.clone();
                            continue;
                        }
                        NativeKind::Namecall => {
                            let target =
                                self.namecall_target(h, self.cur.stack.get(func + 1).cloned().unwrap_or_default())?;
                            self.cur.stack[func] = target;
                            continue;
                        }
                        _ => {}
                    }
                    self.step()?;
                    if let NativeKind::Pcall(handler) = nat.kind {
                        return self.op_pcall(h, func, n, nres, handler, ret);
                    }
                    // The arguments stay in their registers while it runs,
                    // as Luau has them (debug.getstack sees them).
                    let args = self.cur.stack[func + 1..func + 1 + n].to_vec();
                    return match self.call_native(h, &nat, args) {
                        Ok(vals) => {
                            self.deliver(func, vals, nres, ret);
                            Ok(false)
                        }
                        Err(Throw::Yield(vals)) => self.suspend(func, nres, ret, vals),
                        Err(e) => Err(e),
                    };
                }
                other => {
                    let mm = self.meta(&other, Ev::Call);
                    if first
                        && mm.is_none()
                        && let Some((p, pc)) = site
                    {
                        return Err(self.rt_error(format!(
                            "attempt to call a {} value{}",
                            other.type_name(),
                            p.desc(pc, 0)
                        )));
                    }
                    first = false;
                    match mm {
                        Some(mm) if !matches!(mm, Value::Table(_)) => {
                            // f(...) is __call(f, ...).
                            let need = func + n + 2;
                            if self.cur.stack.len() < need {
                                self.cur.stack.resize(need, Value::Nil);
                            }
                            for i in (0..n).rev() {
                                let v = std::mem::take(&mut self.cur.stack[func + 1 + i]);
                                self.cur.stack[func + 2 + i] = v;
                            }
                            self.cur.stack[func + 1] = other;
                            self.cur.stack[func] = mm;
                            n += 1;
                        }
                        _ => return Err(self.rt_error(format!("attempt to call a {} value", other.type_name()))),
                    }
                }
            }
        }
        Err(self.rt_error("'__call' chain too long; possible loop"))
    }
    /// pcall(f, ...) or xpcall(f, handler, ...) at `func`: a record to unwind
    /// to, then the call.
    fn op_pcall(
        &mut self,
        h: &mut dyn Host,
        func: usize,
        n: usize,
        nres: u16,
        handler: bool,
        ret: Ret,
    ) -> Result<bool, Throw> {
        if !handler && n == 0 {
            return Err(bad_arg(self, 0, "pcall", "value expected"));
        }
        if handler && n < 2 {
            return Err(bad_arg(self, 1, "xpcall", "value expected"));
        }
        let skip = if handler { 2 } else { 1 };
        let st = &mut self.cur.stack;
        let hf = handler.then(|| std::mem::take(&mut st[func + 2]));
        st[func] = std::mem::take(&mut st[func + 1]);
        for i in 0..n - skip {
            st[func + 1 + i] = std::mem::take(&mut st[func + 1 + skip + i]);
        }
        self.cur.pcalls.push(Protect {
            frames: self.cur.frames.len(),
            func,
            nres,
            cells: self.cur.cells.len(),
            handler: hf,
            catches: true,
            ret,
        });
        self.op_call(h, func, n - skip, nres, None, Ret::Wrapped)
    }
    /// A native yielded: this coroutine stops here, if it can.
    fn suspend(&mut self, func: usize, nres: u16, ret: Ret, vals: Vec<Value>) -> Result<bool, Throw> {
        if self.thread.main || self.cur.runs > 1 {
            return Err(self.rt_error(self.yield_error()));
        }
        self.cur.pending = Some(Pending { func, nres, ret });
        Err(Throw::Yield(vals))
    }
    /// An error in the frames above `entry`: unwound to the innermost
    /// pcall there, which then carries on; else it goes on out.
    fn catch(&mut self, h: &mut dyn Host, mut err: Throw, entry: usize) -> Result<(), Throw> {
        loop {
            match err {
                Throw::Yield(_) => return Err(err),
                Throw::Fatal(_) => {
                    while self.cur.pcalls.last().is_some_and(|r| r.frames > entry) {
                        self.cur.pcalls.pop();
                    }
                    return Err(err);
                }
                Throw::Error(..) => {}
            }
            if !self.cur.pcalls.last().is_some_and(|r| r.frames > entry) {
                return Err(err);
            }
            let Some(r) = self.cur.pcalls.pop() else {
                return Err(err);
            };
            if !r.catches {
                continue;
            }
            let Throw::Error(v, _) = err else {
                return Err(err);
            };
            self.cur.frames.truncate(r.frames);
            self.cur.cells.truncate(r.cells);
            match r.handler {
                None => {
                    self.deliver(r.func, vec![Value::Bool(false), v], r.nres, r.ret);
                    return Ok(());
                }
                Some(hf) => {
                    // The handler's results get `false` in front.
                    self.cur.pcalls.push(Protect {
                        frames: self.cur.frames.len(),
                        func: r.func,
                        nres: r.nres,
                        cells: self.cur.cells.len(),
                        handler: None,
                        catches: false,
                        ret: r.ret,
                    });
                    if self.cur.stack.len() < r.func + 2 {
                        self.cur.stack.resize(r.func + 2, Value::Nil);
                    }
                    self.cur.stack[r.func] = hf;
                    self.cur.stack[r.func + 1] = v;
                    match self.op_call(h, r.func, 1, r.nres, None, Ret::Wrapped) {
                        Ok(_) => return Ok(()),
                        Err(e) => err = e,
                    }
                }
            }
        }
    }
    /// A VM loop failed: its frames go.
    fn unwind_run(&mut self, entry: usize, func: usize, cells: usize) {
        self.cur.frames.truncate(entry);
        while self.cur.pcalls.last().is_some_and(|r| r.frames > entry) {
            self.cur.pcalls.pop();
        }
        self.cur.cells.truncate(cells);
        self.cur.stack.truncate(func);
    }
    /// The running frame returns registers `start..start + n`.
    fn op_return(&mut self, start: usize, n: usize, entry: usize) -> Option<Vec<Value>> {
        let fr = self.cur.frames.pop()?;
        self.cur.cells.truncate(fr.cells);
        if self.cur.frames.len() <= entry {
            let end = (start + n).min(self.cur.stack.len());
            let vals: Vec<Value> = self.cur.stack.drain(start.min(end)..end).collect();
            self.cur.stack.truncate(fr.func);
            return Some(vals);
        }
        self.place(fr.func, start, n, fr.nres, fr.ret);
        None
    }
    /// What goes before results delivered with `ret`, outermost first.
    fn prefix(&mut self, ret: Ret, out: &mut Vec<Value>) {
        if let Ret::Wrapped = ret
            && let Some(r) = self.cur.pcalls.pop()
        {
            self.prefix(r.ret, out);
            out.push(Value::Bool(r.catches));
        }
    }
    /// Moves results `src..src + n` to `dst`, as the caller wants them.
    fn place(&mut self, dst: usize, src: usize, n: usize, nres: u16, ret: Ret) {
        let mut pre = Vec::new();
        if let Ret::Wrapped = ret {
            self.prefix(ret, &mut pre);
        }
        let to = dst + pre.len();
        let st = &mut self.cur.stack;
        if st.len() < to + n {
            st.resize(to + n, Value::Nil);
        }
        if to <= src {
            for i in 0..n {
                st[to + i] = std::mem::take(&mut st[src + i]);
            }
        } else {
            for i in (0..n).rev() {
                st[to + i] = std::mem::take(&mut st[src + i]);
            }
        }
        let count = pre.len() + n;
        for (i, v) in pre.into_iter().enumerate() {
            st[dst + i] = v;
        }
        self.finish(dst, count, nres);
    }
    /// A native's results to `dst`, as the caller wants them.
    fn deliver(&mut self, dst: usize, vals: Vec<Value>, nres: u16, ret: Ret) {
        let mut pre = Vec::new();
        if let Ret::Wrapped = ret {
            self.prefix(ret, &mut pre);
        }
        let count = pre.len() + vals.len();
        let st = &mut self.cur.stack;
        if st.len() < dst + count {
            st.resize(dst + count, Value::Nil);
        }
        for (i, v) in pre.into_iter().chain(vals).enumerate() {
            st[dst + i] = v;
        }
        self.finish(dst, count, nres);
    }
    /// After `count` results went to `dst`: pads them to `nres` and makes
    /// sure the frame that goes on has its registers. The stack only grows
    /// here (registers above the frame are free); it shrinks when a call
    /// from Rust returns.
    fn finish(&mut self, dst: usize, count: usize, nres: u16) {
        let end = self.cur.frames.last().map_or(0, |f| f.base + f.body.proto.maxstack);
        let st = &mut self.cur.stack;
        if nres == MULTI {
            let top = dst + count;
            self.cur.top = top;
            if st.len() < end.max(top) {
                st.resize(end.max(top), Value::Nil);
            }
        } else {
            let want = nres as usize;
            if st.len() < end.max(dst + want) {
                st.resize(end.max(dst + want), Value::Nil);
            }
            for v in st.iter_mut().skip(dst + count).take(want.saturating_sub(count)) {
                *v = Value::Nil;
            }
        }
    }
    fn vararg(&mut self, fi: usize, dst: usize, n: u16) {
        let (from, have) = {
            let f = &self.cur.frames[fi];
            (f.varargs, f.nvarargs)
        };
        let st = &mut self.cur.stack;
        let n = if n == MULTI {
            self.cur.top = dst + have;
            have
        } else {
            n as usize
        };
        if st.len() < dst + n {
            st.resize(dst + n, Value::Nil);
        }
        for i in 0..n {
            st[dst + i] = if i < have { st[from + i].clone() } else { Value::Nil };
        }
    }
    fn closure(&mut self, body: &Body, cells: usize, i: usize) -> Value {
        let Some(p) = body.proto.protos.get(i).cloned() else {
            return Value::Nil;
        };
        let upvals = p
            .upvals
            .iter()
            .map(|u| {
                let c = if u.from_local {
                    self.cur.cells.get(cells + u.index)
                } else {
                    body.upvals.get(u.index)
                };
                c.cloned().unwrap_or_else(|| self.nil_cell.clone())
            })
            .collect();
        let c = Rc::new(Closure::new(p, upvals, body.env.clone()));
        self.track_closure(&c);
        Value::Func(c)
    }
    fn set_list(&mut self, t: usize, n: u8, start: u32) -> Result<(), Throw> {
        let count = if n == MULTI8 {
            self.cur.top.saturating_sub(t + 1)
        } else {
            n as usize
        };
        let Value::Table(tb) = self.cur.stack[t].clone() else {
            return Ok(());
        };
        let mut tb = tb.borrow_mut();
        for i in 0..count {
            let v = std::mem::take(&mut self.cur.stack[t + 1 + i]);
            let r = tb.set(Value::Num(f64::from(start) + i as f64), v);
            if let Err(e) = r
                && n != MULTI8
            {
                return Err(self.rt_error(e));
            }
        }
        Ok(())
    }
    #[cold]
    fn set_error(&self, e: &'static str) -> Throw {
        match e {
            "table is too large" => self.fatal_here(e),
            e => self.rt_error(e),
        }
    }
    /// `obj:name` for a method call: the method and `obj` in place.
    #[allow(clippy::too_many_arguments)]
    fn method(
        &mut self,
        h: &mut dyn Host,
        base: usize,
        [a, b]: [usize; 2],
        name: &Value,
        made: Option<&Key>,
        p: &Proto,
        pc: usize,
    ) -> Result<(), Throw> {
        let obj = self.cur.stack[base + b].clone();
        if !matches!(obj, Value::Table(_) | Value::Str(_) | Value::Userdata(_)) {
            return Err(self.rt_error(format!(
                "attempt to index a {} value{}",
                obj.type_name(),
                index_desc(p, pc, name)
            )));
        }
        // A userdata's `__namecall` takes every method call on it (and a
        // table's, as executors' hookmetamethod needs).
        if matches!(obj, Value::Userdata(_) | Value::Table(_))
            && obj.metatable().is_some()
            && let Some(nc) = self.meta(&obj, Ev::Namecall)
            && nc.is_callable()
        {
            self.cur.stack[base + a + 1] = obj;
            self.cur.stack[base + a] = nc;
            return Ok(());
        }
        let f = self.index_key(h, &obj, name, made)?;
        if !f.is_callable() && self.meta(&f, Ev::Call).is_none() {
            return Err(self.rt_error(format!(
                "attempt to call a {} value (method '{}')",
                f.type_name(),
                name.plain_text()
            )));
        }
        self.cur.stack[base + a + 1] = obj;
        self.cur.stack[base + a] = f;
        Ok(())
    }
    /// `o[key]` when the quick way would not do: strings, userdata,
    /// `__index`, errors that name `o`.
    fn get_slow(&mut self, h: &mut dyn Host, o: &Value, key: &Value, p: &Proto, pc: usize) -> Result<Value, Throw> {
        match o {
            Value::Table(_) | Value::Str(_) => self.index(h, o, key),
            Value::Userdata(_) if self.meta(o, Ev::Index).is_some() => self.index(h, o, key),
            _ => Err(self.rt_error(format!(
                "attempt to index a {} value{}",
                o.type_name(),
                index_desc(p, pc, key)
            ))),
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn set_slow(
        &mut self,
        h: &mut dyn Host,
        o: &Value,
        key: Value,
        made: Option<&Key>,
        v: Value,
        p: &Proto,
        pc: usize,
    ) -> Result<(), Throw> {
        match o {
            Value::Table(_) => self.store(h, o, key, made, v),
            Value::Userdata(_) if self.meta(o, Ev::NewIndex).is_some() => self.set_index(h, o, key, v),
            _ => Err(self.rt_error(format!(
                "attempt to index a {} value{}",
                o.type_name(),
                index_desc(p, pc, &key)
            ))),
        }
    }
    /// `t[key] = v` on a table, its key made already when it is a constant.
    fn store(&mut self, h: &mut dyn Host, t: &Value, key: Value, made: Option<&Key>, v: Value) -> Result<(), Throw> {
        if let (Value::Table(tb), Some(made)) = (t, made) {
            let quick = {
                let mut tb = tb.borrow_mut();
                if tb.meta.is_none() {
                    Some(tb.store(made.clone(), key.clone(), v.clone()))
                } else {
                    None
                }
            };
            match quick {
                Some(Ok(())) => return Ok(()),
                Some(Err(e)) => return Err(self.set_error(e)),
                None => {}
            }
        }
        self.set_index(h, t, key, v)
    }
    /// GetImport: a global and the fields after it.
    fn import(&mut self, h: &mut dyn Host, p: &Proto, env: &TableRef, i: usize) -> Result<Value, Throw> {
        let Some(names) = p.imports.get(i) else {
            return Ok(Value::Nil);
        };
        let first = names[0] as usize;
        let found = {
            let e = env.borrow();
            let v = match &p.keys[first] {
                Some(key) => e.get_key(key),
                None => e.get(&p.constants[first]),
            };
            (!v.is_nil() || e.meta.is_none()).then_some(v)
        };
        let mut v = match found {
            Some(v) => v,
            None => self.index(h, &Value::Table(env.clone()), &p.constants[first])?,
        };
        for (j, &name) in names.iter().enumerate().skip(1) {
            let name = name as usize;
            v = match &v {
                Value::Table(t) => {
                    let quick = {
                        let t = t.borrow();
                        let x = match &p.keys[name] {
                            Some(key) => t.get_key(key),
                            None => t.get(&p.constants[name]),
                        };
                        (!x.is_nil() || t.meta.is_none()).then_some(x)
                    };
                    match quick {
                        Some(x) => x,
                        None => self.index(h, &v, &p.constants[name])?,
                    }
                }
                Value::Str(_) => self.index(h, &v, &p.constants[name])?,
                Value::Userdata(_) if self.meta(&v, Ev::Index).is_some() => self.index(h, &v, &p.constants[name])?,
                other => {
                    let prev = &p.constants[names[j - 1] as usize];
                    let desc = if j == 1 {
                        Desc::Global(Rc::from(prev.plain_text().as_bytes()))
                    } else {
                        Desc::Field(Rc::from(prev.plain_text().as_bytes()))
                    };
                    return Err(self.rt_error(format!(
                        "attempt to index a {} value{}",
                        other.type_name(),
                        desc.text()
                    )));
                }
            };
        }
        Ok(v)
    }
    #[allow(clippy::too_many_arguments)]
    fn arith_regs(
        &mut self,
        h: &mut dyn Host,
        op: BinOp,
        base: usize,
        a: u16,
        b: u16,
        c: u16,
        p: &Proto,
        pc: usize,
    ) -> Result<(), Throw> {
        let x = self.cur.stack[base + b as usize].clone();
        let y = self.cur.stack[base + c as usize].clone();
        let v = self.arith_slow(h, op, x, y, p, pc)?;
        self.cur.stack[base + a as usize] = v;
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn arith_k(
        &mut self,
        h: &mut dyn Host,
        op: BinOp,
        base: usize,
        a: u16,
        b: u16,
        k: &Value,
        p: &Proto,
        pc: usize,
    ) -> Result<(), Throw> {
        let x = self.cur.stack[base + b as usize].clone();
        let v = self.arith_slow(h, op, x, k.clone(), p, pc)?;
        self.cur.stack[base + a as usize] = v;
        Ok(())
    }
    /// Arithmetic and bitwise operators past the quick cases: coercion,
    /// metamethods, errors.
    fn arith_slow(
        &mut self,
        h: &mut dyn Host,
        op: BinOp,
        x: Value,
        y: Value,
        p: &Proto,
        pc: usize,
    ) -> Result<Value, Throw> {
        match op {
            BinOp::BAnd | BinOp::BOr | BinOp::BXor | BinOp::Shl | BinOp::Shr => {
                let (i, j) = (integer(&x), integer(&y));
                if let (Ok(i), Ok(j)) = (&i, &j) {
                    return Ok(Value::Num(bitwise(op, *i, *j) as f64));
                }
                let ev = arith_event(op);
                if let Some(mm) = self.meta(&x, ev).or_else(|| self.meta(&y, ev)) {
                    return self.call1(h, &mm, vec![x, y]);
                }
                Err(self.rt_error(match i {
                    Err(why) => bitwise_error(why, &x, &p.desc(pc, 0)),
                    Ok(_) => bitwise_error(j.err().unwrap_or(false), &y, &p.desc(pc, 1)),
                }))
            }
            _ => {
                if let (Some(i), Some(j)) = (x.to_num(), y.to_num()) {
                    return Ok(Value::Num(arith(op, i, j)));
                }
                let ev = arith_event(op);
                if let Some(mm) = self.meta(&x, ev).or_else(|| self.meta(&y, ev)) {
                    return self.call1(h, &mm, vec![x, y]);
                }
                let (bad, i) = if x.to_num().is_none() { (&x, 0) } else { (&y, 1) };
                let desc = p.desc(pc, i);
                Err(self.rt_error(if matches!(bad, Value::Str(_)) {
                    format!("attempt to perform arithmetic on a string value{desc} (not a number)")
                } else {
                    format!("attempt to perform arithmetic on a {} value{desc}", bad.type_name())
                }))
            }
        }
    }
    fn concat(&mut self, h: &mut dyn Host, x: Value, y: Value, p: &Proto, pc: usize) -> Result<Value, Throw> {
        fn piece(v: &Value) -> Option<std::borrow::Cow<'_, [u8]>> {
            match v {
                Value::Str(s) => Some(std::borrow::Cow::Borrowed(&s[..])),
                Value::Num(n) => Some(std::borrow::Cow::Owned(fmt_num(*n).into_bytes())),
                _ => None,
            }
        }
        if let (Some(a), Some(b)) = (piece(&x), piece(&y)) {
            let len = a.len() + b.len();
            if len > MAX_STRING {
                return Err(self.fatal_here("string is too long"));
            }
            self.charge(len)?;
            let mut out = Vec::with_capacity(len);
            out.extend_from_slice(&a);
            out.extend_from_slice(&b);
            return Ok(Value::Str(out.into()));
        }
        if let Some(mm) = self.meta(&x, Ev::Concat).or_else(|| self.meta(&y, Ev::Concat)) {
            return self.call1(h, &mm, vec![x, y]);
        }
        let (bad, i) = if piece(&x).is_none() { (&x, 0) } else { (&y, 1) };
        Err(self.rt_error(format!(
            "attempt to concatenate a {} value{}",
            bad.type_name(),
            p.desc(pc, i)
        )))
    }
    #[allow(clippy::too_many_arguments)]
    fn unary(
        &mut self,
        h: &mut dyn Host,
        op: UnOp,
        base: usize,
        a: u16,
        b: u16,
        p: &Proto,
        pc: usize,
    ) -> Result<(), Throw> {
        let x = self.cur.stack[base + b as usize].clone();
        let v = match op {
            UnOp::Neg => match x.to_num() {
                Some(n) => Value::Num(-n),
                None => match self.meta(&x, Ev::Unm) {
                    Some(mm) => self.call1(h, &mm, vec![x.clone(), x])?,
                    None => {
                        return Err(self.rt_error(format!(
                            "attempt to perform arithmetic on a {} value{}",
                            x.type_name(),
                            p.desc(pc, 0)
                        )));
                    }
                },
            },
            UnOp::BNot => match integer(&x) {
                Ok(i) => Value::Num(!i as f64),
                Err(why) => match self.meta(&x, Ev::BNot) {
                    Some(mm) => self.call1(h, &mm, vec![x.clone(), x])?,
                    None => return Err(self.rt_error(bitwise_error(why, &x, &p.desc(pc, 0)))),
                },
            },
            UnOp::Len => match &x {
                Value::Str(s) => Value::Num(s.len() as f64),
                Value::Table(_) | Value::Userdata(_) => match self.meta(&x, Ev::Len) {
                    Some(mm) => self.call1(h, &mm, vec![x.clone()])?,
                    None => match &x {
                        Value::Table(t) => Value::Num(t.borrow().len() as f64),
                        _ => {
                            return Err(
                                self.rt_error(format!("attempt to get length of a userdata value{}", p.desc(pc, 0)))
                            );
                        }
                    },
                },
                _ => {
                    return Err(self.rt_error(format!(
                        "attempt to get length of a {} value{}",
                        x.type_name(),
                        p.desc(pc, 0)
                    )));
                }
            },
            UnOp::Not => Value::Bool(!x.truthy()),
        };
        self.cur.stack[base + a as usize] = v;
        Ok(())
    }

    // ------------------------------------------------------------- metamethods

    #[inline]
    fn meta(&self, v: &Value, ev: Ev) -> Option<Value> {
        let mt = match v {
            Value::Table(t) => t.borrow().meta.clone()?,
            Value::Userdata(u) => u.meta.borrow().clone()?,
            _ => return None,
        };
        let m = mt.borrow().get_key(&self.events[ev as usize]);
        (!m.is_nil()).then_some(m)
    }
    pub fn metamethod(&self, v: &Value, event: &str) -> Option<Value> {
        let mt = v.metatable()?;
        let m = mt.borrow().get_str(event);
        (!m.is_nil()).then_some(m)
    }

    /// `obj[key]`, with `__index`.
    pub fn index(&mut self, h: &mut dyn Host, obj: &Value, key: &Value) -> Result<Value, Throw> {
        self.index_key(h, obj, key, None)
    }
    /// `index`, with the key as tables hold it when it is a constant.
    fn index_key(&mut self, h: &mut dyn Host, obj: &Value, key: &Value, made: Option<&Key>) -> Result<Value, Throw> {
        let mut cur = obj.clone();
        for _ in 0..64 {
            let handler = match &cur {
                Value::Table(t) => {
                    let t = t.borrow();
                    let v = match made {
                        Some(k) => t.get_key(k),
                        None => t.get(key),
                    };
                    if !v.is_nil() {
                        return Ok(v);
                    }
                    let Some(mt) = &t.meta else {
                        return Ok(Value::Nil);
                    };
                    let handler = mt.borrow().get_key(&self.events[Ev::Index as usize]);
                    if handler.is_nil() {
                        return Ok(Value::Nil);
                    }
                    handler
                }
                Value::Str(_) => {
                    return Ok(self.string_lib.as_ref().map_or(Value::Nil, |s| s.borrow().get(key)));
                }
                Value::Userdata(_) => match self.meta(&cur, Ev::Index) {
                    Some(handler) => handler,
                    None => return Err(self.rt_error("attempt to index a userdata value")),
                },
                other => {
                    return Err(self.rt_error(format!("attempt to index a {} value", other.type_name())));
                }
            };
            if handler.is_callable() {
                return self.call1(h, &handler, vec![cur, key.clone()]);
            }
            cur = handler;
        }
        Err(self.rt_error("'__index' chain too long; possible loop"))
    }

    /// `obj[key] = v`, with `__newindex`.
    pub fn set_index(&mut self, h: &mut dyn Host, obj: &Value, key: Value, v: Value) -> Result<(), Throw> {
        let mut cur = obj.clone();
        for _ in 0..64 {
            let handler = match &cur {
                Value::Table(t) => {
                    let handler = {
                        let tb = t.borrow();
                        match &tb.meta {
                            Some(mt) if tb.get(&key).is_nil() => {
                                mt.borrow().get_key(&self.events[Ev::NewIndex as usize])
                            }
                            _ => Value::Nil,
                        }
                    };
                    if handler.is_nil() {
                        let r = t.borrow_mut().set(key, v);
                        return r.map_err(|e| self.set_error(e));
                    }
                    handler
                }
                Value::Userdata(_) => match self.meta(&cur, Ev::NewIndex) {
                    Some(handler) => handler,
                    None => return Err(self.rt_error("attempt to index a userdata value")),
                },
                other => {
                    return Err(self.rt_error(format!("attempt to index a {} value", other.type_name())));
                }
            };
            if handler.is_callable() {
                self.call(h, &handler, vec![cur, key, v])?;
                return Ok(());
            }
            cur = handler;
        }
        Err(self.rt_error("'__newindex' chain too long; possible loop"))
    }

    pub fn equals(&mut self, h: &mut dyn Host, x: &Value, y: &Value) -> Result<bool, Throw> {
        if x.raw_eq(y) {
            return Ok(true);
        }
        if matches!(
            (x, y),
            (Value::Table(_), Value::Table(_)) | (Value::Userdata(_), Value::Userdata(_))
        ) && let Some(mm) = self.meta(x, Ev::Eq).or_else(|| self.meta(y, Ev::Eq))
        {
            return Ok(self.call1(h, &mm, vec![x.clone(), y.clone()])?.truthy());
        }
        Ok(false)
    }

    /// `x < y`, or `x <= y` when `or_equal`.
    pub fn less(&mut self, h: &mut dyn Host, x: &Value, y: &Value, or_equal: bool) -> Result<bool, Throw> {
        match (x, y) {
            (Value::Num(a), Value::Num(b)) => Ok(if or_equal { a <= b } else { a < b }),
            (Value::Str(a), Value::Str(b)) => Ok(if or_equal { a <= b } else { a < b }),
            _ => {
                let ev = if or_equal { Ev::Le } else { Ev::Lt };
                if let Some(mm) = self.meta(x, ev).or_else(|| self.meta(y, ev)) {
                    return Ok(self.call1(h, &mm, vec![x.clone(), y.clone()])?.truthy());
                }
                let (tx, ty) = (x.type_name(), y.type_name());
                Err(self.rt_error(if tx == ty {
                    format!("attempt to compare two {tx} values")
                } else {
                    format!("attempt to compare {tx} with {ty}")
                }))
            }
        }
    }

    /// `tostring`, with `__tostring`.
    pub fn tostring(&mut self, h: &mut dyn Host, v: &Value) -> Result<String, Throw> {
        if let Some(mm) = self.meta(v, Ev::ToString) {
            let r = self.call1(h, &mm, vec![v.clone()])?;
            return match r {
                Value::Str(s) => Ok(String::from_utf8_lossy(&s).into_owned()),
                Value::Num(n) => Ok(fmt_num(n)),
                _ => Err(self.rt_error("'__tostring' must return a string")),
            };
        }
        Ok(v.plain_text())
    }
    /// `typeof`: a userdata's `__type`, else `type`.
    #[allow(dead_code)]
    pub fn type_of(&self, v: &Value) -> String {
        match v {
            Value::Userdata(u) => u.type_name().unwrap_or_else(|| v.type_name().into()),
            other => other.type_name().into(),
        }
    }

    // ------------------------------------------------------------- coroutines

    /// A coroutine that will run `f`.
    pub fn new_thread(&mut self, f: Value) -> Rc<Thread> {
        let t = Rc::new(Thread {
            state: RefCell::new(Some(ThreadState {
                start: Some(f),
                ..ThreadState::default()
            })),
            status: std::cell::Cell::new(Status::Suspended),
            main: false,
            error: RefCell::new(None),
        });
        if let Some(r) = &mut self.registry {
            r.threads.push(Rc::downgrade(&t));
            r.prune();
        }
        t
    }
    /// The running thread (the main one outside coroutines).
    pub fn current_thread(&self) -> Rc<Thread> {
        self.thread.clone()
    }
    /// Whether the running code may yield: in a coroutine, and not inside
    /// a native's call into Lua.
    pub fn is_yieldable(&self) -> bool {
        !self.thread.main && self.cur.runs <= 1
    }
    /// Runs `co` with `args` until it yields, returns or fails.
    pub fn resume(&mut self, h: &mut dyn Host, co: &Rc<Thread>, args: Vec<Value>) -> Resume {
        if self.ccalls == 0 {
            self.stack_base = stack_addr();
        }
        match co.status.get() {
            Status::Suspended => {}
            Status::Dead => {
                return Resume::Error(Throw::Error(Value::str("cannot resume dead coroutine"), String::new()));
            }
            _ => {
                return Resume::Error(Throw::Error(
                    Value::str("cannot resume non-suspended coroutine"),
                    String::new(),
                ));
            }
        }
        if self.ccalls >= MAX_CCALLS || self.stack_base.saturating_sub(stack_addr()) > STACK_LIMIT {
            return Resume::Error(self.rt_error("stack overflow"));
        }
        let state = co.state.borrow_mut().take().unwrap_or_default();
        let mine = std::mem::replace(&mut self.cur, state);
        let prev = std::mem::replace(&mut self.thread, co.clone());
        *prev.state.borrow_mut() = Some(mine);
        prev.status.set(Status::Normal);
        co.status.set(Status::Running);
        self.ccalls += 1;
        let r = self.run_thread(h, args);
        self.ccalls -= 1;
        let mine = prev.state.borrow_mut().take().unwrap_or_default();
        let state = std::mem::replace(&mut self.cur, mine);
        prev.status.set(Status::Running);
        self.thread = prev;
        match r {
            Ok(vals) => {
                co.status.set(Status::Dead);
                Resume::Done(vals)
            }
            Err(Throw::Yield(vals)) => {
                *co.state.borrow_mut() = Some(state);
                co.status.set(Status::Suspended);
                Resume::Yielded(vals)
            }
            Err(e) => {
                co.status.set(Status::Dead);
                if let Throw::Error(v, _) = &e {
                    *co.error.borrow_mut() = Some(v.clone());
                }
                Resume::Error(e)
            }
        }
    }
    /// The running coroutine starts, or goes on from its yield.
    fn run_thread(&mut self, h: &mut dyn Host, args: Vec<Value>) -> Res {
        self.cur.runs = 1;
        let r = if let Some(mut f) = self.cur.start.take() {
            // A hooked function runs its hook; a newcclosure, what it wraps.
            for _ in 0..64 {
                let next = match &f {
                    Value::Func(c) => c.redirect(),
                    Value::Native(n) => n.redirect().or_else(|| match &n.kind {
                        NativeKind::Wrap(inner) => Some(inner.clone()),
                        _ => None,
                    }),
                    _ => None,
                };
                match next {
                    Some(to) => f = to,
                    None => break,
                }
            }
            match f {
                Value::Func(c) => match self.step() {
                    Err(e) => Err(e),
                    Ok(()) => {
                        let n = args.len();
                        self.cur.stack.push(Value::Func(c.clone()));
                        self.cur.stack.extend(args);
                        self.enter(c, 0, n, MULTI, Ret::Plain);
                        self.run(h, 0)
                    }
                },
                Value::Native(n) => match self.step() {
                    Err(e) => Err(e),
                    Ok(()) => match self.call_native(h, &n, args) {
                        Err(Throw::Yield(vals)) => {
                            self.cur.stack.push(Value::Nil);
                            self.cur.pending = Some(Pending {
                                func: 0,
                                nres: MULTI,
                                ret: Ret::Plain,
                            });
                            Err(Throw::Yield(vals))
                        }
                        r => r,
                    },
                },
                other => self.call_value(h, &other, args),
            }
        } else if let Some(p) = self.cur.pending.take() {
            self.deliver(p.func, args, p.nres, p.ret);
            if self.cur.frames.is_empty() {
                let top = self.cur.top.min(self.cur.stack.len());
                let vals = self.cur.stack.drain(p.func.min(top)..top).collect();
                self.cur.stack.clear();
                Ok(vals)
            } else {
                self.run(h, 0)
            }
        } else {
            Err(Throw::Error(Value::str("cannot resume dead coroutine"), String::new()))
        };
        self.cur.runs = 0;
        r
    }
    /// `coroutine.close`: a suspended or dead coroutine ends; the error it
    /// died of, if it failed. Err when it is running or normal.
    pub fn close_thread(&mut self, co: &Rc<Thread>) -> Result<Option<Value>, &'static str> {
        match co.status.get() {
            Status::Running => Err("cannot close a running coroutine"),
            Status::Normal => Err("cannot close a normal coroutine"),
            Status::Suspended | Status::Dead => {
                co.status.set(Status::Dead);
                *co.state.borrow_mut() = None;
                Ok(co.error.borrow_mut().take())
            }
        }
    }
}

/// The host's view into the VM, for the debug functions (Luau's, sUNC's)
/// built on it.
#[allow(dead_code)]
impl Lua {
    /// The method name of the running `obj:Name(...)` call on a userdata
    /// (`getnamecallmethod`).
    pub fn namecall_method(&self) -> Option<Str> {
        self.namecall.clone()
    }
    fn frame_at(&self, level: usize) -> Option<&Frame> {
        let i = self.cur.frames.len().checked_sub(level)?;
        if level == 0 {
            return None;
        }
        self.cur.frames.get(i)
    }
    /// The Lua function `level` frames up (1: the one running).
    pub fn frame_info(&self, level: usize) -> Option<FrameInfo> {
        let f = self.frame_at(level)?;
        Some(FrameInfo {
            closure: f.closure.clone(),
            proto: f.body.proto.clone(),
            line: f.line,
            pc: f.pc,
        })
    }
    /// Register `slot` of the frame `level` up (its cell, for a captured local).
    pub fn get_stack(&self, level: usize, slot: usize) -> Option<Value> {
        let f = self.frame_at(level)?;
        let p = &f.body.proto;
        if slot >= p.maxstack {
            return None;
        }
        if let Some(LocalVar { cell: Some(c), .. }) = p.local_at(slot, f.pc) {
            return self.cur.cells.get(f.cells + *c as usize).map(|c| c.borrow().clone());
        }
        self.cur.stack.get(f.base + slot).cloned()
    }
    pub fn set_stack(&mut self, level: usize, slot: usize, v: Value) -> bool {
        let Some(f) = self.frame_at(level) else {
            return false;
        };
        let p = &f.body.proto;
        if slot >= p.maxstack {
            return false;
        }
        if let Some(LocalVar { cell: Some(c), .. }) = p.local_at(slot, f.pc) {
            return match self.cur.cells.get(f.cells + *c as usize) {
                Some(cell) => {
                    *cell.borrow_mut() = v;
                    true
                }
                None => false,
            };
        }
        let i = f.base + slot;
        match self.cur.stack.get_mut(i) {
            Some(r) => {
                *r = v;
                true
            }
            None => false,
        }
    }
    /// Every register of the frame `level` up.
    pub fn stack_values(&self, level: usize) -> Option<Vec<Value>> {
        let n = self.frame_at(level)?.body.proto.maxstack;
        Some((0..n).map(|i| self.get_stack(level, i).unwrap_or_default()).collect())
    }
    /// Keeps a list of the closures and tables made from now on, for
    /// `getgc` (off by default).
    pub fn track_objects(&mut self, on: bool) {
        if on && self.registry.is_none() {
            self.registry = Some(Registry {
                prune_at: 1024,
                ..Registry::default()
            });
        } else if !on {
            self.registry = None;
        }
    }
    fn track_closure(&mut self, c: &Rc<Closure>) {
        if let Some(r) = &mut self.registry {
            r.closures.push(Rc::downgrade(c));
            r.prune();
        }
    }
    fn track_table(&mut self, t: &TableRef) {
        if let Some(r) = &mut self.registry {
            r.tables.push(Rc::downgrade(t));
            r.prune();
        }
    }
    /// Whether `getgc`'s tracking is on.
    pub fn tracking(&self) -> bool {
        self.registry.is_some()
    }
    /// The tracked threads still alive (`getreg`).
    pub fn live_threads(&mut self) -> Vec<Rc<Thread>> {
        self.registry
            .as_ref()
            .map(|r| r.threads.iter().filter_map(Weak::upgrade).collect())
            .unwrap_or_default()
    }
    /// The tracked closures still alive.
    pub fn live_closures(&mut self) -> Vec<Rc<Closure>> {
        self.registry
            .as_ref()
            .map_or_else(Vec::new, |r| r.closures.iter().filter_map(Weak::upgrade).collect())
    }
    /// The tracked tables still alive.
    pub fn live_tables(&mut self) -> Vec<TableRef> {
        self.registry
            .as_ref()
            .map_or_else(Vec::new, |r| r.tables.iter().filter_map(Weak::upgrade).collect())
    }
}

impl Registry {
    fn prune(&mut self) {
        if self.closures.len() + self.tables.len() < self.prune_at {
            return;
        }
        self.closures.retain(|w| w.strong_count() > 0);
        self.tables.retain(|w| w.strong_count() > 0);
        self.threads.retain(|w| w.strong_count() > 0);
        self.prune_at = ((self.closures.len() + self.tables.len()) * 2).max(1024);
    }
}

#[inline(always)]
fn num(v: &Value) -> f64 {
    match v {
        Value::Num(n) => *n,
        _ => 0.0,
    }
}

#[inline(always)]
fn put_num(slot: &mut Value, x: f64) {
    match slot {
        Value::Num(n) => *n = x,
        slot => *slot = Value::Num(x),
    }
}

/// A value as an integer for a bitwise operator: a number (or a string of
/// one) with an exact integer value; else whether it was a number (that has
/// no integer representation) or something else.
fn integer(v: &Value) -> Result<i64, bool> {
    match v.to_num() {
        Some(n) if n.fract() == 0.0 && (-9.223_372_036_854_776e18..9.223_372_036_854_776e18).contains(&n) => {
            Ok(n as i64)
        }
        Some(_) => Err(true),
        None => Err(false),
    }
}

fn bitwise_error(number: bool, v: &Value, desc: &str) -> String {
    if number {
        format!("number{desc} has no integer representation")
    } else {
        format!(
            "attempt to perform bitwise operation on a {} value{desc}",
            v.type_name()
        )
    }
}

/// Lua 5.3's bitwise operators on 64-bit integers; shifts are logical, and a
/// negative shift goes the other way.
fn bitwise(op: BinOp, a: i64, b: i64) -> i64 {
    let shl = |x: i64, n: i64| -> i64 {
        if n <= -64 || n >= 64 {
            0
        } else if n >= 0 {
            ((x as u64) << n) as i64
        } else {
            ((x as u64) >> -n) as i64
        }
    };
    match op {
        BinOp::BAnd => a & b,
        BinOp::BOr => a | b,
        BinOp::BXor => a ^ b,
        BinOp::Shl => shl(a, b),
        _ => shl(a, b.saturating_neg()),
    }
}

fn arith(op: BinOp, a: f64, b: f64) -> f64 {
    match op {
        BinOp::Add => a + b,
        BinOp::Sub => a - b,
        BinOp::Mul => a * b,
        BinOp::Div => a / b,
        BinOp::IDiv => (a / b).floor(),
        BinOp::Mod => {
            if b.is_infinite() && a.is_finite() {
                if (a >= 0.0) == (b > 0.0) { a } else { b }
            } else {
                let m = a % b;
                if m != 0.0 && (m < 0.0) != (b < 0.0) { m + b } else { m }
            }
        }
        _ => a.powf(b),
    }
}

// ----------------------------------------------------------------- helpers for built-ins

pub fn arg(args: &[Value], i: usize) -> Value {
    args.get(i).cloned().unwrap_or_default()
}

pub fn bad_arg(lua: &Lua, i: usize, fname: &str, msg: &str) -> Throw {
    lua.rt_error(format!("bad argument #{} to '{fname}' ({msg})", i + 1))
}

fn expected(lua: &Lua, args: &[Value], i: usize, fname: &str, what: &str) -> Throw {
    let got = args.get(i).map_or("no value", Value::type_name);
    bad_arg(lua, i, fname, &format!("{what} expected, got {got}"))
}

pub fn check_num(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<f64, Throw> {
    args.get(i)
        .and_then(Value::to_num)
        .ok_or_else(|| expected(lua, args, i, fname, "number"))
}

pub fn check_int(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<i64, Throw> {
    let n = check_num(lua, args, i, fname)?;
    if n.fract() != 0.0 || !n.is_finite() {
        return Err(bad_arg(lua, i, fname, "number has no integer representation"));
    }
    Ok(n as i64)
}

pub fn opt_int(lua: &Lua, args: &[Value], i: usize, fname: &str, default: i64) -> Result<i64, Throw> {
    match args.get(i) {
        None | Some(Value::Nil) => Ok(default),
        _ => check_int(lua, args, i, fname),
    }
}

pub fn opt_num(lua: &Lua, args: &[Value], i: usize, fname: &str, default: f64) -> Result<f64, Throw> {
    match args.get(i) {
        None | Some(Value::Nil) => Ok(default),
        _ => check_num(lua, args, i, fname),
    }
}

pub fn check_str(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<Str, Throw> {
    match args.get(i) {
        Some(Value::Str(s)) => Ok(s.clone()),
        Some(Value::Num(n)) => Ok(Rc::from(fmt_num(*n).as_bytes())),
        _ => Err(expected(lua, args, i, fname, "string")),
    }
}

pub fn opt_str(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<Option<Str>, Throw> {
    match args.get(i) {
        None | Some(Value::Nil) => Ok(None),
        _ => check_str(lua, args, i, fname).map(Some),
    }
}

pub fn check_text(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<String, Throw> {
    check_str(lua, args, i, fname).map(|s| String::from_utf8_lossy(&s).into_owned())
}

pub fn check_table(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<TableRef, Throw> {
    match args.get(i) {
        Some(Value::Table(t)) => Ok(t.clone()),
        _ => Err(expected(lua, args, i, fname, "table")),
    }
}

pub fn check_func(lua: &Lua, args: &[Value], i: usize, fname: &str) -> Result<Value, Throw> {
    match args.get(i) {
        Some(v) if v.is_callable() || matches!(v, Value::Table(_)) => Ok(v.clone()),
        _ => Err(expected(lua, args, i, fname, "function")),
    }
}
