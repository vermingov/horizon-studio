// The Roblox layer on the game (hstudio_rbx_draw): ScreenGuis and
// BillboardGuis laid out by hstudio_rbx_layout and drawn with the design
// system's shapes and fonts (then cut, turned and coloured), UIPageLayout's
// turns, the mouse and keys as Roblox input (hover, clicks, TextBoxes, scrolling,
// UserInputService, ContextActionService, the Mouse), Parts in the
// Workspace drawn in the world, rays through the world's blocks, the
// Camera's projection, and each frame's step (the character and the Camera
// following the player, RunService's events, tweens, Debris).
//
// Roblox's GUI pixels are dp here, Horizon's logical pixels, and there is
// no top bar (GuiService:GetGuiInset() is zero). A stud is a block.

use super::hstudio::{Font as StudioFont, GameApi};
use super::hstudio_lua::{Lua, Res, Table, Throw, Value, check_num, check_text};
use super::hstudio_quads::Shade;
use super::hstudio_rbx::{Env, Id, Rbx, Wait, is_a};
use super::hstudio_rbx_layout::{
    self as layout, AspectType, AutomaticSize, Class, DominantAxis, FillDirection, HorizontalAlignment, Layout, NodeId,
    ScrollBarInset, ScrollingDirection, SizeConstraint, SortOrder, StartCorner, TextXAlignment, TextYAlignment, Tree,
    VerticalAlignment, VerticalScrollBarPosition, Viewport, ZIndexBehavior,
};
use super::hstudio_rbx_types::{
    CFrame, Color3, ColorSequence, Font, FontWeight, NumberSequence, Rect, UDim2, Vector2, Vector3,
};
use super::hstudio_rbx_values::{
    Data, EnumItem, as_c3, as_cf, as_font, as_item, as_udim, as_udim2, as_v2, as_v3, cf, enum_item, enum_value, get,
    v2, v3,
};
use super::hstudio_ui::{Key, LineEdit, Mods};
use std::collections::HashMap;
use std::rc::Rc;

/// A laid-out frame of the GUI: the layout, and which instance each node is.
pub struct Frame {
    pub layout: Layout,
    pub node_inst: Vec<Id>,
    pub inst_node: HashMap<Id, NodeId>,
    pub view: (f64, f64),
    /// The GUI epoch it was laid out at.
    pub epoch: u64,
}

// ---------------------------------------------------------------- the layout

fn enum_name(v: &Value) -> &'static str {
    as_item(v).map_or("", |i| i.name)
}

fn num(inst: &super::hstudio_rbx::Inst, k: &str) -> f64 {
    inst.get(k).to_num().unwrap_or(0.0)
}

fn flag(inst: &super::hstudio_rbx::Inst, k: &str) -> bool {
    inst.get(k).truthy()
}

/// Text without rich text's tags (`<b>`, `<font color="...">`), as it is
/// drawn: `<br />` a new line, `<uppercase>` in capitals.
pub fn strip_tags(s: &str) -> String {
    layout::parse_rich(s).text
}

/// A node's properties from its instance's.
fn fill(node: &mut layout::Node, inst: &super::hstudio_rbx::Inst, focused: bool) {
    node.name = inst.name.clone();
    let c = inst.class;
    if is_a(c, "GuiObject") {
        node.position = as_udim2(&inst.get("Position")).unwrap_or_default();
        node.size = as_udim2(&inst.get("Size")).unwrap_or_default();
        node.anchor_point = as_v2(&inst.get("AnchorPoint")).unwrap_or_default();
        node.rotation = num(inst, "Rotation");
        node.visible = flag(inst, "Visible");
        node.z_index = num(inst, "ZIndex") as i32;
        node.layout_order = num(inst, "LayoutOrder") as i32;
        node.clips_descendants = flag(inst, "ClipsDescendants");
        node.active = flag(inst, "Active");
        node.interactable = flag(inst, "Interactable");
        node.automatic_size = match enum_name(&inst.get("AutomaticSize")) {
            "X" => AutomaticSize::X,
            "Y" => AutomaticSize::Y,
            "XY" => AutomaticSize::XY,
            _ => AutomaticSize::None,
        };
        node.size_constraint = match enum_name(&inst.get("SizeConstraint")) {
            "RelativeXX" => SizeConstraint::RelativeXX,
            "RelativeYY" => SizeConstraint::RelativeYY,
            _ => SizeConstraint::RelativeXY,
        };
    }
    if find(c, "Text") {
        let mut text = inst.get("Text").as_text().unwrap_or_default();
        let mut rich = Vec::new();
        // A TextBox being edited shows its text as it is, tags and all, so
        // the caret goes where the text has it.
        if flag(inst, "RichText") && !(c.name == "TextBox" && focused) {
            let read = layout::parse_rich(&text);
            (text, rich) = (read.text, read.spans);
        }
        if c.name == "TextBox" && text.is_empty() && !focused {
            text = inst.get("PlaceholderText").as_text().unwrap_or_default();
            rich = Vec::new();
        }
        node.text = text;
        node.rich = rich;
        node.text_size = num(inst, "TextSize");
        node.font = as_font(&inst.get("FontFace")).unwrap_or_default();
        node.text_scaled = flag(inst, "TextScaled");
        node.text_wrapped = flag(inst, "TextWrapped") || flag(inst, "TextWrap");
        node.line_height = num(inst, "LineHeight").max(0.1);
        node.text_x_alignment = match enum_name(&inst.get("TextXAlignment")) {
            "Left" => TextXAlignment::Left,
            "Right" => TextXAlignment::Right,
            _ => TextXAlignment::Center,
        };
        node.text_y_alignment = match enum_name(&inst.get("TextYAlignment")) {
            "Top" => TextYAlignment::Top,
            "Bottom" => TextYAlignment::Bottom,
            _ => TextYAlignment::Center,
        };
        node.text_truncate = match enum_name(&inst.get("TextTruncate")) {
            "AtEnd" => layout::TextTruncate::AtEnd,
            "SplitWord" => layout::TextTruncate::SplitWord,
            _ => layout::TextTruncate::None,
        };
        node.max_visible_graphemes = num(inst, "MaxVisibleGraphemes").clamp(-1.0, f64::from(i32::MAX)) as i32;
    }
    match c.name {
        "ScrollingFrame" => {
            node.canvas_size = as_udim2(&inst.get("CanvasSize")).unwrap_or_default();
            node.canvas_position = as_v2(&inst.get("CanvasPosition")).unwrap_or_default();
            node.automatic_canvas_size = match enum_name(&inst.get("AutomaticCanvasSize")) {
                "X" => AutomaticSize::X,
                "Y" => AutomaticSize::Y,
                "XY" => AutomaticSize::XY,
                _ => AutomaticSize::None,
            };
            node.scroll_bar_thickness = num(inst, "ScrollBarThickness");
            node.scrolling_direction = match enum_name(&inst.get("ScrollingDirection")) {
                "X" => ScrollingDirection::X,
                "Y" => ScrollingDirection::Y,
                _ => ScrollingDirection::XY,
            };
            let inset = |v: &Value| match enum_name(v) {
                "ScrollBar" => ScrollBarInset::ScrollBar,
                "Always" => ScrollBarInset::Always,
                _ => ScrollBarInset::None,
            };
            node.vertical_scroll_bar_inset = inset(&inst.get("VerticalScrollBarInset"));
            node.horizontal_scroll_bar_inset = inset(&inst.get("HorizontalScrollBarInset"));
            node.vertical_scroll_bar_position = match enum_name(&inst.get("VerticalScrollBarPosition")) {
                "Left" => VerticalScrollBarPosition::Left,
                _ => VerticalScrollBarPosition::Right,
            };
        }
        "BillboardGui" => {
            node.enabled = flag(inst, "Enabled");
            node.active = flag(inst, "Active");
            node.clips_descendants = flag(inst, "ClipsDescendants");
            node.z_index_behavior = match enum_name(&inst.get("ZIndexBehavior")) {
                "Global" => ZIndexBehavior::Global,
                _ => ZIndexBehavior::Sibling,
            };
        }
        "ScreenGui" => {
            node.enabled = flag(inst, "Enabled");
            node.ignore_gui_inset = flag(inst, "IgnoreGuiInset");
            node.display_order = num(inst, "DisplayOrder") as i32;
            node.z_index_behavior = match enum_name(&inst.get("ZIndexBehavior")) {
                "Global" => ZIndexBehavior::Global,
                _ => ZIndexBehavior::Sibling,
            };
        }
        "UIListLayout" | "UIGridLayout" | "UIPageLayout" | "UITableLayout" => {
            node.fill_direction = match enum_name(&inst.get("FillDirection")) {
                "Vertical" => FillDirection::Vertical,
                _ => FillDirection::Horizontal,
            };
            node.horizontal_alignment = match enum_name(&inst.get("HorizontalAlignment")) {
                "Center" => HorizontalAlignment::Center,
                "Right" => HorizontalAlignment::Right,
                _ => HorizontalAlignment::Left,
            };
            node.vertical_alignment = match enum_name(&inst.get("VerticalAlignment")) {
                "Center" => VerticalAlignment::Center,
                "Bottom" => VerticalAlignment::Bottom,
                _ => VerticalAlignment::Top,
            };
            node.sort_order = match enum_name(&inst.get("SortOrder")) {
                "Name" => SortOrder::Name,
                _ => SortOrder::LayoutOrder,
            };
            if c.name == "UIListLayout" {
                node.padding = as_udim(&inst.get("Padding")).unwrap_or_default();
                node.wraps = flag(inst, "Wraps");
                node.horizontal_flex = flex_alignment(&inst.get("HorizontalFlex"));
                node.vertical_flex = flex_alignment(&inst.get("VerticalFlex"));
                node.item_line_alignment = line_alignment(&inst.get("ItemLineAlignment"));
            } else if c.name == "UITableLayout" {
                node.cell_padding = as_udim2(&inst.get("Padding")).unwrap_or_default();
                node.fill_empty_space_columns = flag(inst, "FillEmptySpaceColumns");
                node.fill_empty_space_rows = flag(inst, "FillEmptySpaceRows");
                node.major_axis = match enum_name(&inst.get("MajorAxis")) {
                    "ColumnMajor" => layout::TableMajorAxis::ColumnMajor,
                    _ => layout::TableMajorAxis::RowMajor,
                };
            } else if c.name == "UIPageLayout" {
                // Its pages, and how far it has turned, are linked once the
                // tree is built (`link_pages`).
                node.padding = as_udim(&inst.get("Padding")).unwrap_or_default();
                node.circular = flag(inst, "Circular");
            } else {
                node.cell_size = as_udim2(&inst.get("CellSize")).unwrap_or_default();
                node.cell_padding = as_udim2(&inst.get("CellPadding")).unwrap_or_default();
                node.fill_direction_max_cells = num(inst, "FillDirectionMaxCells").max(0.0) as u32;
                node.start_corner = match enum_name(&inst.get("StartCorner")) {
                    "TopRight" => StartCorner::TopRight,
                    "BottomLeft" => StartCorner::BottomLeft,
                    "BottomRight" => StartCorner::BottomRight,
                    _ => StartCorner::TopLeft,
                };
            }
        }
        "UIFlexItem" => {
            node.flex_mode = match enum_name(&inst.get("FlexMode")) {
                "Grow" => layout::FlexMode::Grow,
                "Shrink" => layout::FlexMode::Shrink,
                "Fill" => layout::FlexMode::Fill,
                "Custom" => layout::FlexMode::Custom,
                _ => layout::FlexMode::None,
            };
            node.grow_ratio = num(inst, "GrowRatio");
            node.shrink_ratio = num(inst, "ShrinkRatio");
            node.item_line_alignment = line_alignment(&inst.get("ItemLineAlignment"));
        }
        "UIPadding" => {
            node.padding_top = as_udim(&inst.get("PaddingTop")).unwrap_or_default();
            node.padding_bottom = as_udim(&inst.get("PaddingBottom")).unwrap_or_default();
            node.padding_left = as_udim(&inst.get("PaddingLeft")).unwrap_or_default();
            node.padding_right = as_udim(&inst.get("PaddingRight")).unwrap_or_default();
        }
        "UIAspectRatioConstraint" => {
            node.aspect_ratio = num(inst, "AspectRatio");
            node.aspect_type = match enum_name(&inst.get("AspectType")) {
                "ScaleWithParentSize" => AspectType::ScaleWithParentSize,
                _ => AspectType::FitWithinMaxSize,
            };
            node.dominant_axis = match enum_name(&inst.get("DominantAxis")) {
                "Height" => DominantAxis::Height,
                _ => DominantAxis::Width,
            };
        }
        "UISizeConstraint" => {
            node.min_size = as_v2(&inst.get("MinSize")).unwrap_or_default();
            node.max_size = as_v2(&inst.get("MaxSize")).unwrap_or(Vector2::new(f64::INFINITY, f64::INFINITY));
        }
        "UITextSizeConstraint" => {
            node.min_text_size = num(inst, "MinTextSize");
            node.max_text_size = num(inst, "MaxTextSize");
        }
        "UIScale" => node.scale = num(inst, "Scale"),
        "UICorner" => node.corner_radius = as_udim(&inst.get("CornerRadius")).unwrap_or_default(),
        "UIStroke" => {
            node.enabled = flag(inst, "Enabled");
            node.thickness = num(inst, "Thickness");
            node.apply_stroke_mode = match enum_name(&inst.get("ApplyStrokeMode")) {
                "Border" => layout::ApplyStrokeMode::Border,
                _ => layout::ApplyStrokeMode::Contextual,
            };
        }
        _ => {}
    }
}

/// Enum.UIFlexAlignment's item as the layout has it.
fn flex_alignment(v: &Value) -> layout::FlexAlignment {
    match enum_name(v) {
        "Fill" => layout::FlexAlignment::Fill,
        "SpaceAround" => layout::FlexAlignment::SpaceAround,
        "SpaceBetween" => layout::FlexAlignment::SpaceBetween,
        "SpaceEvenly" => layout::FlexAlignment::SpaceEvenly,
        _ => layout::FlexAlignment::None,
    }
}

/// Enum.ItemLineAlignment's item as the layout has it.
fn line_alignment(v: &Value) -> layout::ItemLineAlignment {
    match enum_name(v) {
        "Start" => layout::ItemLineAlignment::Start,
        "Center" => layout::ItemLineAlignment::Center,
        "End" => layout::ItemLineAlignment::End,
        "Stretch" => layout::ItemLineAlignment::Stretch,
        _ => layout::ItemLineAlignment::Automatic,
    }
}

fn find(c: &'static super::hstudio_rbx::C, prop: &str) -> bool {
    let mut at = Some(c);
    while let Some(k) = at {
        if k.props.iter().any(|p| p.name == prop) {
            return true;
        }
        at = k.base.and_then(super::hstudio_rbx::class_def);
    }
    false
}

/// The layout class an instance is laid out as.
fn layout_class(c: &'static super::hstudio_rbx::C) -> Class {
    match c.name {
        "CanvasGroup" | "ViewportFrame" | "VideoFrame" => Class::Frame,
        name => Class::from_name(name).unwrap_or(Class::Folder),
    }
}

/// The studio's font for a Roblox font face.
pub fn studio_font(f: &Font) -> StudioFont {
    let fam = f.family.to_ascii_lowercase();
    if fam.contains("mono") || fam.contains("inconsolata") || fam.contains("code") || fam.contains("pressstart") {
        StudioFont::Mono
    } else if f.weight.value() >= FontWeight::Medium.value() {
        StudioFont::Medium
    } else {
        StudioFont::Regular
    }
}

pub fn font_of_enum(e: Option<EnumItem>) -> Font {
    e.and_then(|i| super::hstudio_rbx_types::FontEnum::from_name(i.name))
        .and_then(|f| Font::from_enum(f).ok())
        .unwrap_or_default()
}

/// A text's size as TextService:GetTextSize gives it: wrapped to `width`.
pub fn text_size(game: &mut dyn GameApi, text: &str, size: f64, font: Font, width: f64) -> (f64, f64) {
    let f = studio_font(&font);
    let mut lines = 0usize;
    let mut widest: f64 = 0.0;
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let trial = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            let w = f64::from(game.dp_measure(&trial, size as f32, f));
            if w > width && !line.is_empty() {
                widest = widest.max(f64::from(game.dp_measure(&line, size as f32, f)));
                lines += 1;
                line = word.to_owned();
            } else {
                line = trial;
            }
        }
        widest = widest.max(f64::from(game.dp_measure(&line, size as f32, f)));
        lines += 1;
    }
    (widest.ceil(), (lines as f64 * size).ceil())
}

/// The ScreenGuis drawn: those in the PlayerGui and in CoreGui.
fn screen_guis(rbx: &Rbx) -> Vec<Id> {
    let mut out = Vec::new();
    for root in [rbx.player_gui, rbx.core_gui, rbx.hui] {
        for &c in rbx.children(root) {
            if rbx.inst(c).class.name == "ScreenGui" {
                out.push(c);
            }
        }
    }
    out
}

