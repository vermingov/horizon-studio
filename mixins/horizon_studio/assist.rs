// The editor's and console's help as you type: suggestions (the API with
// its signatures, the standard library, keywords, event names in on("...",
// the buffer's own words, string methods after ':'), and the live check of a
// script (syntax errors with their line and column from the studio's own
// parser, and names that don't exist: `player.poss`, `plyer.pos`).

use super::hstudio::{API, CONSOLE_API};

/// What a suggestion is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Function,
    Namespace,
    Keyword,
    Event,
    Word,
    Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Suggestion {
    /// What the list shows, and what replaces the word typed.
    pub label: String,
    pub insert: String,
    /// The signature, and a line about it.
    pub detail: String,
    pub doc: String,
    pub kind: Kind,
}

/// The standard library: (name, signature, what it does), as the reference
/// (src/mcp_docs.rs) documents it; tests/assist.rs keeps them in step.
pub const STDLIB: &[(&str, &str, &str)] = &[
    (
        "print",
        "print(...)",
        "Writes its arguments, each through tostring and separated by tabs, as one line in the studio's console lo...",
    ),
    (
        "type",
        "type(v)",
        "The type's name: \"nil\", \"boolean\", \"number\", \"string\", \"table\" or \"function\" (built-in function...",
    ),
    (
        "tostring",
        "tostring(v)",
        "A value as text, with __tostring when its metatable has one",
    ),
    (
        "tonumber",
        "tonumber(v[, base])",
        "A number from a number or a string: decimal (\"1e3\", \".5\", \"  5  \") or hexadecimal integers (\"0x10\")...",
    ),
    (
        "pairs",
        "pairs(t)",
        "An iterator over every key and value of a table: keys 1..n first, then the other keys in the order they wer...",
    ),
    (
        "ipairs",
        "ipairs(t)",
        "An iterator over t[1], t[2], ... up to the first nil",
    ),
    (
        "next",
        "next(t[, key])",
        "The key and value after key in the order pairs uses (nil: the first), or nil at the end",
    ),
    (
        "select",
        "select(n, ...)",
        "local function count(...) return select(\"#\", ...) end",
    ),
    (
        "rawget",
        "rawget(t, k)",
        "local t = setmetatable({}, { __index = function() return \"default\" end })",
    ),
    (
        "rawset",
        "rawset(t, k, v)",
        "local t = setmetatable({}, { __newindex = function() error(\"read only\") end })",
    ),
    (
        "rawequal",
        "rawequal(a, b)",
        "Whether two values are equal without __eq (tables and functions: the same one)",
    ),
    (
        "rawlen",
        "rawlen(v)",
        "The length of a table (without __len) or a string, in bytes",
    ),
    (
        "setmetatable",
        "setmetatable(t, mt)",
        "Sets (or with nil removes) a table's metatable and returns t",
    ),
    (
        "getmetatable",
        "getmetatable(v)",
        "A table's metatable (or its __metatable field when it has one), or nil",
    ),
    ("error", "error(value[, level])", "Raises an error with any value"),
    (
        "assert",
        "assert(v[, message, ...])",
        "Returns all its arguments when v is neither nil nor false; otherwise raises message as it is (any value...",
    ),
    (
        "pcall",
        "pcall(f, ...)",
        "Calls f(...) and catches errors: returns true and f's results, or false and the error value",
    ),
    (
        "xpcall",
        "xpcall(f, handler, ...)",
        "Like pcall, but on an error calls handler(error) and returns false and the handler's results",
    ),
    (
        "load",
        "load(text[, name[, mode[, env]]])",
        "Compiles Lua text into a function (it does not run it)",
    ),
    ("unpack", "unpack(t[, i[, j]])", "local x, y, z = unpack({ 1, 2, 3 })"),
    (
        "dump",
        "dump(v[, depth])",
        "The studio's own: a value as readable text, as the console shows values: strings quoted, tables expanded d...",
    ),
    (
        "inspect",
        "inspect(v[, depth])",
        "The same function as dump: a value as readable text",
    ),
    (
        "collectgarbage",
        "collectgarbage([opt])",
        "Does nothing and returns 0 (memory is managed by the studio; collectgarbage(\"count\") is 0)",
    ),
    (
        "_G",
        "_G",
        "The mod's own globals table (each mod and the console has its own)",
    ),
    ("_VERSION", "_VERSION", "The string \"Lua 5.3 (Horizon Studio)\""),
    (
        "string.len",
        "string.len(s)` / `s:len()",
        "The length in bytes (#s too)",
    ),
    (
        "string.sub",
        "string.sub(s[, i[, j]])` / `s:sub(i, j)",
        "The bytes from i to j (both included; default 1 and -1)",
    ),
    (
        "string.upper",
        "string.upper(s)` / `s:upper()",
        "ASCII letters in upper case (other bytes as they are)",
    ),
    (
        "string.lower",
        "string.lower(s)` / `s:lower()",
        "ASCII letters in lower case",
    ),
    (
        "string.rep",
        "string.rep(s, n[, sep])` / `s:rep(n, sep)",
        "print((\"ab\"):rep(3, \"-\")) -- ab-ab-ab",
    ),
    (
        "string.reverse",
        "string.reverse(s)` / `s:reverse()",
        "The bytes in reverse order",
    ),
    (
        "string.byte",
        "string.byte(s[, i[, j]])` / `s:byte(i, j)",
        "The byte values of s[i..j] (default i 1, j = i), negative positions from the end; nothing when out ...",
    ),
    (
        "string.char",
        "string.char(...)",
        "A string of the given byte values (0 to 255; others raise \"value out of range\")",
    ),
    (
        "string.format",
        "string.format(fmt, ...)` / `fmt:format(...)",
        "Formats values as C's printf: %d and %i (whole numbers only: a fraction raises \"number has no integer...",
    ),
    (
        "string.find",
        "string.find(s, pattern[, init[, plain]])` / `s:find(pattern, init, plain)",
        "The start and end of the first match of a Lua pattern (from byte init, default 1; negative counts from th...",
    ),
    (
        "string.match",
        "string.match(s, pattern[, init])` / `s:match(pattern, init)",
        "The captures of the first match (or the whole match when the pattern has none), or nil",
    ),
    (
        "string.gmatch",
        "string.gmatch(s, pattern)` / `s:gmatch(pattern)",
        "An iterator over every match: each round gives the captures (or the whole match)",
    ),
    (
        "string.gsub",
        "string.gsub(s, pattern, repl[, n])` / `s:gsub(pattern, repl, n)",
        "local s, n = (\"hello world\"):gsub(\"o\", \"0\")             -- \"hell0 w0rld\", 2",
    ),
    (
        "string.split",
        "string.split(s[, sep[, plain]])` / `s:split(sep, plain)",
        "The studio's own: the pieces of s between separators, as a list",
    ),
    (
        "string.trim",
        "string.trim(s)` / `s:trim()",
        "The studio's own: s without ASCII whitespace at both ends",
    ),
    (
        "string.startswith",
        "string.startswith(s, prefix)` / `s:startswith(prefix)",
        "The studio's own: whether s starts with prefix (plain text)",
    ),
    (
        "string.endswith",
        "string.endswith(s, suffix)` / `s:endswith(suffix)",
        "The studio's own: whether s ends with suffix (plain text)",
    ),
    ("math.abs", "math.abs(x)", "The absolute value"),
    (
        "math.ceil",
        "math.ceil(x)",
        "The smallest whole number at or above x (a float, printed without a fraction)",
    ),
    ("math.floor", "math.floor(x)", "The largest whole number at or below x"),
    (
        "math.sqrt",
        "math.sqrt(x)",
        "The square root (nan for negative numbers)",
    ),
    (
        "math.sin",
        "math.sin(x)",
        "The sine of x in radians (math.rad turns degrees into radians)",
    ),
    ("math.cos", "math.cos(x)", "The cosine of x in radians"),
    ("math.tan", "math.tan(x)", "The tangent of x in radians"),
    (
        "math.asin",
        "math.asin(x)",
        "The arc sine, in radians (nan outside -1..1)",
    ),
    ("math.acos", "math.acos(x)", "The arc cosine, in radians"),
    (
        "math.atan",
        "math.atan(y[, x])",
        "The arc tangent of y / x in radians, using both signs for the quadrant (x defaults to 1)",
    ),
    ("math.exp", "math.exp(x)", "e to the power x"),
    (
        "math.log",
        "math.log(x[, base])",
        "The natural logarithm, or in a base (2 and 10 exactly)",
    ),
    (
        "math.fmod",
        "math.fmod(a, b)",
        "The remainder of a / b with the sign of a (% has the sign of b)",
    ),
    (
        "math.modf",
        "math.modf(x)",
        "The whole part (towards zero) and the fraction",
    ),
    ("math.max", "math.max(x, ...)", "The largest argument (at least one)"),
    ("math.min", "math.min(x, ...)", "The smallest argument (at least one)"),
    (
        "math.random",
        "math.random([m[, n]])",
        "A pseudo-random number: with no argument a float in [0, 1); math.random(n) a whole number in 1..n; math....",
    ),
    (
        "math.randomseed",
        "math.randomseed([x])",
        "Restarts the random generator from a seed (default 0): the same seed gives the same numbers",
    ),
    (
        "math.tointeger",
        "math.tointeger(x)",
        "assert(math.tointeger(3.0) == 3 and math.tointeger(3.5) == nil)",
    ),
    (
        "math.type",
        "math.type(x)",
        "print(math.type(3), math.type(3.0), math.type(3.5), math.type(\"3\")) -- integer integer float nil",
    ),
    (
        "math.clamp",
        "math.clamp(x, lo, hi)",
        "The studio's own: x kept within lo and hi",
    ),
    (
        "math.rad",
        "math.rad(x)",
        "Degrees to radians: x * pi / 180 (math.rad(180) is 3.1415926535898)",
    ),
    ("math.deg", "math.deg(x)", "Radians to degrees"),
    (
        "math.ult",
        "math.ult(m, n)",
        "Whether m < n when both are read as unsigned 64-bit integers (whole numbers only)",
    ),
    (
        "math.atan2",
        "math.atan2(y, x)",
        "From Lua 5.1: the same function as math.atan(y, x)",
    ),
    ("math.pow", "math.pow(x, y)", "From Lua 5.1: x ^ y"),
    ("math.log10", "math.log10(x)", "From Lua 5.1: the base 10 logarithm"),
    ("math.sinh", "math.sinh(x)", "From Lua 5.1: the hyperbolic sine"),
    ("math.cosh", "math.cosh(x)", "From Lua 5.1: the hyperbolic cosine"),
    (
        "math.tanh",
        "math.tanh(x)",
        "From Lua 5.1: the hyperbolic tangent (a smooth step from -1 to 1)",
    ),
    (
        "math.frexp",
        "math.frexp(x)",
        "From Lua 5.1: m, e with x = m * 2^e and 0.5 <= |m| < 1 (0 and non-finite numbers give x, 0)",
    ),
    ("math.ldexp", "math.ldexp(m, e)", "From Lua 5.1: m * 2^e"),
    ("math.pi", "math.pi", "The number pi (prints as 3.1415926535898)"),
    (
        "math.huge",
        "math.huge",
        "Infinity (prints as inf); -math.huge is minus infinity",
    ),
    (
        "math.maxinteger",
        "math.maxinteger",
        "The largest whole number numbers hold exactly here: 2^53 - 1 = 9007199254740991 (not 2^63 - 1 as in Lua 5.3...",
    ),
    ("math.mininteger", "math.mininteger", "local lowest = math.mininteger"),
    (
        "table.insert",
        "table.insert(t, [pos,] value)",
        "Appends value at #t + 1, or inserts it at pos (1 to #t + 1), shifting the rest up",
    ),
    (
        "table.remove",
        "table.remove(t[, pos])",
        "Removes and returns t[pos] (default the last), shifting the rest down",
    ),
    (
        "table.concat",
        "table.concat(t[, sep[, i[, j]]])",
        "The strings and numbers t[i]..t[j] (default 1 and #t) joined with sep (default \"\")",
    ),
    (
        "table.sort",
        "table.sort(t[, less])",
        "Sorts t[1..#t] in place: with <, or with less(a, b) returning true when a goes first",
    ),
    (
        "table.unpack",
        "table.unpack(t[, i[, j]])",
        "local x, y, z = table.unpack({ 10, 64, -3 })",
    ),
    (
        "table.pack",
        "table.pack(...)",
        "A list of the arguments with n, their count (nils included)",
    ),
    (
        "table.move",
        "table.move(a1, f, e, t[, a2])",
        "Copies a1[f..e] into a2 (default a1) from position t, correctly when the ranges overlap; returns a2",
    ),
    (
        "utf8.char",
        "utf8.char(...)",
        "A string of the given code points as UTF-8",
    ),
    (
        "utf8.codepoint",
        "utf8.codepoint(s[, i[, j]])",
        "The code points of the characters that start between bytes i and j (default i 1, j = i)",
    ),
    (
        "utf8.len",
        "utf8.len(s[, i[, j]])",
        "The number of UTF-8 characters between bytes i and j (default 1 and -1); for invalid UTF-8, nil and the...",
    ),
    (
        "utf8.offset",
        "utf8.offset(s, n[, i])",
        "The byte position where the nth character (counting from byte i) starts; negative n counts back; nil pa...",
    ),
    (
        "utf8.codes",
        "utf8.codes(s)",
        "An iterator over the characters: each round gives the byte position and the code point",
    ),
    (
        "utf8.charpattern",
        "utf8.charpattern",
        "A pattern that matches one UTF-8 character: \"[\\0-\\x7F\\xC2-\\xFD][\\x80-\\xBF]*\"",
    ),
    (
        "os.clock",
        "os.clock()",
        "Seconds since the studio's interpreter started, by the wall clock (not CPU time as in real Lua)",
    ),
    (
        "os.time",
        "os.time([t])",
        "The current time in whole seconds since 1970 (UTC)",
    ),
    (
        "os.date",
        "os.date([format[, time]])",
        "A time (default now) as text, always in UTC (a leading ! is accepted and changes nothing)",
    ),
    ("os.difftime", "os.difftime(t2[, t1])", "local start = os.time()"),
];

