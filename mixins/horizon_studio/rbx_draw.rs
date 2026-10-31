// The Roblox layer on the game (hstudio_rbx_draw): ScreenGuis laid out by
// hstudio_rbx_layout and drawn with the design system's shapes and fonts,
// the mouse and keys as Roblox input (hover, clicks, TextBoxes, scrolling,
// UserInputService, ContextActionService, the Mouse), Parts in the
// Workspace drawn in the world, rays through the world's blocks, the
// Camera's projection, and each frame's step (the character and the Camera
// following the player, RunService's events, tweens, Debris).
//
// Roblox's GUI pixels are dp here, Horizon's logical pixels, and there is
// no top bar (GuiService:GetGuiInset() is zero). A stud is a block.

use super::hstudio::{Font as StudioFont, GameApi};
use super::hstudio_lua::{Lua, Res, Table, Throw, Value, check_num, check_text};
use super::hstudio_rbx::{Env, Id, Rbx, Wait, is_a};
use super::hstudio_rbx_layout::{
    self as layout, AspectType, AutomaticSize, Class, DominantAxis, FillDirection, HorizontalAlignment, Layout, NodeId,
    ScrollBarInset, ScrollingDirection, SizeConstraint, SortOrder, StartCorner, TextXAlignment, TextYAlignment, Tree,
    VerticalAlignment, VerticalScrollBarPosition, Viewport, ZIndexBehavior,
};
use super::hstudio_rbx_types::{CFrame, Color3, Font, FontWeight, Rect, UDim2, Vector2, Vector3};
use super::hstudio_rbx_values::{
    Data, EnumItem, as_c3, as_cf, as_font, as_item, as_udim, as_udim2, as_v2, as_v3, cf, enum_item, enum_value, get,
    v2, v3,
};
use std::collections::HashMap;
use std::rc::Rc;

