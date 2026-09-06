//! Regression metrics for concentrated terminals and routing lanes.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::path::Path;

use crate::Engine;
use crate::routing::{Marker, Point, SceneEdge};
use crate::scene::{self, Scene};

const SOURCE: &str = include_str!("../tests/fixtures/sample-service-congestion.stack");

#[derive(Debug, PartialEq, Eq)]
struct CongestionMetrics {
    reused_terminal_points: usize,
    proper_crossings: usize,
    crossing_pairs: Vec<String>,
    ambiguous_junctions: usize,
    junction_pairs: Vec<String>,
    shared_length_milli_px: i64,
    close_parallel_length_milli_px: i64,
}

fn sample_scene() -> Result<Scene, Box<dyn Error>> {
    let compiled = stack_compiler::compile_bytes(SOURCE.as_bytes());
    if !compiled.diagnostics.is_empty() {
        return Err(format!("fixture produced diagnostics: {:?}", compiled.diagnostics).into());
    }
    let diagram = compiled.diagram.ok_or("fixture produced no diagram")?;
    Ok(scene::layout(&diagram, stack_theme::catalog())?)
}

fn overlap_length(left_start: i64, left_end: i64, right_start: i64, right_end: i64) -> i64 {
    (left_start.max(left_end).min(right_start.max(right_end))
        - left_start.min(left_end).max(right_start.min(right_end)))
    .max(0)
}

fn segment_projection_overlap(left: [Point; 2], right: [Point; 2]) -> i64 {
    if left[0].y == left[1].y && right[0].y == right[1].y {
        overlap_length(left[0].x, left[1].x, right[0].x, right[1].x).max(0)
    } else if left[0].x == left[1].x && right[0].x == right[1].x {
        overlap_length(left[0].y, left[1].y, right[0].y, right[1].y).max(0)
    } else {
        0
    }
}

fn proper_crossing(left: [Point; 2], right: [Point; 2]) -> bool {
    let Some(point) = orthogonal_intersection(left, right) else {
        return false;
    };
    let (horizontal, vertical) = if left[0].y == left[1].y && right[0].x == right[1].x {
        (left, right)
    } else if right[0].y == right[1].y && left[0].x == left[1].x {
        (right, left)
    } else {
        return false;
    };
    point.x > horizontal[0].x.min(horizontal[1].x)
        && point.x < horizontal[0].x.max(horizontal[1].x)
        && point.y > vertical[0].y.min(vertical[1].y)
        && point.y < vertical[0].y.max(vertical[1].y)
}

fn orthogonal_intersection(left: [Point; 2], right: [Point; 2]) -> Option<Point> {
    let (horizontal, vertical) = if left[0].y == left[1].y && right[0].x == right[1].x {
        (left, right)
    } else if right[0].y == right[1].y && left[0].x == left[1].x {
        (right, left)
    } else {
        return None;
    };
    let point = Point {
        x: vertical[0].x,
        y: horizontal[0].y,
    };
    ((horizontal[0].x.min(horizontal[1].x)..=horizontal[0].x.max(horizontal[1].x))
        .contains(&point.x)
        && (vertical[0].y.min(vertical[1].y)..=vertical[0].y.max(vertical[1].y)).contains(&point.y))
    .then_some(point)
}

