// hstudio_rbx_layout on concrete GUIs, with the numbers worked out by hand
// from the spec's rules: an 800x600 screen with the 58-pixel top bar (so a
// ScreenGui is 800x542), and text measured as a monospace font half as wide
// as it is tall. Anchor points, nested scale and offset, SizeConstraint,
// padding, aspect and size constraints, list and grid layouts, AutomaticSize
// with text, TextScaled, scrolling with clipping, UIScale, draw order and
// hit tests.

use horizon_studio::game::hstudio_rbx_layout::{
    AspectType, AutomaticSize, Class, DominantAxis, FillDirection, HorizontalAlignment, Layout, Node, NodeId,
    ScrollBarInset, SizeConstraint, SortOrder, StartCorner, TOP_BAR, TextTruncate, TextXAlignment, TextYAlignment,
    Tree, VerticalAlignment, Viewport, ZIndexBehavior,
};
use horizon_studio::game::hstudio_rbx_layout::{
    FlexAlignment, FlexMode, ItemLineAlignment, RichStyle, RichText, parse_rich,
};
use horizon_studio::game::hstudio_rbx_types::{Color3, Font, FontWeight, UDim, UDim2, Vector2};

/// A monospace font: each character is half the text size wide.
fn mono(text: &str, size: f64, _font: &Font) -> f64 {
    text.chars().count() as f64 * size / 2.0
}

fn run(tree: &Tree) -> Layout {
    tree.layout(&Viewport::new(800.0, 600.0, TOP_BAR), &mut mono)
}

fn screen(tree: &mut Tree) -> NodeId {
    tree.add(Class::ScreenGui, None)
}

fn add(tree: &mut Tree, class: Class, parent: NodeId, position: UDim2, size: UDim2) -> NodeId {
    let id = tree.add(class, Some(parent));
    tree[id].position = position;
    tree[id].size = size;
    id
}

fn frame(tree: &mut Tree, parent: NodeId, position: UDim2, size: UDim2) -> NodeId {
    add(tree, Class::Frame, parent, position, size)
}

fn px(x: f64, y: f64) -> UDim2 {
    UDim2::from_offset(x, y)
}

fn v(x: f64, y: f64) -> Vector2 {
    Vector2::new(x, y)
}

/// A node's AbsolutePosition and AbsoluteSize as (x, y, w, h).
#[track_caller]
fn rect(layout: &Layout, id: NodeId) -> (f64, f64, f64, f64) {
    let p = layout.get(id).expect("laid out");
    (p.position.x, p.position.y, p.size.x, p.size.y)
}

#[track_caller]
fn assert_rect(layout: &Layout, id: NodeId, want: (f64, f64, f64, f64)) {
    let got = rect(layout, id);
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
    assert!(
        close(got.0, want.0) && close(got.1, want.1) && close(got.2, want.2) && close(got.3, want.3),
        "node {id}: {got:?}, want {want:?}"
    );
}

// ---------------------------------------------------------------- the tree

#[test]
fn nodes_start_with_instance_new_defaults() {
    let label = Node::new(Class::TextLabel);
    assert_eq!(
        (label.name.as_str(), label.text.as_str(), label.text_size),
        ("TextLabel", "Label", 8.0)
    );
    assert!(!label.active && label.visible && label.interactable);
    assert_eq!(label.z_index, 1);
    assert_eq!(label.font, Font::default());
    let button = Node::new(Class::TextButton);
    assert!(button.active);
    assert_eq!(button.text, "Button");
    assert!(Node::new(Class::TextBox).active);
    assert!(Node::new(Class::ImageButton).active);
    assert!(!Node::new(Class::Frame).clips_descendants);
    let scroll = Node::new(Class::ScrollingFrame);
    assert!(scroll.clips_descendants);
    assert_eq!(scroll.canvas_size, UDim2::new(0.0, 0.0, 2.0, 0.0));
    assert_eq!(scroll.scroll_bar_thickness, 12.0);
    assert_eq!(Node::new(Class::UIListLayout).fill_direction, FillDirection::Vertical);
    assert_eq!(Node::new(Class::UIListLayout).sort_order, SortOrder::Name);
    let grid = Node::new(Class::UIGridLayout);
    assert_eq!(grid.fill_direction, FillDirection::Horizontal);
    assert_eq!((grid.cell_size, grid.cell_padding), (px(100.0, 100.0), px(5.0, 5.0)));
    assert_eq!(Node::new(Class::UICorner).corner_radius, UDim::new(0.0, 8.0));
    assert_eq!(Node::new(Class::ScreenGui).z_index_behavior, ZIndexBehavior::Global);
    assert_eq!(
        Node::new(Class::UISizeConstraint).max_size,
        v(f64::INFINITY, f64::INFINITY)
    );
    assert_eq!(
        Class::from_name("UIAspectRatioConstraint"),
        Some(Class::UIAspectRatioConstraint)
    );
    assert_eq!(Class::from_name("Part"), None);
    for class in Class::ALL {
        assert_eq!(Class::from_name(class.name()), Some(class));
    }
}

#[test]
fn tree_parents_children_and_removal() {
    let mut tree = Tree::new();
    assert!(tree.is_empty());
    let gui = screen(&mut tree);
    let a = tree.add(Class::Frame, Some(gui));
    let b = tree.add(Class::Frame, Some(gui));
    let c = tree.add(Class::Frame, Some(a));
    assert_eq!(tree.len(), 4);
    assert_eq!(tree.children(gui), &[a, b]);
    assert_eq!(tree.parent(c), Some(a));
    assert_eq!(tree.roots(), &[gui]);
    // Reparenting appends; a node can't go inside itself.
    tree.set_parent(a, Some(b)).unwrap();
    assert_eq!(tree.children(gui), &[b]);
    assert_eq!(tree.children(b), &[a]);
    assert!(tree.set_parent(b, Some(c)).is_err());
    assert!(tree.set_parent(b, Some(b)).is_err());
    assert_eq!(tree.parent(b), Some(gui));
    assert!(tree.is_descendant_of(c, gui));
    tree.set_parent(c, None).unwrap();
    assert_eq!(tree.roots(), &[gui, c]);
    // Removing takes the subtree; ids come back for new nodes.
    tree.set_parent(c, Some(a)).unwrap();
    tree.remove(b);
    assert_eq!(tree.len(), 1);
    assert!(tree.get(a).is_none() && tree.get(c).is_none());
    assert!(tree.children(gui).is_empty());
    let d = tree.add(Class::TextLabel, Some(gui));
    assert!([a, b, c].contains(&d));
    assert_eq!(tree[d].class, Class::TextLabel);
    assert!(tree.set_parent(a, Some(gui)).is_err());
}

#[test]
fn only_screen_guis_are_laid_out() {
    let mut tree = Tree::new();
    let loose = tree.add(Class::Frame, None);
    let inner = frame(&mut tree, loose, px(0.0, 0.0), px(10.0, 10.0));
    let gui = screen(&mut tree);
    let f = frame(&mut tree, gui, px(0.0, 0.0), px(10.0, 10.0));
    let l = run(&tree);
    assert!(l.get(loose).is_none() && l.get(inner).is_none());
    assert_rect(&l, gui, (0.0, 0.0, 800.0, 542.0));
    assert_rect(&l, f, (0.0, 0.0, 10.0, 10.0));
    assert_eq!(l.draw_order, vec![f]);
}

// ---------------------------------------------------------------- size and position

#[test]
fn centered_anchor_point() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let f = frame(&mut tree, gui, UDim2::from_scale(0.5, 0.5), px(200.0, 100.0));
    tree[f].anchor_point = v(0.5, 0.5);
    let corner = frame(&mut tree, gui, UDim2::from_scale(1.0, 1.0), px(40.0, 30.0));
    tree[corner].anchor_point = v(1.0, 1.0);
    let l = run(&tree);
    // The middle of the 800x542 area is (400, 271).
    assert_rect(&l, f, (300.0, 221.0, 200.0, 100.0));
    assert_rect(&l, corner, (760.0, 512.0, 40.0, 30.0));
    // Over the top bar, the area runs from -58 to 542, so the middle is 242.
    tree[gui].ignore_gui_inset = true;
    let l = run(&tree);
    assert_rect(&l, gui, (0.0, -58.0, 800.0, 600.0));
    assert_rect(&l, f, (300.0, 192.0, 200.0, 100.0));
}

#[test]
fn nested_scale_and_offset() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let outer = frame(&mut tree, gui, px(50.0, 20.0), UDim2::new(0.5, 0.0, 0.5, 100.0));
    let inner = frame(
        &mut tree,
        outer,
        UDim2::new(0.25, 10.0, 1.0, -30.0),
        UDim2::new(0.5, -20.0, 0.0, 30.0),
    );
    let deepest = frame(
        &mut tree,
        inner,
        UDim2::from_scale(1.0, 0.5),
        UDim2::from_scale(0.5, 0.5),
    );
    tree[deepest].anchor_point = v(1.0, 0.5);
    let l = run(&tree);
    // 0.5 x 800 and 0.5 x 542 + 100.
    assert_rect(&l, outer, (50.0, 20.0, 400.0, 371.0));
    // x: 50 + 100 + 10; y: 20 + 371 - 30; w: 200 - 20.
    assert_rect(&l, inner, (160.0, 361.0, 180.0, 30.0));
    // Half of 180x30, its right middle on the inner frame's right middle.
    assert_rect(&l, deepest, (250.0, 368.5, 90.0, 15.0));
}

