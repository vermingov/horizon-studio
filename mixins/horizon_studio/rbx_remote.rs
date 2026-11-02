// The game's packets as Roblox remotes (hstudio_rbx_remote): one RemoteEvent
// (a RemoteFunction where the server answers) per play packet in
// ReplicatedStorage.Minecraft, named after vanilla's name for the packet
// (move_player_pos is MovePlayerPos). A packet's fields are one table, a
// field per field. The game's own serverbound packets reach scripts as
// `remote:FireServer(fields)` through `__namecall`, so remote spies see them
// and a hook that doesn't call the original stops the packet; a script's
// FireServer sends one. Clientbound packets fire the remote's OnClientEvent.
// The game's side (the packets themselves) is hstudio_bridge's.

use super::hstudio::Owner;
use super::hstudio_lua::{Lua, LuaBuffer, NativeKind, Res, Table, Thread, Throw, Value};
use super::hstudio_rbx::{Env, Id, Rbx};
use super::hstudio_rbx_types::Vector3;
use super::hstudio_rbx_values::{as_v3, v3};
use std::collections::HashMap;
use std::rc::Rc;

/// A field's value, as the remotes carry it.
#[derive(Clone, Debug, PartialEq)]
pub enum Field {
    Nil,
    Bool(bool),
    Num(f64),
    /// A whole number: a Lua number while one holds it exactly (to 2^53),
    /// else its digits as a string.
    Int(i128),
    /// Text or bytes (a Lua string).
    Str(Vec<u8>),
    /// A position or a vector: a Vector3.
    Vec3([f64; 3]),
    List(Vec<Field>),
    /// Named fields (a table); a name that is a whole number is a number key.
    Map(Vec<(String, Field)>),
}

impl Field {
    pub fn text(s: &str) -> Field {
        Field::Str(s.as_bytes().to_vec())
    }
}

/// A play packet: its id, vanilla's name for it, its fields and its payload.
#[derive(Clone, Debug, PartialEq)]
pub struct Packet {
    pub id: i32,
    /// Vanilla's name ("move_player_pos").
    pub name: String,
    /// Its fields by name, in the packet's order.
    pub fields: Vec<(String, Field)>,
    /// The fields are the whole packet: FireServer takes them back. Without,
    /// they were read from the game's form of it, to look at, and `raw` is
    /// what is sent.
    pub mapped: bool,
    /// The payload after the id, as the server gets it.
    pub raw: Option<Vec<u8>>,
}

/// What becomes of one of the game's packets after the scripts saw it.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    /// As it was.
    Send,
    /// Not at all: a hook returned without calling the original.
    Block,
    /// This instead: a hook called the original with other arguments.
    Replace(Packet),
}