fn congestion_metrics(edges: &[SceneEdge]) -> CongestionMetrics {
    let mut terminals = BTreeMap::<Point, usize>::new();
    for edge in edges {
        if let Some(point) = edge.path.first() {
            *terminals.entry(*point).or_default() += 1;
        }
        if let Some(point) = edge.path.last() {
            *terminals.entry(*point).or_default() += 1;
        }
    }
    let reused_terminal_points = terminals.values().filter(|uses| **uses > 1).count();

    let mut proper_crossings = 0;
    let mut crossing_pairs = Vec::new();
    let mut junctions = BTreeSet::new();
    let mut shared_length_milli_px = 0;
    let mut close_parallel_length_milli_px = 0;
    for left_index in 0..edges.len() {
        for right_index in left_index + 1..edges.len() {
            for left in edges[left_index].path.windows(2) {
                for right in edges[right_index].path.windows(2) {
                    let left = [left[0], left[1]];
                    let right = [right[0], right[1]];
                    if proper_crossing(left, right) {
                        proper_crossings += 1;
                        crossing_pairs.push(format!(
                            "{} -> {} crosses {} -> {}",
                            edges[left_index].from,
                            edges[left_index].to,
                            edges[right_index].from,
                            edges[right_index].to
                        ));
                    } else if let Some(point) = orthogonal_intersection(left, right) {
                        junctions.insert((left_index, right_index, point));
                    }
                    let overlap = segment_projection_overlap(left, right);
                    if overlap == 0 {
                        continue;
                    }
                    let separation = if left[0].y == left[1].y && right[0].y == right[1].y {
                        (left[0].y - right[0].y).abs()
                    } else if left[0].x == left[1].x && right[0].x == right[1].x {
                        (left[0].x - right[0].x).abs()
                    } else {
                        continue;
                    };
                    if separation == 0 {
                        shared_length_milli_px += overlap;
                    } else if separation < 16_000 {
                        close_parallel_length_milli_px += overlap;
                    }
                }
            }
        }
    }

    let junction_pairs = junctions
        .iter()
        .map(|(left_index, right_index, point)| {
            format!(
                "{} -> {} joins {} -> {} at ({}, {})",
                edges[*left_index].from,
                edges[*left_index].to,
                edges[*right_index].from,
                edges[*right_index].to,
                point.x,
                point.y
            )
        })
        .collect::<Vec<_>>();

    CongestionMetrics {
        reused_terminal_points,
        proper_crossings,
        crossing_pairs,
        ambiguous_junctions: junctions.len(),
        junction_pairs,
        shared_length_milli_px,
        close_parallel_length_milli_px,
    }
}

fn write_candidate_svg() -> Result<(), Box<dyn Error>> {
    let output = Engine::bundled().render(SOURCE.as_bytes())?;
    if !output.diagnostics.is_empty() {
        return Err(format!("fixture produced diagnostics: {:?}", output.diagnostics).into());
    }
    let output_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/layout-congestion");
    fs::create_dir_all(&output_root)?;
    fs::write(
        output_root.join("sample-service.svg"),
        output.svg.ok_or("fixture produced no SVG")?,
    )?;
    Ok(())
}

#[test]
fn sample_service_avoids_terminal_and_lane_congestion() -> Result<(), Box<dyn Error>> {
    let scene = sample_scene()?;
    let metrics = congestion_metrics(&scene.edges);
    write_candidate_svg()?;

    assert!(scene.geometry_is_valid());
    assert_eq!(metrics.proper_crossings, 0, "{metrics:#?}");
    assert_eq!(metrics.ambiguous_junctions, 0, "{metrics:#?}");
    assert_eq!(metrics.reused_terminal_points, 0, "{metrics:#?}");
    assert_eq!(metrics.shared_length_milli_px, 0, "{metrics:#?}");
    assert_eq!(metrics.close_parallel_length_milli_px, 0, "{metrics:#?}");
    Ok(())
}

#[test]
fn interval_overlap_is_orientation_independent_and_excludes_touching() {
    for (left, right, expected) in [
        ((0, 10), (5, 15), 5),
        ((10, 0), (5, 15), 5),
        ((0, 10), (15, 5), 5),
        ((10, 0), (15, 5), 5),
        ((0, 20), (5, 15), 10),
        ((0, 10), (10, 20), 0),
        ((0, 10), (11, 20), 0),
    ] {
        assert_eq!(overlap_length(left.0, left.1, right.0, right.1), expected);
    }
}