pub const KEYWORDS: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "if", "in", "local", "nil", "not", "or",
    "repeat", "return", "then", "true", "until", "while",
];

/// The events on() takes, and what they pass.
pub const EVENTS: &[(&str, &str)] = &[
    ("tick", "20 times a second"),
    ("frame", "every frame: (dt)"),
    ("hud", "as the HUD draws: (w, h); hud.* and gfx.* draw here"),
    ("world", "as the world draws: draw.* draw here"),
    ("key", "a key goes down or up: (key, down)"),
    ("chat", "a chat line arrives: (text)"),
    ("unload", "the mod stops or reloads"),
];

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// A doc line from the API table as (signature, description).
fn split_doc(doc: &str) -> (String, String) {
    match doc.find("): ") {
        Some(at) => (doc[..=at].to_owned(), doc[at + 3..].to_owned()),
        None => (doc.to_owned(), String::new()),
    }
}

/// Every name a script can call: (full name, signature, description), the
/// console's own with `console`.
fn names(console: bool) -> Vec<(String, String, String)> {
    let mut out: Vec<(String, String, String)> = API
        .iter()
        .map(|(t, n, _, d)| {
            let (sig, desc) = split_doc(d);
            (
                if t.is_empty() {
                    (*n).to_owned()
                } else {
                    format!("{t}.{n}")
                },
                sig,
                desc,
            )
        })
        .collect();
    out.extend(
        STDLIB
            .iter()
            .map(|(n, s, d)| ((*n).to_owned(), (*s).to_owned(), (*d).to_owned())),
    );
    if console {
        out.extend(CONSOLE_API.iter().map(|(n, _, d)| {
            let (sig, desc) = split_doc(d);
            ((*n).to_owned(), sig, desc)
        }));
    }
    out
}

