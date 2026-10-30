// Horizon Studio's Lua: an interpreter for a subset of Lua 5.3 in plain Rust,
// with no dependencies, so the bundle can add it to the game as one module.
// Scripts get no file, network or process access: only the functions the
// studio hands them. Each call into a script runs under a step budget and a
// call depth limit, so an endless loop or runaway recursion stops that script
// with an error instead of freezing the game.
//
// What is here: nil, booleans, numbers (64-bit floats, printed as integers
// when they are whole), byte strings, tables with metatables, closures with
// upvalues, varargs and multiple results, every statement but goto, and the
// operators but the bitwise ones. The standard library is in hstudio_lualib.
// Not here: coroutines, goto, integer subtypes, io, os
// beyond the clock, load of binary chunks, string.pack, utf8.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub type Str = Rc<[u8]>;
pub type TableRef = Rc<RefCell<Table>>;
pub type Cell = Rc<RefCell<Value>>;
pub type Res = Result<Vec<Value>, Throw>;
/// A built-in function: the interpreter, the host, the function itself (its
/// `state` holds what an iterator keeps between calls) and the arguments.
pub type Builtin = fn(&mut Lua, &mut dyn Host, &Native, Vec<Value>) -> Res;

/// The longest string a script can make.
pub const MAX_STRING: usize = 4 << 20;
/// The most entries one table can hold.
pub const MAX_TABLE: usize = 1 << 20;
/// Nested calls before "stack overflow".
pub const MAX_DEPTH: usize = 200;
/// The game's stack a script may use below where the host called it: the
/// real guard against overflow, as frames differ between builds (Windows
/// gives the main thread 1 MiB in all). About 120 nested Lua calls in a
/// release build; unoptimised builds (tests) have much bigger frames.
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
        }
    }
}

/// A function the interpreter does not run itself: a built-in, or one the
/// host answers by its id.
pub struct Native {
    pub name: Rc<str>,
    pub kind: NativeKind,
    pub state: RefCell<Vec<Value>>,
}

pub enum NativeKind {
    Builtin(Builtin),
    Host(u32),
}

impl Native {
    pub fn builtin(name: &str, f: Builtin) -> Value {
        Value::Native(Rc::new(Native {
            name: name.into(),
            kind: NativeKind::Builtin(f),
            state: RefCell::new(Vec::new()),
        }))
    }
    pub fn with_state(name: &str, f: Builtin, state: Vec<Value>) -> Value {
        Value::Native(Rc::new(Native {
            name: name.into(),
            kind: NativeKind::Builtin(f),
            state: RefCell::new(state),
        }))
    }
    pub fn host(name: &str, id: u32) -> Value {
        Value::Native(Rc::new(Native {
            name: name.into(),
            kind: NativeKind::Host(id),
            state: RefCell::new(Vec::new()),
        }))
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
        }
    }
    pub fn traceback(&self) -> &str {
        match self {
            Throw::Error(_, t) => t,
            Throw::Fatal(_) => "",
        }
    }
}

// ----------------------------------------------------------------- tables

#[derive(Clone, PartialEq, Eq, Hash)]
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
    })
}

/// A table: an array part for 1..n and an insertion-ordered hash part, so
/// `pairs` visits entries in a stable order.
#[derive(Default)]
pub struct Table {
    arr: Vec<Value>,
    hash: Vec<(Key, Value, Value)>,
    index: HashMap<Key, usize>,
    dead: usize,
    pub meta: Option<TableRef>,
}

