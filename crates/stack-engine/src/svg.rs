//! Safe standalone SVG serialization for a resolved scene.

use stack_compiler::ir::{Diagram, EdgeDirection, EdgeKind, NodeKind};
use stack_theme::{NodeShape, PaletteToken, Theme};

use crate::EngineMetadata;
use crate::resources::{ResolvedNode, Resources};
use crate::routing::{Marker, Point, SceneEdge};
use crate::scene::{self, Rect, Scene, SceneGroup, SceneNode};

const GROUP_CORNER_RADIUS: i64 = 12_000;
const ICON_SIZE: i64 = 24_000;
const NODE_HORIZONTAL_PADDING: i64 = 20_000;
const ICON_TEXT_GAP: i64 = 12_000;
const EDGE_LABEL_HORIZONTAL_PADDING: i64 = 6_000;
const EDGE_LABEL_VERTICAL_PADDING: i64 = 4_000;
const EDGE_LABEL_CLEARANCE: i64 = 8_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SvgError {
    reason: &'static str,
}

impl SvgError {
    pub(crate) const fn reason(self) -> &'static str {
        self.reason
    }
}

pub(crate) fn render(
    diagram: &Diagram,
    scene: &Scene,
    resources: &Resources<'_>,
    metadata: &EngineMetadata,
) -> Result<String, SvgError> {
    let theme = resources.theme;
    let mut output = String::new();
    output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    output.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\" role=\"img\" aria-labelledby=\"stack-title stack-description\" data-engine-version=\"{}\" data-theme-id=\"{}\" data-theme-version=\"{}\" data-theme-revision=\"{}\">\n",
        pixel_dimension(scene.bounds.width),
        pixel_dimension(scene.bounds.height),
        scene.bounds.x,
        scene.bounds.y,
        scene.bounds.width,
        scene.bounds.height,
        escape_attribute(&metadata.engine_version),
        escape_attribute(&theme.id),
        escape_attribute(&metadata.theme_catalog_version),
        escape_attribute(&metadata.theme_catalog_revision),
    ));
    output.push_str(&format!(
        "  <title id=\"stack-title\">{}</title>\n",
        escape_text(&diagram.title)
    ));
    output.push_str(&format!(
        "  <desc id=\"stack-description\">Architecture diagram with {}, {}, and {}.</desc>\n",
        counted(diagram.nodes.len(), "node", "nodes"),
        counted(diagram.groups.len(), "group", "groups"),
        counted(diagram.edges.len(), "relationship", "relationships")
    ));
    output.push_str(&format!(
        "  <metadata>stack-engine {}; language {}.{}; theme {} at {}</metadata>\n",
        escape_text(&metadata.engine_version),
        metadata.language_version.map_or(0, |version| version.major),
        metadata.language_version.map_or(0, |version| version.minor),
        escape_text(&metadata.theme_catalog_version),
        escape_text(&metadata.theme_catalog_revision)
    ));
    output.push_str(&format!(
        "  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>\n",
        scene.bounds.x,
        scene.bounds.y,
        scene.bounds.width,
        scene.bounds.height,
        escape_attribute(&theme.palette.canvas)
    ));
    render_definitions(&mut output, theme);
    render_diagram_title(&mut output, diagram, scene, resources);

    output.push_str("  <g data-stack-layer=\"groups\">\n");
    for group in &scene.groups {
        let authored = diagram
            .groups
            .iter()
            .find(|authored| authored.id == group.id)
            .ok_or(SvgError {
                reason: "scene group has no normalized IR record",
            })?;
        render_group(&mut output, group, &authored.label, resources);
    }
    output.push_str("  </g>\n");

    output.push_str("  <g data-stack-layer=\"edges\">\n");
    for edge in &scene.edges {
        render_edge(&mut output, edge, diagram, resources)?;
    }
    output.push_str("  </g>\n");

    output.push_str("  <g data-stack-layer=\"nodes\">\n");
    for node in &scene.nodes {
        let authored = diagram
            .nodes
            .iter()
            .find(|authored| authored.id == node.id)
            .ok_or(SvgError {
                reason: "scene node has no normalized IR record",
            })?;
        let resolved = resources.node(&node.id).ok_or(SvgError {
            reason: "scene node has no resolved theme record",
        })?;
        render_node(&mut output, node, authored, resolved, resources)?;
    }
    output.push_str("  </g>\n");
    render_edge_labels(&mut output, scene, resources);
    output.push_str("</svg>\n");
    Ok(output)
}

