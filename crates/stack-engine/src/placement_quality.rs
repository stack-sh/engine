//! Behavioral regression tests for graph-aware scene composition.

use std::error::Error;

#[cfg(feature = "conformance")]
use crate::routing::Point;
use crate::scene::{self, Rect, Scene, SceneDirection};

#[test]
fn labelled_dag_inventory_renders_without_hidden_collisions() -> Result<(), Box<dyn Error>> {
    let fixtures: serde_json::Value = serde_json::from_str(include_str!(
        "../tests/fixtures/labelled-dag-regressions.json"
    ))?;
    let cases = fixtures.as_array().ok_or("fixture array")?;
    assert_eq!(cases.len(), 60);
    let mut failures = Vec::new();
    for case in cases {
        let name = case["name"].as_str().ok_or("fixture name")?;
        let source = case["input"]["value"].as_str().ok_or("fixture source")?;
        match scene_from(source) {
            Ok(scene) => {
                assert!(scene.geometry_is_valid(), "{name}: geometry");
                assert_eq!(scene, scene_from(source)?, "{name}: deterministic scene");
            }
            Err(error) => failures.push(format!("{name}: {error}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    Ok(())
}

fn scene_from(source: &str) -> Result<Scene, Box<dyn Error>> {
    let compiled = stack_compiler::compile_bytes(source.as_bytes());
    if !compiled.diagnostics.is_empty() {
        return Err(format!("fixture produced diagnostics: {:?}", compiled.diagnostics).into());
    }
    let diagram = compiled.diagram.ok_or("fixture produced no diagram")?;
    Ok(scene::layout(&diagram, stack_theme::catalog())?)
}

fn node_rect(scene: &Scene, id: &str) -> Result<Rect, Box<dyn Error>> {
    scene
        .nodes
        .iter()
        .find(|node| node.id == id)
        .map(|node| node.rect)
        .ok_or_else(|| format!("missing node {id}").into())
}

fn group_rect(scene: &Scene, id: &str) -> Result<Rect, Box<dyn Error>> {
    scene
        .groups
        .iter()
        .find(|group| group.id == id)
        .map(|group| group.rect)
        .ok_or_else(|| format!("missing group {id}").into())
}

fn assert_progresses(before: Rect, after: Rect, direction: SceneDirection) {
    let (before_end, after_start) = match direction {
        SceneDirection::Right => (before.x + before.width, after.x),
        SceneDirection::Down => (before.y + before.height, after.y),
    };
    assert!(
        before_end < after_start,
        "connected elements must progress along {direction:?}: {before:?} -> {after:?}"
    );
}

fn assert_reordered_dag_progresses(direction: SceneDirection) -> Result<(), Box<dyn Error>> {
    let authored_direction = match direction {
        SceneDirection::Right => "right",
        SceneDirection::Down => "down",
    };
    let source = format!(
        r#"stack 1.0
diagram "Reordered pipeline" {{
  layout {{ direction {authored_direction} }}
  node sink "Sink"
  node source "Source"
  node processor "Processor"
  edge source -> processor
  edge processor -> sink
}}"#
    );
    let scene = scene_from(&source)?;
    assert_eq!(scene.direction, direction);
    assert_progresses(
        node_rect(&scene, "source")?,
        node_rect(&scene, "processor")?,
        direction,
    );
    assert_progresses(
        node_rect(&scene, "processor")?,
        node_rect(&scene, "sink")?,
        direction,
    );
    assert!(scene.geometry_is_valid());
    Ok(())
}

#[test]
fn right_directed_dag_progresses_by_connections_instead_of_declaration_order()
-> Result<(), Box<dyn Error>> {
    assert_reordered_dag_progresses(SceneDirection::Right)
}

#[test]
fn down_directed_dag_progresses_by_connections_instead_of_declaration_order()
-> Result<(), Box<dyn Error>> {
    assert_reordered_dag_progresses(SceneDirection::Down)
}

#[test]
fn graph_ranking_preserves_mandatory_same_rank_and_authored_cross_axis_order()
-> Result<(), Box<dyn Error>> {
    for (authored_direction, direction) in [
        ("right", SceneDirection::Right),
        ("down", SceneDirection::Down),
    ] {
        let source = format!(
            r#"stack 1.0
diagram "Constrained fanout" {{
  layout {{
    direction {authored_direction}
    rank same [primary, secondary]
    order [secondary, primary]
  }}
  node sink "Sink"
  node primary "Primary service"
  node source "Source"
  node secondary "Secondary service"
  edge source -> primary
  edge source -> secondary
  edge primary -> sink
  edge secondary -> sink
}}"#
        );
        let scene = scene_from(&source)?;
        let source = node_rect(&scene, "source")?;
        let sink = node_rect(&scene, "sink")?;
        let primary = node_rect(&scene, "primary")?;
        let secondary = node_rect(&scene, "secondary")?;
        match direction {
            SceneDirection::Right => {
                assert_eq!(primary.x, secondary.x);
                assert!(secondary.y + secondary.height < primary.y);
            }
            SceneDirection::Down => {
                assert_eq!(primary.y, secondary.y);
                assert!(secondary.x + secondary.width < primary.x);
            }
        }
        for branch in [primary, secondary] {
            assert_progresses(source, branch, direction);
            assert_progresses(branch, sink, direction);
        }
        assert!(scene.unsatisfied_orders.is_empty());
        assert!(scene.geometry_is_valid());
    }
    Ok(())
}

#[test]
fn reversing_descendant_connection_reverses_group_progression() -> Result<(), Box<dyn Error>> {
    let source = r#"stack 1.0
diagram "Projected group connection" {
  layout { direction right }
  group storage "Storage" {
    layout { direction down }
    group persistence "Persistence" { node database "Database" }
  }
  group platform "Platform" {
    layout { direction down }
    group application "Application" { node api "API" }
  }
  edge api -> database
}"#;
    let forward = scene_from(source)?;
    let reverse = scene_from(&source.replace("edge api -> database", "edge database -> api"))?;

    assert_progresses(
        group_rect(&forward, "platform")?,
        group_rect(&forward, "storage")?,
        SceneDirection::Right,
    );
    assert_progresses(
        group_rect(&reverse, "storage")?,
        group_rect(&reverse, "platform")?,
        SceneDirection::Right,
    );
    for scene in [&forward, &reverse] {
        for id in ["storage", "platform"] {
            let group = scene
                .groups
                .iter()
                .find(|group| group.id == id)
                .ok_or("missing directed group")?;
            assert_eq!(group.direction, SceneDirection::Down);
        }
        assert!(scene.geometry_is_valid());
    }
    Ok(())
}

#[test]
fn cyclic_graph_composition_is_deterministic() -> Result<(), Box<dyn Error>> {
    let source = r#"stack 1.0
diagram "Deterministic cycle" {
  layout { direction right }
  node downstream "Downstream"
  node second "Second"
  node upstream "Upstream"
  node first "First"
  edge upstream -> first
  edge first -> second
  edge second -> first
  edge second -> downstream
}"#;
    let first = scene_from(source)?;
    let second = scene_from(source)?;
    assert_eq!(first, second);
    assert!(first.geometry_is_valid());
    Ok(())
}

#[cfg(feature = "conformance")]
fn contains(outer: Rect, inner: Rect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
}

#[cfg(feature = "conformance")]
fn overlaps(left: Rect, right: Rect) -> bool {
    left.x < right.x + right.width
        && right.x < left.x + left.width
        && left.y < right.y + right.height
        && right.y < left.y + left.height
}

#[cfg(feature = "conformance")]
fn point_on_segment(point: Point, start: Point, end: Point) -> bool {
    if start.x == end.x {
        point.x == start.x && (start.y.min(end.y)..=start.y.max(end.y)).contains(&point.y)
    } else if start.y == end.y {
        point.y == start.y && (start.x.min(end.x)..=start.x.max(end.x)).contains(&point.x)
    } else {
        false
    }
}

#[cfg(feature = "conformance")]
fn segment_touches_rect(start: Point, end: Point, rect: Rect) -> bool {
    if start.x == end.x {
        (rect.x..=rect.x + rect.width).contains(&start.x)
            && start.y.min(end.y) <= rect.y + rect.height
            && start.y.max(end.y) >= rect.y
    } else if start.y == end.y {
        (rect.y..=rect.y + rect.height).contains(&start.y)
            && start.x.min(end.x) <= rect.x + rect.width
            && start.x.max(end.x) >= rect.x
    } else {
        // An unsupported segment is never accepted as collision-free.
        true
    }
}

#[test]
#[cfg(feature = "conformance")]
fn multilingual_labels_have_scene_rectangles_beside_their_routes_without_collisions_or_clipping()
-> Result<(), Box<dyn Error>> {
    let catalog = stack_theme::catalog();
    let theme = catalog
        .themes
        .iter()
        .find(|theme| theme.id == "light")
        .ok_or("missing light theme")?;
    let metrics = catalog
        .font_metrics
        .iter()
        .find(|metrics| metrics.id == theme.typography.font_metrics_id)
        .ok_or("missing light theme metrics")?;
    let source = include_str!("../../../layout-corpus/sources/multilingual-long-labels.stack");

    for direction in ["right", "down"] {
        let source = source.replace("direction down", &format!("direction {direction}"));
        let scene = scene_from(&source)?;
        let mut occupied_labels = Vec::new();
        for edge in &scene.edges {
            let label = edge.label.as_deref().ok_or("missing authored edge label")?;
            let rect = edge
                .label_rect
                .ok_or("label geometry must be resolved by the scene")?;
            let attachment = edge.label_anchor.ok_or("missing route attachment")?;
            assert!(rect.width > 0 && rect.height > 0);
            assert!(
                rect.width
                    >= scene::text_width(label, theme.typography.edge_label_size_milli_px, metrics),
                "the complete measured label must fit: {label}"
            );
            assert!(
                contains(scene.bounds, rect),
                "label must stay inside the canvas: {label} {rect:?}"
            );
            assert!(
                edge.path
                    .windows(2)
                    .any(|segment| point_on_segment(attachment, segment[0], segment[1])),
                "label attachment must stay on its own route: {label}"
            );
            let nearest_x = attachment.x.clamp(rect.x, rect.x + rect.width);
            let nearest_y = attachment.y.clamp(rect.y, rect.y + rect.height);
            let gap_x = (attachment.x - nearest_x).abs();
            let gap_y = (attachment.y - nearest_y).abs();
            assert!(
                (gap_x == 0 || gap_y == 0)
                    && gap_x + gap_y > 0
                    && gap_x + gap_y <= rect.width.min(rect.height),
                "label must remain immediately beside its attachment: {label} {rect:?} {attachment:?}"
            );
            for node in &scene.nodes {
                assert!(
                    !overlaps(rect, node.rect),
                    "label overlaps node {}: {label}",
                    node.id
                );
            }
            for occupied in &occupied_labels {
                assert!(!overlaps(rect, *occupied), "labels overlap: {label}");
            }
            for route in &scene.edges {
                assert!(
                    route
                        .path
                        .windows(2)
                        .all(|segment| !segment_touches_rect(segment[0], segment[1], rect)),
                    "route {} -> {} touches label {label}",
                    route.from,
                    route.to
                );
            }
            occupied_labels.push(rect);
        }
        assert_eq!(occupied_labels.len(), 3);
        assert!(scene.geometry_is_valid());
    }
    Ok(())
}

#[test]
fn long_labels_on_same_rank_connections_get_space_outside_the_node_column()
-> Result<(), Box<dyn Error>> {
    for direction in ["direction right", ""] {
        let source = format!(
            r#"stack 1.0 diagram "Flow" {{
            layout {{ {direction} rank same [a, b] }}
            node a "A" node b "B"
            edge a -> b "retry request after timeout"
        }}"#
        );
        let scene = scene_from(&source)?;
        assert!(scene.geometry_is_valid());
        assert!(scene.edges[0].label_rect.is_some());
    }
    Ok(())
}

#[test]
fn labelled_skip_edges_have_outer_lanes_separate_from_the_central_spine()
-> Result<(), Box<dyn Error>> {
    let source = r#"stack 1.0 diagram "Probe" {
        layout { direction down }
        node n0 "Node 0" node n1 "Node 1" node n2 "Node 2"
        edge n0 -> n1 "Connection 0"
        edge n1 -> n2 "Connection 0"
        edge n0 -> n2 "Connection 0"
    }"#;
    let scene = scene_from(source)?;
    assert!(scene.geometry_is_valid());
    assert!(scene.edges.iter().all(|edge| edge.label_rect.is_some()));
    Ok(())
}

#[test]
fn fanout_labels_reserve_space_before_later_routes_choose_nearby_lanes()
-> Result<(), Box<dyn Error>> {
    let scene = scene_from(
        r#"stack 1.0 diagram "Probe" {
        layout { direction down }
        node n0 "Node 0" node n1 "Node 1" node n2 "Node 2" node n3 "Node 3"
        edge n0 -> n3 "Connection 0"
        edge n0 -> n2 "Connection 1"
        edge n0 -> n1 "Connection 2"
    }"#,
    )?;
    assert!(scene.geometry_is_valid());
    assert!(scene.edges.iter().all(|edge| edge.label_rect.is_some()));
    Ok(())
}
