//! Independent quality gates over the final SVG, not approved snapshot equality.
//!
//! This module exists only in test builds. Geometry is a logical layout envelope,
//! not a claim about platform-specific glyph rasterization or arbitrary SVG paths.

mod geometry;

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use roxmltree::{Document, Node};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{Engine, ProviderAsset, ProviderPack, scene};
use geometry::{Point, Rect};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const MINIMUM_PAINTED_FRAME_CLEARANCE: i64 = 16_000;
const MINIMUM_LABEL_FRAME_CLEARANCE: i64 = 8_000;

#[derive(Debug)]
struct TextBox {
    id: String,
    rect: Rect,
}

#[derive(Debug)]
struct Route {
    id: String,
    from: String,
    to: String,
    points: Vec<Point>,
    radius: i64,
}

#[derive(Debug)]
struct GroupFrame {
    id: String,
    rect: Rect,
    radius: i64,
}

#[derive(Debug)]
struct Drawing {
    bounds: Rect,
    texts: Vec<TextBox>,
    nodes: Vec<(String, Rect)>,
    frames: Vec<GroupFrame>,
    routes: Vec<Route>,
}

fn rect(value: scene::Rect) -> Rect {
    Rect {
        x: value.x,
        y: value.y,
        width: value.width,
        height: value.height,
    }
}

fn required<'a>(node: Node<'a, '_>, name: &str) -> Result<&'a str> {
    node.attribute(name)
        .ok_or_else(|| format!("missing {name} on {}", node.tag_name().name()).into())
}

fn pixel_value(value: &str) -> Result<i64> {
    // SVG uses pixel decimals; geometry stays in exact integer milli-pixels.
    // Reject SVG's wider numeric grammar rather than silently rounding it.
    let (negative, magnitude) = value
        .strip_prefix('-')
        .map_or((false, value), |magnitude| (true, magnitude));
    let (whole, fraction) = match magnitude.split_once('.') {
        Some((whole, fraction)) => {
            if fraction.is_empty()
                || fraction.len() > 3
                || !fraction.bytes().all(|digit| digit.is_ascii_digit())
            {
                return Err("unsupported pixel fraction".into());
            }
            (whole, fraction)
        }
        None => (magnitude, ""),
    };
    if whole.is_empty()
        || !whole.bytes().all(|digit| digit.is_ascii_digit())
        || (whole.len() > 1 && whole.starts_with('0'))
    {
        return Err("unsupported pixel integer".into());
    }
    let whole: i128 = whole.parse()?;
    let fraction = fraction
        .bytes()
        .chain(std::iter::repeat_n(b'0', 3 - fraction.len()))
        .fold(0_i128, |value, digit| value * 10 + i128::from(digit - b'0'));
    let magnitude = whole
        .checked_mul(1000)
        .and_then(|value| value.checked_add(fraction))
        .ok_or("pixel coordinate overflow")?;
    let value = if negative { -magnitude } else { magnitude };
    Ok(i64::try_from(value)?)
}

fn number(node: Node<'_, '_>, name: &str) -> Result<i64> {
    pixel_value(required(node, name)?)
}

fn rectangle(node: Node<'_, '_>) -> Result<Rect> {
    let value = Rect {
        x: number(node, "x")?,
        y: number(node, "y")?,
        width: number(node, "width")?,
        height: number(node, "height")?,
    };
    if value.width <= 0 || value.height <= 0 {
        return Err("non-positive rectangle".into());
    }
    Ok(value)
}

fn is_embedded(node: Node<'_, '_>) -> bool {
    node.ancestors()
        .skip(1)
        .any(|ancestor| ancestor.has_tag_name("svg") && ancestor.parent_element().is_some())
}

fn text_rectangle(
    node: Node<'_, '_>,
    prepared: &crate::PreparedScene<'_>,
    size: u32,
) -> Result<Rect> {
    let metrics = prepared.resources.metrics;
    let width = scene::text_width(node.text().ok_or("empty text")?, size, metrics);
    let ascent = (i64::from(metrics.ascent) * i64::from(size) + i64::from(metrics.units_per_em)
        - 1)
        / i64::from(metrics.units_per_em);
    let descent = (i64::from(-metrics.descent) * i64::from(size) + i64::from(metrics.units_per_em)
        - 1)
        / i64::from(metrics.units_per_em);
    let height =
        scene::line_height(size, &prepared.resources.theme.typography).max(ascent + descent);
    let offset = match node.attribute("text-anchor").unwrap_or("start") {
        "start" => 0,
        "middle" => width / 2,
        "end" => width,
        _ => return Err("unsupported text-anchor".into()),
    };
    let x = number(node, "x")?
        .checked_sub(offset)
        .ok_or("text coordinate overflow")?;
    let y = number(node, "y")?;
    let offset = match node.attribute("dominant-baseline").unwrap_or("alphabetic") {
        "alphabetic" => ascent + (height - ascent - descent) / 2,
        // A deterministic layout reservation, not the browser's exact glyph box:
        // SVG middle depends on the font's x-height, absent from current metrics.
        "middle" => height / 2,
        _ => return Err("unsupported dominant-baseline".into()),
    };
    let top = y.checked_sub(offset).ok_or("text coordinate overflow")?;
    Ok(Rect {
        x,
        y: top,
        width,
        height,
    })
}

fn point_list(value: &str) -> Result<Vec<Point>> {
    value
        .split_whitespace()
        .map(|pair| {
            let (x, y) = pair.split_once(',').ok_or("invalid point")?;
            Ok(Point {
                x: pixel_value(x)?,
                y: pixel_value(y)?,
            })
        })
        .collect()
}

fn point_envelope(points: &[Point]) -> Result<Rect> {
    let x = points
        .iter()
        .map(|point| point.x)
        .min()
        .ok_or("empty geometry")?;
    let y = points
        .iter()
        .map(|point| point.y)
        .min()
        .ok_or("empty geometry")?;
    let right = points
        .iter()
        .map(|point| point.x)
        .max()
        .ok_or("empty geometry")?;
    let bottom = points
        .iter()
        .map(|point| point.y)
        .max()
        .ok_or("empty geometry")?;
    let width = right.checked_sub(x).ok_or("geometry overflow")?;
    let height = bottom.checked_sub(y).ok_or("geometry overflow")?;
    if width <= 0 || height <= 0 {
        return Err("non-positive geometry envelope".into());
    }
    Ok(Rect {
        x,
        y,
        width,
        height,
    })
}

