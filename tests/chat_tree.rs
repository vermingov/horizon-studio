// The studio's commands in the chat's command tree (hstudio_cmdtree), on a
// fake of Horizon's dispatcher: what joins the root and what it hides, the
// root's copy for redirects, telling a new server tree from the merged one,
// dropping nodes only when no screen can point at them, and which completion
// requests stay in the game.

mod common;

use common::{FakeGame, studio};
use horizon_studio::game::hstudio_chatcmd::{ArgKind, ChatArg, ChatCommand};
use horizon_studio::game::hstudio_cmdtree::{ChatSync, NewNode, Tree};

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Root,
    Literal(String),
    Argument { name: String, kind: ArgKind, asks: bool },
}

#[derive(Clone, Debug)]
struct Node {
    kind: Kind,
    children: Vec<usize>,
    redirect: Option<usize>,
    executes: bool,
}

/// A tree as Horizon's dispatcher holds one: numbered nodes, node 0 the root.
struct FakeTree {
    nodes: Vec<Node>,
    /// Where its nodes are: a tree sent anew is somewhere else, unless a
    /// test puts it at the same place.
    at: usize,
}

impl Tree for FakeTree {
    fn size(&self) -> usize {
        self.nodes.len()
    }
    fn addr(&self) -> usize {
        self.at
    }
    fn children(&self, node: usize) -> &[usize] {
        self.nodes.get(node).map_or(&[], |n| n.children.as_slice())
    }
    fn set_children(&mut self, node: usize, children: Vec<usize>) {
        self.nodes[node].children = children;
    }
    fn literal(&self, node: usize) -> Option<&str> {
        match &self.nodes.get(node)?.kind {
            Kind::Literal(l) => Some(l.as_str()),
            _ => None,
        }
    }
    fn redirect(&self, node: usize) -> Option<usize> {
        self.nodes.get(node)?.redirect
    }
    fn set_redirect(&mut self, node: usize, to: usize) {
        self.nodes[node].redirect = Some(to);
    }
    fn push(&mut self, parent: Option<usize>, node: NewNode<'_>, executes: bool) -> usize {
        let (kind, children) = match node {
            NewNode::RootCopy(children) => (Kind::Root, children),
            NewNode::Literal(name) => (Kind::Literal(name.into()), Vec::new()),
            NewNode::Argument { name, kind, asks } => (
                Kind::Argument {
                    name: name.into(),
                    kind,
                    asks,
                },
                Vec::new(),
            ),
        };
        self.nodes.push(Node {
            kind,
            children,
            redirect: None,
            executes,
        });
        let id = self.nodes.len() - 1;
        if let Some(p) = parent {
            self.nodes[p].children.push(id);
        }
        id
    }
    fn truncate(&mut self, size: usize) {
        self.nodes.truncate(size.max(1));
    }
    /// Words between single spaces: each goes to a literal child of its
    /// name, else to the first argument child (a text one takes the rest);
    /// a redirect goes on from its target.
    fn locate(&self, line: &str) -> (usize, usize) {
        let (mut node, mut at) = (0, 1);
        loop {
            let rest = &line[at..];
            let Some(space) = rest.find(' ') else {
                return (node, at);
            };
            let word = &rest[..space];
            let kids = &self.nodes[node].children;
            let next = kids
                .iter()
                .copied()
                .find(|&c| self.nodes[c].kind == Kind::Literal(word.into()))
                .or_else(|| {
                    kids.iter()
                        .copied()
                        .find(|&c| matches!(self.nodes[c].kind, Kind::Argument { .. }))
                });
            let Some(next) = next else {
                return (node, at);
            };
            if matches!(
                self.nodes[next].kind,
                Kind::Argument {
                    kind: ArgKind::Text,
                    ..
                }
            ) {
                return (node, at);
            }
            at += space + 1;
            node = self.nodes[next].redirect.unwrap_or(next);
        }
    }
}

fn literal(name: &str, children: Vec<usize>) -> Node {
    Node {
        kind: Kind::Literal(name.into()),
        children,
        redirect: None,
        executes: true,
    }
}