/// The BillboardGuis that may be drawn: those in the PlayerGui, CoreGui
/// and gethui()'s folder, and those anywhere in the Workspace (not inside
/// another GUI).
fn billboard_guis(rbx: &Rbx) -> Vec<Id> {
    let mut out = Vec::new();
    for root in [rbx.player_gui, rbx.core_gui, rbx.hui] {
        for &c in rbx.children(root) {
            if rbx.inst(c).class.name == "BillboardGui" {
                out.push(c);
            }
        }
    }
    out.extend(
        rbx.descendants(rbx.workspace)
            .into_iter()
            .filter(|&d| rbx.inst(d).class.name == "BillboardGui" && board_root(rbx, d)),
    );
    out
}

/// Whether a BillboardGui is one `billboard_guis` draws.
fn board_root(rbx: &Rbx, id: Id) -> bool {
    let parent = rbx.try_inst(id).and_then(|i| i.parent);
    if parent.is_some_and(|p| [rbx.player_gui, rbx.core_gui, rbx.hui].contains(&p)) {
        return true;
    }
    let mut at = parent;
    while let Some(p) = at {
        if p == rbx.workspace {
            return true;
        }
        let inst = rbx.inst(p);
        if is_a(inst.class, "LayerCollector") {
            return false;
        }
        at = inst.parent;
    }
    false
}

/// Where AbsolutePosition is measured from for a node under `root`: the
/// BillboardGui's corner when the root is one, else the screen's.
fn board_origin(lay: &Layout, (root, board): (NodeId, bool)) -> Vector2 {
    match lay.get(root) {
        Some(p) if board => p.position,
        _ => Vector2::ZERO,
    }
}

/// A BillboardGui's place this frame.
struct Board {
    /// Its box on screen (dp), and how far its point is from the camera.
    rect: Rect,
    distance: f64,
    /// Whether it shows: no farther than MaxDistance, and no block in the
    /// way unless AlwaysOnTop.
    visible: bool,
}

/// Where a BillboardGui is drawn: over its Adornee (or the part, model or
/// attachment it is in), moved by StudsOffset along the camera's right, up
/// and back, StudsOffsetWorldSpace along the adornee's own axes, and
/// ExtentsOffset and ExtentsOffsetWorldSpace the same ways, in halves of
/// the adornee's size. Its Size's scales are studs, which shrink as it gets
/// farther (between DistanceLowerLimit and DistanceUpperLimit, in steps of
/// DistanceStep), and its offsets pixels; SizeOffset moves it by fractions
/// of its size, up for y. None while it can't be drawn: its adornee isn't
/// in the Workspace, it is behind the camera, or it is hidden from this
/// player (PlayerToHideFrom).
fn billboard(rbx: &Rbx, game: &mut dyn GameApi, id: Id) -> Option<Board> {
    let inst = rbx.inst(id);
    if rbx.id_of(&inst.get("PlayerToHideFrom")) == Some(rbx.local_player) {
        return None;
    }
    let adornee = rbx.id_of(&inst.get("Adornee")).or(inst.parent)?;
    let (frame, half) = adornee_box(rbx, adornee)?;
    let cam = game.camera()?;
    let (w, h, _) = game.dp();
    let v = |a: [f64; 3]| Vector3::new(a[0], a[1], a[2]);
    let [forward, up, right] = cam.basis().map(v);
    let back = -forward;
    let offset = |k: &str| as_v3(&inst.get(k)).unwrap_or(Vector3::ZERO);
    let (studs, studs_world) = (offset("StudsOffset"), offset("StudsOffsetWorldSpace"));
    let (extents, extents_world) = (offset("ExtentsOffset"), offset("ExtentsOffsetWorldSpace"));
    // How far the adornee's box reaches along `d`.
    let reach = |d: Vector3| {
        let o = frame.vector_to_object(d);
        o.x.abs() * half.x + o.y.abs() * half.y + o.z.abs() * half.z
    };
    let at = frame.position()
        + frame.vector_to_world(studs_world)
        + frame.vector_to_world(Vector3::new(
            extents_world.x * half.x,
            extents_world.y * half.y,
            extents_world.z * half.z,
        ))
        + right * (studs.x + extents.x * reach(right))
        + up * (studs.y + extents.y * reach(up))
        + back * (studs.z + extents.z * reach(back));
    let (sx, sy, depth) = cam.project([at.x, at.y, at.z], w, h)?;
    let eye = v(cam.pos);
    let distance = (at - eye).magnitude();
    let lower = num(inst, "DistanceLowerLimit").max(0.0);
    let upper = num(inst, "DistanceUpperLimit");
    let mut d = depth.max(lower);
    if upper >= 0.0 {
        d = d.min(upper.max(lower));
    }
    let step = num(inst, "DistanceStep");
    if step > 0.0 {
        d = ((d / step).round() * step).max(step);
    }
    let per_stud = f64::from(h) / (2.0 * (f64::from(cam.fov) / 2.0).to_radians().tan() * d.max(0.05));
    let size = as_udim2(&inst.get("Size")).unwrap_or_default();
    let bw = (size.x.scale * per_stud + f64::from(size.x.offset)).max(0.0);
    let bh = (size.y.scale * per_stud + f64::from(size.y.offset)).max(0.0);
    let shift = as_v2(&inst.get("SizeOffset")).unwrap_or_default();
    let min = Vector2::new(
        f64::from(sx) - bw / 2.0 + shift.x * bw,
        f64::from(sy) - bh / 2.0 - shift.y * bh,
    );
    let max_distance = inst.get("MaxDistance").to_num().unwrap_or(f64::INFINITY);
    let blocked = !inst.get("AlwaysOnTop").truthy()
        && distance > 0.0
        && first_block(game, eye, (at - eye) / distance, (distance - 0.3).max(0.0), true).is_some();
    Some(Board {
        rect: Rect::new(min, min + Vector2::new(bw, bh)),
        distance,
        visible: distance <= max_distance && !blocked,
    })
}

/// What a BillboardGui is drawn over, as a box: its frame and half its
/// size. A part's own; a model's round all its parts (upright); an
/// attachment's point on its part. None outside the Workspace.
fn adornee_box(rbx: &Rbx, id: Id) -> Option<(CFrame, Vector3)> {
    let inst = rbx.try_inst(id)?;
    if !rbx.is_descendant_of(id, rbx.workspace) {
        return None;
    }
    let part_box = |p: Id| {
        let frame = as_cf(&rbx.raw_get(p, "CFrame")).unwrap_or_default();
        (frame, as_v3(&rbx.raw_get(p, "Size")).unwrap_or(Vector3::ONE) / 2.0)
    };
    if is_a(inst.class, "BasePart") {
        return Some(part_box(id));
    }
    if inst.class.name == "Attachment" {
        let part = inst.parent.filter(|&p| is_a(rbx.inst(p).class, "BasePart"))?;
        let local = match (inst.extra.get("CFrame"), inst.extra.get("Position")) {
            (Some(c), _) if as_cf(c).is_some() => as_cf(c).unwrap_or_default(),
            (_, Some(p)) => CFrame::from_position(as_v3(p).unwrap_or(Vector3::ZERO)),
            _ => CFrame::default(),
        };
        return Some((part_box(part).0 * local, Vector3::ZERO));
    }
    if !is_a(inst.class, "Model") {
        return None;
    }
    let mut corners = Vec::new();
    for d in rbx.descendants(id) {
        if is_a(rbx.inst(d).class, "BasePart") {
            let (f, half) = part_box(d);
            for i in 0..8 {
                let pick = |bit: u32, a: f64| if i & bit == 0 { -a } else { a };
                corners.push(f.point_to_world(Vector3::new(pick(1, half.x), pick(2, half.y), pick(4, half.z))));
            }
        }
    }
    let Some(&first) = corners.first() else {
        let at = super::hstudio_rbx_methods::pivot(rbx, id).position();
        return Some((CFrame::from_position(at), Vector3::ZERO));
    };
    let (lo, hi) = corners
        .iter()
        .fold((first, first), |(lo, hi), &c| (lo.min(c), hi.max(c)));
    Some((CFrame::from_position((lo + hi) / 2.0), (hi - lo) / 2.0))
}

