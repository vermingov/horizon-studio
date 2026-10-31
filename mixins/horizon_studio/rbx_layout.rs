// GUI layout and hit-testing for the Roblox emulation layer
// (hstudio_rbx_layout), apart from any renderer: a tree of GuiObjects and
// their UI components goes in, and each one's AbsolutePosition and
// AbsoluteSize, clip rectangle, content area, wrapped text, the draw order
// and what is under a point come out. The tree is a plain arena of nodes
// with child lists, which the Instance model mirrors; a node holds every
// property any of its classes has, under the property's name, with
// Instance.new's defaults for its class.
//
// It follows the spec's algorithm: the parent's content rect, Size by
// SizeConstraint, UISizeConstraint, UIAspectRatioConstraint, AutomaticSize
// (the Size is a minimum), AnchorPoint, UIPadding, UIScale about the anchor,
// UIListLayout and UIGridLayout with their alignment, padding and SortOrder,
// ScrollingFrame canvases (CanvasSize, AutomaticCanvasSize, scroll bar
// insets, CanvasPosition clamped), ClipsDescendants, ZIndexBehavior Global
// and Sibling, and ScreenGui's DisplayOrder and IgnoreGuiInset. Rotation
// leaves the layout upright, as Roblox's is (and switches clipping off
// under it); each object gets the turns it is drawn with, and hits treat
// objects as upright. Coordinates are Roblox's
// GUI coordinates: the origin is under the top bar, so an IgnoreGuiInset
// ScreenGui starts at -inset.

use super::hstudio_rbx_types::{Font, Rect, UDim, UDim2, Vector2};
use std::ops::{Index, IndexMut};

/// A node's place in its Tree.
pub type NodeId = usize;

/// The top bar's height, GuiService:GetGuiInset()'s Y, since the 2023 redesign.
pub const TOP_BAR: f64 = 58.0;

// ---------------------------------------------------------------- classes and enums

/// The GUI classes laid out here. Folder stands for any other Instance
/// among them: it isn't drawn, and the GuiObjects in it are placed in its
/// nearest GUI ancestor (though no layout of that ancestor arranges them).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Class {
    ScreenGui,
    Frame,
    TextLabel,
    TextButton,
    TextBox,
    ImageLabel,
    ImageButton,
    ScrollingFrame,
    UIListLayout,
    UIGridLayout,
    UIPadding,
    UIAspectRatioConstraint,
    UISizeConstraint,
    UITextSizeConstraint,
    UIScale,
    UICorner,
    UIStroke,
    Folder,
}

impl Class {
    pub const ALL: [Class; 18] = [
        Class::ScreenGui,
        Class::Frame,
        Class::TextLabel,
        Class::TextButton,
        Class::TextBox,
        Class::ImageLabel,
        Class::ImageButton,
        Class::ScrollingFrame,
        Class::UIListLayout,
        Class::UIGridLayout,
        Class::UIPadding,
        Class::UIAspectRatioConstraint,
        Class::UISizeConstraint,
        Class::UITextSizeConstraint,
        Class::UIScale,
        Class::UICorner,
        Class::UIStroke,
        Class::Folder,
    ];

    /// Its ClassName.
    pub fn name(self) -> &'static str {
        match self {
            Class::ScreenGui => "ScreenGui",
            Class::Frame => "Frame",
            Class::TextLabel => "TextLabel",
            Class::TextButton => "TextButton",
            Class::TextBox => "TextBox",
            Class::ImageLabel => "ImageLabel",
            Class::ImageButton => "ImageButton",
            Class::ScrollingFrame => "ScrollingFrame",
            Class::UIListLayout => "UIListLayout",
            Class::UIGridLayout => "UIGridLayout",
            Class::UIPadding => "UIPadding",
            Class::UIAspectRatioConstraint => "UIAspectRatioConstraint",
            Class::UISizeConstraint => "UISizeConstraint",
            Class::UITextSizeConstraint => "UITextSizeConstraint",
            Class::UIScale => "UIScale",
            Class::UICorner => "UICorner",
            Class::UIStroke => "UIStroke",
            Class::Folder => "Folder",
        }
    }

    /// The class with this ClassName.
    pub fn from_name(name: &str) -> Option<Class> {
        Class::ALL.into_iter().find(|c| c.name() == name)
    }

    /// A GuiObject: drawn, sized and placed.
    pub fn is_gui_object(self) -> bool {
        matches!(
            self,
            Class::Frame
                | Class::TextLabel
                | Class::TextButton
                | Class::TextBox
                | Class::ImageLabel
                | Class::ImageButton
                | Class::ScrollingFrame
        )
    }

    /// TextLabel, TextButton or TextBox.
    pub fn has_text(self) -> bool {
        matches!(self, Class::TextLabel | Class::TextButton | Class::TextBox)
    }

    /// A GuiButton: TextButton or ImageButton.
    pub fn is_button(self) -> bool {
        matches!(self, Class::TextButton | Class::ImageButton)
    }
}

/// Enum.FillDirection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FillDirection {
    #[default]
    Horizontal,
    Vertical,
}

/// Enum.HorizontalAlignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HorizontalAlignment {
    Center,
    #[default]
    Left,
    Right,
}

/// Enum.VerticalAlignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum VerticalAlignment {
    Center,
    #[default]
    Top,
    Bottom,
}

/// Enum.SortOrder: how a layout orders its items.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SortOrder {
    /// By Name (byte order), ties in child order.
    #[default]
    Name,
    /// By LayoutOrder, ties in child order.
    LayoutOrder,
}

/// Enum.AutomaticSize.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[allow(clippy::upper_case_acronyms)]
pub enum AutomaticSize {
    #[default]
    None,
    X,
    Y,
    XY,
}

impl AutomaticSize {
    pub fn x(self) -> bool {
        matches!(self, AutomaticSize::X | AutomaticSize::XY)
    }

    pub fn y(self) -> bool {
        matches!(self, AutomaticSize::Y | AutomaticSize::XY)
    }
}

/// Enum.SizeConstraint: which parent lengths Size's scales use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SizeConstraint {
    #[default]
    RelativeXY,
    /// Both scales use the parent's width.
    RelativeXX,
    /// Both scales use the parent's height.
    RelativeYY,
}

/// Enum.ZIndexBehavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ZIndexBehavior {
    /// Every object in the ScreenGui sorted by ZIndex together.
    #[default]
    Global,
    /// Children over their parent, siblings sorted by ZIndex.
    Sibling,
}

/// Enum.AspectType.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AspectType {
    #[default]
    FitWithinMaxSize,
    ScaleWithParentSize,
}

/// Enum.DominantAxis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DominantAxis {
    #[default]
    Width,
    Height,
}

/// Enum.StartCorner: where a UIGridLayout puts its first cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StartCorner {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Enum.ScrollingDirection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[allow(clippy::upper_case_acronyms)]
pub enum ScrollingDirection {
    X,
    Y,
    #[default]
    XY,
}

/// Enum.ScrollBarInset: whether a scroll bar narrows the canvas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ScrollBarInset {
    #[default]
    None,
    /// While the bar shows.
    ScrollBar,
    Always,
}

/// Enum.VerticalScrollBarPosition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum VerticalScrollBarPosition {
    #[default]
    Right,
    Left,
}

/// Enum.TextXAlignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextXAlignment {
    Left,
    Right,
    #[default]
    Center,
}