/// The tables scripts reach things through: the API's and the library's.
pub fn namespaces() -> Vec<String> {
    let mut ns: Vec<String> = API
        .iter()
        .filter(|(t, ..)| !t.is_empty())
        .map(|(t, ..)| (*t).to_owned())
        .chain(
            STDLIB
                .iter()
                .filter_map(|(n, ..)| n.split_once('.').map(|(t, _)| t.to_owned())),
        )
        .collect();
    ns.sort();
    ns.dedup();
    ns
}

/// How well `name` matches what was typed: a prefix first, then any case,
/// then its letters in order; None when it doesn't.
fn score(name: &str, typed: &str) -> Option<u32> {
    if typed.is_empty() {
        return Some(1);
    }
    if name.starts_with(typed) {
        return Some(400 - name.len().min(300) as u32);
    }
    let (n, t) = (name.to_lowercase(), typed.to_lowercase());
    if n.starts_with(&t) {
        return Some(300 - name.len().min(200) as u32);
    }
    if n.contains(&t) {
        return Some(150 - name.len().min(100) as u32);
    }
    let mut it = n.chars();
    t.chars()
        .all(|c| it.any(|x| x == c))
        .then(|| 50 - name.len().min(40) as u32)
}

/// Words of the buffer (names it defines and uses), for suggestions.
fn buffer_words(lines: &[String]) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for line in lines {
        let mut word = String::new();
        for c in line.chars().chain([' ']) {
            if is_word(c) {
                word.push(c);
            } else {
                if word.len() >= 3
                    && !word.starts_with(|c: char| c.is_ascii_digit())
                    && !KEYWORDS.contains(&word.as_str())
                {
                    words.push(std::mem::take(&mut word));
                }
                word.clear();
            }
        }
    }
    words.sort();
    words.dedup();
    words
}