impl Table {
    pub fn new() -> Table {
        Table::default()
    }
    pub fn from_list(items: Vec<Value>) -> Table {
        let mut t = Table::new();
        for v in items {
            t.push(v);
        }
        t
    }
    fn get_key(&self, k: &Key) -> Value {
        if let Key::Int(i) = k
            && *i >= 1
            && (*i as usize) <= self.arr.len()
        {
            return self.arr[*i as usize - 1].clone();
        }
        match self.index.get(k) {
            Some(&p) => self.hash[p].2.clone(),
            None => Value::Nil,
        }
    }
    pub fn get(&self, k: &Value) -> Value {
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
                    match self.index.get(&next) {
                        Some(&p) if !self.hash[p].2.is_nil() => {
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
        if let Some(&p) = self.index.get(&key) {
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
        self.index.insert(key.clone(), self.hash.len());
        self.hash.push((key, k, v));
        Ok(())
    }
    fn kill(&mut self, key: &Key) {
        if let Some(&p) = self.index.get(key)
            && !self.hash[p].2.is_nil()
        {
            self.hash[p].2 = Value::Nil;
            self.dead += 1;
        }
    }
    fn compact(&mut self) {
        self.hash.retain(|e| !e.2.is_nil());
        self.index.clear();
        for (i, e) in self.hash.iter().enumerate() {
            self.index.insert(e.0.clone(), i);
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
                    from_hash = *self.index.get(&key).ok_or(())? + 1;
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
    /// Removes position `pos` (1-based) of the array part, shifting the rest down.
    pub fn remove(&mut self, pos: usize) -> Value {
        let n = self.len();
        if pos == 0 || pos > n {
            return Value::Nil;
        }
        self.arr.truncate(n);
        self.arr.remove(pos - 1)
    }
}

// ----------------------------------------------------------------- functions

pub struct UpDesc {
    pub name: Rc<str>,
    pub from_local: bool,
    pub index: usize,
}

pub struct Proto {
    /// `function 'name'`, `function <main.lua:12>` or `main chunk`.
    pub name: Rc<str>,
    pub chunk: Rc<str>,
    #[allow(dead_code)]
    pub line: u32,
    pub nparams: usize,
    pub vararg: bool,
    pub nslots: usize,
    pub upvals: Vec<UpDesc>,
    body: Vec<Stmt>,
}

pub struct Closure {
    pub proto: Rc<Proto>,
    pub upvals: Vec<Cell>,
    pub env: TableRef,
}

struct Stmt {
    line: u32,
    kind: StmtKind,
}

enum StmtKind {
    Local(Vec<usize>, Vec<Expr>),
    LocalFunction(usize, Rc<Proto>),
    Assign(Vec<Expr>, Vec<Expr>),
    Call(Expr),
    Do(Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    Repeat(Vec<Stmt>, Expr),
    If(Vec<(Expr, Vec<Stmt>)>, Option<Vec<Stmt>>),
    NumFor(usize, Expr, Expr, Option<Expr>, Vec<Stmt>),
    GenFor(Vec<usize>, Vec<Expr>, Vec<Stmt>),
    Return(Vec<Expr>),
    Break,
}

#[derive(Clone, Copy, PartialEq)]
enum BinOp {
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

enum Expr {
    Nil,
    True,
    False,
    Num(f64),
    Str(Str),
    Vararg,
    Function(Rc<Proto>),
    Local(usize, Rc<str>),
    Upval(usize, Rc<str>),
    Global(Str),
    Index(Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Method(Box<Expr>, Str, Vec<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Un(UnOp, Box<Expr>),
    Table(Vec<(Option<Expr>, Expr)>),
    Paren(Box<Expr>),
}

impl Expr {
    fn is_multi(&self) -> bool {
        matches!(self, Expr::Call(..) | Expr::Method(..) | Expr::Vararg)
    }
    /// ` (global 'x')` for error messages, or "".
    fn describe(&self) -> String {
        match self {
            Expr::Local(_, n) => format!(" (local '{n}')"),
            Expr::Upval(_, n) => format!(" (upvalue '{n}')"),
            Expr::Global(n) => format!(" (global '{}')", String::from_utf8_lossy(n)),
            Expr::Index(_, k) => match &**k {
                Expr::Str(s) => format!(" (field '{}')", String::from_utf8_lossy(s)),
                _ => String::new(),
            },
            Expr::Str(_) | Expr::Num(_) => " (constant)".into(),
            _ => String::new(),
        }
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
    Eof,
}

const KEYWORDS: [&str; 22] = [
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in", "local", "nil",
    "not", "or", "repeat", "return", "then", "true", "until", "while",
];

// Longest first, so `...` wins over `..` and `.`.
const SYMBOLS: [&str; 33] = [
    "...", "..", "==", "~=", "<=", ">=", "//", "::", "<<", ">>", "+", "-", "*", "/", "%", "^", "#", "&", "~", "|", "<",
    ">", "=", "(", ")", "{", "}", "[", "]", ";", ":", ",", ".",
];

fn tok_text(t: &Tok) -> String {
    match t {
        Tok::Name(n) => n.to_string(),
        Tok::Num(n) => fmt_num(*n),
        Tok::Str(s) => String::from_utf8_lossy(s).into_owned(),
        Tok::Op(o) => (*o).to_owned(),
        Tok::Eof => "<eof>".into(),
    }
}

fn lex(src: &[u8], chunk: &str) -> Result<Vec<(Tok, u32)>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    let mut line = 1u32;
    let err = |line: u32, msg: &str| format!("{chunk}:{line}: {msg}");
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
            match KEYWORDS.iter().find(|k| **k == word) {
                Some(k) => out.push((Tok::Op(k), line)),
                None => out.push((Tok::Name(word.into()), line)),
            }
            continue;
        }
        if c.is_ascii_digit() || (c == b'.' && src.get(i + 1).is_some_and(u8::is_ascii_digit)) {
            let start = i;
            if c == b'0' && matches!(src.get(i + 1), Some(b'x' | b'X')) {
                i += 2;
                while i < src.len() && (src[i].is_ascii_hexdigit()) {
                    i += 1;
                }
            } else {
                while i < src.len() {
                    let d = src[i];
                    let exp_sign = matches!(d, b'+' | b'-') && matches!(src[i - 1], b'e' | b'E');
                    if d.is_ascii_alphanumeric() || d == b'.' || exp_sign {
                        i += 1;
                    } else {
                        break;
                    }
                }
            }
            let text = &src[start..i];
            let n = str_to_num(text).ok_or_else(|| {
                err(
                    line,
                    &format!("malformed number near '{}'", String::from_utf8_lossy(text)),
                )
            })?;
            out.push((Tok::Num(n), line));
            continue;
        }
        if c == b'"' || c == b'\'' {
            let (s, end, lines) = short_string(src, i).map_err(|m| err(line, m))?;
            out.push((Tok::Str(s.into()), line));
            line += lines;
            i = end;
            continue;
        }
        if c == b'['
            && let Some(level) = long_bracket(src, i)
        {
            let (s, end, lines) = long_string(src, i, level).ok_or_else(|| err(line, "unfinished long string"))?;
            out.push((Tok::Str(s.into()), line));
            line += lines;
            i = end;
            continue;
        }
        match SYMBOLS.iter().find(|s| src[i..].starts_with(s.as_bytes())) {
            Some(s) => {
                out.push((Tok::Op(s), line));
                i += s.len();
            }
            None => {
                return Err(err(
                    line,
                    &format!("unexpected symbol near '{}'", String::from_utf8_lossy(&src[i..i + 1])),
                ));
            }
        }
    }
    out.push((Tok::Eof, line));
    Ok(out)
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
    let quote = src[i];
    let mut j = i + 1;
    let mut out = Vec::new();
    let mut lines = 0;
    loop {
        let Some(&c) = src.get(j) else {
            return Err("unfinished string");
        };
        if c == quote {
            return Ok((out, j + 1, lines));
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

// ----------------------------------------------------------------- parser

struct FuncState {
    actives: Vec<(Rc<str>, usize)>,
    blocks: Vec<usize>,
    nslots: usize,
    upvals: Vec<UpDesc>,
    vararg: bool,
    loops: u32,
}

impl FuncState {
    fn new(vararg: bool) -> FuncState {
        FuncState {
            actives: Vec::new(),
            blocks: Vec::new(),
            nslots: 0,
            upvals: Vec::new(),
            vararg,
            loops: 0,
        }
    }
}

enum Var {
    Local(usize),
    Up(usize),
}

struct Parser {
    toks: Vec<(Tok, u32)>,
    pos: usize,
    chunk: Rc<str>,
    fs: Vec<FuncState>,
    depth: u32,
    stack_base: usize,
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
    fn declare(&mut self, name: Rc<str>) -> usize {
        let fs = self.fs();
        let slot = fs.actives.len();
        fs.actives.push((name, slot));
        fs.nslots = fs.nslots.max(slot + 1);
        slot
    }
    fn resolve(&mut self, level: usize, name: &str) -> Option<Var> {
        let fs = &self.fs[level];
        if let Some((_, s)) = fs.actives.iter().rev().find(|(n, _)| &**n == name) {
            return Some(Var::Local(*s));
        }
        if let Some(i) = fs.upvals.iter().position(|u| &*u.name == name) {
            return Some(Var::Up(i));
        }
        if level == 0 {
            return None;
        }
        let (from_local, index) = match self.resolve(level - 1, name)? {
            Var::Local(s) => (true, s),
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
            None => Expr::Global(Rc::from(name.as_bytes())),
        }
    }

    fn chunk(mut self) -> PResult<Proto> {
        self.fs.push(FuncState::new(true));
        let body = self.block()?;
        if *self.peek() != Tok::Eof {
            return self.error("'<eof>' expected");
        }
        let fs = self.fs.pop().expect("the chunk's function");
        Ok(Proto {
            name: "main chunk".into(),
            chunk: self.chunk.clone(),
            line: 0,
            nparams: 0,
            vararg: true,
            nslots: fs.nslots,
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
        self.fs().loops += 1;
        let b = self.block();
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
                    let slot = self.declare(name.clone());
                    let proto = self.body(format!("function '{name}'"), false, line)?;
                    StmtKind::LocalFunction(slot, proto)
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
            _ => self.expr_stat()?,
        };
        Ok(Some(Stmt { line, kind }))
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
        if self.accept("=") {
            let start = self.expr()?;
            self.expect(",", "")?;
            let limit = self.expr()?;
            let step = if self.accept(",") { Some(self.expr()?) } else { None };
            self.expect("do", "")?;
            self.open_scope();
            let slot = self.declare(first);
            let body = self.loop_block()?;
            self.close_scope();
            self.expect_match("end", "for", line)?;
            return Ok(StmtKind::NumFor(slot, start, limit, step, body));
        }
        let mut names = vec![first];
        while self.accept(",") {
            names.push(self.name()?);
        }
        self.expect("in", "")?;
        let exprs = self.exprlist()?;
        self.expect("do", "")?;
        self.open_scope();
        let slots = names.into_iter().map(|n| self.declare(n)).collect();
        let body = self.loop_block()?;
        self.close_scope();
        self.expect_match("end", "for", line)?;
        Ok(StmtKind::GenFor(slots, exprs, body))
    }
    fn func_stat(&mut self, line: u32) -> PResult<StmtKind> {
        self.advance();
        let first = self.name()?;
        let mut full = first.to_string();
        let mut target = self.var(first);
        let mut method = false;
        loop {
            if self.accept(".") {
                let n = self.name()?;
                full.push('.');
                full.push_str(&n);
                target = Expr::Index(Box::new(target), Box::new(Expr::Str(Rc::from(n.as_bytes()))));
            } else if self.accept(":") {
                let n = self.name()?;
                full.push(':');
                full.push_str(&n);
                target = Expr::Index(Box::new(target), Box::new(Expr::Str(Rc::from(n.as_bytes()))));
                method = true;
                break;
            } else {
                break;
            }
        }
        let proto = self.body(format!("function '{full}'"), method, line)?;
        Ok(StmtKind::Assign(vec![target], vec![Expr::Function(proto)]))
    }
    fn local_stat(&mut self) -> PResult<StmtKind> {
        let mut names = vec![self.name()?];
        self.attrib()?;
        while self.accept(",") {
            names.push(self.name()?);
            self.attrib()?;
        }
        let exprs = if self.accept("=") { self.exprlist()? } else { Vec::new() };
        let slots = names.into_iter().map(|n| self.declare(n)).collect();
        Ok(StmtKind::Local(slots, exprs))
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
            return Ok(StmtKind::Assign(targets, values));
        }
        if !matches!(e, Expr::Call(..) | Expr::Method(..)) {
            return self.error("syntax error (expected a call or an assignment)");
        }
        Ok(StmtKind::Call(e))
    }
    fn body(&mut self, name: String, method: bool, line: u32) -> PResult<Rc<Proto>> {
        self.fs.push(FuncState::new(false));
        self.open_scope();
        if method {
            self.declare("self".into());
        }
        self.expect("(", "")?;
        if !self.check(")") {
            loop {
                if self.accept("...") {
                    self.fs().vararg = true;
                    break;
                }
                let n = self.name()?;
                self.declare(n);
                if !self.accept(",") {
                    break;
                }
            }
        }
        self.expect(")", "")?;
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
        Ok(Rc::new(Proto {
            name: name.into(),
            chunk: self.chunk.clone(),
            line,
            nparams,
            vararg: fs.vararg,
            nslots: fs.nslots,
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
                Expr::Function(self.body(String::new(), false, line)?)
            }
            _ => self.suffixed()?,
        })
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
                items.push((Some(k), self.expr()?));
            } else if matches!(self.peek(), Tok::Name(_)) && matches!(self.peek2(), Tok::Op("=")) {
                let n = self.name()?;
                self.advance();
                items.push((Some(Expr::Str(Rc::from(n.as_bytes()))), self.expr()?));
            } else {
                items.push((None, self.expr()?));
            }
            if !self.accept(",") && !self.accept(";") {
                break;
            }
        }
        self.expect_match("}", "{", line)?;
        Ok(Expr::Table(items))
    }
}

// ----------------------------------------------------------------- interpreter

enum Flow {
    Normal,
    Break,
    Return(Vec<Value>),
}

/// Gives a local its new value: a declaration makes a new variable, but its
/// old cell can be reused when no closure kept it.
#[inline]
fn fresh(slot: &mut Cell, v: Value) {
    if Rc::strong_count(slot) == 1 {
        *slot.borrow_mut() = v;
    } else {
        *slot = Rc::new(RefCell::new(v));
    }
}

struct Frame {
    slots: Vec<Cell>,
    closure: Rc<Closure>,
    varargs: Vec<Value>,
}

enum Place {
    Local(usize),
    Up(usize),
    Global(Str),
    Index(Value, Value, String),
}

pub struct Lua {
    /// Steps left before the running script is stopped.
    pub steps: u64,
    /// The `string` table: methods of strings (`s:upper()`).
    pub string_lib: Option<TableRef>,
    /// The random generator's state (math.random).
    pub rng: u64,
    pub started: std::time::Instant,
    depth: usize,
    stack_base: usize,
    line: u32,
    chunk: Rc<str>,
    /// For each running Lua function: its name and where it was called from.
    frames: Vec<(Rc<str>, Rc<str>, u32)>,
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
            string_lib: None,
            rng: 0x2545_F491_4F6C_DD1D,
            started: std::time::Instant::now(),
            depth: 0,
            stack_base: 0,
            line: 0,
            chunk: HOST_CHUNK.into(),
            frames: Vec::new(),
        }
    }

    /// Compiles `src` into a function whose globals are `env`.
    pub fn load(&mut self, src: &str, chunk: &str, env: &TableRef) -> Result<Value, String> {
        self.load_bytes(src.as_bytes(), chunk, env)
    }
    pub fn load_bytes(&mut self, src: &[u8], chunk: &str, env: &TableRef) -> Result<Value, String> {
        let toks = lex(src, chunk)?;
        let parser = Parser {
            toks,
            pos: 0,
            chunk: chunk.into(),
            fs: Vec::new(),
            depth: 0,
            stack_base: stack_addr(),
        };
        let proto = parser.chunk()?;
        Ok(Value::Func(Rc::new(Closure {
            proto: Rc::new(proto),
            upvals: Vec::new(),
            env: env.clone(),
        })))
    }

    /// An error at the running statement: `chunk:line: msg`, with a traceback.
    pub fn rt_error(&self, msg: impl AsRef<str>) -> Throw {
        let text = if &*self.chunk == HOST_CHUNK {
            msg.as_ref().to_owned()
        } else {
            format!("{}:{}: {}", self.chunk, self.line, msg.as_ref())
        };
        Throw::Error(Value::str(&text), self.traceback())
    }
    /// Where `error(msg, level)` puts its message: level 1 is the running
    /// function, 2 its caller.
    pub fn position(&self, level: usize) -> Option<String> {
        if level == 0 {
            return None;
        }
        let (chunk, line) = if level == 1 {
            (self.chunk.clone(), self.line)
        } else {
            let f = self.frames.len().checked_sub(level - 1)?;
            let (_, c, l) = self.frames.get(f)?;
            (c.clone(), *l)
        };
        (&*chunk != HOST_CHUNK).then(|| format!("{chunk}:{line}:"))
    }
    pub fn traceback(&self) -> String {
        let mut lines = Vec::new();
        let (mut chunk, mut line) = (self.chunk.clone(), self.line);
        for (name, caller, at) in self.frames.iter().rev() {
            if &*chunk != HOST_CHUNK {
                if &**name == "main chunk" {
                    lines.push(format!("{chunk}:{line}: in main chunk"));
                } else {
                    lines.push(format!("{chunk}:{line}: in {name}"));
                }
            }
            chunk = caller.clone();
            line = *at;
        }
        if lines.len() > 12 {
            let tail = lines.split_off(lines.len() - 5);
            lines.truncate(6);
            lines.push("...".into());
            lines.extend(tail);
        }
        lines.join("\n")
    }
    #[inline]
    fn step(&mut self) -> Result<(), Throw> {
        if self.steps == 0 {
            let at = if &*self.chunk == HOST_CHUNK {
                String::new()
            } else {
                format!("{}:{}: ", self.chunk, self.line)
            };
            return Err(Throw::Fatal(format!(
                "{at}script ran too long and was stopped (an endless loop?)"
            )));
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

    /// Calls `f` with `args`.
    pub fn call(&mut self, h: &mut dyn Host, f: &Value, args: Vec<Value>) -> Res {
        if self.depth == 0 {
            self.stack_base = stack_addr();
        }
        match f {
            Value::Func(c) => self.call_closure(h, c.clone(), args),
            Value::Native(n) => {
                self.step()?;
                match n.kind {
                    NativeKind::Builtin(b) => b(self, h, n, args),
                    NativeKind::Host(id) => h.call(self, id, args),
                }
            }
            other => self.call_other(h, other, args),
        }
    }
    #[inline(never)]
    fn call_other(&mut self, h: &mut dyn Host, f: &Value, args: Vec<Value>) -> Res {
        if let Some(mm) = self.metamethod(f, "__call")
            && !matches!(mm, Value::Table(_))
        {
            let mut a = Vec::with_capacity(args.len() + 1);
            a.push(f.clone());
            a.extend(args);
            return self.call(h, &mm, a);
        }
        Err(self.rt_error(format!("attempt to call a {} value", f.type_name())))
    }
    /// The first result of a call, or nil.
    pub fn call1(&mut self, h: &mut dyn Host, f: &Value, args: Vec<Value>) -> Result<Value, Throw> {
        Ok(self.call(h, f, args)?.into_iter().next().unwrap_or_default())
    }
    fn call_closure(&mut self, h: &mut dyn Host, c: Rc<Closure>, args: Vec<Value>) -> Res {
        if self.depth >= MAX_DEPTH || self.stack_base.saturating_sub(stack_addr()) > STACK_LIMIT {
            return Err(self.rt_error("stack overflow"));
        }
        self.step()?;
        let proto = c.proto.clone();
        let slots: Vec<Cell> = (0..proto.nslots).map(|_| Rc::new(RefCell::new(Value::Nil))).collect();
        let mut args = args.into_iter();
        for slot in slots.iter().take(proto.nparams) {
            *slot.borrow_mut() = args.next().unwrap_or_default();
        }
        let varargs = if proto.vararg { args.collect() } else { Vec::new() };
        self.frames.push((proto.name.clone(), self.chunk.clone(), self.line));
        let saved = (std::mem::replace(&mut self.chunk, proto.chunk.clone()), self.line);
        self.depth += 1;
        let mut frame = Frame {
            slots,
            closure: c,
            varargs,
        };
        let r = self.exec_block(h, &mut frame, &proto.body);
        self.depth -= 1;
        (self.chunk, self.line) = saved;
        self.frames.pop();
        match r? {
            Flow::Return(v) => Ok(v),
            _ => Ok(Vec::new()),
        }
    }

    fn exec_block(&mut self, h: &mut dyn Host, f: &mut Frame, b: &[Stmt]) -> Result<Flow, Throw> {
        for s in b {
            self.line = s.line;
            self.step()?;
            match self.exec(h, f, s)? {
                Flow::Normal => {}
                other => return Ok(other),
            }
        }
        Ok(Flow::Normal)
    }

    // Each kind of statement runs in its own function, so a call nested in
    // a script holds only the frames of the statements it passes through.
    fn exec(&mut self, h: &mut dyn Host, f: &mut Frame, s: &Stmt) -> Result<Flow, Throw> {
        match &s.kind {
            StmtKind::Local(slots, exprs) => self.exec_local(h, f, slots, exprs)?,
            StmtKind::LocalFunction(slot, proto) => {
                f.slots[*slot] = Rc::new(RefCell::new(Value::Nil));
                let c = self.closure(f, proto);
                *f.slots[*slot].borrow_mut() = c;
            }
            StmtKind::Assign(targets, exprs) => self.exec_assign(h, f, targets, exprs)?,
            StmtKind::Call(e) => {
                self.eval_call(h, f, e)?;
            }
            StmtKind::Do(b) => return self.exec_block(h, f, b),
            StmtKind::While(cond, body) => return self.exec_while(h, f, cond, body),
            StmtKind::Repeat(body, cond) => return self.exec_repeat(h, f, body, cond),
            StmtKind::If(arms, otherwise) => {
                for (cond, body) in arms {
                    if self.eval(h, f, cond)?.truthy() {
                        return self.exec_block(h, f, body);
                    }
                }
                if let Some(body) = otherwise {
                    return self.exec_block(h, f, body);
                }
            }
            StmtKind::NumFor(slot, start, limit, step, body) => {
                return self.exec_numfor(h, f, *slot, [start, limit], step.as_ref(), body);
            }
            StmtKind::GenFor(slots, exprs, body) => return self.exec_genfor(h, f, slots, exprs, body),
            StmtKind::Return(exprs) => return self.exec_return(h, f, exprs),
            StmtKind::Break => return Ok(Flow::Break),
        }
        Ok(Flow::Normal)
    }
    #[inline(never)]
    fn exec_local(&mut self, h: &mut dyn Host, f: &mut Frame, slots: &[usize], exprs: &[Expr]) -> Result<(), Throw> {
        if let ([slot], [e]) = (slots, exprs) {
            let v = self.eval(h, f, e)?;
            fresh(&mut f.slots[*slot], v);
        } else {
            let vals = self.eval_list_n(h, f, exprs, slots.len())?;
            for (slot, v) in slots.iter().zip(vals) {
                fresh(&mut f.slots[*slot], v);
            }
        }
        Ok(())
    }
    #[inline(never)]
    fn exec_assign(&mut self, h: &mut dyn Host, f: &mut Frame, targets: &[Expr], exprs: &[Expr]) -> Result<(), Throw> {
        if let ([t], [e]) = (targets, exprs) {
            let place = self.place(h, f, t)?;
            let v = self.eval(h, f, e)?;
            return self.assign(h, f, place, v);
        }
        let mut places = Vec::with_capacity(targets.len());
        for t in targets {
            places.push(self.place(h, f, t)?);
        }
        let vals = self.eval_list_n(h, f, exprs, targets.len())?;
        for (p, v) in places.into_iter().zip(vals) {
            self.assign(h, f, p, v)?;
        }
        Ok(())
    }
    #[inline(never)]
    fn exec_return(&mut self, h: &mut dyn Host, f: &mut Frame, exprs: &[Expr]) -> Result<Flow, Throw> {
        if let [e @ (Expr::Call(..) | Expr::Method(..))] = exprs {
            return Ok(Flow::Return(self.eval_call(h, f, e)?));
        }
        Ok(Flow::Return(self.eval_list(h, f, exprs)?))
    }
    #[inline(never)]
    fn exec_while(&mut self, h: &mut dyn Host, f: &mut Frame, cond: &Expr, body: &[Stmt]) -> Result<Flow, Throw> {
        loop {
            self.step()?;
            if !self.eval(h, f, cond)?.truthy() {
                return Ok(Flow::Normal);
            }
            match self.exec_block(h, f, body)? {
                Flow::Break => return Ok(Flow::Normal),
                Flow::Return(v) => return Ok(Flow::Return(v)),
                Flow::Normal => {}
            }
        }
    }
    #[inline(never)]
    fn exec_repeat(&mut self, h: &mut dyn Host, f: &mut Frame, body: &[Stmt], cond: &Expr) -> Result<Flow, Throw> {
        loop {
            match self.exec_block(h, f, body)? {
                Flow::Break => return Ok(Flow::Normal),
                Flow::Return(v) => return Ok(Flow::Return(v)),
                Flow::Normal => {}
            }
            if self.eval(h, f, cond)?.truthy() {
                return Ok(Flow::Normal);
            }
        }
    }
    #[inline(never)]
    fn exec_numfor(
        &mut self,
        h: &mut dyn Host,
        f: &mut Frame,
        slot: usize,
        [start, limit]: [&Expr; 2],
        step: Option<&Expr>,
        body: &[Stmt],
    ) -> Result<Flow, Throw> {
        let mut bounds = [0.0; 3];
        for (i, (e, what)) in [(Some(start), "initial"), (Some(limit), "limit"), (step, "step")]
            .into_iter()
            .enumerate()
        {
            bounds[i] = match e {
                Some(e) => {
                    let v = self.eval(h, f, e)?;
                    v.to_num()
                        .ok_or_else(|| self.rt_error(format!("'for' {what} value must be a number")))?
                }
                None => 1.0,
            };
        }
        let [a, b, c] = bounds;
        if c == 0.0 {
            return Err(self.rt_error("'for' step is zero"));
        }
        let mut k = 0u64;
        loop {
            let i = a + k as f64 * c;
            if (c > 0.0 && i > b) || (c < 0.0 && i < b) {
                return Ok(Flow::Normal);
            }
            self.step()?;
            fresh(&mut f.slots[slot], Value::Num(i));
            match self.exec_block(h, f, body)? {
                Flow::Break => return Ok(Flow::Normal),
                Flow::Return(v) => return Ok(Flow::Return(v)),
                Flow::Normal => {}
            }
            k += 1;
        }
    }
    #[inline(never)]
    fn exec_genfor(
        &mut self,
        h: &mut dyn Host,
        f: &mut Frame,
        slots: &[usize],
        exprs: &[Expr],
        body: &[Stmt],
    ) -> Result<Flow, Throw> {
        let mut init = self.eval_list_n(h, f, exprs, 3)?.into_iter();
        let iter = init.next().unwrap_or_default();
        let state = init.next().unwrap_or_default();
        let mut control = init.next().unwrap_or_default();
        if !iter.is_callable() && !matches!(iter, Value::Table(_)) {
            return Err(self.rt_error(format!("attempt to call a {} value (for iterator)", iter.type_name())));
        }
        loop {
            self.step()?;
            let r = self.call(h, &iter, vec![state.clone(), control.clone()])?;
            let first = r.first().cloned().unwrap_or_default();
            if first.is_nil() {
                return Ok(Flow::Normal);
            }
            control = first;
            let mut r = r.into_iter();
            for slot in slots {
                fresh(&mut f.slots[*slot], r.next().unwrap_or_default());
            }
            match self.exec_block(h, f, body)? {
                Flow::Break => return Ok(Flow::Normal),
                Flow::Return(v) => return Ok(Flow::Return(v)),
                Flow::Normal => {}
            }
        }
    }

    fn closure(&self, f: &Frame, proto: &Rc<Proto>) -> Value {
        let upvals = proto
            .upvals
            .iter()
            .map(|u| {
                if u.from_local {
                    f.slots[u.index].clone()
                } else {
                    f.closure.upvals[u.index].clone()
                }
            })
            .collect();
        Value::Func(Rc::new(Closure {
            proto: proto.clone(),
            upvals,
            env: f.closure.env.clone(),
        }))
    }

    fn place(&mut self, h: &mut dyn Host, f: &mut Frame, e: &Expr) -> Result<Place, Throw> {
        Ok(match e {
            Expr::Local(s, _) => Place::Local(*s),
            Expr::Upval(i, _) => Place::Up(*i),
            Expr::Global(n) => Place::Global(n.clone()),
            Expr::Index(o, k) => {
                let ov = self.eval(h, f, o)?;
                let kv = self.eval(h, f, k)?;
                Place::Index(ov, kv, o.describe())
            }
            _ => return Err(self.rt_error("cannot assign to this expression")),
        })
    }
    fn assign(&mut self, h: &mut dyn Host, f: &mut Frame, p: Place, v: Value) -> Result<(), Throw> {
        match p {
            Place::Local(s) => *f.slots[s].borrow_mut() = v,
            Place::Up(i) => *f.closure.upvals[i].borrow_mut() = v,
            Place::Global(n) => {
                let env = Value::Table(f.closure.env.clone());
                self.set_index(h, &env, Value::Str(n), v)?;
            }
            Place::Index(o, k, desc) => {
                if !matches!(o, Value::Table(_)) {
                    return Err(self.rt_error(format!("attempt to index a {} value{desc}", o.type_name())));
                }
                self.set_index(h, &o, k, v)?;
            }
        }
        Ok(())
    }

    pub fn metamethod(&self, v: &Value, event: &str) -> Option<Value> {
        let Value::Table(t) = v else {
            return None;
        };
        let mt = t.borrow().meta.clone()?;
        let m = mt.borrow().get_str(event);
        (!m.is_nil()).then_some(m)
    }

    /// `obj[key]`, with `__index`.
    pub fn index(&mut self, h: &mut dyn Host, obj: &Value, key: &Value) -> Result<Value, Throw> {
        let mut cur = obj.clone();
        for _ in 0..64 {
            let handler = match &cur {
                Value::Table(t) => {
                    let t = t.borrow();
                    let v = t.get(key);
                    if !v.is_nil() {
                        return Ok(v);
                    }
                    let Some(mt) = &t.meta else {
                        return Ok(Value::Nil);
                    };
                    let handler = mt.borrow().get_str("__index");
                    if handler.is_nil() {
                        return Ok(Value::Nil);
                    }
                    handler
                }
                Value::Str(_) => {
                    return Ok(self.string_lib.as_ref().map_or(Value::Nil, |s| s.borrow().get(key)));
                }
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
                            Some(mt) if tb.get(&key).is_nil() => mt.borrow().get_str("__newindex"),
                            _ => Value::Nil,
                        }
                    };
                    if handler.is_nil() {
                        let r = t.borrow_mut().set(key, v);
                        return r.map_err(|e| match e {
                            "table is too large" => {
                                Throw::Fatal(format!("{}:{}: table is too large", self.chunk, self.line))
                            }
                            e => self.rt_error(e),
                        });
                    }
                    handler
                }
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

    // As with statements, the bigger expressions have functions of their
    // own, which keeps this one's frame small on the path of nested calls.
    fn eval(&mut self, h: &mut dyn Host, f: &mut Frame, e: &Expr) -> Result<Value, Throw> {
        Ok(match e {
            Expr::Nil => Value::Nil,
            Expr::True => Value::Bool(true),
            Expr::False => Value::Bool(false),
            Expr::Num(n) => Value::Num(*n),
            Expr::Str(s) => Value::Str(s.clone()),
            Expr::Vararg => f.varargs.first().cloned().unwrap_or_default(),
            Expr::Function(p) => self.closure(f, p),
            Expr::Local(s, _) => f.slots[*s].borrow().clone(),
            Expr::Upval(i, _) => f.closure.upvals[*i].borrow().clone(),
            Expr::Global(n) => return self.eval_global(h, f, n),
            Expr::Index(o, k) => return self.eval_index(h, f, o, k),
            Expr::Call(..) | Expr::Method(..) => self.eval_call(h, f, e)?.into_iter().next().unwrap_or_default(),
            Expr::And(a, b) => {
                let v = self.eval(h, f, a)?;
                if v.truthy() { self.eval(h, f, b)? } else { v }
            }
            Expr::Or(a, b) => {
                let v = self.eval(h, f, a)?;
                if v.truthy() { v } else { self.eval(h, f, b)? }
            }
            Expr::Bin(op, a, b) => return self.eval_binary(h, f, *op, a, b),
            Expr::Un(op, a) => return self.eval_unary(h, f, *op, a),
            Expr::Table(items) => return self.eval_table(h, f, items),
            Expr::Paren(inner) => self.eval(h, f, inner)?,
        })
    }
    #[inline(never)]
    fn eval_global(&mut self, h: &mut dyn Host, f: &mut Frame, n: &Str) -> Result<Value, Throw> {
        let env = f.closure.env.clone();
        let v = env.borrow().get(&Value::Str(n.clone()));
        if v.is_nil() && env.borrow().meta.is_some() {
            return self.index(h, &Value::Table(env), &Value::Str(n.clone()));
        }
        Ok(v)
    }
    #[inline(never)]
    fn eval_index(&mut self, h: &mut dyn Host, f: &mut Frame, o: &Expr, k: &Expr) -> Result<Value, Throw> {
        let ov = self.eval(h, f, o)?;
        let kv = self.eval(h, f, k)?;
        if let Value::Table(t) = &ov {
            let v = t.borrow().get(&kv);
            if !v.is_nil() || t.borrow().meta.is_none() {
                return Ok(v);
            }
        } else if !matches!(ov, Value::Str(_)) {
            return Err(self.index_error(&ov, o));
        }
        self.index(h, &ov, &kv)
    }
    #[inline(never)]
    fn eval_binary(&mut self, h: &mut dyn Host, f: &mut Frame, op: BinOp, a: &Expr, b: &Expr) -> Result<Value, Throw> {
        let x = self.eval(h, f, a)?;
        let y = self.eval(h, f, b)?;
        if let (Value::Num(p), Value::Num(q)) = (&x, &y) {
            match op {
                BinOp::Eq => return Ok(Value::Bool(p == q)),
                BinOp::Ne => return Ok(Value::Bool(p != q)),
                BinOp::Lt => return Ok(Value::Bool(p < q)),
                BinOp::Le => return Ok(Value::Bool(p <= q)),
                BinOp::Gt => return Ok(Value::Bool(p > q)),
                BinOp::Ge => return Ok(Value::Bool(p >= q)),
                BinOp::Concat | BinOp::BAnd | BinOp::BOr | BinOp::BXor | BinOp::Shl | BinOp::Shr => {}
                _ => return Ok(Value::Num(arith(op, *p, *q))),
            }
        }
        self.binary(h, op, x, y, a, b)
    }
    #[inline(never)]
    fn eval_unary(&mut self, h: &mut dyn Host, f: &mut Frame, op: UnOp, a: &Expr) -> Result<Value, Throw> {
        let x = self.eval(h, f, a)?;
        Ok(match op {
            UnOp::Not => Value::Bool(!x.truthy()),
            UnOp::Neg => match x.to_num() {
                Some(n) => Value::Num(-n),
                None => match self.metamethod(&x, "__unm") {
                    Some(mm) => self.call1(h, &mm, vec![x.clone(), x])?,
                    None => {
                        return Err(self.rt_error(format!(
                            "attempt to perform arithmetic on a {} value{}",
                            x.type_name(),
                            a.describe()
                        )));
                    }
                },
            },
            UnOp::BNot => match integer(&x) {
                Ok(i) => Value::Num(!i as f64),
                Err(why) => match self.metamethod(&x, "__bnot") {
                    Some(mm) => self.call1(h, &mm, vec![x.clone(), x])?,
                    None => return Err(self.rt_error(bitwise_error(why, &x, a))),
                },
            },
            UnOp::Len => match &x {
                Value::Str(s) => Value::Num(s.len() as f64),
                Value::Table(t) => match self.metamethod(&x, "__len") {
                    Some(mm) => self.call1(h, &mm, vec![x.clone()])?,
                    None => Value::Num(t.borrow().len() as f64),
                },
                _ => {
                    return Err(self.rt_error(format!(
                        "attempt to get length of a {} value{}",
                        x.type_name(),
                        a.describe()
                    )));
                }
            },
        })
    }
    #[inline(never)]
    fn eval_table(&mut self, h: &mut dyn Host, f: &mut Frame, items: &[(Option<Expr>, Expr)]) -> Result<Value, Throw> {
        let mut t = Table::new();
        let mut n = 0usize;
        for (i, (k, v)) in items.iter().enumerate() {
            match k {
                Some(k) => {
                    let kv = self.eval(h, f, k)?;
                    let vv = self.eval(h, f, v)?;
                    if let Err(e) = t.set(kv, vv) {
                        return Err(self.rt_error(e));
                    }
                }
                None if i + 1 == items.len() && v.is_multi() => {
                    let mut vals = Vec::new();
                    self.eval_multi(h, f, v, &mut vals)?;
                    for vv in vals {
                        n += 1;
                        let _ = t.set(Value::Num(n as f64), vv);
                    }
                }
                None => {
                    n += 1;
                    let vv = self.eval(h, f, v)?;
                    if let Err(e) = t.set(Value::Num(n as f64), vv) {
                        return Err(self.rt_error(e));
                    }
                }
            }
        }
        self.charge(items.len() * 16)?;
        Ok(Value::table(t))
    }

    fn eval_multi(&mut self, h: &mut dyn Host, f: &mut Frame, e: &Expr, out: &mut Vec<Value>) -> Result<(), Throw> {
        match e {
            Expr::Call(..) | Expr::Method(..) => out.extend(self.eval_call(h, f, e)?),
            Expr::Vararg => out.extend(f.varargs.iter().cloned()),
            _ => out.push(self.eval(h, f, e)?),
        }
        Ok(())
    }
    fn eval_list(&mut self, h: &mut dyn Host, f: &mut Frame, exprs: &[Expr]) -> Result<Vec<Value>, Throw> {
        let mut out = Vec::with_capacity(exprs.len());
        for (i, e) in exprs.iter().enumerate() {
            if i + 1 == exprs.len() {
                self.eval_multi(h, f, e, &mut out)?;
            } else {
                out.push(self.eval(h, f, e)?);
            }
        }
        Ok(out)
    }
    fn eval_list_n(&mut self, h: &mut dyn Host, f: &mut Frame, exprs: &[Expr], n: usize) -> Result<Vec<Value>, Throw> {
        let mut v = self.eval_list(h, f, exprs)?;
        v.resize(n, Value::Nil);
        Ok(v)
    }
    fn eval_call(&mut self, h: &mut dyn Host, f: &mut Frame, e: &Expr) -> Res {
        match e {
            Expr::Call(fe, args) => {
                let fv = self.eval(h, f, fe)?;
                let argv = self.eval_list(h, f, args)?;
                if !fv.is_callable() && self.metamethod(&fv, "__call").is_none() {
                    return Err(self.call_error(&fv, fe));
                }
                self.call(h, &fv, argv)
            }
            Expr::Method(oe, name, args) => {
                let ov = self.eval(h, f, oe)?;
                if !matches!(ov, Value::Table(_) | Value::Str(_)) {
                    return Err(self.index_error(&ov, oe));
                }
                let fv = self.index(h, &ov, &Value::Str(name.clone()))?;
                if !fv.is_callable() && self.metamethod(&fv, "__call").is_none() {
                    return Err(self.method_error(&fv, name));
                }
                let mut argv = Vec::with_capacity(args.len() + 1);
                argv.push(ov);
                argv.extend(self.eval_list(h, f, args)?);
                self.call(h, &fv, argv)
            }
            _ => Ok(vec![self.eval(h, f, e)?]),
        }
    }
    #[cold]
    #[inline(never)]
    fn call_error(&self, fv: &Value, fe: &Expr) -> Throw {
        self.rt_error(format!("attempt to call a {} value{}", fv.type_name(), fe.describe()))
    }
    #[cold]
    #[inline(never)]
    fn index_error(&self, ov: &Value, oe: &Expr) -> Throw {
        self.rt_error(format!("attempt to index a {} value{}", ov.type_name(), oe.describe()))
    }
    #[cold]
    #[inline(never)]
    fn method_error(&self, fv: &Value, name: &[u8]) -> Throw {
        self.rt_error(format!(
            "attempt to call a {} value (method '{}')",
            fv.type_name(),
            String::from_utf8_lossy(name)
        ))
    }

    #[inline(never)]
    fn binary(&mut self, h: &mut dyn Host, op: BinOp, x: Value, y: Value, a: &Expr, b: &Expr) -> Result<Value, Throw> {
        match op {
            BinOp::Eq => Ok(Value::Bool(self.equals(h, &x, &y)?)),
            BinOp::Ne => Ok(Value::Bool(!self.equals(h, &x, &y)?)),
            BinOp::Lt => Ok(Value::Bool(self.less(h, &x, &y, false)?)),
            BinOp::Le => Ok(Value::Bool(self.less(h, &x, &y, true)?)),
            BinOp::Gt => Ok(Value::Bool(self.less(h, &y, &x, false)?)),
            BinOp::Ge => Ok(Value::Bool(self.less(h, &y, &x, true)?)),
            BinOp::BAnd | BinOp::BOr | BinOp::BXor | BinOp::Shl | BinOp::Shr => {
                let (p, q) = (integer(&x), integer(&y));
                if let (Ok(p), Ok(q)) = (&p, &q) {
                    return Ok(Value::Num(bitwise(op, *p, *q) as f64));
                }
                let event = match op {
                    BinOp::BAnd => "__band",
                    BinOp::BOr => "__bor",
                    BinOp::BXor => "__bxor",
                    BinOp::Shl => "__shl",
                    _ => "__shr",
                };
                if let Some(mm) = self.metamethod(&x, event).or_else(|| self.metamethod(&y, event)) {
                    return self.call1(h, &mm, vec![x, y]);
                }
                Err(self.rt_error(match p {
                    Err(why) => bitwise_error(why, &x, a),
                    Ok(_) => bitwise_error(q.err().unwrap_or(false), &y, b),
                }))
            }
            BinOp::Concat => {
                let piece = |v: &Value| -> Option<Vec<u8>> {
                    match v {
                        Value::Str(s) => Some(s.to_vec()),
                        Value::Num(n) => Some(fmt_num(*n).into_bytes()),
                        _ => None,
                    }
                };
                if let (Some(mut p), Some(q)) = (piece(&x), piece(&y)) {
                    if p.len() + q.len() > MAX_STRING {
                        return Err(Throw::Fatal(format!(
                            "{}:{}: string is too long",
                            self.chunk, self.line
                        )));
                    }
                    self.charge(p.len() + q.len())?;
                    p.extend_from_slice(&q);
                    return Ok(Value::Str(p.into()));
                }
                if let Some(mm) = self
                    .metamethod(&x, "__concat")
                    .or_else(|| self.metamethod(&y, "__concat"))
                {
                    return self.call1(h, &mm, vec![x, y]);
                }
                let (bad, e) = if piece(&x).is_none() { (&x, a) } else { (&y, b) };
                Err(self.rt_error(format!(
                    "attempt to concatenate a {} value{}",
                    bad.type_name(),
                    e.describe()
                )))
            }
            _ => {
                if let (Some(p), Some(q)) = (x.to_num(), y.to_num()) {
                    return Ok(Value::Num(arith(op, p, q)));
                }
                let event = match op {
                    BinOp::Add => "__add",
                    BinOp::Sub => "__sub",
                    BinOp::Mul => "__mul",
                    BinOp::Div => "__div",
                    BinOp::IDiv => "__idiv",
                    BinOp::Mod => "__mod",
                    _ => "__pow",
                };
                if let Some(mm) = self.metamethod(&x, event).or_else(|| self.metamethod(&y, event)) {
                    return self.call1(h, &mm, vec![x, y]);
                }
                let (bad, e) = if x.to_num().is_none() { (&x, a) } else { (&y, b) };
                let what = if matches!(bad, Value::Str(_)) {
                    format!(
                        "attempt to perform arithmetic on a string value{} (not a number)",
                        e.describe()
                    )
                } else {
                    format!(
                        "attempt to perform arithmetic on a {} value{}",
                        bad.type_name(),
                        e.describe()
                    )
                };
                Err(self.rt_error(what))
            }
        }
    }

    pub fn equals(&mut self, h: &mut dyn Host, x: &Value, y: &Value) -> Result<bool, Throw> {
        if x.raw_eq(y) {
            return Ok(true);
        }
        if let (Value::Table(_), Value::Table(_)) = (x, y)
            && let Some(mm) = self.metamethod(x, "__eq").or_else(|| self.metamethod(y, "__eq"))
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
                let event = if or_equal { "__le" } else { "__lt" };
                if let Some(mm) = self.metamethod(x, event).or_else(|| self.metamethod(y, event)) {
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
        if let Some(mm) = self.metamethod(v, "__tostring") {
            let r = self.call1(h, &mm, vec![v.clone()])?;
            return match r {
                Value::Str(s) => Ok(String::from_utf8_lossy(&s).into_owned()),
                Value::Num(n) => Ok(fmt_num(n)),
                _ => Err(self.rt_error("'__tostring' must return a string")),
            };
        }
        Ok(v.plain_text())
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

fn bitwise_error(number: bool, v: &Value, e: &Expr) -> String {
    if number {
        format!("number{} has no integer representation", e.describe())
    } else {
        format!(
            "attempt to perform bitwise operation on a {} value{}",
            v.type_name(),
            e.describe()
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