/// Enum.TextYAlignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextYAlignment {
    Top,
    #[default]
    Center,
    Bottom,
}

/// Enum.ApplyStrokeMode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ApplyStrokeMode {
    #[default]
    Contextual,
    Border,
}

/// Enum.TextTruncate: how text that doesn't fit ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextTruncate {
    /// Drawn whole, past its area.
    #[default]
    None,
    /// Cut where the line and "..." still fit.
    AtEnd,
    /// Cut after the last whole word that fits with "...".
    SplitWord,
}

/// Where along a length an alignment puts things: 0 start, 0.5 middle, 1 end.
fn h_fraction(a: HorizontalAlignment) -> f64 {
    match a {
        HorizontalAlignment::Left => 0.0,
        HorizontalAlignment::Center => 0.5,
        HorizontalAlignment::Right => 1.0,
    }
}

fn v_fraction(a: VerticalAlignment) -> f64 {
    match a {
        VerticalAlignment::Top => 0.0,
        VerticalAlignment::Center => 0.5,
        VerticalAlignment::Bottom => 1.0,
    }
}

// ---------------------------------------------------------------- nodes

/// One Instance of the GUI tree. Each field is the Roblox property of the
/// same name (snake_case) on the classes that have it; the rest go unused.
#[derive(Clone, Debug)]
pub struct Node {
    pub class: Class,
    pub name: String,
    parent: Option<NodeId>,
    children: Vec<NodeId>,

    // GuiObject
    pub position: UDim2,
    pub size: UDim2,
    pub anchor_point: Vector2,
    /// Degrees clockwise about its centre: it turns the drawing, not the layout.
    pub rotation: f64,
    pub visible: bool,
    pub z_index: i32,
    pub layout_order: i32,
    pub automatic_size: AutomaticSize,
    pub size_constraint: SizeConstraint,
    pub clips_descendants: bool,
    /// Sinks input; buttons take clicks only when Active.
    pub active: bool,
    pub interactable: bool,
    /// Only its size is wanted (a partial layout's node): it is sized for
    /// its parent's layout, but neither it nor what is under it is placed.
    pub size_only: bool,

    // TextLabel, TextButton, TextBox
    pub text: String,
    pub text_size: f64,
    /// FontFace, which the measure callback is given.
    pub font: Font,
    pub text_scaled: bool,
    pub text_wrapped: bool,
    pub line_height: f64,
    pub text_x_alignment: TextXAlignment,
    pub text_y_alignment: TextYAlignment,
    pub text_truncate: TextTruncate,
    /// The characters drawn, from the start; -1 draws them all.
    pub max_visible_graphemes: i32,

    // ScrollingFrame
    pub canvas_size: UDim2,
    pub canvas_position: Vector2,
    pub automatic_canvas_size: AutomaticSize,
    pub scroll_bar_thickness: f64,
    pub scrolling_direction: ScrollingDirection,
    pub vertical_scroll_bar_inset: ScrollBarInset,
    pub horizontal_scroll_bar_inset: ScrollBarInset,
    pub vertical_scroll_bar_position: VerticalScrollBarPosition,

    // ScreenGui (and UIStroke's Enabled)
    pub enabled: bool,
    pub ignore_gui_inset: bool,
    pub display_order: i32,
    pub z_index_behavior: ZIndexBehavior,

    // UIListLayout and UIGridLayout
    pub fill_direction: FillDirection,
    pub horizontal_alignment: HorizontalAlignment,
    pub vertical_alignment: VerticalAlignment,
    pub sort_order: SortOrder,
    /// UIListLayout.Padding: the gap between items.
    pub padding: UDim,
    pub wraps: bool,
    pub cell_size: UDim2,
    pub cell_padding: UDim2,
    /// 0 is no limit.
    pub fill_direction_max_cells: u32,
    pub start_corner: StartCorner,

    // UIPadding
    pub padding_top: UDim,
    pub padding_bottom: UDim,
    pub padding_left: UDim,
    pub padding_right: UDim,

    // UIAspectRatioConstraint
    /// Width over height.
    pub aspect_ratio: f64,
    pub aspect_type: AspectType,
    pub dominant_axis: DominantAxis,

    // UISizeConstraint
    pub min_size: Vector2,
    pub max_size: Vector2,

    // UITextSizeConstraint
    pub min_text_size: f64,
    pub max_text_size: f64,

    // UIScale
    pub scale: f64,

    // UICorner
    pub corner_radius: UDim,

    // UIStroke
    pub thickness: f64,
    pub apply_stroke_mode: ApplyStrokeMode,
}

impl Node {
    /// A node as Instance.new makes one of `class`, outside any tree.
    pub fn new(class: Class) -> Node {
        let button = class.is_button() || class == Class::TextBox;
        let list = class == Class::UIListLayout;
        Node {
            class,
            name: class.name().to_string(),
            parent: None,
            children: Vec::new(),
            position: UDim2::default(),
            size: UDim2::default(),
            anchor_point: Vector2::ZERO,
            rotation: 0.0,
            visible: true,
            z_index: 1,
            layout_order: 0,
            automatic_size: AutomaticSize::None,
            size_constraint: SizeConstraint::RelativeXY,
            clips_descendants: class == Class::ScrollingFrame,
            active: button,
            interactable: true,
            size_only: false,
            text: match class {
                Class::TextLabel => "Label",
                Class::TextButton => "Button",
                Class::TextBox => "TextBox",
                _ => "",
            }
            .to_string(),
            text_size: 8.0,
            font: Font::default(),
            text_scaled: false,
            text_wrapped: false,
            line_height: 1.0,
            text_x_alignment: TextXAlignment::Center,
            text_y_alignment: TextYAlignment::Center,
            text_truncate: TextTruncate::None,
            max_visible_graphemes: -1,
            canvas_size: UDim2::new(0.0, 0.0, 2.0, 0.0),
            canvas_position: Vector2::ZERO,
            automatic_canvas_size: AutomaticSize::None,
            scroll_bar_thickness: 12.0,
            scrolling_direction: ScrollingDirection::XY,
            vertical_scroll_bar_inset: ScrollBarInset::None,
            horizontal_scroll_bar_inset: ScrollBarInset::None,
            vertical_scroll_bar_position: VerticalScrollBarPosition::Right,
            enabled: true,
            ignore_gui_inset: false,
            display_order: 0,
            z_index_behavior: ZIndexBehavior::Global,
            fill_direction: if list {
                FillDirection::Vertical
            } else {
                FillDirection::Horizontal
            },
            horizontal_alignment: HorizontalAlignment::Left,
            vertical_alignment: VerticalAlignment::Top,
            sort_order: SortOrder::Name,
            padding: UDim::default(),
            wraps: false,
            cell_size: UDim2::from_offset(100.0, 100.0),
            cell_padding: UDim2::from_offset(5.0, 5.0),
            fill_direction_max_cells: 0,
            start_corner: StartCorner::TopLeft,
            padding_top: UDim::default(),
            padding_bottom: UDim::default(),
            padding_left: UDim::default(),
            padding_right: UDim::default(),
            aspect_ratio: 1.0,
            aspect_type: AspectType::FitWithinMaxSize,
            dominant_axis: DominantAxis::Width,
            min_size: Vector2::ZERO,
            max_size: Vector2::new(f64::INFINITY, f64::INFINITY),
            min_text_size: 1.0,
            max_text_size: 100.0,
            scale: 1.0,
            corner_radius: UDim::new(0.0, 8.0),
            thickness: 1.0,
            apply_stroke_mode: ApplyStrokeMode::Contextual,
        }
    }