/// The Roblox name of a packet: vanilla's in PascalCase.
pub fn remote_name(packet: &str) -> String {
    let mut out = String::with_capacity(packet.len());
    let mut up = true;
    for c in packet.chars() {
        if c == '_' || c == '/' || c == ':' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// Serverbound packets the server answers, as RemoteFunctions: the request,
/// its answer (clientbound) and the field both carry the same value in.
const ANSWERED: &[(&str, &str, &str)] = &[
    ("command_suggestion", "command_suggestions", "id"),
    ("block_entity_tag_query", "tag_query", "transaction"),
    ("entity_tag_query", "tag_query", "transaction"),
    ("ping_request", "pong_response", "time"),
];

/// Packets only the game sends, as it sends them: one wrong or missing (a
/// keep-alive's id, the chat's acknowledgements or session, the switch back
/// to configuration) disconnects the player. Hooks see them but can't stop
/// or change them, and scripts can't fire them.
pub const GAME_ONLY: &[&str] = &[
    "keep_alive",
    "configuration_acknowledged",
    "chat_ack",
    "chat_session_update",
];

/// The most packets scripts send in one frame (a loop can't flood the server).
pub const MAX_PER_FRAME: u32 = 256;

/// The game's packet in a hook: whether it called the original, with what.
pub struct Pending {
    pub remote: Id,
    pub passed: Option<Packet>,
}

/// An InvokeServer waiting for the server's answer.
pub struct Invoke {
    pub owner: Owner,
    pub thread: Rc<Thread>,
    pub answer: String,
    pub key: Field,
}

/// The remotes and what goes through them.
#[derive(Default)]
pub struct Remotes {
    /// ReplicatedStorage.Minecraft, once the game named its packets.
    pub folder: Option<Id>,
    /// Each remote, by its packet name (vanilla's), and each remote's packet
    /// names and ids, serverbound and clientbound.
    pub by_packet: HashMap<String, Id>,
    pub serverbound: HashMap<Id, (i32, String)>,
    pub clientbound: HashMap<Id, (i32, String)>,
    pub pending: Option<Pending>,
    pub invokes: Vec<Invoke>,
    /// Sent this frame (at `frame_at`, the layer's clock).
    pub sent: u32,
    pub frame_at: f64,
    /// The last error a hook of the game's packets had, printed once (a
    /// broken hook fails on every packet).
    pub last_error: Option<String>,
}

impl Remotes {
    /// The remote of a packet.
    pub fn remote(&self, packet: &str) -> Option<Id> {
        self.by_packet.get(packet).copied()
    }
    /// Whether `id` is one of the packets' remotes.
    pub fn is_remote(&self, id: Id) -> bool {
        self.serverbound.contains_key(&id) || self.clientbound.contains_key(&id)
    }
}

/// Makes ReplicatedStorage.Minecraft and its remotes, once: `names` are the
/// game's play packets by id, serverbound then clientbound.
pub fn ensure(rbx: &mut Rbx, names: (&[&str], &[&str])) {
    if rbx.remotes.folder.is_some() || (names.0.is_empty() && names.1.is_empty()) {
        return;
    }
    let Some(rs) = rbx.service("ReplicatedStorage") else {
        return;
    };
    let folder = rbx.make("Folder", None);
    rbx.inst_mut(folder).name = "Minecraft".into();
    let _ = rbx.set_parent(folder, Some(rs));
    rbx.remotes.folder = Some(folder);
    let mut by_name: HashMap<String, Id> = HashMap::new();
    for (bound, list) in [(true, names.0), (false, names.1)] {
        for (i, name) in list.iter().enumerate() {
            let rname = remote_name(name);
            let id = match by_name.get(&rname) {
                Some(&id) => id,
                None => {
                    let answered = bound && ANSWERED.iter().any(|(q, _, _)| q == name);
                    let id = rbx.make(if answered { "RemoteFunction" } else { "RemoteEvent" }, None);
                    rbx.inst_mut(id).name = rname.clone();
                    let _ = rbx.set_parent(id, Some(folder));
                    by_name.insert(rname, id);
                    id
                }
            };
            let entry = (i as i32, (*name).to_owned());
            if bound {
                rbx.remotes.serverbound.insert(id, entry);
            } else {
                rbx.remotes.clientbound.insert(id, entry);
            }
            rbx.remotes.by_packet.entry((*name).to_owned()).or_insert(id);
        }
    }
    // The answers' remotes are found by their packets' names too.
    for list in [names.0, names.1] {
        for name in list {
            if let Some(&id) = by_name.get(&remote_name(name)) {
                rbx.remotes.by_packet.insert((*name).to_owned(), id);
            }
        }
    }
}

/// What a `__namecall` hook is, if a script put one there: the hooked
/// `__namecall` of Instances (hookmetamethod, or a function set in the
/// metatable), else FireServer when hookfunction hooked it. The game's
/// packets go through Lua only then.
pub fn hook_entry(rbx: &Rbx) -> Option<Value> {
    let nc = rbx.inst_meta().borrow().get_str("__namecall");
    if hooked_namecall(&nc) {
        return Some(nc);
    }
    let fire = rbx.method_value("FireServer")?;
    match &fire {
        Value::Native(n) if n.redirect().is_some() => Some(fire),
        _ => None,
    }
}

/// Whether `__namecall` is a script's: anything but the layer's own (or a
/// copy of it, which restoring a hookmetamethod leaves).
fn hooked_namecall(v: &Value) -> bool {
    let mut f = v.clone();
    for _ in 0..8 {
        match f {
            Value::Native(n) => match n.redirect() {
                Some(to) => f = to,
                None => return !matches!(n.kind, NativeKind::Namecall),
            },
            Value::Nil => return false,
            _ => return true,
        }
    }
    true
}

/// The arguments a packet's remote gets: its fields, and its payload
/// unless the fields are the whole packet.
pub fn packet_args(p: &Packet) -> Vec<Value> {
    let mut t = Table::new();
    for (k, v) in &p.fields {
        set_key(&mut t, k, to_lua(v));
    }
    let mut args = vec![Value::table(t)];
    if !p.mapped
        && let Some(raw) = &p.raw
    {
        args.push(Value::bytes(raw));
    }
    args
}

fn set_key(t: &mut Table, k: &str, v: Value) {
    if v.is_nil() {
        return;
    }
    match k.parse::<i64>() {
        Ok(n) if n.to_string() == k => {
            let _ = t.set(Value::Num(n as f64), v);
        }
        _ => t.set_str(k, v),
    }
}

/// A field as Lua has it.
pub fn to_lua(f: &Field) -> Value {
    match f {
        Field::Nil => Value::Nil,
        Field::Bool(b) => Value::Bool(*b),
        Field::Num(n) => Value::Num(*n),
        Field::Int(n) => {
            if n.unsigned_abs() <= 1 << 53 {
                Value::Num(*n as f64)
            } else {
                Value::str(&n.to_string())
            }
        }
        Field::Str(s) => Value::bytes(s),
        Field::Vec3([x, y, z]) => v3(Vector3 { x: *x, y: *y, z: *z }),
        Field::List(items) => Value::table(Table::from_list(items.iter().map(to_lua).collect())),
        Field::Map(entries) => {
            let mut t = Table::new();
            for (k, v) in entries {
                set_key(&mut t, k, to_lua(v));
            }
            Value::table(t)
        }
    }
}

/// The deepest table a field may be, and the most values in one.
const MAX_DEPTH: usize = 24;
const MAX_VALUES: usize = 1 << 16;

/// A Lua value as a field: numbers, booleans, strings, buffers, Vector3s
/// and tables of them.
pub fn from_lua(v: &Value) -> Result<Field, String> {
    let mut budget = MAX_VALUES;
    field_of(v, 0, &mut budget)
}

fn field_of(v: &Value, depth: usize, budget: &mut usize) -> Result<Field, String> {
    if *budget == 0 {
        return Err("too many values".into());
    }
    *budget -= 1;
    Ok(match v {
        Value::Nil => Field::Nil,
        Value::Bool(b) => Field::Bool(*b),
        Value::Num(n) => Field::Num(*n),
        Value::Str(s) => Field::Str(s.to_vec()),
        Value::Table(t) => {
            if depth >= MAX_DEPTH {
                return Err("a table too deep (or one inside itself)".into());
            }
            let entries = t.borrow().entries();
            let n = t.borrow().len();
            if entries.len() == n && n > 0 {
                let items = t.borrow().list();
                Field::List(
                    items
                        .iter()
                        .map(|x| field_of(x, depth + 1, budget))
                        .collect::<Result<_, _>>()?,
                )
            } else {
                let mut out = Vec::with_capacity(entries.len());
                for (k, x) in &entries {
                    let key = match k {
                        Value::Str(s) => String::from_utf8_lossy(s).into_owned(),
                        Value::Num(n) if n.fract() == 0.0 => format!("{}", *n as i64),
                        other => return Err(format!("a table with a {} key", other.type_name())),
                    };
                    out.push((key, field_of(x, depth + 1, budget)?));
                }
                out.sort_by(|a, b| a.0.cmp(&b.0));
                Field::Map(out)
            }
        }
        Value::Userdata(u) => {
            if let Some(p) = as_v3(v) {
                return Ok(Field::Vec3([p.x, p.y, p.z]));
            }
            if let Some(b) = u.data.borrow().downcast_ref::<LuaBuffer>() {
                return Ok(Field::Str(b.0.clone()));
            }
            let name = u.type_name().unwrap_or_else(|| "userdata".into());
            return Err(format!(
                "a{} {name} can't be sent",
                if name.starts_with(['A', 'E', 'I', 'O', 'U']) {
                    "n"
                } else {
                    ""
                }
            ));
        }
        other => return Err(format!("a {} can't be sent", other.type_name())),
    })
}

/// FireServer's arguments as a packet of the remote's: a fields table
/// (the packet's fields), or the payload (a string or buffer) after it or
/// alone.
pub fn args_packet(rbx: &Rbx, remote: Id, args: &[Value]) -> Result<Packet, String> {
    let Some((id, name)) = rbx.remotes.serverbound.get(&remote).cloned() else {
        return Err(format!(
            "{} is a packet of the server's: the client doesn't send it",
            rbx.inst(remote).name
        ));
    };
    let bytes = |v: &Value| -> Option<Vec<u8>> {
        match v {
            Value::Str(s) => Some(s.to_vec()),
            Value::Userdata(u) => u.data.borrow().downcast_ref::<LuaBuffer>().map(|b| b.0.clone()),
            _ => None,
        }
    };
    let first = args.first().cloned().unwrap_or_default();
    let (fields, raw) = match (&first, args.get(1)) {
        (Value::Table(_), rest) => {
            let fields = match from_lua(&first)? {
                Field::Map(m) => m,
                Field::List(l) if l.is_empty() => Vec::new(),
                _ => return Err("the fields are a table of names (fields.x, not fields[1])".into()),
            };
            let raw = match rest {
                None | Some(Value::Nil) => None,
                Some(v) => Some(
                    bytes(v).ok_or_else(|| format!("the payload is a string or buffer, not a {}", v.type_name()))?,
                ),
            };
            (fields, raw)
        }
        (Value::Nil, None) => (Vec::new(), None),
        (v, _) => match bytes(v) {
            Some(b) => (Vec::new(), Some(b)),
            None => {
                return Err(format!(
                    "FireServer takes the packet's fields as a table (or its payload as a string or buffer), not a {}",
                    v.type_name()
                ));
            }
        },
    };
    let fields = fields.into_iter().filter(|(_, f)| *f != Field::Nil).collect();
    Ok(Packet {
        id,
        name,
        fields,
        mapped: raw.is_none(),
        raw,
    })
}

/// A field of a packet, by name.
pub fn field<'a>(p: &'a Packet, name: &str) -> Option<&'a Field> {
    p.fields.iter().find(|(k, _)| k == name).map(|(_, v)| v)
}

