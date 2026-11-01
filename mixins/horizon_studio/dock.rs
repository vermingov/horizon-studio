// The Editor tab's panes, as in Visual Studio: a tree of splits, side by
// side or stacked, each with its ratio, whose leaves are groups of tabs,
// each pane in it once. Where they go in the tab's area (no pane smaller
// than it is useful in while there is room, the pane in use first where
// there isn't, and a stacked group that gives way keeping its strip), the
// moves a drag, a menu or a key makes, where a dragged tab would land, and
// the layout kept in `layout.txt` in the studio's folder. Plain std: it
// draws nothing and knows nothing of the game.

use std::path::Path;

/// A pane of the Editor tab. Each is in the dock once at most: the script
/// list, the code, the console, the problems and the agent always (they
/// hide with their group, in place), the Library and the Docs once docked.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Pane {
    Scripts,
    #[default]
    Code,
    Console,
    Problems,
    Agent,
    Library,
    Docs,
}

/// Every pane, in the order of the per-pane arrays (`Pane::index`).
pub const PANES: [Pane; 7] = [
    Pane::Scripts,
    Pane::Code,
    Pane::Console,
    Pane::Problems,
    Pane::Agent,
    Pane::Library,
    Pane::Docs,
];

/// The gap between two groups, which the bar in it sizes.
pub const BAR: f32 = 8.0;
/// How near the Editor tab's left, right or bottom edge a dragged tab
/// drops along that edge.
pub const EDGE: f32 = 16.0;
/// A group's tab strip.
pub const STRIP: f32 = 32.0;
/// The least height a pane's body is drawn at.
pub const LEAST_BODY: f32 = 18.0;

/// The most of `layout.txt` that is read, the most of a dock's text, and
/// how deep its splits may nest: a damaged file never builds a deep tree.
const MOST_FILE: u64 = 16 * 1024;
const MOST_TEXT: usize = 4096;
const MOST_DEPTH: usize = 8;

impl Pane {
    /// Its place in `PANES`.
    pub fn index(self) -> usize {
        self as usize
    }
    /// Its name in `layout.txt`.
    pub fn id(self) -> &'static str {
        match self {
            Pane::Scripts => "scripts",
            Pane::Code => "code",
            Pane::Console => "console",
            Pane::Problems => "problems",
            Pane::Agent => "agent",
            Pane::Library => "library",
            Pane::Docs => "docs",
        }
    }
    pub fn from_id(id: &str) -> Option<Pane> {
        PANES.into_iter().find(|p| p.id() == id)
    }
    /// Its tab's name.
    pub fn title(self) -> &'static str {
        match self {
            Pane::Scripts => "Scripts",
            Pane::Code => "Code",
            Pane::Console => "Console",
            Pane::Problems => "Problems",
            Pane::Agent => "Agent",
            Pane::Library => "Library",
            Pane::Docs => "Docs",
        }
    }
    /// Whether typing goes to it: all but the script list, which has no
    /// use for keys.
    pub fn takes_keys(self) -> bool {
        self != Pane::Scripts
    }
    /// Whether it can leave the dock: the Library and the Docs, which have
    /// pages of their own. The others hide with their group.
    pub fn closable(self) -> bool {
        matches!(self, Pane::Library | Pane::Docs)
    }
    /// The least room it is useful in, wide and high, its strip included.
    /// 120 high is the strip, the body's inset, an input line and two lines
    /// of a log; the Library's 350 wide is its three buttons in a row.
    pub fn least(self) -> [f32; 2] {
        match self {
            Pane::Scripts => [140.0, 100.0],
            Pane::Code => [300.0, 140.0],
            Pane::Console | Pane::Problems => [200.0, 120.0],
            Pane::Agent => [220.0, 120.0],
            Pane::Library => [350.0, 220.0],
            Pane::Docs => [240.0, 120.0],
        }
    }
    /// Its body in a group placed at `r`: under the strip, inset as the
    /// panel's panes are, or a unit at the sides and bottom for the code
    /// and the script list, which fill their group.
    pub fn body(self, [x, y, w, h]: [f32; 4]) -> [f32; 4] {
        match self {
            Pane::Code | Pane::Scripts => [x + 1.0, y + STRIP, w - 2.0, h - STRIP - 1.0],
            _ => [x + 14.0, y + STRIP + 5.0, w - 28.0, h - STRIP - 10.0],
        }
    }
}

/// A side of a group, or an edge of the Editor tab.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

/// A group of tabs: its panes in their order, the one at the front, and
/// whether the group is hidden (it keeps its place and its size).
#[derive(Clone, Debug, PartialEq)]
pub struct Leaf {
    pub tabs: Vec<Pane>,
    pub active: Pane,
    pub hidden: bool,
}

/// A group, or two sides split: side by side with `row` (a at the left),
/// stacked without (a on top). `ratio` is a's share of the length less
/// the bar.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Leaf(Leaf),
    Split {
        row: bool,
        ratio: f32,
        a: Box<Node>,
        b: Box<Node>,
    },
}

