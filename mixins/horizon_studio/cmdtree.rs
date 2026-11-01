// The chat's command tree with the studio's commands in it. Horizon's chat
// parses, colours and completes a command line with the tree the server sent
// (Remote.chat_tree, the game's own before one comes). Each frame
// hstudio_bridge puts the commands the studio takes under that tree's root,
// so they are listed, coloured and completed like the server's, and the
// chat's completion requests for them stay in the game, where the studio
// answers them. This module decides what to do to the tree and which
// requests are the studio's; it reaches the tree only through `Tree`, which
// the bridge implements over Horizon's dispatcher and the tests over a fake.
//
// The tree is changed in place and only grows while a screen may hold a
// parse of it (the chat screen keeps one with node numbers until the text
// changes): nodes are added, the root's children are swapped, and nodes that
// nothing links to any more are dropped only when no screen can point at
// them. Redirects to the root (`execute ... run`, a server's own) are pointed
// at a copy of the root with the server's commands alone, as the server runs
// what follows them.

use super::hstudio_chatcmd::{ArgKind, ChatCommand, split_line};

/// A node to add to the tree.
pub enum NewNode<'a> {
    /// The copy of the root that redirects to the root reach instead, with
    /// these children (the server's commands).
    RootCopy(Vec<usize>),
    /// A command's name.
    Literal(&'a str),
    /// An argument: its name and kind, and whether its completions come
    /// from the studio (the chat then asks for them).
    Argument { name: &'a str, kind: ArgKind, asks: bool },
}

/// The chat's command tree, as the studio changes it. Node 0 is the root.
pub trait Tree {
    /// How many nodes it has.
    fn size(&self) -> usize;
    /// Where its nodes are in memory: a tree the server sends anew is
    /// somewhere else (or has another size or other children at the root).
    fn addr(&self) -> usize;
    /// A node's children.
    fn children(&self, node: usize) -> &[usize];
    fn set_children(&mut self, node: usize, children: Vec<usize>);
    /// A literal node's name; None for an argument or a root.
    fn literal(&self, node: usize) -> Option<&str>;
    /// The node a node redirects to.
    fn redirect(&self, node: usize) -> Option<usize>;
    fn set_redirect(&mut self, node: usize, to: usize);
    /// Adds a node, as the last child of `parent` when given; a line that
    /// ends at it runs the command when `executes`. Returns its number.
    fn push(&mut self, parent: Option<usize>, node: NewNode<'_>, executes: bool) -> usize;
    /// Drops the nodes from `size` on.
    fn truncate(&mut self, size: usize);
    /// Where the chat completes `line` (a command up to the cursor, with
    /// its `/`), as its own parse finds it: the node whose children complete
    /// it, and the byte the text completed starts at.
    fn locate(&self, line: &str) -> (usize, usize);
}

/// What the studio did to one tree.
struct Merged {
    /// The tree it is: where its nodes were, how many, and its root's
    /// children after the studio last changed it.
    addr: usize,
    size: usize,
    root: Vec<usize>,
    /// The tree as the server sent it: its size and its root's children.
    server_size: usize,
    server_root: Vec<usize>,
    /// The root's copy, made with the first command added.
    copy: Option<usize>,
    /// Every command added and its nodes (its literal, then each argument),
    /// under the root still or not.
    built: Vec<(ChatCommand, Vec<usize>)>,
    /// The commands under the root now.
    linked: Vec<ChatCommand>,
}

impl Merged {
    fn new(tree: &dyn Tree) -> Merged {
        Merged {
            addr: tree.addr(),
            size: tree.size(),
            root: tree.children(0).to_vec(),
            server_size: tree.size(),
            server_root: tree.children(0).to_vec(),
            copy: None,
            built: Vec::new(),
            linked: Vec::new(),
        }
    }
    /// Whether `tree` is the tree this is about: the server sent none since.
    fn is(&self, tree: &dyn Tree) -> bool {
        tree.addr() == self.addr && tree.size() == self.size && tree.children(0) == self.root.as_slice()
    }
    /// The first node the studio may drop: the server's and the root's copy stay.
    fn base(&self) -> usize {
        self.server_size + usize::from(self.copy.is_some())
    }
    /// The argument (from 0) the chat completes under `parent`, when that
    /// is one of the studio's nodes: argument n under a command's literal
    /// (n = 0) or under its argument n - 1.
    fn argument_under(&self, parent: usize) -> Option<usize> {
        self.built
            .iter()
            .find_map(|(_, nodes)| nodes.iter().position(|&n| n == parent))
    }
}

/// A completion request the chat made for a command the studio takes.
pub struct Asked {
    pub id: i32,
    /// The command up to the cursor, with its `/`.
    pub line: String,
    /// The argument the chat completes (from 0) and the byte its text starts
    /// at, when the chat's parse reached one of the studio's nodes; None
    /// when it reached none of them (the tree does not have the command
    /// yet), and gets no completions from the studio.
    pub at: Option<(usize, usize)>,
    /// The tree it was made in.
    generation: u64,
}

/// The studio's side of the window's chat tree.
pub struct ChatSync {
    merged: Option<Merged>,
    /// The commands wanted, and the stamp they were read at
    /// (`Studio::command_stamp`; None for none at all).
    stamp: Option<u64>,
    wanted: Vec<ChatCommand>,
    /// Counts the trees seen: a request is answered only in the one it was
    /// made in.
    generation: u64,
    asked: Option<Asked>,
}

impl Default for ChatSync {
    fn default() -> Self {
        ChatSync::new()
    }
}

impl ChatSync {
    pub const fn new() -> ChatSync {
        ChatSync {
            merged: None,
            stamp: None,
            wanted: Vec::new(),
            generation: 0,
            asked: None,
        }
    }

    /// The commands the tree is to have: read with `commands` when `stamp`
    /// (the studio's command stamp, None for no commands at all) is not the
    /// one they were last read at.
    pub fn want(&mut self, stamp: Option<u64>, commands: impl FnOnce() -> Vec<ChatCommand>) {
        if stamp != self.stamp {
            self.wanted = if stamp.is_some() { commands() } else { Vec::new() };
            self.stamp = stamp;
        }
    }

    /// The commands under the root now (none before a tree, or in a tree
    /// that just came).
    pub fn linked(&self) -> &[ChatCommand] {
        self.merged.as_ref().map_or(&[], |m| &m.linked)
    }

    /// Each frame: puts the commands wanted under the root of `tree` (the
    /// chat's, None while there is none) and takes away those no longer
    /// wanted, leaving the root's other children, a server's root argument
    /// among them. A command hides the server's command of its name, which
    /// the studio takes in its place. `quiet`: no screen holds a parse of the
    /// tree (the chat shows no command, no command block's command is being
    /// edited), so the nodes nothing links to any more can go.
    pub fn sync(&mut self, tree: Option<&mut dyn Tree>, quiet: bool) {
        let Some(tree) = tree else {
            if self.merged.take().is_some() {
                self.asked = None;
            }
            return;
        };
        if !self.merged.as_ref().is_some_and(|m| m.is(tree)) {
            // A tree the studio hasn't seen: the server sent it anew, and a
            // request made in the one before is not answered.
            self.generation += 1;
            self.asked = None;
            self.merged = Some(Merged::new(tree));
        }
        let m = self.merged.as_mut().expect("made above");
        let wanted = &self.wanted;
        let unlinked = m.built.len() > m.linked.len();
        if m.linked == *wanted && !(quiet && unlinked) {
            return;
        }
        if quiet && tree.size() > m.base() {
            tree.truncate(m.base());
            m.built.clear();
        }
        if !wanted.is_empty() && m.copy.is_none() {
            m.copy = Some(copy_root(tree, m));
        }
        let mut ours = Vec::with_capacity(wanted.len());
        for c in wanted {
            let literal = match m.built.iter().find(|(b, _)| b == c) {
                Some((_, nodes)) => nodes[0],
                None => {
                    let nodes = add_command(tree, c);
                    let literal = nodes[0];
                    m.built.push((c.clone(), nodes));
                    literal
                }
            };
            ours.push(literal);
        }
        // Only the server's literals of those names go: a root argument
        // (an old server's, through ViaProxy) is what lines naming no
        // literal parse as.
        let mut root: Vec<usize> = m
            .server_root
            .iter()
            .copied()
            .filter(|&n| !tree.literal(n).is_some_and(|l| wanted.iter().any(|c| c.name == l)))
            .collect();
        root.extend(ours);
        tree.set_children(0, root.clone());
        m.linked = wanted.clone();
        m.root = root;
        m.addr = tree.addr();
        m.size = tree.size();
    }

    /// A completion request the chat made (`line`: the command up to the
    /// cursor, with its `/`): whether it is the studio's, and so never sent
    /// to the server. It is when the cursor is past the command's name and
    /// the studio takes the command the line names (`takes`) or the tree has
    /// that command from it. It is kept to answer, unless the tree changed
    /// since the chat parsed the line (the server sent another): an answer
    /// would reach a parse of a tree that is gone, so it is dropped.
    pub fn request(&mut self, tree: Option<&dyn Tree>, id: i32, line: &str, takes: bool) -> bool {
        // While the name is being typed (an old server's root argument,
        // through ViaProxy, asks then), the line holds that name alone: it
        // goes to the server as before, and the chat lists the server's
        // commands that start so beside the studio's own.
        let Some((name, Some(_))) = split_line(line) else {
            return false;
        };
        if !takes && !self.linked().iter().any(|c| c.name == name) {
            return false;
        }
        self.asked = match (tree, &self.merged) {
            (Some(tree), Some(m)) if m.is(tree) => {
                let (parent, start) = tree.locate(line);
                Some(Asked {
                    id,
                    line: line.to_owned(),
                    at: m.argument_under(parent).map(|arg| (arg, start)),
                    generation: self.generation,
                })
            }
            _ => None,
        };
        true
    }

    /// The request to answer now, if the tree is still the one it was made in.
    pub fn take_asked(&mut self, tree: Option<&dyn Tree>) -> Option<Asked> {
        let asked = self.asked.take()?;
        let same = asked.generation == self.generation && tree.zip(self.merged.as_ref()).is_some_and(|(t, m)| m.is(t));
        same.then_some(asked)
    }
}

/// Adds the root's copy, with the server's commands, and points every
/// redirect to the root at it.
fn copy_root(tree: &mut dyn Tree, m: &Merged) -> usize {
    let copy = tree.push(None, NewNode::RootCopy(m.server_root.clone()), false);
    for n in 1..m.server_size {
        if tree.redirect(n) == Some(0) {
            tree.set_redirect(n, copy);
        }
    }
    copy
}

/// Adds a command's nodes, which nothing links to yet: its literal, then
/// its arguments, each under the one before; each node where the command
/// may end runs it. Returns them in that order.
fn add_command(tree: &mut dyn Tree, c: &ChatCommand) -> Vec<usize> {
    let mut nodes = vec![tree.push(None, NewNode::Literal(&c.name), c.can_end(0))];
    for (i, a) in c.args.iter().enumerate() {
        let parent = nodes[nodes.len() - 1];
        let node = NewNode::Argument {
            name: &a.name,
            kind: a.kind,
            asks: a.asks,
        };
        nodes.push(tree.push(Some(parent), node, c.can_end(i + 1)));
    }
    nodes
}