fn render_definitions(output: &mut String, theme: &Theme) {
    let connector = palette_color(theme, theme.connector.stroke);
    output.push_str("  <defs>\n");
    output.push_str(&format!(
        "    <marker id=\"stack-arrow\" markerUnits=\"userSpaceOnUse\" markerWidth=\"{}\" markerHeight=\"{}\" refX=\"9000\" refY=\"5000\" viewBox=\"0 0 10000 10000\" orient=\"auto-start-reverse\">\n",
        theme.connector.arrow_size_milli_px,
        theme.connector.arrow_size_milli_px
    ));
    output.push_str(&format!(
        "      <path d=\"M 0 0 L 10000 5000 L 0 10000 z\" fill=\"{}\"/>\n",
        escape_attribute(connector)
    ));
    output.push_str("    </marker>\n  </defs>\n");
}

fn render_diagram_title(
    output: &mut String,
    diagram: &Diagram,
    scene: &Scene,
    resources: &Resources<'_>,
) {
    let typography = &resources.theme.typography;
    let baseline = scene.content_rect.y - 20_000;
    output.push_str(&format!(
        "  <text x=\"{}\" y=\"{}\" fill=\"{}\" font-family=\"{}\" font-size=\"{}\" font-weight=\"{}\">{}</text>\n",
        scene.content_rect.x,
        baseline,
        escape_attribute(&resources.theme.palette.text),
        escape_attribute(&resources.metrics.family),
        typography.group_label_size_milli_px,
        typography.label_weight,
        escape_text(&diagram.title)
    ));
}

fn render_group(output: &mut String, group: &SceneGroup, label: &str, resources: &Resources<'_>) {
    let theme = resources.theme;
    output.push_str(&format!(
        "    <g role=\"group\" aria-label=\"{}\" data-stack-id=\"{}\">\n",
        escape_attribute(label),
        escape_attribute(&group.id)
    ));
    output.push_str(&format!("      <title>{}</title>\n", escape_text(label)));
    output.push_str(&format!(
        "      <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"{}\" fill-opacity=\"0.55\" stroke=\"{}\" stroke-width=\"1000\"/>\n",
        group.rect.x,
        group.rect.y,
        group.rect.width,
        group.rect.height,
        GROUP_CORNER_RADIUS,
        escape_attribute(&theme.palette.surface_muted),
        escape_attribute(&theme.palette.border)
    ));
    output.push_str(&format!(
        "      <text x=\"{}\" y=\"{}\" fill=\"{}\" font-family=\"{}\" font-size=\"{}\" font-weight=\"{}\">{}</text>\n",
        group.rect.x + 24_000,
        group.rect.y + 24_000 + i64::from(theme.typography.group_label_size_milli_px),
        escape_attribute(&theme.palette.text),
        escape_attribute(&resources.metrics.family),
        theme.typography.group_label_size_milli_px,
        theme.typography.label_weight,
        escape_text(label)
    ));
    output.push_str("    </g>\n");
}