#[test]
fn size_constraint_picks_the_reference_axis() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let parent = frame(&mut tree, gui, px(0.0, 0.0), px(400.0, 200.0));
    let xx = frame(&mut tree, parent, px(0.0, 0.0), UDim2::from_scale(0.5, 0.5));
    tree[xx].size_constraint = SizeConstraint::RelativeXX;
    let yy = frame(&mut tree, parent, px(0.0, 0.0), UDim2::new(0.5, 10.0, 0.5, 0.0));
    tree[yy].size_constraint = SizeConstraint::RelativeYY;
    let l = run(&tree);
    assert_rect(&l, xx, (0.0, 0.0, 200.0, 200.0));
    assert_rect(&l, yy, (0.0, 0.0, 110.0, 100.0));
}

#[test]
fn padding_insets_the_content_area() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let f = frame(&mut tree, gui, px(100.0, 50.0), px(300.0, 200.0));
    let pad = tree.add(Class::UIPadding, Some(f));
    tree[pad].padding_left = UDim::new(0.0, 10.0);
    tree[pad].padding_right = UDim::new(0.0, 20.0);
    tree[pad].padding_top = UDim::new(0.0, 5.0);
    tree[pad].padding_bottom = UDim::new(0.1, 0.0);
    let child = frame(&mut tree, f, px(0.0, 0.0), UDim2::from_scale(1.0, 1.0));
    let l = run(&tree);
    // The bottom's 0.1 is of the frame's 200 height: 20.
    let content = l.get(f).unwrap().content;
    assert_eq!((content.min, content.max), (v(110.0, 55.0), v(380.0, 230.0)));
    assert_rect(&l, child, (110.0, 55.0, 270.0, 175.0));
    assert!(l.get(pad).is_none());
}

#[test]
fn aspect_ratio_constraint() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let wide = frame(&mut tree, gui, px(0.0, 0.0), px(300.0, 100.0));
    let a = tree.add(Class::UIAspectRatioConstraint, Some(wide));
    tree[a].aspect_ratio = 2.0;
    let tall = frame(&mut tree, gui, px(0.0, 0.0), px(300.0, 100.0));
    let b = tree.add(Class::UIAspectRatioConstraint, Some(tall));
    tree[b].aspect_ratio = 0.5;
    // ScaleWithParentSize keeps the dominant axis, within the parent.
    let by_width = frame(&mut tree, gui, px(0.0, 0.0), UDim2::new(1.0, 0.0, 0.0, 100.0));
    let c = tree.add(Class::UIAspectRatioConstraint, Some(by_width));
    tree[c].aspect_ratio = 2.0;
    tree[c].aspect_type = AspectType::ScaleWithParentSize;
    let too_big = frame(&mut tree, gui, px(0.0, 0.0), UDim2::new(1.0, 0.0, 0.0, 100.0));
    let d = tree.add(Class::UIAspectRatioConstraint, Some(too_big));
    tree[d].aspect_type = AspectType::ScaleWithParentSize;
    let by_height = frame(&mut tree, gui, px(0.0, 0.0), px(10.0, 300.0));
    let e = tree.add(Class::UIAspectRatioConstraint, Some(by_height));
    tree[e].aspect_ratio = 1.5;
    tree[e].aspect_type = AspectType::ScaleWithParentSize;
    tree[e].dominant_axis = DominantAxis::Height;
    let l = run(&tree);
    assert_rect(&l, wide, (0.0, 0.0, 200.0, 100.0));
    assert_rect(&l, tall, (0.0, 0.0, 50.0, 100.0));
    assert_rect(&l, by_width, (0.0, 0.0, 800.0, 400.0));
    // 800x800 doesn't fit 800x542: the largest square that does.
    assert_rect(&l, too_big, (0.0, 0.0, 542.0, 542.0));
    assert_rect(&l, by_height, (0.0, 0.0, 450.0, 300.0));
}

#[test]
fn size_constraint_clamps() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let big = frame(&mut tree, gui, px(0.0, 0.0), UDim2::from_scale(1.0, 1.0));
    let c = tree.add(Class::UISizeConstraint, Some(big));
    tree[c].min_size = v(100.0, 100.0);
    tree[c].max_size = v(400.0, 300.0);
    let small = frame(&mut tree, gui, px(0.0, 0.0), px(50.0, 150.0));
    let d = tree.add(Class::UISizeConstraint, Some(small));
    tree[d].min_size = v(100.0, 100.0);
    let l = run(&tree);
    assert_rect(&l, big, (0.0, 0.0, 400.0, 300.0));
    assert_rect(&l, small, (0.0, 0.0, 100.0, 150.0));
}

// ---------------------------------------------------------------- UIListLayout

/// A 200x300 frame at the origin with a vertical UIListLayout (Padding 10,
/// LayoutOrder) and items B (50x30), A (100x50) and C (200x20) by
/// LayoutOrder, plus a hidden one.
fn vertical_list() -> (Tree, NodeId, NodeId, [NodeId; 4]) {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let f = frame(&mut tree, gui, px(0.0, 0.0), px(200.0, 300.0));
    let list = tree.add(Class::UIListLayout, Some(f));
    tree[list].padding = UDim::new(0.0, 10.0);
    tree[list].sort_order = SortOrder::LayoutOrder;
    // Position and AnchorPoint don't count in a layout.
    let a = frame(&mut tree, f, px(999.0, 999.0), px(100.0, 50.0));
    tree[a].anchor_point = v(1.0, 1.0);
    tree[a].layout_order = 2;
    let b = frame(&mut tree, f, px(0.0, 0.0), px(50.0, 30.0));
    tree[b].layout_order = 1;
    let c = frame(&mut tree, f, px(0.0, 0.0), px(200.0, 20.0));
    tree[c].layout_order = 3;
    let hidden = frame(&mut tree, f, px(7.0, 7.0), px(80.0, 80.0));
    tree[hidden].visible = false;
    tree[hidden].layout_order = 0;
    (tree, f, list, [a, b, c, hidden])
}

#[test]
fn vertical_list_layout() {
    let (tree, _, list, [a, b, c, hidden]) = vertical_list();
    let l = run(&tree);
    assert_rect(&l, b, (0.0, 0.0, 50.0, 30.0));
    assert_rect(&l, a, (0.0, 40.0, 100.0, 50.0));
    assert_rect(&l, c, (0.0, 100.0, 200.0, 20.0));
    // 30 + 10 + 50 + 10 + 20, and the widest.
    assert_eq!(l.get(list).unwrap().content_size, v(200.0, 120.0));
    // A hidden item is left out, placed by its own Position, and not drawn.
    assert_rect(&l, hidden, (7.0, 7.0, 80.0, 80.0));
    assert!(!l.get(hidden).unwrap().shown);
    assert!(!l.draw_order.contains(&hidden));
}

#[test]
fn vertical_list_alignment() {
    let (mut tree, _, list, [a, b, c, _]) = vertical_list();
    // Across the fill direction each item aligns on its own; along it the group does.
    tree[list].horizontal_alignment = HorizontalAlignment::Center;
    tree[list].vertical_alignment = VerticalAlignment::Bottom;
    let l = run(&tree);
    assert_rect(&l, b, (75.0, 180.0, 50.0, 30.0));
    assert_rect(&l, a, (50.0, 220.0, 100.0, 50.0));
    assert_rect(&l, c, (0.0, 280.0, 200.0, 20.0));
    tree[list].horizontal_alignment = HorizontalAlignment::Right;
    tree[list].vertical_alignment = VerticalAlignment::Center;
    let l = run(&tree);
    assert_rect(&l, b, (150.0, 90.0, 50.0, 30.0));
    assert_rect(&l, a, (100.0, 130.0, 100.0, 50.0));
}

#[test]
fn horizontal_list_layout_by_name() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let f = frame(&mut tree, gui, px(10.0, 10.0), px(400.0, 100.0));
    let list = tree.add(Class::UIListLayout, Some(f));
    tree[list].fill_direction = FillDirection::Horizontal;
    tree[list].horizontal_alignment = HorizontalAlignment::Right;
    tree[list].vertical_alignment = VerticalAlignment::Center;
    // A scale padding is of the container's length on the fill axis: 0.05 x 400.
    tree[list].padding = UDim::new(0.05, 0.0);
    let c = frame(&mut tree, f, px(0.0, 0.0), px(70.0, 100.0));
    tree[c].name = "c".into();
    let a = frame(&mut tree, f, px(0.0, 0.0), px(50.0, 20.0));
    tree[a].name = "a".into();
    let b = frame(&mut tree, f, px(0.0, 0.0), px(60.0, 40.0));
    tree[b].name = "b".into();
    let l = run(&tree);
    // 50 + 60 + 70 + 2 x 20 = 220, against the right edge: starting at 10 + 180.
    assert_rect(&l, a, (190.0, 50.0, 50.0, 20.0));
    assert_rect(&l, b, (260.0, 40.0, 60.0, 40.0));
    assert_rect(&l, c, (340.0, 10.0, 70.0, 100.0));
    assert_eq!(l.get(list).unwrap().content_size, v(220.0, 100.0));
}

