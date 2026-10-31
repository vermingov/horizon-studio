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
// UIListLayout (with its flex and UIFlexItem), UIGridLayout, UIPageLayout
// and UITableLayout with their alignment, padding and SortOrder,
// ScrollingFrame canvases (CanvasSize, AutomaticCanvasSize, scroll bar
// insets, CanvasPosition clamped), ClipsDescendants, ZIndexBehavior Global
// and Sibling, ScreenGui's DisplayOrder and IgnoreGuiInset, and BillboardGui
// roots in the box the camera gives them. Rotation leaves the layout
// upright, as Roblox's is (and switches clipping off under it); each object
// gets the turns it is drawn with, and is hit where it is drawn. RichText
// is read here into runs, which the text is wrapped and measured by, and
// images are cut into the pieces their ScaleType draws them in.
// Coordinates are Roblox's GUI coordinates: the origin is under the top
// bar, so an IgnoreGuiInset ScreenGui starts at -inset.

use super::hstudio_rbx_types::{Color3, Font, FontEnum, FontStyle, FontWeight, Rect, UDim, UDim2, Vector2};
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
    /// A root laid out in the box the camera gives it (`Node::billboard`).
    BillboardGui,
    Frame,
    TextLabel,
    TextButton,
    TextBox,
    ImageLabel,
    ImageButton,
    ScrollingFrame,
    UIListLayout,
    UIGridLayout,
    UIPageLayout,
    UITableLayout,
    UIPadding,
    UIAspectRatioConstraint,
    UISizeConstraint,
    UITextSizeConstraint,
    UIScale,
    UICorner,
    UIStroke,
    UIFlexItem,
    Folder,
}

impl Class {
    pub const ALL: [Class; 22] = [
        Class::ScreenGui,
        Class::BillboardGui,
        Class::Frame,
        Class::TextLabel,
        Class::TextButton,
        Class::TextBox,
        Class::ImageLabel,
        Class::ImageButton,
        Class::ScrollingFrame,
        Class::UIListLayout,
        Class::UIGridLayout,
        Class::UIPageLayout,
        Class::UITableLayout,
        Class::UIPadding,
        Class::UIAspectRatioConstraint,
        Class::UISizeConstraint,
        Class::UITextSizeConstraint,
        Class::UIScale,
        Class::UICorner,
        Class::UIStroke,
        Class::UIFlexItem,
        Class::Folder,
    ];

    /// Its ClassName.
    pub fn name(self) -> &'static str {
        match self {
            Class::ScreenGui => "ScreenGui",
            Class::BillboardGui => "BillboardGui",
            Class::Frame => "Frame",
            Class::TextLabel => "TextLabel",
            Class::TextButton => "TextButton",
            Class::TextBox => "TextBox",
            Class::ImageLabel => "ImageLabel",
            Class::ImageButton => "ImageButton",
            Class::ScrollingFrame => "ScrollingFrame",
            Class::UIListLayout => "UIListLayout",
            Class::UIGridLayout => "UIGridLayout",
            Class::UIPageLayout => "UIPageLayout",
            Class::UITableLayout => "UITableLayout",
            Class::UIPadding => "UIPadding",
            Class::UIAspectRatioConstraint => "UIAspectRatioConstraint",
            Class::UISizeConstraint => "UISizeConstraint",
            Class::UITextSizeConstraint => "UITextSizeConstraint",
            Class::UIScale => "UIScale",
            Class::UICorner => "UICorner",
            Class::UIStroke => "UIStroke",
            Class::UIFlexItem => "UIFlexItem",
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

/// Enum.UIFlexAlignment: what a UIListLayout does with the room left along
/// an axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FlexAlignment {
    /// Nothing: the alignment places what is there.
    #[default]
    None,
    /// Along the line, the items grow (or shrink) to fill it; across, the
    /// lines do.
    Fill,
    /// In gaps, half a gap at each end.
    SpaceAround,
    /// In gaps between them only.
    SpaceBetween,
    /// In gaps, as large at the ends as between.
    SpaceEvenly,
}

/// Enum.ItemLineAlignment: where an item goes across its line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ItemLineAlignment {
    /// As the layout's alignment across says.
    #[default]
    Automatic,
    Start,
    Center,
    End,
    /// As thick as the line.
    Stretch,
}

/// Enum.UIFlexMode: how a UIFlexItem's object grows and shrinks in its
/// line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FlexMode {
    #[default]
    None,
    Grow,
    Shrink,
    /// Both.
    Fill,
    /// By GrowRatio and ShrinkRatio.
    Custom,
}