/// For a RemoteFunction's packet: the answer's packet and the value that
/// matches it to the request.
pub fn answer_of(p: &Packet) -> Option<(String, Field)> {
    let (_, answer, key) = ANSWERED.iter().find(|(q, _, _)| *q == p.name)?;
    let v = field(p, key)?.clone();
    Some(((*answer).to_owned(), norm(&v)))
}

/// A value as a script gave it or the game read it, alike (whole numbers
/// that fit are numbers).
fn norm(v: &Field) -> Field {
    match v {
        Field::Int(n) if n.unsigned_abs() <= 1 << 53 => Field::Num(*n as f64),
        Field::Int(n) => Field::Str(n.to_string().into_bytes()),
        other => other.clone(),
    }
}

/// Whether a packet answers an InvokeServer: its threads to resume with its fields.
pub fn take_answers(rbx: &mut Rbx, p: &Packet) -> Vec<(Owner, Rc<Thread>)> {
    let mut out = Vec::new();
    let key_of = |answer: &str| ANSWERED.iter().find(|(_, a, _)| *a == answer).map(|(_, _, k)| *k);
    let Some(key) = key_of(&p.name) else {
        return out;
    };
    let Some(v) = field(p, key).map(norm) else {
        return out;
    };
    let mut i = 0;
    while i < rbx.remotes.invokes.len() {
        let w = &rbx.remotes.invokes[i];
        if w.answer == p.name && w.key == v {
            let w = rbx.remotes.invokes.remove(i);
            out.push((w.owner, w.thread));
        } else {
            i += 1;
        }
    }
    out
}