fn shape_envelope(node: Node<'_, '_>) -> Result<Rect> {
    match node.tag_name().name() {
        "rect" => rectangle(node),
        "circle" | "ellipse" => {
            let rx = number(
                node,
                if node.has_tag_name("circle") {
                    "r"
                } else {
                    "rx"
                },
            )?;
            let ry = number(
                node,
                if node.has_tag_name("circle") {
                    "r"
                } else {
                    "ry"
                },
            )?;
            if rx <= 0 || ry <= 0 {
                return Err("invalid shape radius".into());
            }
            Ok(Rect {
                x: number(node, "cx")?
                    .checked_sub(rx)
                    .ok_or("geometry overflow")?,
                y: number(node, "cy")?
                    .checked_sub(ry)
                    .ok_or("geometry overflow")?,
                width: rx.checked_mul(2).ok_or("geometry overflow")?,
                height: ry.checked_mul(2).ok_or("geometry overflow")?,
            })
        }
        "polygon" => point_envelope(&point_list(required(node, "points")?)?),
        "path" => {
            // Only the current absolute cylinder grammar is supported. Control
            // points bound each Bezier's convex hull, not its exact painted arc.
            let tokens = required(node, "d")?.split_whitespace().collect::<Vec<_>>();
            if tokens.len() != 22
                || tokens[0] != "M"
                || tokens[3] != "V"
                || tokens[5] != "C"
                || tokens[12] != "V"
                || tokens[14] != "C"
                || tokens[21] != "Z"
            {
                return Err("unsupported node path grammar".into());
            }
            let n = |index: usize| pixel_value(tokens[index]);
            let mut points = vec![Point { x: n(1)?, y: n(2)? }, Point { x: n(1)?, y: n(4)? }];
            for start in [6, 8, 10, 15, 17, 19] {
                points.push(Point {
                    x: n(start)?,
                    y: n(start + 1)?,
                });
            }
            points.push(Point {
                x: n(10)?,
                y: n(13)?,
            });
            point_envelope(&points)
        }
        _ => Err("unsupported node shape".into()),
    }
}

fn drawing_layer<'a, 'input>(root: Node<'a, 'input>, name: &str) -> Result<Node<'a, 'input>> {
    let layers = root
        .children()
        .filter(|node| node.attribute("data-stack-layer") == Some(name))
        .collect::<Vec<_>>();
    if layers.len() != 1 || !layers[0].has_tag_name("g") {
        return Err(format!("expected one visible {name} group layer").into());
    }
    Ok(layers[0])
}

fn viewport_dimension(node: Node<'_, '_>, name: &str) -> Result<i64> {
    let value = number(node, name)?;
    if value <= 0 {
        return Err("non-positive viewport".into());
    }
    Ok(value)
}

fn node_envelopes(
    root: Node<'_, '_>,
    prepared: &crate::PreparedScene<'_>,
) -> Result<Vec<(String, Rect)>> {
    let layer = drawing_layer(root, "nodes")?;
    let groups = layer
        .children()
        .filter(Node::is_element)
        .collect::<Vec<_>>();
    if groups.len() != prepared.scene.nodes.len() {
        return Err("node inventory drift".into());
    }
    let mut nodes = Vec::new();
    for (group, expected) in groups.into_iter().zip(&prepared.scene.nodes) {
        if !group.has_tag_name("g")
            || !group.has_attribute("data-node-kind")
            || required(group, "data-stack-id")? != expected.id
        {
            return Err("node inventory drift".into());
        }
        let shapes = group
            .children()
            .filter(Node::is_element)
            .filter(|node| !matches!(node.tag_name().name(), "title" | "text" | "svg"))
            .collect::<Vec<_>>();
        let shape = prepared
            .resources
            .node(&expected.id)
            .ok_or("missing resolved node")?
            .visual
            .shape;
        let expected_tags: &[&str] = match shape {
            stack_theme::NodeShape::RoundedRectangle | stack_theme::NodeShape::Capsule => &["rect"],
            stack_theme::NodeShape::Circle => &["circle"],
            stack_theme::NodeShape::Cylinder => &["path", "ellipse"],
            stack_theme::NodeShape::Hexagon => &["polygon"],
        };
        if shapes
            .iter()
            .map(|node| node.tag_name().name())
            .collect::<Vec<_>>()
            != expected_tags
        {
            return Err("node shape inventory drift".into());
        }
        let envelope = shape_envelope(shapes[0])?;
        if envelope != rect(expected.rect) {
            return Err("SVG/scene node envelope drift".into());
        }
        for detail in &shapes[1..] {
            if !geometry::contains(envelope, shape_envelope(*detail)?) {
                return Err("node decoration outside layout envelope".into());
            }
        }
        let icons = group
            .children()
            .filter(|node| node.has_tag_name("svg"))
            .collect::<Vec<_>>();
        if icons.len() != 1
            || required(icons[0], "data-icon-id")?
                != prepared
                    .resources
                    .node(&expected.id)
                    .ok_or("missing resolved node")?
                    .icon_id
            || !geometry::contains(envelope, rectangle(icons[0])?)
        {
            return Err("unverified icon SVG viewport".into());
        }
        nodes.push((expected.id.clone(), envelope));
    }
    Ok(nodes)
}

fn group_frames(
    root: Node<'_, '_>,
    prepared: &crate::PreparedScene<'_>,
) -> Result<Vec<GroupFrame>> {
    let groups = drawing_layer(root, "groups")?
        .children()
        .filter(Node::is_element)
        .collect::<Vec<_>>();
    if groups.len() != prepared.scene.groups.len() {
        return Err("group frame inventory drift".into());
    }
    let mut frames = Vec::new();
    for (group, expected) in groups.into_iter().zip(&prepared.scene.groups) {
        if !group.has_tag_name("g")
            || group.has_attribute("data-node-kind")
            || required(group, "data-stack-id")? != expected.id
        {
            return Err("group frame owner drift".into());
        }
        let elements = group
            .children()
            .filter(Node::is_element)
            .collect::<Vec<_>>();
        if elements
            .iter()
            .map(|element| element.tag_name().name())
            .collect::<Vec<_>>()
            != ["title", "rect", "text"]
        {
            return Err("group frame shape inventory drift".into());
        }
        let element = elements[1];
        let envelope = rectangle(element)?;
        if envelope != rect(expected.rect) {
            return Err("SVG/scene group frame drift".into());
        }
        if matches!(required(element, "stroke")?, "none" | "transparent")
            || element.ancestors().any(|ancestor| {
                [
                    "stroke-opacity",
                    "stroke-dasharray",
                    "stroke-dashoffset",
                    "vector-effect",
                ]
                .iter()
                .any(|attribute| ancestor.has_attribute(*attribute))
            })
        {
            return Err("unsupported group frame stroke".into());
        }
        let width = number(element, "stroke-width")?;
        if width <= 0 {
            return Err("non-positive group frame stroke width".into());
        }
        frames.push(GroupFrame {
            id: expected.id.clone(),
            rect: envelope,
            radius: width / 2 + width % 2,
        });
    }
    Ok(frames)
}