/// Lays out every ScreenGui for this frame, and writes back what Roblox
/// reports (AbsolutePosition, AbsoluteSize, TextBounds...).
pub fn lay_out(rbx: &mut Rbx, game: &mut dyn GameApi) {
    if !rbx.active {
        return;
    }
    let (w, h) = see_view(rbx, game);
    let mut tree = Tree::new();
    let mut node_inst: Vec<Id> = Vec::new();
    let mut inst_node: HashMap<Id, NodeId> = HashMap::new();
    let focus = rbx.pointer.focus;
    // The BillboardGuis the camera sees, the farthest first, then the
    // ScreenGuis; each node's root, for what is reported inside a billboard.
    let mut boards: Vec<(Id, Board)> = billboard_guis(rbx)
        .into_iter()
        .filter_map(|b| billboard(rbx, game, b).map(|at| (b, at)))
        .collect();
    boards.sort_by(|a, b| b.1.distance.total_cmp(&a.1.distance));
    let roots = boards
        .into_iter()
        .map(|(b, at)| (b, Some(at)))
        .chain(screen_guis(rbx).into_iter().map(|s| (s, None)));
    let mut node_root: Vec<(NodeId, bool)> = Vec::new();
    for (sg, board) in roots {
        let mut stack: Vec<(Id, Option<NodeId>)> = vec![(sg, None)];
        let mut root = 0;
        while let Some((id, parent)) = stack.pop() {
            let inst = rbx.inst(id);
            let class = layout_class(inst.class);
            let n = tree.add(class, parent);
            fill(&mut tree[n], inst, focus == Some(id));
            if parent.is_none() {
                root = n;
                if let Some(b) = &board {
                    tree[n].billboard = b.rect;
                    tree[n].enabled &= b.visible;
                }
            }
            if node_inst.len() <= n {
                node_inst.resize(n + 1, 0);
                node_root.resize(n + 1, (0, false));
            }
            node_inst[n] = id;
            node_root[n] = (root, board.is_some());
            inst_node.insert(id, n);
            for &c in inst.children.iter().rev() {
                stack.push((c, Some(n)));
            }
        }
    }
    link_pages(rbx, &mut tree, &node_inst, &|i| inst_node.get(&i).copied());
    let mut measure = |text: &str, size: f64, font: &Font| -> f64 {
        f64::from(game.dp_measure(text, size as f32, studio_font(font)))
    };
    let lay = tree.layout(&Viewport::new(f64::from(w), f64::from(h), 0.0), &mut measure);
    let mut writes: Vec<(Id, &'static str, Value, bool)> = Vec::new();
    for (n, &id) in node_inst.iter().enumerate() {
        let Some(p) = lay.get(n) else { continue };
        outputs(rbx, p, id, board_origin(&lay, node_root[n]), &mut writes);
        // A UIPageLayout is on its first page until it turns to another.
        if let Some(page) = p.page
            && tree[n].page != Some(page)
            && !rbx.page_turns.iter().any(|t| t.layout == id)
        {
            writes.push((id, "CurrentPage", rbx.value(node_inst[page]), true));
        }
    }
    write_outputs(rbx, writes);
    let epoch = rbx.gui_epoch;
    for &id in &node_inst {
        rbx.inst_mut(id).laid_out = epoch;
    }
    rbx.layout = Some(Frame {
        layout: lay,
        node_inst,
        inst_node,
        view: (f64::from(w), f64::from(h)),
        epoch,
    });
}

/// The properties the layout writes back. A change to one of them doesn't
/// move the layout, and reading one lays the GUI out first if it moved.
pub const OUTPUTS: &[&str] = &[
    "AbsolutePosition",
    "AbsoluteSize",
    "AbsoluteRotation",
    "TextBounds",
    "TextFits",
    "AbsoluteCanvasSize",
    "AbsoluteWindowSize",
    "AbsoluteContentSize",
    "AbsoluteCellSize",
    "AbsoluteCellCount",
];

/// The outputs of where an object is. A read in a list works them out for
/// the list's other items too, and they stay fresh until the GUI moves.
pub const PLACE: &[&str] = &["AbsolutePosition", "AbsoluteSize", "AbsoluteRotation"];

/// Properties only the drawing reads: colours, transparencies, images and
/// a TextBox's caret. A change to one doesn't move the layout either.
pub const DRAWN: &[&str] = &[
    "BackgroundColor3",
    "BackgroundTransparency",
    "BorderColor3",
    "BorderSizePixel",
    "Transparency",
    "Color",
    "Offset",
    "TextColor3",
    "TextTransparency",
    "TextStrokeColor3",
    "TextStrokeTransparency",
    "PlaceholderColor3",
    "Image",
    "ImageColor3",
    "ImageTransparency",
    "HoverImage",
    "PressedImage",
    "ImageRectOffset",
    "ImageRectSize",
    "ScaleType",
    "SliceCenter",
    "SliceScale",
    "TileSize",
    "ResampleMode",
    "ScrollBarImageColor3",
    "ScrollBarImageTransparency",
    "TopImage",
    "MidImage",
    "BottomImage",
    "GroupColor3",
    "GroupTransparency",
    "AutoButtonColor",
    "CursorPosition",
    "SelectionStart",
];

/// The screen's size in dp; a new size moves the GUI's layout.
fn see_view(rbx: &mut Rbx, game: &mut dyn GameApi) -> (f32, f32) {
    let (w, h, _) = game.dp();
    if rbx.gui_view != (w, h) {
        rbx.gui_view = (w, h);
        rbx.gui_epoch += 1;
    }
    (w, h)
}

/// What Roblox reports of a laid-out instance: (instance, property, value,
/// whether Changed fires for it). AbsolutePosition is from `origin`: a
/// BillboardGui's top left corner inside it, the screen's elsewhere.
fn outputs(rbx: &Rbx, p: &layout::Placed, id: Id, origin: Vector2, writes: &mut Vec<(Id, &'static str, Value, bool)>) {
    let c = rbx.inst(id).class;
    place_outputs(rbx, p, id, origin, writes);
    if let Some(t) = &p.text {
        writes.push((id, "TextBounds", v2(t.bounds), true));
        writes.push((id, "TextFits", Value::Bool(t.fits), false));
    }
    if c.name == "ScrollingFrame" {
        writes.push((id, "AbsoluteCanvasSize", v2(p.canvas_size), true));
        writes.push((id, "AbsoluteWindowSize", v2(p.window_size), true));
    }
    if matches!(
        c.name,
        "UIListLayout" | "UIGridLayout" | "UIPageLayout" | "UITableLayout"
    ) {
        writes.push((id, "AbsoluteContentSize", v2(p.content_size), true));
        if c.name == "UIGridLayout" {
            writes.push((id, "AbsoluteCellSize", v2(p.cell_size), false));
            writes.push((id, "AbsoluteCellCount", v2(p.cell_count), false));
        }
    }
}

/// The outputs of where a laid-out instance is (`PLACE`), as `outputs`.
fn place_outputs(
    rbx: &Rbx,
    p: &layout::Placed,
    id: Id,
    origin: Vector2,
    writes: &mut Vec<(Id, &'static str, Value, bool)>,
) {
    if is_a(rbx.inst(id).class, "GuiBase2d") {
        writes.push((id, "AbsolutePosition", v2(p.position - origin), true));
        writes.push((id, "AbsoluteSize", v2(p.size), true));
        writes.push((id, "AbsoluteRotation", Value::Num(p.rotation), false));
    }
}

fn write_outputs(rbx: &mut Rbx, writes: Vec<(Id, &'static str, Value, bool)>) {
    for (id, k, v, loud) in writes {
        if loud {
            rbx.raw_set(id, k, v);
        } else {
            rbx.quiet_set(id, k, v);
        }
    }
}

/// Brings `id`'s layout outputs (AbsoluteSize, TextBounds...) up to date
/// before a script reads one, as Roblox works them out when they are read:
/// when the GUI moved since they were last worked out, its ScreenGui is
/// laid out again, though only as far as `id` needs (the way down to it,
/// and the sizes of what a layout on the way arranges, whose places it
/// writes too). Returns the nodes laid out, none when its values were
/// fresh or it isn't in a ScreenGui that draws.
pub fn fresh_layout(rbx: &mut Rbx, game: &mut dyn GameApi, id: Id) -> usize {
    if !rbx.try_inst(id).is_some_and(|i| i.gui) {
        return 0;
    }
    let (w, h) = see_view(rbx, game);
    if rbx.inst(id).laid_out == rbx.gui_epoch {
        return 0;
    }
    let Some(path) = drawn_path(rbx, id) else {
        return 0;
    };
    // A BillboardGui is laid out in the box the camera gives it, if it has one.
    let board = match rbx.inst(path[0]).class.name {
        "BillboardGui" => match billboard(rbx, game, path[0]) {
            Some(b) => Some(b.rect),
            None => return 0,
        },
        _ => None,
    };
    let mut part = Partial {
        rbx,
        tree: Tree::new(),
        node_inst: Vec::new(),
        path: &path,
        rows: std::collections::HashSet::new(),
        items: std::collections::HashSet::new(),
    };
    part.add(path[0], None, true, false);
    let (mut tree, node_inst, items) = (part.tree, part.node_inst, part.items);
    if let (Some(r), Some(&root)) = (board, tree.roots().first()) {
        tree[root].billboard = r;
    }
    let nodes: HashMap<Id, NodeId> = node_inst.iter().enumerate().map(|(n, &i)| (i, n)).collect();
    link_pages(rbx, &mut tree, &node_inst, &|i| nodes.get(&i).copied());
    let mut measure = |text: &str, size: f64, font: &Font| -> f64 {
        f64::from(game.dp_measure(text, size as f32, studio_font(font)))
    };
    let lay = tree.layout(&Viewport::new(f64::from(w), f64::from(h), 0.0), &mut measure);
    // Only the way down is laid out as the whole GUI would be, with the
    // layouts on it whose items are all there.
    let wanted = |n: NodeId| {
        path.contains(&node_inst[n])
            || matches!(
                tree[n].class,
                Class::UIListLayout | Class::UIGridLayout | Class::UIPageLayout | Class::UITableLayout
            ) && tree.parent(n).is_some_and(|p| items.contains(&p))
    };
    let mut writes = Vec::new();
    let mut fresh = Vec::new();
    let mut placed = Vec::new();
    let origin = tree
        .roots()
        .first()
        .map_or(Vector2::ZERO, |&r| board_origin(&lay, (r, board.is_some())));
    for (n, &i) in node_inst.iter().enumerate() {
        if wanted(n) {
            fresh.push(i);
            if let Some(p) = lay.get(n) {
                outputs(rbx, p, i, origin, &mut writes);
            }
        } else if tree[n].size_only
            && tree[n].class.is_gui_object()
            && let Some(p) = lay.get(n)
        {
            // Beside the way, where the way's object put it (the items of
            // its layout, say): its place is the whole layout's, its text
            // and what it holds aren't laid out.
            placed.push(i);
            place_outputs(rbx, p, i, origin, &mut writes);
        }
    }
    write_outputs(rbx, writes);
    let epoch = rbx.gui_epoch;
    for i in fresh {
        rbx.inst_mut(i).laid_out = epoch;
    }
    for i in placed {
        rbx.inst_mut(i).placed_at = epoch;
    }
    rbx.fresh_layouts += 1;
    rbx.fresh_nodes += node_inst.len() as u64;
    node_inst.len()
}

/// `fresh_layout` for a read of `key`, one of `id`'s outputs: where an
/// object is, when a read beside it worked that out since the GUI last
/// moved, is fresh already.
pub fn fresh_output(rbx: &mut Rbx, game: &mut dyn GameApi, id: Id, key: &str) -> usize {
    if PLACE.contains(&key) && rbx.try_inst(id).is_some_and(|i| i.gui) {
        see_view(rbx, game);
        if rbx.inst(id).placed_at == rbx.gui_epoch {
            return 0;
        }
    }
    fresh_layout(rbx, game, id)
}

/// The instances from the ScreenGui that draws `id` down to it, if one does.
fn drawn_path(rbx: &Rbx, id: Id) -> Option<Vec<Id>> {
    let drawn = [rbx.player_gui, rbx.core_gui, rbx.hui];
    let mut path = vec![id];
    let mut at = id;
    loop {
        let inst = rbx.try_inst(at)?;
        let parent = inst.parent?;
        if inst.class.name == "ScreenGui" && drawn.contains(&parent)
            || inst.class.name == "BillboardGui" && board_root(rbx, at)
        {
            path.reverse();
            return Some(path);
        }
        path.push(parent);
        at = parent;
    }
}

/// The part of a ScreenGui's tree a read needs, as `fresh_layout` builds it.
struct Partial<'a> {
    rbx: &'a Rbx,
    tree: Tree,
    node_inst: Vec<Id>,
    /// From the ScreenGui down to the instance read.
    path: &'a [Id],
    /// The rows (or columns) of a table on the way, or of one that sizes
    /// an object: all their cells are sized, since each column is as wide
    /// as its widest.
    rows: std::collections::HashSet<Id>,
    /// The nodes on the path whose GuiObjects are all in the tree, so that
    /// their layout's values are what the whole layout gives.
    items: std::collections::HashSet<NodeId>,
}

impl Partial<'_> {
    /// Adds `id` under `parent`: placed as in the whole layout when it is
    /// on the path, else only sized. Under it go the UI components that
    /// apply, and the children its size, its canvas or its layout (on the
    /// path) depends on. `contents` says, for a Folder, whether what it
    /// holds counts: its GUI ancestor's size or canvas comes from it.
    fn add(&mut self, id: Id, parent: Option<NodeId>, on_path: bool, contents: bool) {
        let rbx = self.rbx;
        let inst = rbx.inst(id);
        let class = layout_class(inst.class);
        let n = self.tree.add(class, parent);
        let auto = is_a(inst.class, "GuiObject") && enum_name(&inst.get("AutomaticSize")) != "None";
        if on_path || auto || !class.is_gui_object() {
            fill(&mut self.tree[n], inst, rbx.pointer.focus == Some(id));
        } else {
            fill_size(&mut self.tree[n], inst);
        }
        self.tree[n].size_only = !on_path;
        if self.node_inst.len() <= n {
            self.node_inst.resize(n + 1, 0);
        }
        self.node_inst[n] = id;
        let next = self
            .path
            .iter()
            .position(|&p| p == id)
            .and_then(|i| self.path.get(i + 1).copied())
            .filter(|_| on_path);
        let folder = class == Class::Folder;
        let canvas = inst.class.name == "ScrollingFrame" && enum_name(&inst.get("AutomaticCanvasSize")) != "None";
        let arranged = inst.children.iter().any(|&c| {
            matches!(
                rbx.inst(c).class.name,
                "UIListLayout" | "UIGridLayout" | "UIPageLayout" | "UITableLayout"
            )
        });
        // What it holds counts for its size (or, on the path, its canvas);
        // a Folder's counts as much as for the object it is in.
        let contents = if folder { contents } else { auto || on_path && canvas };
        if (on_path || auto) && inst.children.iter().any(|&c| rbx.inst(c).class.name == "UITableLayout") {
            self.rows.extend(inst.children.iter().copied());
        }
        let row = self.rows.contains(&id);
        if on_path && !folder && (contents || row || arranged && next.is_some()) {
            self.items.insert(n);
        }
        for &c in &inst.children {
            if Some(c) == next {
                self.add(c, Some(n), true, contents);
                continue;
            }
            let cc = layout_class(rbx.inst(c).class);
            if cc == Class::ScreenGui {
                continue;
            }
            if !cc.is_gui_object() && cc != Class::Folder {
                // A UI component: a sized node needs only its size
                // constraints, and its UIScale for where it is drawn.
                if on_path
                    || auto
                    || matches!(
                        cc,
                        Class::UISizeConstraint | Class::UIAspectRatioConstraint | Class::UIFlexItem | Class::UIScale
                    )
                {
                    self.add(c, Some(n), false, false);
                }
                continue;
            }
            // What its size comes from, a row's cells, and the items its
            // layout arranges on the way.
            let needed = contents || !folder && cc.is_gui_object() && (row || arranged && next.is_some());
            if needed {
                self.add(c, Some(n), false, contents);
            }
        }
    }
}

/// The properties a node's size and its place in its parent's layout come
/// from: enough for a node a partial layout only sizes.
fn fill_size(node: &mut layout::Node, inst: &super::hstudio_rbx::Inst) {
    node.name = inst.name.clone();
    node.position = as_udim2(&inst.get("Position")).unwrap_or_default();
    node.size = as_udim2(&inst.get("Size")).unwrap_or_default();
    node.anchor_point = as_v2(&inst.get("AnchorPoint")).unwrap_or_default();
    node.rotation = num(inst, "Rotation");
    node.visible = flag(inst, "Visible");
    node.layout_order = num(inst, "LayoutOrder") as i32;
    node.size_constraint = match enum_name(&inst.get("SizeConstraint")) {
        "RelativeXX" => SizeConstraint::RelativeXX,
        "RelativeYY" => SizeConstraint::RelativeYY,
        _ => SizeConstraint::RelativeXY,
    };
}

// ---------------------------------------------------------------- pages

/// A UIPageLayout turning from one page to another, over its TweenTime with
/// its easing.
pub struct PageTurn {
    pub layout: Id,
    pub from: Id,
    pub to: Id,
    /// Going on to the next pages (else back), which counts with Circular.
    pub forward: bool,
    pub elapsed: f64,
    pub time: f64,
    pub style: super::hstudio_rbx_types::EasingStyle,
    pub direction: super::hstudio_rbx_types::EasingDirection,
}

impl PageTurn {
    /// How far it has turned, eased, from 0 to 1.
    fn eased(&self) -> f64 {
        super::hstudio_rbx_types::ease(self.style, self.direction, self.elapsed / self.time.max(1e-9))
    }
}

/// A UIPageLayout's pages, in its SortOrder: its container's visible GuiObjects.
pub fn pages(rbx: &Rbx, layout: Id) -> Vec<Id> {
    let Some(parent) = rbx.try_inst(layout).and_then(|i| i.parent) else {
        return Vec::new();
    };
    let mut pages: Vec<Id> = rbx
        .children(parent)
        .iter()
        .copied()
        .filter(|&c| {
            let i = rbx.inst(c);
            is_a(i.class, "GuiObject") && i.get("Visible").truthy()
        })
        .collect();
    match enum_name(&rbx.raw_get(layout, "SortOrder")) {
        "Name" => pages.sort_by(|a, b| rbx.inst(*a).name.cmp(&rbx.inst(*b).name)),
        _ => pages.sort_by_key(|&c| num(rbx.inst(c), "LayoutOrder") as i64),
    }
    pages
}

/// The page it is on: CurrentPage, or the first page.
fn current_page(rbx: &Rbx, layout: Id, pages: &[Id]) -> Option<Id> {
    rbx.id_of(&rbx.raw_get(layout, "CurrentPage"))
        .filter(|p| pages.contains(p))
        .or(pages.first().copied())
}

/// The page after (or before) the one it is on; past the ends, round to
/// the other end with Circular, else none.
pub fn page_beside(rbx: &Rbx, layout: Id, pages: &[Id], next: bool) -> Option<Id> {
    let at = pages
        .iter()
        .position(|&p| Some(p) == current_page(rbx, layout, pages))?;
    let n = pages.len();
    let to = match (next, at) {
        (true, _) if at + 1 < n => at + 1,
        (false, _) if at > 0 => at - 1,
        _ if !rbx.raw_get(layout, "Circular").truthy() => return None,
        (true, _) => 0,
        (false, _) => n - 1,
    };
    pages.get(to).copied()
}

/// Turns a UIPageLayout to `page`, one of its pages: PageLeave fires for
/// the page it leaves and PageEnter for this one, CurrentPage becomes it,
/// and it turns there over TweenTime with its easing when Animated (else at
/// once); Stopped fires once it is there. `forward` says which way it goes
/// round with Circular (by default the way the page lies).
pub fn turn_page(rbx: &mut Rbx, layout: Id, page: Id, forward: Option<bool>) {
    let pages = pages(rbx, layout);
    let (Some(to), Some(from)) = (pages.iter().position(|&p| p == page), current_page(rbx, layout, &pages)) else {
        return;
    };
    let at = pages.iter().position(|&p| p == from).unwrap_or(0);
    if from == page && !rbx.page_turns.iter().any(|t| t.layout == layout) {
        return;
    }
    let time = rbx.raw_get(layout, "TweenTime").to_num().unwrap_or(1.0);
    let animated = rbx.raw_get(layout, "Animated").truthy() && time > 0.0;
    let style = as_item(&rbx.raw_get(layout, "EasingStyle"))
        .and_then(|i| super::hstudio_rbx_types::EasingStyle::from_name(i.name))
        .unwrap_or(super::hstudio_rbx_types::EasingStyle::Back);
    let direction = as_item(&rbx.raw_get(layout, "EasingDirection"))
        .and_then(|i| super::hstudio_rbx_types::EasingDirection::from_name(i.name))
        .unwrap_or(super::hstudio_rbx_types::EasingDirection::Out);
    rbx.page_turns.retain(|t| t.layout != layout);
    if animated {
        rbx.page_turns.push(PageTurn {
            layout,
            from,
            to: page,
            forward: forward.unwrap_or(to >= at),
            elapsed: 0.0,
            time,
            style,
            direction,
        });
    }
    let (left, entered) = (rbx.value(from), rbx.value(page));
    if from != page {
        rbx.fire(layout, "PageLeave", vec![left]);
    }
    rbx.raw_set(layout, "CurrentPage", entered.clone());
    rbx.fire(layout, "PageEnter", vec![entered.clone()]);
    if !animated {
        rbx.fire(layout, "Stopped", vec![entered]);
    }
    rbx.gui_epoch += 1;
}

/// The page turns' frame: each moves on, and Stopped fires as one ends. A
/// layout destroyed since stops turning.
fn step_pages(rbx: &mut Rbx, dt: f64) {
    let insts = &rbx.insts;
    rbx.page_turns
        .retain(|t| insts.get(&t.layout).is_some_and(|i| !i.destroyed));
    if rbx.page_turns.is_empty() {
        return;
    }
    rbx.gui_epoch += 1;
    let mut stopped = Vec::new();
    for t in &mut rbx.page_turns {
        t.elapsed += dt;
        if t.elapsed >= t.time {
            stopped.push((t.layout, t.to));
        }
    }
    rbx.page_turns.retain(|t| t.elapsed < t.time);
    for (layout, page) in stopped {
        let v = rbx.value(page);
        rbx.fire(layout, "Stopped", vec![v]);
    }
}

/// Gives each UIPageLayout node of `tree` its page (CurrentPage), the page
/// it turns from and how far it has turned, as nodes (`node_of` finds an
/// instance's).
fn link_pages(rbx: &Rbx, tree: &mut Tree, nodes: &[Id], node_of: &dyn Fn(Id) -> Option<NodeId>) {
    for (n, &id) in nodes.iter().enumerate() {
        if tree.get(n).is_none_or(|t| t.class != Class::UIPageLayout) {
            continue;
        }
        tree[n].page = rbx.id_of(&rbx.raw_get(id, "CurrentPage")).and_then(node_of);
        let (from, turn, forward) = match rbx.page_turns.iter().find(|t| t.layout == id) {
            Some(t) => (node_of(t.from), t.eased(), t.forward),
            None => (None, 1.0, true),
        };
        tree[n].page_from = from;
        tree[n].page_turn = turn;
        tree[n].page_forward = forward;
    }
}

/// The wheel over a UIPageLayout's container (or one of its pages), when
/// ScrollWheelInputEnabled, turns a page: down to the next, up to the one
/// before. Whether it did.
fn wheel_pages(rbx: &mut Rbx, over: &[Id], steps: f64) -> bool {
    let layout = over.iter().find_map(|&o| {
        let inst = rbx.try_inst(o)?;
        let with = |i: Id| {
            rbx.children(i)
                .iter()
                .copied()
                .find(|&c| rbx.inst(c).class.name == "UIPageLayout")
        };
        with(o).or_else(|| inst.parent.and_then(with))
    });
    let Some(layout) = layout.filter(|&l| rbx.raw_get(l, "ScrollWheelInputEnabled").truthy()) else {
        return false;
    };
    let pages = pages(rbx, layout);
    if let Some(p) = page_beside(rbx, layout, &pages, steps < 0.0) {
        turn_page(rbx, layout, p, Some(steps < 0.0));
    }
    true
}

// ---------------------------------------------------------------- drawing

fn argb(c: Color3, alpha: f64) -> u32 {
    let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
    let ch = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u32;
    a << 24 | ch(c.r) << 16 | ch(c.g) << 8 | ch(c.b)
}

fn color(inst: &super::hstudio_rbx::Inst, k: &str) -> Color3 {
    as_c3(&inst.get(k)).unwrap_or_default()
}

fn mix(a: Color3, b: Color3, t: f64) -> Color3 {
    a.lerp(b, t)
}

fn rect4(r: Rect) -> [f32; 4] {
    [
        r.min.x as f32,
        r.min.y as f32,
        (r.max.x - r.min.x) as f32,
        (r.max.y - r.min.y) as f32,
    ]
}

/// A child of `id` of `class`, enabled.
fn component(rbx: &Rbx, id: Id, class: &str) -> Option<Id> {
    rbx.children(id).iter().copied().find(|&c| {
        let i = rbx.inst(c);
        i.class.name == class && !matches!(i.get("Enabled"), Value::Bool(false))
    })
}

/// The image an image object shows: an ImageButton's PressedImage while it
/// is pressed and its HoverImage while the pointer is over it, when they
/// are set, else its Image.
fn shown_image(inst: &super::hstudio_rbx::Inst, hover: bool, pressed: bool) -> String {
    let set = |k: &str| inst.get(k).as_text().filter(|s| !s.is_empty());
    let state = match inst.class.name {
        "ImageButton" if pressed => set("PressedImage").or_else(|| hover.then(|| set("HoverImage")).flatten()),
        "ImageButton" if hover => set("HoverImage"),
        _ => None,
    };
    state.or_else(|| set("Image")).unwrap_or_default()
}

/// How opaque CanvasGroups above `id` leave it.
fn group_alpha(rbx: &Rbx, id: Id) -> f64 {
    let mut a = 1.0;
    let mut at = Some(id);
    while let Some(i) = at {
        let inst = rbx.inst(i);
        if inst.class.name == "CanvasGroup" {
            a *= 1.0 - inst.get("GroupTransparency").to_num().unwrap_or(0.0);
        }
        at = inst.parent;
    }
    a
}

/// Draws the GUI as last laid out: each object bottom first.
pub fn draw(
    rbx: &Rbx,
    game: &mut dyn GameApi,
    now: f64,
    images: &mut super::hstudio_sunc::ImageCache,
    workspace: &std::path::Path,
) {
    let Some(frame) = &rbx.layout else { return };
    images.begin(now);
    // ScrollingFrames' bars, each drawn over what its frame holds: once the
    // drawing has gone past that.
    let mut bars_over: Vec<(Id, Vec<[f32; 4]>, f32, u32)> = Vec::new();
    for &n in &frame.layout.draw_order {
        while let Some((sf, _, _, _)) = bars_over.last()
            && !rbx.is_descendant_of(frame.node_inst[n], *sf)
        {
            draw_bars(rbx, game, frame, bars_over.pop());
        }
        let Some(p) = frame.layout.get(n) else { continue };
        if !p.shown {
            continue;
        }
        let id = frame.node_inst[n];
        let Some(inst) = rbx.try_inst(id) else { continue };
        let vis = p.visible_rect();
        if vis.max.x <= vis.min.x || vis.max.y <= vis.min.y {
            continue;
        }
        let ga = group_alpha(rbx, id);
        let gc = group_color(rbx, id);
        // A mark where the game can still cut, turn and colour the drawing:
        // then the object is drawn whole, and changed once it is drawn.
        let mark = game.dp_mark();
        let radius = p.corner_radius as f32;
        let hover = rbx.pointer.hover.first() == Some(&id);
        let pressed = rbx.pointer.pressed.is_some_and(|(b, _)| b == id);
        let auto = find(inst.class, "AutoButtonColor") && inst.get("AutoButtonColor").truthy();
        // The background, tinted by a button's state (and by a gradient's
        // middle where the drawing can't be coloured once drawn).
        let mut bg = color(inst, "BackgroundColor3");
        let mut bg_alpha = (1.0 - inst.get("BackgroundTransparency").to_num().unwrap_or(0.0)) * ga;
        let gradient = component(rbx, id, "UIGradient");
        if mark.is_none()
            && let Some(g) = gradient
        {
            let gi = rbx.inst(g);
            if let Some(Data::ColorSeq(seq)) = get(&gi.get("Color")) {
                let mid = seq.value_at(0.5);
                bg = Color3::new(bg.r * mid.r, bg.g * mid.g, bg.b * mid.b);
            }
            if let Some(Data::NumSeq(seq)) = get(&gi.get("Transparency")) {
                bg_alpha *= 1.0 - seq.value_at(0.5);
            }
        }
        if auto && p.interactable {
            if pressed {
                bg = mix(bg, Color3::new(0.0, 0.0, 0.0), 0.3);
            } else if hover {
                bg = mix(bg, Color3::new(0.0, 0.0, 0.0), 0.15);
            }
        }
        let bg = times(bg, gc);
        let r = rect4(if mark.is_some() { p.rect() } else { vis });
        let mut extent = p.rect();
        if bg_alpha > 0.0 {
            game.dp_rect(r, radius, argb(bg, bg_alpha));
        }
        // The image: a getcustomasset PNG is drawn as it is; Roblox's asset
        // ids don't load here, and show only their tint, faintly.
        if find(inst.class, "Image") {
            let img = shown_image(inst, hover && p.interactable, pressed && p.interactable);
            let ia = (1.0 - inst.get("ImageTransparency").to_num().unwrap_or(0.0)) * ga;
            // getcustomasset's images draw; Roblox's asset ids don't load here.
            let file = img
                .strip_prefix("rbxasset://horizon-studio/")
                .and_then(|p| images.file_image(workspace, p));
            match file {
                Some(Some(t)) if ia > 0.0 => {
                    draw_image(inst, p, game, images, &t, gc, ia, mark.is_none().then_some(vis))
                }
                Some(_) => {}
                None if !img.is_empty() && ia > 0.0 => {
                    game.dp_rect(r, radius, argb(times(color(inst, "ImageColor3"), gc), ia * 0.18))
                }
                _ => {}
            }
        }
        // The border, and a UIStroke on the edge.
        let border = inst.get("BorderSizePixel").to_num().unwrap_or(0.0);
        if border >= 1.0 && bg_alpha > 0.0 && radius == 0.0 {
            let c = argb(times(color(inst, "BorderColor3"), gc), bg_alpha);
            let b = border.min(8.0) as f32;
            extent = grown(extent, f64::from(b));
            let [x, y, w, h] = r;
            game.dp_rect([x - b, y - b, w + 2.0 * b, b], 0.0, c);
            game.dp_rect([x - b, y + h, w + 2.0 * b, b], 0.0, c);
            game.dp_rect([x - b, y, b, h], 0.0, c);
            game.dp_rect([x + w, y, b, h], 0.0, c);
        }
        // The object's own UIGradient colours what it drew, its UIStroke
        // aside: a UIGradient in the UIStroke colours that.
        let own = mark.and(gradient).and_then(|g| gradient_shade(rbx, g, p.rect()));
        if let (Some(m), Some(s)) = (mark, &own) {
            game.dp_shade(m, s);
        }
        let stroke_id = component(rbx, id, "UIStroke");
        let stroke = stroke_id.map(|s| {
            let si = rbx.inst(s);
            let alpha = (1.0 - si.get("Transparency").to_num().unwrap_or(0.0)) * ga;
            let thick = si.get("Thickness").to_num().unwrap_or(1.0);
            let border = enum_name(&si.get("ApplyStrokeMode")) == "Border";
            (argb(times(color(si, "Color"), gc), alpha), thick, border)
        });
        let stroke_shade = mark
            .and(stroke_id)
            .and_then(|s| component(rbx, s, "UIGradient"))
            .and_then(|g| gradient_shade(rbx, g, p.rect()));
        let text_object = p.text.is_some();
        if let Some((c, thick, border_mode)) = stroke
            && (border_mode || !text_object)
            && thick > 0.0
        {
            let t = thick.min(16.0) as f32;
            let sm = stroke_shade.as_ref().and_then(|_| game.dp_mark());
            for i in 0..t.ceil() as i32 {
                let o = i as f32 + 0.5;
                let [x, y, w, h] = r;
                game.dp_border([x - o, y - o, w + 2.0 * o, h + 2.0 * o], radius + o, c);
            }
            if let (Some(m), Some(s)) = (sm, &stroke_shade) {
                game.dp_shade(m, s);
            }
            extent = grown(extent, f64::from(t.ceil()) + 1.0);
        }
        // Text.
        if let Some(t) = &p.text {
            let placeholder = inst.class.name == "TextBox"
                && inst.get("Text").as_text().is_none_or(|s| s.is_empty())
                && rbx.pointer.focus != Some(id);
            let tc = times(
                if placeholder {
                    color(inst, "PlaceholderColor3")
                } else {
                    color(inst, "TextColor3")
                },
                gc,
            );
            let ta = (1.0 - inst.get("TextTransparency").to_num().unwrap_or(0.0)) * ga;
            let face = as_font(&inst.get("FontFace")).unwrap_or_default();
            let italic = face.style == super::hstudio_rbx_types::FontStyle::Italic;
            let font = studio_font(&face);
            let size = t.size as f32;
            let lh = inst.get("LineHeight").to_num().unwrap_or(1.0) as f32;
            // Geist's line box is 1.45 times its size; Roblox's is the size.
            let lift = (size * 1.45 - size * lh) / 2.0;
            let outline = if text_object {
                match stroke {
                    Some((c, thick, false)) if thick > 0.0 => Some((c, thick as f32)),
                    _ => {
                        let st = inst.get("TextStrokeTransparency").to_num().unwrap_or(1.0);
                        (st < 1.0).then(|| (argb(times(color(inst, "TextStrokeColor3"), gc), (1.0 - st) * ta), 1.0))
                    }
                }
            } else {
                None
            };
            let clip = p.clip;
            // A UIStroke round the text is coloured by the UIStroke's
            // gradient; TextStroke's outline is the text's, and the text's
            // own gradient colours it.
            let outline_shade = match stroke {
                Some((_, thick, false)) if text_object && thick > 0.0 => &stroke_shade,
                _ => &own,
            };
            let upright = p.turns.is_empty() || mark.is_none();
            // A TextBox being edited: its selection, behind the text.
            let edit = (rbx.pointer.focus == Some(id) && !placeholder).then(|| edit_of(rbx, id));
            if let Some((a, b)) = edit.as_ref().and_then(LineEdit::selection) {
                let band = argb(Color3::from_rgb(0.0, 120.0, 215.0), 0.45 * ga);
                for line in &t.lines {
                    let (from, to) = (a.max(line.start), b.min(line.start + line.text.len()));
                    if from >= to {
                        continue;
                    }
                    let x0 = width_to(game, t, line, from - line.start, p.scale, font);
                    let x1 = width_to(game, t, line, to - line.start, p.scale, font);
                    let at = [(line.position.x + x0) as f32, line.position.y as f32];
                    game.dp_rect([at[0], at[1], (x1 - x0) as f32, size], 0.0, band);
                }
            }
            for line in &t.lines {
                let at = [line.position.x as f32, line.position.y as f32 - lift];
                // A line wholly outside the clip goes (a turned one may turn into it).
                if upright
                    && let Some(c) = clip
                    && (line.position.y + line.height.max(t.size) < c.min.y || line.position.y > c.max.y)
                {
                    continue;
                }
                let mut o = 0.0;
                if !line.runs.is_empty() {
                    // Rich text, run by run.
                    let tm = own.as_ref().and_then(|_| game.dp_mark());
                    let paint = RunPaint {
                        color: tc,
                        alpha: ta,
                        group: (gc, ga),
                        line_height: lh,
                        outline,
                    };
                    o = draw_runs(game, line, &paint);
                    if let (Some(m), Some(s)) = (tm, &own) {
                        game.dp_shade(m, s);
                    }
                } else if let Some((c, thick)) = outline {
                    o = thick;
                    let om = (outline_shade.is_some() || italic).then(|| game.dp_mark()).flatten();
                    for (dx, dy) in [(-thick, 0.0), (thick, 0.0), (0.0, -thick), (0.0, thick)] {
                        game.dp_text(&line.text, [at[0] + dx, at[1] + dy], size, font, c);
                    }
                    if let Some(m) = om {
                        lean(game, m, italic, at[1], size);
                    }
                    if let (Some(m), Some(s)) = (om, outline_shade) {
                        game.dp_shade(m, s);
                    }
                }
                if ta > 0.0 && line.runs.is_empty() {
                    let tm = (own.is_some() || italic).then(|| game.dp_mark()).flatten();
                    game.dp_text(&line.text, at, size, font, argb(tc, ta));
                    if let Some(m) = tm {
                        lean(game, m, italic, at[1], size);
                    }
                    if let (Some(m), Some(s)) = (tm, &own) {
                        game.dp_shade(m, s);
                    }
                }
                // Geist's line box is 1.45 times the size, about the line (a
                // rich line's is its largest run's).
                let o = f64::from(o) + 1.0;
                let (x, y) = (line.position.x, f64::from(at[1]));
                let line_box = Rect::new(
                    Vector2::new(x - o, y - o),
                    Vector2::new(x + line.width + o, y + line.height.max(t.size) * 1.45 + o),
                );
                extent = Rect::new(extent.min.min(line_box.min), extent.max.max(line_box.max));
            }
            // The caret, blinking, where CursorPosition has it.
            if rbx.pointer.focus == Some(id) && (now * 2.0).fract() < 0.6 {
                let caret = edit.as_ref().and_then(|e| caret_at(game, t, e.cursor, p.scale, font));
                let (cx, cy) = match caret {
                    Some((i, x)) => (x, t.lines[i].position.y),
                    None => (
                        p.content.min.x,
                        p.content.min.y + (p.content.height() - t.size).max(0.0) / 2.0,
                    ),
                };
                game.dp_rect([cx as f32 + 1.0, cy as f32, 1.5, size], 0.0, argb(tc, ta));
            }
        }
        // Once it is drawn: the gradients of the groups it is in, its turns
        // and its clip.
        if let Some(m) = mark {
            finish(rbx, game, frame, id, p, m, extent);
        }
        // A ScrollingFrame's bars, over what it holds.
        if inst.class.name == "ScrollingFrame" {
            let bars = scroll_bars(inst, p);
            if let Some(b) = bars.first() {
                let ca = (1.0 - inst.get("ScrollBarImageTransparency").to_num().unwrap_or(0.0)) * ga;
                let c = argb(times(color(inst, "ScrollBarImageColor3"), gc), ca * 0.7);
                let thumbs = bars.iter().map(|b| rect4(b.thumb)).collect();
                bars_over.push((id, thumbs, (b.thickness / 2.0) as f32, c));
            }
        }
    }
    while let Some(bars) = bars_over.pop() {
        draw_bars(rbx, game, frame, Some(bars));
    }
}

/// How far italic letters lean: across, for each unit up.
const ITALIC_SLOPE: f32 = 0.2;

/// Leans what was drawn since `mark` as italic letters do (there is no
/// italic Geist), about the baseline of text of `size` whose line box (1.45
/// times the size) has its top at `top`; nothing when not `italic`.
fn lean(game: &mut dyn GameApi, mark: usize, italic: bool, top: f32, size: f32) {
    if italic {
        game.dp_slant(mark, top + size * 1.45 / 2.0 + size * 0.33, ITALIC_SLOPE);
    }
}

/// What a line of rich text is drawn with: the text's colour and opacity,
/// the tint and opacity of the groups it is in, LineHeight, and the
/// object's outline (a UIStroke round the text, or its TextStroke).
struct RunPaint {
    color: Color3,
    alpha: f64,
    group: (Color3, f64),
    line_height: f32,
    outline: Option<(u32, f32)>,
}

/// Draws a line of rich text run by run, each on the line's bottom: its
/// mark behind it, its stroke (or the object's outline) round it, its text
/// in its own colour, size and font, and its underline and strike-through.
/// Returns how far the strokes reach out.
fn draw_runs(game: &mut dyn GameApi, line: &layout::TextLine, paint: &RunPaint) -> f32 {
    let (gc, ga) = paint.group;
    let mut reach: f32 = 0.0;
    for run in &line.runs {
        let size = run.size as f32;
        let font = studio_font(&run.font);
        let st = &run.style;
        let (x, w) = (run.x as f32, run.width as f32);
        // Its own line box, on the line's bottom; Geist's is 1.45 times the size.
        let tall = size * paint.line_height;
        let top = (line.position.y + line.height) as f32 - tall;
        let at = [x, top - (size * 1.45 - tall) / 2.0];
        let color = st.color.map_or(paint.color, |c| times(c, gc));
        let alpha = paint.alpha * (1.0 - st.transparency.unwrap_or(0.0));
        if let Some((mc, mt)) = st.mark {
            game.dp_rect([x, top, w, tall], 0.0, argb(times(mc, gc), (1.0 - mt) * ga));
        }
        let stroke = st
            .stroke
            .map(|(sc, thick, t)| (argb(times(sc, gc), (1.0 - t) * alpha), thick as f32))
            .or(paint.outline);
        let italic = run.font.style == super::hstudio_rbx_types::FontStyle::Italic;
        let letters = italic.then(|| game.dp_mark()).flatten();
        if let Some((c, thick)) = stroke
            && thick > 0.0
        {
            reach = reach.max(thick);
            for (dx, dy) in [(-thick, 0.0), (thick, 0.0), (0.0, -thick), (0.0, thick)] {
                game.dp_text(&run.text, [at[0] + dx, at[1] + dy], size, font, c);
            }
        }
        let c = argb(color, alpha);
        if alpha > 0.0 {
            game.dp_text(&run.text, at, size, font, c);
        }
        // The letters lean; their lines don't.
        if let Some(m) = letters {
            lean(game, m, true, at[1], size);
        }
        if alpha > 0.0 {
            // The baseline: about a third of the size below the box's middle.
            let base = top + tall / 2.0 + size * 0.33;
            let thick = (size / 14.0).max(1.0);
            if st.underline {
                game.dp_rect([x, base + size * 0.08, w, thick], 0.0, c);
            }
            if st.strike {
                game.dp_rect([x, base - size * 0.3, w, thick], 0.0, c);
            }
        }
    }
    reach
}

/// Draws a ScrollingFrame's thumbs: (the frame, the thumbs, their corner
/// radius and colour), turned and cut as the frame is.
fn draw_bars(rbx: &Rbx, game: &mut dyn GameApi, frame: &Frame, bars: Option<(Id, Vec<[f32; 4]>, f32, u32)>) {
    let Some((id, thumbs, radius, c)) = bars else {
        return;
    };
    let mark = game.dp_mark();
    let mut extent: Option<Rect> = None;
    for r in thumbs {
        game.dp_rect(r, radius, c);
        let at = Rect::new(
            Vector2::new(f64::from(r[0]), f64::from(r[1])),
            Vector2::new(f64::from(r[0] + r[2]), f64::from(r[1] + r[3])),
        );
        extent = Some(extent.map_or(at, |e| Rect::new(e.min.min(at.min), e.max.max(at.max))));
    }
    if let (Some(m), Some(extent)) = (mark, extent)
        && let Some(p) = frame.inst_node.get(&id).and_then(|&n| frame.layout.get(n))
    {
        finish(rbx, game, frame, id, p, m, extent);
    }
}

/// An image object's image, placed in its box as its ScaleType,
/// ImageRectOffset and ImageRectSize, TileSize, SliceCenter and SliceScale
/// say, tinted by its ImageColor3 (and `gc`, the groups it is in), `alpha`
/// opaque; cut to `clip` where the game can't cut it once drawn.
#[allow(clippy::too_many_arguments)]
fn draw_image(
    inst: &super::hstudio_rbx::Inst,
    p: &layout::Placed,
    game: &mut dyn GameApi,
    images: &mut super::hstudio_sunc::ImageCache,
    t: &Rc<super::hstudio_meshes::Texture>,
    gc: Color3,
    alpha: f64,
    clip: Option<Rect>,
) {
    let (w, h) = (t.w, t.h);
    let fit = layout::ImageFit {
        scale_type: match enum_name(&inst.get("ScaleType")) {
            "Slice" => layout::ScaleType::Slice,
            "Tile" => layout::ScaleType::Tile,
            "Fit" => layout::ScaleType::Fit,
            "Crop" => layout::ScaleType::Crop,
            _ => layout::ScaleType::Stretch,
        },
        rect_offset: as_v2(&inst.get("ImageRectOffset")).unwrap_or_default(),
        rect_size: as_v2(&inst.get("ImageRectSize")).unwrap_or_default(),
        tile_size: as_udim2(&inst.get("TileSize")).unwrap_or(UDim2::from_scale(1.0, 1.0)),
        slice_center: match get(&inst.get("SliceCenter")) {
            Some(Data::Rect(r)) => r,
            _ => Rect::default(),
        },
        slice_scale: inst.get("SliceScale").to_num().unwrap_or(1.0),
    };
    let rgb = argb(times(color(inst, "ImageColor3"), gc), 1.0) & 0xFF_FFFF;
    let size = Vector2::new(f64::from(w), f64::from(h));
    for (place, part) in layout::image_pieces(size, p.rect(), p.scale, &fit, clip) {
        images.draw_part(game, t, rect4(place), rect4(part), rgb, alpha);
    }
}

/// How the CanvasGroups above `id` (and `id`, if it is one) colour it:
/// their GroupColor3s multiplied.
fn group_color(rbx: &Rbx, id: Id) -> Color3 {
    let mut c = Color3::new(1.0, 1.0, 1.0);
    let mut at = Some(id);
    while let Some(i) = at {
        let inst = rbx.inst(i);
        if inst.class.name == "CanvasGroup" {
            c = times(c, as_c3(&inst.get("GroupColor3")).unwrap_or(Color3::new(1.0, 1.0, 1.0)));
        }
        at = inst.parent;
    }
    c
}

/// Two colours multiplied, channel by channel.
fn times(a: Color3, b: Color3) -> Color3 {
    Color3::new(a.r * b.r, a.g * b.g, a.b * b.b)
}

/// `r` grown by `by` on every side.
fn grown(r: Rect, by: f64) -> Rect {
    Rect::new(r.min - Vector2::new(by, by), r.max + Vector2::new(by, by))
}

/// Whether `inner` lies within `outer`.
fn within(inner: Rect, outer: Rect) -> bool {
    inner.min.x >= outer.min.x && inner.min.y >= outer.min.y && inner.max.x <= outer.max.x && inner.max.y <= outer.max.y
}

/// A sequence's value on either side of `t`: two keypoints at one time are
/// a hard edge, the first's value before it and the last's after.
fn sides<T: Copy>(keys: &[(f64, T)], t: f64, at: impl Fn(f64) -> T) -> (T, T) {
    match (keys.iter().position(|k| k.0 == t), keys.iter().rposition(|k| k.0 == t)) {
        (Some(a), Some(b)) => (keys[a].1, keys[b].1),
        _ => (at(t), at(t)),
    }
}

/// A UIGradient as the colours it gives what an object at `rect` draws.
/// Its line runs left to right turned clockwise by its Rotation, through
/// the object's own unit square (so a turn of 45 runs corner to corner on a
/// square), moved by its Offset (a fraction of the object's size); its
/// Color's and its Transparency's keypoints are the stops.
fn gradient_shade(rbx: &Rbx, g: Id, rect: Rect) -> Option<Shade> {
    let gi = rbx.inst(g);
    let (w, h) = (rect.width(), rect.height());
    if !(w > 0.0 && h > 0.0) {
        return None;
    }
    let colors = match get(&gi.get("Color")) {
        Some(Data::ColorSeq(s)) => s,
        _ => ColorSequence::constant(Color3::new(1.0, 1.0, 1.0)),
    };
    let fades = match get(&gi.get("Transparency")) {
        Some(Data::NumSeq(s)) => s,
        _ => NumberSequence::constant(0.0),
    };
    let turn = gi.get("Rotation").to_num().unwrap_or(0.0).to_radians();
    let offset = as_v2(&gi.get("Offset")).unwrap_or_default();
    let (dx, dy) = (turn.cos(), turn.sin());
    // t = ((p - min) / size - 0.5 - offset) · (dx, dy) + 0.5
    let axis = [(dx / w) as f32, (dy / h) as f32];
    let base = 0.5 - dx * (rect.min.x / w + 0.5 + offset.x) - dy * (rect.min.y / h + 0.5 + offset.y);
    let color_keys: Vec<(f64, Color3)> = colors.keypoints().iter().map(|k| (k.time, k.value)).collect();
    let fade_keys: Vec<(f64, f64)> = fades.keypoints().iter().map(|k| (k.time, k.value)).collect();
    let mut ts: Vec<f64> = color_keys
        .iter()
        .map(|k| k.0)
        .chain(fade_keys.iter().map(|k| k.0))
        .collect();
    ts.sort_by(f64::total_cmp);
    ts.dedup();
    let mut stops = Vec::with_capacity(ts.len() + 2);
    for t in ts {
        let (c0, c1) = sides(&color_keys, t, |t| colors.value_at(t));
        let (f0, f1) = sides(&fade_keys, t, |t| fades.value_at(t));
        stops.push((t as f32, argb(c0, 1.0 - f0)));
        if (c0, f0) != (c1, f1) {
            stops.push((t as f32, argb(c1, 1.0 - f1)));
        }
    }
    Some(Shade {
        axis,
        base: base as f32,
        stops,
    })
}

/// The UIGradient of CanvasGroup `group`, as the colours it gives what is
/// drawn in the group (a CanvasGroup's gradient colours all it holds).
fn group_gradient(rbx: &Rbx, frame: &Frame, group: Id) -> Option<Shade> {
    let g = component(rbx, group, "UIGradient")?;
    let p = frame.layout.get(*frame.inst_node.get(&group)?)?;
    gradient_shade(rbx, g, p.rect())
}

/// What an object's drawing since `mark` gets once it is all drawn: the
/// gradients of the CanvasGroups it is in, its rotation and its turned
/// ancestors' (innermost first, so a group's gradient turns with the group),
/// then the cut to what clips it, when its `extent` reaches out of that.
fn finish(rbx: &Rbx, game: &mut dyn GameApi, frame: &Frame, id: Id, p: &layout::Placed, mark: usize, extent: Rect) {
    let mut turns = p.turns.iter().peekable();
    let mut at = Some(id);
    while let Some(a) = at {
        let inst = rbx.inst(a);
        if is_a(inst.class, "LayerCollector") {
            break;
        }
        if a != id
            && inst.class.name == "CanvasGroup"
            && let Some(s) = group_gradient(rbx, frame, a)
        {
            game.dp_shade(mark, &s);
        }
        if let Some(t) = turns.next_if(|t| frame.node_inst.get(t.node) == Some(&a)) {
            game.dp_rotate(mark, [t.centre.x as f32, t.centre.y as f32], t.degrees as f32);
        }
        at = inst.parent;
    }
    for t in turns {
        game.dp_rotate(mark, [t.centre.x as f32, t.centre.y as f32], t.degrees as f32);
    }
    if let Some(c) = p.clip
        && (!p.turns.is_empty() || !within(extent, c))
    {
        game.dp_clip(mark, rect4(c));
    }
}

/// A ScrollingFrame's scroll bar on screen: its track, its thumb, and
/// whether it scrolls down (else across).
#[derive(Clone, Copy, Debug)]
struct Bar {
    vertical: bool,
    track: Rect,
    thumb: Rect,
    thickness: f64,
}

impl Bar {
    /// Where a rectangle starts along the bar, and how long it is.
    fn along(&self, r: Rect) -> (f64, f64) {
        if self.vertical {
            (r.min.y, r.max.y - r.min.y)
        } else {
            (r.min.x, r.max.x - r.min.x)
        }
    }
}

/// The scroll bars a ScrollingFrame shows: the vertical one, on its right
/// (or its left, with VerticalScrollBarPosition Left), while its canvas is
/// taller than its window and it scrolls down; the horizontal one at its
/// bottom while the canvas is wider and it scrolls across. When both show,
/// neither runs into the corner they share.
fn scroll_bars(inst: &super::hstudio_rbx::Inst, p: &layout::Placed) -> Vec<Bar> {
    let thick = inst.get("ScrollBarThickness").to_num().unwrap_or(12.0) * p.scale;
    if thick <= 0.0 {
        return Vec::new();
    }
    let direction = enum_name(&inst.get("ScrollingDirection"));
    let down = direction != "X" && p.canvas_size.y > p.window_size.y + 0.5;
    let across = direction != "Y" && p.canvas_size.x > p.window_size.x + 0.5;
    let left = enum_name(&inst.get("VerticalScrollBarPosition")) == "Left";
    // How far the canvas scrolls each way, in its own (unscaled) units.
    let room = (p.canvas_size - p.window_size) / p.scale.max(1e-9);
    let mut bars = Vec::new();
    let mut bar = |vertical: bool, track: Rect, window: f64, canvas: f64, at: f64, room: f64| {
        let ratio = (window / canvas).clamp(0.05, 1.0);
        let off = if room > 0.0 { (at / room).clamp(0.0, 1.0) } else { 0.0 };
        let thumb = if vertical {
            let len = track.height() * ratio;
            let y = track.min.y + (track.height() - len) * off;
            Rect::new(Vector2::new(track.min.x, y), Vector2::new(track.max.x, y + len))
        } else {
            let len = track.width() * ratio;
            let x = track.min.x + (track.width() - len) * off;
            Rect::new(Vector2::new(x, track.min.y), Vector2::new(x + len, track.max.y))
        };
        bars.push(Bar {
            vertical,
            track,
            thumb,
            thickness: thick,
        });
    };
    let (x, y, w, h) = (p.position.x, p.position.y, p.size.x, p.size.y);
    if down {
        let bx = if left { x } else { x + w - thick };
        let bottom = y + h - if across { thick } else { 0.0 };
        let track = Rect::new(Vector2::new(bx, y), Vector2::new(bx + thick, bottom));
        bar(
            true,
            track,
            p.window_size.y,
            p.canvas_size.y,
            p.canvas_position.y,
            room.y,
        );
    }
    if across {
        let from = x + if down && left { thick } else { 0.0 };
        let to = x + w - if down && !left { thick } else { 0.0 };
        let track = Rect::new(Vector2::new(from, y + h - thick), Vector2::new(to, y + h));
        bar(
            false,
            track,
            p.window_size.x,
            p.canvas_size.x,
            p.canvas_position.x,
            room.x,
        );
    }
    bars
}

/// The scroll bar under the pointer, of the topmost ScrollingFrame with one
/// there.
fn bar_at(rbx: &Rbx) -> Option<(Id, Bar)> {
    let f = rbx.layout.as_ref()?;
    let at = Vector2::new(rbx.pointer.x, rbx.pointer.y);
    rbx.pointer.hover.iter().find_map(|&id| {
        let inst = rbx.try_inst(id).filter(|i| i.class.name == "ScrollingFrame")?;
        if !inst.get("ScrollingEnabled").truthy() {
            return None;
        }
        let p = f.layout.get(*f.inst_node.get(&id)?)?;
        let inside = |r: Rect| at.x >= r.min.x && at.x < r.max.x && at.y >= r.min.y && at.y < r.max.y;
        scroll_bars(inst, p)
            .into_iter()
            .find(|b| inside(b.track))
            .map(|b| (id, b))
    })
}

/// A press on a ScrollingFrame's bar takes hold of its thumb (a press on
/// the track beside the thumb brings the thumb's middle there first).
fn grab_bar(rbx: &mut Rbx) -> bool {
    let Some((id, bar)) = bar_at(rbx) else {
        return false;
    };
    let at = if bar.vertical { rbx.pointer.y } else { rbx.pointer.x };
    let (start, len) = bar.along(bar.thumb);
    let grip = if at >= start && at < start + len {
        at - start
    } else {
        len / 2.0
    };
    rbx.pointer.bar = Some((id, bar.vertical, grip));
    drag_bar(rbx);
    true
}

/// The thumb held follows the pointer, and the canvas with it.
fn drag_bar(rbx: &mut Rbx) {
    let Some((id, vertical, grip)) = rbx.pointer.bar else {
        return;
    };
    let Some(f) = &rbx.layout else { return };
    let Some(p) = f.inst_node.get(&id).and_then(|&n| f.layout.get(n)) else {
        return;
    };
    // The frame may have gone since the press.
    let Some(inst) = rbx.try_inst(id).filter(|i| !i.destroyed) else {
        rbx.pointer.bar = None;
        return;
    };
    let Some(bar) = scroll_bars(inst, p).into_iter().find(|b| b.vertical == vertical) else {
        return;
    };
    let room = (p.canvas_size - p.window_size) / p.scale.max(1e-9);
    let (track, track_len) = bar.along(bar.track);
    let (_, len) = bar.along(bar.thumb);
    let at = if vertical { rbx.pointer.y } else { rbx.pointer.x };
    let off = if track_len > len {
        ((at - grip - track) / (track_len - len)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let pos = as_v2(&rbx.raw_get(id, "CanvasPosition")).unwrap_or_default();
    let next = if vertical {
        Vector2::new(pos.x, off * room.y.max(0.0))
    } else {
        Vector2::new(off * room.x.max(0.0), pos.y)
    };
    rbx.raw_set(id, "CanvasPosition", v2(next));
}

/// The GuiObjects under a point, topmost first.
pub fn objects_at(rbx: &Rbx, x: f64, y: f64) -> Vec<Id> {
    let Some(f) = &rbx.layout else { return Vec::new() };
    f.layout
        .objects_at(Vector2::new(x, y))
        .into_iter()
        .map(|n| f.node_inst[n])
        .collect()
}

/// Whether the GUI wants the pointer: a Modal button or a focused TextBox
/// shows, or Left Alt asked for it over a GUI one can click.
pub fn wants_pointer(rbx: &Rbx) -> bool {
    let Some(f) = &rbx.layout else { return false };
    let shown = |n: &NodeId| f.layout.get(*n).is_some_and(|p| p.shown);
    let mut clickable = false;
    for n in f.layout.draw_order.iter().filter(|n| shown(n)) {
        let Some(inst) = rbx.try_inst(f.node_inst[*n]) else {
            continue;
        };
        if find(inst.class, "Modal") && inst.get("Modal").truthy() {
            return true;
        }
        if find(inst.class, "AutoButtonColor") || inst.class.name == "TextBox" || inst.class.name == "ScrollingFrame" {
            clickable = true;
        }
    }
    if rbx.pointer.focus.is_some() {
        return true;
    }
    rbx.pointer.asked && clickable
}

// ---------------------------------------------------------------- input

/// An InputObject for scripts: a table with Roblox's fields.
pub fn input_object(rbx: &mut Rbx, key: Option<&'static str>, kind: &str, state: &str, pos: Option<Vector3>) -> Value {
    let mut t = Table::new();
    t.set_str("KeyCode", enum_value(enum_item("KeyCode", key.unwrap_or("Unknown"))));
    t.set_str("UserInputType", enum_value(enum_item("UserInputType", kind)));
    t.set_str("UserInputState", enum_value(enum_item("UserInputState", state)));
    let p = pos.unwrap_or(Vector3::new(rbx.pointer.x, rbx.pointer.y, 0.0));
    t.set_str("Position", v3(p));
    t.set_str("Delta", v3(Vector3::ZERO));
    t.set_str("ClassName", Value::str("InputObject"));
    t.set_str("Name", Value::str("InputObject"));
    Value::table(t)
}

/// The KeyCode of a key as the game names it ("key.keyboard.e").
pub fn keycode(key: &str) -> Option<&'static str> {
    let k = key.strip_prefix("key.keyboard.")?;
    let letters = [
        "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V",
        "W", "X", "Y", "Z",
    ];
    let digits = [
        "Zero", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine",
    ];
    if k.len() == 1 {
        let c = k.as_bytes()[0];
        if c.is_ascii_lowercase() {
            return Some(letters[(c - b'a') as usize]);
        }
        if c.is_ascii_digit() {
            return Some(digits[(c - b'0') as usize]);
        }
    }
    if let Some(n) = k.strip_prefix('f').and_then(|n| n.parse::<usize>().ok())
        && (1..=15).contains(&n)
    {
        const F: [&str; 15] = [
            "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "F13", "F14", "F15",
        ];
        return Some(F[n - 1]);
    }
    if let Some(n) = k.strip_prefix("keypad.").and_then(|n| n.parse::<usize>().ok())
        && n < 10
    {
        const P: [&str; 10] = [
            "KeypadZero",
            "KeypadOne",
            "KeypadTwo",
            "KeypadThree",
            "KeypadFour",
            "KeypadFive",
            "KeypadSix",
            "KeypadSeven",
            "KeypadEight",
            "KeypadNine",
        ];
        return Some(P[n]);
    }
    Some(match k {
        "space" => "Space",
        "enter" => "Return",
        "escape" => "Escape",
        "tab" => "Tab",
        "backspace" => "Backspace",
        "delete" => "Delete",
        "insert" => "Insert",
        "home" => "Home",
        "end" => "End",
        "page.up" => "PageUp",
        "page.down" => "PageDown",
        "up" => "Up",
        "down" => "Down",
        "left" => "Left",
        "right" => "Right",
        "left.shift" => "LeftShift",
        "right.shift" => "RightShift",
        "left.control" => "LeftControl",
        "right.control" => "RightControl",
        "left.alt" => "LeftAlt",
        "right.alt" => "RightAlt",
        "left.win" => "LeftSuper",
        "right.win" => "RightSuper",
        "caps.lock" => "CapsLock",
        "num.lock" => "NumLock",
        "scroll.lock" => "ScrollLock",
        "print.screen" => "Print",
        "pause" => "Pause",
        "menu" => "Menu",
        "minus" => "Minus",
        "equal" => "Equals",
        "comma" => "Comma",
        "period" => "Period",
        "slash" => "Slash",
        "semicolon" => "Semicolon",
        "apostrophe" => "Quote",
        "left.bracket" => "LeftBracket",
        "right.bracket" => "RightBracket",
        "backslash" => "BackSlash",
        "grave.accent" => "Backquote",
        "keypad.enter" => "KeypadEnter",
        "keypad.add" => "KeypadPlus",
        "keypad.subtract" => "KeypadMinus",
        "keypad.multiply" => "KeypadMultiply",
        "keypad.divide" => "KeypadDivide",
        "keypad.decimal" => "KeypadPeriod",
        "keypad.equal" => "KeypadEquals",
        _ => return None,
    })
}

/// The KeyCode of a key a screen names ("enter", "a").
fn screen_keycode(key: &str) -> Option<&'static str> {
    match key {
        "enter" => Some("Return"),
        "backspace" => Some("Backspace"),
        "delete" => Some("Delete"),
        "tab" => Some("Tab"),
        "left" => Some("Left"),
        "right" => Some("Right"),
        "up" => Some("Up"),
        "down" => Some("Down"),
        "home" => Some("Home"),
        "end" => Some("End"),
        "pageup" => Some("PageUp"),
        "pagedown" => Some("PageDown"),
        "escape" => Some("Escape"),
        " " => Some("Space"),
        "`" => Some("Backquote"),
        k if k.chars().count() == 1 => keycode(&format!("key.keyboard.{}", k.to_ascii_lowercase())),
        _ => None,
    }
}

/// A key or mouse button as the game reports it while the player plays:
/// UserInputService's events, ContextActionService's actions and the Mouse.
pub fn game_input(rbx: &mut Rbx, key: &str, down: bool, processed: bool) {
    if key == "key.keyboard.left.alt" && down && !processed {
        rbx.pointer.asked = !rbx.pointer.asked;
        rbx.pointer.dismissed = false;
    }
    if let Some(button) = key.strip_prefix("key.mouse.") {
        let (kind, i) = match button {
            "left" => ("MouseButton1", 0),
            "right" => ("MouseButton2", 1),
            "middle" => ("MouseButton3", 2),
            _ => return,
        };
        rbx.pointer.buttons[i] = down;
        mouse_input(rbx, kind, down, processed);
        return;
    }
    if let Some(k) = keycode(key) {
        key_input(rbx, k, down, processed);
    }
}

fn key_input(rbx: &mut Rbx, k: &'static str, down: bool, processed: bool) {
    if down {
        rbx.pointer.held.insert(k);
    } else {
        rbx.pointer.held.remove(k);
    }
    let state = if down { "Begin" } else { "End" };
    let input = input_object(rbx, Some(k), "Keyboard", state, None);
    let uis = rbx.service("UserInputService").unwrap_or(rbx.game);
    let event = if down { "InputBegan" } else { "InputEnded" };
    rbx.fire(uis, event, vec![input.clone(), Value::Bool(processed)]);
    if down && k == "Space" && !processed {
        rbx.fire(uis, "JumpRequest", Vec::new());
    }
    if processed {
        return;
    }
    let item = enum_item("KeyCode", k);
    let hit = rbx
        .actions
        .iter()
        .find(|a| a.3.contains(&item))
        .map(|a| (a.0.clone(), a.1, a.2.clone()));
    if let Some((name, owner, f)) = hit {
        let st = enum_value(enum_item("UserInputState", state));
        rbx.deferred.push((owner, f, vec![Value::str(&name), st, input]));
    }
}

fn mouse_input(rbx: &mut Rbx, kind: &'static str, down: bool, processed: bool) {
    let state = if down { "Begin" } else { "End" };
    let input = input_object(rbx, None, kind, state, None);
    let uis = rbx.service("UserInputService").unwrap_or(rbx.game);
    let event = if down { "InputBegan" } else { "InputEnded" };
    rbx.fire(uis, event, vec![input.clone(), Value::Bool(processed)]);
    let mouse = rbx.mouse;
    match (kind, down) {
        ("MouseButton1", true) => rbx.fire(mouse, "Button1Down", Vec::new()),
        ("MouseButton1", false) => rbx.fire(mouse, "Button1Up", Vec::new()),
        ("MouseButton2", true) => rbx.fire(mouse, "Button2Down", Vec::new()),
        ("MouseButton2", false) => rbx.fire(mouse, "Button2Up", Vec::new()),
        _ => {}
    }
    let item = enum_item("UserInputType", kind);
    if !processed
        && let Some((name, owner, f)) = rbx
            .actions
            .iter()
            .find(|a| a.3.contains(&item))
            .map(|a| (a.0.clone(), a.1, a.2.clone()))
    {
        let st = enum_value(enum_item("UserInputState", state));
        rbx.deferred.push((owner, f, vec![Value::str(&name), st, input]));
    }
}

pub fn focus(rbx: &mut Rbx, id: Option<Id>) {
    if rbx.pointer.focus == id {
        return;
    }
    unfocus(rbx, false);
    let Some(id) = id else { return };
    rbx.pointer.focus = Some(id);
    // An empty TextBox shows its placeholder only without the focus.
    rbx.gui_epoch += 1;
    if rbx.raw_get(id, "ClearTextOnFocus").truthy() && rbx.raw_get(id, "TextEditable").truthy() {
        rbx.raw_set(id, "Text", Value::str(""));
    }
    // The caret starts at the end, nothing selected.
    let end = rbx.raw_get(id, "Text").as_bytes().map_or(0, <[u8]>::len);
    rbx.raw_set(id, "CursorPosition", Value::Num(end as f64 + 1.0));
    rbx.raw_set(id, "SelectionStart", Value::Num(-1.0));
    rbx.fire(id, "Focused", Vec::new());
    let uis = rbx.service("UserInputService").unwrap_or(rbx.game);
    let v = rbx.value(id);
    rbx.fire(uis, "TextBoxFocused", vec![v]);
}

pub fn unfocus(rbx: &mut Rbx, enter: bool) {
    let Some(id) = rbx.pointer.focus.take() else { return };
    rbx.gui_epoch += 1;
    rbx.pointer.selecting = None;
    // Not being edited, a TextBox has no caret.
    rbx.raw_set(id, "CursorPosition", Value::Num(-1.0));
    rbx.raw_set(id, "SelectionStart", Value::Num(-1.0));
    let input = input_object(
        rbx,
        Some(if enter { "Return" } else { "Unknown" }),
        "Keyboard",
        "Begin",
        None,
    );
    rbx.fire(id, "FocusLost", vec![Value::Bool(enter), input]);
    let uis = rbx.service("UserInputService").unwrap_or(rbx.game);
    let v = rbx.value(id);
    rbx.fire(uis, "TextBoxFocusReleased", vec![v]);
}

fn xy(lua: &Lua, args: &[Value]) -> Result<(f64, f64), Throw> {
    Ok((check_num(lua, args, 0, "screen")?, check_num(lua, args, 1, "screen")?))
}

/// The pointer's place for GUI events: x, y.
fn at(rbx: &Rbx) -> Vec<Value> {
    vec![Value::Num(rbx.pointer.x), Value::Num(rbx.pointer.y)]
}

/// The calls of the screen the GUI takes the pointer with (canvas, dp).
pub fn screen_call(env: &mut dyn Env, lua: &mut Lua, which: u32, args: &[Value]) -> Res {
    let now = env.now();
    match which {
        // draw(w, h)
        0 => {
            let (rbx, game) = env.rbx_and_game();
            lay_out(rbx, game);
            let mut images = std::mem::take(&mut rbx.screen_images);
            draw(rbx, game, now, &mut images, &rbx.workspace_dir.clone());
            rbx.screen_images = images;
            Ok(Vec::new())
        }
        // move(x, y)
        1 => {
            let (x, y) = xy(lua, args)?;
            let (rbx, game) = env.rbx_and_game();
            pointer_moved(rbx, x, y);
            // A press in a TextBox being dragged selects its text.
            if let Some(s) = rbx.pointer.selecting
                && rbx.pointer.focus == Some(s)
            {
                caret_to_pointer(rbx, game, s, true);
            }
            Ok(Vec::new())
        }
        // click(x, y, button)
        2 => {
            let (x, y) = xy(lua, args)?;
            let button = args.get(2).and_then(Value::as_text);
            let (rbx, game) = env.rbx_and_game();
            pointer_moved(rbx, x, y);
            if button.as_deref() == Some("middle") {
                middle_button(rbx, true);
            } else {
                let left = button.is_none_or(|b| b == "left");
                pointer_button(rbx, left, true);
                // A press in the focused TextBox puts its caret there.
                if left
                    && let Some(f) = rbx.pointer.focus
                    && hits(rbx).0 == Some(f)
                {
                    rbx.pointer.selecting = Some(f);
                    caret_to_pointer(rbx, game, f, false);
                }
            }
            Ok(Vec::new())
        }
        // release(x, y, button)
        3 => {
            let (x, y) = xy(lua, args)?;
            let button = args.get(2).and_then(Value::as_text);
            let rbx = env.rbx();
            pointer_moved(rbx, x, y);
            if button.as_deref() == Some("middle") {
                middle_button(rbx, false);
            } else {
                let left = button.is_none_or(|b| b == "left");
                pointer_button(rbx, left, false);
                if left {
                    rbx.pointer.selecting = None;
                }
            }
            Ok(Vec::new())
        }
        // key(key, down, mods)
        4 => {
            let key = check_text(lua, args, 0, "key")?;
            let down = args.get(1).is_some_and(Value::truthy);
            let mods = mods_arg(args.get(2));
            let (rbx, game) = env.rbx_and_game();
            if let Some(f) = rbx.pointer.focus {
                // Keys typed into a TextBox are the GUI's: gameProcessedEvent.
                if let Some(k) = screen_keycode(&key) {
                    key_input(rbx, k, down, true);
                }
                if down {
                    textbox_key(rbx, game, f, &key, mods);
                }
                return Ok(Vec::new());
            }
            if let Some(k) = screen_keycode(&key) {
                key_input(rbx, k, down, false);
            }
            Ok(Vec::new())
        }
        // text(t)
        5 => {
            let t = check_text(lua, args, 0, "text")?;
            type_text(env.rbx(), &t);
            Ok(Vec::new())
        }
        // wheel(steps)
        6 => {
            let steps = check_num(lua, args, 0, "wheel")?;
            wheel(env.rbx(), steps);
            Ok(Vec::new())
        }
        // escape(): a TextBox being edited lets go first, as in Roblox, and
        // the screen stays (it goes by itself when nothing else wants it).
        8 => {
            let rbx = env.rbx();
            let editing = rbx.pointer.focus.is_some();
            unfocus(rbx, false);
            Ok(vec![Value::Bool(editing)])
        }
        // close()
        _ => {
            let rbx = env.rbx();
            rbx.pointer.free = false;
            rbx.pointer.dismissed = true;
            rbx.pointer.asked = false;
            unfocus(rbx, false);
            leave_all(rbx);
            Ok(Vec::new())
        }
    }
}

fn leave_all(rbx: &mut Rbx) {
    let before = gui_states(rbx);
    let (x, y) = (rbx.pointer.x, rbx.pointer.y);
    for id in std::mem::take(&mut rbx.pointer.hover) {
        rbx.fire(id, "MouseLeave", vec![Value::Num(x), Value::Num(y)]);
    }
    rbx.pointer.pressed = None;
    rbx.pointer.drag = None;
    rbx.pointer.bar = None;
    rbx.pointer.selecting = None;
    gui_states_changed(rbx, before);
}

/// The GUI's screen went without its button's release reaching the GUI
/// (nothing wanted the pointer any more, or the studio closed it): what
/// the held button was doing ends, a press, a Draggable's drag, a scroll
/// bar held or a selection.
pub fn let_go(rbx: &mut Rbx) {
    let before = gui_states(rbx);
    rbx.pointer.pressed = None;
    rbx.pointer.drag = None;
    rbx.pointer.bar = None;
    rbx.pointer.selecting = None;
    gui_states_changed(rbx, before);
}

/// A GuiObject's GuiState: NonInteractable unless Interactable, then Press
/// while it is the button pressed, Hover while the pointer is over it, and
/// Idle.
pub fn gui_state(rbx: &Rbx, id: Id) -> &'static str {
    if rbx.try_inst(id).is_some_and(|i| !i.get("Interactable").truthy()) {
        "NonInteractable"
    } else if rbx.pointer.pressed.is_some_and(|(p, _)| p == id) {
        "Press"
    } else if rbx.pointer.hover.contains(&id) {
        "Hover"
    } else {
        "Idle"
    }
}

/// The GuiState of each object the pointer is over or pressing.
fn gui_states(rbx: &Rbx) -> Vec<(Id, &'static str)> {
    let mut ids = rbx.pointer.hover.clone();
    if let Some((p, _)) = rbx.pointer.pressed
        && !ids.contains(&p)
    {
        ids.push(p);
    }
    ids.into_iter()
        .filter(|&id| rbx.try_inst(id).is_some())
        .map(|id| (id, gui_state(rbx, id)))
        .collect()
}

/// Fires GuiState's signal for each object whose state is not what it was
/// in `before` (an object in only one of the two was at rest in the other).
fn gui_states_changed(rbx: &mut Rbx, before: Vec<(Id, &'static str)>) {
    let after = gui_states(rbx);
    let resting = |rbx: &Rbx, id: Id| {
        if rbx.try_inst(id).is_some_and(|i| !i.get("Interactable").truthy()) {
            "NonInteractable"
        } else {
            "Idle"
        }
    };
    let mut changed = Vec::new();
    for &(id, old) in &before {
        let new = after
            .iter()
            .find(|a| a.0 == id)
            .map_or_else(|| resting(rbx, id), |a| a.1);
        if new != old {
            changed.push(id);
        }
    }
    for &(id, new) in &after {
        if !before.iter().any(|b| b.0 == id) && new != resting(rbx, id) {
            changed.push(id);
        }
    }
    for id in changed {
        rbx.fire(id, "prop:GuiState", Vec::new());
    }
}

pub fn pointer_moved(rbx: &mut Rbx, x: f64, y: f64) {
    let before = gui_states(rbx);
    let (ox, oy) = (rbx.pointer.x, rbx.pointer.y);
    rbx.pointer.x = x;
    rbx.pointer.y = y;
    if let Some((id, lx, ly)) = rbx.pointer.drag {
        if let Some(pos) = as_udim2(&rbx.raw_get(id, "Position")) {
            let moved = UDim2::new(
                pos.x.scale,
                f64::from(pos.x.offset) + (x - lx),
                pos.y.scale,
                f64::from(pos.y.offset) + (y - ly),
            );
            rbx.raw_set(id, "Position", super::hstudio_rbx_values::udim2(moved));
        }
        rbx.pointer.drag = Some((id, x, y));
    }
    drag_bar(rbx);
    let over = objects_at(rbx, x, y);
    let old = std::mem::take(&mut rbx.pointer.hover);
    for &id in old.iter().filter(|i| !over.contains(i)) {
        rbx.fire(id, "MouseLeave", vec![Value::Num(x), Value::Num(y)]);
    }
    for &id in over.iter().filter(|i| !old.contains(i)) {
        rbx.fire(id, "MouseEnter", vec![Value::Num(x), Value::Num(y)]);
    }
    for &id in &over {
        rbx.fire(id, "MouseMoved", vec![Value::Num(x), Value::Num(y)]);
    }
    rbx.pointer.hover = over;
    if (ox, oy) != (x, y) {
        let input = input_object(rbx, None, "MouseMovement", "Change", None);
        let uis = rbx.service("UserInputService").unwrap_or(rbx.game);
        let processed = !rbx.pointer.hover.is_empty();
        rbx.fire(uis, "InputChanged", vec![input.clone(), Value::Bool(processed)]);
        if let Some(&top) = rbx.pointer.hover.first() {
            rbx.fire(top, "InputChanged", vec![input]);
        }
        let mouse = rbx.mouse;
        rbx.quiet_set(mouse, "X", Value::Num(x));
        rbx.quiet_set(mouse, "Y", Value::Num(y));
        rbx.fire(mouse, "Move", Vec::new());
    }
    gui_states_changed(rbx, before);
}

/// The topmost Active object under the pointer, and the button there.
fn hits(rbx: &Rbx) -> (Option<Id>, Option<Id>) {
    let Some(f) = &rbx.layout else { return (None, None) };
    let p = Vector2::new(rbx.pointer.x, rbx.pointer.y);
    (
        f.layout.hit_test(p).map(|n| f.node_inst[n]),
        f.layout.button_at(p).map(|n| f.node_inst[n]),
    )
}

/// The middle button over the GUI: InputBegan and InputEnded of the GUI
/// object under the pointer and of UserInputService, as MouseButton3
/// (Roblox's buttons have no click of their own for it).
pub fn middle_button(rbx: &mut Rbx, down: bool) {
    rbx.pointer.buttons[2] = down;
    let (top, _) = hits(rbx);
    let state = if down { "Begin" } else { "End" };
    let input = input_object(rbx, None, "MouseButton3", state, None);
    if let Some(t) = top {
        rbx.fire(t, if down { "InputBegan" } else { "InputEnded" }, vec![input]);
    }
    mouse_input(rbx, "MouseButton3", down, top.is_some());
}

pub fn pointer_button(rbx: &mut Rbx, left: bool, down: bool) {
    let before = gui_states(rbx);
    let kind = if left { "MouseButton1" } else { "MouseButton2" };
    rbx.pointer.buttons[usize::from(!left)] = down;
    // A press on a ScrollingFrame's bar takes hold of it, and is the GUI's.
    if left && !down {
        rbx.pointer.bar = None;
    }
    if left && down && grab_bar(rbx) {
        mouse_input(rbx, kind, true, true);
        gui_states_changed(rbx, before);
        return;
    }
    let (top, button) = hits(rbx);
    let over_gui = top.is_some();
    let state = if down { "Begin" } else { "End" };
    let input = input_object(rbx, None, kind, state, None);
    if let Some(t) = top {
        rbx.fire(t, if down { "InputBegan" } else { "InputEnded" }, vec![input.clone()]);
    }
    mouse_input(rbx, kind, down, over_gui);
    let args = at(rbx);
    if down {
        // A click on a TextBox focuses it; anywhere else lets go.
        let textbox = top.filter(|&t| rbx.inst(t).class.name == "TextBox");
        if left {
            match textbox {
                Some(t) => focus(rbx, Some(t)),
                None => unfocus(rbx, false),
            }
        }
        if let Some(b) = button {
            rbx.pointer.pressed = Some((b, left));
            rbx.fire(
                b,
                if left { "MouseButton1Down" } else { "MouseButton2Down" },
                args.clone(),
            );
        }
        if left
            && let Some(t) = top
            && rbx.raw_get(t, "Draggable").truthy()
        {
            rbx.pointer.drag = Some((t, rbx.pointer.x, rbx.pointer.y));
        }
    } else {
        if left {
            rbx.pointer.drag = None;
        }
        if let Some(b) = button {
            rbx.fire(b, if left { "MouseButton1Up" } else { "MouseButton2Up" }, args.clone());
        }
        if let Some((p, l)) = rbx.pointer.pressed.take()
            && l == left
            && button == Some(p)
        {
            if left {
                rbx.fire(p, "MouseButton1Click", Vec::new());
                rbx.fire(p, "Activated", vec![input, Value::Num(1.0)]);
            } else {
                rbx.fire(p, "MouseButton2Click", Vec::new());
            }
        }
    }
    gui_states_changed(rbx, before);
}

pub fn wheel(rbx: &mut Rbx, steps: f64) {
    let over = rbx.pointer.hover.clone();
    let mouse = rbx.mouse;
    rbx.fire(
        mouse,
        if steps > 0.0 { "WheelForward" } else { "WheelBackward" },
        Vec::new(),
    );
    let input = input_object(
        rbx,
        None,
        "MouseWheel",
        "Change",
        Some(Vector3::new(rbx.pointer.x, rbx.pointer.y, steps)),
    );
    let uis = rbx.service("UserInputService").unwrap_or(rbx.game);
    rbx.fire(uis, "InputChanged", vec![input, Value::Bool(!over.is_empty())]);
    for &id in &over {
        let args = at(rbx);
        rbx.fire(
            id,
            if steps > 0.0 {
                "MouseWheelForward"
            } else {
                "MouseWheelBackward"
            },
            args,
        );
    }
    if wheel_pages(rbx, &over, steps) {
        return;
    }
    // The topmost ScrollingFrame under the pointer scrolls.
    let Some(sf) = over
        .iter()
        .copied()
        .find(|&i| rbx.inst(i).class.name == "ScrollingFrame" && rbx.raw_get(i, "ScrollingEnabled").truthy())
    else {
        return;
    };
    let Some(f) = &rbx.layout else { return };
    let Some(p) = f.inst_node.get(&sf).and_then(|&n| f.layout.get(n)) else {
        return;
    };
    // How far the canvas scrolls, in its own units (a UIScale scales the frame).
    let max = Vector2::new(
        (p.canvas_size.x - p.window_size.x).max(0.0),
        (p.canvas_size.y - p.window_size.y).max(0.0),
    ) / p.scale.max(1e-9);
    let pos = as_v2(&rbx.raw_get(sf, "CanvasPosition")).unwrap_or_default();
    // ScrollingDirection X scrolls across, Y down, and XY down while it can.
    let vertical = match enum_name(&rbx.raw_get(sf, "ScrollingDirection")) {
        "X" => false,
        "Y" => true,
        _ => max.y > 0.0,
    };
    let delta = -steps * 40.0;
    let next = if vertical {
        Vector2::new(pos.x, (pos.y + delta).clamp(0.0, max.y))
    } else {
        Vector2::new((pos.x + delta).clamp(0.0, max.x), pos.y)
    };
    rbx.raw_set(sf, "CanvasPosition", v2(next));
}

// ---------------------------------------------------------------- the world

fn rgb_u32(c: Color3, alpha: f64) -> u32 {
    argb(c, alpha)
}

fn arr(v: Vector3) -> [f64; 3] {
    [v.x, v.y, v.z]
}

/// The Workspace's visible parts, in the world (inside on("world")).
pub fn draw_parts(rbx: &Rbx, game: &mut dyn GameApi, budget: &mut usize) {
    if !rbx.active {
        return;
    }
    for id in rbx.descendants(rbx.workspace) {
        if *budget == 0 {
            return;
        }
        let inst = rbx.inst(id);
        if !is_a(inst.class, "BasePart") || inst.class.name == "Terrain" {
            continue;
        }
        let t = inst.get("Transparency").to_num().unwrap_or(0.0);
        if t >= 1.0 {
            continue;
        }
        let frame = as_cf(&inst.get("CFrame")).unwrap_or_default();
        let size = as_v3(&inst.get("Size")).unwrap_or(Vector3::ONE);
        let c = as_c3(&inst.get("Color")).unwrap_or_default();
        let neon = as_item(&inst.get("Material")).is_some_and(|m| m.name == "Neon");
        let col = rgb_u32(c, 1.0 - t);
        let shape = as_item(&inst.get("Shape")).map_or("Block", |s| s.name);
        let quads = match shape {
            "Ball" => sphere(frame, size.x.min(size.y).min(size.z) / 2.0),
            "Cylinder" => cylinder(frame, size),
            _ => block(frame, size),
        };
        let light = Some(arr(frame.position()));
        for (corners, normal) in quads {
            if *budget == 0 {
                return;
            }
            *budget -= 1;
            if neon {
                game.world_quad(corners, col);
            } else {
                game.world_quad_lit(corners, col, normal, light);
            }
        }
    }
}

type Quad = ([[f64; 3]; 4], [f64; 3]);

fn block(f: CFrame, s: Vector3) -> Vec<Quad> {
    let h = s / 2.0;
    let p = |x: f64, y: f64, z: f64| arr(f.point_to_world(Vector3::new(x * h.x, y * h.y, z * h.z)));
    let n = |x: f64, y: f64, z: f64| arr(f.vector_to_world(Vector3::new(x, y, z)));
    vec![
        (
            [p(1., -1., -1.), p(1., 1., -1.), p(1., 1., 1.), p(1., -1., 1.)],
            n(1., 0., 0.),
        ),
        (
            [p(-1., -1., 1.), p(-1., 1., 1.), p(-1., 1., -1.), p(-1., -1., -1.)],
            n(-1., 0., 0.),
        ),
        (
            [p(-1., 1., -1.), p(-1., 1., 1.), p(1., 1., 1.), p(1., 1., -1.)],
            n(0., 1., 0.),
        ),
        (
            [p(-1., -1., 1.), p(-1., -1., -1.), p(1., -1., -1.), p(1., -1., 1.)],
            n(0., -1., 0.),
        ),
        (
            [p(-1., -1., 1.), p(1., -1., 1.), p(1., 1., 1.), p(-1., 1., 1.)],
            n(0., 0., 1.),
        ),
        (
            [p(1., -1., -1.), p(-1., -1., -1.), p(-1., 1., -1.), p(1., 1., -1.)],
            n(0., 0., -1.),
        ),
    ]
}

fn sphere(f: CFrame, r: f64) -> Vec<Quad> {
    let (rings, segs) = (6, 12);
    let pt = |i: usize, j: usize| {
        let th = std::f64::consts::PI * i as f64 / rings as f64;
        let ph = std::f64::consts::TAU * j as f64 / segs as f64;
        Vector3::new(th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin())
    };
    let mut out = Vec::new();
    for i in 0..rings {
        for j in 0..segs {
            let q = [pt(i, j), pt(i + 1, j), pt(i + 1, j + 1), pt(i, j + 1)];
            let mid = (q[0] + q[1] + q[2] + q[3]) / 4.0;
            out.push((
                q.map(|v| arr(f.point_to_world(v * r))),
                arr(f.vector_to_world(mid.unit())),
            ));
        }
    }
    out
}

/// Roblox's cylinder lies along its X axis.
fn cylinder(f: CFrame, s: Vector3) -> Vec<Quad> {
    let segs = 12;
    let (hx, r) = (s.x / 2.0, s.y.min(s.z) / 2.0);
    let ring = |j: usize| {
        let a = std::f64::consts::TAU * j as f64 / segs as f64;
        (a.cos() * r, a.sin() * r)
    };
    let p = |x: f64, (y, z): (f64, f64)| arr(f.point_to_world(Vector3::new(x, y, z)));
    let mut out = Vec::new();
    for j in 0..segs {
        let (a, b) = (ring(j), ring(j + 1));
        let mid = ring(j * 2 + 1);
        let side = arr(f.vector_to_world(Vector3::new(0.0, mid.0, mid.1).unit()));
        let _ = mid;
        out.push(([p(-hx, a), p(hx, a), p(hx, b), p(-hx, b)], side));
        out.push((
            [p(hx, (0.0, 0.0)), p(hx, b), p(hx, a), p(hx, (0.0, 0.0))],
            arr(f.vector_to_world(Vector3::X_AXIS)),
        ));
        out.push((
            [p(-hx, (0.0, 0.0)), p(-hx, a), p(-hx, b), p(-hx, (0.0, 0.0))],
            arr(f.vector_to_world(-Vector3::X_AXIS)),
        ));
    }
    out
}

/// The game's camera as a CFrame (Roblox's: -Z forward).
fn camera_cframe(cam: &super::hstudio::Camera) -> CFrame {
    let [fw, up, _] = cam.basis();
    // `+ 0.0` turns -0 into 0, which prints as Roblox's would.
    let f = Vector3::new(fw[0] + 0.0, fw[1] + 0.0, fw[2] + 0.0);
    let u = Vector3::new(up[0] + 0.0, up[1] + 0.0, up[2] + 0.0);
    let back = -f;
    let right = u.cross(back);
    CFrame::from_matrix(Vector3::new(cam.pos[0], cam.pos[1], cam.pos[2]), right, u, Some(back))
}

/// Where a world point is on the screen (dp), its depth, and whether it is in view.
pub fn to_screen(env: &mut dyn Env, p: Vector3) -> (Vector3, bool) {
    let game = env.game();
    let (w, h, _) = game.dp();
    let Some(cam) = game.camera() else {
        return (Vector3::ZERO, false);
    };
    match cam.project([p.x, p.y, p.z], w, h) {
        Some((x, y, depth)) => {
            let on = x >= 0.0 && y >= 0.0 && x <= w && y <= h;
            (Vector3::new(f64::from(x), f64::from(y), depth), on)
        }
        None => (Vector3::new(0.0, 0.0, -cam.depth([p.x, p.y, p.z]).abs()), false),
    }
}

/// A ray from the camera through a screen point (dp): origin and unit direction.
pub fn screen_ray(env: &mut dyn Env, x: f64, y: f64) -> (Vector3, Vector3) {
    let game = env.game();
    let (w, h, _) = game.dp();
    let Some(cam) = game.camera() else {
        return (Vector3::ZERO, -Vector3::Z_AXIS);
    };
    ray_through(&cam, x, y, f64::from(w), f64::from(h))
}

fn ray_through(cam: &super::hstudio::Camera, x: f64, y: f64, w: f64, h: f64) -> (Vector3, Vector3) {
    let [f, u, r] = cam.basis();
    let v = |a: [f64; 3]| Vector3::new(a[0], a[1], a[2]);
    let focal = 1.0 / (f64::from(cam.fov) / 2.0).to_radians().tan();
    let aspect = w / h.max(1.0);
    let sx = (x / w.max(1.0)) * 2.0 - 1.0;
    let sy = 1.0 - (y / h.max(1.0)) * 2.0;
    let dir = v(f) + v(r) * (sx * aspect / focal) + v(u) * (sy / focal);
    (v(cam.pos), dir.unit())
}

/// The Roblox material a block looks most like.
pub fn block_material(id: &str) -> &'static str {
    let b = id.trim_start_matches("minecraft:");
    match b {
        _ if b.contains("water") => "Water",
        _ if b.contains("grass") || b.contains("moss") => "Grass",
        _ if b.contains("leaves") => "LeafyGrass",
        _ if b.contains("planks") => "WoodPlanks",
        _ if b.contains("log") || b.contains("wood") => "Wood",
        _ if b.contains("sandstone") => "Sandstone",
        _ if b.contains("sand") => "Sand",
        _ if b.contains("dirt") || b.contains("mud") || b.contains("podzol") => "Ground",
        _ if b.contains("cobble") => "Cobblestone",
        _ if b.contains("brick") => "Brick",
        _ if b.contains("glass") => "Glass",
        _ if b.contains("ice") => "Ice",
        _ if b.contains("snow") => "Snow",
        _ if b.contains("basalt") => "Basalt",
        _ if b.contains("granite") => "Granite",
        _ if b.contains("marble") || b.contains("quartz") => "Marble",
        _ if b.contains("concrete") => "Concrete",
        _ if b.contains("iron") || b.contains("gold") || b.contains("copper") => "Metal",
        _ if b.contains("wool") || b.contains("carpet") => "Fabric",
        _ if b.contains("stone") || b.contains("deepslate") || b.contains("ore") => "Slate",
        _ => "Plastic",
    }
}

pub fn passable(id: &str) -> bool {
    let b = id.trim_start_matches("minecraft:");
    b == "air"
        || b == "cave_air"
        || b == "void_air"
        || b.ends_with("grass") && b != "grass_block"
        || b == "short_grass"
        || b == "tall_grass"
}

/// The first solid block a ray meets within `limit`: how far, the face's
/// normal and the block's id (Amanatides and Woo's walk, a block at a time).
pub fn first_block(
    game: &mut dyn GameApi,
    origin: Vector3,
    unit: Vector3,
    limit: f64,
    ignore_water: bool,
) -> Option<(f64, Vector3, String)> {
    let mut cell = [
        origin.x.floor() as i64,
        origin.y.floor() as i64,
        origin.z.floor() as i64,
    ];
    let d = [unit.x, unit.y, unit.z];
    let o = [origin.x, origin.y, origin.z];
    let step: [i64; 3] = d.map(|c| if c > 0.0 { 1 } else { -1 });
    let mut next = [f64::INFINITY; 3];
    let mut delta = [f64::INFINITY; 3];
    for i in 0..3 {
        if d[i].abs() > 1e-12 {
            delta[i] = (1.0 / d[i]).abs();
            let edge = if d[i] > 0.0 {
                cell[i] as f64 + 1.0
            } else {
                cell[i] as f64
            };
            next[i] = (edge - o[i]) / d[i];
        }
    }
    let mut t = 0.0;
    let mut normal = Vector3::ZERO;
    for _ in 0..2048 {
        if t > limit {
            return None;
        }
        match game.block(cell[0] as i32, cell[1] as i32, cell[2] as i32) {
            None => return None,
            Some(b) if !passable(&b) && !(ignore_water && b.contains("water")) => return Some((t, normal, b)),
            _ => {}
        }
        let i = if next[0] < next[1] && next[0] < next[2] {
            0
        } else if next[1] < next[2] {
            1
        } else {
            2
        };
        t = next[i];
        next[i] += delta[i];
        cell[i] += step[i];
        let mut n = [0.0; 3];
        n[i] = -(step[i] as f64);
        normal = Vector3::new(n[0], n[1], n[2]);
    }
    None
}

/// The Mouse's Hit, Target, TargetSurface, UnitRay and Origin (the camera,
/// looking along the ray), worked out as a script reads them: where the
/// pointer (or the crosshair) points, at a part or a block
/// (hstudio_rbx_query).
pub fn mouse_refresh(env: &mut dyn Env) {
    let (free, px, py) = {
        let p = &env.rbx().pointer;
        (p.free, p.x, p.y)
    };
    let game = env.game();
    let (w, h, _) = game.dp();
    let Some(cam) = game.camera() else { return };
    let (mx, my) = if free {
        (px, py)
    } else {
        (f64::from(w) / 2.0, f64::from(h) / 2.0)
    };
    let (o, d) = ray_through(&cam, mx, my, f64::from(w), f64::from(h));
    let (hit, target, surface) = super::hstudio_rbx_query::mouse_hit(env, o, d);
    let rbx = env.rbx();
    let m = rbx.mouse;
    rbx.quiet_set(m, "Hit", cf(CFrame::from_position(hit)));
    rbx.quiet_set(m, "Target", target);
    rbx.quiet_set(m, "TargetSurface", enum_value(enum_item("NormalId", surface)));
    rbx.quiet_set(m, "UnitRay", super::hstudio_rbx_values::wrap(Data::Ray(o, d)));
    rbx.quiet_set(m, "Origin", cf(CFrame::look_at(o, o + d, Vector3::Y_AXIS)));
}

// ---------------------------------------------------------------- each frame

/// A GUI put in StarterGui (a ScreenGui, a BillboardGui) is copied into
/// the PlayerGui at the next frame, as a player's spawn copies them in
/// Roblox: once each, what it holds then with it, for the mod that made it.
/// What changes in StarterGui after doesn't show, as in Roblox.
fn copy_starter_gui(rbx: &mut Rbx) {
    let Some(starter) = rbx.service("StarterGui") else {
        return;
    };
    let new: Vec<Id> = rbx
        .children(starter)
        .iter()
        .copied()
        .filter(|&c| is_a(rbx.inst(c).class, "LayerCollector") && !rbx.starter_copied.contains(&c))
        .collect();
    for id in new {
        rbx.starter_copied.insert(id);
        let Some(owner) = rbx.inst(id).owner else {
            continue;
        };
        let copy = rbx.clone_tree(id, owner, true);
        let gui = rbx.player_gui;
        let _ = rbx.set_parent(copy, Some(gui));
    }
}

/// What the player and the world are this frame, put into the DataModel,
/// and the frame's events: RunService's, tweens, Debris, timeouts.
pub fn step(rbx: &mut Rbx, game: &mut dyn GameApi, now: f64, dt: f64, frame_no: u64) {
    rbx.now = now;
    if !rbx.active {
        return;
    }
    sync(rbx, game);
    copy_starter_gui(rbx);
    step_events(rbx, now, dt, frame_no);
    super::hstudio_sunc::poll_network(rbx);
}

/// The DataModel brought up to the game: the world (hstudio_rbx_world: the
/// player and their character), the Camera and the time of day.
pub fn sync(rbx: &mut Rbx, game: &mut dyn GameApi) {
    let now = rbx.now;
    let ws = rbx.workspace;
    rbx.quiet_set(ws, "DistributedGameTime", Value::Num(now));
    super::hstudio_rbx_world::sync(rbx, game);
    if let Some(cam) = game.camera() {
        let c = rbx.camera;
        rbx.quiet_set(c, "CFrame", cf(camera_cframe(&cam)));
        rbx.quiet_set(c, "FieldOfView", Value::Num(f64::from(cam.fov)));
        let (w, h, _) = game.dp();
        rbx.quiet_set(c, "ViewportSize", v2(Vector2::new(f64::from(w), f64::from(h))));
        // The pointer is the crosshair while the game has it.
        let (mx, my) = (f64::from(w) / 2.0, f64::from(h) / 2.0);
        let m = rbx.mouse;
        if !rbx.pointer.free {
            rbx.pointer.x = mx;
            rbx.pointer.y = my;
            rbx.quiet_set(m, "X", Value::Num(mx));
            rbx.quiet_set(m, "Y", Value::Num(my));
        }
    }
    if let Some((day, _)) = game.clock() {
        let hours = ((day as f64 / 1000.0) + 6.0).rem_euclid(24.0);
        let l = rbx.service("Lighting").unwrap_or(rbx.game);
        rbx.quiet_set(l, "ClockTime", Value::Num(hours));
        let (hh, mm) = (hours.floor() as u32, ((hours.fract()) * 60.0).floor() as u32);
        rbx.quiet_set(l, "TimeOfDay", Value::str(&format!("{hh:02}:{mm:02}:00")));
    }
}

/// The frame's events: RunService's, tweens, Debris and WaitForChild's timeouts.
fn step_events(rbx: &mut Rbx, now: f64, dt: f64, frame_no: u64) {
    // RunService's events, then BindToRenderStep's functions, in priority order.
    let rs = rbx.service("RunService").unwrap_or(rbx.game);
    for ev in [
        "PreRender",
        "RenderStepped",
        "PreAnimation",
        "PreSimulation",
        "PostSimulation",
        "Heartbeat",
    ] {
        rbx.fire(rs, ev, vec![Value::Num(dt)]);
    }
    rbx.fire(rs, "Stepped", vec![Value::Num(now), Value::Num(dt)]);
    let steps: Vec<(super::hstudio::Owner, Value)> = rbx.render_steps.iter().map(|r| (r.2, r.3.clone())).collect();
    for (owner, f) in steps {
        rbx.deferred.push((owner, f, vec![Value::Num(dt)]));
    }
    super::hstudio_rbx_methods::step_tweens(rbx, dt);
    step_pages(rbx, dt);
    // Debris whose time came. What Destroy can't destroy (a service, the
    // character, the world's mirrors and other players) stays, as in Roblox.
    let due: Vec<Id> = rbx.debris.iter().filter(|d| d.1 <= now).map(|d| d.0).collect();
    rbx.debris.retain(|d| d.1 > now);
    for id in due {
        match rbx.try_inst(id) {
            Some(i) if i.locked && i.parent.is_some() && !i.destroyed => {
                let note = format!("Debris can't destroy {}: its Parent property is locked", i.name);
                rbx.warnings.push(note);
            }
            _ => rbx.destroy(id),
        }
    }
    // WaitForChild's timeouts, and its warning after 5 seconds.
    let mut i = 0;
    while i < rbx.waiters.len() {
        let (timed_out, warn) = match &rbx.waiters[i].what {
            Wait::Child {
                until, since, warned, ..
            } => (
                until.is_some_and(|u| u <= now),
                until.is_none() && !warned && now - since >= 5.0,
            ),
            Wait::Signal(..) => (false, false),
        };
        if warn {
            let (parent, name) = match &rbx.waiters[i].what {
                Wait::Child { parent, name, .. } => (*parent, name.clone()),
                Wait::Signal(..) => unreachable!(),
            };
            let full = rbx.full_name(parent);
            rbx.warnings
                .push(format!("Infinite yield possible on '{full}:WaitForChild(\"{name}\")'"));
            if let Wait::Child { warned, .. } = &mut rbx.waiters[i].what {
                *warned = true;
            }
        }
        if timed_out {
            let w = rbx.waiters.remove(i);
            rbx.ready.push((w.owner, w.thread, vec![Value::Nil]));
        } else {
            i += 1;
        }
    }
    // Destroyed instances no script holds any more are let go.
    if frame_no.is_multiple_of(120) {
        rbx.insts.retain(|_, inst| {
            !(inst.destroyed && matches!(&inst.value, Value::Userdata(u) if Rc::strong_count(u) == 1))
        });
    }
}

/// Text typed into the focused TextBox (the screen's text input): at its
/// caret, over its selection.
pub fn type_text(rbx: &mut Rbx, text: &str) {
    let Some(f) = rbx.pointer.focus else { return };
    if !rbx.raw_get(f, "TextEditable").truthy() {
        return;
    }
    let mut e = edit_of(rbx, f);
    if e.text.len() + text.len() <= MAX_TEXT {
        e.insert(text);
        store_edit(rbx, f, &e);
    }
}

// ---------------------------------------------------------------- TextBox editing

/// The longest a TextBox's text gets by typing and pasting, in bytes.
const MAX_TEXT: usize = 200_000;

/// The modifier keys a screen's key(key, down, mods) gets.
fn mods_arg(v: Option<&Value>) -> Mods {
    let held = |k: &str| match v {
        Some(Value::Table(t)) => t.borrow().get_str(k).truthy(),
        _ => false,
    };
    Mods {
        ctrl: held("ctrl"),
        shift: held("shift"),
        alt: held("alt"),
    }
}

/// The byte offset a 1-based CursorPosition or SelectionStart stands for in
/// `text`; None for -1.
fn offset_of(text: &str, v: &Value) -> Option<usize> {
    let n = v.to_num().filter(|n| *n >= 1.0)?;
    let mut at = ((n - 1.0) as usize).min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    Some(at)
}

/// A TextBox's text, caret and selection, from its Text, CursorPosition and
/// SelectionStart (so a script moves them by setting those).
fn edit_of(rbx: &Rbx, id: Id) -> LineEdit {
    let text = rbx.raw_get(id, "Text").as_text().unwrap_or_default();
    let cursor = offset_of(&text, &rbx.raw_get(id, "CursorPosition")).unwrap_or(text.len());
    let anchor = offset_of(&text, &rbx.raw_get(id, "SelectionStart")).filter(|&a| a != cursor);
    LineEdit {
        text,
        cursor,
        anchor,
        multiline: rbx.raw_get(id, "MultiLine").truthy(),
    }
}

/// An edit written back: Text, CursorPosition, and SelectionStart (-1 with
/// nothing selected).
fn store_edit(rbx: &mut Rbx, id: Id, e: &LineEdit) {
    if rbx.raw_get(id, "Text").as_text().as_deref() != Some(e.text.as_str()) {
        rbx.raw_set(id, "Text", Value::str(&e.text));
    }
    rbx.raw_set(id, "CursorPosition", Value::Num(e.cursor as f64 + 1.0));
    let start = match (e.selection(), e.anchor) {
        (Some(_), Some(a)) => a as f64 + 1.0,
        _ => -1.0,
    };
    rbx.raw_set(id, "SelectionStart", Value::Num(start));
}

/// A key pressed in the focused TextBox `id`: the arrows, Home, End,
/// Backspace and Delete as a text field has them (Control by word, Shift
/// selecting), Control+A, C, X and V, Up and Down a line as it is drawn in
/// MultiLine text, and Enter (a new line in MultiLine text, else the end of
/// editing: FocusLost(true)). A TextBox that isn't TextEditable only moves
/// its caret, selects and copies.
fn textbox_key(rbx: &mut Rbx, game: &mut dyn GameApi, id: Id, key: &str, mods: Mods) {
    let editable = rbx.raw_get(id, "TextEditable").truthy();
    let mut e = edit_of(rbx, id);
    let k = match key {
        "left" => Key::Left,
        "right" => Key::Right,
        "up" => Key::Up,
        "down" => Key::Down,
        "home" => Key::Home,
        "end" => Key::End,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "enter" => Key::Enter,
        k => {
            let mut chars = k.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Key::Char(c),
                _ => Key::Other,
            }
        }
    };
    match k {
        Key::Enter if e.multiline && editable => {
            if e.text.len() < MAX_TEXT {
                e.insert("\n");
            }
        }
        Key::Enter => {
            unfocus(rbx, true);
            return;
        }
        Key::Char('c') if mods.ctrl => {
            if !e.selected().is_empty() {
                game.set_clipboard(e.selected());
            }
            return;
        }
        Key::Char('x') if mods.ctrl => {
            if !e.selected().is_empty() {
                game.set_clipboard(e.selected());
                if editable {
                    e.delete_selection();
                }
            }
        }
        Key::Char('v') if mods.ctrl => {
            let clip = game.clipboard();
            if editable && e.text.len() + clip.len() <= MAX_TEXT {
                e.insert(&clip);
            }
        }
        Key::Up | Key::Down if e.multiline => {
            let down = k == Key::Down;
            let to = drawn_line_move(rbx, game, id, e.cursor, down).unwrap_or_else(|| e.line_move(down));
            e.move_cursor(to.min(e.text.len()), mods.shift);
        }
        Key::Backspace | Key::Delete if !editable => return,
        k => {
            if !e.edit_key(k, mods) {
                return;
            }
        }
    }
    // A layout's offsets fall on the text's characters, but the text may
    // have changed under them.
    while !e.text.is_char_boundary(e.cursor) {
        e.cursor -= 1;
    }
    store_edit(rbx, id, &e);
}