#[test]
fn list_layout_sorts_stably() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let f = frame(&mut tree, gui, px(0.0, 0.0), px(100.0, 100.0));
    tree.add(Class::UIListLayout, Some(f));
    let names = ["b", "a", "b", "A"];
    let ids: Vec<NodeId> = names
        .iter()
        .map(|n| {
            let id = frame(&mut tree, f, px(0.0, 0.0), px(10.0, 10.0));
            tree[id].name = n.to_string();
            id
        })
        .collect();
    let l = run(&tree);
    // By name in byte order ("A" before "a"), ties in child order.
    let y = |id: NodeId| rect(&l, id).1;
    assert_eq!([y(ids[3]), y(ids[1]), y(ids[0]), y(ids[2])], [0.0, 10.0, 20.0, 30.0]);
}

#[test]
fn list_layout_wraps() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let f = frame(&mut tree, gui, px(0.0, 0.0), px(200.0, 100.0));
    let list = tree.add(Class::UIListLayout, Some(f));
    tree[list].fill_direction = FillDirection::Horizontal;
    tree[list].padding = UDim::new(0.0, 10.0);
    tree[list].sort_order = SortOrder::LayoutOrder;
    tree[list].wraps = true;
    let items: Vec<NodeId> = (0..4)
        .map(|_| frame(&mut tree, f, px(0.0, 0.0), px(80.0, 20.0)))
        .collect();
    let l = run(&tree);
    // Two fit a line (80 + 10 + 80 = 170); a third would make 260.
    assert_rect(&l, items[0], (0.0, 0.0, 80.0, 20.0));
    assert_rect(&l, items[1], (90.0, 0.0, 80.0, 20.0));
    assert_rect(&l, items[2], (0.0, 30.0, 80.0, 20.0));
    assert_rect(&l, items[3], (90.0, 30.0, 80.0, 20.0));
    assert_eq!(l.get(list).unwrap().content_size, v(170.0, 50.0));
}

#[test]
fn constraints_override_a_list_layout() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let f = frame(&mut tree, gui, px(0.0, 0.0), px(300.0, 300.0));
    tree.add(Class::UIListLayout, Some(f));
    let a = frame(&mut tree, f, px(0.0, 0.0), UDim2::new(1.0, 0.0, 0.0, 40.0));
    tree[a].name = "a".into();
    let c = tree.add(Class::UISizeConstraint, Some(a));
    tree[c].max_size = v(120.0, 30.0);
    let b = frame(&mut tree, f, px(0.0, 0.0), UDim2::new(1.0, 0.0, 0.0, 40.0));
    tree[b].name = "b".into();
    let r = tree.add(Class::UIAspectRatioConstraint, Some(b));
    tree[r].aspect_ratio = 3.0;
    let l = run(&tree);
    assert_rect(&l, a, (0.0, 0.0, 120.0, 30.0));
    assert_rect(&l, b, (0.0, 30.0, 120.0, 40.0));
}

// ---------------------------------------------------------------- UIGridLayout

/// A 350x300 frame with a default UIGridLayout (100x100 cells, 5 apart) and five items.
/// A 300 x 100 frame with a horizontal list, and items of these sizes in it.
fn flex_row(sizes: &[(f64, f64)]) -> (Tree, NodeId, Vec<NodeId>) {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let f = frame(&mut tree, gui, px(0.0, 0.0), px(300.0, 100.0));
    let list = tree.add(Class::UIListLayout, Some(f));
    tree[list].fill_direction = FillDirection::Horizontal;
    tree[list].sort_order = SortOrder::LayoutOrder;
    let items = sizes
        .iter()
        .map(|&(w, h)| frame(&mut tree, f, px(0.0, 0.0), px(w, h)))
        .collect();
    (tree, list, items)
}

/// Gives `item` a UIFlexItem.
fn flex_item(tree: &mut Tree, item: NodeId, mode: FlexMode) -> NodeId {
    let f = tree.add(Class::UIFlexItem, Some(item));
    tree[f].flex_mode = mode;
    f
}

#[test]
fn flex_items_grow_shrink_and_fill() {
    // Two 50-wide items that grow share the 200 left: 150 each.
    let (mut tree, _, items) = flex_row(&[(50.0, 20.0), (50.0, 20.0)]);
    for &i in &items {
        flex_item(&mut tree, i, FlexMode::Grow);
    }
    let l = run(&tree);
    assert_rect(&l, items[0], (0.0, 0.0, 150.0, 20.0));
    assert_rect(&l, items[1], (150.0, 0.0, 150.0, 20.0));
    // GrowRatio 1 and 3 share it as 50 and 150.
    let (mut tree, _, items) = flex_row(&[(50.0, 20.0), (50.0, 20.0)]);
    for (&i, ratio) in items.iter().zip([1.0, 3.0]) {
        let f = flex_item(&mut tree, i, FlexMode::Custom);
        tree[f].grow_ratio = ratio;
    }
    let l = run(&tree);
    assert_rect(&l, items[0], (0.0, 0.0, 100.0, 20.0));
    assert_rect(&l, items[1], (100.0, 0.0, 200.0, 20.0));
    // Shrink fits oversized items, each by its share of their sizes.
    let (mut tree, _, items) = flex_row(&[(200.0, 20.0), (200.0, 20.0)]);
    for &i in &items {
        flex_item(&mut tree, i, FlexMode::Shrink);
    }
    let l = run(&tree);
    assert_rect(&l, items[0], (0.0, 0.0, 150.0, 20.0));
    assert_rect(&l, items[1], (150.0, 0.0, 150.0, 20.0));
    // The layout's HorizontalFlex: gaps, or every item filling the line.
    let (mut tree, list2, items) = flex_row(&[(50.0, 20.0), (50.0, 20.0), (50.0, 20.0)]);
    let xs = |tree: &Tree| -> Vec<f64> {
        let l = run(tree);
        items.iter().map(|&i| l.get(i).unwrap().position.x).collect()
    };
    tree[list2].horizontal_flex = FlexAlignment::SpaceBetween;
    assert_eq!(xs(&tree), [0.0, 125.0, 250.0]);
    tree[list2].horizontal_flex = FlexAlignment::SpaceEvenly;
    assert_eq!(xs(&tree), [37.5, 125.0, 212.5]);
    tree[list2].horizontal_flex = FlexAlignment::SpaceAround;
    assert_eq!(xs(&tree), [25.0, 125.0, 225.0]);
    tree[list2].horizontal_flex = FlexAlignment::Fill;
    let l = run(&tree);
    assert_rect(&l, items[2], (200.0, 0.0, 100.0, 20.0));
    // A UIFlexItem that doesn't flex keeps its size in a filling line.
    flex_item(&mut tree, items[0], FlexMode::None);
    let l = run(&tree);
    assert_rect(&l, items[0], (0.0, 0.0, 50.0, 20.0));
    assert_rect(&l, items[1], (50.0, 0.0, 125.0, 20.0));
    // Across: Stretch fills the line; an item's own ItemLineAlignment wins.
    let (mut tree, list, items) = flex_row(&[(50.0, 20.0), (50.0, 20.0), (50.0, 20.0)]);
    tree[list].item_line_alignment = ItemLineAlignment::Stretch;
    let f = flex_item(&mut tree, items[1], FlexMode::None);
    tree[f].item_line_alignment = ItemLineAlignment::End;
    let f = flex_item(&mut tree, items[2], FlexMode::None);
    tree[f].item_line_alignment = ItemLineAlignment::Center;
    let l = run(&tree);
    assert_rect(&l, items[0], (0.0, 0.0, 50.0, 100.0));
    assert_rect(&l, items[1], (50.0, 80.0, 50.0, 20.0));
    assert_rect(&l, items[2], (100.0, 40.0, 50.0, 20.0));
}

#[test]
fn flex_shares_the_room_between_wrapped_lines() {
    // Items 200 wide wrap one to a line; VerticalFlex shares the 60 left.
    let (mut tree, list, items) = flex_row(&[(200.0, 20.0), (200.0, 20.0)]);
    tree[list].wraps = true;
    let ys = |tree: &Tree| -> Vec<(f64, f64)> {
        let l = run(tree);
        items
            .iter()
            .map(|&i| (l.get(i).unwrap().position.y, l.get(i).unwrap().size.y))
            .collect()
    };
    assert_eq!(ys(&tree), [(0.0, 20.0), (20.0, 20.0)]);
    tree[list].vertical_flex = FlexAlignment::Fill;
    assert_eq!(ys(&tree), [(0.0, 20.0), (50.0, 20.0)]);
    tree[list].item_line_alignment = ItemLineAlignment::Stretch;
    assert_eq!(ys(&tree), [(0.0, 50.0), (50.0, 50.0)]);
    tree[list].item_line_alignment = ItemLineAlignment::Automatic;
    tree[list].vertical_flex = FlexAlignment::SpaceBetween;
    assert_eq!(ys(&tree), [(0.0, 20.0), (80.0, 20.0)]);
    tree[list].vertical_flex = FlexAlignment::SpaceEvenly;
    assert_eq!(ys(&tree), [(20.0, 20.0), (60.0, 20.0)]);
}

fn grid() -> (Tree, NodeId, Vec<NodeId>) {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let f = frame(&mut tree, gui, px(0.0, 0.0), px(350.0, 300.0));
    let g = tree.add(Class::UIGridLayout, Some(f));
    tree[g].sort_order = SortOrder::LayoutOrder;
    let items = (0..5)
        .map(|_| frame(&mut tree, f, px(3.0, 3.0), px(1.0, 1.0)))
        .collect();
    (tree, g, items)
}