fn read_drawing(
    svg: &str,
    prepared: &crate::PreparedScene<'_>,
    diagram: &stack_compiler::ir::Diagram,
) -> Result<Drawing> {
    let document = Document::parse(svg)?;
    if document.descendants().any(|node| node.is_pi()) {
        return Err("unsupported SVG processing instruction".into());
    }
    let root = document.root_element();
    if !root.has_tag_name(("http://www.w3.org/2000/svg", "svg")) {
        return Err("expected SVG root".into());
    }
    let view_box = required(root, "viewBox")?
        .split_whitespace()
        .map(pixel_value)
        .collect::<Result<Vec<_>>>()?;
    let expected = prepared.scene.bounds;
    if view_box != [expected.x, expected.y, expected.width, expected.height] {
        return Err("SVG/scene bounds drift".into());
    }
    if viewport_dimension(root, "width")? != expected.width
        || viewport_dimension(root, "height")? != expected.height
    {
        return Err("SVG/scene viewport drift".into());
    }
    let nodes_layer = drawing_layer(root, "nodes")?;
    let groups_layer = drawing_layer(root, "groups")?;
    let edge_layer = drawing_layer(root, "edges")?;
    let labels_layer = drawing_layer(root, "edge-labels")?;
    for tag in ["title", "desc", "metadata", "rect", "defs", "text"] {
        if root
            .children()
            .filter(|node| node.has_tag_name(tag))
            .count()
            != 1
        {
            return Err(format!("unexpected root {tag} inventory").into());
        }
    }
    // The owned drawing uses untransformed pixel decimals. A new transform or
    // text positioning mode must be supported, not silently skipped.
    for node in root
        .descendants()
        .filter(Node::is_element)
        .filter(|node| !is_embedded(*node))
    {
        if node.tag_name().namespace() != Some("http://www.w3.org/2000/svg") {
            return Err("unexpected drawing namespace".into());
        }
        if node.has_attribute("data-stack-layer")
            && (node.parent_element() != Some(root)
                || ![nodes_layer, groups_layer, edge_layer, labels_layer].contains(&node))
        {
            return Err("unexpected drawing layer".into());
        }
        let parent = node.parent_element();
        let in_group = |layer| {
            parent.is_some_and(|parent| {
                parent.has_tag_name("g") && parent.parent_element() == Some(layer)
            })
        };
        let in_root = parent == Some(root);
        let in_marker = parent.is_some_and(|parent| {
            parent.has_tag_name("marker")
                && parent.parent_element().is_some_and(|defs| {
                    defs.has_tag_name("defs") && defs.parent_element() == Some(root)
                })
        });
        let positioned = match node.tag_name().name() {
            "svg" => node == root || (in_group(nodes_layer) && node.has_attribute("data-icon-id")),
            "g" => {
                [nodes_layer, groups_layer, edge_layer, labels_layer].contains(&node)
                    || parent.is_some_and(|parent| {
                        [nodes_layer, groups_layer, edge_layer, labels_layer].contains(&parent)
                    })
            }
            "text" => {
                in_root || in_group(nodes_layer) || in_group(groups_layer) || in_group(labels_layer)
            }
            "title" => {
                in_root || in_group(nodes_layer) || in_group(groups_layer) || in_group(edge_layer)
            }
            "desc" | "metadata" | "defs" => in_root,
            "marker" => parent.is_some_and(|parent| {
                parent.has_tag_name("defs") && parent.parent_element() == Some(root)
            }),
            "rect" => {
                in_root || in_group(nodes_layer) || in_group(groups_layer) || in_group(labels_layer)
            }
            "circle" | "ellipse" | "polygon" => in_group(nodes_layer),
            "path" => in_group(nodes_layer) || in_marker,
            "polyline" => in_group(edge_layer),
            _ => false,
        };
        if !positioned {
            return Err(format!(
                "unsupported drawing element placement: {}",
                node.tag_name().name()
            )
            .into());
        }
        for attribute in [
            "transform",
            "style",
            "clip-path",
            "mask",
            "display",
            "visibility",
            "opacity",
            "filter",
            "dx",
            "dy",
            "rotate",
            "textLength",
            "lengthAdjust",
            "alignment-baseline",
            "baseline-shift",
            "writing-mode",
            "direction",
            "letter-spacing",
            "word-spacing",
            "font-style",
            "font-stretch",
        ] {
            if node.has_attribute(attribute) {
                return Err(format!("unsupported drawing attribute {attribute}").into());
            }
        }
        if !node.has_tag_name("text")
            && [
                "dominant-baseline",
                "text-anchor",
                "font-family",
                "font-size",
                "font-weight",
            ]
            .iter()
            .any(|attribute| node.has_attribute(*attribute))
        {
            return Err("unsupported inherited text properties".into());
        }
    }
    let mut texts = Vec::new();
    let mut owners = BTreeSet::new();
    let mut edge_label_index = 0;
    for node in root
        .descendants()
        .filter(|node| node.has_tag_name("text") && !is_embedded(*node))
    {
        if node.children().count() != 1
            || !node.first_child().is_some_and(|child| child.is_text())
            || ["dx", "dy", "rotate", "textLength", "lengthAdjust"]
                .iter()
                .any(|attribute| node.has_attribute(*attribute))
        {
            return Err("unsupported text positioning".into());
        }
        let parent = node.parent_element().ok_or("text has no parent")?;
        let value = node.text().ok_or("empty semantic text")?;
        if required(node, "font-family")? != prepared.resources.metrics.family {
            return Err("text uses unmeasured font family".into());
        }
        let size = u32::try_from(number(node, "font-size")?)?;
        let text_bounds = text_rectangle(node, prepared, size)?;
        let (id, owner, bounds) = if parent.has_attribute("data-edge-label") {
            if !parent.has_tag_name("g") || parent.parent_element() != Some(labels_layer) {
                return Err("edge label outside visible label layer".into());
            }
            let expected_label = diagram
                .edges
                .iter()
                .filter_map(|edge| edge.label.as_deref())
                .nth(edge_label_index)
                .ok_or("unexpected edge label")?;
            if value != expected_label || required(parent, "data-edge-label")? != expected_label {
                return Err("edge label inventory drift".into());
            }
            if size != prepared.resources.theme.typography.edge_label_size_milli_px {
                return Err("edge label font size drift".into());
            }
            let backgrounds = parent
                .children()
                .filter(|child| child.has_tag_name("rect"))
                .collect::<Vec<_>>();
            if backgrounds.len() != 1 {
                return Err("edge label background inventory drift".into());
            }
            let background = rectangle(backgrounds[0])?;
            if !geometry::contains(background, text_bounds) {
                return Err("edge label text outside its background".into());
            }
            let id = format!("edge-label:{edge_label_index}:{value}");
            let owner = format!("edge-label:{edge_label_index}");
            edge_label_index += 1;
            (id, owner, background)
        } else {
            let (owner, expected_text, expected_size) = if let Some(id) =
                parent.attribute("data-stack-id")
            {
                if parent.has_attribute("data-node-kind") {
                    if !parent.has_tag_name("g") || parent.parent_element() != Some(nodes_layer) {
                        return Err("node text outside visible node layer".into());
                    }
                    let authored = diagram
                        .nodes
                        .iter()
                        .find(|candidate| candidate.id == id)
                        .ok_or("unknown text node")?;
                    let ordinal = parent
                        .children()
                        .filter(|child| child.has_tag_name("text"))
                        .position(|child| child == node)
                        .ok_or("text position missing")?;
                    match ordinal {
                        0 => (
                            format!("node:{id}:label"),
                            authored.label.as_str(),
                            prepared.resources.theme.typography.node_label_size_milli_px,
                        ),
                        1 => (
                            format!("node:{id}:detail"),
                            authored.detail.as_deref().ok_or("unexpected node detail")?,
                            prepared
                                .resources
                                .theme
                                .typography
                                .node_detail_size_milli_px,
                        ),
                        _ => return Err("unexpected node text".into()),
                    }
                } else {
                    if !parent.has_tag_name("g") || parent.parent_element() != Some(groups_layer) {
                        return Err("group title outside visible group layer".into());
                    }
                    let group = diagram
                        .groups
                        .iter()
                        .find(|candidate| candidate.id == id)
                        .ok_or("unknown group title")?;
                    (
                        format!("group:{id}:title"),
                        group.label.as_str(),
                        prepared
                            .resources
                            .theme
                            .typography
                            .group_label_size_milli_px,
                    )
                }
            } else if parent == root {
                (
                    "diagram:title".to_owned(),
                    diagram.title.as_str(),
                    prepared
                        .resources
                        .theme
                        .typography
                        .group_label_size_milli_px,
                )
            } else {
                return Err("unknown semantic text owner".into());
            };
            if value != expected_text || size != expected_size {
                return Err("semantic text content or size drift".into());
            }
            let id = format!(
                "text:{}:{value}",
                parent.attribute("data-stack-id").unwrap_or("diagram-title")
            );
            (id, owner, text_bounds)
        };
        if !owners.insert(owner) {
            return Err("duplicate semantic text owner".into());
        }
        texts.push(TextBox { id, rect: bounds });
    }
    let expected_texts = 1
        + diagram.groups.len()
        + diagram.nodes.len()
        + diagram
            .nodes
            .iter()
            .filter(|node| node.detail.is_some())
            .count()
        + diagram
            .edges
            .iter()
            .filter(|edge| edge.label.is_some())
            .count();
    if texts.len() != expected_texts
        || edge_label_index
            != diagram
                .edges
                .iter()
                .filter(|edge| edge.label.is_some())
                .count()
    {
        return Err("semantic text inventory drift".into());
    }
    let nodes = node_envelopes(root, prepared)?;
    let frames = group_frames(root, prepared)?;
    let mut routes = Vec::new();
    for (index, group) in edge_layer.children().filter(Node::is_element).enumerate() {
        if !group.has_tag_name("g") {
            return Err("edge outside visible group".into());
        }
        let expected = prepared.scene.edges.get(index).ok_or("unexpected edge")?;
        let lines = group
            .children()
            .filter(|node| node.has_tag_name("polyline"))
            .collect::<Vec<_>>();
        if lines.len() != 1 {
            return Err("unsupported edge geometry: expected one polyline".into());
        }
        let line = lines[0];
        let points = point_list(required(line, "points")?)?;
        let expected_points = expected
            .path
            .iter()
            .map(|point| Point {
                x: point.x,
                y: point.y,
            })
            .collect::<Vec<_>>();
        if points != expected_points {
            return Err("SVG/scene route drift".into());
        }
        if points.len() < 2
            || points.windows(2).any(|pair| {
                pair[0] == pair[1] || (pair[0].x != pair[1].x && pair[0].y != pair[1].y)
            })
        {
            return Err("unsupported non-orthogonal or degenerate route".into());
        }
        let width = number(line, "stroke-width")?;
        if width <= 0 {
            return Err("invalid stroke width".into());
        }
        routes.push(Route {
            id: format!("edge:{index}:{}->{}", expected.from, expected.to),
            from: expected.from.clone(),
            to: expected.to.clone(),
            points,
            radius: width / 2 + width % 2,
        });
    }
    if routes.len() != diagram.edges.len() {
        return Err("edge inventory drift".into());
    }
    Ok(Drawing {
        bounds: rect(expected),
        texts,
        nodes,
        frames,
        routes,
    })
}

