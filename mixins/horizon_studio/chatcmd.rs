// Script commands in the chat: what a command declares besides its name and
// function (`command(name, fn, {description, args, suggest})`), the shape the
// chat's command tree gets of it, and the rules its completions follow. The
// studio keeps each mod's commands and answers the chat's completion
// requests (studio.rs); hstudio_cmdtree puts the shapes into the chat's tree.
// Nothing here knows the game.

use super::hstudio_lua::{Lua, Table, Throw, Value, bad_arg};
use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

/// The most arguments one command declares.
pub const MAX_ARGS: usize = 16;
/// The most options one argument lists, and the most completions one answer gives.
pub const MAX_COMPLETIONS: usize = 256;
/// The longest completion text and tooltip, in bytes: a longer text is left
/// out, a longer tooltip is cut.
pub const MAX_COMPLETION_BYTES: usize = 256;
/// The most commands one mod, or the console, has at once.
pub const MAX_COMMANDS: usize = 256;
/// The most entries of a suggest function's list that are looked at.
const MAX_SCANNED: usize = 4096;

/// What an argument takes, and so how the chat reads, colours and completes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArgKind {
    /// One word: anything but a space.
    Word,
    /// The rest of the line.
    Text,
    Int {
        min: i32,
        max: i32,
    },
    Number {
        min: f64,
        max: f64,
    },
    Bool,
    /// A player's name or a selector.
    Player,
    /// A block position: three coordinates, `~ ~ ~` or numbers.
    Pos,
    /// An item id, as /give takes it.
    Item,
    /// A block id, as /setblock takes it.
    Block,
}

impl ArgKind {
    /// The kinds by their names, as an error lists them.
    pub const NAMES: &'static str = "word, text, int, number, bool, player, pos, item or block";

    /// A kind by its name, with the widest bounds for int and number.
    pub fn named(name: &str) -> Option<ArgKind> {
        Some(match name {
            "word" => ArgKind::Word,
            "text" => ArgKind::Text,
            "int" => ArgKind::Int {
                min: i32::MIN,
                max: i32::MAX,
            },
            "number" => ArgKind::Number {
                min: f64::MIN,
                max: f64::MAX,
            },
            "bool" => ArgKind::Bool,
            "player" => ArgKind::Player,
            "pos" => ArgKind::Pos,
            "item" => ArgKind::Item,
            "block" => ArgKind::Block,
            _ => return None,
        })
    }

    /// How many words of a line it reads: three for pos, the rest of the
    /// line for text (None), one for the others.
    pub fn words(self) -> Option<usize> {
        match self {
            ArgKind::Text => None,
            ArgKind::Pos => Some(3),
            _ => Some(1),
        }
    }
}

/// A completion: its text, and the tooltip the chat shows over it.
pub type Completion = (String, Option<String>);

/// An answer to the chat's completion request: the completions for the text
/// at the end of its line, and where that text starts and how long it is, in
/// UTF-16 units of the line (as a server's answer counts them).
#[derive(Clone, Debug, PartialEq)]
pub struct Completions {
    pub start: usize,
    pub len: usize,
    pub list: Vec<Completion>,
}

/// A function that completes, and the last error it raised (logged once,
/// until another comes or it works again).
#[derive(Clone)]
pub struct Suggest {
    pub f: Value,
    pub failing: Option<String>,
}

impl Suggest {
    fn new(f: Value) -> Suggest {
        Suggest { f, failing: None }
    }
}

/// An argument a command declares.
#[derive(Clone)]
pub struct CommandArg {
    pub name: String,
    pub kind: ArgKind,
    pub optional: bool,
    /// Its fixed completions.
    pub options: Vec<Completion>,
    /// Its function: `suggest(rest, words, name)` returns more.
    pub suggest: Option<Suggest>,
}