#[test]
fn grid_layout() {
    let (tree, g, items) = grid();
    let l = run(&tree);
    // Three cells fit a row: 3 x 100 + 2 x 5 = 310 <= 350, four would be 415.
    let want = [(0.0, 0.0), (105.0, 0.0), (210.0, 0.0), (0.0, 105.0), (105.0, 105.0)];
    for (&id, (x, y)) in items.iter().zip(want) {
        assert_rect(&l, id, (x, y, 100.0, 100.0));
    }
    let p = l.get(g).unwrap();
    assert_eq!(p.content_size, v(310.0, 205.0));
    assert_eq!(p.cell_size, v(100.0, 100.0));
    assert_eq!(p.cell_count, v(3.0, 2.0));
}

#[test]
fn grid_layout_alignment_and_corners() {
    let (mut tree, g, items) = grid();
    tree[g].horizontal_alignment = HorizontalAlignment::Center;
    tree[g].vertical_alignment = VerticalAlignment::Bottom;
    let l = run(&tree);
    // The 310x205 block: 20 in from the left, 95 down.
    assert_rect(&l, items[0], (20.0, 95.0, 100.0, 100.0));
    assert_rect(&l, items[4], (125.0, 200.0, 100.0, 100.0));

    let (mut tree, g, items) = grid();
    tree[g].start_corner = StartCorner::BottomRight;
    let l = run(&tree);
    assert_rect(&l, items[0], (210.0, 105.0, 100.0, 100.0));
    assert_rect(&l, items[2], (0.0, 105.0, 100.0, 100.0));
    assert_rect(&l, items[3], (210.0, 0.0, 100.0, 100.0));
    assert_rect(&l, items[4], (105.0, 0.0, 100.0, 100.0));
}

#[test]
fn grid_layout_columns_and_max_cells() {
    let (mut tree, g, items) = grid();
    tree[g].fill_direction = FillDirection::Vertical;
    let l = run(&tree);
    // Two cells fit a 300-high column (205; three would be 310), so three columns.
    assert_rect(&l, items[1], (0.0, 105.0, 100.0, 100.0));
    assert_rect(&l, items[2], (105.0, 0.0, 100.0, 100.0));
    assert_rect(&l, items[4], (210.0, 0.0, 100.0, 100.0));
    assert_eq!(l.get(g).unwrap().content_size, v(310.0, 205.0));
    assert_eq!(l.get(g).unwrap().cell_count, v(3.0, 2.0));

    let (mut tree, g, items) = grid();
    tree[g].fill_direction_max_cells = 2;
    tree[g].cell_size = UDim2::new(0.5, -5.0, 0.0, 50.0);
    tree[g].cell_padding = UDim2::new(0.0, 10.0, 0.0, 0.0);
    let l = run(&tree);
    // Cells of 0.5 x 350 - 5 = 170 by 50, two a row.
    assert_rect(&l, items[1], (180.0, 0.0, 170.0, 50.0));
    assert_rect(&l, items[2], (0.0, 50.0, 170.0, 50.0));
    assert_rect(&l, items[4], (0.0, 100.0, 170.0, 50.0));
    assert_eq!(l.get(g).unwrap().content_size, v(350.0, 150.0));
}

// ---------------------------------------------------------------- AutomaticSize and text

#[test]
fn automatic_size_fits_text() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let label = add(&mut tree, Class::TextLabel, gui, px(10.0, 10.0), px(0.0, 0.0));
    tree[label].text = "Hello".into();
    tree[label].text_size = 20.0;
    tree[label].automatic_size = AutomaticSize::XY;
    let padded = add(&mut tree, Class::TextLabel, gui, px(0.0, 100.0), px(0.0, 0.0));
    tree[padded].text = "Hello".into();
    tree[padded].text_size = 20.0;
    tree[padded].automatic_size = AutomaticSize::XY;
    let pad = tree.add(Class::UIPadding, Some(padded));
    for side in ["top", "bottom", "left", "right"] {
        let n = &mut tree[pad];
        let u = UDim::new(0.0, 5.0);
        match side {
            "top" => n.padding_top = u,
            "bottom" => n.padding_bottom = u,
            "left" => n.padding_left = u,
            _ => n.padding_right = u,
        }
    }
    // The Size is a minimum.
    let floor = add(&mut tree, Class::TextLabel, gui, px(0.0, 200.0), px(80.0, 30.0));
    tree[floor].text = "Hi".into();
    tree[floor].text_size = 20.0;
    tree[floor].automatic_size = AutomaticSize::XY;
    let l = run(&tree);
    // Five characters at 10 wide, one 20-high line.
    assert_rect(&l, label, (10.0, 10.0, 50.0, 20.0));
    assert_eq!(l.get(label).unwrap().text.as_ref().unwrap().bounds, v(50.0, 20.0));
    assert_rect(&l, padded, (0.0, 100.0, 60.0, 30.0));
    assert_rect(&l, floor, (0.0, 200.0, 80.0, 30.0));
}

#[test]
fn automatic_size_y_wraps_at_the_width() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let label = add(&mut tree, Class::TextLabel, gui, px(0.0, 0.0), px(100.0, 0.0));
    tree[label].text = "aaaa bbbb cccc dddd eeee".into();
    tree[label].text_size = 10.0;
    tree[label].text_wrapped = true;
    tree[label].automatic_size = AutomaticSize::Y;
    tree[label].text_x_alignment = TextXAlignment::Left;
    let l = run(&tree);
    // 19 characters (95 wide) fit 100; the last word goes to a second line.
    assert_rect(&l, label, (0.0, 0.0, 100.0, 20.0));
    let text = l.get(label).unwrap().text.clone().unwrap();
    let lines: Vec<&str> = text.lines.iter().map(|t| t.text.as_str()).collect();
    assert_eq!(lines, ["aaaa bbbb cccc dddd", "eeee"]);
    assert_eq!(text.lines[1].position, v(0.0, 10.0));
    assert_eq!(text.lines[0].width, 95.0);
    assert!(text.fits);
}

#[test]
fn automatic_size_keeps_the_anchor() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let label = add(
        &mut tree,
        Class::TextLabel,
        gui,
        UDim2::from_scale(1.0, 0.5),
        px(0.0, 0.0),
    );
    tree[label].anchor_point = v(1.0, 0.5);
    tree[label].text = "Hello".into();
    tree[label].text_size = 20.0;
    tree[label].automatic_size = AutomaticSize::XY;
    let l = run(&tree);
    // It grows left and both ways from its right middle at (800, 271).
    assert_rect(&l, label, (750.0, 261.0, 50.0, 20.0));
}

#[test]
fn automatic_size_with_a_list_layout() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let f = frame(&mut tree, gui, px(0.0, 0.0), px(200.0, 0.0));
    tree[f].automatic_size = AutomaticSize::Y;
    let list = tree.add(Class::UIListLayout, Some(f));
    tree[list].padding = UDim::new(0.0, 5.0);
    let rows: Vec<NodeId> = (0..3)
        .map(|_| frame(&mut tree, f, px(0.0, 0.0), UDim2::new(1.0, 0.0, 0.0, 30.0)))
        .collect();
    let l = run(&tree);
    // 3 x 30 + 2 x 5.
    assert_rect(&l, f, (0.0, 0.0, 200.0, 100.0));
    assert_rect(&l, rows[2], (0.0, 70.0, 200.0, 30.0));
    assert_eq!(l.get(list).unwrap().content_size, v(200.0, 100.0));
}

#[test]
fn automatic_size_bounds_the_children() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let f = frame(&mut tree, gui, px(5.0, 5.0), px(0.0, 0.0));
    tree[f].automatic_size = AutomaticSize::XY;
    frame(&mut tree, f, px(10.0, 10.0), px(50.0, 20.0));
    frame(&mut tree, f, px(100.0, 0.0), px(20.0, 60.0));
    // Scale-sized children don't count on that axis; hidden ones not at all.
    frame(&mut tree, f, px(0.0, 0.0), UDim2::new(1.0, 0.0, 1.0, 0.0));
    let hidden = frame(&mut tree, f, px(500.0, 500.0), px(10.0, 10.0));
    tree[hidden].visible = false;
    let pad = tree.add(Class::UIPadding, Some(f));
    tree[pad].padding_right = UDim::new(0.0, 4.0);
    tree[pad].padding_bottom = UDim::new(0.0, 6.0);
    let l = run(&tree);
    assert_rect(&l, f, (5.0, 5.0, 124.0, 66.0));
}

#[test]
fn automatic_size_is_capped_by_a_size_constraint() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let label = add(&mut tree, Class::TextLabel, gui, px(0.0, 0.0), px(0.0, 0.0));
    tree[label].text = "a long line of text".into();
    tree[label].text_size = 10.0;
    tree[label].automatic_size = AutomaticSize::XY;
    let c = tree.add(Class::UISizeConstraint, Some(label));
    tree[c].max_size = v(60.0, f64::INFINITY);
    let l = run(&tree);
    assert_rect(&l, label, (0.0, 0.0, 60.0, 10.0));
    assert!(!l.get(label).unwrap().text.as_ref().unwrap().fits);
}