fn rectangle_json(rect: Rect) -> Value {
    json!({"x":rect.x,"y":rect.y,"width":rect.width,"height":rect.height})
}

fn composition_metrics(drawing: &Drawing) -> Value {
    let mut length = 0_i64;
    let mut bends = 0;
    for route in &drawing.routes {
        for pair in route.points.windows(2) {
            length += (pair[0].x - pair[1].x).abs() + (pair[0].y - pair[1].y).abs();
        }
        for triple in route.points.windows(3) {
            if (triple[0].x == triple[1].x) != (triple[1].x == triple[2].x) {
                bends += 1;
            }
        }
    }
    json!({"widthMilliPx":drawing.bounds.width,"heightMilliPx":drawing.bounds.height,"routeLengthMilliPx":length,"bends":bends,"nodes":drawing.nodes.len(),"textBoxes":drawing.texts.len(),"edges":drawing.routes.len()})
}

fn text_violations(drawing: &Drawing) -> Vec<Value> {
    let mut violations = Vec::new();
    for (index, text) in drawing.texts.iter().enumerate() {
        if !geometry::contains(drawing.bounds, text.rect) {
            violations.push(json!({"kind":"text-out-of-bounds","entity":text.id,"rect":rectangle_json(text.rect)}));
        }
        for other in &drawing.texts[index + 1..] {
            if geometry::overlaps(text.rect, other.rect) {
                violations.push(json!({"kind":"text-overlap","entities":[text.id,other.id],"rectangles":[rectangle_json(text.rect),rectangle_json(other.rect)]}));
            }
        }
    }
    violations
}

fn edge_violations(drawing: &Drawing) -> Result<Vec<Value>> {
    let mut violations = Vec::new();
    for route in &drawing.routes {
        for text in &drawing.texts {
            for pair in route.points.windows(2) {
                if geometry::segment_hits_rect(pair[0], pair[1], text.rect, route.radius)? {
                    violations.push(json!({"kind":"edge-text-collision","entities":[route.id,text.id],"rect":rectangle_json(text.rect)}));
                    break;
                }
            }
        }
        for (id, bounds) in &drawing.nodes {
            for (index, pair) in route.points.windows(2).enumerate() {
                let allow_start = index == 0 && id == &route.from;
                let allow_end = index + 2 == route.points.len() && id == &route.to;
                if geometry::segment_hits_node(
                    pair[0],
                    pair[1],
                    *bounds,
                    route.radius,
                    allow_start,
                    allow_end,
                )? {
                    violations.push(json!({"kind":"edge-node-collision","entities":[route.id,id],"rect":rectangle_json(*bounds)}));
                    break;
                }
            }
        }
    }
    Ok(violations)
}