/// Whether scripts listen to a clientbound packet: its remote's
/// OnClientEvent has a handler or a waiter, or an InvokeServer waits for it.
pub fn heard(rbx: &Rbx, packet: &str) -> bool {
    if rbx.remotes.invokes.iter().any(|w| w.answer == packet) {
        return true;
    }
    let Some(id) = rbx.remotes.remote(packet) else {
        return false;
    };
    rbx.listens(id, "OnClientEvent")
}

// ----------------------------------------------------------------- Debug

/// Words that name a position or a vector: three numbers under one are a Vector3.
const VECTOR_WORDS: &[&str] = &[
    "pos", "position", "location", "delta", "velocity", "movement", "cursor", "origin", "center", "offset", "motion",
    "target", "block", "start", "end", "size",
];

/// The fields of a packet from its Rust `Debug` form, `Variant(Struct { a:
/// 1, b: [1.0, 2.0, 3.0] })`: structs are tables (a nested one names its
/// type in `type`), `Some(x)` is x and `None` nil, a unit variant its name,
/// lists are lists. None where the text isn't Debug's (or is an Unknown
/// packet's).
pub fn debug_fields(text: &str) -> Option<Vec<(String, Field)>> {
    let mut p = Parser {
        s: text.as_bytes(),
        i: 0,
    };
    let v = p.value(None, 0)?;
    p.ws();
    if p.i != p.s.len() {
        return None;
    }
    // The enum's variant, around the packet's struct.
    let inner = match v {
        Field::Map(mut m) => {
            let ty = m.iter().position(|(k, _)| k == "type");
            let ty = ty.map(|i| m.remove(i).1);
            if ty == Some(Field::text("Unknown")) {
                return None;
            }
            if m.len() == 1 && m[0].0 == "1" {
                m.remove(0).1
            } else {
                Field::Map(m)
            }
        }
        other => other,
    };
    Some(match inner {
        Field::Map(mut m) => {
            m.retain(|(k, _)| k != "type");
            m
        }
        // A unit struct (no fields).
        Field::Str(_) => Vec::new(),
        other => vec![("value".into(), other)],
    })
}