/// Enum.TableMajorAxis: what a UITableLayout's container holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TableMajorAxis {
    /// Rows, whose objects are the cells across.
    #[default]
    RowMajor,
    /// Columns, whose objects are the cells down.
    ColumnMajor,
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
    /// its parent's layout, and only its rectangle is placed, nothing under it.
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
    /// RichText's runs over `text` (which is then the text without its
    /// tags), in order and covering it all; empty for plain text.
    pub rich: Vec<Span>,

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
    /// A BillboardGui's box on screen, as the camera sees its adornee.
    pub billboard: Rect,
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
    pub horizontal_flex: FlexAlignment,
    pub vertical_flex: FlexAlignment,
    /// The layout's, and a UIFlexItem's own for its object.
    pub item_line_alignment: ItemLineAlignment,
    // UIFlexItem
    pub flex_mode: FlexMode,
    pub grow_ratio: f64,
    pub shrink_ratio: f64,

    // UIPageLayout: the page it is on (CurrentPage), the one it turns from
    // and how far it has turned (0 to 1, eased), which way, and whether the
    // pages go round.
    pub page: Option<NodeId>,
    pub page_from: Option<NodeId>,
    pub page_turn: f64,
    pub page_forward: bool,
    pub circular: bool,
    pub cell_size: UDim2,
    /// UIGridLayout's CellPadding, and UITableLayout's Padding.
    pub cell_padding: UDim2,
    /// 0 is no limit.
    pub fill_direction_max_cells: u32,
    // UITableLayout
    pub fill_empty_space_columns: bool,
    pub fill_empty_space_rows: bool,
    pub major_axis: TableMajorAxis,
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
            rich: Vec::new(),
            canvas_size: UDim2::new(0.0, 0.0, 2.0, 0.0),
            canvas_position: Vector2::ZERO,
            automatic_canvas_size: AutomaticSize::None,
            scroll_bar_thickness: 12.0,
            scrolling_direction: ScrollingDirection::XY,
            vertical_scroll_bar_inset: ScrollBarInset::None,
            horizontal_scroll_bar_inset: ScrollBarInset::None,
            vertical_scroll_bar_position: VerticalScrollBarPosition::Right,
            enabled: true,
            billboard: Rect::default(),
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
            horizontal_flex: FlexAlignment::None,
            vertical_flex: FlexAlignment::None,
            item_line_alignment: ItemLineAlignment::Automatic,
            flex_mode: FlexMode::None,
            grow_ratio: 1.0,
            shrink_ratio: 1.0,
            page: None,
            page_from: None,
            page_turn: 1.0,
            page_forward: true,
            circular: false,
            cell_size: UDim2::from_offset(100.0, 100.0),
            cell_padding: if class == Class::UITableLayout {
                UDim2::default()
            } else {
                UDim2::from_offset(5.0, 5.0)
            },
            fill_direction_max_cells: 0,
            fill_empty_space_columns: false,
            fill_empty_space_rows: false,
            major_axis: TableMajorAxis::RowMajor,
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

    /// Lays out every root ScreenGui on `view`, and every root BillboardGui
    /// in its box. `measure(text, size, font)` gives the width of one line of
    /// text at a text size.
    pub fn layout(&self, view: &Viewport, measure: &mut dyn FnMut(&str, f64, &Font) -> f64) -> Layout {
        let mut pass = Pass {
            tree: self,
            measure,
            placed: vec![None; self.slots.len()],
            forced: vec![None; self.slots.len()],
        };
        for &root in &self.roots {
            match self[root].class {
                Class::ScreenGui => pass.screen_gui(root, view),
                Class::BillboardGui => pass.billboard_gui(root),
                _ => {}
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
    /// How tall the line is on screen: its text size times LineHeight, the
    /// largest of its runs' for rich text.
    pub height: f64,
    /// Rich text's runs, left to right; empty for plain text.
    pub runs: Vec<TextRun>,
}

/// A run of a line of rich text, in one style.
#[derive(Clone, Debug, PartialEq)]
pub struct TextRun {
    pub text: String,
    /// Where it starts on screen, and how wide it is.
    pub x: f64,
    pub width: f64,
    /// Its text size on screen, and its font.
    pub size: f64,
    pub font: Font,
    pub style: RichStyle,
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
/// applied. Layout nodes (UIListLayout, UIGridLayout, UIPageLayout,
/// UITableLayout) fill in only `content_size`, the cell fields and `page`.
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
    /// In a BillboardGui that isn't Active: the pointer passes through it.
    pub inert: bool,
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
    /// UIPageLayout: the page it shows (the one it is nearest while it turns).
    pub page: Option<NodeId>,
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

    /// A point on the screen in its own upright space: its turns undone,
    /// the outermost first.
    pub fn unturned(&self, point: Vector2) -> Vector2 {
        self.turns.iter().rev().fold(point, |q, t| {
            let (sin, cos) = (-t.degrees).to_radians().sin_cos();
            let d = q - t.centre;
            t.centre + Vector2::new(d.x * cos - d.y * sin, d.x * sin + d.y * cos)
        })
    }

    /// Whether `point` is on it as it is drawn: in what clips it, and in its
    /// rectangle turned as it is.
    pub fn holds(&self, point: Vector2) -> bool {
        self.clip.is_none_or(|c| contains(c, point)) && contains(self.rect(), self.unturned(point))
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

    /// Every drawn GuiObject `point` is on as drawn (turned, and cut by what
    /// clips it), topmost first (LayerCollector:GetGuiObjectsAtPosition;
    /// MouseEnter and MouseLeave go by this, covered or not).
    pub fn objects_at(&self, point: Vector2) -> Vec<NodeId> {
        let over = |id: &NodeId| self.get(*id).is_some_and(|p| !p.inert && p.holds(point));
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

// ---------------------------------------------------------------- rich text

/// A stretch of rich text's style: what its tags set (None and false leave
/// the text object's own).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RichStyle {
    /// `<font color>`, `<font transparency>` and `<font size>`.
    pub color: Option<Color3>,
    pub transparency: Option<f64>,
    pub size: Option<f64>,
    /// `<font face>` or `<font family>`: a family's asset; `<font weight>`.
    pub family: Option<String>,
    pub weight: Option<FontWeight>,
    /// `<b>`, `<i>`, `<u>` and `<s>`.
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    /// `<stroke>` round the letters: its colour, thickness and transparency.
    pub stroke: Option<(Color3, f64, f64)>,
    /// `<mark>` behind them: its colour and transparency.
    pub mark: Option<(Color3, f64)>,
    /// `<uppercase>` (`<uc>`) and `<smallcaps>` (`<sc>`); `small` marks the
    /// small capitals smallcaps makes of lowercase letters (0.8 the size).
    pub uppercase: bool,
    pub smallcaps: bool,
    pub small: bool,
}

/// A run of rich text: bytes `start` to `end` of its text, in one style.
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub style: RichStyle,
}

/// Rich text read: its text without the tags, and its runs, in order and
/// covering it all.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RichText {
    pub text: String,
    pub spans: Vec<Span>,
}

/// A run's text size and font: its own, or the text object's (`size` and
/// `font`) with what its tags change.
pub fn run_font(style: &RichStyle, size: f64, font: &Font) -> (f64, Font) {
    let mut f = font.clone();
    if let Some(family) = &style.family {
        f.family = family.clone();
    }
    if let Some(w) = style.weight {
        f.weight = w;
    }
    if style.bold && f.weight.value() < FontWeight::Bold.value() {
        f.weight = FontWeight::Bold;
    }
    if style.italic {
        f.style = FontStyle::Italic;
    }
    let size = style.size.unwrap_or(size) * if style.small { 0.8 } else { 1.0 };
    (size, f)
}

/// The largest text size of the runs over bytes `at` to `at + len` (the
/// run at `at` for an empty line), or `size` for plain text.
fn rich_size(spans: &[Span], at: usize, len: usize, size: f64) -> f64 {
    let font = Font::default();
    let sized = |sp: &Span| run_font(&sp.style, size, &font).0;
    let end = at + len;
    let over = spans.iter().filter(|sp| sp.end > at && sp.start < end);
    match over.map(sized).reduce(f64::max) {
        Some(s) => s,
        None => spans
            .iter()
            .find(|sp| sp.start <= at && at <= sp.end)
            .map_or(size, sized),
    }
}

/// Rich text as Roblox reads its markup: `<b>`, `<i>`, `<u>`, `<s>`,
/// `<br />`, `<font color size face family weight transparency>`,
/// `<stroke color thickness transparency>`, `<mark color transparency>`,
/// `<uppercase>` (`<uc>`), `<smallcaps>` (`<sc>`) and `<!-- comments -->`,
/// with `&lt;`, `&gt;`, `&quot;`, `&apos;` and `&amp;` for the characters.
/// Tags it doesn't know are dropped; a `<` that starts no tag (`1 < 2`) is
/// text.
pub fn parse_rich(s: &str) -> RichText {
    let mut out = RichText::default();
    let mut stack: Vec<(&'static str, RichStyle)> = Vec::new();
    let plain = RichStyle::default();
    let mut i = 0;
    while i < s.len() {
        let rest = &s[i..];
        let style = stack.last().map_or(&plain, |t| &t.1).clone();
        if rest.starts_with("<!--") {
            i += rest.find("-->").map_or(rest.len(), |e| e + 3);
            continue;
        }
        // A tag starts with a letter or a slash after its `<`, and holds no other `<`.
        if rest.starts_with('<')
            && rest[1..].starts_with(|c: char| c.is_ascii_alphabetic() || c == '/')
            && let Some(end) = rest.find('>')
            && !rest[1..end].contains('<')
        {
            i += end + 1;
            rich_tag(&rest[1..end], &style, &mut stack, &mut out);
            continue;
        }
        let entities = [
            ("&lt;", '<'),
            ("&gt;", '>'),
            ("&quot;", '"'),
            ("&apos;", '\''),
            ("&amp;", '&'),
        ];
        if let Some((name, c)) = entities.iter().find(|(e, _)| rest.starts_with(e)) {
            rich_push(&mut out, *c, &style);
            i += name.len();
            continue;
        }
        let c = rest.chars().next().unwrap_or(' ');
        rich_push(&mut out, c, &style);
        i += c.len_utf8();
    }
    out
}

/// One tag's work: a style opened, closed, or a new line.
fn rich_tag(tag: &str, style: &RichStyle, stack: &mut Vec<(&'static str, RichStyle)>, out: &mut RichText) {
    let tag = tag.trim();
    let (closing, body) = match tag.strip_prefix('/') {
        Some(b) => (true, b.trim()),
        None => (false, tag),
    };
    let lone = body.ends_with('/');
    let body = body.trim_end_matches('/').trim();
    let (name, attrs) = body.split_once(char::is_whitespace).unwrap_or((body, ""));
    let name: &'static str = match name.to_ascii_lowercase().as_str() {
        "b" => "b",
        "i" => "i",
        "u" => "u",
        "s" => "s",
        "br" => "br",
        "font" => "font",
        "stroke" => "stroke",
        "mark" => "mark",
        "uppercase" | "uc" => "uppercase",
        "smallcaps" | "sc" => "smallcaps",
        _ => return,
    };
    if closing {
        if let Some(at) = stack.iter().rposition(|(n, _)| *n == name) {
            stack.truncate(at);
        }
        return;
    }
    if name == "br" {
        rich_push(out, '\n', style);
        return;
    }
    let mut st = style.clone();
    let attrs = rich_attributes(attrs);
    let attr = |k: &str| attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str());
    let number = |k: &str| attr(k).and_then(|v| v.trim().parse::<f64>().ok());
    match name {
        "b" => st.bold = true,
        "i" => st.italic = true,
        "u" => st.underline = true,
        "s" => st.strike = true,
        "uppercase" => st.uppercase = true,
        "smallcaps" => st.smallcaps = true,
        "font" => {
            if let Some(c) = attr("color").and_then(rich_color) {
                st.color = Some(c);
            }
            if let Some(n) = number("size") {
                st.size = Some(n.max(1.0));
            }
            if let Some(n) = number("transparency") {
                st.transparency = Some(n.clamp(0.0, 1.0));
            }
            if let Some(face) = attr("face") {
                match FontEnum::from_name_or_alias(face.trim()).and_then(|e| Font::from_enum(e).ok()) {
                    Some(f) => {
                        st.family = Some(f.family);
                        st.weight = Some(f.weight);
                    }
                    None => {
                        st.family = Some(Font::from_name(face.trim(), FontWeight::Regular, FontStyle::Normal).family)
                    }
                }
            }
            if let Some(family) = attr("family") {
                st.family = Some(family.trim().to_string());
            }
            if let Some(w) = attr("weight").and_then(rich_weight) {
                st.weight = Some(w);
            }
        }
        "stroke" => {
            let color = attr("color").and_then(rich_color).unwrap_or(Color3::new(0.0, 0.0, 0.0));
            st.stroke = Some((
                color,
                number("thickness").unwrap_or(1.0).max(0.0),
                number("transparency").unwrap_or(0.0).clamp(0.0, 1.0),
            ));
        }
        _ => {
            let color = attr("color")
                .and_then(rich_color)
                .unwrap_or(Color3::from_rgb(255.0, 255.0, 0.0));
            st.mark = Some((color, number("transparency").unwrap_or(0.0).clamp(0.0, 1.0)));
        }
    }
    if !lone {
        stack.push((name, st));
    }
}

/// A tag's attributes: `name="value"`, `name='value'` or `name=value`.
fn rich_attributes(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = s.trim();
    while let Some(eq) = rest.find('=') {
        let key = rest[..eq].trim().to_ascii_lowercase();
        let after = rest[eq + 1..].trim_start();
        let (value, next) = match after.chars().next() {
            Some(q @ ('"' | '\'')) => match after[1..].find(q) {
                Some(e) => (&after[1..1 + e], &after[e + 2..]),
                None => (&after[1..], ""),
            },
            _ => {
                let e = after.find(char::is_whitespace).unwrap_or(after.len());
                (&after[..e], &after[e..])
            }
        };
        out.push((key, value.to_string()));
        rest = next.trim_start();
    }
    out
}

/// A colour as rich text writes one: `#FF7800`, `#F80` or `rgb(255, 120, 0)`.
fn rich_color(v: &str) -> Option<Color3> {
    let v = v.trim();
    if let Some(hex) = v.strip_prefix('#') {
        let n = u32::from_str_radix(hex, 16).ok()?;
        let ch = |x: u32| f64::from(x & 0xFF);
        return match hex.len() {
            6 => Some(Color3::from_rgb(ch(n >> 16), ch(n >> 8), ch(n))),
            3 => Some(Color3::from_rgb(
                ch((n >> 8) & 0xF) * 17.0,
                ch((n >> 4) & 0xF) * 17.0,
                ch(n & 0xF) * 17.0,
            )),
            _ => None,
        };
    }
    let inner = v.strip_prefix("rgb(")?.strip_suffix(')')?;
    let parts: Vec<f64> = inner
        .split(',')
        .map(|p| p.trim().parse().ok())
        .collect::<Option<Vec<f64>>>()?;
    match parts[..] {
        [r, g, b] => Some(Color3::from_rgb(r, g, b)),
        _ => None,
    }
}

/// A weight by its name (`bold`, `SemiBold`) or its number (100 to 900).
fn rich_weight(v: &str) -> Option<FontWeight> {
    let v = v.trim();
    if let Ok(n) = v.parse::<f64>() {
        let n = ((n / 100.0).round() * 100.0).clamp(100.0, 900.0) as i32;
        return FontWeight::from_value(n);
    }
    FontWeight::ALL
        .iter()
        .copied()
        .find(|w| w.name().eq_ignore_ascii_case(v))
}

/// A character of rich text in `style`: capitals for `<uppercase>`, small
/// ones for the lowercase letters `<smallcaps>` raises.
fn rich_push(out: &mut RichText, c: char, style: &RichStyle) {
    let small = style.smallcaps && c.is_lowercase();
    let mut st = style.clone();
    st.small = small;
    let mut put = |c: char| {
        let at = out.text.len();
        out.text.push(c);
        let end = out.text.len();
        match out.spans.last_mut() {
            Some(last) if last.end == at && last.style == st => last.end = end,
            _ => out.spans.push(Span {
                start: at,
                end,
                style: st.clone(),
            }),
        }
    };
    if style.uppercase || small {
        c.to_uppercase().for_each(&mut put);
    } else {
        put(c);
    }
}

/// How large text is drawn, as TextBounds has it: wrapped to `width` if
/// given, each rich run in its own size and font.
pub fn text_bounds(
    text: &str,
    spans: &[Span],
    size: f64,
    font: &Font,
    width: Option<f64>,
    measure: &mut dyn FnMut(&str, f64, &Font) -> f64,
) -> Vector2 {
    let tree = Tree::new();
    let mut pass = Pass {
        tree: &tree,
        measure,
        placed: Vec::new(),
        forced: Vec::new(),
    };
    pass.wrap(text, spans, size, font, 1.0, width).1
}

// ---------------------------------------------------------------- images

/// Enum.ScaleType: how an image fills its object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ScaleType {
    /// Stretched over the object's box.
    #[default]
    Stretch,
    /// In nine pieces: the corners keep their size, the edges stretch
    /// along them and the middle both ways.
    Slice,
    /// Repeated at TileSize from the top left.
    Tile,
    /// As large as fits, keeping its shape, in the middle.
    Fit,
    /// As small as covers the box, keeping its shape, cut to it.
    Crop,
}

/// How an image object draws its image: ScaleType, the part of the image
/// it shows (ImageRectOffset and ImageRectSize, in pixels; a zero size is
/// all of it), TileSize, and SliceCenter (in the part's pixels) with
/// SliceScale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageFit {
    pub scale_type: ScaleType,
    pub rect_offset: Vector2,
    pub rect_size: Vector2,
    pub tile_size: UDim2,
    pub slice_center: Rect,
    pub slice_scale: f64,
}