#[test]
fn text_scaled_picks_the_largest_size_that_fits() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let one = add(&mut tree, Class::TextLabel, gui, px(0.0, 0.0), px(100.0, 20.0));
    tree[one].text = "abcd".into();
    tree[one].text_scaled = true;
    let capped = add(&mut tree, Class::TextLabel, gui, px(0.0, 100.0), px(100.0, 20.0));
    tree[capped].text = "abcd".into();
    tree[capped].text_scaled = true;
    let limit = tree.add(Class::UITextSizeConstraint, Some(capped));
    tree[limit].max_text_size = 14.0;
    let two = add(&mut tree, Class::TextLabel, gui, px(0.0, 200.0), px(40.0, 40.0));
    tree[two].text = "aaaa bbbb".into();
    tree[two].text_scaled = true;
    let l = run(&tree);
    // The height allows 20 (the width would allow 50).
    let t = l.get(one).unwrap().text.clone().unwrap();
    assert_eq!(t.size, 20.0);
    assert_eq!(t.bounds, v(40.0, 20.0));
    assert_eq!(t.lines[0].position, v(30.0, 0.0));
    // UITextSizeConstraint caps it; centered in 100x20.
    let t = l.get(capped).unwrap().text.clone().unwrap();
    assert_eq!(t.size, 14.0);
    assert_eq!(t.lines[0].position, v(36.0, 103.0));
    // Wrapped to two lines it can be 20; on one line only 8.
    let t = l.get(two).unwrap().text.clone().unwrap();
    assert_eq!(t.size, 20.0);
    assert_eq!(t.lines.len(), 2);
    assert!(t.fits);
}

#[test]
fn text_alignment_and_constraints() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let label = add(&mut tree, Class::TextLabel, gui, px(0.0, 0.0), px(200.0, 50.0));
    tree[label].text = "hi".into();
    tree[label].text_size = 10.0;
    let at = |tree: &mut Tree, x: TextXAlignment, y: TextYAlignment| {
        tree[label].text_x_alignment = x;
        tree[label].text_y_alignment = y;
        run(tree).get(label).unwrap().text.clone().unwrap().lines[0].position
    };
    assert_eq!(
        at(&mut tree, TextXAlignment::Center, TextYAlignment::Center),
        v(95.0, 20.0)
    );
    assert_eq!(at(&mut tree, TextXAlignment::Left, TextYAlignment::Top), v(0.0, 0.0));
    assert_eq!(
        at(&mut tree, TextXAlignment::Right, TextYAlignment::Bottom),
        v(190.0, 40.0)
    );
    // Padding moves the text's area; a UITextSizeConstraint clamps a plain TextSize too.
    let pad = tree.add(Class::UIPadding, Some(label));
    tree[pad].padding_left = UDim::new(0.0, 8.0);
    let limit = tree.add(Class::UITextSizeConstraint, Some(label));
    tree[limit].min_text_size = 16.0;
    let l = run(&tree);
    let t = l.get(label).unwrap().text.clone().unwrap();
    assert_eq!(t.size, 16.0);
    assert_eq!(t.lines[0].position, v(184.0, 34.0));
    // Lines break at "\n" even without wrapping.
    tree[label].text = "one\ntwo".into();
    let t = run(&tree).get(label).unwrap().text.clone().unwrap();
    assert_eq!(t.lines.len(), 2);
    assert_eq!(t.bounds, v(24.0, 32.0));
}

#[test]
fn wrapped_text_drops_lines_below_its_area() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let label = add(&mut tree, Class::TextLabel, gui, px(0.0, 0.0), px(50.0, 15.0));
    tree[label].text = "aaaa bbbb cccc".into();
    tree[label].text_size = 10.0;
    tree[label].text_wrapped = true;
    let word = add(&mut tree, Class::TextLabel, gui, px(0.0, 100.0), px(30.0, 100.0));
    tree[word].text = "abcdefghij".into();
    tree[word].text_size = 10.0;
    tree[word].text_wrapped = true;
    let l = run(&tree);
    let t = l.get(label).unwrap().text.clone().unwrap();
    assert_eq!(t.bounds, v(45.0, 20.0));
    assert!(!t.fits);
    assert_eq!(t.lines.len(), 1);
    assert_eq!(t.lines[0].text, "aaaa bbbb");
    // A word too long for the line breaks inside it.
    let t = l.get(word).unwrap().text.clone().unwrap();
    let lines: Vec<&str> = t.lines.iter().map(|t| t.text.as_str()).collect();
    assert_eq!(lines, ["abcdef", "ghij"]);
}

/// A label's lines as drawn: (text, x, y, width, start).
fn text_lines(l: &Layout, id: NodeId) -> Vec<(String, f64, f64, f64, usize)> {
    let t = l.get(id).unwrap().text.clone().unwrap();
    t.lines
        .into_iter()
        .map(|l| (l.text, l.position.x, l.position.y, l.width, l.start))
        .collect()
}

#[test]
fn lines_know_where_they_start() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    // 10 characters a line; the space a line breaks at belongs to neither.
    let label = add(&mut tree, Class::TextLabel, gui, px(0.0, 0.0), px(50.0, 100.0));
    tree[label].text = " aaaa bbbb cccc\nabcdefghijkl".into();
    tree[label].text_size = 10.0;
    tree[label].text_wrapped = true;
    tree[label].text_y_alignment = TextYAlignment::Top;
    let starts: Vec<(String, usize)> = text_lines(&run(&tree), label).into_iter().map(|l| (l.0, l.4)).collect();
    let want = [("aaaa bbbb", 1), ("cccc", 11), ("abcdefghij", 16), ("kl", 26)];
    let want: Vec<(String, usize)> = want.iter().map(|(t, s)| (t.to_string(), *s)).collect();
    assert_eq!(starts, want);
    // Unwrapped, each line is a paragraph.
    tree[label].text_wrapped = false;
    let starts: Vec<usize> = text_lines(&run(&tree), label).iter().map(|l| l.4).collect();
    assert_eq!(starts, [0, 16]);
}

#[test]
fn text_truncate_at_end_and_split_word() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    // 55 wide: 11 characters of 5.
    let label = add(&mut tree, Class::TextLabel, gui, px(0.0, 0.0), px(55.0, 20.0));
    tree[label].text = "Hello world wide".into();
    tree[label].text_size = 10.0;
    tree[label].text_x_alignment = TextXAlignment::Left;
    let drawn = |tree: &Tree| -> Vec<String> { text_lines(&run(tree), label).into_iter().map(|l| l.0).collect() };
    assert_eq!(drawn(&tree), ["Hello world wide"], "None draws it all");
    tree[label].text_truncate = TextTruncate::AtEnd;
    let l = run(&tree);
    let lines = text_lines(&l, label);
    assert_eq!((lines[0].0.as_str(), lines[0].3), ("Hello wo...", 55.0));
    let t = l.get(label).unwrap().text.clone().unwrap();
    assert!(!t.fits);
    assert_eq!(t.bounds, v(80.0, 10.0), "the bounds are the whole text's");
    tree[label].text_truncate = TextTruncate::SplitWord;
    assert_eq!(drawn(&tree), ["Hello..."]);
    // A centred line is centred as cut.
    tree[label].text_x_alignment = TextXAlignment::Center;
    let lines = text_lines(&run(&tree), label);
    assert_eq!(lines[0].1, (55.0 - 40.0) / 2.0);
    // Text that fits is left alone.
    tree[label].text = "Hello".into();
    assert_eq!(drawn(&tree), ["Hello"]);
    // Wrapped text cut off by its height ends its last line with "...".
    tree[label].text = "aaaa bbbb cccc".into();
    tree[label].text_wrapped = true;
    tree[label].size = px(50.0, 15.0);
    tree[label].text_truncate = TextTruncate::AtEnd;
    assert_eq!(drawn(&tree), ["aaaa bb..."]);
    tree[label].text_truncate = TextTruncate::SplitWord;
    assert_eq!(drawn(&tree), ["aaaa..."]);
    // A first word too long for the line is cut inside it; too narrow for "..." leaves what dots fit.
    tree[label].text = "abcdefghijklmnop".into();
    tree[label].text_wrapped = false;
    assert_eq!(drawn(&tree), ["abcdefg..."]);
    tree[label].size = px(12.0, 15.0);
    assert_eq!(drawn(&tree), [".."]);
}

#[test]
fn max_visible_graphemes_shows_the_start() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let label = add(&mut tree, Class::TextLabel, gui, px(0.0, 0.0), px(200.0, 50.0));
    tree[label].text = "Hello\nWorld".into();
    tree[label].text_size = 10.0;
    let whole = text_lines(&run(&tree), label);
    tree[label].max_visible_graphemes = 7;
    let l = run(&tree);
    let lines = text_lines(&l, label);
    // "Hello", the line break, then "W", each where the whole text has it.
    assert_eq!(lines.len(), 2);
    assert_eq!(
        (lines[0].0.as_str(), lines[0].1, lines[0].2),
        ("Hello", whole[0].1, whole[0].2)
    );
    assert_eq!(
        (lines[1].0.as_str(), lines[1].1, lines[1].2),
        ("W", whole[1].1, whole[1].2)
    );
    assert_eq!(lines[1].3, 5.0);
    let t = l.get(label).unwrap().text.clone().unwrap();
    assert_eq!(t.bounds, v(25.0, 20.0), "the bounds are the whole text's");
    tree[label].max_visible_graphemes = 3;
    let lines = text_lines(&run(&tree), label);
    assert_eq!(lines.iter().map(|l| l.0.as_str()).collect::<Vec<_>>(), ["Hel"]);
    tree[label].max_visible_graphemes = 0;
    assert!(text_lines(&run(&tree), label).is_empty());
    tree[label].max_visible_graphemes = -1;
    assert_eq!(text_lines(&run(&tree), label), whole);
    // Characters, not bytes.
    tree[label].text = "héllo".into();
    tree[label].max_visible_graphemes = 2;
    assert_eq!(text_lines(&run(&tree), label)[0].0, "hé");
}