/// A server's tree: /give <targets>, /execute run (to the root), the
/// server's own /wp, and a root argument, as ViaProxy gives old servers.
fn server_tree(at: usize) -> FakeTree {
    let mut run = literal("run", Vec::new());
    run.redirect = Some(0);
    let mut root = literal("", vec![1, 3, 5, 6]);
    root.kind = Kind::Root;
    FakeTree {
        nodes: vec![
            root,
            literal("give", vec![2]),
            Node {
                kind: Kind::Argument {
                    name: "targets".into(),
                    kind: ArgKind::Player,
                    asks: false,
                },
                children: Vec::new(),
                redirect: None,
                executes: true,
            },
            literal("execute", vec![4]),
            run,
            literal("wp", Vec::new()),
            Node {
                kind: Kind::Argument {
                    name: "args".into(),
                    kind: ArgKind::Text,
                    asks: true,
                },
                children: Vec::new(),
                redirect: None,
                executes: true,
            },
        ],
        at,
    }
}

fn cmd(name: &str, args: &[(&str, ArgKind, bool, bool)]) -> ChatCommand {
    ChatCommand {
        name: name.into(),
        args: args
            .iter()
            .map(|&(n, kind, optional, asks)| ChatArg {
                name: n.into(),
                kind,
                optional,
                asks,
            })
            .collect(),
    }
}

/// /hello [<args>] and /wp [<action>] [<name>].
fn commands() -> Vec<ChatCommand> {
    vec![
        cmd("hello", &[("args", ArgKind::Text, true, false)]),
        cmd(
            "wp",
            &[
                ("action", ArgKind::Word, true, true),
                ("name", ArgKind::Text, true, false),
            ],
        ),
    ]
}

/// The names of a node's children.
fn names(t: &FakeTree, node: usize) -> Vec<String> {
    t.nodes[node]
        .children
        .iter()
        .map(|&c| match &t.nodes[c].kind {
            Kind::Literal(l) => l.clone(),
            Kind::Argument { name, .. } => format!("<{name}>"),
            Kind::Root => "(root)".into(),
        })
        .collect()
}

/// The root child named `name` that the studio added.
fn ours(t: &FakeTree, name: &str) -> usize {
    t.nodes[0]
        .children
        .iter()
        .copied()
        .find(|&c| c > 6 && t.nodes[c].kind == Kind::Literal(name.into()))
        .unwrap_or_else(|| panic!("no /{name} of the studio's"))
}

fn merged(stamp: u64, list: Vec<ChatCommand>) -> (ChatSync, FakeTree) {
    let mut tree = server_tree(1000);
    let mut sync = ChatSync::new();
    sync.want(Some(stamp), || list);
    sync.sync(Some(&mut tree), false);
    (sync, tree)
}

#[test]
fn the_studios_commands_join_the_servers() {
    let (sync, tree) = merged(1, commands());
    // The server's own /wp is hidden (the studio takes it), its root
    // argument stays.
    assert_eq!(names(&tree, 0), ["give", "execute", "<args>", "hello", "wp"]);
    assert_eq!(sync.linked(), commands().as_slice());
    let wp = ours(&tree, "wp");
    assert!(tree.nodes[wp].executes, "its arguments are optional");
    let action = tree.nodes[wp].children[0];
    assert_eq!(
        tree.nodes[action].kind,
        Kind::Argument {
            name: "action".into(),
            kind: ArgKind::Word,
            asks: true
        }
    );
    assert!(tree.nodes[action].executes);
    let name = tree.nodes[action].children[0];
    assert!(matches!(
        tree.nodes[name].kind,
        Kind::Argument {
            kind: ArgKind::Text,
            asks: false,
            ..
        }
    ));
    // /execute run reaches the root's copy: the server's commands only.
    let copy = tree.nodes[4].redirect.expect("run redirects");
    assert_ne!(copy, 0);
    assert_eq!(names(&tree, copy), ["give", "execute", "wp", "<args>"]);
    // The server's nodes are as they were.
    assert_eq!(names(&tree, 1), ["<targets>"]);
}

#[test]
fn nothing_is_done_while_the_commands_stay() {
    let (mut sync, mut tree) = merged(1, commands());
    let size = tree.size();
    let root = tree.nodes[0].children.clone();
    for quiet in [false, true, false] {
        sync.want(Some(1), || panic!("the stamp is the same"));
        sync.sync(Some(&mut tree), quiet);
    }
    assert_eq!(tree.size(), size);
    assert_eq!(tree.nodes[0].children, root);
}

#[test]
fn commands_are_read_again_only_when_their_stamp_changes() {
    let mut sync = ChatSync::new();
    let mut reads = 0;
    for stamp in [Some(1), Some(1), Some(2), Some(2), None, None, Some(2)] {
        sync.want(stamp, || {
            reads += 1;
            commands()
        });
    }
    assert_eq!(reads, 3);
}