impl Default for ImageFit {
    /// Instance.new's: stretched, all of the image.
    fn default() -> ImageFit {
        ImageFit {
            scale_type: ScaleType::Stretch,
            rect_offset: Vector2::ZERO,
            rect_size: Vector2::ZERO,
            tile_size: UDim2::from_scale(1.0, 1.0),
            slice_center: Rect::default(),
            slice_scale: 1.0,
        }
    }
}

/// The tiles an image is drawn in at most; past that it is stretched.
const MAX_TILES: usize = 1024;

/// The pieces an image of `image` pixels is drawn in to fill `area` (on
/// screen, `scale` the UIScale on it) as `fit` says: each the place it goes
/// and the part of the image (in pixels) drawn there. With `clip`, each is
/// cut to it, and its part of the image with it.
pub fn image_pieces(image: Vector2, area: Rect, scale: f64, fit: &ImageFit, clip: Option<Rect>) -> Vec<(Rect, Rect)> {
    let v = Vector2::new;
    let src = if fit.rect_size.x > 0.0 && fit.rect_size.y > 0.0 {
        let min = fit.rect_offset.max(Vector2::ZERO).min(image);
        Rect::new(min, (fit.rect_offset + fit.rect_size).min(image).max(min))
    } else {
        Rect::new(Vector2::ZERO, image)
    };
    let (sw, sh) = (src.width(), src.height());
    let (aw, ah) = (area.width(), area.height());
    if !(sw > 0.0 && sh > 0.0 && aw > 0.0 && ah > 0.0) {
        return Vec::new();
    }
    let tile = v(
        fit.tile_size.x.scale * aw + f64::from(fit.tile_size.x.offset) * scale,
        fit.tile_size.y.scale * ah + f64::from(fit.tile_size.y.offset) * scale,
    );
    let tiles = (tile.x > 0.0 && tile.y > 0.0).then(|| ((aw / tile.x).ceil(), (ah / tile.y).ceil()));
    let mut pieces = Vec::new();
    match fit.scale_type {
        ScaleType::Fit => {
            let k = (aw / sw).min(ah / sh);
            let size = v(sw * k, sh * k);
            let at = area.min + (v(aw, ah) - size) / 2.0;
            pieces.push((Rect::new(at, at + size), src));
        }
        ScaleType::Crop => {
            let k = (aw / sw).max(ah / sh);
            let shown = v(aw / k, ah / k);
            let at = src.min + (v(sw, sh) - shown) / 2.0;
            pieces.push((area, Rect::new(at, at + shown)));
        }
        ScaleType::Tile if tiles.is_none() => {}
        ScaleType::Tile if tiles.is_some_and(|(nx, ny)| nx * ny <= MAX_TILES as f64) => {
            let (nx, ny) = tiles.unwrap_or_default();
            for j in 0..ny as usize {
                for i in 0..nx as usize {
                    let at = area.min + v(i as f64 * tile.x, j as f64 * tile.y);
                    let end = (at + tile).min(area.max);
                    let shown = v(sw * (end.x - at.x) / tile.x, sh * (end.y - at.y) / tile.y);
                    pieces.push((Rect::new(at, end), Rect::new(src.min, src.min + shown)));
                }
            }
        }
        ScaleType::Slice => {
            let c = fit.slice_center;
            let (l, t) = (c.min.x.clamp(0.0, sw), c.min.y.clamp(0.0, sh));
            let (r, b) = (c.max.x.clamp(l, sw), c.max.y.clamp(t, sh));
            if !(r > l && b > t) {
                // No middle to stretch: the whole part is.
                pieces.push((area, src));
            } else {
                let k = fit.slice_scale.max(0.0) * scale;
                let (mut left, mut right, mut top, mut bottom) = (l * k, (sw - r) * k, t * k, (sh - b) * k);
                // Corners too big for the box share it.
                if left + right > aw {
                    let f = aw / (left + right);
                    (left, right) = (left * f, right * f);
                }
                if top + bottom > ah {
                    let f = ah / (top + bottom);
                    (top, bottom) = (top * f, bottom * f);
                }
                let xs = [area.min.x, area.min.x + left, area.max.x - right, area.max.x];
                let ys = [area.min.y, area.min.y + top, area.max.y - bottom, area.max.y];
                let us = [src.min.x, src.min.x + l, src.min.x + r, src.max.x];
                let vs = [src.min.y, src.min.y + t, src.min.y + b, src.max.y];
                for j in 0..3 {
                    for i in 0..3 {
                        let place = Rect::new(v(xs[i], ys[j]), v(xs[i + 1], ys[j + 1]));
                        let part = Rect::new(v(us[i], vs[j]), v(us[i + 1], vs[j + 1]));
                        if place.width() > 0.0 && place.height() > 0.0 && part.width() > 0.0 && part.height() > 0.0 {
                            pieces.push((place, part));
                        }
                    }
                }
            }
        }
        _ => pieces.push((area, src)),
    }
    match clip {
        Some(c) => pieces.into_iter().filter_map(|(p, s)| cut_piece(p, s, c)).collect(),
        None => pieces,
    }
}