fn frame_violations(drawing: &Drawing) -> Result<Vec<Value>> {
    let mut violations = Vec::new();
    for route in &drawing.routes {
        for frame in &drawing.frames {
            let clearance = MINIMUM_PAINTED_FRAME_CLEARANCE
                .checked_add(route.radius)
                .and_then(|value| value.checked_add(frame.radius))
                .ok_or("frame clearance overflow")?;
            for (index, pair) in route.points.windows(2).enumerate() {
                if geometry::parallel_frame_contact(pair[0], pair[1], frame.rect, clearance)? {
                    violations.push(json!({
                        "kind":"edge-group-frame-clearance",
                        "entities":[route.id,frame.id],
                        "segmentIndex":index,
                        "segment":[{"x":pair[0].x,"y":pair[0].y},{"x":pair[1].x,"y":pair[1].y}],
                        "rect":rectangle_json(frame.rect),
                        "requiredCenterlineClearanceMilliPx":clearance,
                        "routeStrokeRadiusMilliPx":route.radius,
                        "frameStrokeRadiusMilliPx":frame.radius
                    }));
                }
            }
        }
    }
    for label in drawing
        .texts
        .iter()
        .filter(|text| text.id.starts_with("edge-label:"))
    {
        for frame in &drawing.frames {
            let clearance = MINIMUM_LABEL_FRAME_CLEARANCE
                .checked_add(frame.radius)
                .ok_or("label frame clearance overflow")?;
            if geometry::label_frame_contact(label.rect, frame.rect, clearance)? {
                violations.push(json!({
                    "kind":"label-group-frame-clearance",
                    "entities":[label.id,frame.id],
                    "labelRect":rectangle_json(label.rect),
                    "frameRect":rectangle_json(frame.rect),
                    "requiredCenterlineClearanceMilliPx":clearance,
                    "frameStrokeRadiusMilliPx":frame.radius
                }));
            }
        }
    }
    Ok(violations)
}

#[derive(Deserialize)]
struct Catalog {
    cases: Vec<Case>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    id: String,
    source: String,
    provider_fixture: Option<String>,
}
#[derive(Deserialize)]
struct PackInput {
    manifest: stack_theme::ProviderPack,
    assets: Vec<AssetInput>,
}
#[derive(Deserialize)]
struct AssetInput {
    path: String,
    svg: String,
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run_corpus(gate: &str) -> Result<()> {
    let root = root();
    let catalog: Catalog =
        serde_json::from_slice(&fs::read(root.join("layout-corpus/catalog.json"))?)?;
    if catalog.cases.len() != 7 {
        return Err("review corpus inventory changed; update quality coverage explicitly".into());
    }
    let output_dir = root.join("target/layout-quality").join(gate);
    fs::create_dir_all(&output_dir)?;
    let mut results = Vec::new();
    let mut count = 0;
    for case in catalog.cases {
        let source = fs::read(root.join("layout-corpus").join(case.source))?;
        let mut packs = Vec::new();
        if let Some(provider) = case.provider_fixture {
            let inputs: Vec<PackInput> =
                serde_json::from_slice(&fs::read(root.join("layout-corpus").join(provider))?)?;
            for input in inputs {
                packs.push(ProviderPack::new(
                    input.manifest,
                    input
                        .assets
                        .into_iter()
                        .map(|asset| ProviderAsset::new(asset.path, asset.svg))
                        .collect(),
                )?);
            }
        }
        let engine = Engine::with_provider_packs(&packs)?;
        let output = engine.render(&source)?;
        if !output.diagnostics.is_empty() {
            return Err(format!("{}: unexpected diagnostics", case.id).into());
        }
        let svg = output.svg.ok_or("render returned no SVG")?;
        let compiled = stack_compiler::compile_bytes_with_source_map(&source);
        let diagram = compiled.diagram.as_ref().ok_or("no compiled diagram")?;
        let prepared = engine.prepare_scene(
            diagram,
            compiled.source_map.as_ref().ok_or("no source map")?,
        )?;
        let drawing = read_drawing(&svg, &prepared, diagram)?;
        let violations = match gate {
            "text" => text_violations(&drawing),
            "edge" => edge_violations(&drawing)?,
            "frame" => frame_violations(&drawing)?,
            _ => return Err("unknown layout quality gate".into()),
        };
        count += violations.len();
        fs::write(output_dir.join(format!("{}.svg", case.id)), svg)?;
        eprintln!("{}: {} {gate} violation(s)", case.id, violations.len());
        results.push(json!({"id":case.id,"violations":violations,"composition":composition_metrics(&drawing)}));
    }
    let mut report = json!({"schemaVersion":"1.0","gate":gate,"units":"1/1000 CSS px","engineVersion":crate::ENGINE_VERSION,"limitations":["logical text envelopes, not raster glyph bounds","middle-baseline reservation is centered line height, not font x-height measurement","node SVG and Bezier control-point envelopes, not exact painted shape outlines","arrowheads and group frame collisions are not measured","global composition metrics are not a beauty score"],"violations":count,"cases":results});
    if gate == "frame" {
        report["minimumPaintedClearanceMilliPx"] = json!(MINIMUM_PAINTED_FRAME_CLEARANCE);
        report["minimumLabelPaintedFrameClearanceMilliPx"] = json!(MINIMUM_LABEL_FRAME_CLEARANCE);
        report["limitations"] = json!([
            "finite group frame centerline rectangles with conservative rounded-corner clearance",
            "perpendicular crossings away from corners are allowed",
            "arrowheads are not measured",
            "global composition metrics are not a beauty score"
        ]);
    }
    fs::write(
        output_dir.join("report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    if count > 0 {
        return Err(format!(
            "{count} {gate} quality violations; inspect {}",
            output_dir.display()
        )
        .into());
    }
    Ok(())
}

#[test]
fn corpus_text_quality() -> Result<()> {
    run_corpus("text")
}

#[test]
fn corpus_edge_quality() -> Result<()> {
    run_corpus("edge")
}

#[test]
fn corpus_frame_quality() -> Result<()> {
    run_corpus("frame")
}

#[test]
fn detector_flags_text_collision_and_clipping_without_requiring_a_snapshot() {
    let drawing = Drawing {
        bounds: Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        },
        texts: vec![
            TextBox {
                id: "title".into(),
                rect: Rect {
                    x: 0,
                    y: 0,
                    width: 40,
                    height: 20,
                },
            },
            TextBox {
                id: "label".into(),
                rect: Rect {
                    x: -5,
                    y: 10,
                    width: 30,
                    height: 20,
                },
            },
        ],
        nodes: vec![],
        frames: vec![],
        routes: vec![],
    };
    let violations = text_violations(&drawing);
    assert_eq!(violations.len(), 2);
    assert!(
        violations
            .iter()
            .any(|value| value["kind"] == "text-overlap")
    );
    assert!(
        violations
            .iter()
            .any(|value| value["kind"] == "text-out-of-bounds")
    );
}

#[test]
fn detector_allows_separated_text_inside_the_canvas() {
    let drawing = Drawing {
        bounds: Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        },
        texts: vec![
            TextBox {
                id: "left".into(),
                rect: Rect {
                    x: 0,
                    y: 0,
                    width: 40,
                    height: 20,
                },
            },
            TextBox {
                id: "right".into(),
                rect: Rect {
                    x: 60,
                    y: 0,
                    width: 40,
                    height: 20,
                },
            },
        ],
        nodes: vec![],
        frames: vec![],
        routes: vec![],
    };
    assert!(text_violations(&drawing).is_empty());
}