    pub fn parent(&self) -> Option<NodeId> {
        self.parent
    }

    pub fn children(&self) -> &[NodeId] {
        &self.children
    }
}

/// The GUI tree: an arena of nodes, each with its parent and children in
/// order. Removed nodes' ids are reused.
#[derive(Clone, Debug, Default)]
pub struct Tree {
    slots: Vec<Option<Node>>,
    free: Vec<NodeId>,
    roots: Vec<NodeId>,
}

impl Tree {
    pub fn new() -> Tree {
        Tree::default()
    }

    /// A new node of `class` with Instance.new's defaults, last among its
    /// parent's children (or a root, without one).
    pub fn add(&mut self, class: Class, parent: Option<NodeId>) -> NodeId {
        let node = Node::new(class);
        let id = match self.free.pop() {
            Some(id) => {
                self.slots[id] = Some(node);
                id
            }
            None => {
                self.slots.push(Some(node));
                self.slots.len() - 1
            }
        };
        self.roots.push(id);
        if parent.is_some() {
            // A fresh node can't make a cycle; a removed parent leaves it a root.
            let _ = self.set_parent(id, parent);
        }
        id
    }

    /// The node, unless it was removed.
    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.slots.get(id).and_then(Option::as_ref)
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.slots.get_mut(id).and_then(Option::as_mut)
    }

    /// The nodes without a parent, in the order they became roots.
    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    /// A node's parent.
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.get(id).and_then(|n| n.parent)
    }

    /// A node's children, in order.
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        self.get(id).map_or(&[], |n| &n.children)
    }

    /// How many nodes there are.
    pub fn len(&self) -> usize {
        self.slots.iter().flatten().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether `id` is `ancestor` or under it.
    pub fn is_descendant_of(&self, id: NodeId, ancestor: NodeId) -> bool {
        let mut at = Some(id);
        while let Some(n) = at {
            if n == ancestor {
                return true;
            }
            at = self.get(n).and_then(|node| node.parent);
        }
        false
    }

    /// Moves `id` to the end of `parent`'s children (or to the roots).
    /// Refused, and nothing moves, if `parent` is `id` or under it.
    pub fn set_parent(&mut self, id: NodeId, parent: Option<NodeId>) -> Result<(), String> {
        if self.get(id).is_none() {
            return Err(format!("node {id} doesn't exist"));
        }
        if let Some(p) = parent {
            if self.get(p).is_none() {
                return Err(format!("node {p} doesn't exist"));
            }
            if self.is_descendant_of(p, id) {
                return Err("a node can't be put inside itself".into());
            }
        }
        match self[id].parent {
            Some(old) => self[old].children.retain(|&c| c != id),
            None => self.roots.retain(|&r| r != id),
        }
        self[id].parent = parent;
        match parent {
            Some(p) => self[p].children.push(id),
            None => self.roots.push(id),
        }
        Ok(())
    }

    /// Removes the node and everything under it.
    pub fn remove(&mut self, id: NodeId) {
        if self.get(id).is_none() {
            return;
        }
        match self[id].parent {
            Some(p) => self[p].children.retain(|&c| c != id),
            None => self.roots.retain(|&r| r != id),
        }
        let mut stack = vec![id];
        while let Some(n) = stack.pop() {
            if let Some(node) = self.slots[n].take() {
                stack.extend(node.children);
                self.free.push(n);
            }
        }
    }

    /// Lays out every root ScreenGui on `view`. `measure(text, size, font)`
    /// gives the width of one line of text at a text size.
    pub fn layout(&self, view: &Viewport, measure: &mut dyn FnMut(&str, f64, &Font) -> f64) -> Layout {
        let mut pass = Pass {
            tree: self,
            measure,
            placed: vec![None; self.slots.len()],
        };
        for &root in &self.roots {
            if self[root].class == Class::ScreenGui {
                pass.screen_gui(root, view);
            }
        }
        let draw_order = pass.draw_order();
        Layout {
            placed: pass.placed,
            draw_order,
        }
    }
}

/// The node; panics if it was removed.
impl Index<NodeId> for Tree {
    type Output = Node;
    fn index(&self, id: NodeId) -> &Node {
        self.get(id).expect("a node of this tree")
    }
}

impl IndexMut<NodeId> for Tree {
    fn index_mut(&mut self, id: NodeId) -> &mut Node {
        self.get_mut(id).expect("a node of this tree")
    }
}

// ---------------------------------------------------------------- results

/// The screen the GUI is laid out on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    /// In GUI pixels.
    pub size: Vector2,
    /// The top bar's height, which ScreenGuis start under unless IgnoreGuiInset.
    pub inset: f64,
}

impl Viewport {
    pub fn new(width: f64, height: f64, inset: f64) -> Viewport {
        Viewport {
            size: Vector2::new(width, height),
            inset,
        }
    }
}

/// One line of laid-out text.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLine {
    pub text: String,
    /// Its top-left corner.
    pub position: Vector2,
    pub width: f64,
    /// The byte offset in the node's text of its first character (the
    /// spaces a wrap breaks at belong to no line).
    pub start: usize,
}

/// A text object's text, laid out in its content area.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLayout {
    /// The text size drawn (TextScaled's choice, UITextSizeConstraint applied).
    pub size: f64,
    /// TextBounds: the size of all the lines.
    pub bounds: Vector2,
    /// TextFits: whether that fits the content area.
    pub fits: bool,
    /// The lines drawn; a wrapped text drops those below its area. A
    /// TextTruncate ends the lines that don't fit with "...", and
    /// MaxVisibleGraphemes keeps only the characters it shows, each line
    /// where it is for the whole text.
    pub lines: Vec<TextLine>,
}

/// A rotation an object is drawn with: `node`'s Rotation, by `degrees`
/// clockwise about `centre`, the middle of that node's upright rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Turn {
    pub node: NodeId,
    pub centre: Vector2,
    pub degrees: f64,
}

/// Where a node ended up. Sizes and positions are on screen, with UIScale
/// applied. Layout nodes (UIListLayout, UIGridLayout) fill in only
/// `content_size` and the cell fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Placed {
    pub class: Option<Class>,
    /// AbsolutePosition: the top-left corner.
    pub position: Vector2,
    /// AbsoluteSize.
    pub size: Vector2,
    /// AbsoluteRotation: its Rotation and its ancestors', in degrees.
    pub rotation: f64,
    /// The rotations it is drawn with, innermost first: its own Rotation
    /// about its centre, then each turned ancestor's about that one's. Its
    /// rectangle is upright, as Roblox lays it out; these turn its drawing.
    pub turns: Vec<Turn>,
    /// Whether it is drawn: it and its ancestors Visible, under an enabled ScreenGui.
    pub shown: bool,
    /// The area its ancestors' ClipsDescendants leave it; None when nothing clips it.
    pub clip: Option<Rect>,
    /// Where its children and text go: inside UIPadding, or a ScrollingFrame's scrolled canvas.
    pub content: Rect,
    /// UIScale on it and its ancestors, multiplied.
    pub scale: f64,
    pub active: bool,
    pub interactable: bool,
    /// UICorner's radius in pixels, at most half the shorter side; 0 without one.
    pub corner_radius: f64,
    /// An enabled UIStroke's thickness in pixels; 0 without one.
    pub stroke: f64,
    pub text: Option<TextLayout>,
    /// ScrollingFrame: AbsoluteCanvasSize, AbsoluteWindowSize, and CanvasPosition clamped to the canvas.
    pub canvas_size: Vector2,
    pub window_size: Vector2,
    pub canvas_position: Vector2,
    /// UIListLayout and UIGridLayout: AbsoluteContentSize.
    pub content_size: Vector2,
    /// UIGridLayout: AbsoluteCellSize, and AbsoluteCellCount (cells across, rows down).
    pub cell_size: Vector2,
    pub cell_count: Vector2,
}