/// The Editor tab's layout. A pane missing from it is closed, which only
/// the Library and the Docs can be.
#[derive(Clone, Debug, PartialEq)]
pub struct Dock {
    pub root: Node,
}

impl Default for Dock {
    /// The tab as it always was: the script list at the left, and at its
    /// right the code over the panel of the console, the problems and the
    /// agent.
    fn default() -> Dock {
        Dock {
            root: Node::Split {
                row: true,
                ratio: 0.18,
                a: Box::new(group(&[Pane::Scripts])),
                b: Box::new(Node::Split {
                    row: false,
                    ratio: 0.6,
                    a: Box::new(group(&[Pane::Code])),
                    b: Box::new(group(&[Pane::Console, Pane::Problems, Pane::Agent])),
                }),
            },
        }
    }
}

/// A bar between the two sides of a split, as laid out: its split's path
/// (1 for the root, 2n and 2n + 1 for split n's sides), where it is,
/// whether the sides are side by side, and the range its position (its
/// left or top edge) runs in. The split's ratio is (position - start) / len.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    pub path: u16,
    pub rect: [f32; 4],
    pub row: bool,
    pub start: f32,
    pub len: f32,
    pub lo: f32,
    pub hi: f32,
}

/// A group as laid out: where, its tabs, the one at the front, and whether
/// it is only its strip (it gave way to the pane in use).
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub rect: [f32; 4],
    pub tabs: Vec<Pane>,
    pub active: Pane,
    pub collapsed: bool,
}

impl Placed {
    /// Whether the front pane's body is drawn: the group isn't only its
    /// strip, and the body is high enough for a line.
    pub fn has_body(&self) -> bool {
        !self.collapsed && self.active.body(self.rect)[3] >= LEAST_BODY
    }
}

/// Where a pane is moved: among a group's tabs at a place (counted
/// without the pane moved), beside a group on a side, or along an edge of
/// the whole tab. A group is named by a pane in it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DropAt {
    Into { anchor: Pane, at: usize },
    Beside { anchor: Pane, side: Side },
    Edge(Side),
}

/// A group as the last frame drew it: its place, its strip, its tabs
/// with theirs, its front pane, and whether it was only its strip.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawnLeaf {
    pub rect: [f32; 4],
    pub strip: [f32; 4],
    pub tabs: Vec<(Pane, [f32; 4])>,
    pub active: Pane,
    pub collapsed: bool,
}

/// Where a dragged tab would land: the move, the place its pane's group
/// takes after it, and for a place among a strip's tabs the bar shown
/// there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DropSpot {
    pub at: DropAt,
    pub rect: [f32; 4],
    pub bar: Option<[f32; 4]>,
}

/// What a move tried on a copy of the dock gives.
enum Trial {
    /// Its pane's group's place.
    Lands([f32; 4]),
    /// It would take a pane's place, or leave its own pane no body.
    Crowds,
    /// Nothing would change (or it names a group not in the dock).
    Nothing,
}

impl Dock {
    /// Where each group goes in `ws`, and the bars between them, in the
    /// tree's order. Where a split can't give both sides their least room,
    /// the side with `keep` (the pane with the keys) gets it, else the side
    /// with the code; a group stacked against it keeps its strip where
    /// there is room for that.
    pub fn layout(&self, ws: [f32; 4], keep: Pane) -> (Vec<Bar>, Vec<Placed>) {
        let ws = ws.map(|v| if v.is_finite() { v } else { 0.0 });
        let ws = [ws[0], ws[1], ws[2].max(0.0), ws[3].max(0.0)];
        let mut out = (Vec::new(), Vec::new());
        place(&self.root, ws, 1, keep, &mut out);
        out
    }

    fn leaf(&self, p: Pane) -> Option<&Leaf> {
        fn find(node: &Node, p: Pane) -> Option<&Leaf> {
            match node {
                Node::Leaf(l) => l.tabs.contains(&p).then_some(l),
                Node::Split { a, b, .. } => find(a, p).or_else(|| find(b, p)),
            }
        }
        find(&self.root, p)
    }

    fn leaf_mut(&mut self, p: Pane) -> Option<&mut Leaf> {
        match node_with(&mut self.root, p)? {
            Node::Leaf(l) => Some(l),
            Node::Split { .. } => None,
        }
    }