#[test]
fn nodes_go_only_when_no_screen_can_point_at_them() {
    let (mut sync, mut tree) = merged(1, commands());
    let first = tree.size();
    let old_wp = ours(&tree, "wp");
    // /wp changes shape while the chat shows a command: its old nodes stay,
    // so a parse the chat holds still points at nodes that exist.
    let mut changed = commands();
    changed[1] = cmd("wp", &[("action", ArgKind::Word, false, true)]);
    sync.want(Some(2), || changed.clone());
    sync.sync(Some(&mut tree), false);
    assert!(tree.size() > first);
    let new_wp = ours(&tree, "wp");
    assert_ne!(new_wp, old_wp);
    assert!(!tree.nodes[new_wp].executes, "its argument is required");
    assert_eq!(names(&tree, old_wp), ["<action>"], "kept as it was");
    // The chat closes: what nothing links to goes, the copy stays.
    sync.sync(Some(&mut tree), true);
    let copy = tree.nodes[4].redirect.unwrap();
    assert_eq!(copy, 7, "the copy follows the server's nodes");
    assert_eq!(
        tree.size(),
        8 + 2 + 2,
        "the copy, hello and its argument, wp and its argument"
    );
    assert_eq!(names(&tree, 0), ["give", "execute", "<args>", "hello", "wp"]);
    assert!(!tree.nodes[ours(&tree, "wp")].executes);
    assert_eq!(names(&tree, copy), ["give", "execute", "wp", "<args>"]);
    // Again with nothing to drop: nothing changes.
    let size = tree.size();
    sync.sync(Some(&mut tree), true);
    assert_eq!(tree.size(), size);
}

#[test]
fn without_commands_the_root_is_the_servers_again() {
    let (mut sync, mut tree) = merged(1, commands());
    let size = tree.size();
    // A command block's command is edited (or the studio is off).
    sync.want(None, Vec::new);
    sync.sync(Some(&mut tree), false);
    assert_eq!(names(&tree, 0), ["give", "execute", "wp", "<args>"]);
    assert!(sync.linked().is_empty());
    assert_eq!(tree.size(), size, "nodes stay while a screen is open");
    // Back: the nodes built are used again.
    sync.want(Some(1), commands);
    sync.sync(Some(&mut tree), false);
    assert_eq!(names(&tree, 0), ["give", "execute", "<args>", "hello", "wp"]);
    assert_eq!(tree.size(), size);
    // Off and quiet: only the server's and the copy are left.
    sync.want(None, Vec::new);
    sync.sync(Some(&mut tree), true);
    assert_eq!(tree.size(), 8);
    assert_eq!(names(&tree, 0), ["give", "execute", "wp", "<args>"]);
}

#[test]
fn a_tree_the_server_sends_anew_is_merged_anew() {
    let (mut sync, tree) = merged(1, commands());
    assert!(sync.request(Some(&tree), 3, "/wp re", true));
    // Another tree comes before the frame answers: the request was parsed
    // in the old one, so it is dropped (and never sent).
    let mut new = server_tree(2000);
    sync.sync(Some(&mut new), false);
    assert!(sync.take_asked(Some(&new)).is_none());
    assert_eq!(names(&new, 0), ["give", "execute", "<args>", "hello", "wp"]);
    assert_ne!(new.nodes[4].redirect, Some(0));
    // A request made in the old tree, arriving after the new one came.
    let mut newer = server_tree(3000);
    assert!(sync.request(Some(&newer), 4, "/wp re", true));
    sync.sync(Some(&mut newer), false);
    assert!(sync.take_asked(Some(&newer)).is_none());
    // One made in the tree as it is gets its answer.
    assert!(sync.request(Some(&newer), 5, "/wp re", true));
    sync.sync(Some(&mut newer), false);
    assert_eq!(sync.take_asked(Some(&newer)).map(|a| a.id), Some(5));
    // Even one at the old tree's place, as long, is told apart by its root.
    let mut same_place = server_tree(3000);
    while same_place.size() < newer.size() {
        same_place.nodes.push(literal("filler", Vec::new()));
    }
    assert!(sync.request(Some(&same_place), 6, "/wp re", true));
    sync.sync(Some(&mut same_place), false);
    assert!(sync.take_asked(Some(&same_place)).is_none());
    assert!(names(&same_place, 0).contains(&"hello".to_owned()));
    // No tree (in-frame single player, before a server's first): nothing.
    sync.sync(None, false);
    assert!(sync.linked().is_empty());
    assert!(sync.request(None, 7, "/wp re", true), "still never sent");
    assert!(sync.take_asked(None).is_none());
}