impl Placed {
    /// Its rectangle, before clipping.
    pub fn rect(&self) -> Rect {
        Rect::new(self.position, self.position + self.size)
    }

    /// The part of its rectangle that shows: clipped by its ancestors.
    pub fn visible_rect(&self) -> Rect {
        match self.clip {
            Some(c) => intersect(self.rect(), c),
            None => self.rect(),
        }
    }
}

fn intersect(a: Rect, b: Rect) -> Rect {
    Rect::new(a.min.max(b.min), a.max.min(b.max))
}

/// Whether `p` is in `r`, its top and left edges in and the others out.
fn contains(r: Rect, p: Vector2) -> bool {
    p.x >= r.min.x && p.x < r.max.x && p.y >= r.min.y && p.y < r.max.y
}

/// A laid-out tree: where each node is, and the draw order.
#[derive(Clone, Debug, Default)]
pub struct Layout {
    placed: Vec<Option<Placed>>,
    /// The GuiObjects drawn, bottom first.
    pub draw_order: Vec<NodeId>,
}

impl Layout {
    /// Where a node is: None for one not under a root ScreenGui, or a UI
    /// component other than a layout.
    pub fn get(&self, id: NodeId) -> Option<&Placed> {
        self.placed.get(id).and_then(Option::as_ref)
    }

    /// Every drawn GuiObject whose visible rectangle holds `point`, topmost
    /// first (LayerCollector:GetGuiObjectsAtPosition; MouseEnter and
    /// MouseLeave go by this, covered or not).
    pub fn objects_at(&self, point: Vector2) -> Vec<NodeId> {
        let over = |id: &NodeId| self.get(*id).is_some_and(|p| contains(p.visible_rect(), point));
        self.draw_order.iter().rev().copied().filter(over).collect()
    }

    /// The topmost Active object at `point`: the one that takes a click or
    /// the hover there.
    pub fn hit_test(&self, point: Vector2) -> Option<NodeId> {
        self.objects_at(point)
            .into_iter()
            .find(|&id| self.get(id).is_some_and(|p| p.active))
    }

    /// The button a click at `point` goes to: the topmost Active object, if
    /// it is a GuiButton that is Interactable.
    pub fn button_at(&self, point: Vector2) -> Option<NodeId> {
        let id = self.hit_test(point)?;
        let p = self.get(id)?;
        (p.class.is_some_and(Class::is_button) && p.interactable).then_some(id)
    }
}

// ---------------------------------------------------------------- the pass

/// An area in a container's own (unscaled) space.
#[derive(Clone, Copy, Debug, Default)]
struct Area {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Area {
    /// An area of `size` at the origin.
    fn sized(size: Vector2) -> Area {
        Area {
            x: 0.0,
            y: 0.0,
            w: size.x,
            h: size.y,
        }
    }

    fn size(self) -> Vector2 {
        Vector2::new(self.w, self.h)
    }

    /// Inset by [left, top, right, bottom].
    fn inset(self, [l, t, r, b]: [f64; 4]) -> Area {
        Area {
            x: self.x + l,
            y: self.y + t,
            w: self.w - l - r,
            h: self.h - t - b,
        }
    }
}

/// What a subtree inherits: how its space maps to the screen (UIScale),
/// its clip, and whether it is drawn.
#[derive(Clone, Copy, Debug)]
struct Context {
    /// screen = origin + k * local.
    k: f64,
    origin: Vector2,
    clip: Option<Rect>,
    shown: bool,
    rotation: f64,
    /// The GuiObject whose turns the subtree is drawn with too.
    turned_by: Option<NodeId>,
}

impl Context {
    fn point(&self, p: Vector2) -> Vector2 {
        self.origin + p * self.k
    }

    fn rect(&self, a: Area) -> Rect {
        Rect::new(
            self.point(Vector2::new(a.x, a.y)),
            self.point(Vector2::new(a.x + a.w, a.y + a.h)),
        )
    }

    /// The space scaled by `s` about `pivot` (in this space).
    fn scaled(self, s: f64, pivot: Vector2) -> Context {
        let origin = self.point(pivot) - pivot * (self.k * s);
        Context {
            k: self.k * s,
            origin,
            ..self
        }
    }
}

/// The text-size limits a text object has: its UITextSizeConstraint, or 1 to 100.
struct TextLimits {
    min: f64,
    max: f64,
    constrained: bool,
}

/// What a layout did with its container's items.
struct Arranged {
    items: Vec<(NodeId, Area)>,
    /// AbsoluteContentSize, in the container's space.
    content_size: Vector2,
    /// UIGridLayout's AbsoluteCellSize and AbsoluteCellCount.
    cell_size: Vector2,
    cell_count: Vector2,
}

struct Pass<'a> {
    tree: &'a Tree,
    measure: &'a mut dyn FnMut(&str, f64, &Font) -> f64,
    placed: Vec<Option<Placed>>,
}