/// A command as the chat's command tree has it: its name and its arguments.
#[derive(Clone, Debug, PartialEq)]
pub struct ChatCommand {
    pub name: String,
    pub args: Vec<ChatArg>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChatArg {
    pub name: String,
    pub kind: ArgKind,
    pub optional: bool,
    /// Its completions come from the studio (options or a function), so the
    /// chat asks for them instead of using the game's own.
    pub asks: bool,
}

impl ChatCommand {
    /// Whether the command may end before argument `n`: every argument
    /// from `n` on is optional.
    pub fn can_end(&self, n: usize) -> bool {
        self.args.iter().skip(n).all(|a| a.optional)
    }
}

/// The chat's shape of a command. Without declared arguments it takes any
/// text after its name, as one optional text argument `args` (the shape
/// Horizon gives native mods' commands), which asks when the command has a
/// suggest function.
pub fn shape(name: &str, args: Option<&[CommandArg]>, suggest: bool) -> ChatCommand {
    let args = match args {
        None => vec![ChatArg {
            name: "args".into(),
            kind: ArgKind::Text,
            optional: true,
            asks: suggest,
        }],
        Some(list) => list
            .iter()
            .map(|a| ChatArg {
                name: a.name.clone(),
                kind: a.kind,
                optional: a.optional,
                asks: !a.options.is_empty() || a.suggest.is_some(),
            })
            .collect(),
    };
    ChatCommand {
        name: name.to_owned(),
        args,
    }
}

/// Declared arguments as a usage line, the way Brigadier writes one:
/// `<name> [<count>]`.
pub fn usage(args: &[CommandArg]) -> String {
    let each: Vec<String> = args
        .iter()
        .map(|a| {
            if a.optional {
                format!("[<{}>]", a.name)
            } else {
                format!("<{}>", a.name)
            }
        })
        .collect();
    each.join(" ")
}

/// Numbers each registration, so the chat's tree can tell when the commands
/// change: a new registration (or a command whose completions changed) gets
/// a number no other ever had.
pub fn next_serial() -> u64 {
    static SERIAL: AtomicU64 = AtomicU64::new(1);
    SERIAL.fetch_add(1, Ordering::Relaxed)
}

/// One number for the serials of the commands taken, in order: it changes
/// whenever they do.
pub fn stamp(serials: impl Iterator<Item = u64>) -> u64 {
    let mut h = DefaultHasher::new();
    for s in serials {
        s.hash(&mut h);
    }
    h.finish()
}

/// The studio's own commands as the chat's tree has them: `/studio
/// [<action>]`, whose action it completes, and `/lua <code>`.
pub fn studio_shapes() -> [ChatCommand; 2] {
    [
        ChatCommand {
            name: "studio".into(),
            args: vec![ChatArg {
                name: "action".into(),
                kind: ArgKind::Word,
                optional: true,
                asks: true,
            }],
        },
        ChatCommand {
            name: "lua".into(),
            args: vec![ChatArg {
                name: "code".into(),
                kind: ArgKind::Text,
                optional: false,
                asks: false,
            }],
        },
    ]
}

/// What `/studio` takes after it, with what each does.
pub fn studio_actions() -> Vec<Completion> {
    vec![
        ("help".into(), Some("Lists the commands the studio takes".into())),
        ("reload".into(), Some("Reloads every script mod".into())),
    ]
}

// ----------------------------------------------------------------- command's third argument

/// What `command(name, fn, third)` declares besides the name and the function.
#[derive(Default)]
pub struct CommandOptions {
    /// The description: the help text, or an options table's `description`.
    pub help: String,
    pub args: Option<Vec<CommandArg>>,
    /// Completes a command without declared arguments.
    pub suggest: Option<Suggest>,
}

/// `bad argument #3 to 'command' (...)`.
fn bad(lua: &Lua, msg: &str) -> Throw {
    bad_arg(lua, 2, "command", msg)
}

/// A string or a number as text; None for anything else.
fn text(v: &Value) -> Option<String> {
    match v {
        Value::Str(_) | Value::Num(_) => Some(v.plain_text()),
        _ => None,
    }
}

/// The items of a table that is a list (its only keys are 1 to #t), or None.
fn list_of(t: &Table) -> Option<Vec<Value>> {
    let n = t.len();
    let last = if n == 0 { Value::Nil } else { Value::Num(n as f64) };
    match t.next(&last) {
        Ok(None) => Some(t.list()),
        _ => None,
    }
}

/// Whether `name` is a command's or an argument's name: 1 to 32 letters,
/// digits, `_` or `-`.
pub fn good_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 32 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Reads command's third argument: nothing, a help text, or a table of options.
pub fn command_options(lua: &Lua, third: &Value) -> Result<CommandOptions, Throw> {
    match third {
        Value::Nil => Ok(CommandOptions::default()),
        Value::Str(_) | Value::Num(_) => Ok(CommandOptions {
            help: third.plain_text(),
            ..CommandOptions::default()
        }),
        Value::Table(t) => {
            let t = t.borrow();
            let help = match t.get_str("description") {
                Value::Nil => String::new(),
                v => text(&v).ok_or_else(|| bad(lua, "description: string expected"))?,
            };
            let suggest = match t.get_str("suggest") {
                Value::Nil => None,
                v if v.is_callable() => Some(Suggest::new(v)),
                _ => return Err(bad(lua, "suggest: function expected")),
            };
            let args = match t.get_str("args") {
                Value::Nil => None,
                Value::Table(list) => Some(read_args(lua, &list.borrow())?),
                _ => return Err(bad(lua, "args: a list of at most 16 arguments expected")),
            };
            if suggest.is_some() && args.is_some() {
                return Err(bad(lua, "suggest: with args, each argument takes its own suggest"));
            }
            Ok(CommandOptions { help, args, suggest })
        }
        other => Err(bad(
            lua,
            &format!("string or table expected, got {}", other.type_name()),
        )),
    }
}

fn read_args(lua: &Lua, list: &Table) -> Result<Vec<CommandArg>, Throw> {
    let items = match list_of(list) {
        Some(items) if items.len() <= MAX_ARGS => items,
        _ => return Err(bad(lua, "args: a list of at most 16 arguments expected")),
    };
    let mut args = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        args.push(read_arg(lua, i + 1, item)?);
    }
    for (i, pair) in args.windows(2).enumerate() {
        if pair[0].kind == ArgKind::Text {
            return Err(bad(
                lua,
                &format!("args[{}]: only the last argument can be text", i + 1),
            ));
        }
        if pair[0].optional && !pair[1].optional {
            return Err(bad(
                lua,
                &format!("args[{}]: a required argument can't follow an optional one", i + 2),
            ));
        }
    }
    Ok(args)
}