/// Suggestions for the text before the cursor on its line (`before`), with
/// the buffer's lines for its words (`console`: the console's own functions
/// too, no keywords): where the word being typed starts (a
/// byte in `before`), and the suggestions, best first. None where nothing
/// fits (in a comment or a string, or after a number).
pub fn suggest(before: &str, lines: &[String], console: bool) -> Option<(usize, Vec<Suggestion>)> {
    // Inside on(" or on(': the events.
    let trimmed = before.trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == ':');
    if let Some(head) = trimmed.strip_suffix('"').or_else(|| trimmed.strip_suffix('\'')) {
        let head = head.trim_end();
        if head.ends_with("on(")
            && head[..head.len() - 3]
                .chars()
                .last()
                .is_none_or(|c| !is_word(c) && c != '.')
        {
            let start = trimmed.len();
            let typed = &before[start..];
            let mut items: Vec<(u32, Suggestion)> = EVENTS
                .iter()
                .filter_map(|(e, d)| {
                    score(e, typed).map(|s| {
                        (
                            s,
                            Suggestion {
                                label: (*e).to_owned(),
                                insert: (*e).to_owned(),
                                detail: format!("on(\"{e}\", function(...) end)"),
                                doc: (*d).to_owned(),
                                kind: Kind::Event,
                            },
                        )
                    })
                })
                .collect();
            items.sort_by_key(|x| std::cmp::Reverse(x.0));
            return (!items.is_empty()).then(|| (start, items.into_iter().map(|x| x.1).collect()));
        }
    }
    if in_string_or_comment(before) {
        return None;
    }
    // The name being typed: a word, after a '.' or ':' chain.
    let word_start = before.rfind(|c: char| !is_word(c)).map_or(0, |i| i + 1);
    let typed = &before[word_start..];
    if typed.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let sep = before[..word_start].chars().last();
    let mut items: Vec<(u32, Suggestion)> = Vec::new();
    let push = |label: &str, detail: &str, doc: &str, kind: Kind, items: &mut Vec<(u32, Suggestion)>| {
        if let Some(s) = score(label, typed).filter(|_| label != typed) {
            items.push((
                s + match kind {
                    Kind::Function | Kind::Namespace | Kind::Event => 5,
                    _ => 0,
                },
                Suggestion {
                    label: label.to_owned(),
                    insert: label.to_owned(),
                    detail: detail.to_owned(),
                    doc: doc.to_owned(),
                    kind,
                },
            ));
        }
    };
    let mut start = word_start;
    match sep {
        Some(sep @ ('.' | ':')) => {
            // What is before the '.' or ':': a library, a table of the
            // file's (`a` or `a.b`), a string, or a call's result.
            let owner_end = word_start - 1;
            let owner_start = before[..owner_end]
                .rfind(|c: char| !is_word(c) && c != '.')
                .map_or(0, |i| i + 1);
            let owner = before[owner_start..owner_end].trim_matches('.');
            let library: Vec<(String, String, String)> = names(console)
                .into_iter()
                .filter(|(n, ..)| n.strip_prefix(owner).is_some_and(|r| r.starts_with('.')) && !owner.is_empty())
                .collect();
            let mine = if owner.is_empty() {
                Vec::new()
            } else {
                file_members(lines, owner)
            };
            if sep == ':' && !library.is_empty() {
                // `player:` means `player.`: the library's functions, the
                // ':' made a '.' as they are taken.
                start = owner_end;
                for (n, sig, d) in &library {
                    let member = &n[owner.len() + 1..];
                    if score(member, typed).is_some_and(|_| member != typed) {
                        let kind = if sig.contains('(') { Kind::Function } else { Kind::Value };
                        items.push((
                            score(member, typed).unwrap_or(0) + 5,
                            Suggestion {
                                label: member.to_owned(),
                                insert: format!(".{member}"),
                                detail: sig.clone(),
                                doc: d.clone(),
                                kind,
                            },
                        ));
                    }
                }
            } else {
                for (n, sig, d) in &library {
                    let kind = if sig.contains('(') { Kind::Function } else { Kind::Value };
                    push(&n[owner.len() + 1..], sig, d, kind, &mut items);
                }
                // The file's own: its methods after ':', all of it after '.'.
                for (name, func) in &mine {
                    if sep == ':' && !func {
                        continue;
                    }
                    let (detail, kind) = if *func {
                        (format!("{owner}{sep}{name}(...)"), Kind::Function)
                    } else {
                        (format!("{owner}.{name}"), Kind::Value)
                    };
                    push(name, &detail, "in this file", kind, &mut items);
                }
                // A string's methods: after one, or after what the file
                // doesn't say anything about.
                let methods = mine.iter().any(|(_, f)| *f);
                if sep == ':' && (stringy(lines, &before[..owner_end], owner) || !methods) {
                    for (n, sig, d) in STDLIB.iter().filter(|(n, ..)| n.starts_with("string.")) {
                        let m = &n["string.".len()..];
                        push(m, &sig.replacen("string.", ":", 1), d, Kind::Function, &mut items);
                    }
                }
            }
        }
        _ => {
            for ns in namespaces() {
                let doc = format!("{ns}.*");
                push(&ns, &doc, "", Kind::Namespace, &mut items);
            }
            for (n, sig, d) in names(console).iter().filter(|(n, ..)| !n.contains('.')) {
                let kind = if sig.contains('(') { Kind::Function } else { Kind::Value };
                push(n, sig, d, kind, &mut items);
            }
            if !console {
                for k in KEYWORDS {
                    push(k, k, "", Kind::Keyword, &mut items);
                }
            }
            let known: Vec<String> = items.iter().map(|i| i.1.label.clone()).collect();
            for w in buffer_words(lines) {
                if !known.contains(&w) && w != typed {
                    push(&w, "", "in this file", Kind::Word, &mut items);
                }
            }
        }
    }
    if items.is_empty() || (typed.is_empty() && sep.is_none()) {
        return None;
    }
    items.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.label.cmp(&b.1.label)));
    items.dedup_by(|a, b| a.1.label == b.1.label);
    items.truncate(50);
    Some((start, items.into_iter().map(|x| x.1).collect()))
}