/// A piece of an image cut to `clip`, the part of the image it shows cut alike.
fn cut_piece(place: Rect, part: Rect, clip: Rect) -> Option<(Rect, Rect)> {
    let cut = intersect(place, clip);
    if !(cut.width() > 0.0 && cut.height() > 0.0) {
        return None;
    }
    let map = |p: Vector2| {
        Vector2::new(
            part.min.x + (p.x - place.min.x) / place.width() * part.width(),
            part.min.y + (p.y - place.min.y) / place.height() * part.height(),
        )
    };
    Some((cut, Rect::new(map(cut.min), map(cut.max))))
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
    /// In a BillboardGui that takes no input.
    inert: bool,
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
    /// UIPageLayout's page shown.
    page: Option<NodeId>,
}

struct Pass<'a> {
    tree: &'a Tree,
    measure: &'a mut dyn FnMut(&str, f64, &Font) -> f64,
    placed: Vec<Option<Placed>>,
    /// Where a UITableLayout put its cells, in their row's content area.
    forced: Vec<Option<Area>>,
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
        let layouts = [
            Class::UIListLayout,
            Class::UIGridLayout,
            Class::UIPageLayout,
            Class::UITableLayout,
        ];
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
            let (_, bounds) = self.wrap(&node.text, &node.rich, text_size, &node.font, node.line_height, wrap);
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

    /// How an item grows and shrinks along its line (UIFlexItem's FlexMode,
    /// or every item alike when the line fills), and its own place across.
    fn flex_of(&self, item: NodeId, line: FlexAlignment) -> (f64, f64, ItemLineAlignment) {
        match self.component(item, Class::UIFlexItem) {
            Some(f) => {
                let (grow, shrink) = match f.flex_mode {
                    FlexMode::None => (0.0, 0.0),
                    FlexMode::Grow => (1.0, 0.0),
                    FlexMode::Shrink => (0.0, 1.0),
                    FlexMode::Fill => (1.0, 1.0),
                    FlexMode::Custom => (f.grow_ratio.max(0.0), f.shrink_ratio.max(0.0)),
                };
                (grow, shrink, f.item_line_alignment)
            }
            None if line == FlexAlignment::Fill => (1.0, 1.0, ItemLineAlignment::Automatic),
            None => (0.0, 0.0, ItemLineAlignment::Automatic),
        }
    }

    /// Arranges a container's items by its layout in `area`: each item's
    /// area, and the layout's AbsoluteContentSize.
    fn arrange(&mut self, container: NodeId, layout: NodeId, area: Area) -> Arranged {
        let items = self.layout_items(container, layout);
        if self.node(layout).class == Class::UIGridLayout {
            return self.arrange_grid(layout, &items, area);
        }
        if self.node(layout).class == Class::UIPageLayout {
            return self.arrange_pages(layout, &items, area);
        }
        if self.node(layout).class == Class::UITableLayout {
            return self.arrange_table(layout, &items, area);
        }
        let mut sizes: Vec<Vector2> = items.iter().map(|&c| self.size_of(c, area.size())).collect();
        let l = self.node(layout);
        let vertical = l.fill_direction == FillDirection::Vertical;
        let (main_flex, cross_flex) = if vertical {
            (l.vertical_flex, l.horizontal_flex)
        } else {
            (l.horizontal_flex, l.vertical_flex)
        };
        let flex: Vec<(f64, f64, ItemLineAlignment)> = items.iter().map(|&c| self.flex_of(c, main_flex)).collect();
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
        // Along each line, the room left goes to the items that grow, or
        // comes from those that shrink (by ShrinkRatio times their size).
        for line in &lines {
            let used = line.iter().map(|&i| main(sizes[i])).sum::<f64>() + gap * line.len().saturating_sub(1) as f64;
            let free = main_len - used;
            let grow: f64 = line.iter().map(|&i| flex[i].0).sum();
            let shrink: f64 = line.iter().map(|&i| flex[i].1 * main(sizes[i])).sum();
            for &i in line {
                let along = main(sizes[i]);
                let flexed = if free > 0.0 && grow > 0.0 {
                    along + free * flex[i].0 / grow
                } else if free < 0.0 && shrink > 0.0 {
                    (along + free * flex[i].1 * along / shrink).max(0.0)
                } else {
                    continue;
                };
                let s = if vertical {
                    Vector2::new(sizes[i].x, flexed)
                } else {
                    Vector2::new(flexed, sizes[i].y)
                };
                sizes[i] = self.clamp_size(items[i], s);
            }
        }
        let line_main = |line: &[usize]| {
            line.iter().map(|&i| main(sizes[i])).sum::<f64>() + gap * line.len().saturating_sub(1) as f64
        };
        let line_cross = |line: &[usize]| line.iter().map(|&i| cross(sizes[i])).fold(0.0, f64::max);
        let mut block_cross: f64 =
            lines.iter().map(|l| line_cross(l)).sum::<f64>() + gap * lines.len().saturating_sub(1) as f64;
        let mut content_main: f64 = 0.0;

        // Across, the room left goes to the lines when they wrap.
        let (mut cross_at, line_gap, line_grow) = if l.wraps {
            let free = (cross_len - block_cross).max(0.0);
            let n = lines.len() as f64;
            match cross_flex {
                FlexAlignment::Fill => (0.0, 0.0, free / n),
                FlexAlignment::SpaceBetween if n > 1.0 => (0.0, free / (n - 1.0), 0.0),
                FlexAlignment::SpaceAround => (free / n / 2.0, free / n, 0.0),
                FlexAlignment::SpaceEvenly => (free / (n + 1.0), free / (n + 1.0), 0.0),
                _ => ((cross_len - block_cross) * cross_align, 0.0, 0.0),
            }
        } else {
            (0.0, 0.0, 0.0)
        };
        if l.wraps && cross_flex != FlexAlignment::None {
            block_cross = block_cross + line_gap * (lines.len() as f64 - 1.0).max(0.0) + line_grow * lines.len() as f64;
        }
        let mut placed = Vec::with_capacity(items.len());
        for line in &lines {
            let (span, thick) = (line_main(line), line_cross(line) + line_grow);
            // Without Wraps each item aligns across the whole container.
            let band = if l.wraps { thick } else { cross_len };
            // Along the line, the room left is the alignment's, or gaps.
            let free = main_len - span;
            let n = line.len() as f64;
            let (mut main_at, spacing) = match main_flex {
                FlexAlignment::SpaceBetween if free > 0.0 && n > 1.0 => (0.0, free / (n - 1.0)),
                FlexAlignment::SpaceAround if free > 0.0 => (free / n / 2.0, free / n),
                FlexAlignment::SpaceEvenly if free > 0.0 => (free / (n + 1.0), free / (n + 1.0)),
                _ => (free * main_align, 0.0),
            };
            content_main = content_main.max(span + spacing * (n - 1.0).max(0.0));
            for &i in line {
                let mut s = sizes[i];
                let along = match flex[i].2 {
                    ItemLineAlignment::Automatic => l.item_line_alignment,
                    own => own,
                };
                let fraction = match along {
                    ItemLineAlignment::Start => 0.0,
                    ItemLineAlignment::Center => 0.5,
                    ItemLineAlignment::End => 1.0,
                    ItemLineAlignment::Stretch => {
                        s = if vertical {
                            Vector2::new(band, s.y)
                        } else {
                            Vector2::new(s.x, band)
                        };
                        0.0
                    }
                    ItemLineAlignment::Automatic => cross_align,
                };
                let c = cross_at + (band - cross(s)) * fraction;
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
                main_at += main(s) + gap + spacing;
            }
            cross_at += thick + gap + line_gap;
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
            page: None,
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
            page: None,
        }
    }

    /// UIPageLayout: the pages one after another along FillDirection, with
    /// Padding between, moved so that the page it is at (part of the way
    /// between two while it turns) lines up in the container as the
    /// alignments say. With Circular the pages go round, each where it is
    /// nearest that place.
    fn arrange_pages(&mut self, layout: NodeId, items: &[NodeId], area: Area) -> Arranged {
        let sizes: Vec<Vector2> = items.iter().map(|&c| self.size_of(c, area.size())).collect();
        let l = self.node(layout);
        let horizontal = l.fill_direction == FillDirection::Horizontal;
        let main = |v: Vector2| if horizontal { v.x } else { v.y };
        let cross = |v: Vector2| if horizontal { v.y } else { v.x };
        let (main_len, cross_len) = if horizontal { (area.w, area.h) } else { (area.h, area.w) };
        let (main_align, cross_align) = if horizontal {
            (h_fraction(l.horizontal_alignment), v_fraction(l.vertical_alignment))
        } else {
            (v_fraction(l.vertical_alignment), h_fraction(l.horizontal_alignment))
        };
        let gap = l.padding.resolve(main_len);
        let n = items.len();
        if n == 0 {
            return Arranged {
                items: Vec::new(),
                content_size: Vector2::ZERO,
                cell_size: Vector2::ZERO,
                cell_count: Vector2::ZERO,
                page: None,
            };
        }
        // Where each page starts along the axis, and how long a round is.
        let mut starts = Vec::with_capacity(n);
        let mut round = 0.0;
        for s in &sizes {
            starts.push(round);
            round += main(*s) + gap;
        }
        let index = |p: Option<NodeId>| p.and_then(|p| items.iter().position(|&i| i == p));
        let to = index(l.page).unwrap_or(0) as f64;
        let from = index(l.page_from).map_or(to, |f| f as f64);
        // Going round forward from the last page to the first is going on.
        let count = n as f64;
        let (from, to) = match (l.circular, l.page_forward) {
            (true, true) if to < from => (from, to + count),
            (true, false) if to > from => (from + count, to),
            _ => (from, to),
        };
        let at = from + (to - from) * l.page_turn.clamp(0.0, 1.0);
        // The start of the place `k` pages along, part of the way between two.
        let start_at = |k: f64| {
            let whole = |j: f64| starts[j.rem_euclid(count) as usize] + (j / count).floor() * round;
            let i = k.floor();
            whole(i) + (whole(i + 1.0) - whole(i)) * (k - i)
        };
        let scroll = start_at(at);
        let shown = at.round().rem_euclid(count) as usize;
        let offset = (main_len - main(sizes[shown])) * main_align;
        let mut placed = Vec::with_capacity(n);
        for (i, &item) in items.iter().enumerate() {
            let mut along = starts[i] - scroll;
            if l.circular && round > 0.0 {
                along -= (along / round).round() * round;
            }
            let s = sizes[i];
            let c = (cross_len - cross(s)) * cross_align;
            let m = along + offset;
            let (x, y) = if horizontal { (m, c) } else { (c, m) };
            placed.push((
                item,
                Area {
                    x: area.x + x,
                    y: area.y + y,
                    w: s.x,
                    h: s.y,
                },
            ));
        }
        let thick = sizes.iter().map(|&s| cross(s)).fold(0.0, f64::max);
        let long = round - gap;
        Arranged {
            items: placed,
            content_size: if horizontal {
                Vector2::new(long, thick)
            } else {
                Vector2::new(thick, long)
            },
            cell_size: Vector2::ZERO,
            cell_count: Vector2::ZERO,
            page: Some(items[shown]),
        }
    }

    /// UITableLayout: the container's items are its rows (RowMajor) or its
    /// columns (ColumnMajor), and their GuiObjects the cells, in SortOrder.
    /// Each column is as wide as its widest cell and each row as tall as its
    /// tallest, with Padding between; FillEmptySpaceColumns and
    /// FillEmptySpaceRows share the room left among them. Each cell takes
    /// its cell's size, each row (or column) runs the table's length, and
    /// the table aligns in the area.
    fn arrange_table(&mut self, layout: NodeId, items: &[NodeId], area: Area) -> Arranged {
        let cells: Vec<Vec<NodeId>> = items.iter().map(|&m| self.layout_items(m, layout)).collect();
        let sizes: Vec<Vec<Vector2>> = cells
            .iter()
            .map(|line| line.iter().map(|&c| self.size_of(c, area.size())).collect())
            .collect();
        let l = self.node(layout);
        let by_rows = l.major_axis == TableMajorAxis::RowMajor;
        let pad = l.cell_padding.resolve(area.size());
        // In a table of rows, the minor axis runs across (the columns) and
        // the major one down (the rows); in one of columns, the other way.
        let minor = |v: Vector2| if by_rows { v.x } else { v.y };
        let major = |v: Vector2| if by_rows { v.y } else { v.x };
        let (pad_minor, pad_major) = (minor(pad), major(pad));
        let (room_minor, room_major) = (minor(area.size()), major(area.size()));
        let (fill_minor, fill_major) = if by_rows {
            (l.fill_empty_space_columns, l.fill_empty_space_rows)
        } else {
            (l.fill_empty_space_rows, l.fill_empty_space_columns)
        };
        let n_minor = cells.iter().map(Vec::len).max().unwrap_or(0);
        let mut minor_len = vec![0.0f64; n_minor];
        let mut major_len = vec![0.0f64; items.len()];
        for (r, line) in sizes.iter().enumerate() {
            for (c, &s) in line.iter().enumerate() {
                minor_len[c] = minor_len[c].max(minor(s));
                major_len[r] = major_len[r].max(major(s));
            }
        }
        let span = |lens: &[f64], gap: f64| lens.iter().sum::<f64>() + gap * lens.len().saturating_sub(1) as f64;
        let share = |lens: &mut [f64], gap: f64, room: f64| {
            let free = room - span(lens, gap);
            if free > 0.0 && !lens.is_empty() {
                let each = free / lens.len() as f64;
                lens.iter_mut().for_each(|x| *x += each);
            }
        };
        if fill_minor {
            share(&mut minor_len, pad_minor, room_minor);
        }
        if fill_major {
            share(&mut major_len, pad_major, room_major);
        }
        let (long_minor, long_major) = (span(&minor_len, pad_minor), span(&major_len, pad_major));
        let block = if by_rows {
            Vector2::new(long_minor, long_major)
        } else {
            Vector2::new(long_major, long_minor)
        };
        let left = area.x + (area.w - block.x) * h_fraction(l.horizontal_alignment);
        let top = area.y + (area.h - block.y) * v_fraction(l.vertical_alignment);
        let at = |minor_at: f64, major_at: f64, minor_size: f64, major_size: f64| {
            if by_rows {
                Area {
                    x: minor_at,
                    y: major_at,
                    w: minor_size,
                    h: major_size,
                }
            } else {
                Area {
                    x: major_at,
                    y: minor_at,
                    w: major_size,
                    h: minor_size,
                }
            }
        };
        let mut placed = Vec::with_capacity(items.len());
        let mut major_at = 0.0;
        for (r, &item) in items.iter().enumerate() {
            let line = at(0.0, major_at, long_minor, major_len[r]);
            placed.push((
                item,
                Area {
                    x: left + line.x,
                    y: top + line.y,
                    ..line
                },
            ));
            let mut minor_at = 0.0;
            for (c, &cell) in cells[r].iter().enumerate() {
                self.forced[cell] = Some(at(minor_at, 0.0, minor_len[c], major_len[r]));
                minor_at += minor_len[c] + pad_minor;
            }
            major_at += major_len[r] + pad_major;
        }
        Arranged {
            items: placed,
            content_size: block,
            cell_size: Vector2::ZERO,
            cell_count: Vector2::ZERO,
            page: None,
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
            inert: false,
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

    /// A root BillboardGui: the box the camera gives it, its children
    /// placed in it, cut to it with ClipsDescendants; one that isn't Active
    /// lets the pointer through.
    fn billboard_gui(&mut self, id: NodeId) {
        let node = self.node(id);
        let r = node.billboard;
        let area = Area {
            x: r.min.x,
            y: r.min.y,
            w: r.width(),
            h: r.height(),
        };
        let shown = node.enabled;
        let ctx = Context {
            k: 1.0,
            origin: Vector2::ZERO,
            clip: node.clips_descendants.then_some(r),
            shown,
            rotation: 0.0,
            turned_by: None,
            inert: !node.active,
        };
        let content = area.inset(self.padding(id, area.size()));
        self.placed[id] = Some(Placed {
            class: Some(Class::BillboardGui),
            position: r.min,
            size: area.size(),
            shown,
            active: node.active,
            inert: !node.active,
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
                page: done.page,
                ..Placed::default()
            });
            areas.extend(done.items);
        }
        for (child, _) in self.gui_children(id) {
            if arranged.contains(&child) {
                continue;
            }
            // A table's cell goes where the table put it.
            if let Some(a) = self.forced.get(child).copied().flatten() {
                areas.push((
                    child,
                    Area {
                        x: content.x + a.x,
                        y: content.y + a.y,
                        ..a
                    },
                ));
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
            } else {
                self.place_sized(child, area, ctx);
            }
        }
    }

    /// The space a GuiObject at `area` is drawn in: its parent's, scaled by
    /// its UIScale about its AnchorPoint.
    fn own_space(&self, id: NodeId, area: Area, parent: Context) -> Context {
        let anchor = self.node(id).anchor_point;
        match self.component(id, Class::UIScale).map(|c| c.scale) {
            Some(s) if s != 1.0 => {
                let pivot = Vector2::new(area.x + anchor.x * area.w, area.y + anchor.y * area.h);
                parent.scaled(s, pivot)
            }
            _ => parent,
        }
    }

    /// A node only sized, at `area`: where it is, how large and how turned,
    /// as `place` has them, and nothing under it.
    fn place_sized(&mut self, id: NodeId, area: Area, parent: Context) {
        let ctx = self.own_space(id, area, parent);
        let node = self.node(id);
        self.placed[id] = Some(Placed {
            class: Some(node.class),
            position: ctx.rect(area).min,
            size: area.size() * ctx.k,
            rotation: parent.rotation + node.rotation,
            scale: ctx.k,
            ..Placed::default()
        });
    }

    /// One GuiObject at `area` (in its parent's space), then its subtree.
    fn place(&mut self, id: NodeId, area: Area, parent: Context) {
        let node = self.node(id);
        let mut ctx = self.own_space(id, area, parent);
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
            inert: ctx.inert,
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
        let (text, font, spans) = (&node.text, &node.font, &node.rich);
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
                let (_, b) = self.wrap(text, spans, s, font, line_height, width);
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
        let (mut lines, bounds) = self.wrap(text, spans, size, font, line_height, width);
        let fits = bounds.x <= content.w && bounds.y <= content.h;
        let step = size * line_height;
        // Rich text's lines are as tall as their largest runs.
        let heights: Vec<f64> = lines
            .iter()
            .map(|l| rich_size(spans, l.2, l.0.len(), size) * line_height)
            .collect();
        let drawn = if wrapped && step > 0.0 && spans.is_empty() {
            lines.len().min(((content.h / step) + 1e-9).floor().max(0.0) as usize)
        } else if wrapped && step > 0.0 {
            let mut used = 0.0;
            heights
                .iter()
                .take_while(|&&h| {
                    used += h;
                    used <= content.h + 1e-9
                })
                .count()
        } else {
            lines.len()
        };
        if node.text_truncate != TextTruncate::None {
            let cut_below = drawn < lines.len();
            lines.truncate(drawn);
            self.truncate(&mut lines, spans, cut_below, content.w, size, font, node.text_truncate);
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
        let total = if spans.is_empty() {
            drawn as f64 * step
        } else {
            heights.iter().take(drawn).sum()
        };
        let top = content.y + (content.h - total) * fy;
        let mut below = 0.0;
        let mut lines: Vec<TextLine> = lines
            .into_iter()
            .take(drawn)
            .enumerate()
            .map(|(i, (text, w, start))| {
                let y = if spans.is_empty() {
                    top + i as f64 * step
                } else {
                    top + below
                };
                below += heights[i];
                TextLine {
                    text,
                    position: ctx.point(Vector2::new(content.x + (content.w - w) * fx, y)),
                    width: w * ctx.k,
                    start,
                    height: heights[i] * ctx.k,
                    runs: Vec::new(),
                }
            })
            .collect();
        if let Ok(max) = usize::try_from(node.max_visible_graphemes) {
            self.visible(&mut lines, text, spans, max, size, font, ctx.k);
        }
        if !spans.is_empty() {
            for line in &mut lines {
                self.runs(line, text, spans, size, font, ctx.k);
            }
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
        spans: &[Span],
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
                Some(w) => self.wrap_paragraph(paragraph, spans, at, size, font, w, &mut lines),
            }
            at += paragraph.len() + 1;
        }
        let measured: Vec<(String, f64, usize)> = lines
            .into_iter()
            .map(|(line, start)| {
                let w = self.width(spans, start, &line, size, font);
                (line, w, start)
            })
            .collect();
        let widest = measured.iter().map(|l| l.1).fold(0.0, f64::max);
        let tall = if spans.is_empty() {
            measured.len() as f64 * size * line_height
        } else {
            measured
                .iter()
                .map(|l| rich_size(spans, l.2, l.0.len(), size) * line_height)
                .sum()
        };
        let bounds = Vector2::new(widest, tall);
        (measured, bounds)
    }

    /// How wide `s` is, which starts at byte `at` of a text object's text,
    /// at text size `size`: each of rich text's runs in its own size and font.
    fn width(&mut self, spans: &[Span], at: usize, s: &str, size: f64, font: &Font) -> f64 {
        if spans.is_empty() {
            return (self.measure)(s, size, font);
        }
        let end = at + s.len();
        let mut w = 0.0;
        for span in spans.iter().filter(|sp| sp.end > at && sp.start < end) {
            let (a, b) = (span.start.max(at) - at, span.end.min(end) - at);
            if let Some(piece) = s.get(a..b) {
                let (run_size, run_font) = run_font(&span.style, size, font);
                w += (self.measure)(piece, run_size, &run_font);
            }
        }
        w
    }

    /// A rich line's runs, from its text: the parts of the object's text it
    /// shows, each in its run's style, and what truncation added ("...") in
    /// the style of what it follows. The line's width becomes theirs.
    fn runs(&mut self, line: &mut TextLine, text: &str, spans: &[Span], size: f64, font: &Font, k: f64) {
        let from = &text[line.start.min(text.len())..];
        let mut kept = line.text.bytes().zip(from.bytes()).take_while(|(a, b)| a == b).count();
        while !line.text.is_char_boundary(kept) {
            kept -= 1;
        }
        let (at, end) = (line.start, line.start + kept);
        let mut x = line.position.x;
        let mut runs = Vec::new();
        let mut last = spans
            .iter()
            .find(|sp| sp.end > at)
            .or(spans.last())
            .map(|sp| sp.style.clone())
            .unwrap_or_default();
        let mut push = |pass: &mut Self, piece: &str, style: &RichStyle| {
            let (run_size, run_font) = run_font(style, size, font);
            let width = (pass.measure)(piece, run_size, &run_font) * k;
            runs.push(TextRun {
                text: piece.to_string(),
                x,
                width,
                size: run_size * k,
                font: run_font,
                style: style.clone(),
            });
            x += width;
        };
        for span in spans.iter().filter(|sp| sp.end > at && sp.start < end) {
            let piece = &text[span.start.max(at)..span.end.min(end)];
            push(self, piece, &span.style);
            last = span.style.clone();
        }
        if kept < line.text.len() {
            let suffix = line.text[kept..].to_string();
            push(self, &suffix, &last);
        }
        line.width = x - line.position.x;
        line.runs = runs;
    }

    /// A paragraph that starts at byte `at` of its text, wrapped to `width`:
    /// its lines and where each starts.
    #[allow(clippy::too_many_arguments)]
    fn wrap_paragraph(
        &mut self,
        paragraph: &str,
        spans: &[Span],
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
                if self.width(spans, start, &candidate, size, font) <= width {
                    line = candidate;
                    continue;
                }
                lines.push((std::mem::take(&mut line), start));
            }
            if self.width(spans, here, word, size, font) <= width {
                line = word.to_string();
                start = here;
                continue;
            }
            // A word wider than the line breaks where it must.
            for (i, ch) in word.char_indices() {
                let mut candidate = line.clone();
                candidate.push(ch);
                if !line.is_empty() && self.width(spans, start, &candidate, size, font) > width {
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
    #[allow(clippy::too_many_arguments)]
    fn truncate(
        &mut self,
        lines: &mut [(String, f64, usize)],
        spans: &[Span],
        cut_below: bool,
        width: f64,
        size: f64,
        font: &Font,
        how: TextTruncate,
    ) {
        let last = lines.len().saturating_sub(1);
        for (i, line) in lines.iter_mut().enumerate() {
            if line.1 > width + 1e-9 || (cut_below && i == last) {
                if !spans.is_empty() {
                    let (cut, w) = self.rich_ellipsis(&line.0, spans, line.2, width, size, font, how);
                    (line.0, line.1) = (cut, w);
                    continue;
                }
                let cut = self.ellipsis(&line.0, width, size, font, how);
                line.1 = (self.measure)(&cut, size, font);
                line.0 = cut;
            }
        }
    }

    /// `ellipsis` for a line of rich text starting at byte `at`: its start
    /// measured run by run, and "..." in the style of what it follows. The
    /// line cut, and its width.
    #[allow(clippy::too_many_arguments)]
    fn rich_ellipsis(
        &mut self,
        line: &str,
        spans: &[Span],
        at: usize,
        width: f64,
        size: f64,
        font: &Font,
        how: TextTruncate,
    ) -> (String, f64) {
        let ends: Vec<usize> = line
            .char_indices()
            .map(|(i, _)| i)
            .chain(std::iter::once(line.len()))
            .collect();
        let wide = |pass: &mut Self, end: usize| {
            let kept = line[..end].trim_end();
            // The dots take the style of the last character kept.
            let style = spans
                .iter()
                .find(|sp| sp.start < at + kept.len().max(1) && sp.end >= at + kept.len())
                .map(|sp| sp.style.clone())
                .unwrap_or_default();
            let (dot_size, dot_font) = run_font(&style, size, font);
            pass.width(spans, at, kept, size, font) + (pass.measure)("...", dot_size, &dot_font)
        };
        let (mut lo, mut hi) = (0, ends.len());
        while lo < hi {
            let mid = (lo + hi) / 2;
            if wide(self, ends[mid]) <= width {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let Some(best) = lo.checked_sub(1) else {
            let mut dots = String::from("...");
            while !dots.is_empty() && (self.measure)(&dots, size, font) > width {
                dots.pop();
            }
            let w = (self.measure)(&dots, size, font);
            return (dots, w);
        };
        let mut end = ends[best];
        let mid_word = end > 0 && !line[..end].ends_with(' ') && line[end..].starts_with(|c: char| c != ' ');
        if how == TextTruncate::SplitWord
            && mid_word
            && let Some(space) = line[..end].rfind(' ')
        {
            end = space;
        }
        let w = wide(self, end);
        (format!("{}...", line[..end].trim_end()), w)
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
    #[allow(clippy::too_many_arguments)]
    fn visible(
        &mut self,
        lines: &mut Vec<TextLine>,
        text: &str,
        spans: &[Span],
        max: usize,
        size: f64,
        font: &Font,
        k: f64,
    ) {
        let end = text.char_indices().nth(max).map_or(text.len(), |(i, _)| i);
        lines.retain(|l| l.start < end);
        for line in lines.iter_mut() {
            let mut keep = (end - line.start).min(line.text.len());
            while !line.text.is_char_boundary(keep) {
                keep -= 1;
            }
            if keep < line.text.len() {
                line.text.truncate(keep);
                line.width = self.width(spans, line.start, &line.text, size, font) * k;
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
        // BillboardGuis go under every ScreenGui, in the order they were
        // added (the farthest first).
        let boards = self
            .tree
            .roots
            .iter()
            .copied()
            .filter(|&r| self.node(r).class == Class::BillboardGui && self.node(r).enabled);
        let roots: Vec<NodeId> = boards.chain(roots).collect();
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