    /// Whether `p` is in the dock (not closed).
    pub fn has(&self, p: Pane) -> bool {
        self.leaf(p).is_some()
    }
    /// Whether `p` is in the dock and its group isn't hidden.
    pub fn shown(&self, p: Pane) -> bool {
        self.leaf(p).is_some_and(|l| !l.hidden)
    }
    /// Whether `p` is shown at the front of its group.
    pub fn is_front(&self, p: Pane) -> bool {
        self.leaf(p).is_some_and(|l| !l.hidden && l.active == p)
    }
    /// The tabs of `p`'s group (none when it is closed).
    pub fn leaf_tabs(&self, p: Pane) -> &[Pane] {
        self.leaf(p).map_or(&[], |l| &l.tabs)
    }
    /// Every group, hidden ones too, in the tree's order.
    pub fn leaves(&self) -> Vec<&Leaf> {
        fn walk<'a>(node: &'a Node, out: &mut Vec<&'a Leaf>) {
            match node {
                Node::Leaf(l) => out.push(l),
                Node::Split { a, b, .. } => {
                    walk(a, out);
                    walk(b, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.root, &mut out);
        out
    }
    /// The ratio of the split at `path`.
    pub fn ratio(&self, path: u16) -> Option<f32> {
        if path == 0 {
            return None;
        }
        let mut node = &self.root;
        for i in (0..15 - path.leading_zeros()).rev() {
            node = match node {
                Node::Split { a, b, .. } => {
                    if (path >> i) & 1 == 0 {
                        a
                    } else {
                        b
                    }
                }
                Node::Leaf(_) => return None,
            };
        }
        match node {
            Node::Split { ratio, .. } => Some(*ratio),
            Node::Leaf(_) => None,
        }
    }

    /// The pane that showing `p` would put behind it: the front of its
    /// group, or for a closed Library or Docs the front of the group its
    /// home joins. None when it is at the front already or would have a
    /// group of its own.
    pub fn covers(&self, p: Pane) -> Option<Pane> {
        match self.leaf(p) {
            Some(l) => (l.active != p).then_some(l.active),
            None => self.leaf(other_page(p)?).map(|l| l.active),
        }
    }

    /// Where the keys settle from `focus`: there when it is at the front of
    /// a shown group, else the front of its group (another tab covered it),
    /// else (its group hidden, or it closed) the front of the code's group.
    pub fn settled(&self, focus: Pane) -> Pane {
        match self.leaf(focus) {
            Some(l) if !l.hidden => l.active,
            _ => self.leaf(Pane::Code).map_or(Pane::Code, |l| l.active),
        }
    }

    /// The panes whose body `layout(ws, keep)` draws, by `Pane::index`:
    /// the front of each group not collapsed to its strip whose body is
    /// high enough for a line.
    pub fn bodies(&self, ws: [f32; 4], keep: Pane) -> [bool; 7] {
        let mut out = [false; 7];
        for l in self.layout(ws, keep).1 {
            if l.has_body() {
                out[l.active.index()] = true;
            }
        }
        out
    }

    /// `p` to the front of its group, the group shown.
    pub fn activate(&mut self, p: Pane) -> bool {
        let Some(l) = self.leaf_mut(p) else {
            return false;
        };
        let changed = l.active != p || l.hidden;
        l.active = p;
        l.hidden = false;
        changed
    }

    /// `p` at the front of its group, shown; a closed Library or Docs comes
    /// back at its home: with the other of the two, else in a column at
    /// the right taking 0.3 of the tab.
    pub fn show(&mut self, p: Pane) -> bool {
        if self.has(p) {
            return self.activate(p);
        }
        let Some(other) = other_page(p) else {
            return false;
        };
        if self.has(other) {
            let at = self.leaf_tabs(other).len();
            self.moved(p, DropAt::Into { anchor: other, at }, 0.3)
        } else {
            self.moved(p, DropAt::Edge(Side::Right), 0.3)
        }
    }

    /// `show`, but a closed Library or Docs whose home would take a pane's
    /// place in `ws` (or leave it no room itself) goes among the code's
    /// tabs, at the front. Without `ws` (the Editor tab never drawn) it is
    /// `show`.
    pub fn show_fitting(&mut self, p: Pane, ws: Option<[f32; 4]>, keep: Pane) -> bool {
        let Some(ws) = ws.filter(|_| !self.has(p)) else {
            return self.show(p);
        };
        let mut home = self.clone();
        if !home.show(p) {
            return false;
        }
        if fits(self, &home, ws, keep, p, p) {
            *self = home;
            return true;
        }
        let at = self.leaf_tabs(Pane::Code).len();
        self.move_pane(p, DropAt::Into { anchor: Pane::Code, at })
    }

    /// Takes the Library or the Docs out of the dock; the tab before it
    /// comes to the front of its group.
    pub fn close(&mut self, p: Pane) -> bool {
        if !p.closable() || !self.has(p) {
            return false;
        }
        self.take(p);
        self.tidy();
        true
    }

    /// Hides or shows `p`'s group, in place. The code's group never hides.
    pub fn set_hidden(&mut self, p: Pane, hidden: bool) -> bool {
        let Some(l) = self.leaf_mut(p) else {
            return false;
        };
        if l.hidden == hidden || (hidden && l.tabs.contains(&Pane::Code)) {
            return false;
        }
        l.hidden = hidden;
        true
    }

    /// The ratio of the split at `path`, kept between 0.05 and 0.95 and
    /// rounded to a thousandth (so it reads back from the text as it was).
    pub fn set_ratio(&mut self, path: u16, r: f32) -> bool {
        if !r.is_finite() || path == 0 {
            return false;
        }
        let mut node = &mut self.root;
        for i in (0..15 - path.leading_zeros()).rev() {
            node = match node {
                Node::Split { a, b, .. } => {
                    if (path >> i) & 1 == 0 {
                        a
                    } else {
                        b
                    }
                }
                Node::Leaf(_) => return false,
            };
        }
        let Node::Split { ratio, .. } = node else {
            return false;
        };
        let r = clean(r);
        if *ratio == r {
            return false;
        }
        *ratio = r;
        true
    }

    /// Whether the groups, their places and sizes are the default's,
    /// whichever tab is at the front of each: a reset would bring nothing
    /// back.
    pub fn is_default(&self) -> bool {
        fn fronts(node: &mut Node) {
            match node {
                Node::Leaf(l) => l.active = l.tabs.first().copied().unwrap_or(l.active),
                Node::Split { a, b, .. } => {
                    fronts(a);
                    fronts(b);
                }
            }
        }
        let mut d = self.clone();
        fronts(&mut d.root);
        d == Dock::default()
    }

    /// The tab as it always was.
    pub fn reset(&mut self) -> bool {
        let default = Dock::default();
        if *self == default {
            return false;
        }
        *self = default;
        true
    }

    /// Moves `p` to `at`: out of its group (a group it empties goes, its
    /// neighbour taking the room), then among the tabs there (at the front,
    /// the group shown), beside a group (halving its room) or along an edge
    /// (taking a quarter of the tab). A closed Library or Docs comes in. A
    /// move to a group not in the dock, or one that leaves every split and
    /// group as it was (whatever the ratios), changes nothing.
    pub fn move_pane(&mut self, p: Pane, at: DropAt) -> bool {
        self.moved(p, at, 0.25)
    }

    /// `move_pane`, an edge's new column or row taking `edge` of the tab.
    fn moved(&mut self, p: Pane, at: DropAt, edge: f32) -> bool {
        let mut next = self.clone();
        if !next.put(p, at, edge) {
            return false;
        }
        next.tidy();
        if same_shape(&next.root, &self.root) {
            return false;
        }
        *self = next;
        true
    }

    fn put(&mut self, p: Pane, at: DropAt, edge: f32) -> bool {
        // A group named by `p` itself is named by another of its tabs,
        // which stays when `p` leaves it.
        let anchor = match at {
            DropAt::Into { anchor, .. } | DropAt::Beside { anchor, .. } if anchor == p => {
                match self.leaf_tabs(p).iter().find(|t| **t != p) {
                    Some(t) => Some(*t),
                    None => return false,
                }
            }
            DropAt::Into { anchor, .. } | DropAt::Beside { anchor, .. } if !self.has(anchor) => return false,
            DropAt::Into { anchor, .. } | DropAt::Beside { anchor, .. } => Some(anchor),
            DropAt::Edge(_) => None,
        };
        self.take(p);
        let new = group(&[p]);
        match (at, anchor) {
            (DropAt::Into { at, .. }, Some(anchor)) => {
                let Some(l) = self.leaf_mut(anchor) else {
                    return false;
                };
                l.tabs.insert(at.min(l.tabs.len()), p);
                l.active = p;
                l.hidden = false;
            }
            (DropAt::Beside { side, .. }, Some(anchor)) => {
                let Some(slot) = node_with(&mut self.root, anchor) else {
                    return false;
                };
                let old = std::mem::replace(slot, hole());
                *slot = pair(side, 0.5, new, old);
            }
            (DropAt::Edge(side), _) => {
                let old = std::mem::replace(&mut self.root, hole());
                self.root = pair(side, edge, new, old);
            }
            _ => return false,
        }
        true
    }

    /// Takes `p` out of its group: the tab before it comes to the front if
    /// it was, and a group it empties goes.
    fn take(&mut self, p: Pane) {
        let root = std::mem::replace(&mut self.root, hole());
        self.root = take_from(root, p).unwrap_or_else(hole);
    }

    /// Keeps the dock's rules after an edit or a read: each pane once (the
    /// first kept), no empty group (its split gives way to the other
    /// side), the front one of its group's tabs, the code's group shown,
    /// ratios between 0.05 and 0.95 in thousandths. False when one of the
    /// five panes that are always there is missing.
    fn tidy(&mut self) -> bool {
        let mut seen = [false; 7];
        let root = std::mem::replace(&mut self.root, hole());
        self.root = tidy_node(root, &mut seen).unwrap_or_else(hole);
        PANES[..5].iter().all(|p| seen[p.index()])
    }

    /// Where `dragged`, its tab dragged to `(x, y)` over the groups as the
    /// last frame drew them in `ws`, would land: among a strip's tabs where
    /// the pointer is, along the left, right or bottom edge, beside a group
    /// in its outer quarter, or among its tabs in its middle. A move that
    /// would take a pane's place (laid out with the keys where they would
    /// be) lands among the tabs of the group under the pointer instead;
    /// one that changes nothing, or still takes a place, lands nowhere.
    pub fn drop_at(
        &self,
        ws: [f32; 4],
        leaves: &[DrawnLeaf],
        dragged: Pane,
        focus: Pane,
        (x, y): (f32, f32),
    ) -> Option<DropSpot> {
        if !inside(ws, (x, y)) {
            return None;
        }
        let others = |l: &DrawnLeaf| -> Vec<(Pane, [f32; 4])> {
            l.tabs.iter().copied().filter(|(t, _)| *t != dragged).collect()
        };
        let last = |l: &DrawnLeaf| -> Option<DropAt> {
            let tabs = others(l);
            Some(DropAt::Into {
                anchor: tabs.first()?.0,
                at: tabs.len(),
            })
        };
        // Over a strip: among its tabs, before the first one whose middle
        // is right of the pointer, with a bar there.
        if let Some(l) = leaves.iter().find(|l| inside(l.strip, (x, y))) {
            let tabs = others(l);
            let anchor = tabs.first()?.0;
            let at = tabs
                .iter()
                .position(|(_, r)| r[0] + r[2] / 2.0 > x)
                .unwrap_or(tabs.len());
            let edge = match tabs.get(at) {
                Some((_, r)) => r[0],
                None => tabs.last().map_or(l.strip[0], |(_, r)| r[0] + r[2]),
            };
            let at = DropAt::Into { anchor, at };
            return match self.trial(ws, dragged, focus, at) {
                Trial::Lands(rect) => Some(DropSpot {
                    at,
                    rect,
                    bar: Some([edge - 1.0, l.strip[1] + 4.0, 2.0, l.strip[3] - 8.0]),
                }),
                Trial::Crowds | Trial::Nothing => None,
            };
        }
        let under = leaves.iter().find(|l| !l.collapsed && inside(l.rect, (x, y)));
        // Along an edge (not the top, which the strips line).
        let edges = [
            (x - ws[0], Side::Left),
            (ws[0] + ws[2] - x, Side::Right),
            (ws[1] + ws[3] - y, Side::Bottom),
        ];
        let near = edges
            .into_iter()
            .filter(|(d, _)| *d < EDGE)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let at = match (near, under) {
            (Some((_, side)), _) => DropAt::Edge(side),
            (None, Some(l)) => {
                let r = l.rect;
                let (d, side) = [
                    ((x - r[0]) / r[2], Side::Left),
                    ((r[0] + r[2] - x) / r[2], Side::Right),
                    ((y - r[1]) / r[3], Side::Top),
                    ((r[1] + r[3] - y) / r[3], Side::Bottom),
                ]
                .into_iter()
                .min_by(|a, b| a.0.total_cmp(&b.0))?;
                if d < 0.25 {
                    DropAt::Beside {
                        anchor: others(l).first()?.0,
                        side,
                    }
                } else {
                    last(l)?
                }
            }
            // A bar's gap.
            (None, None) => return None,
        };
        match self.trial(ws, dragged, focus, at) {
            Trial::Lands(rect) => Some(DropSpot { at, rect, bar: None }),
            Trial::Crowds if !matches!(at, DropAt::Into { .. }) => {
                let at = last(under?)?;
                match self.trial(ws, dragged, focus, at) {
                    Trial::Lands(rect) => Some(DropSpot { at, rect, bar: None }),
                    Trial::Crowds | Trial::Nothing => None,
                }
            }
            Trial::Crowds | Trial::Nothing => None,
        }
    }

    /// `move_pane(dragged, at)` on a copy, laid out at `ws` with the keys
    /// where the drop puts them.
    fn trial(&self, ws: [f32; 4], dragged: Pane, focus: Pane, at: DropAt) -> Trial {
        let mut after = self.clone();
        if !after.move_pane(dragged, at) {
            return Trial::Nothing;
        }
        let keep = if dragged.takes_keys() {
            dragged
        } else {
            after.settled(focus)
        };
        if !fits(self, &after, ws, focus, keep, dragged) {
            return Trial::Crowds;
        }
        match after.layout(ws, keep).1.into_iter().find(|l| l.tabs.contains(&dragged)) {
            Some(l) => Trial::Lands(l.rect),
            None => Trial::Crowds,
        }
    }

    /// The dock as `layout.txt` keeps it: `row:` or `col:` and a ratio for
    /// a split, its sides in brackets; `[pane,pane]` for a group, `*` after
    /// its front pane and `-` before a hidden one.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        write_node(&self.root, &mut out);
        out
    }

    /// A dock from its text, tidied (unknown and repeated panes dropped,
    /// empty groups gone, ratios kept in range), or None where it is too
    /// long, nests too deep, isn't the format (a ratio is digits, with a
    /// point and digits or not) or misses one of the five panes that are
    /// always there.
    pub fn parse(text: &str) -> Option<Dock> {
        if text.len() > MOST_TEXT {
            return None;
        }
        let s: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
        let mut depth = 0usize;
        for b in &s {
            match b {
                b'(' if depth == MOST_DEPTH => return None,
                b'(' => depth += 1,
                b')' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        let mut at = 0;
        let root = parse_node(&s, &mut at)?;
        if at != s.len() {
            return None;
        }
        let mut dock = Dock { root };
        dock.tidy().then_some(dock)
    }
}

/// Whether `after`, laid out at `ws` with `keep_after`, gives `p` a body,
/// and a place (whole or its strip) to every pane `now` gives one laid out
/// with `keep_now`: so a drop, a move or a pane shown never pushes another
/// out.
pub fn fits(now: &Dock, after: &Dock, ws: [f32; 4], keep_now: Pane, keep_after: Pane, p: Pane) -> bool {
    let (_, before) = now.layout(ws, keep_now);
    let (_, placed) = after.layout(ws, keep_after);
    placed.iter().any(|l| l.active == p && l.has_body())
        && before
            .iter()
            .flat_map(|l| &l.tabs)
            .all(|q| placed.iter().any(|l| l.tabs.contains(q)))
}

/// `layout.txt` in the studio's folder: the dock, and whether the tip that
/// tabs can be dragged still shows (until a tab or a bar is first moved).
/// No file, another version, a file too big to be one or a damaged dock
/// gives the default, the tip showing.
pub fn load(dir: &Path) -> (Dock, bool) {
    let fallback = (Dock::default(), true);
    if dir.as_os_str().is_empty() {
        return fallback;
    }
    let path = dir.join("layout.txt");
    match std::fs::metadata(&path) {
        Ok(m) if m.is_file() && m.len() <= MOST_FILE => {}
        _ => return fallback,
    }
    let Ok(text) = std::fs::read_to_string(&path) else {
        return fallback;
    };
    let (mut version, mut dock, mut tips) = (None, None, true);
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        match k.trim() {
            "version" => version = Some(v.trim()),
            "dock" => dock = Some(v.trim()),
            "tips" => tips = v.trim() != "0",
            _ => {}
        }
    }
    if version != Some("1") {
        return fallback;
    }
    match dock.and_then(Dock::parse) {
        Some(dock) => (dock, tips),
        None => fallback,
    }
}

/// What `save` writes.
pub fn file_text(dock: &Dock, tips: bool) -> String {
    let mut text = format!(
        "# Horizon Studio's Editor panes. Delete this file for the default.\nversion=1\ndock={}\n",
        dock.to_text()
    );
    if !tips {
        text.push_str("tips=0\n");
    }
    text
}

/// Writes `layout.txt`; nothing for a studio that never started (no folder).
pub fn save(dir: &Path, dock: &Dock, tips: bool) {
    if dir.as_os_str().is_empty() {
        return;
    }
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(dir.join("layout.txt"), file_text(dock, tips));
}

fn group(tabs: &[Pane]) -> Node {
    Node::Leaf(Leaf {
        tabs: tabs.to_vec(),
        active: tabs.first().copied().unwrap_or_default(),
        hidden: false,
    })
}

/// An empty group, for a moment where a node is being rebuilt; tidy
/// takes it away.
fn hole() -> Node {
    group(&[])
}

/// The Library for the Docs and the Docs for the Library, whose home
/// is with each other.
fn other_page(p: Pane) -> Option<Pane> {
    match p {
        Pane::Library => Some(Pane::Docs),
        Pane::Docs => Some(Pane::Library),
        _ => None,
    }
}

/// A split of `new` and `old`, `new` on `side` taking `share` of it.
fn pair(side: Side, share: f32, new: Node, old: Node) -> Node {
    let first = matches!(side, Side::Left | Side::Top);
    let (a, b, ratio) = if first {
        (new, old, share)
    } else {
        (old, new, 1.0 - share)
    };
    Node::Split {
        row: matches!(side, Side::Left | Side::Right),
        ratio,
        a: Box::new(a),
        b: Box::new(b),
    }
}

/// The group node holding `p`.
fn node_with(node: &mut Node, p: Pane) -> Option<&mut Node> {
    if matches!(node, Node::Leaf(l) if l.tabs.contains(&p)) {
        return Some(node);
    }
    match node {
        Node::Split { a, b, .. } => match node_with(a, p) {
            Some(n) => Some(n),
            None => node_with(b, p),
        },
        Node::Leaf(_) => None,
    }
}

fn take_from(node: Node, p: Pane) -> Option<Node> {
    match node {
        Node::Leaf(mut l) => {
            if let Some(i) = l.tabs.iter().position(|t| *t == p) {
                l.tabs.remove(i);
                if l.active == p
                    && let Some(t) = l.tabs.get(i.saturating_sub(1)).copied()
                {
                    l.active = t;
                }
            }
            (!l.tabs.is_empty()).then_some(Node::Leaf(l))
        }
        Node::Split { row, ratio, a, b } => match (take_from(*a, p), take_from(*b, p)) {
            (Some(a), Some(b)) => Some(Node::Split {
                row,
                ratio,
                a: Box::new(a),
                b: Box::new(b),
            }),
            (one, None) | (None, one) => one,
        },
    }
}

fn tidy_node(node: Node, seen: &mut [bool; 7]) -> Option<Node> {
    match node {
        Node::Leaf(mut l) => {
            l.tabs.retain(|p| !std::mem::replace(&mut seen[p.index()], true));
            let first = *l.tabs.first()?;
            if !l.tabs.contains(&l.active) {
                l.active = first;
            }
            l.hidden &= !l.tabs.contains(&Pane::Code);
            Some(Node::Leaf(l))
        }
        Node::Split { row, ratio, a, b } => match (tidy_node(*a, seen), tidy_node(*b, seen)) {
            (Some(a), Some(b)) => Some(Node::Split {
                row,
                ratio: clean(ratio),
                a: Box::new(a),
                b: Box::new(b),
            }),
            (one, None) | (None, one) => one,
        },
    }
}

fn clean(r: f32) -> f32 {
    let r = if r.is_finite() { r } else { 0.5 };
    (r.clamp(0.05, 0.95) * 1000.0).round() / 1000.0
}

/// Whether two trees have the same splits and groups, whatever their ratios.
fn same_shape(a: &Node, b: &Node) -> bool {
    match (a, b) {
        (Node::Leaf(x), Node::Leaf(y)) => x == y,
        (
            Node::Split {
                row: r1, a: a1, b: b1, ..
            },
            Node::Split {
                row: r2, a: a2, b: b2, ..
            },
        ) => r1 == r2 && same_shape(a1, a2) && same_shape(b1, b2),
        _ => false,
    }
}

/// Whether anything of `node` shows: a group in it not hidden.
fn shows(node: &Node) -> bool {
    match node {
        Node::Leaf(l) => !l.hidden,
        Node::Split { a, b, .. } => shows(a) || shows(b),
    }
}

/// Whether `p` is in a shown group of `node`.
fn holds(node: &Node, p: Pane) -> bool {
    match node {
        Node::Leaf(l) => !l.hidden && l.tabs.contains(&p),
        Node::Split { a, b, .. } => holds(a, p) || holds(b, p),
    }
}

/// The least room `node` takes across a row (its width) or down a
/// column: nothing hidden, a group its largest pane's (and across, what
/// its strip's tabs take), a split along its own way both sides and the
/// bar between (one side when the other is hidden), across it the larger
/// side.
fn least(node: &Node, row: bool) -> f32 {
    match node {
        Node::Leaf(l) if l.hidden => 0.0,
        Node::Leaf(l) if row => l.tabs.iter().map(|p| p.least()[0]).fold(strip_least(&l.tabs), f32::max),
        Node::Leaf(l) => l.tabs.iter().map(|p| p.least()[1]).fold(0.0, f32::max),
        Node::Split { row: r, a, b, .. } => match (shows(a), shows(b)) {
            (false, false) => 0.0,
            (true, false) => least(a, row),
            (false, true) => least(b, row),
            (true, true) if *r == row => least(a, row) + BAR + least(b, row),
            (true, true) => least(a, row).max(least(b, row)),
        },
    }
}

/// The width a group's strip takes with each tab at its least (its label
/// cut to 28 units, its padding, the gap after it, and what follows the
/// label: the problems' count, the agent's busy dot, the code's unsaved
/// dot), its margins and its × (none on the code's group or the script
/// list's alone): so a group of many tabs keeps them on its strip.
fn strip_least(tabs: &[Pane]) -> f32 {
    let marks: f32 = tabs
        .iter()
        .map(|p| match p {
            Pane::Problems => 30.0,
            Pane::Agent | Pane::Code => 14.0,
            _ => 0.0,
        })
        .sum();
    let cross = !tabs.contains(&Pane::Code) && tabs != [Pane::Scripts];
    16.0 + 54.0 * tabs.len() as f32 + marks + if cross { 30.0 } else { 0.0 }
}

/// The part of `r` from `pos`, `len` long, across a row or down a column.
fn along(r: [f32; 4], row: bool, pos: f32, len: f32) -> [f32; 4] {
    if row {
        [pos, r[1], len, r[3]]
    } else {
        [r[0], pos, r[2], len]
    }
}

fn place(node: &Node, r: [f32; 4], path: u16, keep: Pane, out: &mut (Vec<Bar>, Vec<Placed>)) {
    let (row, ratio, a, b) = match node {
        Node::Leaf(l) => {
            if !l.hidden {
                out.1.push(Placed {
                    rect: r,
                    tabs: l.tabs.clone(),
                    active: l.active,
                    collapsed: false,
                });
            }
            return;
        }
        Node::Split { row, ratio, a, b } => (*row, *ratio, &**a, &**b),
    };
    let (pa, pb) = (path.wrapping_mul(2), path.wrapping_mul(2).wrapping_add(1));
    match (shows(a), shows(b)) {
        (false, false) => return,
        (true, false) => return place(a, r, pa, keep, out),
        (false, true) => return place(b, r, pb, keep, out),
        (true, true) => {}
    }
    let (start, extent) = if row { (r[0], r[2]) } else { (r[1], r[3]) };
    let len = (extent - BAR).max(0.0);
    let (la, lb) = (least(a, row), least(b, row));
    if len >= la + lb {
        // Room for both: a's share, kept between the sides' least.
        let hi = (len - lb).floor().max(la);
        let ratio = if ratio.is_finite() { ratio } else { 0.5 };
        let al = (len * ratio).round().max(la).min(hi);
        out.0.push(Bar {
            path,
            rect: along(r, row, start + al, BAR),
            row,
            start,
            len,
            lo: start + la,
            hi: start + hi,
        });
        place(a, along(r, row, start, al), pa, keep, out);
        place(b, along(r, row, start + al + BAR, len - al), pb, keep, out);
        return;
    }
    // Cramped: the side with the keys (else the code) takes the room, and
    // a group stacked against it keeps its strip where that leaves the
    // side its least.
    let a_wins = if holds(a, keep) || holds(b, keep) {
        holds(a, keep)
    } else {
        holds(a, Pane::Code) || !holds(b, Pane::Code)
    };
    let (win, lose, pw) = if a_wins { (a, b, pa) } else { (b, a, pb) };
    let strip = match lose {
        Node::Leaf(l) if !row && extent >= least(win, row) + BAR + STRIP => Some(l),
        _ => None,
    };
    let Some(l) = strip else {
        return place(win, r, pw, keep, out);
    };
    let wl = extent - BAR - STRIP;
    let collapsed = |at: f32| Placed {
        rect: along(r, row, at, STRIP),
        tabs: l.tabs.clone(),
        active: l.active,
        collapsed: true,
    };
    if a_wins {
        place(win, along(r, row, start, wl), pw, keep, out);
        out.1.push(collapsed(start + wl + BAR));
    } else {
        out.1.push(collapsed(start));
        place(win, along(r, row, start + STRIP + BAR, wl), pw, keep, out);
    }
}

fn write_node(node: &Node, out: &mut String) {
    match node {
        Node::Leaf(l) => {
            if l.hidden {
                out.push('-');
            }
            out.push('[');
            for (i, p) in l.tabs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(p.id());
                if *p == l.active {
                    out.push('*');
                }
            }
            out.push(']');
        }
        Node::Split { row, ratio, a, b } => {
            out.push_str(if *row { "row:" } else { "col:" });
            out.push_str(&format!("{ratio:.3}("));
            write_node(a, out);
            out.push(',');
            write_node(b, out);
            out.push(')');
        }
    }
}

/// Takes `c` if it is next.
fn eat(s: &[u8], at: &mut usize, c: u8) -> bool {
    let next = s.get(*at) == Some(&c);
    if next {
        *at += 1;
    }
    next
}

/// Takes `c`, which must be next.
fn expect(s: &[u8], at: &mut usize, c: u8) -> Option<()> {
    eat(s, at, c).then_some(())
}

fn parse_node(s: &[u8], at: &mut usize) -> Option<Node> {
    let rest = &s[*at..];
    if rest.starts_with(b"row:") || rest.starts_with(b"col:") {
        let row = rest[0] == b'r';
        *at += 4;
        let ratio = parse_ratio(s, at)?;
        expect(s, at, b'(')?;
        let a = parse_node(s, at)?;
        expect(s, at, b',')?;
        let b = parse_node(s, at)?;
        expect(s, at, b')')?;
        return Some(Node::Split {
            row,
            ratio,
            a: Box::new(a),
            b: Box::new(b),
        });
    }
    let hidden = eat(s, at, b'-');
    expect(s, at, b'[')?;
    let (mut tabs, mut active) = (Vec::new(), None);
    loop {
        let start = *at;
        while s.get(*at).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_') {
            *at += 1;
        }
        if *at == start {
            return None;
        }
        let id = std::str::from_utf8(&s[start..*at]).ok();
        let star = eat(s, at, b'*');
        // Unknown names are dropped (a newer studio's panes, say).
        if let Some(p) = id.and_then(Pane::from_id) {
            tabs.push(p);
            if star && active.is_none() {
                active = Some(p);
            }
        }
        if eat(s, at, b']') {
            break;
        }
        expect(s, at, b',')?;
    }
    let active = active.or(tabs.first().copied()).unwrap_or_default();
    Some(Node::Leaf(Leaf { tabs, active, hidden }))
}

/// Digits, with a point and digits or not: no sign, exponent, `nan` or
/// `inf`, which Rust's float parsing would take.
fn parse_ratio(s: &[u8], at: &mut usize) -> Option<f32> {
    let start = *at;
    let digits = |at: &mut usize| {
        let from = *at;
        while s.get(*at).is_some_and(u8::is_ascii_digit) {
            *at += 1;
        }
        *at > from
    };
    if !digits(at) || (eat(s, at, b'.') && !digits(at)) {
        return None;
    }
    let r: f32 = std::str::from_utf8(&s[start..*at]).ok()?.parse().ok()?;
    r.is_finite().then_some(r)
}

fn inside(r: [f32; 4], (x, y): (f32, f32)) -> bool {
    x >= r[0] && y >= r[1] && x < r[0] + r[2] && y < r[1] + r[3]
}