/// Whether `word` is at `at` in `line` as a whole name (not part of a
/// longer one, nor a field of another table: `x.obj` isn't `obj`).
fn whole_at(line: &str, at: usize, word: &str) -> bool {
    let b = line.as_bytes();
    let before_ok = at == 0 || !(is_word(b[at - 1] as char) || b[at - 1] == b'.' || b[at - 1] == b':');
    let end = at + word.len();
    before_ok && (end >= b.len() || !is_word(b[end] as char))
}

/// What the file puts in the table `owner` ("obj", "a.b"), as (name,
/// whether it is a function): `function owner:m(`, `function owner.f(`,
/// `owner.x = ...`, `owner:m(...)` calls, and the fields of a
/// `owner = { ... }` constructor.
fn file_members(lines: &[String], owner: &str) -> Vec<(String, bool)> {
    let mut out: Vec<(String, bool)> = Vec::new();
    let add = |name: &str, func: bool, out: &mut Vec<(String, bool)>| {
        if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) || KEYWORDS.contains(&name) {
            return;
        }
        match out.iter_mut().find(|(n, _)| n == name) {
            Some(e) => e.1 |= func,
            None => out.push((name.to_owned(), func)),
        }
    };
    let text: Vec<String> = lines.iter().map(|l| strip_strings(l)).collect();
    for (row, line) in text.iter().enumerate() {
        for (at, _) in line.match_indices(owner) {
            if !whole_at(line, at, owner) {
                continue;
            }
            let rest = &line[at + owner.len()..];
            let rest_t = rest.trim_start();
            if let Some(after) = rest_t.strip_prefix('.').or_else(|| rest_t.strip_prefix(':')) {
                let name: String = after.chars().take_while(|c| is_word(*c)).collect();
                if name.is_empty() {
                    continue;
                }
                let tail = after[name.len()..].trim_start();
                let defined = line[..at].trim_end().ends_with("function");
                let func = defined
                    || tail.starts_with('(')
                    || (tail.starts_with('=')
                        && !tail.starts_with("==")
                        && tail[1..].trim_start().starts_with("function"));
                // A deeper field (`owner.a.b`) is `a`'s, still `owner`'s member.
                add(&name, func && !tail.starts_with('.'), &mut out);
            } else if let Some(rhs) = rest_t.strip_prefix('=').filter(|r| !r.starts_with('='))
                && rhs.trim_start().starts_with('{')
            {
                // A constructor: its fields at the first depth.
                let open = line.len() - rhs.trim_start().len();
                constructor_fields(&text, row, open, &mut out, &add);
            }
        }
    }
    out
}