#[test]
fn intersection_metric_distinguishes_crossings_junctions_and_disjoint_segments() {
    let horizontal = [Point { x: 0, y: 10 }, Point { x: 20, y: 10 }];
    let crossing = [Point { x: 10, y: 0 }, Point { x: 10, y: 20 }];
    let tee = [Point { x: 20, y: 0 }, Point { x: 20, y: 20 }];
    let elbow = [Point { x: 20, y: 10 }, Point { x: 20, y: 30 }];
    let disjoint = [Point { x: 30, y: 0 }, Point { x: 30, y: 20 }];

    assert!(proper_crossing(horizontal, crossing));
    assert!(proper_crossing(crossing, horizontal));
    assert_eq!(
        orthogonal_intersection(horizontal, crossing),
        Some(Point { x: 10, y: 10 })
    );
    assert!(!proper_crossing(horizontal, tee));
    assert_eq!(
        orthogonal_intersection(horizontal, tee),
        Some(Point { x: 20, y: 10 })
    );
    assert!(!proper_crossing(horizontal, elbow));
    assert_eq!(
        orthogonal_intersection(horizontal, elbow),
        Some(Point { x: 20, y: 10 })
    );
    assert_eq!(orthogonal_intersection(horizontal, disjoint), None);
}

#[test]
fn shared_segment_metric_is_orientation_independent() {
    let forward = [Point { x: 0, y: 10 }, Point { x: 20, y: 10 }];
    let reverse = [Point { x: 15, y: 10 }, Point { x: 5, y: 10 }];
    let separated = [Point { x: 5, y: 20 }, Point { x: 15, y: 20 }];

    assert_eq!(segment_projection_overlap(forward, reverse), 10);
    assert_eq!(segment_projection_overlap(forward, separated), 10);
}

fn metric_edge(from: &str, to: &str, path: Vec<Point>) -> SceneEdge {
    SceneEdge {
        from: from.to_owned(),
        to: to.to_owned(),
        direction: stack_compiler::ir::EdgeDirection::Forward,
        kind: stack_compiler::ir::EdgeKind::Flow,
        label: None,
        path,
        start_marker: Marker::None,
        end_marker: Marker::Arrow,
        label_anchor: None,
        label_rect: None,
    }
}

#[test]
fn congestion_metrics_report_each_independent_failure_signal() {
    let edges = [
        metric_edge(
            "a",
            "b",
            vec![Point { x: 0, y: 0 }, Point { x: 20_000, y: 0 }],
        ),
        metric_edge(
            "c",
            "d",
            vec![
                Point {
                    x: 10_000,
                    y: -10_000,
                },
                Point {
                    x: 10_000,
                    y: 10_000,
                },
            ],
        ),
        metric_edge(
            "e",
            "f",
            vec![
                Point {
                    x: 20_000,
                    y: -10_000,
                },
                Point { x: 20_000, y: 0 },
            ],
        ),
        metric_edge(
            "g",
            "h",
            vec![Point { x: 5_000, y: 0 }, Point { x: 15_000, y: 0 }],
        ),
        metric_edge(
            "i",
            "j",
            vec![
                Point {
                    x: 5_000,
                    y: 10_000,
                },
                Point {
                    x: 15_000,
                    y: 10_000,
                },
            ],
        ),
    ];
    let metrics = congestion_metrics(&edges);

    assert!(metrics.reused_terminal_points > 0, "{metrics:#?}");
    assert!(metrics.proper_crossings > 0, "{metrics:#?}");
    assert!(!metrics.crossing_pairs.is_empty(), "{metrics:#?}");
    assert!(metrics.ambiguous_junctions > 0, "{metrics:#?}");
    assert!(!metrics.junction_pairs.is_empty(), "{metrics:#?}");
    assert!(metrics.shared_length_milli_px > 0, "{metrics:#?}");
    assert!(metrics.close_parallel_length_milli_px > 0, "{metrics:#?}");
}

#[test]
fn congestion_metrics_accept_an_edge_without_routed_points() {
    let metrics = congestion_metrics(&[metric_edge("missing", "route", Vec::new())]);
    assert_eq!(
        metrics,
        CongestionMetrics {
            reused_terminal_points: 0,
            proper_crossings: 0,
            crossing_pairs: Vec::new(),
            ambiguous_junctions: 0,
            junction_pairs: Vec::new(),
            shared_length_milli_px: 0,
            close_parallel_length_milli_px: 0,
        }
    );
}