/// One value from its Rust `Debug` form, as `debug_fields` reads a field.
pub fn debug_value(text: &str) -> Option<Field> {
    let mut p = Parser {
        s: text.as_bytes(),
        i: 0,
    };
    let v = p.value(None, 0)?;
    p.ws();
    (p.i == p.s.len()).then_some(v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.i).copied()
    }
    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn ident(&mut self) -> Option<String> {
        self.ws();
        let start = self.i;
        while self.i < self.s.len() {
            let c = self.s[self.i];
            if c.is_ascii_alphanumeric() || c == b'_' {
                self.i += 1;
            } else if c == b':' && self.s.get(self.i + 1) == Some(&b':') {
                self.i += 2;
            } else {
                break;
            }
        }
        (self.i > start).then(|| String::from_utf8_lossy(&self.s[start..self.i]).into_owned())
    }
    fn value(&mut self, key: Option<&str>, depth: usize) -> Option<Field> {
        if depth > 64 {
            return None;
        }
        match self.peek()? {
            b'"' => self.string().map(Field::Str),
            b'\'' => self.char_lit(),
            b'[' => {
                self.i += 1;
                let items = self.list(b']', depth)?;
                let vector = key.is_some_and(|k| VECTOR_WORDS.iter().any(|w| k == *w || k.ends_with(&format!("_{w}"))));
                if vector && items.len() == 3 {
                    let num = |f: &Field| match f {
                        Field::Num(n) => Some(*n),
                        Field::Int(n) => Some(*n as f64),
                        _ => None,
                    };
                    if let (Some(x), Some(y), Some(z)) = (num(&items[0]), num(&items[1]), num(&items[2])) {
                        return Some(Field::Vec3([x, y, z]));
                    }
                }
                Some(Field::List(items))
            }
            b'(' => {
                self.i += 1;
                Some(Field::List(self.list(b')', depth)?))
            }
            b'{' => {
                self.i += 1;
                self.map(depth)
            }
            c if c == b'-' || c.is_ascii_digit() => self.number(),
            _ => {
                let name = self.ident()?;
                match name.as_str() {
                    "true" => return Some(Field::Bool(true)),
                    "false" => return Some(Field::Bool(false)),
                    "None" => return Some(Field::Nil),
                    "NaN" => return Some(Field::Num(f64::NAN)),
                    "inf" => return Some(Field::Num(f64::INFINITY)),
                    _ => {}
                }
                let short = name.rsplit("::").next().unwrap_or(&name).to_owned();
                if short == "Some" && self.eat(b'(') {
                    // Some(x) is x, read as the field it is.
                    let v = self.value(key, depth + 1)?;
                    return self.eat(b')').then_some(v);
                }
                match self.peek() {
                    Some(b'(') => {
                        self.i += 1;
                        let items = self.list(b')', depth)?;
                        let mut m = vec![("type".to_owned(), Field::text(&short))];
                        for (i, v) in items.into_iter().enumerate() {
                            m.push(((i + 1).to_string(), v));
                        }
                        Some(Field::Map(m))
                    }
                    Some(b'{') => {
                        self.i += 1;
                        let Field::Map(mut m) = self.map(depth)? else {
                            return None;
                        };
                        m.insert(0, ("type".to_owned(), Field::text(&short)));
                        Some(Field::Map(m))
                    }
                    _ => Some(Field::text(&short)),
                }
            }
        }
    }
    fn list(&mut self, close: u8, depth: usize) -> Option<Vec<Field>> {
        let mut out = Vec::new();
        loop {
            if self.eat(close) {
                return Some(out);
            }
            out.push(self.value(None, depth + 1)?);
            if !self.eat(b',') {
                return self.eat(close).then_some(out);
            }
        }
    }
    /// `{ name: value, .. }` (a struct's fields), `{k: v}` (a map) or `{a, b}` (a set).
    fn map(&mut self, depth: usize) -> Option<Field> {
        let mut fields = Vec::new();
        let mut set = Vec::new();
        loop {
            if self.eat(b'}') {
                break;
            }
            if self.peek() == Some(b'.') {
                // A non-exhaustive struct's `..`.
                while self.peek() == Some(b'.') {
                    self.i += 1;
                }
                continue;
            }
            let save = self.i;
            let key = match self.ident() {
                Some(k) if self.peek() == Some(b':') && self.s.get(self.i + 1) != Some(&b':') => {
                    self.i += 1;
                    k
                }
                _ => {
                    self.i = save;
                    let k = self.value(None, depth + 1)?;
                    if self.eat(b':') {
                        match k {
                            Field::Str(s) => String::from_utf8_lossy(&s).into_owned(),
                            Field::Int(n) => n.to_string(),
                            Field::Num(n) => n.to_string(),
                            _ => return None,
                        }
                    } else {
                        set.push(k);
                        if !self.eat(b',') {
                            self.eat(b'}').then_some(())?;
                            break;
                        }
                        continue;
                    }
                }
            };
            let v = self.value(Some(&key), depth + 1)?;
            fields.push((key, v));
            if !self.eat(b',') {
                self.eat(b'}').then_some(())?;
                break;
            }
        }
        if fields.is_empty() && !set.is_empty() {
            return Some(Field::List(set));
        }
        fields.retain(|(_, v)| *v != Field::Nil);
        Some(Field::Map(fields))
    }
    fn number(&mut self) -> Option<Field> {
        self.ws();
        let start = self.i;
        if self.s.get(self.i) == Some(&b'-') {
            self.i += 1;
            if self.s[self.i..].starts_with(b"inf") {
                self.i += 3;
                return Some(Field::Num(f64::NEG_INFINITY));
            }
        }
        while self.i < self.s.len() && matches!(self.s[self.i], b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-' | b'_') {
            // A minus inside is an exponent's only.
            if matches!(self.s[self.i], b'-' | b'+') && !matches!(self.s[self.i - 1], b'e' | b'E') {
                break;
            }
            self.i += 1;
        }
        let text = std::str::from_utf8(&self.s[start..self.i]).ok()?.replace('_', "");
        if let Ok(n) = text.parse::<i128>() {
            return Some(Field::Int(n));
        }
        // A whole number past i128 (a u128's): its digits.
        if !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()) {
            return Some(Field::Str(text.into_bytes()));
        }
        text.parse::<f64>().ok().map(Field::Num)
    }
    fn string(&mut self) -> Option<Vec<u8>> {
        self.i += 1;
        let mut out = Vec::new();
        while self.i < self.s.len() {
            let c = self.s[self.i];
            self.i += 1;
            match c {
                b'"' => return Some(out),
                b'\\' => self.escape(&mut out)?,
                _ => out.push(c),
            }
        }
        None
    }
    fn char_lit(&mut self) -> Option<Field> {
        self.i += 1;
        let mut out = Vec::new();
        while self.i < self.s.len() {
            let c = self.s[self.i];
            self.i += 1;
            match c {
                b'\'' => return Some(Field::Str(out)),
                b'\\' => self.escape(&mut out)?,
                _ => out.push(c),
            }
        }
        None
    }
    fn escape(&mut self, out: &mut Vec<u8>) -> Option<()> {
        let c = *self.s.get(self.i)?;
        self.i += 1;
        match c {
            b'n' => out.push(b'\n'),
            b'r' => out.push(b'\r'),
            b't' => out.push(b'\t'),
            b'0' => out.push(0),
            b'\\' | b'"' | b'\'' => out.push(c),
            b'u' => {
                // \u{1f600}
                if self.s.get(self.i) != Some(&b'{') {
                    return None;
                }
                let end = self.s[self.i..].iter().position(|&b| b == b'}')? + self.i;
                let hex = std::str::from_utf8(&self.s[self.i + 1..end]).ok()?;
                let ch = char::from_u32(u32::from_str_radix(hex, 16).ok()?)?;
                let mut buf = [0; 4];
                out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                self.i = end + 1;
            }
            _ => return None,
        }
        Some(())
    }
}