#[test]
fn metrics_do_not_trade_collisions_for_a_smaller_drawing() {
    let drawing = Drawing {
        bounds: Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        },
        texts: vec![],
        nodes: vec![],
        frames: vec![],
        routes: vec![Route {
            id: "edge:0".into(),
            from: "a".into(),
            to: "b".into(),
            radius: 1,
            points: vec![
                Point { x: 10, y: 10 },
                Point { x: 40, y: 10 },
                Point { x: 40, y: 20 },
                Point { x: 40, y: 40 },
            ],
        }],
    };
    let metrics = composition_metrics(&drawing);
    assert_eq!(metrics["routeLengthMilliPx"], 60);
    assert_eq!(metrics["bends"], 1);
    assert!(metrics.get("beautyScore").is_none());
}

#[test]
fn detector_does_not_exempt_an_edges_own_label() -> Result<()> {
    let drawing = Drawing {
        bounds: Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        },
        texts: vec![TextBox {
            id: "edge-label:0:Request".into(),
            rect: Rect {
                x: 40,
                y: 40,
                width: 20,
                height: 20,
            },
        }],
        nodes: vec![],
        frames: vec![],
        routes: vec![Route {
            id: "edge:0".into(),
            from: "a".into(),
            to: "b".into(),
            points: vec![Point { x: 0, y: 50 }, Point { x: 100, y: 50 }],
            radius: 1,
        }],
    };
    let violations = edge_violations(&drawing)?;
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0]["kind"], "edge-text-collision");
    Ok(())
}

#[test]
fn detector_allows_only_terminal_node_contact() -> Result<()> {
    let mut drawing = Drawing {
        bounds: Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        },
        texts: vec![],
        nodes: vec![
            (
                "a".into(),
                Rect {
                    x: 0,
                    y: 0,
                    width: 20,
                    height: 20,
                },
            ),
            (
                "b".into(),
                Rect {
                    x: 80,
                    y: 0,
                    width: 20,
                    height: 20,
                },
            ),
        ],
        frames: vec![],
        routes: vec![Route {
            id: "edge:0".into(),
            from: "a".into(),
            to: "b".into(),
            points: vec![Point { x: 20, y: 10 }, Point { x: 80, y: 10 }],
            radius: 1,
        }],
    };
    assert!(edge_violations(&drawing)?.is_empty());
    drawing.nodes.push((
        "unrelated".into(),
        Rect {
            x: 40,
            y: 0,
            width: 20,
            height: 20,
        },
    ));
    let violations = edge_violations(&drawing)?;
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0]["entities"][1], "unrelated");
    // Re-entering the source on a later segment must not be exempted.
    drawing.routes[0].points = vec![
        Point { x: 20, y: 10 },
        Point { x: 30, y: 10 },
        Point { x: 30, y: 30 },
        Point { x: 10, y: 30 },
        Point { x: 10, y: 10 },
        Point { x: 80, y: 10 },
    ];
    assert!(
        edge_violations(&drawing)?
            .iter()
            .any(|violation| violation["entities"][1] == "a")
    );
    Ok(())
}

#[test]
fn parser_rejects_unmeasured_geometry_and_dropped_content() -> Result<()> {
    let source =
        b"stack 1.0 diagram \"Example\" { node a \"A\" node b \"B\" edge a -> b \"Request\" }";
    let engine = Engine::bundled();
    let output = engine.render(source)?;
    let svg = output.svg.ok_or("missing SVG")?;
    let compiled = stack_compiler::compile_bytes_with_source_map(source);
    let diagram = compiled.diagram.as_ref().ok_or("missing diagram")?;
    let prepared = engine.prepare_scene(
        diagram,
        compiled.source_map.as_ref().ok_or("missing source map")?,
    )?;
    assert!(read_drawing(&svg, &prepared, diagram).is_ok());
    for mutant in [
        svg.replacen("<svg ", "<svg transform=\"scale(2)\" ", 1),
        svg.replacen(">Example</text>", ">Wrong</text>", 1),
        svg.replacen("<polyline ", "<path ", 1),
        svg.replacen("<text ", "<text dx=\"0.1\" ", 1),
        svg.replacen("data-node-kind=", "removed-node-kind=", 1),
    ] {
        assert_ne!(mutant, svg, "mutation must exercise the checker");
        assert!(read_drawing(&mutant, &prepared, diagram).is_err());
    }
    Ok(())
}

#[test]
fn parser_rejects_reviewed_false_green_mutations() -> Result<()> {
    let source =
        b"stack 1.0 diagram \"Example\" { node a \"A\" node b \"B\" edge a -> b \"Request\" }";
    let engine = Engine::bundled();
    let svg = engine.render(source)?.svg.ok_or("missing SVG")?;
    let compiled = stack_compiler::compile_bytes_with_source_map(source);
    let diagram = compiled.diagram.as_ref().ok_or("missing diagram")?;
    let prepared = engine.prepare_scene(
        diagram,
        compiled.source_map.as_ref().ok_or("missing source map")?,
    )?;
    let document = Document::parse(&svg)?;
    let title = document
        .root_element()
        .children()
        .find(|node| node.has_tag_name("text"))
        .ok_or("missing title")?;
    let owner = document
        .descendants()
        .find(|node| node.attribute("data-stack-id") == Some("a"))
        .ok_or("missing node")?;
    let label = owner
        .children()
        .find(|node| node.has_tag_name("text"))
        .ok_or("missing node label")?;
    let shape = owner
        .children()
        .find(|node| node.has_tag_name("rect"))
        .ok_or("missing node shape")?;
    let edge_text = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node
                    .parent_element()
                    .is_some_and(|parent| parent.has_attribute("data-edge-label"))
        })
        .ok_or("missing edge text")?;
    let replace = |node: Node<'_, '_>, replacement: &str| {
        let mut mutant = svg.clone();
        mutant.replace_range(node.range(), replacement);
        mutant
    };
    let edge_markup = &svg[edge_text.range()];
    let moved_edge = edge_markup.replacen(
        &format!("x=\"{}\"", required(edge_text, "x")?),
        "x=\"-999.999\"",
        1,
    );
    let oversized_edge = edge_markup.replacen(
        &format!("font-size=\"{}\"", required(edge_text, "font-size")?),
        "font-size=\"900\"",
        1,
    );
    let moved_shape = svg[shape.range()].replacen(
        &format!("x=\"{}\"", required(shape, "x")?),
        "x=\"-999.999\"",
        1,
    );
    let mut duplicated_title = replace(label, "");
    let end = duplicated_title.rfind("</svg>").ok_or("missing root end")?;
    duplicated_title.insert_str(end, &svg[title.range()]);
    let hide_group = |group: Node<'_, '_>| -> Result<String> {
        let mut hidden = svg[group.range()].replacen("<g ", "<defs ", 1);
        let close = hidden.rfind("</g>").ok_or("missing group end")?;
        hidden.replace_range(close..close + 4, "</defs>");
        Ok(replace(group, &hidden))
    };
    let layer = |name: &str| -> Result<Node<'_, '_>> {
        document
            .root_element()
            .children()
            .find(|node| node.attribute("data-stack-layer") == Some(name))
            .ok_or_else(|| "missing layer".into())
    };
    let hidden_edge_group = hide_group(
        layer("edges")?
            .children()
            .find(Node::is_element)
            .ok_or("missing edge group")?,
    )?;
    let mutants = [
        (
            "edge text moved outside its background",
            replace(edge_text, &moved_edge),
        ),
        ("edge text enlarged", replace(edge_text, &oversized_edge)),
        ("node shape moved", replace(shape, &moved_shape)),
        ("node shape removed", replace(shape, "")),
        ("node label replaced by duplicate title", duplicated_title),
        (
            "unsupported baseline",
            svg.replacen(
                "dominant-baseline=\"middle\"",
                "dominant-baseline=\"text-before-edge\"",
                1,
            ),
        ),
        (
            "stylesheet hides text",
            svg.replacen("<defs>", "<style>text { display: none }</style><defs>", 1),
        ),
        (
            "inherited text rotation",
            svg.replacen("<svg ", "<svg rotate=\"90\" ", 1),
        ),
        (
            "node layer hidden in definitions",
            hide_group(layer("nodes")?)?,
        ),
        (
            "edge layer hidden in definitions",
            hide_group(layer("edges")?)?,
        ),
        ("edge group hidden in definitions", hidden_edge_group),
        (
            "label owner hidden in definitions",
            hide_group(edge_text.parent_element().ok_or("missing owner")?)?,
        ),
        (
            "zero root width",
            svg.replacen(
                &format!("width=\"{}\"", required(document.root_element(), "width")?),
                "width=\"0\"",
                1,
            ),
        ),
        (
            "zero root height",
            svg.replacen(
                &format!(
                    "height=\"{}\"",
                    required(document.root_element(), "height")?
                ),
                "height=\"0\"",
                1,
            ),
        ),
        ("extra root route", svg.replacen("</metadata>", "</metadata><polyline points=\"0,0 999.999,0\" stroke=\"black\" stroke-width=\"1.5\"/>", 1)),
        ("unmeasured trailing text", svg.replacen(">Example</text>", ">Example<!-- split -->UNMEASURED</text>", 1)),
        ("unverified nested SVG", replace(owner, &svg[owner.range()].replacen("</title>", "</title><svg><text x=\"0\" y=\"0\">UNMEASURED</text></svg>", 1))),
        ("document stylesheet", svg.replacen("<svg ", "<?xml-stylesheet type=\"text/css\" href=\"data:text/css,text%7Bdisplay%3Anone%7D\"?><svg ", 1)),
    ];
    let mut accepted = Vec::new();
    for (name, mutant) in mutants {
        assert_ne!(mutant, svg, "mutation must exercise {name}");
        if read_drawing(&mutant, &prepared, diagram).is_ok() {
            accepted.push(name);
        }
    }
    assert!(
        accepted.is_empty(),
        "unmeasured mutations accepted: {accepted:?}"
    );
    Ok(())
}