/// Argument `i` (from 1): a name (a required word), or a table.
fn read_arg(lua: &Lua, i: usize, item: &Value) -> Result<CommandArg, Throw> {
    let at = format!("args[{i}]");
    let name_error = || {
        bad(
            lua,
            &format!("{at}.name: a name of letters, digits, '_' or '-' expected"),
        )
    };
    let t = match item {
        Value::Str(_) => {
            let name = item.plain_text();
            if !good_name(&name) {
                return Err(bad(
                    lua,
                    &format!("{at}: a name of letters, digits, '_' or '-' expected"),
                ));
            }
            return Ok(CommandArg {
                name,
                kind: ArgKind::Word,
                optional: false,
                options: Vec::new(),
                suggest: None,
            });
        }
        Value::Table(t) => t.borrow(),
        _ => return Err(bad(lua, &format!("{at}: a name or a table expected"))),
    };
    let name = match t.get_str("name") {
        Value::Str(s) => String::from_utf8_lossy(&s).into_owned(),
        _ => return Err(name_error()),
    };
    if !good_name(&name) {
        return Err(name_error());
    }
    let mut kind = match t.get_str("kind") {
        Value::Nil => ArgKind::Word,
        Value::Str(s) => {
            let k = String::from_utf8_lossy(&s).into_owned();
            ArgKind::named(&k)
                .ok_or_else(|| bad(lua, &format!("{at}.kind: unknown kind '{k}' ({})", ArgKind::NAMES)))?
        }
        _ => return Err(bad(lua, &format!("{at}.kind: a kind expected ({})", ArgKind::NAMES))),
    };
    let optional = match t.get_str("optional") {
        Value::Nil => false,
        Value::Bool(b) => b,
        _ => return Err(bad(lua, &format!("{at}.optional: true or false expected"))),
    };
    match &mut kind {
        ArgKind::Int { min, max } => {
            let whole = |key: &str, default: i32| -> Result<i32, Throw> {
                match t.get_str(key) {
                    Value::Nil => Ok(default),
                    Value::Num(n) if n.fract() == 0.0 && n >= f64::from(i32::MIN) && n <= f64::from(i32::MAX) => {
                        Ok(n as i32)
                    }
                    _ => Err(bad(
                        lua,
                        &format!("{at}.{key}: a whole number from -2147483648 to 2147483647 expected"),
                    )),
                }
            };
            *min = whole("min", i32::MIN)?;
            *max = whole("max", i32::MAX)?;
            if *min > *max {
                return Err(bad(lua, &format!("{at}.min is more than its max")));
            }
        }
        ArgKind::Number { min, max } => {
            let finite = |key: &str, default: f64| -> Result<f64, Throw> {
                match t.get_str(key) {
                    Value::Nil => Ok(default),
                    Value::Num(n) if n.is_finite() => Ok(n),
                    _ => Err(bad(lua, &format!("{at}.{key}: a finite number expected"))),
                }
            };
            *min = finite("min", f64::MIN)?;
            *max = finite("max", f64::MAX)?;
            if *min > *max {
                return Err(bad(lua, &format!("{at}.min is more than its max")));
            }
        }
        _ => {}
    }
    let options = match t.get_str("options") {
        Value::Nil => Vec::new(),
        Value::Table(o) => read_options(&o.borrow()).ok_or_else(|| {
            bad(
                lua,
                &format!("{at}.options: a list of at most 256 strings or {{text, tooltip}} tables expected"),
            )
        })?,
        _ => {
            return Err(bad(
                lua,
                &format!("{at}.options: a list of at most 256 strings or {{text, tooltip}} tables expected"),
            ));
        }
    };
    let suggest = match t.get_str("suggest") {
        Value::Nil => None,
        v if v.is_callable() => Some(Suggest::new(v)),
        _ => return Err(bad(lua, &format!("{at}.suggest: function expected"))),
    };
    Ok(CommandArg {
        name,
        kind,
        optional,
        options,
        suggest,
    })
}