/// A laid-out frame of the GUI: the layout, and which instance each node is.
pub struct Frame {
    pub layout: Layout,
    pub node_inst: Vec<Id>,
    pub inst_node: HashMap<Id, NodeId>,
    pub view: (f64, f64),
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

/// Text without rich text's tags (`<b>`, `<font color="...">`).
pub fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = false;
    for c in s.chars() {
        match c {
            '<' => depth = true,
            '>' if depth => depth = false,
            _ if !depth => out.push(c),
            _ => {}
        }
    }
    out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
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
        if flag(inst, "RichText") {
            text = strip_tags(&text);
        }
        if c.name == "TextBox" && text.is_empty() && !focused {
            text = inst.get("PlaceholderText").as_text().unwrap_or_default();
        }
        node.text = text;
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
        "ScreenGui" => {
            node.enabled = flag(inst, "Enabled");
            node.ignore_gui_inset = flag(inst, "IgnoreGuiInset");
            node.display_order = num(inst, "DisplayOrder") as i32;
            node.z_index_behavior = match enum_name(&inst.get("ZIndexBehavior")) {
                "Global" => ZIndexBehavior::Global,
                _ => ZIndexBehavior::Sibling,
            };
        }
        "UIListLayout" | "UIGridLayout" => {
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
        "CanvasGroup" => Class::Frame,
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

/// Lays out every ScreenGui for this frame, and writes back what Roblox
/// reports (AbsolutePosition, AbsoluteSize, TextBounds...).
pub fn lay_out(rbx: &mut Rbx, game: &mut dyn GameApi) {
    if !rbx.active {
        return;
    }
    let (w, h, _) = game.dp();
    let mut tree = Tree::new();
    let mut node_inst: Vec<Id> = Vec::new();
    let mut inst_node: HashMap<Id, NodeId> = HashMap::new();
    let focus = rbx.pointer.focus;
    for sg in screen_guis(rbx) {
        let mut stack: Vec<(Id, Option<NodeId>)> = vec![(sg, None)];
        while let Some((id, parent)) = stack.pop() {
            let inst = rbx.inst(id);
            let class = layout_class(inst.class);
            let n = tree.add(class, parent);
            fill(&mut tree[n], inst, focus == Some(id));
            if node_inst.len() <= n {
                node_inst.resize(n + 1, 0);
            }
            node_inst[n] = id;
            inst_node.insert(id, n);
            for &c in inst.children.iter().rev() {
                stack.push((c, Some(n)));
            }
        }
    }
    let mut measure = |text: &str, size: f64, font: &Font| -> f64 {
        f64::from(game.dp_measure(text, size as f32, studio_font(font)))
    };
    let lay = tree.layout(&Viewport::new(f64::from(w), f64::from(h), 0.0), &mut measure);
    let mut writes: Vec<(Id, &'static str, Value, bool)> = Vec::new();
    for (n, &id) in node_inst.iter().enumerate() {
        let Some(p) = lay.get(n) else { continue };
        let inst = rbx.inst(id);
        let c = inst.class;
        if is_a(c, "GuiBase2d") {
            writes.push((id, "AbsolutePosition", v2(p.position), true));
            writes.push((id, "AbsoluteSize", v2(p.size), true));
            writes.push((id, "AbsoluteRotation", Value::Num(p.rotation), false));
        }
        if let Some(t) = &p.text {
            writes.push((id, "TextBounds", v2(t.bounds), true));
            writes.push((id, "TextFits", Value::Bool(t.fits), false));
        }
        if c.name == "ScrollingFrame" {
            writes.push((id, "AbsoluteCanvasSize", v2(p.canvas_size), true));
            writes.push((id, "AbsoluteWindowSize", v2(p.window_size), true));
        }
        if matches!(c.name, "UIListLayout" | "UIGridLayout") {
            writes.push((id, "AbsoluteContentSize", v2(p.content_size), true));
            if c.name == "UIGridLayout" {
                writes.push((id, "AbsoluteCellSize", v2(p.cell_size), false));
                writes.push((id, "AbsoluteCellCount", v2(p.cell_count), false));
            }
        }
    }
    for (id, k, v, loud) in writes {
        if loud {
            rbx.raw_set(id, k, v);
        } else {
            rbx.quiet_set(id, k, v);
        }
    }
    rbx.layout = Some(Frame {
        layout: lay,
        node_inst,
        inst_node,
        view: (f64::from(w), f64::from(h)),
    });
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
    for &n in &frame.layout.draw_order {
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
        let radius = p.corner_radius as f32;
        let hover = rbx.pointer.hover.first() == Some(&id);
        let pressed = rbx.pointer.pressed.is_some_and(|(b, _)| b == id);
        let auto = find(inst.class, "AutoButtonColor") && inst.get("AutoButtonColor").truthy();
        // The background, tinted by a gradient's middle and a button's state.
        let mut bg = color(inst, "BackgroundColor3");
        let mut bg_alpha = (1.0 - inst.get("BackgroundTransparency").to_num().unwrap_or(0.0)) * ga;
        if let Some(g) = component(rbx, id, "UIGradient") {
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
        let r = rect4(vis);
        if bg_alpha > 0.0 {
            game.dp_rect(r, radius, argb(bg, bg_alpha));
        }
        // Images aren't loaded here: an ImageLabel shows its tint faintly.
        if find(inst.class, "Image") {
            let img = inst.get("Image").as_text().unwrap_or_default();
            let ia = (1.0 - inst.get("ImageTransparency").to_num().unwrap_or(0.0)) * ga;
            // getcustomasset's images draw; Roblox's asset ids don't load here.
            let file = img
                .strip_prefix("rbxasset://horizon-studio/")
                .and_then(|p| images.file(workspace, p));
            match file {
                Some(bytes) if ia > 0.0 => images.draw(game, &bytes, r, ia),
                _ if !img.is_empty() && ia > 0.0 => {
                    game.dp_rect(r, radius, argb(color(inst, "ImageColor3"), ia * 0.18))
                }
                _ => {}
            }
        }
        // The border, and a UIStroke on the edge.
        let border = inst.get("BorderSizePixel").to_num().unwrap_or(0.0);
        if border >= 1.0 && bg_alpha > 0.0 && radius == 0.0 {
            let c = argb(color(inst, "BorderColor3"), bg_alpha);
            let b = border.min(8.0) as f32;
            let [x, y, w, h] = r;
            game.dp_rect([x - b, y - b, w + 2.0 * b, b], 0.0, c);
            game.dp_rect([x - b, y + h, w + 2.0 * b, b], 0.0, c);
            game.dp_rect([x - b, y, b, h], 0.0, c);
            game.dp_rect([x + w, y, b, h], 0.0, c);
        }
        let stroke = component(rbx, id, "UIStroke").map(|s| {
            let si = rbx.inst(s);
            let alpha = (1.0 - si.get("Transparency").to_num().unwrap_or(0.0)) * ga;
            let thick = si.get("Thickness").to_num().unwrap_or(1.0);
            let border = enum_name(&si.get("ApplyStrokeMode")) == "Border";
            (argb(color(si, "Color"), alpha), thick, border)
        });
        let text_object = p.text.is_some();
        if let Some((c, thick, border_mode)) = stroke
            && (border_mode || !text_object)
            && thick > 0.0
        {
            let t = thick.min(16.0) as f32;
            for i in 0..t.ceil() as i32 {
                let o = i as f32 + 0.5;
                let [x, y, w, h] = r;
                game.dp_border([x - o, y - o, w + 2.0 * o, h + 2.0 * o], radius + o, c);
            }
        }
        // Text.
        if let Some(t) = &p.text {
            let placeholder = inst.class.name == "TextBox"
                && inst.get("Text").as_text().is_none_or(|s| s.is_empty())
                && rbx.pointer.focus != Some(id);
            let tc = if placeholder {
                color(inst, "PlaceholderColor3")
            } else {
                color(inst, "TextColor3")
            };
            let ta = (1.0 - inst.get("TextTransparency").to_num().unwrap_or(0.0)) * ga;
            let font = studio_font(&as_font(&inst.get("FontFace")).unwrap_or_default());
            let size = t.size as f32;
            let lh = inst.get("LineHeight").to_num().unwrap_or(1.0) as f32;
            // Geist's line box is 1.45 times its size; Roblox's is the size.
            let lift = (size * 1.45 - size * lh) / 2.0;
            let outline = if text_object {
                match stroke {
                    Some((c, thick, false)) if thick > 0.0 => Some((c, thick as f32)),
                    _ => {
                        let st = inst.get("TextStrokeTransparency").to_num().unwrap_or(1.0);
                        (st < 1.0).then(|| (argb(color(inst, "TextStrokeColor3"), (1.0 - st) * ta), 1.0))
                    }
                }
            } else {
                None
            };
            let clip = p.clip;
            for line in &t.lines {
                let at = [line.position.x as f32, line.position.y as f32 - lift];
                if let Some(c) = clip
                    && (line.position.y + t.size < c.min.y || line.position.y > c.max.y)
                {
                    continue;
                }
                if let Some((c, o)) = outline {
                    for (dx, dy) in [(-o, 0.0), (o, 0.0), (0.0, -o), (0.0, o)] {
                        game.dp_text(&line.text, [at[0] + dx, at[1] + dy], size, font, c);
                    }
                }
                if ta > 0.0 {
                    game.dp_text(&line.text, at, size, font, argb(tc, ta));
                }
            }
            // The caret, blinking.
            if rbx.pointer.focus == Some(id) && (now * 2.0).fract() < 0.6 {
                let (cx, cy) = match t.lines.last() {
                    Some(l) if !placeholder => (l.position.x + l.width, l.position.y),
                    _ => (
                        p.content.min.x,
                        p.content.min.y + (p.content.height() - t.size).max(0.0) / 2.0,
                    ),
                };
                game.dp_rect([cx as f32 + 1.0, cy as f32, 1.5, size], 0.0, argb(tc, ta));
            }
        }
        // A ScrollingFrame's bar.
        if inst.class.name == "ScrollingFrame" && p.canvas_size.y > p.window_size.y + 0.5 {
            let thick = inst.get("ScrollBarThickness").to_num().unwrap_or(12.0);
            if thick > 0.0 {
                let ratio = (p.window_size.y / p.canvas_size.y).clamp(0.05, 1.0);
                let travel = p.size.y * (1.0 - ratio);
                let off = if p.canvas_size.y > p.window_size.y {
                    p.canvas_position.y / (p.canvas_size.y - p.window_size.y)
                } else {
                    0.0
                };
                let bx = p.position.x + p.size.x - thick;
                let by = p.position.y + travel * off.clamp(0.0, 1.0);
                let ca = (1.0 - inst.get("ScrollBarImageTransparency").to_num().unwrap_or(0.0)) * ga;
                let c = argb(color(inst, "ScrollBarImageColor3"), ca * 0.7);
                game.dp_rect(
                    [bx as f32, by as f32, thick as f32, (p.size.y * ratio) as f32],
                    (thick / 2.0) as f32,
                    c,
                );
            }
        }
    }
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
    if rbx.raw_get(id, "ClearTextOnFocus").truthy() && rbx.raw_get(id, "TextEditable").truthy() {
        rbx.raw_set(id, "Text", Value::str(""));
    }
    rbx.fire(id, "Focused", Vec::new());
    let uis = rbx.service("UserInputService").unwrap_or(rbx.game);
    let v = rbx.value(id);
    rbx.fire(uis, "TextBoxFocused", vec![v]);
}

pub fn unfocus(rbx: &mut Rbx, enter: bool) {
    let Some(id) = rbx.pointer.focus.take() else { return };
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
            pointer_moved(env.rbx(), x, y);
            Ok(Vec::new())
        }
        // click(x, y, button)
        2 => {
            let (x, y) = xy(lua, args)?;
            let left = args.get(2).and_then(Value::as_text).is_none_or(|b| b == "left");
            let rbx = env.rbx();
            pointer_moved(rbx, x, y);
            pointer_button(rbx, left, true);
            Ok(Vec::new())
        }
        // release(x, y, button)
        3 => {
            let (x, y) = xy(lua, args)?;
            let left = args.get(2).and_then(Value::as_text).is_none_or(|b| b == "left");
            let rbx = env.rbx();
            pointer_moved(rbx, x, y);
            pointer_button(rbx, left, false);
            Ok(Vec::new())
        }
        // key(key, down)
        4 => {
            let key = check_text(lua, args, 0, "key")?;
            let down = args.get(1).is_some_and(Value::truthy);
            let rbx = env.rbx();
            if let Some(f) = rbx.pointer.focus {
                if down {
                    match key.as_str() {
                        "enter" => unfocus(rbx, true),
                        "backspace" if rbx.raw_get(f, "TextEditable").truthy() => {
                            let mut t = rbx.raw_get(f, "Text").as_text().unwrap_or_default();
                            t.pop();
                            rbx.raw_set(f, "Text", Value::str(&t));
                        }
                        _ => {}
                    }
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
            let rbx = env.rbx();
            if let Some(f) = rbx.pointer.focus
                && rbx.raw_get(f, "TextEditable").truthy()
            {
                let mut s = rbx.raw_get(f, "Text").as_text().unwrap_or_default();
                if s.len() + t.len() <= 200_000 {
                    s.push_str(&t);
                    rbx.raw_set(f, "Text", Value::str(&s));
                }
            }
            Ok(Vec::new())
        }
        // wheel(steps)
        6 => {
            let steps = check_num(lua, args, 0, "wheel")?;
            wheel(env.rbx(), steps);
            Ok(Vec::new())
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
    let (x, y) = (rbx.pointer.x, rbx.pointer.y);
    for id in std::mem::take(&mut rbx.pointer.hover) {
        rbx.fire(id, "MouseLeave", vec![Value::Num(x), Value::Num(y)]);
    }
    rbx.pointer.pressed = None;
    rbx.pointer.drag = None;
}

pub fn pointer_moved(rbx: &mut Rbx, x: f64, y: f64) {
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

pub fn pointer_button(rbx: &mut Rbx, left: bool, down: bool) {
    let kind = if left { "MouseButton1" } else { "MouseButton2" };
    rbx.pointer.buttons[usize::from(!left)] = down;
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
    let max = Vector2::new(
        (p.canvas_size.x - p.window_size.x).max(0.0),
        (p.canvas_size.y - p.window_size.y).max(0.0),
    );
    let pos = as_v2(&rbx.raw_get(sf, "CanvasPosition")).unwrap_or_default();
    let vertical = max.y > 0.0;
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
fn block_material(id: &str) -> &'static str {
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

fn passable(id: &str) -> bool {
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
fn first_block(
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

/// The Mouse's Hit, Target and UnitRay, worked out as a script reads them:
/// where the pointer (or the crosshair) points, through the blocks.
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
    let hit = first_block(game, o, d, 500.0, true).map_or(o + d * 500.0, |(t, _, _)| o + d * t);
    let rbx = env.rbx();
    let m = rbx.mouse;
    let terrain = rbx.raw_get(rbx.workspace, "Terrain");
    let target = if (hit - o).magnitude() < 499.0 {
        terrain
    } else {
        Value::Nil
    };
    rbx.quiet_set(m, "Hit", cf(CFrame::from_position(hit)));
    rbx.quiet_set(m, "Target", target);
    rbx.quiet_set(m, "UnitRay", super::hstudio_rbx_values::wrap(Data::Ray(o, d)));
}

/// workspace:Raycast: through the world's blocks and the Workspace's parts;
/// a RaycastResult table, or nil.
pub fn raycast(
    env: &mut dyn Env,
    origin: Vector3,
    dir: Vector3,
    params: Option<&super::hstudio_lua::TableRef>,
) -> Value {
    let length = dir.magnitude().min(5000.0);
    if length <= 0.0 {
        return Value::Nil;
    }
    let unit = dir.unit();
    let ignore_water = params.is_some_and(|p| p.borrow().get_str("IgnoreWater").truthy());
    // The parts first: their nearest hit.
    let (filter, include) = match params {
        Some(p) => {
            let p = p.borrow();
            let include = as_item(&p.get_str("FilterType")).is_some_and(|i| i.value == 1);
            let list = match p.get_str("FilterDescendantsInstances") {
                Value::Table(t) => t.borrow().list(),
                _ => Vec::new(),
            };
            (list, include)
        }
        None => (Vec::new(), false),
    };
    let rbx = env.rbx();
    let filter: Vec<Id> = filter.iter().filter_map(|v| rbx.id_of(v)).collect();
    let in_filter = |rbx: &Rbx, id: Id| filter.iter().any(|&f| f == id || rbx.is_descendant_of(id, f));
    let mut best: Option<(f64, Vector3, Id)> = None;
    for id in rbx.descendants(rbx.workspace) {
        let inst = rbx.inst(id);
        if !is_a(inst.class, "BasePart") || inst.class.name == "Terrain" || !inst.get("CanQuery").truthy() {
            continue;
        }
        // The character's own parts only when a filter asks for them.
        if (id == rbx.root_part || id == rbx.head) && (!include || !in_filter(rbx, id)) {
            continue;
        }
        let hit = in_filter(rbx, id);
        if include != hit {
            continue;
        }
        let frame = as_cf(&inst.get("CFrame")).unwrap_or_default();
        let half = as_v3(&inst.get("Size")).unwrap_or(Vector3::ONE) / 2.0;
        // The ray in the part's space, against its box.
        let o = frame.point_to_object(origin);
        let d = frame.vector_to_object(unit);
        let mut t0 = 0.0f64;
        let mut t1 = length;
        let mut normal = Vector3::ZERO;
        let mut ok = true;
        for (axis, (oc, dc, hc)) in [(o.x, d.x, half.x), (o.y, d.y, half.y), (o.z, d.z, half.z)]
            .into_iter()
            .enumerate()
        {
            if dc.abs() < 1e-12 {
                if oc.abs() > hc {
                    ok = false;
                    break;
                }
                continue;
            }
            let (mut a, mut b) = ((-hc - oc) / dc, (hc - oc) / dc);
            let mut sign = -1.0;
            if a > b {
                std::mem::swap(&mut a, &mut b);
                sign = 1.0;
            }
            if a > t0 {
                t0 = a;
                let mut n = [0.0; 3];
                n[axis] = sign;
                normal = Vector3::new(n[0], n[1], n[2]);
            }
            t1 = t1.min(b);
            if t0 > t1 {
                ok = false;
                break;
            }
        }
        if ok && best.is_none_or(|b| t0 < b.0) {
            best = Some((t0, frame.vector_to_world(normal), id));
        }
    }
    // Then the blocks, a step at a time (Amanatides and Woo).
    let limit = best.map_or(length, |b| b.0);
    let terrain = rbx.raw_get(rbx.workspace, "Terrain");
    let block_hit = first_block(env.game(), origin, unit, limit, ignore_water);
    let mut t = Table::new();
    match (block_hit, best) {
        (Some((bt, n, b)), _) if bt <= limit => {
            t.set_str("Position", v3(origin + unit * bt));
            t.set_str("Normal", v3(n));
            t.set_str("Distance", Value::Num(bt));
            t.set_str("Instance", terrain);
            t.set_str("Material", enum_value(enum_item("Material", block_material(&b))));
            t.set_str("Block", Value::str(&b));
        }
        (_, Some((pt, n, id))) => {
            let rbx = env.rbx();
            t.set_str("Position", v3(origin + unit * pt));
            t.set_str("Normal", v3(n));
            t.set_str("Distance", Value::Num(pt));
            t.set_str("Instance", rbx.value(id));
            let mat = rbx.raw_get(id, "Material");
            t.set_str("Material", mat);
        }
        _ => return Value::Nil,
    }
    Value::table(t)
}

// ---------------------------------------------------------------- each frame

/// What the player and the world are this frame, put into the DataModel,
/// and the frame's events: RunService's, tweens, Debris, timeouts.
pub fn step(rbx: &mut Rbx, game: &mut dyn GameApi, now: f64, dt: f64, frame_no: u64) {
    rbx.now = now;
    if !rbx.active {
        return;
    }
    sync(rbx, game);
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
    // Debris whose time came.
    let due: Vec<Id> = rbx.debris.iter().filter(|d| d.1 <= now).map(|d| d.0).collect();
    rbx.debris.retain(|d| d.1 > now);
    for id in due {
        rbx.destroy(id);
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

/// The screen's text input and Changed for a TextBox as one types (tests).
pub fn type_text(rbx: &mut Rbx, text: &str) {
    if let Some(f) = rbx.pointer.focus {
        let mut s = rbx.raw_get(f, "Text").as_text().unwrap_or_default();
        s.push_str(text);
        rbx.raw_set(f, "Text", Value::str(&s));
    }
}