#[test]
fn parser_reserves_logical_middle_baseline_text_boxes() -> Result<()> {
    let source = b"stack 1.0 diagram \"Example\" { node a \"A\" }";
    let engine = Engine::bundled();
    let svg = engine.render(source)?.svg.ok_or("missing SVG")?;
    let compiled = stack_compiler::compile_bytes_with_source_map(source);
    let diagram = compiled.diagram.as_ref().ok_or("missing diagram")?;
    let prepared = engine.prepare_scene(
        diagram,
        compiled.source_map.as_ref().ok_or("missing source map")?,
    )?;
    let drawing = read_drawing(&svg, &prepared, diagram)?;
    let text = drawing
        .texts
        .iter()
        .find(|text| text.id == "text:a:A")
        .ok_or("missing measured label")?;
    let document = Document::parse(&svg)?;
    let element = document
        .descendants()
        .find(|node| node.has_tag_name("text") && node.text() == Some("A"))
        .ok_or("missing label element")?;
    assert_eq!(text.rect.y, number(element, "y")? - text.rect.height / 2);
    Ok(())
}

#[test]
fn parser_reads_every_current_node_shape_envelope() -> Result<()> {
    for kind in ["actor", "client", "function", "worker", "database"] {
        let source = format!("stack 1.0 diagram \"Shape\" {{ node a \"A\" {{ kind {kind} }} }}");
        let engine = Engine::bundled();
        let svg = engine.render(source.as_bytes())?.svg.ok_or("missing SVG")?;
        let compiled = stack_compiler::compile_bytes_with_source_map(source.as_bytes());
        let diagram = compiled.diagram.as_ref().ok_or("missing diagram")?;
        let prepared = engine.prepare_scene(
            diagram,
            compiled.source_map.as_ref().ok_or("missing source map")?,
        )?;
        let drawing = read_drawing(&svg, &prepared, diagram)?;
        assert_eq!(
            drawing.nodes,
            vec![("a".to_owned(), rect(prepared.scene.nodes[0].rect))],
            "{kind}"
        );
    }
    Ok(())
}

#[test]
fn frame_detector_includes_both_stroke_radii_without_terminal_exemptions() -> Result<()> {
    let mut drawing = Drawing {
        bounds: Rect {
            x: 0,
            y: 0,
            width: 500_000,
            height: 800_000,
        },
        texts: vec![],
        nodes: vec![],
        frames: vec![GroupFrame {
            id: "a".into(),
            rect: Rect {
                x: 100_000,
                y: 200_000,
                width: 300_000,
                height: 400_000,
            },
            radius: 500,
        }],
        routes: vec![Route {
            id: "edge:0".into(),
            from: "a".into(),
            to: "b".into(),
            points: vec![
                Point {
                    x: 150_000,
                    y: 182_751,
                },
                Point {
                    x: 350_000,
                    y: 182_751,
                },
            ],
            radius: 750,
        }],
    };
    let violations = frame_violations(&drawing)?;
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0]["requiredCenterlineClearanceMilliPx"], 17_250);
    for point in &mut drawing.routes[0].points {
        point.y = 182_750;
    }
    assert!(frame_violations(&drawing)?.is_empty());
    for point in &mut drawing.routes[0].points {
        point.y = 200_000;
    }
    assert_eq!(frame_violations(&drawing)?.len(), 1);
    drawing.routes[0].points = vec![
        Point { x: 0, y: 400_000 },
        Point {
            x: 500_000,
            y: 400_000,
        },
    ];
    assert!(frame_violations(&drawing)?.is_empty());
    Ok(())
}