fn render_edge(
    output: &mut String,
    edge: &SceneEdge,
    diagram: &Diagram,
    resources: &Resources<'_>,
) -> Result<(), SvgError> {
    let source_label = diagram
        .nodes
        .iter()
        .find(|node| node.id == edge.from)
        .map(|node| node.label.as_str())
        .ok_or(SvgError {
            reason: "edge source has no normalized node",
        })?;
    let target_label = diagram
        .nodes
        .iter()
        .find(|node| node.id == edge.to)
        .map(|node| node.label.as_str())
        .ok_or(SvgError {
            reason: "edge target has no normalized node",
        })?;
    let direction = direction_name(edge.direction);
    let relationship = accessible_direction(edge.direction);
    let kind = kind_name(edge.kind);
    let accessible_label = edge.label.as_ref().map_or_else(
        || format!("{source_label} {relationship} {target_label}"),
        |label| format!("{source_label} {relationship} {target_label}: {label}"),
    );
    output.push_str(&format!(
        "    <g role=\"group\" aria-label=\"{}\" data-edge-kind=\"{}\" data-edge-direction=\"{}\">\n",
        escape_attribute(&accessible_label),
        kind,
        direction
    ));
    output.push_str(&format!(
        "      <title>{}</title>\n",
        escape_text(&accessible_label)
    ));
    let points = edge
        .path
        .iter()
        .map(|point| format!("{},{}", point.x, point.y))
        .collect::<Vec<_>>()
        .join(" ");
    let marker_start = marker_attribute("marker-start", edge.start_marker);
    let marker_end = marker_attribute("marker-end", edge.end_marker);
    let dash = dash_attribute(edge, resources.theme);
    output.push_str(&format!(
        "      <polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"{}{}{} />\n",
        points,
        escape_attribute(palette_color(resources.theme, resources.theme.connector.stroke)),
        resources.theme.connector.width_milli_px,
        marker_start,
        marker_end,
        dash
    ));
    output.push_str("    </g>\n");
    Ok(())
}

fn marker_attribute(name: &str, marker: Marker) -> String {
    match marker {
        Marker::None => String::new(),
        Marker::Arrow => format!(" {name}=\"url(#stack-arrow)\""),
    }
}

fn dash_attribute(edge: &SceneEdge, theme: &Theme) -> String {
    let values = if edge.direction == EdgeDirection::Association {
        Some(vec![8_000, 6_000])
    } else {
        theme.connector.dash_milli_px.clone()
    };
    values.map_or_else(String::new, |values| {
        format!(
            " stroke-dasharray=\"{}\"",
            values
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        )
    })
}

fn render_edge_labels(output: &mut String, scene: &Scene, resources: &Resources<'_>) {
    let mut occupied = scene.nodes.iter().map(|node| node.rect).collect::<Vec<_>>();
    output.push_str("  <g data-stack-layer=\"edge-labels\" aria-hidden=\"true\">\n");
    for edge in &scene.edges {
        if let (Some(label), Some(anchor)) = (&edge.label, edge.label_anchor) {
            let dimensions = edge_label_dimensions(label, resources);
            let anchor = choose_label_anchor(
                anchor,
                dimensions.width,
                dimensions.height,
                scene.bounds,
                &occupied,
            );
            render_edge_label(output, label, anchor, dimensions, resources);
            occupied.push(centered_rect(anchor, dimensions.width, dimensions.height));
        }
    }
    output.push_str("  </g>\n");
}

fn edge_label_dimensions(label: &str, resources: &Resources<'_>) -> Rect {
    let typography = &resources.theme.typography;
    let text_width = scene::text_width(
        label,
        typography.edge_label_size_milli_px,
        resources.metrics,
    );
    let line_height = scene::line_height(typography.edge_label_size_milli_px, typography);
    Rect {
        x: 0,
        y: 0,
        width: text_width + 2 * EDGE_LABEL_HORIZONTAL_PADDING,
        height: line_height + 2 * EDGE_LABEL_VERTICAL_PADDING,
    }
}