// ----------------------------------------------------------------- calls

/// `remote:FireServer(...)` on a packet's remote: the game's own packet
/// going through a hook (the original called), or a script's, sent.
pub fn fire_server(env: &mut dyn Env, lua: &mut Lua, remote: Id, args: &[Value]) -> Res {
    let rbx = env.rbx();
    let p = args_packet(rbx, remote, args).map_err(|e| lua.rt_error(format!("FireServer: {e}")))?;
    if let Some(pending) = rbx.remotes.pending.as_mut()
        && pending.remote == remote
        && pending.passed.is_none()
    {
        pending.passed = Some(p);
        return Ok(Vec::new());
    }
    send(env, lua, &p, "FireServer")?;
    Ok(Vec::new())
}

/// `remote:InvokeServer(...)`: the packet sent, then the thread waits for
/// the server's answer (its fields).
pub fn invoke_server(env: &mut dyn Env, lua: &mut Lua, remote: Id, args: &[Value]) -> Res {
    let rbx = env.rbx();
    let p = args_packet(rbx, remote, args).map_err(|e| lua.rt_error(format!("InvokeServer: {e}")))?;
    if let Some(pending) = rbx.remotes.pending.as_mut()
        && pending.remote == remote
        && pending.passed.is_none()
    {
        pending.passed = Some(p);
        return Ok(Vec::new());
    }
    let Some((answer, key)) = answer_of(&p) else {
        send(env, lua, &p, "InvokeServer")?;
        return Ok(Vec::new());
    };
    if !lua.is_yieldable() {
        return Err(lua.rt_error(
            "InvokeServer can't wait for the server here: this code isn't in a thread that can wait (run it with task.spawn)",
        ));
    }
    send(env, lua, &p, "InvokeServer")?;
    let owner = env.owner();
    env.rbx().remotes.invokes.push(Invoke {
        owner,
        thread: lua.current_thread(),
        answer,
        key,
    });
    Err(Throw::Yield(Vec::new()))
}