#[test]
fn parser_measures_final_svg_group_frames_and_rejects_missing_or_hidden_frames() -> Result<()> {
    let source = b"stack 1.0 diagram \"Frame\" { group system \"System\" { node a \"A\" } }";
    let engine = Engine::bundled();
    let svg = engine.render(source)?.svg.ok_or("missing SVG")?;
    let compiled = stack_compiler::compile_bytes_with_source_map(source);
    let diagram = compiled.diagram.as_ref().ok_or("missing diagram")?;
    let prepared = engine.prepare_scene(
        diagram,
        compiled.source_map.as_ref().ok_or("missing source map")?,
    )?;
    let document = Document::parse(&svg)?;
    let owner = document
        .descendants()
        .find(|node| node.attribute("data-stack-id") == Some("system"))
        .ok_or("missing group frame owner")?;
    let frame = owner
        .children()
        .find(|node| node.has_tag_name("rect"))
        .ok_or("missing group frame")?;
    let frame_markup = &svg[frame.range()];
    let replace = |node: Node<'_, '_>, replacement: &str| {
        let mut mutant = svg.clone();
        mutant.replace_range(node.range(), replacement);
        mutant
    };
    let drawing = read_drawing(&svg, &prepared, diagram)?;
    assert_eq!(drawing.frames.len(), 1);
    assert_eq!(drawing.frames[0].rect, rectangle(frame)?);
    assert_eq!(drawing.frames[0].radius, 500);

    let wider_stroke = replace(
        frame,
        &frame_markup.replacen(
            &format!("stroke-width=\"{}\"", required(frame, "stroke-width")?),
            "stroke-width=\"1.001\"",
            1,
        ),
    );
    assert_ne!(wider_stroke, svg, "mutation must widen the frame stroke");
    let wider = read_drawing(&wider_stroke, &prepared, diagram)?;
    assert_eq!(wider.frames[0].radius, 501);

    let mutations = [
        ("removed frame", replace(frame, "")),
        (
            "duplicate frame",
            replace(frame, &format!("{frame_markup}{frame_markup}")),
        ),
        (
            "moved frame",
            replace(
                frame,
                &frame_markup.replacen(
                    &format!("x=\"{}\"", required(frame, "x")?),
                    "x=\"-999.999\"",
                    1,
                ),
            ),
        ),
        (
            "zero frame width",
            replace(
                frame,
                &frame_markup.replacen(
                    &format!("width=\"{}\"", required(frame, "width")?),
                    "width=\"0\"",
                    1,
                ),
            ),
        ),
        (
            "zero frame stroke",
            replace(
                frame,
                &frame_markup.replacen(
                    &format!("stroke-width=\"{}\"", required(frame, "stroke-width")?),
                    "stroke-width=\"0\"",
                    1,
                ),
            ),
        ),
        (
            "invisible frame stroke",
            replace(
                frame,
                &frame_markup.replacen(
                    &format!("stroke=\"{}\"", required(frame, "stroke")?),
                    "stroke=\"none\"",
                    1,
                ),
            ),
        ),
        (
            "inherited invisible frame",
            replace(
                owner,
                &svg[owner.range()].replacen("<g ", "<g stroke-opacity=\"0\" ", 1),
            ),
        ),
        (
            "missing owner id",
            replace(
                owner,
                &svg[owner.range()].replacen("data-stack-id=", "removed-stack-id=", 1),
            ),
        ),
    ];
    for (name, mutant) in mutations {
        assert_ne!(mutant, svg, "mutation must exercise {name}");
        assert!(
            read_drawing(&mutant, &prepared, diagram).is_err(),
            "accepted {name}"
        );
    }
    Ok(())
}

#[test]
fn frame_detector_measures_edge_label_backgrounds_without_excluding_group_interiors() -> Result<()>
{
    let mut drawing = Drawing {
        bounds: Rect {
            x: 0,
            y: 0,
            width: 500_000,
            height: 800_000,
        },
        texts: vec![TextBox {
            id: "edge-label:0:Request".into(),
            rect: Rect {
                x: 150_000,
                y: 190_000,
                width: 100_000,
                height: 20_000,
            },
        }],
        nodes: vec![],
        frames: vec![GroupFrame {
            id: "system".into(),
            rect: Rect {
                x: 100_000,
                y: 200_000,
                width: 300_000,
                height: 400_000,
            },
            radius: 500,
        }],
        routes: vec![],
    };
    let violations = frame_violations(&drawing)?;
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0]["kind"], "label-group-frame-clearance");
    assert_eq!(violations[0]["requiredCenterlineClearanceMilliPx"], 8_500);
    drawing.texts[0].rect.y = 208_500;
    assert!(frame_violations(&drawing)?.is_empty());
    drawing.texts[0].rect.y -= 1;
    assert_eq!(frame_violations(&drawing)?.len(), 1);
    drawing.texts[0].rect.y = 171_500;
    assert!(frame_violations(&drawing)?.is_empty());
    Ok(())
}

#[test]
fn px_parser_restores_exact_milli_pixels_without_floating_point() -> Result<()> {
    for (value, expected) in [
        ("0", 0),
        ("-0", 0),
        ("32", 32_000),
        ("-32", -32_000),
        ("1.5", 1_500),
        ("1.25", 1_250),
        ("1.001", 1_001),
        ("0.001", 1),
        ("-0.001", -1),
        ("123.000", 123_000),
        ("9223372036854775.807", i64::MAX),
        ("-9223372036854775.808", i64::MIN),
    ] {
        assert_eq!(pixel_value(value)?, expected, "{value}");
    }
    Ok(())
}

#[test]
fn px_parser_rejects_invalid_signs_precision_and_overflow() {
    for value in [
        "",
        "+1",
        "--1",
        "-+1",
        "1-",
        "-",
        ".5",
        "-.5",
        "1.",
        "1.2.3",
        "1.0001",
        "-0.0001",
        "1e3",
        "NaN",
        "Infinity",
        "1px",
        "1%",
        " 1",
        "1 ",
        "1\n",
        "1,5",
        "１２",
        "01",
        "00.5",
        "9223372036854775.808",
        "-9223372036854775.809",
        "9223372036854776",
        "-9223372036854776",
        "999999999999999999999999999999999999999999999999",
    ] {
        assert!(
            pixel_value(value).is_err(),
            "accepted unsupported px value {value:?}"
        );
    }
}

#[test]
fn px_parser_scales_rectangle_points_and_absolute_path_coordinates() -> Result<()> {
    assert_eq!(
        point_list("-1.125,2.5 4,5.875")?,
        vec![
            Point {
                x: -1_125,
                y: 2_500
            },
            Point { x: 4_000, y: 5_875 }
        ]
    );
    for (markup, expected) in [
        (
            r#"<rect x="-1.125" y="2.5" width="4" height="5.875"/>"#,
            Rect {
                x: -1_125,
                y: 2_500,
                width: 4_000,
                height: 5_875,
            },
        ),
        (
            r#"<ellipse cx="2.125" cy="4.5" rx="1.125" ry="2.5"/>"#,
            Rect {
                x: 1_000,
                y: 2_000,
                width: 2_250,
                height: 5_000,
            },
        ),
        (
            r#"<polygon points="-1.125,2.5 4,5.875 1,1.25"/>"#,
            Rect {
                x: -1_125,
                y: 1_250,
                width: 5_125,
                height: 4_625,
            },
        ),
        (
            r#"<path d="M 1.25 2.5 V 6.75 C 1.25 7.5 5.875 7.5 5.875 6.75 V 2.5 C 5.875 1.125 1.25 1.125 1.25 2.5 Z"/>"#,
            Rect {
                x: 1_250,
                y: 1_125,
                width: 4_625,
                height: 6_375,
            },
        ),
    ] {
        assert_eq!(
            shape_envelope(Document::parse(markup)?.root_element())?,
            expected,
            "{markup}"
        );
    }
    Ok(())
}