/// The fields `name = value` at the first depth of the constructor whose
/// `{` is at (`row`, `col`) of the stripped `text`.
fn constructor_fields(
    text: &[String],
    row: usize,
    col: usize,
    out: &mut Vec<(String, bool)>,
    add: &impl Fn(&str, bool, &mut Vec<(String, bool)>),
) {
    let mut depth = 0i32;
    for (r, line) in text.iter().enumerate().skip(row) {
        let from = if r == row { col } else { 0 };
        let b = line.as_bytes();
        let mut i = from;
        while i < b.len() {
            match b[i] {
                b'{' | b'(' | b'[' => depth += 1,
                b'}' | b')' | b']' => {
                    depth -= 1;
                    if depth == 0 {
                        return;
                    }
                }
                c if depth == 1 && is_word(c as char) && (i == 0 || !is_word(b[i - 1] as char)) => {
                    let name: String = line[i..].chars().take_while(|c| is_word(*c)).collect();
                    let tail = line[i + name.len()..].trim_start();
                    let prev = line[from..i].trim_end();
                    let field_start =
                        prev.ends_with('{') || prev.ends_with(',') || prev.ends_with(';') || prev.is_empty();
                    if field_start && tail.starts_with('=') && !tail.starts_with("==") {
                        add(&name, tail[1..].trim_start().starts_with("function"), out);
                    }
                    i += name.len();
                    continue;
                }
                _ => {}
            }
            i += 1;
        }
        if r > row + 2000 {
            return;
        }
    }
}