// ---------------------------------------------------------------- rich text

/// The runs of rich text as (text, its style).
fn spans(rich: &RichText) -> Vec<(&str, RichStyle)> {
    rich.spans
        .iter()
        .map(|sp| (&rich.text[sp.start..sp.end], sp.style.clone()))
        .collect()
}

fn red() -> RichStyle {
    RichStyle {
        color: Some(Color3::new(1.0, 0.0, 0.0)),
        ..RichStyle::default()
    }
}

fn bold() -> RichStyle {
    RichStyle {
        bold: true,
        ..RichStyle::default()
    }
}

#[test]
fn rich_text_reads_roblox_markup() {
    let r = parse_rich(r##"<font color="#FF0000">Red</font> plain <b>bold</b><br/>next"##);
    assert_eq!(r.text, "Red plain bold\nnext");
    let plain = RichStyle::default();
    assert_eq!(
        spans(&r),
        [
            ("Red", red()),
            (" plain ", plain.clone()),
            ("bold", bold()),
            ("\nnext", plain.clone())
        ]
    );
    // The runs cover the text, in order.
    assert_eq!(r.spans.first().unwrap().start, 0);
    assert!(r.spans.windows(2).all(|w| w[0].end == w[1].start));
    assert_eq!(r.spans.last().unwrap().end, r.text.len());
    // Escapes, comments, line breaks; unknown tags go, a lone < stays.
    let text = |s: &str| parse_rich(s).text;
    assert_eq!(text("&lt;b&gt; &amp; &quot;q&quot; &apos;"), "<b> & \"q\" '");
    assert_eq!(text("a<!-- hidden -->b<!-- open"), "ab");
    assert_eq!(text("a<br>b<br />c<BR/>d"), "a\nb\nc\nd");
    assert_eq!(text("<blink>x</blink> 1 < 2 and 3 > 2"), "x 1 < 2 and 3 > 2");
    assert_eq!(text("<uppercase>abc</uppercase> <uc>d</uc>"), "ABC D");
    // Small capitals: the lowercase letters as capitals 0.8 the size.
    let r = parse_rich("<sc>Hello</sc>");
    assert_eq!(r.text, "HELLO");
    let small: Vec<(&str, bool)> = spans(&r).into_iter().map(|(t, s)| (t, s.small)).collect();
    assert_eq!(small, [("H", false), ("ELLO", true)]);
    // Nesting, and a closing tag that closes what it holds too.
    let r = parse_rich("<b><i>x</i>y</b>z<b>a<i>b</b>c");
    let styles: Vec<(&str, bool, bool)> = spans(&r).into_iter().map(|(t, s)| (t, s.bold, s.italic)).collect();
    assert_eq!(
        styles,
        [
            ("x", true, true),
            ("y", true, false),
            ("z", false, false),
            ("a", true, false),
            ("b", true, true),
            ("c", false, false)
        ]
    );
    // Font attributes, quoted either way.
    let r = parse_rich(r#"<font size='30' face="GothamBold" transparency="0.5">x</font>"#);
    let s = &r.spans[0].style;
    assert_eq!((s.size, s.transparency), (Some(30.0), Some(0.5)));
    assert_eq!(s.family.as_deref(), Some("rbxasset://fonts/families/Montserrat.json"));
    assert_eq!(s.weight, Some(FontWeight::Bold));
    let style = |m: &str| parse_rich(m).spans[0].style.clone();
    assert_eq!(
        style(r#"<font color="rgb(0, 51, 255)">x</font>"#).color,
        Some(Color3::new(0.0, 0.2, 1.0))
    );
    assert_eq!(
        style(r##"<font color="#f80">x</font>"##).color,
        Some(Color3::from_rgb(255.0, 136.0, 0.0))
    );
    assert_eq!(
        style(r#"<font weight="heavy">x</font>"#).weight,
        Some(FontWeight::Heavy)
    );
    assert_eq!(style(r#"<font weight="650">x</font>"#).weight, Some(FontWeight::Bold));
    assert_eq!(
        style(r#"<font family="rbxasset://fonts/families/Ubuntu.json">x</font>"#)
            .family
            .as_deref(),
        Some("rbxasset://fonts/families/Ubuntu.json")
    );
    assert_eq!(
        style(r##"<stroke color="#00A2FF" thickness="2" transparency="0.25">x</stroke>"##).stroke,
        Some((Color3::from_rgb(0.0, 162.0, 255.0), 2.0, 0.25))
    );
    assert_eq!(
        style(r##"<mark color="#009966" transparency="0.5">x</mark>"##).mark,
        Some((Color3::from_rgb(0.0, 153.0, 102.0), 0.5))
    );
    let s = style("<u><s>x</s></u>");
    assert!(s.underline && s.strike);
}

/// Monospace, half as wide as tall, and half again as wide in a bold weight.
fn mono_bold(text: &str, size: f64, font: &Font) -> f64 {
    let w = text.chars().count() as f64 * size / 2.0;
    if font.bold() { w * 1.5 } else { w }
}

/// A label's rich lines as (text, x, y, width, height) and their runs as
/// (text, x, width, size).
#[allow(clippy::type_complexity)]
fn rich_lines(l: &Layout, id: NodeId) -> Vec<((String, f64, f64, f64, f64), Vec<(String, f64, f64, f64)>)> {
    let t = l.get(id).unwrap().text.clone().unwrap();
    t.lines
        .into_iter()
        .map(|l| {
            let runs = l.runs.iter().map(|r| (r.text.clone(), r.x, r.width, r.size)).collect();
            ((l.text, l.position.x, l.position.y, l.width, l.height), runs)
        })
        .collect()
}

fn s(t: &str) -> String {
    t.to_string()
}

#[test]
fn rich_text_runs() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let label = add(&mut tree, Class::TextLabel, gui, px(0.0, 0.0), px(400.0, 100.0));
    let rich = parse_rich(r##"<font color="#FF0000">Red</font> plain <b>bold</b><br/>next"##);
    tree[label].text = rich.text;
    tree[label].rich = rich.spans;
    tree[label].text_size = 20.0;
    tree[label].text_x_alignment = TextXAlignment::Left;
    tree[label].text_y_alignment = TextYAlignment::Top;
    let l = tree.layout(&Viewport::new(800.0, 600.0, TOP_BAR), &mut mono_bold);
    // Two lines: three runs (red, plain, bold, each measured in its own
    // font: bold is wider here), then "next".
    assert_eq!(
        rich_lines(&l, label),
        [
            (
                (s("Red plain bold"), 0.0, 0.0, 160.0, 20.0),
                vec![
                    (s("Red"), 0.0, 30.0, 20.0),
                    (s(" plain "), 30.0, 70.0, 20.0),
                    (s("bold"), 100.0, 60.0, 20.0)
                ]
            ),
            ((s("next"), 0.0, 20.0, 40.0, 20.0), vec![(s("next"), 0.0, 40.0, 20.0)])
        ]
    );
    let t = l.get(label).unwrap().text.clone().unwrap();
    assert_eq!(t.bounds, v(160.0, 40.0));
    let run = &t.lines[0].runs[2];
    assert_eq!((run.font.weight, run.style.bold), (FontWeight::Bold, true));
    // A larger run makes its line taller, and the lines below move down.
    let rich = parse_rich(r#"small <font size="40">big</font><br/>after"#);
    tree[label].text = rich.text;
    tree[label].rich = rich.spans;
    let l = tree.layout(&Viewport::new(800.0, 600.0, TOP_BAR), &mut mono_bold);
    let lines = rich_lines(&l, label);
    assert_eq!(lines[0].0, (s("small big"), 0.0, 0.0, 120.0, 40.0));
    assert_eq!(lines[0].1[1], (s("big"), 60.0, 60.0, 40.0));
    assert_eq!(lines[1].0, (s("after"), 0.0, 40.0, 50.0, 20.0));
    assert_eq!(l.get(label).unwrap().text.clone().unwrap().bounds, v(120.0, 60.0));
    // Wrapping measures run by run too: "small" and "big" don't fit 100.
    tree[label].size = px(100.0, 100.0);
    tree[label].text_wrapped = true;
    let l = tree.layout(&Viewport::new(800.0, 600.0, TOP_BAR), &mut mono_bold);
    let lines: Vec<String> = rich_lines(&l, label).into_iter().map(|l| l.0.0).collect();
    assert_eq!(lines, ["small", "big", "after"]);
}

#[test]
fn rich_text_truncates_and_types_out_run_by_run() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    // 55 wide: 11 characters of 5.
    let label = add(&mut tree, Class::TextLabel, gui, px(0.0, 0.0), px(55.0, 20.0));
    let set = |tree: &mut Tree, markup: &str| {
        let rich = parse_rich(markup);
        tree[label].text = rich.text;
        tree[label].rich = rich.spans;
    };
    set(&mut tree, "<b>Hello</b> world wide");
    tree[label].text_size = 10.0;
    tree[label].text_x_alignment = TextXAlignment::Left;
    tree[label].text_truncate = TextTruncate::AtEnd;
    let runs = |tree: &Tree| -> Vec<(String, bool)> {
        let l = run(tree);
        let t = l.get(label).unwrap().text.clone().unwrap();
        t.lines[0].runs.iter().map(|r| (r.text.clone(), r.style.bold)).collect()
    };
    // The dots take the style of what they follow.
    assert_eq!(runs(&tree), [(s("Hello"), true), (s(" wo"), false), (s("..."), false)]);
    set(&mut tree, "<b>Hello world</b> wide");
    assert_eq!(runs(&tree), [(s("Hello wo"), true), (s("..."), true)]);
    // MaxVisibleGraphemes cuts the runs.
    set(&mut tree, "<b>Hel</b>lo");
    tree[label].text_truncate = TextTruncate::None;
    tree[label].max_visible_graphemes = 4;
    assert_eq!(runs(&tree), [(s("Hel"), true), (s("l"), false)]);
}

// ---------------------------------------------------------------- scrolling and clipping

/// A 200x150 ScrollingFrame at (100, 100) with a 600-high canvas.
fn scroller() -> (Tree, NodeId, NodeId) {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let s = add(
        &mut tree,
        Class::ScrollingFrame,
        gui,
        px(100.0, 100.0),
        px(200.0, 150.0),
    );
    tree[s].canvas_size = UDim2::new(0.0, 0.0, 0.0, 600.0);
    (tree, gui, s)
}

#[test]
fn scrolling_moves_and_clips_the_canvas() {
    let (mut tree, _, s) = scroller();
    tree[s].canvas_position = v(0.0, 100.0);
    let row = frame(&mut tree, s, px(0.0, 120.0), UDim2::new(1.0, 0.0, 0.0, 50.0));
    let above = add(
        &mut tree,
        Class::TextButton,
        s,
        px(0.0, 0.0),
        UDim2::new(1.0, 0.0, 0.0, 50.0),
    );
    let below = add(
        &mut tree,
        Class::TextButton,
        s,
        px(0.0, 300.0),
        UDim2::new(1.0, 0.0, 0.0, 50.0),
    );
    let l = run(&tree);
    let p = l.get(s).unwrap();
    assert_eq!(
        (p.canvas_size, p.window_size, p.canvas_position),
        (v(200.0, 600.0), v(200.0, 150.0), v(0.0, 100.0))
    );
    // 100 down the canvas is the frame's top: a row at 120 is 20 below it.
    assert_rect(&l, row, (100.0, 120.0, 200.0, 50.0));
    assert_rect(&l, above, (100.0, 0.0, 200.0, 50.0));
    // Everything inside is clipped to the frame.
    let clip = l.get(row).unwrap().clip.unwrap();
    assert_eq!((clip.min, clip.max), (v(100.0, 100.0), v(300.0, 250.0)));
    assert_eq!(l.objects_at(v(150.0, 130.0)), vec![row, s]);
    // The button scrolled out above isn't under its old spot; the one below the window is cut off.
    assert_eq!(l.button_at(v(150.0, 20.0)), None);
    assert_eq!(l.button_at(v(150.0, 260.0)), None);
    assert!(l.objects_at(v(150.0, 320.0)).is_empty());
    assert!(l.objects_at(v(150.0, 245.0)).contains(&s));
    // Scrolled to the button below, it takes the click.
    tree[s].canvas_position = v(0.0, 250.0);
    let l = run(&tree);
    assert_rect(&l, below, (100.0, 150.0, 200.0, 50.0));
    assert_eq!(l.button_at(v(150.0, 160.0)), Some(below));
}

#[test]
fn canvas_position_is_clamped() {
    let (mut tree, _, s) = scroller();
    tree[s].canvas_position = v(50.0, 1000.0);
    let child = frame(&mut tree, s, px(0.0, 0.0), px(10.0, 10.0));
    let l = run(&tree);
    // The canvas is as wide as the window, and 600 - 150 is as far down as it goes.
    assert_eq!(l.get(s).unwrap().canvas_position, v(0.0, 450.0));
    assert_rect(&l, child, (100.0, -350.0, 10.0, 10.0));
    tree[s].canvas_position = v(-20.0, -20.0);
    assert_eq!(run(&tree).get(s).unwrap().canvas_position, v(0.0, 0.0));
}

#[test]
fn canvas_size_defaults_and_scroll_bar_insets() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let s = add(&mut tree, Class::ScrollingFrame, gui, px(0.0, 0.0), px(200.0, 150.0));
    let fill = frame(&mut tree, s, px(0.0, 0.0), UDim2::from_scale(1.0, 1.0));
    let l = run(&tree);
    // CanvasSize {0, 0}, {2, 0}: twice the frame's height.
    assert_eq!(l.get(s).unwrap().canvas_size, v(200.0, 300.0));
    assert_rect(&l, fill, (0.0, 0.0, 200.0, 300.0));
    // A vertical bar that shows takes its 12 pixels from the canvas width.
    tree[s].vertical_scroll_bar_inset = ScrollBarInset::ScrollBar;
    let l = run(&tree);
    assert_eq!(l.get(s).unwrap().window_size, v(188.0, 150.0));
    assert_rect(&l, fill, (0.0, 0.0, 188.0, 300.0));
    // A bar that doesn't show takes nothing, unless Always.
    tree[s].canvas_size = UDim2::from_scale(1.0, 1.0);
    assert_rect(&run(&tree), fill, (0.0, 0.0, 200.0, 150.0));
    tree[s].vertical_scroll_bar_inset = ScrollBarInset::Always;
    assert_rect(&run(&tree), fill, (0.0, 0.0, 188.0, 150.0));
}

#[test]
fn automatic_canvas_size_fits_a_list() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let s = add(&mut tree, Class::ScrollingFrame, gui, px(0.0, 0.0), px(200.0, 150.0));
    tree[s].canvas_size = UDim2::default();
    tree[s].automatic_canvas_size = AutomaticSize::Y;
    let list = tree.add(Class::UIListLayout, Some(s));
    tree[list].sort_order = SortOrder::LayoutOrder;
    let rows: Vec<NodeId> = (0..10)
        .map(|_| frame(&mut tree, s, px(0.0, 0.0), UDim2::new(1.0, 0.0, 0.0, 40.0)))
        .collect();
    tree[s].canvas_position = v(0.0, 100.0);
    let l = run(&tree);
    assert_eq!(l.get(s).unwrap().canvas_size, v(200.0, 400.0));
    assert_eq!(l.get(list).unwrap().content_size, v(200.0, 400.0));
    assert_rect(&l, rows[3], (0.0, 20.0, 200.0, 40.0));
    // The CanvasSize is still the least it can be.
    tree[s].canvas_size = UDim2::new(0.0, 0.0, 0.0, 1000.0);
    assert_eq!(run(&tree).get(s).unwrap().canvas_size, v(200.0, 1000.0));
}

#[test]
fn clips_descendants_intersects() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let outer = frame(&mut tree, gui, px(0.0, 0.0), px(100.0, 100.0));
    tree[outer].clips_descendants = true;
    let inner = frame(&mut tree, outer, px(50.0, 50.0), px(100.0, 100.0));
    tree[inner].clips_descendants = true;
    let leaf = frame(&mut tree, inner, px(-40.0, -40.0), px(200.0, 200.0));
    let l = run(&tree);
    assert!(l.get(outer).unwrap().clip.is_none());
    let c = l.get(leaf).unwrap().visible_rect();
    assert_eq!((c.min, c.max), (v(50.0, 50.0), v(100.0, 100.0)));
    assert_eq!(l.objects_at(v(75.0, 75.0)), vec![leaf, inner, outer]);
    assert_eq!(l.objects_at(v(20.0, 20.0)), vec![outer]);
    // A rotation switches clipping off for its subtree.
    tree[outer].rotation = 15.0;
    let l = run(&tree);
    assert!(l.get(inner).unwrap().clip.is_none());
    assert!(l.get(leaf).unwrap().clip.is_none());
    assert_eq!(l.get(leaf).unwrap().rotation, 15.0);
}

// ---------------------------------------------------------------- UIScale, UICorner, UIStroke

#[test]
fn ui_scale_multiplies_the_subtree() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let f = frame(&mut tree, gui, px(100.0, 100.0), px(100.0, 50.0));
    let s = tree.add(Class::UIScale, Some(f));
    tree[s].scale = 2.0;
    tree.add(Class::UICorner, Some(f));
    let child = frame(&mut tree, f, px(10.0, 10.0), px(20.0, 20.0));
    let stroke = tree.add(Class::UIStroke, Some(child));
    tree[stroke].thickness = 1.5;
    let centered = frame(&mut tree, gui, UDim2::from_scale(0.5, 0.5), px(100.0, 50.0));
    tree[centered].anchor_point = v(0.5, 0.5);
    let s2 = tree.add(Class::UIScale, Some(centered));
    tree[s2].scale = 2.0;
    let l = run(&tree);
    // Scaled about its anchor, the top-left here.
    assert_rect(&l, f, (100.0, 100.0, 200.0, 100.0));
    assert_rect(&l, child, (120.0, 120.0, 40.0, 40.0));
    assert_eq!(l.get(child).unwrap().scale, 2.0);
    assert_eq!(l.get(f).unwrap().corner_radius, 16.0);
    assert_eq!(l.get(child).unwrap().stroke, 3.0);
    // About its center, it stays centered.
    assert_rect(&l, centered, (300.0, 221.0, 200.0, 100.0));
}

#[test]
fn corner_radius() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let corners = [
        UDim::new(0.0, 8.0),
        UDim::new(0.5, 0.0),
        UDim::new(0.0, 30.0),
        UDim::new(0.25, 2.0),
    ];
    let ids: Vec<NodeId> = corners
        .iter()
        .map(|&radius| {
            let f = frame(&mut tree, gui, px(0.0, 0.0), px(100.0, 40.0));
            let c = tree.add(Class::UICorner, Some(f));
            tree[c].corner_radius = radius;
            f
        })
        .collect();
    let plain = frame(&mut tree, gui, px(0.0, 0.0), px(100.0, 40.0));
    let l = run(&tree);
    // Scale is of the shorter side, and it stops at half of it: a pill.
    let radius: Vec<f64> = ids.iter().map(|&id| l.get(id).unwrap().corner_radius).collect();
    assert_eq!(radius, [8.0, 20.0, 20.0, 12.0]);
    assert_eq!(l.get(plain).unwrap().corner_radius, 0.0);
}

// ---------------------------------------------------------------- draw order

#[test]
fn global_z_index_can_put_a_child_under_its_parent() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let a = frame(&mut tree, gui, px(0.0, 0.0), px(100.0, 100.0));
    tree[a].z_index = 2;
    let a1 = frame(&mut tree, a, px(0.0, 0.0), px(50.0, 50.0));
    let b = frame(&mut tree, gui, px(0.0, 0.0), px(100.0, 100.0));
    let l = run(&tree);
    // By ZIndex over the whole ScreenGui, ties in tree order.
    assert_eq!(l.draw_order, vec![a1, b, a]);
    assert_eq!(l.objects_at(v(10.0, 10.0)), vec![a, b, a1]);
    tree[gui].z_index_behavior = ZIndexBehavior::Sibling;
    let l = run(&tree);
    // Children over their parent; siblings by ZIndex.
    assert_eq!(l.draw_order, vec![b, a, a1]);
}

#[test]
fn sibling_z_index_ties_go_in_child_order() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    tree[gui].z_index_behavior = ZIndexBehavior::Sibling;
    let p = frame(&mut tree, gui, px(0.0, 0.0), px(100.0, 100.0));
    let x = frame(&mut tree, p, px(0.0, 0.0), px(10.0, 10.0));
    let y = frame(&mut tree, p, px(0.0, 0.0), px(10.0, 10.0));
    let z = frame(&mut tree, p, px(0.0, 0.0), px(10.0, 10.0));
    tree[x].z_index = 3;
    let y1 = frame(&mut tree, y, px(0.0, 0.0), px(5.0, 5.0));
    tree[y1].z_index = -10;
    let l = run(&tree);
    assert_eq!(l.draw_order, vec![p, y, y1, z, x]);
}

#[test]
fn display_order_and_hidden_things() {
    let mut tree = Tree::new();
    let top = screen(&mut tree);
    tree[top].display_order = 5;
    let over = frame(&mut tree, top, px(0.0, 0.0), px(100.0, 100.0));
    let bottom = screen(&mut tree);
    let under = frame(&mut tree, bottom, px(0.0, 0.0), px(100.0, 100.0));
    tree[under].z_index = 100;
    let hidden = frame(&mut tree, bottom, px(0.0, 0.0), px(100.0, 100.0));
    tree[hidden].visible = false;
    let in_hidden = frame(&mut tree, hidden, px(0.0, 0.0), px(10.0, 10.0));
    let off = screen(&mut tree);
    tree[off].enabled = false;
    let in_off = frame(&mut tree, off, px(0.0, 0.0), px(100.0, 100.0));
    let l = run(&tree);
    // A higher DisplayOrder draws over a lower one, whatever the ZIndex.
    assert_eq!(l.draw_order, vec![under, over]);
    assert!(!l.get(in_hidden).unwrap().shown);
    assert!(!l.get(in_off).unwrap().shown);
    // Still laid out: scripts read AbsoluteSize of hidden objects.
    assert_rect(&l, in_hidden, (0.0, 0.0, 10.0, 10.0));
    assert_eq!(l.objects_at(v(5.0, 5.0)), vec![over, under]);
}

#[test]
fn folders_pass_their_gui_objects_through() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let f = frame(&mut tree, gui, px(50.0, 50.0), px(200.0, 200.0));
    tree.add(Class::UIListLayout, Some(f));
    let folder = tree.add(Class::Folder, Some(f));
    let listed = frame(&mut tree, f, px(0.0, 0.0), px(20.0, 20.0));
    let filed = frame(&mut tree, folder, px(30.0, 40.0), px(20.0, 20.0));
    let l = run(&tree);
    // The list arranges only its direct siblings; the one in the Folder keeps its Position.
    assert_rect(&l, listed, (50.0, 50.0, 20.0, 20.0));
    assert_rect(&l, filed, (80.0, 90.0, 20.0, 20.0));
    assert!(l.get(folder).is_none());
    // In tree order, the Folder's contents come where the Folder is.
    assert_eq!(l.draw_order, vec![f, filed, listed]);
}

// ---------------------------------------------------------------- hit tests

#[test]
fn the_topmost_active_object_takes_the_click() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let low = add(&mut tree, Class::TextButton, gui, px(0.0, 0.0), px(100.0, 100.0));
    let high = add(&mut tree, Class::TextButton, gui, px(50.0, 50.0), px(100.0, 100.0));
    tree[low].z_index = 2;
    tree[high].z_index = 3;
    let l = run(&tree);
    assert_eq!(l.button_at(v(75.0, 75.0)), Some(high));
    assert_eq!(l.button_at(v(25.0, 25.0)), Some(low));
    assert_eq!(l.button_at(v(125.0, 125.0)), Some(high));
    assert_eq!(l.button_at(v(200.0, 200.0)), None);
    // Edges: the top-left is in, the bottom-right out.
    assert_eq!(l.button_at(v(0.0, 0.0)), Some(low));
    assert_eq!(l.button_at(v(150.0, 150.0)), None);
    assert_eq!(l.objects_at(v(75.0, 75.0)), vec![high, low]);
}

#[test]
fn inactive_frames_let_clicks_through_and_active_ones_sink_them() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let button = add(&mut tree, Class::TextButton, gui, px(0.0, 0.0), px(100.0, 100.0));
    let cover = frame(&mut tree, gui, px(0.0, 0.0), px(100.0, 100.0));
    tree[cover].z_index = 5;
    let label = add(&mut tree, Class::TextLabel, button, px(0.0, 0.0), px(100.0, 20.0));
    let l = run(&tree);
    assert_eq!(l.hit_test(v(10.0, 10.0)), Some(button));
    assert_eq!(l.button_at(v(10.0, 10.0)), Some(button));
    assert_eq!(l.objects_at(v(10.0, 10.0)), vec![cover, label, button]);
    // An Active frame on top takes it: no button gets the click.
    tree[cover].active = true;
    let l = run(&tree);
    assert_eq!(l.hit_test(v(10.0, 10.0)), Some(cover));
    assert_eq!(l.button_at(v(10.0, 10.0)), None);
    // A button that isn't Interactable still sinks the click but doesn't take it.
    tree[cover].active = false;
    tree[button].interactable = false;
    let l = run(&tree);
    assert_eq!(l.hit_test(v(10.0, 10.0)), Some(button));
    assert_eq!(l.button_at(v(10.0, 10.0)), None);
    // Hidden buttons aren't there at all.
    tree[button].interactable = true;
    tree[button].visible = false;
    let l = run(&tree);
    assert_eq!(l.hit_test(v(10.0, 10.0)), None);
    assert_eq!(l.objects_at(v(10.0, 10.0)), vec![cover]);
}