/// A list of at most 256 completions: strings, numbers, or `{text = , tooltip = }`.
fn read_options(t: &Table) -> Option<Vec<Completion>> {
    if t.len() > MAX_COMPLETIONS {
        return None;
    }
    list_of(t)?.iter().map(completion_of).collect()
}

/// A completion as a script gives one: a string, a number, or a table with
/// `text` (and `tooltip`). None for anything else.
fn completion_of(v: &Value) -> Option<Completion> {
    if let Some(t) = text(v) {
        return Some((t, None));
    }
    let Value::Table(t) = v else { return None };
    let t = t.borrow();
    let shown = text(&t.get_str("text"))?;
    let tooltip = match t.get_str("tooltip") {
        Value::Nil => None,
        tip => Some(text(&tip)?),
    };
    Some((shown, tooltip))
}

/// What a suggest function returned: nothing (nil), or a list whose entries
/// are completions (entries of other types are skipped); an error message
/// for anything else.
pub fn returned_completions(v: Option<&Value>) -> Result<Vec<Completion>, String> {
    match v {
        None | Some(Value::Nil) => Ok(Vec::new()),
        Some(Value::Table(t)) => {
            let t = t.borrow();
            let n = t.len().min(MAX_SCANNED);
            Ok((1..=n).filter_map(|i| completion_of(&t.get_int(i as i64))).collect())
        }
        Some(other) => Err(format!("suggest returned a {}, not a list", other.type_name())),
    }
}

// ----------------------------------------------------------------- completing