fn choose_label_anchor(
    preferred: Point,
    width: i64,
    height: i64,
    bounds: Rect,
    occupied: &[Rect],
) -> Point {
    let mut candidates = vec![preferred];
    for rect in occupied {
        candidates.extend([
            Point {
                x: preferred.x,
                y: rect.y - height / 2 - EDGE_LABEL_CLEARANCE,
            },
            Point {
                x: preferred.x,
                y: rect.y + rect.height + height / 2 + EDGE_LABEL_CLEARANCE,
            },
            Point {
                x: rect.x - width / 2 - EDGE_LABEL_CLEARANCE,
                y: preferred.y,
            },
            Point {
                x: rect.x + rect.width + width / 2 + EDGE_LABEL_CLEARANCE,
                y: preferred.y,
            },
        ]);
    }
    candidates
        .into_iter()
        .enumerate()
        .filter(|(_, candidate)| {
            let candidate = centered_rect(*candidate, width, height);
            rect_contains(bounds, candidate)
                && occupied
                    .iter()
                    .all(|occupied| !rects_overlap(candidate, *occupied))
        })
        .min_by_key(|(index, candidate)| {
            (
                (candidate.x - preferred.x).abs() + (candidate.y - preferred.y).abs(),
                *index,
            )
        })
        .map_or(preferred, |(_, candidate)| candidate)
}

fn centered_rect(center: Point, width: i64, height: i64) -> Rect {
    Rect {
        x: center.x - width / 2,
        y: center.y - height / 2,
        width,
        height,
    }
}

fn rect_contains(outer: Rect, inner: Rect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
}

fn rects_overlap(left: Rect, right: Rect) -> bool {
    left.x < right.x + right.width
        && right.x < left.x + left.width
        && left.y < right.y + right.height
        && right.y < left.y + left.height
}

fn render_edge_label(
    output: &mut String,
    label: &str,
    anchor: Point,
    dimensions: Rect,
    resources: &Resources<'_>,
) {
    let typography = &resources.theme.typography;
    output.push_str(&format!(
        "    <g data-edge-label=\"{}\">\n      <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"4000\" fill=\"{}\"/>\n",
        escape_attribute(label),
        anchor.x - dimensions.width / 2,
        anchor.y - dimensions.height / 2,
        dimensions.width,
        dimensions.height,
        escape_attribute(palette_color(
            resources.theme,
            resources.theme.connector.label_background
        ))
    ));
    output.push_str(&format!(
        "      <text x=\"{}\" y=\"{}\" text-anchor=\"middle\" dominant-baseline=\"middle\" fill=\"{}\" font-family=\"{}\" font-size=\"{}\">{}</text>\n    </g>\n",
        anchor.x,
        anchor.y,
        escape_attribute(palette_color(
            resources.theme,
            resources.theme.connector.text
        )),
        escape_attribute(&resources.metrics.family),
        typography.edge_label_size_milli_px,
        escape_text(label)
    ));
}

fn render_node(
    output: &mut String,
    scene_node: &SceneNode,
    node: &stack_compiler::ir::Node,
    resolved: &ResolvedNode<'_>,
    resources: &Resources<'_>,
) -> Result<(), SvgError> {
    let accessible_label = node.detail.as_ref().map_or_else(
        || node.label.clone(),
        |detail| format!("{}: {detail}", node.label),
    );
    output.push_str(&format!(
        "    <g role=\"group\" aria-label=\"{}\" data-stack-id=\"{}\" data-node-kind=\"{}\">\n",
        escape_attribute(&accessible_label),
        escape_attribute(&node.id),
        node_kind_name(node.kind)
    ));
    output.push_str(&format!(
        "      <title>{}</title>\n",
        escape_text(&accessible_label)
    ));
    render_node_shape(output, scene_node.rect, resolved, resources.theme);
    render_icon(output, scene_node.rect, resolved, resources.theme)?;
    render_node_text(output, scene_node.rect, node, resolved, resources);
    output.push_str("    </g>\n");
    Ok(())
}