#[test]
fn hits_respect_clipping() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    let window = frame(&mut tree, gui, px(0.0, 0.0), px(100.0, 100.0));
    tree[window].clips_descendants = true;
    let button = add(&mut tree, Class::ImageButton, window, px(50.0, 50.0), px(100.0, 100.0));
    let l = run(&tree);
    assert_eq!(l.button_at(v(90.0, 90.0)), Some(button));
    // The part outside the clipping frame isn't there to click.
    assert_eq!(l.button_at(v(120.0, 120.0)), None);
    tree[window].clips_descendants = false;
    assert_eq!(run(&tree).button_at(v(120.0, 120.0)), Some(button));
}

#[test]
fn hits_use_gui_coordinates() {
    let mut tree = Tree::new();
    let gui = screen(&mut tree);
    tree[gui].ignore_gui_inset = true;
    let bar = add(
        &mut tree,
        Class::TextButton,
        gui,
        px(0.0, 0.0),
        UDim2::new(1.0, 0.0, 0.0, 58.0),
    );
    let l = run(&tree);
    // Over the top bar, from -58 to 0.
    assert_rect(&l, bar, (0.0, -58.0, 800.0, 58.0));
    assert_eq!(l.button_at(v(400.0, -30.0)), Some(bar));
    assert_eq!(l.button_at(v(400.0, 0.0)), None);
}