/// A script's packet to the server. Chat goes through the game's chat
/// (which signs it, as the server checks); the rest as packets.
fn send(env: &mut dyn Env, lua: &mut Lua, p: &Packet, what: &str) -> Result<(), Throw> {
    let now = env.now();
    let rbx = env.rbx();
    if rbx.remotes.frame_at != now {
        rbx.remotes.frame_at = now;
        rbx.remotes.sent = 0;
    }
    if rbx.remotes.sent >= MAX_PER_FRAME {
        return Err(lua.rt_error(format!("{what}: too many packets this frame (at most {MAX_PER_FRAME})")));
    }
    rbx.remotes.sent += 1;
    if GAME_ONLY.contains(&p.name.as_str()) {
        return Err(lua.rt_error(format!(
            "{what}: {}: only the game sends it (a wrong one disconnects the player)",
            remote_name(&p.name)
        )));
    }
    if let Some(line) = chat_line(p).map_err(|e| lua.rt_error(format!("{what}: {e}")))? {
        env.game().send(&line);
        return Ok(());
    }
    env.game()
        .send_packet(p)
        .map_err(|e| lua.rt_error(format!("{what}: {}: {e}", remote_name(&p.name))))
}

/// What a chat packet says, as the chat sends it (a command with its
/// slash); None for other packets.
pub fn chat_line(p: &Packet) -> Result<Option<String>, String> {
    let (key, slash) = match p.name.as_str() {
        "chat" => ("message", ""),
        "chat_command" | "chat_command_signed" => ("command", "/"),
        _ => return Ok(None),
    };
    let text = match field(p, key) {
        Some(Field::Str(s)) => String::from_utf8_lossy(s).into_owned(),
        Some(_) => return Err(format!("{key} is a string")),
        None if p.raw.is_some() => return Err("chat is signed: give its fields, not its payload".into()),
        None => return Err(format!("{key} is missing")),
    };
    let text = text.trim();
    if text.is_empty() {
        return Err(format!("{key} is empty"));
    }
    if text.chars().any(char::is_control) {
        return Err(format!("{key} has control characters"));
    }
    if slash.is_empty() && text.starts_with('/') {
        return Err("a chat message starting with / is a command: fire ChatCommand".into());
    }
    let max = if slash.is_empty() { 256 } else { 32767 };
    if text.chars().count() > max {
        return Err(format!("{key} is longer than {max} characters"));
    }
    Ok(Some(format!("{slash}{text}")))
}