#[test]
fn requests_for_the_studios_commands_stay_in_the_game() {
    let (mut sync, tree) = merged(1, commands());
    let ask = |sync: &mut ChatSync, line: &str, takes: bool| -> Option<Option<(usize, usize)>> {
        sync.request(Some(&tree), 1, line, takes)
            .then(|| sync.take_asked(Some(&tree)).map(|a| a.at).expect("kept to answer"))
    };
    assert_eq!(ask(&mut sync, "/wp re", true), Some(Some((0, 4))));
    assert_eq!(ask(&mut sync, "/wp add ho", true), Some(Some((1, 8))));
    assert_eq!(ask(&mut sync, "/hello there you", true), Some(Some((0, 7))));
    // The name being typed (a root argument asks): the studio's, nothing to add.
    assert_eq!(ask(&mut sync, "/wp", true), Some(None));
    // A command the tree has from the studio that its mod just dropped.
    assert_eq!(ask(&mut sync, "/hello x", false), Some(Some((0, 7))));
    // Everything else goes to the server, /execute run included: the
    // studio's commands are not under the root's copy.
    assert_eq!(ask(&mut sync, "/give @p", false), None);
    assert_eq!(ask(&mut sync, "/execute run wp re", false), None);
    assert_eq!(ask(&mut sync, "/w", false), None);
    assert_eq!(tree.locate("/execute run wp re"), (5, 16), "the server's /wp");
}

#[test]
fn the_studios_list_goes_into_the_tree() {
    let mut game = FakeGame::new("chat-tree");
    let mut st = studio(&mut game);
    st.run_console(
        &mut game,
        "command('wp', function() end, { args = { { name = 'action', options = { 'add' } }, { name = 'at', kind = 'pos', optional = true } } })",
    );
    let mut tree = server_tree(1000);
    let mut sync = ChatSync::new();
    sync.want(Some(st.command_stamp()), || st.chat_commands());
    sync.sync(Some(&mut tree), true);
    let root = names(&tree, 0);
    for name in ["studio", "lua", "hello", "wp"] {
        assert!(root.contains(&name.to_owned()), "{root:?}");
    }
    assert!(!root[..3].contains(&"wp".to_owned()), "the server's /wp is hidden");
    let wp = ours(&tree, "wp");
    assert!(!tree.nodes[wp].executes);
    let action = tree.nodes[wp].children[0];
    assert!(tree.nodes[action].executes);
    let at = tree.nodes[action].children[0];
    assert!(matches!(
        tree.nodes[at].kind,
        Kind::Argument {
            kind: ArgKind::Pos,
            asks: false,
            ..
        }
    ));
    // A request the chat makes is answered by the studio from where the parse put it.
    assert!(sync.request(Some(&tree), 9, "/wp a", st.takes("wp")));
    let asked = sync.take_asked(Some(&tree)).unwrap();
    let (arg, start) = asked.at.unwrap();
    let c = st.complete(&mut game, &asked.line, arg, start).unwrap();
    assert_eq!(c.list, vec![("add".to_owned(), None)]);
    assert_eq!((c.start, c.len), (4, 1));
}

/// The bundle declares the modules and the hook the game side needs (only
/// CI's `python3 tools/studio.py check` compiles them against Horizon).
#[test]
fn the_game_side_is_declared() {
    let manifest = include_str!("../mixins/horizon_studio/horizon-mixin.toml");
    for source in ["chatcmd.rs", "cmdtree.rs"] {
        assert!(
            manifest.contains(&format!("source = \"{source}\"")),
            "{source} is not mounted"
        );
    }
    // The chat's completion requests reach the bridge before the game sends them.
    let at = manifest.find("id = \"suggest\"").expect("the suggest injection");
    let end = manifest[at..].find("\n\n").map_or(manifest.len(), |e| at + e);
    let block = &manifest[at..end];
    for line in [
        "kind = \"HEAD\"",
        "cancellable = true",
        "target = \"horizon_client::game::remote::Game<'_>::send_suggestion_request\"",
        "helper = \"suggest.rs\"",
        "{ name = \"game\", ty = \"&mut crate::game::Game<'_>\", value = \"&mut *self\" }",
    ] {
        assert!(block.contains(line), "{line} is missing from {block}");
    }
    let helper = include_str!("../mixins/horizon_studio/suggest.rs");
    assert!(helper.contains("-> core::ops::ControlFlow<()>"));
    assert!(helper.contains("crate::game::hstudio_bridge::suggestion_request(game)"));
    let bridge = include_str!("../mixins/horizon_studio/bridge.rs");
    assert!(bridge.contains("pub(super) fn suggestion_request(game: &mut Game<'_>) -> bool"));
}