/// Whether what is before a ':' (`before`, ending at it; `owner` its name)
/// is a string: a literal, or a name the file sets to one.
fn stringy(lines: &[String], before: &str, owner: &str) -> bool {
    let b = before.trim_end();
    if b.ends_with('"') || b.ends_with('\'') || b.ends_with("]]") || b.ends_with("\")") || b.ends_with("')") {
        return true;
    }
    if owner.is_empty() {
        return false;
    }
    lines.iter().any(|line| {
        line.match_indices(owner).any(|(at, _)| {
            if !whole_at(line, at, owner) {
                return false;
            }
            let rest = line[at + owner.len()..].trim_start();
            let Some(rhs) = rest.strip_prefix('=').filter(|r| !r.starts_with('=')) else {
                return false;
            };
            let rhs = rhs.trim_start();
            rhs.starts_with('"')
                || rhs.starts_with('\'')
                || rhs.starts_with("[[")
                || rhs.starts_with("tostring(")
                || rhs.starts_with("string.")
                || rhs.contains("..")
        })
    })
}

/// Whether the end of `before` is in a string or a comment (a line's own:
/// long strings and comments over lines aren't followed).
fn in_string_or_comment(before: &str) -> bool {
    let mut quote: Option<char> = None;
    let mut chars = before.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                if c == '\\' {
                    chars.next();
                } else if c == q {
                    quote = None;
                }
            }
            None => {
                if c == '"' || c == '\'' {
                    quote = Some(c);
                } else if c == '-' && chars.peek() == Some(&'-') {
                    return true;
                }
            }
        }
    }
    quote.is_some()
}

// ------------------------------------------------------------------ the live check

/// A problem in a script, with where it is.
#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    /// 1-based.
    pub line: usize,
    /// Byte columns on the line it marks (all of it: None).
    pub span: Option<(usize, usize)>,
    pub message: String,
    /// An error (it won't run) or a warning.
    pub error: bool,
}

/// Checks a script as it is typed: its syntax (with the studio's parser),
/// then names that don't exist (`player.poss`, `plyer.pos`).
pub fn check(lines: &[String], chunk: &str) -> Vec<Problem> {
    let src = lines.join("\n");
    let mut lua = super::hstudio_lua::Lua::new();
    let env = std::rc::Rc::new(std::cell::RefCell::new(super::hstudio_lua::Table::new()));
    if let Err(e) = lua.load(&src, chunk, &env) {
        return vec![syntax_problem(&e, lines, chunk, super::hstudio_lua::syntax_error_at())];
    }
    unknown_names(lines)
}

/// A parser error ("chunk:12: unexpected symbol near 'end'") as a problem
/// under the token the parser stopped at (`at`, a byte of the source), else
/// on its line.
fn syntax_problem(e: &str, lines: &[String], chunk: &str, at: Option<usize>) -> Problem {
    let rest = e.strip_prefix(chunk).and_then(|r| r.strip_prefix(':')).unwrap_or(e);
    let (line, message) = match rest.split_once(": ") {
        Some((n, m)) if n.parse::<usize>().is_ok() => (n.parse().unwrap_or(1), m.to_owned()),
        _ => (1, rest.to_owned()),
    };
    // The parser's own place for it: its line and column.
    let mut place = None;
    if let Some(at) = at {
        let mut start = 0;
        for (i, l) in lines.iter().enumerate() {
            if at <= start + l.len() {
                place = Some((i + 1, at - start));
                break;
            }
            start += l.len() + 1;
        }
    }
    let line = place.map_or(line, |p| p.0).clamp(1, lines.len().max(1));
    let near = message
        .rsplit_once("near '")
        .and_then(|(_, t)| t.strip_suffix('\''))
        .map(str::to_owned);
    let text = lines.get(line - 1).map_or("", String::as_str);
    let token_len = |t: &str| t.len().max(1);
    let span = place
        .filter(|p| p.0 == line && p.1 < text.len())
        .map(|(_, col)| {
            let len = near.as_deref().filter(|t| text[col..].starts_with(*t)).map_or_else(
                || {
                    text[col..]
                        .find(|c: char| !is_word(c))
                        .unwrap_or(text.len() - col)
                        .max(1)
                },
                token_len,
            );
            (col, (col + len).min(text.len()))
        })
        .or_else(|| {
            near.clone()
                .filter(|t| !t.is_empty())
                .and_then(|t| text.find(t.as_str()).map(|at| (at, at + t.len())))
        })
        .or_else(|| {
            let start = text.len() - text.trim_start().len();
            (start < text.len()).then_some((start, text.trim_end().len()))
        });
    Problem {
        line,
        span,
        message,
        error: true,
    }
}