/// Lays the GUI out again if it moved since the last frame's layout (the
/// caret's place needs the text as it is now).
fn fresh_frame(rbx: &mut Rbx, game: &mut dyn GameApi) {
    if rbx.layout.as_ref().is_none_or(|f| f.epoch != rbx.gui_epoch) {
        lay_out(rbx, game);
    }
}

/// A text object's text as last laid out, with its UIScale and font.
fn laid_text(rbx: &Rbx, id: Id) -> Option<(layout::TextLayout, f64, StudioFont)> {
    let f = rbx.layout.as_ref()?;
    let p = f.layout.get(*f.inst_node.get(&id)?)?;
    let font = studio_font(&as_font(&rbx.raw_get(id, "FontFace")).unwrap_or_default());
    Some((p.text.clone()?, p.scale, font))
}

/// How wide the start of a laid-out line is, to byte `end` of it.
fn width_to(
    game: &mut dyn GameApi,
    t: &layout::TextLayout,
    line: &layout::TextLine,
    end: usize,
    scale: f64,
    font: StudioFont,
) -> f64 {
    let size = (t.size / scale.max(1e-9)) as f32;
    f64::from(game.dp_measure(&line.text[..end], size, font)) * scale
}

/// The line the caret at byte `cursor` is on, and its x.
fn caret_at(
    game: &mut dyn GameApi,
    t: &layout::TextLayout,
    cursor: usize,
    scale: f64,
    font: StudioFont,
) -> Option<(usize, f64)> {
    let i = t.lines.iter().rposition(|l| l.start <= cursor)?;
    let line = &t.lines[i];
    let mut end = (cursor - line.start).min(line.text.len());
    while !line.text.is_char_boundary(end) {
        end -= 1;
    }
    Some((i, line.position.x + width_to(game, t, line, end, scale, font)))
}