fn render_node_shape(output: &mut String, rect: Rect, resolved: &ResolvedNode<'_>, theme: &Theme) {
    let fill = escape_attribute(palette_color(theme, resolved.visual.fill));
    let stroke = escape_attribute(palette_color(theme, resolved.visual.stroke));
    match resolved.visual.shape {
        NodeShape::RoundedRectangle | NodeShape::Capsule => output.push_str(&format!(
            "      <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1500\"/>\n",
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            resolved.visual.corner_radius_milli_px,
            fill,
            stroke
        )),
        NodeShape::Circle => output.push_str(&format!(
            "      <circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1500\"/>\n",
            rect.x + rect.width / 2,
            rect.y + rect.height / 2,
            rect.width.min(rect.height) / 2,
            fill,
            stroke
        )),
        NodeShape::Cylinder => {
            let radius_y = 10_000;
            output.push_str(&format!(
                "      <path d=\"M {} {} V {} C {} {} {} {} {} {} V {} C {} {} {} {} {} {} Z\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1500\"/>\n",
                rect.x,
                rect.y + radius_y,
                rect.y + rect.height - radius_y,
                rect.x,
                rect.y + rect.height,
                rect.x + rect.width,
                rect.y + rect.height,
                rect.x + rect.width,
                rect.y + rect.height - radius_y,
                rect.y + radius_y,
                rect.x + rect.width,
                rect.y,
                rect.x,
                rect.y,
                rect.x,
                rect.y + radius_y,
                fill,
                stroke
            ));
            output.push_str(&format!(
                "      <ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1500\"/>\n",
                rect.x + rect.width / 2,
                rect.y + radius_y,
                rect.width / 2,
                radius_y,
                stroke
            ));
        }
        NodeShape::Hexagon => output.push_str(&format!(
            "      <polygon points=\"{},{} {},{} {},{} {},{} {},{} {},{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1500\"/>\n",
            rect.x + 16_000,
            rect.y,
            rect.x + rect.width - 16_000,
            rect.y,
            rect.x + rect.width,
            rect.y + rect.height / 2,
            rect.x + rect.width - 16_000,
            rect.y + rect.height,
            rect.x + 16_000,
            rect.y + rect.height,
            rect.x,
            rect.y + rect.height / 2,
            fill,
            stroke
        )),
    }
}

fn render_icon(
    output: &mut String,
    rect: Rect,
    resolved: &ResolvedNode<'_>,
    theme: &Theme,
) -> Result<(), SvgError> {
    let body = embedded_svg_body(resolved.icon_svg).ok_or(SvgError {
        reason: "embedded icon is not a complete SVG document",
    })?;
    let view_box = resolved.icon.asset.view_box;
    output.push_str(&format!(
        "      <svg x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\" color=\"{}\" aria-hidden=\"true\" focusable=\"false\" data-icon-id=\"{}\">{} </svg>\n",
        rect.x + NODE_HORIZONTAL_PADDING,
        rect.y + (rect.height - ICON_SIZE) / 2,
        ICON_SIZE,
        ICON_SIZE,
        view_box[0],
        view_box[1],
        view_box[2],
        view_box[3],
        escape_attribute(palette_color(theme, resolved.visual.accent)),
        escape_attribute(&resolved.icon.id),
        body.trim()
    ));
    Ok(())
}

fn embedded_svg_body(svg: &str) -> Option<&str> {
    let start = svg.find('>')? + 1;
    let end = svg.rfind("</svg>")?;
    (start <= end).then_some(&svg[start..end])
}

fn render_node_text(
    output: &mut String,
    rect: Rect,
    node: &stack_compiler::ir::Node,
    resolved: &ResolvedNode<'_>,
    resources: &Resources<'_>,
) {
    let typography = &resources.theme.typography;
    let text_x = rect.x + NODE_HORIZONTAL_PADDING + ICON_SIZE + ICON_TEXT_GAP;
    let label_y = if node.detail.is_some() {
        rect.y + rect.height / 2 - 9_000
    } else {
        rect.y + rect.height / 2
    };
    output.push_str(&format!(
        "      <text x=\"{}\" y=\"{}\" dominant-baseline=\"middle\" fill=\"{}\" font-family=\"{}\" font-size=\"{}\" font-weight=\"{}\">{}</text>\n",
        text_x,
        label_y,
        escape_attribute(palette_color(resources.theme, resolved.visual.text)),
        escape_attribute(&resources.metrics.family),
        typography.node_label_size_milli_px,
        typography.label_weight,
        escape_text(&node.label)
    ));
    if let Some(detail) = &node.detail {
        output.push_str(&format!(
            "      <text x=\"{}\" y=\"{}\" dominant-baseline=\"middle\" fill=\"{}\" font-family=\"{}\" font-size=\"{}\" font-weight=\"{}\">{}</text>\n",
            text_x,
            rect.y + rect.height / 2 + 10_000,
            escape_attribute(&resources.theme.palette.text_muted),
            escape_attribute(&resources.metrics.family),
            typography.node_detail_size_milli_px,
            typography.detail_weight,
            escape_text(detail)
        ));
    }
}