/// Uses of a library's member it doesn't have (`player.poss`), and of a
/// table name nothing defines (`plyer.pos`), outside strings and comments.
fn unknown_names(lines: &[String]) -> Vec<Problem> {
    let ns = namespaces();
    let all = names(true);
    let defined: std::collections::HashSet<String> = {
        let mut d = std::collections::HashSet::new();
        for line in lines {
            let code = strip_strings(line);
            let words: Vec<&str> = code.split(|c: char| !is_word(c)).filter(|w| !w.is_empty()).collect();
            for w in words.windows(2) {
                if w[0] == "local" || w[0] == "function" || w[0] == "for" {
                    d.insert(w[1].to_owned());
                }
            }
            // Assignments, parameters and loop names: every word on a line
            // before an '=' or inside function( ... ) and for ... in.
            if let Some(eq) = code.find('=').filter(|i| !code[*i..].starts_with("==")) {
                for w in code[..eq].split(|c: char| !is_word(c)).filter(|w| !w.is_empty()) {
                    d.insert(w.to_owned());
                }
            }
            for (open, close) in [("function", ")"), ("for", " in ")] {
                if let Some(a) = code.find(open)
                    && let Some(b) = code[a..].find(close)
                {
                    for w in code[a..a + b].split(|c: char| !is_word(c)).filter(|w| !w.is_empty()) {
                        d.insert(w.to_owned());
                    }
                }
            }
        }
        d
    };
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let code = strip_strings(line);
        let bytes = code.as_bytes();
        let mut at = 0;
        while at < bytes.len() {
            let c = bytes[at] as char;
            if !is_word(c) || c.is_ascii_digit() || (at > 0 && matches!(bytes[at - 1], b'.' | b':')) {
                at += 1;
                continue;
            }
            let end = code[at..].find(|c: char| !is_word(c)).map_or(code.len(), |e| at + e);
            let root = &code[at..end];
            if code[end..].starts_with('.') && !code[end..].starts_with("..") {
                let m_end = code[end + 1..]
                    .find(|c: char| !is_word(c))
                    .map_or(code.len(), |e| end + 1 + e);
                let member = &code[end + 1..m_end];
                let full = format!("{root}.{member}");
                if ns.iter().any(|n| n == root) && !defined.contains(root) {
                    if !member.is_empty() && !all.iter().any(|(n, ..)| n == &full) {
                        out.push(Problem {
                            line: i + 1,
                            span: Some((end + 1, m_end)),
                            message: format!("{root} has no '{member}'"),
                            error: false,
                        });
                    }
                } else if !defined.contains(root)
                    && !KEYWORDS.contains(&root)
                    && root != "self"
                    && !all.iter().any(|(n, ..)| n == root)
                    && let Some(close) = ns.iter().find(|n| near(n, root))
                {
                    out.push(Problem {
                        line: i + 1,
                        span: Some((at, end)),
                        message: format!("nothing defines '{root}' (did you mean {close}?)"),
                        error: false,
                    });
                }
                at = m_end.max(end + 1);
                continue;
            }
            at = end;
        }
    }
    out
}

/// Whether two names are a typo apart (one letter added, missing, changed
/// or two swapped).
fn near(a: &str, b: &str) -> bool {
    if a == b || a.len().abs_diff(b.len()) > 1 || a.len() < 3 {
        return false;
    }
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            d[i][j] = (d[i - 1][j] + 1).min(d[i][j - 1] + 1).min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[a.len()][b.len()] == 1
}

/// A line with its strings blanked and its comment cut, the same length up
/// to the comment (so byte columns stay right).
fn strip_strings(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut quote: Option<char> = None;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                out.push(' ');
                if c == '\\' {
                    if let Some(n) = chars.next() {
                        out.extend(std::iter::repeat_n(' ', n.len_utf8()));
                    }
                } else if c == q {
                    quote = None;
                } else {
                    out.extend(std::iter::repeat_n(' ', c.len_utf8() - 1));
                }
            }
            None => {
                if c == '-' && chars.peek() == Some(&'-') {
                    break;
                }
                if c == '"' || c == '\'' {
                    quote = Some(c);
                    out.push(' ');
                } else {
                    out.push(c);
                }
            }
        }
    }
    out
}