/// The byte of the text nearest `x` on laid-out line `i`.
fn nearest(game: &mut dyn GameApi, t: &layout::TextLayout, i: usize, x: f64, scale: f64, font: StudioFont) -> usize {
    let line = &t.lines[i];
    let ends: Vec<usize> = line
        .text
        .char_indices()
        .map(|(b, _)| b)
        .chain(std::iter::once(line.text.len()))
        .collect();
    let mut at = |end: usize| line.position.x + width_to(game, t, line, end, scale, font);
    // The first end at or past x; the one before it may be nearer.
    let (mut lo, mut hi) = (0, ends.len() - 1);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if at(ends[mid]) < x {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    let best = if lo > 0 && x - at(ends[lo - 1]) < at(ends[lo]) - x {
        lo - 1
    } else {
        lo
    };
    line.start + ends[best]
}

/// Puts the focused TextBox `id`'s caret where the pointer is; `extend`
/// keeps the other end of the selection where it was.
fn caret_to_pointer(rbx: &mut Rbx, game: &mut dyn GameApi, id: Id, extend: bool) {
    fresh_frame(rbx, game);
    let Some((t, scale, font)) = laid_text(rbx, id) else {
        return;
    };
    if t.lines.is_empty() {
        return;
    }
    // The pointer in the box's upright space, if it is turned.
    let at = Vector2::new(rbx.pointer.x, rbx.pointer.y);
    let at = rbx
        .layout
        .as_ref()
        .and_then(|f| f.layout.get(*f.inst_node.get(&id)?))
        .map_or(at, |p| p.unturned(at));
    let (x, y) = (at.x, at.y);
    let i = t.lines.iter().rposition(|l| l.position.y <= y).unwrap_or(0);
    let to = nearest(game, &t, i, x, scale, font);
    let mut e = edit_of(rbx, id);
    let mut to = to.min(e.text.len());
    while !e.text.is_char_boundary(to) {
        to -= 1;
    }
    e.move_cursor(to, extend);
    store_edit(rbx, id, &e);
}

/// The caret a drawn line up or down from byte `cursor`, as far across as
/// it is: the text's start above the first line, its end below the last.
/// None without a layout of the box.
fn drawn_line_move(rbx: &mut Rbx, game: &mut dyn GameApi, id: Id, cursor: usize, down: bool) -> Option<usize> {
    fresh_frame(rbx, game);
    let (t, scale, font) = laid_text(rbx, id)?;
    let (i, x) = caret_at(game, &t, cursor, scale, font)?;
    let j = if down { i + 1 } else { i.checked_sub(1)? };
    if j >= t.lines.len() {
        return None;
    }
    Some(nearest(game, &t, j, x, scale, font))
}