fn palette_color(theme: &Theme, token: PaletteToken) -> &str {
    match token {
        PaletteToken::Canvas => &theme.palette.canvas,
        PaletteToken::Surface => &theme.palette.surface,
        PaletteToken::SurfaceMuted => &theme.palette.surface_muted,
        PaletteToken::Text => &theme.palette.text,
        PaletteToken::TextMuted => &theme.palette.text_muted,
        PaletteToken::Border => &theme.palette.border,
        PaletteToken::Accent => &theme.palette.accent,
        PaletteToken::Danger => &theme.palette.danger,
        PaletteToken::Connector => &theme.palette.connector,
    }
}

fn direction_name(direction: EdgeDirection) -> &'static str {
    match direction {
        EdgeDirection::Forward => "forward",
        EdgeDirection::Bidirectional => "bidirectional",
        EdgeDirection::Association => "association",
    }
}

fn accessible_direction(direction: EdgeDirection) -> &'static str {
    match direction {
        EdgeDirection::Forward => "flows to",
        EdgeDirection::Bidirectional => "is connected bidirectionally with",
        EdgeDirection::Association => "is associated with",
    }
}

fn kind_name(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Flow => "flow",
        EdgeKind::Request => "request",
        EdgeKind::Event => "event",
        EdgeKind::Data => "data",
        EdgeKind::Dependency => "dependency",
    }
}

fn node_kind_name(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Actor => "actor",
        NodeKind::Client => "client",
        NodeKind::Service => "service",
        NodeKind::Function => "function",
        NodeKind::Worker => "worker",
        NodeKind::Database => "database",
        NodeKind::Cache => "cache",
        NodeKind::Queue => "queue",
        NodeKind::Storage => "storage",
        NodeKind::External => "external",
    }
}

fn pixel_dimension(milli_pixels: i64) -> String {
    let whole = milli_pixels / 1000;
    let remainder = milli_pixels % 1000;
    if remainder == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{remainder:03}")
            .trim_end_matches('0')
            .to_owned()
    }
}

fn counted(count: usize, singular: &str, plural: &str) -> String {
    format!("{count} {}", if count == 1 { singular } else { plural })
}

fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attribute(value: &str) -> String {
    escape_text(value)
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use crate::routing::Point;
    use crate::scene::Rect;

    use super::{choose_label_anchor, escape_attribute, escape_text, pixel_dimension};

    #[test]
    fn escapes_untrusted_text_and_attributes() {
        assert_eq!(escape_text("<&>"), "&lt;&amp;&gt;");
        assert_eq!(escape_attribute("\"<&>'"), "&quot;&lt;&amp;&gt;&apos;");
    }

    #[test]
    fn formats_milli_pixel_dimensions_without_float_rounding() {
        assert_eq!(pixel_dimension(504_000), "504");
        assert_eq!(pixel_dimension(1_081_200), "1081.2");
        assert_eq!(pixel_dimension(1_081_234), "1081.234");
    }

    #[test]
    fn moves_edge_labels_to_the_nearest_clear_position() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 500_000,
            height: 500_000,
        };
        let node = Rect {
            x: 80_000,
            y: 100_000,
            width: 160_000,
            height: 72_000,
        };
        assert_eq!(
            choose_label_anchor(
                Point {
                    x: 80_000,
                    y: 136_000,
                },
                32_000,
                24_000,
                bounds,
                &[node],
            ),
            Point {
                x: 56_000,
                y: 136_000,
            }
        );
    }
}