/// The name a request line (`/name ...`, up to the cursor) starts with, and
/// what follows the space after it; None for the rest while the name is
/// still being typed.
pub fn split_line(line: &str) -> Option<(&str, Option<&str>)> {
    let body = line.strip_prefix('/')?;
    Some(match body.split_once(' ') {
        Some((name, rest)) => (name, Some(rest)),
        None => (body, None),
    })
}

/// What a suggest function is given for the text after a command's name:
/// that text without the spaces it starts with, and its words, the last
/// being the word typed ("" after a space).
pub fn suggest_args(rest: &str) -> (String, Vec<String>) {
    let rest = rest.trim_start();
    let mut words: Vec<String> = rest.split_whitespace().map(str::to_owned).collect();
    if rest.is_empty() || rest.ends_with(char::is_whitespace) {
        words.push(String::new());
    }
    (rest.to_owned(), words)
}

/// Where the word at the end of `line` starts (after the last whitespace,
/// but not before `from`).
pub fn last_word(line: &str, from: usize) -> usize {
    let mut start = from.min(line.len());
    for (i, c) in line.char_indices() {
        if i >= from && c.is_whitespace() {
            start = i + c.len_utf8();
        }
    }
    start
}

/// Text in UTF-16 units, as a server's answer counts the request's text.
pub fn utf16(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Horizon's `matches_sub_str` (args.rs, SharedSuggestionProvider's
/// filter), on lower-cased text: `candidate` starts with `input`, or a part
/// of it after a `.`, `_` or `/` does.
pub fn sub_match(input: &str, candidate: &str) -> bool {
    let mut index = 0;
    loop {
        if candidate[index..].starts_with(input) {
            return true;
        }
        match candidate[index..].find(['.', '_', '/']) {
            Some(i) => index += i + 1,
            None => return false,
        }
    }
}

/// A tooltip cut to `MAX_COMPLETION_BYTES`, at a character's end.
fn cut(mut s: String) -> String {
    if s.len() > MAX_COMPLETION_BYTES {
        let mut end = MAX_COMPLETION_BYTES;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
    }
    s
}

/// The completions an answer gives for `typed`: those that match it, as the
/// game's own providers match (`sub_match`), without the text typed itself,
/// once each, at most `MAX_COMPLETIONS`. Empty texts, texts over
/// `MAX_COMPLETION_BYTES` and, except for text (`spaces`), texts with
/// whitespace are left out; tooltips are cut.
pub fn finish(typed: &str, spaces: bool, found: Vec<Completion>) -> Vec<Completion> {
    let lower = typed.to_lowercase();
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for (text, tooltip) in found {
        if text.is_empty()
            || text.len() > MAX_COMPLETION_BYTES
            || (!spaces && text.contains(char::is_whitespace))
            || text == typed
            || !sub_match(&lower, &text.to_lowercase())
            || !seen.insert(text.clone())
        {
            continue;
        }
        out.push((text, tooltip.map(cut)));
        if out.len() >= MAX_COMPLETIONS {
            break;
        }
    }
    out
}

/// Where the chat completes `line` (a command up to the cursor) of a command
/// shaped `shape`, read the simple way: words between single spaces, three
/// for a pos, the rest of the line for text. The argument (from 0) and the
/// byte its text starts at; past the last argument, the argument count and
/// the line's end. None while the name is still being typed. In the game the
/// chat's own parse decides this (hstudio_cmdtree); this stands in for it in
/// headless tests.
pub fn locate(shape: &ChatCommand, line: &str) -> Option<(usize, usize)> {
    let (_, rest) = split_line(line)?;
    let rest = rest?;
    let mut at = line.len() - rest.len();
    for (i, a) in shape.args.iter().enumerate() {
        let Some(words) = a.kind.words() else {
            return Some((i, at));
        };
        let mut end = at;
        for w in 0..words {
            match line[end..].find(' ') {
                Some(space) if w + 1 < words => end += space + 1,
                Some(space) => end += space,
                None => return Some((i, at)),
            }
        }
        at = end + 1;
    }
    Some((shape.args.len(), line.len()))
}