impl<'a> Pass<'a> {
    fn node(&self, id: NodeId) -> &'a Node {
        let tree: &'a Tree = self.tree;
        &tree[id]
    }

    /// The first child of this class: the UI component that applies.
    fn component(&self, id: NodeId, class: Class) -> Option<&'a Node> {
        let tree: &'a Tree = self.tree;
        tree[id].children.iter().map(|&c| &tree[c]).find(|c| c.class == class)
    }

    /// The UIListLayout or UIGridLayout that arranges this container's children.
    fn arranger(&self, id: NodeId) -> Option<NodeId> {
        let layouts = [Class::UIListLayout, Class::UIGridLayout];
        self.node(id)
            .children
            .iter()
            .copied()
            .find(|&c| layouts.contains(&self.node(c).class))
    }

    /// The GuiObjects placed in a container: its children, and those in its
    /// Folders; `true` marks the direct children.
    fn gui_children(&self, id: NodeId) -> Vec<(NodeId, bool)> {
        let mut out = Vec::new();
        self.collect_gui(id, true, &mut out);
        out
    }

    fn collect_gui(&self, id: NodeId, direct: bool, out: &mut Vec<(NodeId, bool)>) {
        for &c in &self.node(id).children {
            match self.node(c).class {
                class if class.is_gui_object() => out.push((c, direct)),
                Class::Folder => self.collect_gui(c, false, out),
                _ => {}
            }
        }
    }

    /// UIPadding's [left, top, right, bottom] in pixels for an area of `size`.
    fn padding(&self, id: NodeId, size: Vector2) -> [f64; 4] {
        match self.component(id, Class::UIPadding) {
            Some(p) => [
                p.padding_left.resolve(size.x),
                p.padding_top.resolve(size.y),
                p.padding_right.resolve(size.x),
                p.padding_bottom.resolve(size.y),
            ],
            None => [0.0; 4],
        }
    }

    fn text_limits(&self, id: NodeId) -> TextLimits {
        match self.component(id, Class::UITextSizeConstraint) {
            Some(c) => TextLimits {
                min: c.min_text_size,
                max: c.max_text_size,
                constrained: true,
            },
            None => TextLimits {
                min: 1.0,
                max: 100.0,
                constrained: false,
            },
        }
    }

    /// UISizeConstraint's clamp; MinSize wins when it is over MaxSize.
    fn clamp_size(&self, id: NodeId, size: Vector2) -> Vector2 {
        match self.component(id, Class::UISizeConstraint) {
            Some(c) => size.min(c.max_size).max(c.min_size),
            None => size,
        }
    }

    /// UIAspectRatioConstraint: FitWithinMaxSize takes the largest rectangle
    /// of the ratio inside `size`; ScaleWithParentSize keeps the dominant
    /// axis and shrinks to fit the parent if it must.
    fn aspect(&self, id: NodeId, size: Vector2, parent: Vector2) -> Vector2 {
        let Some(c) = self.component(id, Class::UIAspectRatioConstraint) else {
            return size;
        };
        let ratio = c.aspect_ratio;
        if !(ratio > 0.0 && ratio.is_finite()) {
            return size;
        }
        let fit = |w: f64, h: f64| {
            if w > h * ratio {
                Vector2::new(h * ratio, h)
            } else {
                Vector2::new(w, w / ratio)
            }
        };
        match c.aspect_type {
            AspectType::FitWithinMaxSize => fit(size.x, size.y),
            AspectType::ScaleWithParentSize => {
                let kept = match c.dominant_axis {
                    DominantAxis::Width => Vector2::new(size.x, size.x / ratio),
                    DominantAxis::Height => Vector2::new(size.y * ratio, size.y),
                };
                if kept.x > parent.x || kept.y > parent.y {
                    fit(kept.x.min(parent.x), kept.y.min(parent.y))
                } else {
                    kept
                }
            }
        }
    }

    /// A GuiObject's size in a parent area of `parent` (before its own
    /// UIScale): Size by SizeConstraint, UISizeConstraint, the aspect ratio,
    /// then AutomaticSize growing it to its content, clamped again.
    fn size_of(&mut self, id: NodeId, parent: Vector2) -> Vector2 {
        let node = self.node(id);
        let reference = match node.size_constraint {
            SizeConstraint::RelativeXY => parent,
            SizeConstraint::RelativeXX => Vector2::new(parent.x, parent.x),
            SizeConstraint::RelativeYY => Vector2::new(parent.y, parent.y),
        };
        let auto = node.automatic_size;
        let size = node.size.resolve(reference);
        let size = self.clamp_size(id, size);
        let mut size = self.aspect(id, size, parent);
        if auto.x() || auto.y() {
            let extent = self.extent(id, size);
            if auto.x() {
                size.x = size.x.max(extent.x);
            }
            if auto.y() {
                size.y = size.y.max(extent.y);
            }
            size = self.clamp_size(id, size);
        }
        size
    }

    /// What AutomaticSize grows to: the text's bounds and the children's
    /// extent, plus padding, for an object of (minimum) `size`.
    fn extent(&mut self, id: NodeId, size: Vector2) -> Vector2 {
        let pad = self.padding(id, size);
        let inner = Area::sized(size).inset(pad).size();
        let mut extent = self.children_extent(id, inner);
        let node = self.node(id);
        if node.class.has_text() {
            let wrap = (node.text_wrapped && !node.automatic_size.x()).then_some(inner.x);
            let limits = self.text_limits(id);
            let mut text_size = node.text_size;
            if limits.constrained {
                text_size = text_size.min(limits.max).max(limits.min);
            }
            let (_, bounds) = self.wrap(&node.text, text_size, &node.font, node.line_height, wrap);
            extent = extent.max(bounds);
        }
        Vector2::new(extent.x + pad[0] + pad[2], extent.y + pad[1] + pad[3])
    }

    /// How far a container's children reach in an inner area of `inner`:
    /// its layout's AbsoluteContentSize, or the box from the content's
    /// corner to the visible children's far edges (a child sized by scale
    /// on an axis doesn't count on that one).
    fn children_extent(&mut self, id: NodeId, inner: Vector2) -> Vector2 {
        let area = Area::sized(inner);
        if let Some(layout) = self.arranger(id) {
            return self.arrange(id, layout, area).content_size;
        }
        let mut extent = Vector2::ZERO;
        for (child, _) in self.gui_children(id) {
            if !self.node(child).visible {
                continue;
            }
            let size = self.size_of(child, inner);
            let at = self.own_position(child, size, area);
            let scaled = self.node(child).size;
            if scaled.x.scale == 0.0 {
                extent.x = extent.x.max(at.x + size.x);
            }
            if scaled.y.scale == 0.0 {
                extent.y = extent.y.max(at.y + size.y);
            }
        }
        extent
    }

    /// Where a GuiObject's top-left goes by its own Position and AnchorPoint.
    fn own_position(&self, id: NodeId, size: Vector2, area: Area) -> Vector2 {
        let node = self.node(id);
        let p = node.position;
        Vector2::new(
            area.x + p.x.resolve(area.w) - node.anchor_point.x * size.x,
            area.y + p.y.resolve(area.h) - node.anchor_point.y * size.y,
        )
    }

    /// The items a layout arranges: the container's direct, visible
    /// GuiObjects, in SortOrder.
    fn layout_items(&self, container: NodeId, layout: NodeId) -> Vec<NodeId> {
        let mut items: Vec<NodeId> = self
            .gui_children(container)
            .into_iter()
            .filter(|&(c, direct)| direct && self.node(c).visible)
            .map(|(c, _)| c)
            .collect();
        match self.node(layout).sort_order {
            SortOrder::LayoutOrder => items.sort_by_key(|&c| self.node(c).layout_order),
            SortOrder::Name => items.sort_by(|&a, &b| self.node(a).name.cmp(&self.node(b).name)),
        }
        items
    }

    /// Arranges a container's items by its layout in `area`: each item's
    /// area, and the layout's AbsoluteContentSize.
    fn arrange(&mut self, container: NodeId, layout: NodeId, area: Area) -> Arranged {
        let items = self.layout_items(container, layout);
        if self.node(layout).class == Class::UIGridLayout {
            return self.arrange_grid(layout, &items, area);
        }
        let sizes: Vec<Vector2> = items.iter().map(|&c| self.size_of(c, area.size())).collect();
        let l = self.node(layout);
        let vertical = l.fill_direction == FillDirection::Vertical;
        // The fill axis is "main", the other "cross".
        let (main_len, cross_len) = if vertical { (area.h, area.w) } else { (area.w, area.h) };
        let main = |v: Vector2| if vertical { v.y } else { v.x };
        let cross = |v: Vector2| if vertical { v.x } else { v.y };
        let (main_align, cross_align) = if vertical {
            (v_fraction(l.vertical_alignment), h_fraction(l.horizontal_alignment))
        } else {
            (h_fraction(l.horizontal_alignment), v_fraction(l.vertical_alignment))
        };
        let gap = l.padding.resolve(main_len);

        // Lines: one, or as many as Wraps needs.
        let mut lines: Vec<Vec<usize>> = vec![Vec::new()];
        let mut used = 0.0;
        for (i, &s) in sizes.iter().enumerate() {
            let fresh = lines.last().is_none_or(Vec::is_empty);
            if l.wraps && !fresh && used + gap + main(s) > main_len {
                lines.push(Vec::new());
                used = 0.0;
            }
            let line = lines.last_mut().expect("a line");
            used += if line.is_empty() { main(s) } else { gap + main(s) };
            line.push(i);
        }
        let line_main = |line: &[usize]| {
            line.iter().map(|&i| main(sizes[i])).sum::<f64>() + gap * line.len().saturating_sub(1) as f64
        };
        let line_cross = |line: &[usize]| line.iter().map(|&i| cross(sizes[i])).fold(0.0, f64::max);
        let block_cross: f64 =
            lines.iter().map(|l| line_cross(l)).sum::<f64>() + gap * lines.len().saturating_sub(1) as f64;
        let content_main = lines.iter().map(|l| line_main(l)).fold(0.0, f64::max);

        let mut placed = Vec::with_capacity(items.len());
        let mut cross_at = if l.wraps {
            (cross_len - block_cross) * cross_align
        } else {
            0.0
        };
        for line in &lines {
            let (span, thick) = (line_main(line), line_cross(line));
            // Without Wraps each item aligns across the whole container.
            let band = if l.wraps { thick } else { cross_len };
            let mut main_at = (main_len - span) * main_align;
            for &i in line {
                let s = sizes[i];
                let c = cross_at + (band - cross(s)) * cross_align;
                let (x, y) = if vertical { (c, main_at) } else { (main_at, c) };
                placed.push((
                    items[i],
                    Area {
                        x: area.x + x,
                        y: area.y + y,
                        w: s.x,
                        h: s.y,
                    },
                ));
                main_at += main(s) + gap;
            }
            cross_at += thick + gap;
        }
        let content_size = if vertical {
            Vector2::new(block_cross, content_main)
        } else {
            Vector2::new(content_main, block_cross)
        };
        Arranged {
            items: placed,
            content_size,
            cell_size: Vector2::ZERO,
            cell_count: Vector2::ZERO,
        }
    }

    /// UIGridLayout: items take the cell size (their own UISizeConstraint and
    /// aspect ratio still apply), fill rows (or columns) from StartCorner, and
    /// the block aligns in the area.
    fn arrange_grid(&self, layout: NodeId, items: &[NodeId], area: Area) -> Arranged {
        let l = self.node(layout);
        let cell = l.cell_size.resolve(area.size());
        let pad = l.cell_padding.resolve(area.size());
        let horizontal = l.fill_direction == FillDirection::Horizontal;
        let (cell_main, pad_main, room) = if horizontal {
            (cell.x, pad.x, area.w)
        } else {
            (cell.y, pad.y, area.h)
        };
        let n = items.len();
        let mut per_line = if cell_main + pad_main > 0.0 {
            (((room + pad_main) / (cell_main + pad_main)).floor() as usize).max(1)
        } else {
            n.max(1)
        };
        if l.fill_direction_max_cells > 0 {
            per_line = per_line.min(l.fill_direction_max_cells as usize);
        }
        let lines = n.div_ceil(per_line);
        let across = n.min(per_line);
        let (cols, rows) = if horizontal { (across, lines) } else { (lines, across) };
        let span = |count: usize, cell: f64, pad: f64| {
            if count == 0 {
                0.0
            } else {
                count as f64 * cell + (count - 1) as f64 * pad
            }
        };
        let block = Vector2::new(span(cols, cell.x, pad.x), span(rows, cell.y, pad.y));
        let left = area.x + (area.w - block.x) * h_fraction(l.horizontal_alignment);
        let top = area.y + (area.h - block.y) * v_fraction(l.vertical_alignment);
        let (from_right, from_bottom) = match l.start_corner {
            StartCorner::TopLeft => (false, false),
            StartCorner::TopRight => (true, false),
            StartCorner::BottomLeft => (false, true),
            StartCorner::BottomRight => (true, true),
        };
        let mut placed = Vec::with_capacity(n);
        for (i, &item) in items.iter().enumerate() {
            let (along, line) = (i % per_line, i / per_line);
            let (col, row) = if horizontal { (along, line) } else { (line, along) };
            let mut x = col as f64 * (cell.x + pad.x);
            let mut y = row as f64 * (cell.y + pad.y);
            if from_right {
                x = block.x - x - cell.x;
            }
            if from_bottom {
                y = block.y - y - cell.y;
            }
            let size = self.clamp_size(item, cell);
            let size = self.aspect(item, size, area.size());
            placed.push((
                item,
                Area {
                    x: left + x,
                    y: top + y,
                    w: size.x,
                    h: size.y,
                },
            ));
        }
        let cell_count = Vector2::new(cols as f64, rows as f64);
        Arranged {
            items: placed,
            content_size: block,
            cell_size: cell,
            cell_count,
        }
    }

    /// A root ScreenGui: the viewport under the top bar (or all of it), its
    /// children placed in it.
    fn screen_gui(&mut self, id: NodeId, view: &Viewport) {
        let node = self.node(id);
        let top = if node.ignore_gui_inset { -view.inset } else { 0.0 };
        let area = Area {
            x: 0.0,
            y: top,
            w: view.size.x,
            h: view.size.y - view.inset - top,
        };
        let shown = node.enabled;
        let ctx = Context {
            k: 1.0,
            origin: Vector2::ZERO,
            clip: None,
            shown,
            rotation: 0.0,
            turned_by: None,
        };
        let content = area.inset(self.padding(id, area.size()));
        self.placed[id] = Some(Placed {
            class: Some(Class::ScreenGui),
            position: Vector2::new(area.x, area.y),
            size: area.size(),
            shown,
            content: ctx.rect(content),
            scale: 1.0,
            ..Placed::default()
        });
        self.place_children(id, content, ctx);
    }

    /// Sizes and places a container's GuiObjects in its content area.
    fn place_children(&mut self, id: NodeId, content: Area, ctx: Context) {
        let mut areas: Vec<(NodeId, Area)> = Vec::new();
        let mut arranged: Vec<NodeId> = Vec::new();
        if let Some(layout) = self.arranger(id) {
            let done = self.arrange(id, layout, content);
            arranged = done.items.iter().map(|&(c, _)| c).collect();
            self.placed[layout] = Some(Placed {
                class: Some(self.node(layout).class),
                content_size: done.content_size * ctx.k,
                cell_size: done.cell_size * ctx.k,
                cell_count: done.cell_count,
                scale: ctx.k,
                ..Placed::default()
            });
            areas.extend(done.items);
        }
        for (child, _) in self.gui_children(id) {
            if arranged.contains(&child) {
                continue;
            }
            let size = self.size_of(child, content.size());
            let at = self.own_position(child, size, content);
            areas.push((
                child,
                Area {
                    x: at.x,
                    y: at.y,
                    w: size.x,
                    h: size.y,
                },
            ));
        }
        for (child, area) in areas {
            if !self.node(child).size_only {
                self.place(child, area, ctx);
            }
        }
    }

    /// One GuiObject at `area` (in its parent's space), then its subtree.
    fn place(&mut self, id: NodeId, area: Area, parent: Context) {
        let node = self.node(id);
        let anchor = node.anchor_point;
        let mut ctx = parent;
        if let Some(s) = self.component(id, Class::UIScale).map(|c| c.scale)
            && s != 1.0
        {
            let pivot = Vector2::new(area.x + anchor.x * area.w, area.y + anchor.y * area.h);
            ctx = ctx.scaled(s, pivot);
        }
        let rect = ctx.rect(area);
        let rotation = parent.rotation + node.rotation;
        ctx.shown = parent.shown && node.visible;
        ctx.rotation = rotation;
        // ClipsDescendants is off under a rotation.
        if node.clips_descendants && rotation == 0.0 {
            ctx.clip = Some(match parent.clip {
                Some(c) => intersect(c, rect),
                None => rect,
            });
        }

        let turns = self.turns(id, rect, parent.turned_by);
        if !turns.is_empty() {
            ctx.turned_by = Some(id);
        }
        let mut placed = Placed {
            class: Some(node.class),
            position: rect.min,
            size: area.size() * ctx.k,
            rotation,
            turns,
            shown: ctx.shown,
            clip: parent.clip,
            scale: ctx.k,
            active: node.active,
            interactable: node.interactable,
            ..Placed::default()
        };
        let mut content = area.inset(self.padding(id, area.size()));
        if node.class == Class::ScrollingFrame {
            content = self.canvas(id, area, &mut placed, ctx.k);
        }
        placed.content = ctx.rect(content);
        if node.class.has_text() {
            placed.text = Some(self.text(id, content, &ctx));
        }
        let short = placed.size.x.min(placed.size.y);
        if let Some(corner) = self.component(id, Class::UICorner) {
            let radius = corner.corner_radius.scale * area.w.min(area.h) + f64::from(corner.corner_radius.offset);
            placed.corner_radius = (radius * ctx.k).min(short / 2.0).max(0.0);
        }
        if let Some(stroke) = self.component(id, Class::UIStroke).filter(|s| s.enabled) {
            placed.stroke = stroke.thickness * ctx.k;
        }
        self.placed[id] = Some(placed);
        self.place_children(id, content, ctx);
    }

    /// The turns a GuiObject at `rect` is drawn with: its own Rotation about
    /// its centre, then those of the turned object it is in.
    fn turns(&self, id: NodeId, rect: Rect, turned_by: Option<NodeId>) -> Vec<Turn> {
        let degrees = self.node(id).rotation;
        let mut turns = Vec::new();
        if degrees != 0.0 && degrees.is_finite() {
            turns.push(Turn {
                node: id,
                centre: (rect.min + rect.max) / 2.0,
                degrees,
            });
        }
        if let Some(p) = turned_by.and_then(|n| self.placed[n].as_ref()) {
            turns.extend(p.turns.iter().copied());
        }
        turns
    }

    /// A ScrollingFrame's canvas, as the area its children go in: CanvasSize
    /// against the window (the frame less the scroll bars it insets), never
    /// smaller than the window, grown by AutomaticCanvasSize, and moved by
    /// CanvasPosition clamped to it.
    fn canvas(&mut self, id: NodeId, area: Area, placed: &mut Placed, k: f64) -> Area {
        let node = self.node(id);
        let (canvas_size, automatic) = (node.canvas_size, node.automatic_canvas_size);
        let (thickness, direction) = (node.scroll_bar_thickness, node.scrolling_direction);
        let (v_inset, h_inset) = (node.vertical_scroll_bar_inset, node.horizontal_scroll_bar_inset);
        let (bar_left, wanted) = (
            node.vertical_scroll_bar_position == VerticalScrollBarPosition::Left,
            node.canvas_position,
        );

        let mut grown = None;
        let mut resolve = |pass: &mut Self, window: Vector2| {
            let mut canvas = canvas_size.resolve(window).max(window);
            if automatic.x() || automatic.y() {
                let extent = *grown.get_or_insert_with(|| {
                    let pad = pass.padding(id, canvas);
                    let inner = Area::sized(canvas).inset(pad).size();
                    let e = pass.children_extent(id, inner);
                    Vector2::new(e.x + pad[0] + pad[2], e.y + pad[1] + pad[3])
                });
                if automatic.x() {
                    canvas.x = canvas.x.max(extent.x);
                }
                if automatic.y() {
                    canvas.y = canvas.y.max(extent.y);
                }
            }
            canvas
        };
        let mut window = area.size();
        let canvas = resolve(self, window);
        let scrolls_y = matches!(direction, ScrollingDirection::Y | ScrollingDirection::XY) && canvas.y > window.y;
        let scrolls_x = matches!(direction, ScrollingDirection::X | ScrollingDirection::XY) && canvas.x > window.x;
        let inset = |mode: ScrollBarInset, shown: bool| {
            mode == ScrollBarInset::Always || (mode == ScrollBarInset::ScrollBar && shown)
        };
        let narrowed = inset(v_inset, scrolls_y);
        if narrowed {
            window.x = (window.x - thickness).max(0.0);
        }
        if inset(h_inset, scrolls_x) {
            window.y = (window.y - thickness).max(0.0);
        }
        let canvas = resolve(self, window);
        let limit = (canvas - window).max(Vector2::ZERO);
        let position = wanted.max(Vector2::ZERO).min(limit);
        let left = if bar_left && narrowed { thickness } else { 0.0 };
        placed.canvas_size = canvas * k;
        placed.window_size = window * k;
        placed.canvas_position = position;
        let canvas_area = Area {
            x: area.x + left - position.x,
            y: area.y - position.y,
            w: canvas.x,
            h: canvas.y,
        };
        canvas_area.inset(self.padding(id, canvas))
    }

    /// A text object's text in its content area.
    fn text(&mut self, id: NodeId, content: Area, ctx: &Context) -> TextLayout {
        let node = self.node(id);
        let (text, font) = (&node.text, &node.font);
        let (line_height, scaled) = (node.line_height, node.text_scaled);
        let wrapped = node.text_wrapped || scaled;
        let (x_align, y_align) = (node.text_x_alignment, node.text_y_alignment);
        let limits = self.text_limits(id);
        let width = wrapped.then_some(content.w);
        let size = if scaled {
            // TextScaled: the largest whole size, up to 100, whose wrapped text fits.
            let top = limits.max.min(100.0).floor();
            let bottom = limits.min.max(1.0).ceil().min(top);
            let mut size = bottom;
            let mut s = top;
            while s >= bottom {
                let (_, b) = self.wrap(text, s, font, line_height, width);
                if b.x <= content.w && b.y <= content.h {
                    size = s;
                    break;
                }
                s -= 1.0;
            }
            size
        } else if limits.constrained {
            node.text_size.min(limits.max).max(limits.min)
        } else {
            node.text_size
        };
        let (mut lines, bounds) = self.wrap(text, size, font, line_height, width);
        let fits = bounds.x <= content.w && bounds.y <= content.h;
        let step = size * line_height;
        let drawn = if wrapped && step > 0.0 {
            lines.len().min(((content.h / step) + 1e-9).floor().max(0.0) as usize)
        } else {
            lines.len()
        };
        if node.text_truncate != TextTruncate::None {
            let cut_below = drawn < lines.len();
            lines.truncate(drawn);
            self.truncate(&mut lines, cut_below, content.w, size, font, node.text_truncate);
        }
        let fx = match x_align {
            TextXAlignment::Left => 0.0,
            TextXAlignment::Center => 0.5,
            TextXAlignment::Right => 1.0,
        };
        let fy = match y_align {
            TextYAlignment::Top => 0.0,
            TextYAlignment::Center => 0.5,
            TextYAlignment::Bottom => 1.0,
        };
        let top = content.y + (content.h - drawn as f64 * step) * fy;
        let mut lines: Vec<TextLine> = lines
            .into_iter()
            .take(drawn)
            .enumerate()
            .map(|(i, (text, w, start))| TextLine {
                text,
                position: ctx.point(Vector2::new(content.x + (content.w - w) * fx, top + i as f64 * step)),
                width: w * ctx.k,
                start,
            })
            .collect();
        if let Ok(max) = usize::try_from(node.max_visible_graphemes) {
            self.visible(&mut lines, text, max, size, font, ctx.k);
        }
        TextLayout {
            size: size * ctx.k,
            bounds: bounds * ctx.k,
            fits,
            lines,
        }
    }

    /// Text broken into lines (at "\n", and at spaces, or inside a word too
    /// long for a line, when `width` is given), each with its width and
    /// the byte offset in `text` it starts at, and the bounds of them all.
    fn wrap(
        &mut self,
        text: &str,
        size: f64,
        font: &Font,
        line_height: f64,
        width: Option<f64>,
    ) -> (Vec<(String, f64, usize)>, Vector2) {
        let mut lines = Vec::new();
        let mut at = 0;
        for paragraph in text.split('\n') {
            match width {
                None => lines.push((paragraph.to_string(), at)),
                Some(w) => self.wrap_paragraph(paragraph, at, size, font, w, &mut lines),
            }
            at += paragraph.len() + 1;
        }
        let measured: Vec<(String, f64, usize)> = lines
            .into_iter()
            .map(|(line, start)| {
                let w = (self.measure)(&line, size, font);
                (line, w, start)
            })
            .collect();
        let widest = measured.iter().map(|l| l.1).fold(0.0, f64::max);
        let bounds = Vector2::new(widest, measured.len() as f64 * size * line_height);
        (measured, bounds)
    }

    /// A paragraph that starts at byte `at` of its text, wrapped to `width`:
    /// its lines and where each starts.
    fn wrap_paragraph(
        &mut self,
        paragraph: &str,
        at: usize,
        size: f64,
        font: &Font,
        width: f64,
        lines: &mut Vec<(String, usize)>,
    ) {
        let mut line = String::new();
        let mut start = at;
        let mut word_at = at;
        for word in paragraph.split(' ') {
            let here = word_at;
            word_at += word.len() + 1;
            if !line.is_empty() {
                let candidate = format!("{line} {word}");
                if (self.measure)(&candidate, size, font) <= width {
                    line = candidate;
                    continue;
                }
                lines.push((std::mem::take(&mut line), start));
            }
            if (self.measure)(word, size, font) <= width {
                line = word.to_string();
                start = here;
                continue;
            }
            // A word wider than the line breaks where it must.
            for (i, ch) in word.char_indices() {
                let mut candidate = line.clone();
                candidate.push(ch);
                if !line.is_empty() && (self.measure)(&candidate, size, font) > width {
                    lines.push((std::mem::take(&mut line), start));
                    line.push(ch);
                    start = here + i;
                } else {
                    if line.is_empty() {
                        start = here + i;
                    }
                    line = candidate;
                }
            }
        }
        lines.push((line, start));
    }

    /// TextTruncate: each line too wide for `width`, and the last one when
    /// lines below it are cut off, ends in "..." where that still fits.
    fn truncate(
        &mut self,
        lines: &mut [(String, f64, usize)],
        cut_below: bool,
        width: f64,
        size: f64,
        font: &Font,
        how: TextTruncate,
    ) {
        let last = lines.len().saturating_sub(1);
        for (i, line) in lines.iter_mut().enumerate() {
            if line.1 > width + 1e-9 || (cut_below && i == last) {
                let cut = self.ellipsis(&line.0, width, size, font, how);
                line.1 = (self.measure)(&cut, size, font);
                line.0 = cut;
            }
        }
    }

    /// The longest start of `line` that fits `width` with "..." after it
    /// (at a word's end for SplitWord, unless the first word is too long).
    fn ellipsis(&mut self, line: &str, width: f64, size: f64, font: &Font, how: TextTruncate) -> String {
        let ends: Vec<usize> = line
            .char_indices()
            .map(|(i, _)| i)
            .chain(std::iter::once(line.len()))
            .collect();
        let mut fits = |end: usize| (self.measure)(&format!("{}...", line[..end].trim_end()), size, font) <= width;
        // The first end that doesn't fit: the starts get wider as they get longer.
        let (mut lo, mut hi) = (0, ends.len());
        while lo < hi {
            let mid = (lo + hi) / 2;
            if fits(ends[mid]) {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let Some(best) = lo.checked_sub(1) else {
            // Not even "..." fits: as many of its dots as do.
            let mut dots = String::from("...");
            while !dots.is_empty() && (self.measure)(&dots, size, font) > width {
                dots.pop();
            }
            return dots;
        };
        let mut end = ends[best];
        let mid_word = end > 0 && !line[..end].ends_with(' ') && line[end..].starts_with(|c: char| c != ' ');
        if how == TextTruncate::SplitWord
            && mid_word
            && let Some(space) = line[..end].rfind(' ')
        {
            end = space;
        }
        format!("{}...", line[..end].trim_end())
    }

    /// MaxVisibleGraphemes: only the first `max` characters of `text` stay
    /// in the lines, which keep their places.
    fn visible(&mut self, lines: &mut Vec<TextLine>, text: &str, max: usize, size: f64, font: &Font, k: f64) {
        let end = text.char_indices().nth(max).map_or(text.len(), |(i, _)| i);
        lines.retain(|l| l.start < end);
        for line in lines.iter_mut() {
            let mut keep = (end - line.start).min(line.text.len());
            while !line.text.is_char_boundary(keep) {
                keep -= 1;
            }
            if keep < line.text.len() {
                line.text.truncate(keep);
                line.width = (self.measure)(&line.text, size, font) * k;
            }
        }
    }

    /// The drawn GuiObjects, bottom first: ScreenGuis by DisplayOrder, and in
    /// each, by ZIndex over all of it (Global) or among siblings (Sibling).
    fn draw_order(&self) -> Vec<NodeId> {
        let mut roots: Vec<NodeId> = self
            .tree
            .roots
            .iter()
            .copied()
            .filter(|&r| self.node(r).class == Class::ScreenGui && self.node(r).enabled)
            .collect();
        roots.sort_by_key(|&r| self.node(r).display_order);
        let mut order = Vec::new();
        for root in roots {
            match self.node(root).z_index_behavior {
                ZIndexBehavior::Sibling => self.sibling_order(root, &mut order),
                ZIndexBehavior::Global => {
                    let mut all = Vec::new();
                    self.tree_order(root, &mut all);
                    all.sort_by_key(|&id| self.node(id).z_index);
                    order.extend(all);
                }
            }
        }
        order
    }

    /// Children over their parent, each set of siblings by ZIndex.
    fn sibling_order(&self, id: NodeId, out: &mut Vec<NodeId>) {
        let mut children: Vec<NodeId> = self.gui_children(id).into_iter().map(|(c, _)| c).collect();
        children.sort_by_key(|&c| self.node(c).z_index);
        for c in children {
            if self.node(c).visible {
                out.push(c);
                self.sibling_order(c, out);
            }
        }
    }

    /// The visible GuiObjects under `id`, parents before children.
    fn tree_order(&self, id: NodeId, out: &mut Vec<NodeId>) {
        for (c, _) in self.gui_children(id) {
            if self.node(c).visible {
                out.push(c);
                self.tree_order(c, out);
            }
        }
    }
}
