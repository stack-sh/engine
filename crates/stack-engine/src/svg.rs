//! Safe standalone SVG serialization for a resolved scene.

use stack_compiler::ir::{Diagram, EdgeDirection, EdgeKind, NodeKind};
use stack_theme::{NodeShape, PaletteToken, Theme};

use crate::EngineMetadata;
use crate::resources::{ResolvedNode, Resources};
use crate::routing::{Marker, SceneEdge};
use crate::scene::{GROUP_PADDING, Rect, Scene, SceneGroup, SceneNode};

const GROUP_CORNER_RADIUS: i64 = 12_000;
const ICON_SIZE: i64 = 24_000;
const NODE_HORIZONTAL_PADDING: i64 = 20_000;
const ICON_TEXT_GAP: i64 = 12_000;

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
        pixel_dimension(scene.bounds.x),
        pixel_dimension(scene.bounds.y),
        pixel_dimension(scene.bounds.width),
        pixel_dimension(scene.bounds.height),
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
    let provider_metadata = resources
        .provider_notices()
        .into_iter()
        .map(|notice| {
            format!(
                "{} at {} using {}",
                notice.provider_id,
                notice.pack_revision,
                notice
                    .icons
                    .iter()
                    .map(|icon| icon.id.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    let provider_metadata = if provider_metadata.is_empty() {
        String::new()
    } else {
        format!("; providers {provider_metadata}")
    };
    output.push_str(&format!(
        "  <metadata>stack-engine {}; language {}.{}; theme {} at {}{}</metadata>\n",
        escape_text(&metadata.engine_version),
        metadata.language_version.map_or(0, |version| version.major),
        metadata.language_version.map_or(0, |version| version.minor),
        escape_text(&metadata.theme_catalog_version),
        escape_text(&metadata.theme_catalog_revision),
        escape_text(&provider_metadata)
    ));
    output.push_str(&format!(
        "  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>\n",
        pixel_dimension(scene.bounds.x),
        pixel_dimension(scene.bounds.y),
        pixel_dimension(scene.bounds.width),
        pixel_dimension(scene.bounds.height),
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
        pixel_dimension(i64::from(theme.connector.arrow_size_milli_px)),
        pixel_dimension(i64::from(theme.connector.arrow_size_milli_px))
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
        pixel_dimension(scene.content_rect.x),
        pixel_dimension(baseline),
        escape_attribute(&resources.theme.palette.text),
        escape_attribute(&resources.metrics.family),
        pixel_dimension(i64::from(typography.group_label_size_milli_px)),
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
        "      <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"{}\" fill-opacity=\"0.55\" stroke=\"{}\" stroke-width=\"1\"/>\n",
        pixel_dimension(group.rect.x),
        pixel_dimension(group.rect.y),
        pixel_dimension(group.rect.width),
        pixel_dimension(group.rect.height),
        pixel_dimension(GROUP_CORNER_RADIUS),
        escape_attribute(&theme.palette.surface_muted),
        escape_attribute(&theme.palette.border)
    ));
    output.push_str(&format!(
        "      <text x=\"{}\" y=\"{}\" fill=\"{}\" font-family=\"{}\" font-size=\"{}\" font-weight=\"{}\">{}</text>\n",
        pixel_dimension(group.rect.x + GROUP_PADDING),
        pixel_dimension(group.rect.y + GROUP_PADDING + i64::from(theme.typography.group_label_size_milli_px)),
        escape_attribute(&theme.palette.text),
        escape_attribute(&resources.metrics.family),
        pixel_dimension(i64::from(theme.typography.group_label_size_milli_px)),
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
        .map(|point| format!("{},{}", pixel_dimension(point.x), pixel_dimension(point.y)))
        .collect::<Vec<_>>()
        .join(" ");
    let marker_start = marker_attribute("marker-start", edge.start_marker);
    let marker_end = marker_attribute("marker-end", edge.end_marker);
    let dash = dash_attribute(edge, resources.theme);
    output.push_str(&format!(
        "      <polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"{}{}{} />\n",
        points,
        escape_attribute(palette_color(resources.theme, resources.theme.connector.stroke)),
        pixel_dimension(i64::from(resources.theme.connector.width_milli_px)),
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
                .map(|value| pixel_dimension(i64::from(*value)))
                .collect::<Vec<_>>()
                .join(" ")
        )
    })
}

fn render_edge_labels(output: &mut String, scene: &Scene, resources: &Resources<'_>) {
    output.push_str("  <g data-stack-layer=\"edge-labels\" aria-hidden=\"true\">\n");
    for edge in &scene.edges {
        if let (Some(label), Some(rect)) = (&edge.label, edge.label_rect) {
            render_edge_label(output, label, rect, resources);
        }
    }
    output.push_str("  </g>\n");
}

fn render_edge_label(output: &mut String, label: &str, rect: Rect, resources: &Resources<'_>) {
    let typography = &resources.theme.typography;
    output.push_str(&format!(
        "    <g data-edge-label=\"{}\">\n      <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"4\" fill=\"{}\"/>\n",
        escape_attribute(label),
        pixel_dimension(rect.x),
        pixel_dimension(rect.y),
        pixel_dimension(rect.width),
        pixel_dimension(rect.height),
        escape_attribute(palette_color(
            resources.theme,
            resources.theme.connector.label_background
        ))
    ));
    output.push_str(&format!(
        "      <text x=\"{}\" y=\"{}\" text-anchor=\"middle\" dominant-baseline=\"middle\" fill=\"{}\" font-family=\"{}\" font-size=\"{}\">{}</text>\n    </g>\n",
        pixel_dimension(rect.x + rect.width / 2),
        pixel_dimension(rect.y + rect.height / 2),
        escape_attribute(palette_color(
            resources.theme,
            resources.theme.connector.text
        )),
        escape_attribute(&resources.metrics.family),
        pixel_dimension(i64::from(typography.edge_label_size_milli_px)),
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
            "      <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.5\"/>\n",
            pixel_dimension(rect.x),
            pixel_dimension(rect.y),
            pixel_dimension(rect.width),
            pixel_dimension(rect.height),
            pixel_dimension(i64::from(resolved.visual.corner_radius_milli_px)),
            fill,
            stroke
        )),
        NodeShape::Circle => output.push_str(&format!(
            "      <circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.5\"/>\n",
            pixel_dimension(rect.x + rect.width / 2),
            pixel_dimension(rect.y + rect.height / 2),
            pixel_dimension(rect.width.min(rect.height) / 2),
            fill,
            stroke
        )),
        NodeShape::Cylinder => {
            let radius_y = 10_000;
            output.push_str(&format!(
                "      <path d=\"M {} {} V {} C {} {} {} {} {} {} V {} C {} {} {} {} {} {} Z\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.5\"/>\n",
                pixel_dimension(rect.x),
                pixel_dimension(rect.y + radius_y),
                pixel_dimension(rect.y + rect.height - radius_y),
                pixel_dimension(rect.x),
                pixel_dimension(rect.y + rect.height),
                pixel_dimension(rect.x + rect.width),
                pixel_dimension(rect.y + rect.height),
                pixel_dimension(rect.x + rect.width),
                pixel_dimension(rect.y + rect.height - radius_y),
                pixel_dimension(rect.y + radius_y),
                pixel_dimension(rect.x + rect.width),
                pixel_dimension(rect.y),
                pixel_dimension(rect.x),
                pixel_dimension(rect.y),
                pixel_dimension(rect.x),
                pixel_dimension(rect.y + radius_y),
                fill,
                stroke
            ));
            output.push_str(&format!(
                "      <ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.5\"/>\n",
                pixel_dimension(rect.x + rect.width / 2),
                pixel_dimension(rect.y + radius_y),
                pixel_dimension(rect.width / 2),
                pixel_dimension(radius_y),
                stroke
            ));
        }
        NodeShape::Hexagon => output.push_str(&format!(
            "      <polygon points=\"{},{} {},{} {},{} {},{} {},{} {},{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.5\"/>\n",
            pixel_dimension(rect.x + 16_000),
            pixel_dimension(rect.y),
            pixel_dimension(rect.x + rect.width - 16_000),
            pixel_dimension(rect.y),
            pixel_dimension(rect.x + rect.width),
            pixel_dimension(rect.y + rect.height / 2),
            pixel_dimension(rect.x + rect.width - 16_000),
            pixel_dimension(rect.y + rect.height),
            pixel_dimension(rect.x + 16_000),
            pixel_dimension(rect.y + rect.height),
            pixel_dimension(rect.x),
            pixel_dimension(rect.y + rect.height / 2),
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
    let view_box = resolved.icon_view_box;
    output.push_str(&format!(
        "      <svg x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\" color=\"{}\" aria-hidden=\"true\" focusable=\"false\" data-icon-id=\"{}\">{} </svg>\n",
        pixel_dimension(rect.x + NODE_HORIZONTAL_PADDING),
        pixel_dimension(rect.y + (rect.height - ICON_SIZE) / 2),
        pixel_dimension(ICON_SIZE),
        pixel_dimension(ICON_SIZE),
        view_box[0],
        view_box[1],
        view_box[2],
        view_box[3],
        escape_attribute(palette_color(theme, resolved.visual.accent)),
        escape_attribute(resolved.icon_id),
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
        pixel_dimension(text_x),
        pixel_dimension(label_y),
        escape_attribute(palette_color(resources.theme, resolved.visual.text)),
        escape_attribute(&resources.metrics.family),
        pixel_dimension(i64::from(typography.node_label_size_milli_px)),
        typography.label_weight,
        escape_text(&node.label)
    ));
    if let Some(detail) = &node.detail {
        output.push_str(&format!(
            "      <text x=\"{}\" y=\"{}\" dominant-baseline=\"middle\" fill=\"{}\" font-family=\"{}\" font-size=\"{}\" font-weight=\"{}\">{}</text>\n",
            pixel_dimension(text_x),
            pixel_dimension(rect.y + rect.height / 2 + 10_000),
            escape_attribute(&resources.theme.palette.text_muted),
            escape_attribute(&resources.metrics.family),
            pixel_dimension(i64::from(typography.node_detail_size_milli_px)),
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
    // Serialize only at the SVG boundary; scene geometry stays in integer milli-pixels.
    let magnitude = milli_pixels.unsigned_abs();
    let sign = if milli_pixels < 0 { "-" } else { "" };
    let whole = magnitude / 1000;
    let remainder = magnitude % 1000;
    if remainder == 0 {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{remainder:03}")
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
    use super::{escape_attribute, escape_text, pixel_dimension};
    use crate::resources::Resources;
    use crate::routing::Point;
    use crate::scene::Rect;

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
    fn formats_signed_milli_pixels_including_integer_limits() {
        for (input, expected) in [
            (0, "0"),
            (1, "0.001"),
            (-1, "-0.001"),
            (-10, "-0.01"),
            (-100, "-0.1"),
            (-1000, "-1"),
            (-1001, "-1.001"),
            (i64::MIN, "-9223372036854775.808"),
            (i64::MAX, "9223372036854775.807"),
        ] {
            assert_eq!(pixel_dimension(input), expected);
        }
    }

    #[test]
    fn serializes_the_outer_view_box_and_all_text_sizes_in_css_pixels()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = b"stack 1.0 diagram \"Units\" { group g \"Group\" { node a \"A\" { detail \"Detail\" } node b \"B\" } edge a -> b \"Label\" }";
        let diagram = stack_compiler::compile_bytes(source)
            .diagram
            .ok_or("valid diagram")?;
        let scene = crate::scene::layout(&diagram, stack_theme::catalog())?;
        let output = crate::Engine::bundled()
            .render(source)?
            .svg
            .ok_or("SVG document")?;
        assert!(output.contains(&format!(
            "width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\"",
            pixel_dimension(scene.bounds.width),
            pixel_dimension(scene.bounds.height),
            pixel_dimension(scene.bounds.width),
            pixel_dimension(scene.bounds.height),
        )));
        let theme = &stack_theme::catalog().themes[0];
        for size in [
            theme.typography.node_label_size_milli_px,
            theme.typography.node_detail_size_milli_px,
            theme.typography.edge_label_size_milli_px,
            theme.typography.group_label_size_milli_px,
        ] {
            assert!(output.contains(&format!(
                "font-size=\"{}\"",
                pixel_dimension(i64::from(size))
            )));
            assert!(!output.contains(&format!("font-size=\"{size}\"")));
        }
        assert!(output.contains("font-weight=\"600\""));
        assert!(output.contains("rx=\"12\""));
        assert!(output.contains("fill-opacity=\"0.55\""));
        assert!(output.contains("stroke-width=\"1\""));
        Ok(())
    }

    #[test]
    fn renderer_rejects_missing_semantic_and_resource_records()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = b"stack 1.0 diagram \"Integrity\" { group g \"Group\" { node a \"A\" node b \"B\" } edge a -> b }";
        let diagram = stack_compiler::compile_bytes(source)
            .diagram
            .ok_or("missing diagram")?;
        let scene = crate::scene::layout(&diagram, stack_theme::catalog())?;
        let metadata = crate::Engine::bundled().check(source)?.metadata;
        for (case, expected) in [
            (0, "scene group has no normalized IR record"),
            (1, "edge source has no normalized node"),
            (2, "edge target has no normalized node"),
            (3, "scene node has no normalized IR record"),
            (4, "scene node has no resolved theme record"),
        ] {
            let mut invalid = diagram.clone();
            let mut scene = scene.clone();
            let mut resources = Resources::resolve(&diagram, stack_theme::catalog(), &[])
                .map_err(|error| error.reason())?;
            match case {
                0 => invalid.groups.clear(),
                1 => {
                    invalid.nodes.remove(0);
                }
                2 => {
                    invalid.nodes.remove(1);
                }
                3 => {
                    scene.edges.clear();
                    invalid.nodes.clear();
                }
                _ => resources.nodes.clear(),
            }
            assert_eq!(
                super::render(&invalid, &scene, &resources, &metadata)
                    .map_err(|error| error.reason()),
                Err(expected)
            );
        }
        Ok(())
    }

    #[test]
    fn serializes_every_node_shape_in_css_pixels() -> Result<(), Box<dyn std::error::Error>> {
        let rect = Rect {
            x: -1250,
            y: 267_800,
            width: 208_300,
            height: 72_000,
        };
        for (kind, expected) in [
            (
                "service",
                "<rect x=\"-1.25\" y=\"267.8\" width=\"208.3\" height=\"72\" rx=\"8\"",
            ),
            (
                "worker",
                "<rect x=\"-1.25\" y=\"267.8\" width=\"208.3\" height=\"72\" rx=\"16\"",
            ),
            ("actor", "<circle cx=\"102.9\" cy=\"303.8\" r=\"36\""),
            (
                "cache",
                "<rect x=\"-1.25\" y=\"267.8\" width=\"208.3\" height=\"72\" rx=\"8\"",
            ),
            (
                "function",
                "<polygon points=\"14.75,267.8 191.05,267.8 207.05,303.8 191.05,339.8 14.75,339.8 -1.25,303.8\"",
            ),
        ] {
            let source =
                format!("stack 1.0 diagram \"Shape\" {{ node a \"A\" {{ kind {kind} }} }}");
            let diagram = stack_compiler::compile_bytes(source.as_bytes())
                .diagram
                .ok_or("valid diagram")?;
            let resources = Resources::resolve(&diagram, stack_theme::catalog(), &[])
                .map_err(|error| error.reason())?;
            let mut output = String::new();
            super::render_node_shape(
                &mut output,
                rect,
                resources.node("a").ok_or("resolved node")?,
                resources.theme,
            );
            assert!(output.contains(expected), "{kind}: {output}");
            assert!(output.contains("stroke-width=\"1.5\""));
            if kind == "cache" {
                assert!(!output.contains("<ellipse"));
            }
        }
        Ok(())
    }

    #[test]
    fn custom_catalogs_can_still_render_cylinder_nodes() -> Result<(), Box<dyn std::error::Error>> {
        let rect = Rect {
            x: -1250,
            y: 267_800,
            width: 208_300,
            height: 72_000,
        };
        let source = b"stack 1.0 diagram \"Shape\" { node a \"A\" { kind cache } }";
        let diagram = stack_compiler::compile_bytes(source)
            .diagram
            .ok_or("valid diagram")?;
        let mut catalog = stack_theme::catalog().clone();
        catalog.themes[0].node_kind_fallbacks.cache.shape = stack_theme::NodeShape::Cylinder;
        catalog.themes[0]
            .node_kind_fallbacks
            .cache
            .corner_radius_milli_px = 0;
        let resources =
            Resources::resolve(&diagram, &catalog, &[]).map_err(|error| error.reason())?;
        let mut output = String::new();

        super::render_node_shape(
            &mut output,
            rect,
            resources.node("a").ok_or("resolved node")?,
            resources.theme,
        );

        assert!(output.contains(
            "<path d=\"M -1.25 277.8 V 329.8 C -1.25 339.8 207.05 339.8 207.05 329.8 V 277.8 C 207.05 267.8 -1.25 267.8 -1.25 277.8 Z\""
        ));
        assert!(output.contains("<ellipse cx=\"102.9\" cy=\"277.8\" rx=\"104.15\" ry=\"10\""));
        Ok(())
    }

    #[test]
    fn scales_routes_dash_patterns_and_only_the_marker_viewport()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = b"stack 1.0 diagram \"Routes\" { node a \"A\" node b \"B\" edge a -> b }";
        let diagram = stack_compiler::compile_bytes(source)
            .diagram
            .ok_or("valid diagram")?;
        let resources = Resources::resolve(&diagram, stack_theme::catalog(), &[])
            .map_err(|error| error.reason())?;
        let mut scene = crate::scene::layout(&diagram, stack_theme::catalog())?;
        let edge = scene.edges.first_mut().ok_or("scene edge")?;
        edge.path = vec![Point { x: -1, y: 1250 }, Point { x: 20_000, y: 1250 }];
        let mut output = String::new();
        super::render_edge(&mut output, edge, &diagram, &resources)
            .map_err(|error| error.reason())?;
        assert!(output.contains("points=\"-0.001,1.25 20,1.25\""));
        assert!(output.contains(&format!(
            "stroke-width=\"{}\"",
            pixel_dimension(i64::from(resources.theme.connector.width_milli_px))
        )));
        let mut theme = resources.theme.clone();
        theme.connector.arrow_size_milli_px = 10_125;
        theme.connector.dash_milli_px = Some(vec![1250, 2001]);
        assert_eq!(
            super::dash_attribute(edge, &theme),
            " stroke-dasharray=\"1.25 2.001\""
        );
        edge.direction = stack_compiler::ir::EdgeDirection::Association;
        assert_eq!(
            super::dash_attribute(edge, &theme),
            " stroke-dasharray=\"8 6\""
        );
        super::render_definitions(&mut output, &theme);
        assert!(output.contains("markerWidth=\"10.125\" markerHeight=\"10.125\""));
        assert!(output.contains("refX=\"9000\" refY=\"5000\" viewBox=\"0 0 10000 10000\""));
        assert!(output.contains("d=\"M 0 0 L 10000 5000 L 0 10000 z\""));
        Ok(())
    }

    #[test]
    fn scales_the_icon_viewport_without_rewriting_its_local_geometry()
    -> Result<(), Box<dyn std::error::Error>> {
        let diagram = stack_compiler::compile_bytes(
            b"stack 1.0 diagram \"Icon\" { node a \"A\" { kind database } }",
        )
        .diagram
        .ok_or("valid diagram")?;
        let resources = Resources::resolve(&diagram, stack_theme::catalog(), &[])
            .map_err(|error| error.reason())?;
        let resolved = resources.node("a").ok_or("resolved node")?;
        let mut output = String::new();
        super::render_icon(
            &mut output,
            Rect {
                x: -1250,
                y: 267_800,
                width: 208_300,
                height: 72_000,
            },
            resolved,
            resources.theme,
        )
        .map_err(|error| error.reason())?;
        assert!(output.contains(
            "<svg x=\"18.75\" y=\"291.8\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\""
        ));
        let body = super::embedded_svg_body(resolved.icon_svg).ok_or("icon body")?;
        assert!(output.contains(body.trim()));
        Ok(())
    }

    #[test]
    fn serializes_the_scene_label_rectangle_without_repositioning()
    -> Result<(), Box<dyn std::error::Error>> {
        let engine = crate::Engine::bundled();
        let source =
            b"stack 1.0 diagram \"Labels\" { node a \"A\" node b \"B\" edge a -> b \"request\" }";
        let compiled = stack_compiler::compile_bytes(source);
        let scene = crate::scene::layout(
            &compiled.diagram.ok_or("valid diagram")?,
            stack_theme::catalog(),
        )?;
        let rect = scene.edges[0].label_rect.ok_or("scene label geometry")?;
        let output = engine.render(source)?.svg.ok_or("SVG document")?;
        assert!(output.contains(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"4\"",
            pixel_dimension(rect.x),
            pixel_dimension(rect.y),
            pixel_dimension(rect.width),
            pixel_dimension(rect.height)
        )));
        Ok(())
    }
}
