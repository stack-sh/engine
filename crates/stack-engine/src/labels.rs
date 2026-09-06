//! Deterministic label placement adjacent to each edge's own route.

use std::cmp::Reverse;

use stack_theme::{FontMetrics, Theme};

use crate::routing::{Point, RoutingError, SceneEdge};
use crate::scene::{self, Rect, SceneNode};

const HORIZONTAL_PADDING: i64 = 6_000;
const VERTICAL_PADDING: i64 = 4_000;
const ROUTE_GAP: i64 = 8_000;

// Preserve core geometry while keeping labels outside thicker custom strokes.
fn route_gap(stroke_width: i64) -> i64 {
    ROUTE_GAP.max(stroke_width / 2 + stroke_width % 2 + 1_000)
}

#[derive(Clone, Copy)]
struct Placement {
    rect: Rect,
    attachment: Point,
}

pub(crate) fn dimensions(label: &str, theme: &Theme, metrics: &FontMetrics) -> Rect {
    let typography = &theme.typography;
    Rect {
        x: 0,
        y: 0,
        width: scene::text_width(label, typography.edge_label_size_milli_px, metrics)
            + 2 * HORIZONTAL_PADDING,
        height: scene::line_height(typography.edge_label_size_milli_px, typography)
            + 2 * VERTICAL_PADDING,
    }
}

pub(crate) fn place(
    edges: &mut [SceneEdge],
    nodes: &[SceneNode],
    bounds: Rect,
    fixed_text: &[Rect],
    theme: &Theme,
    metrics: &FontMetrics,
) -> Result<(), RoutingError> {
    let mut order = edges
        .iter()
        .enumerate()
        .filter_map(|(index, edge)| {
            edge.label
                .as_ref()
                .map(|label| (index, dimensions(label, theme, metrics)))
        })
        .collect::<Vec<_>>();
    order.sort_by_key(|(index, size)| (Reverse(size.width), *index));
    let mut occupied = nodes.iter().map(|node| node.rect).collect::<Vec<_>>();
    occupied.extend_from_slice(fixed_text);
    let mut placements = vec![None; edges.len()];
    let stroke_width = i64::from(theme.connector.width_milli_px);

    for (index, size) in order {
        let placement = candidates(&edges[index].path, size, bounds, route_gap(stroke_width))
            .into_iter()
            .find(|candidate| rect_is_clear(candidate.rect, bounds, &occupied, edges, stroke_width))
            .ok_or(RoutingError)?;
        occupied.push(placement.rect);
        placements[index] = Some(placement);
    }

    // Commit only a complete placement so a failed attempt leaves its input intact.
    for (edge, placement) in edges.iter_mut().zip(placements) {
        edge.label_rect = placement.map(|placement| placement.rect);
        edge.label_anchor = placement.map(|placement| placement.attachment);
    }
    Ok(())
}

pub(crate) fn place_next(
    edge: &mut SceneEdge,
    previous: &[SceneEdge],
    nodes: &[SceneNode],
    bounds: Rect,
    fixed_text: &[Rect],
    theme: &Theme,
    metrics: &FontMetrics,
) -> Result<(), RoutingError> {
    let Some(label) = edge.label.as_ref() else {
        edge.label_rect = None;
        edge.label_anchor = None;
        return Ok(());
    };
    let mut occupied = nodes.iter().map(|node| node.rect).collect::<Vec<_>>();
    occupied.extend_from_slice(fixed_text);
    occupied.extend(previous.iter().filter_map(|edge| edge.label_rect));
    let stroke_width = i64::from(theme.connector.width_milli_px);
    let placement = candidates(
        &edge.path,
        dimensions(label, theme, metrics),
        bounds,
        route_gap(stroke_width),
    )
    .into_iter()
    .find(|candidate| {
        rect_is_clear(candidate.rect, bounds, &occupied, previous, stroke_width)
            && rect_is_clear(
                candidate.rect,
                bounds,
                &[],
                std::slice::from_ref(edge),
                stroke_width,
            )
    })
    .ok_or(RoutingError)?;
    edge.label_rect = Some(placement.rect);
    edge.label_anchor = Some(placement.attachment);
    Ok(())
}

pub(crate) fn geometry_is_valid(
    edges: &[SceneEdge],
    nodes: &[SceneNode],
    bounds: Rect,
    fixed_text: &[Rect],
    stroke_width: i64,
) -> bool {
    let mut occupied = nodes.iter().map(|node| node.rect).collect::<Vec<_>>();
    occupied.extend_from_slice(fixed_text);
    for edge in edges {
        match (&edge.label, edge.label_rect, edge.label_anchor) {
            (None, None, None) => {}
            (Some(_), Some(rect), Some(attachment)) => {
                if !rect_is_clear(rect, bounds, &occupied, edges, stroke_width)
                    || !edge.path.windows(2).any(|segment| {
                        is_adjacent(
                            rect,
                            attachment,
                            segment[0],
                            segment[1],
                            route_gap(stroke_width),
                        )
                    })
                {
                    return false;
                }
                occupied.push(rect);
            }
            _ => return false,
        }
    }
    true
}

fn candidates(path: &[Point], size: Rect, bounds: Rect, gap: i64) -> Vec<Placement> {
    let mut segments = path.windows(2).enumerate().collect::<Vec<_>>();
    segments.sort_by_key(|(index, segment)| {
        (
            Reverse((segment[1].x - segment[0].x).abs() + (segment[1].y - segment[0].y).abs()),
            *index,
        )
    });
    let mut candidates = Vec::new();
    for (_, segment) in segments {
        let start = segment[0];
        let end = segment[1];
        if start == end {
            continue;
        }
        if start.y == end.y {
            let positions = axis_positions(start.x, end.x, size.width, bounds.x, bounds.width);
            for y in [start.y - gap - size.height, start.y + gap] {
                for &x in &positions {
                    candidates.push(Placement {
                        rect: Rect {
                            x: x - size.width / 2,
                            y,
                            ..size
                        },
                        attachment: Point { x, y: start.y },
                    });
                }
            }
        } else if start.x == end.x {
            let positions = axis_positions(start.y, end.y, size.height, bounds.y, bounds.height);
            for x in [start.x + gap, start.x - gap - size.width] {
                for &y in &positions {
                    candidates.push(Placement {
                        rect: Rect {
                            x,
                            y: y - size.height / 2,
                            ..size
                        },
                        attachment: Point { x: start.x, y },
                    });
                }
            }
        }
    }
    candidates
}

fn axis_positions(start: i64, end: i64, size: i64, origin: i64, extent: i64) -> Vec<i64> {
    let low = start.min(end);
    let high = start.max(end);
    let minimum = (low + 1).max(origin + size / 2);
    let maximum = (high - 1).min(origin + extent - (size - size / 2));
    if size <= 0 || minimum > maximum {
        return Vec::new();
    }
    let length = high - low;
    let mut positions = Vec::new();
    for preferred in [
        low + length / 2,
        low + length / 3,
        low + 2 * length / 3,
        low + size / 2,
        high - (size - size / 2),
    ] {
        let position = preferred.clamp(minimum, maximum);
        if !positions.contains(&position) {
            positions.push(position);
        }
    }
    positions
}

fn is_adjacent(rect: Rect, attachment: Point, start: Point, end: Point, gap: i64) -> bool {
    if start.y == end.y && start.x != end.x {
        attachment.y == start.y
            && attachment.x > start.x.min(end.x)
            && attachment.x < start.x.max(end.x)
            && attachment.x == rect.x + rect.width / 2
            && (rect.y + rect.height + gap == attachment.y || rect.y - gap == attachment.y)
    } else if start.x == end.x && start.y != end.y {
        attachment.x == start.x
            && attachment.y > start.y.min(end.y)
            && attachment.y < start.y.max(end.y)
            && attachment.y == rect.y + rect.height / 2
            && (rect.x - gap == attachment.x || rect.x + rect.width + gap == attachment.x)
    } else {
        false
    }
}

fn rect_is_clear(
    rect: Rect,
    bounds: Rect,
    occupied: &[Rect],
    edges: &[SceneEdge],
    stroke_width: i64,
) -> bool {
    rect.width > 0
        && rect.height > 0
        && stroke_width >= 0
        && rect.x >= bounds.x
        && rect.y >= bounds.y
        && rect.x + rect.width <= bounds.x + bounds.width
        && rect.y + rect.height <= bounds.y + bounds.height
        && occupied.iter().all(|other| !rects_touch(rect, *other))
        && edges.iter().all(|edge| {
            edge.path.windows(2).all(|segment| {
                !stroke_hits_rect(segment[0], segment[1], rect, (stroke_width + 1) / 2)
            })
        })
}

fn rects_touch(left: Rect, right: Rect) -> bool {
    left.x <= right.x + right.width
        && right.x <= left.x + left.width
        && left.y <= right.y + right.height
        && right.y <= left.y + left.height
}

fn stroke_hits_rect(start: Point, end: Point, rect: Rect, radius: i64) -> bool {
    if start == end || (start.x != end.x && start.y != end.y) {
        return true;
    }
    let envelope = Rect {
        x: start.x.min(end.x) - radius,
        y: start.y.min(end.y) - radius,
        width: (end.x - start.x).abs() + 2 * radius,
        height: (end.y - start.y).abs() + 2 * radius,
    };
    rects_touch(rect, envelope)
}

#[cfg(test)]
mod tests {
    use stack_compiler::ir::{EdgeDirection, EdgeKind};

    use super::{dimensions, geometry_is_valid, place, place_next};
    use crate::routing::{Marker, Point, SceneEdge};
    use crate::scene::{self, Rect, SceneNode};

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    fn resources() -> TestResult<(
        &'static stack_theme::Theme,
        &'static stack_theme::FontMetrics,
    )> {
        let catalog = stack_theme::catalog();
        let theme = catalog
            .themes
            .first()
            .ok_or("bundled catalog has no theme")?;
        let metrics = catalog
            .font_metrics
            .iter()
            .find(|metrics| metrics.id == theme.typography.font_metrics_id)
            .ok_or("bundled theme has no font metrics")?;
        Ok((theme, metrics))
    }

    fn bounds() -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: 800_000,
            height: 600_000,
        }
    }

    fn edge(label: Option<&str>, path: &[(i64, i64)]) -> SceneEdge {
        SceneEdge {
            from: "source".to_owned(),
            to: "target".to_owned(),
            direction: EdgeDirection::Forward,
            kind: EdgeKind::Flow,
            label: label.map(str::to_owned),
            path: path.iter().map(|&(x, y)| Point { x, y }).collect(),
            start_marker: Marker::None,
            end_marker: Marker::Arrow,
            label_anchor: None,
            label_rect: None,
        }
    }

    fn place_labels(edges: &mut [SceneEdge], nodes: &[SceneNode], fixed: &[Rect]) -> TestResult {
        let (theme, metrics) = resources()?;
        place(edges, nodes, bounds(), fixed, theme, metrics)
            .map_err(|_| "labels do not fit synthetic scene")?;
        assert!(geometry_is_valid(
            edges,
            nodes,
            bounds(),
            fixed,
            i64::from(theme.connector.width_milli_px),
        ));
        Ok(())
    }

    #[test]
    fn dimensions_use_font_metrics_and_background_padding() -> TestResult {
        let (theme, metrics) = resources()?;
        let measured = dimensions("Read records", theme, metrics);
        assert_eq!(measured.x, 0);
        assert_eq!(measured.y, 0);
        assert_eq!(
            measured.width,
            scene::text_width(
                "Read records",
                theme.typography.edge_label_size_milli_px,
                metrics
            ) + 12_000,
        );
        assert_eq!(
            measured.height,
            scene::line_height(theme.typography.edge_label_size_milli_px, &theme.typography)
                + 8_000,
        );
        Ok(())
    }

    #[test]
    fn wide_connector_labels_clear_their_own_painted_stroke() -> TestResult {
        let (theme, metrics) = resources()?;
        for width in [1_500, 15_000, 15_999, 16_000, 32_000] {
            let mut theme = theme.clone();
            theme.connector.width_milli_px = width;
            let mut edges = [edge(
                Some("Call"),
                &[(100_000, 300_000), (700_000, 300_000)],
            )];
            place(&mut edges, &[], bounds(), &[], &theme, metrics)
                .map_err(|_| "wide connector label failed")?;
            assert!(geometry_is_valid(
                &edges,
                &[],
                bounds(),
                &[],
                i64::from(width)
            ));
            let label = edges[0].label_rect.ok_or("missing label rectangle")?;
            let gap = 300_000 - label.y - label.height;
            assert!(gap > (i64::from(width) + 1) / 2);
            if width == 1_500 {
                assert_eq!(gap, 8_000);
            }
        }
        Ok(())
    }

    #[test]
    fn malformed_routes_cannot_produce_valid_label_attachments() -> TestResult {
        let (theme, metrics) = resources()?;
        for path in [
            vec![],
            vec![(100_000, 100_000)],
            vec![(100_000, 100_000), (100_000, 100_000)],
            vec![(100_000, 100_000), (200_000, 200_000)],
        ] {
            let original = edge(Some("Call"), &path);
            let mut candidate = original.clone();
            assert!(place_next(&mut candidate, &[], &[], bounds(), &[], theme, metrics).is_err());
            assert_eq!(candidate, original);
            assert!(!geometry_is_valid(&[candidate], &[], bounds(), &[], 1_500));
        }
        let rect = Rect {
            x: 200_000,
            y: 200_000,
            width: 50_000,
            height: 30_000,
        };
        let start = Point {
            x: 100_000,
            y: 100_000,
        };
        let end = Point {
            x: 300_000,
            y: 300_000,
        };
        assert!(super::stroke_hits_rect(start, end, rect, 1_500));
        assert!(super::stroke_hits_rect(start, start, rect, -1));
        assert!(!super::is_adjacent(rect, start, start, end, 8_000));
        assert!(!super::is_adjacent(rect, start, start, start, 8_000));
        Ok(())
    }

    #[test]
    fn horizontal_label_prefers_above_with_its_attachment_on_the_segment() -> TestResult {
        let mut edges = [edge(
            Some("HTTPS"),
            &[(100_000, 200_000), (600_000, 200_000)],
        )];
        place_labels(&mut edges, &[], &[])?;
        let rect = edges[0].label_rect.ok_or("missing label rectangle")?;
        let anchor = edges[0].label_anchor.ok_or("missing label attachment")?;
        assert_eq!(
            anchor,
            Point {
                x: 350_000,
                y: 200_000
            }
        );
        assert_eq!(rect.x + rect.width / 2, anchor.x);
        assert_eq!(rect.y + rect.height + 8_000, anchor.y);
        Ok(())
    }

    #[test]
    fn long_vertical_label_uses_reserved_space_on_the_right() -> TestResult {
        let mut edges = [edge(
            Some("A long description of the database connection"),
            &[(100_000, 100_000), (100_000, 500_000)],
        )];
        place_labels(&mut edges, &[], &[])?;
        let rect = edges[0].label_rect.ok_or("missing label rectangle")?;
        let anchor = edges[0].label_anchor.ok_or("missing label attachment")?;
        assert_eq!(rect.x, 108_000);
        assert_eq!(rect.y + rect.height / 2, anchor.y);
        assert_eq!(anchor.x, 100_000);
        Ok(())
    }

    #[test]
    fn unlabeled_routes_block_label_backgrounds() -> TestResult {
        let mut edges = [
            edge(Some("HTTPS"), &[(100_000, 200_000), (600_000, 200_000)]),
            edge(None, &[(0, 180_000), (800_000, 180_000)]),
        ];
        place_labels(&mut edges, &[], &[])?;
        assert!(edges[0].label_rect.ok_or("missing label rectangle")?.y > 200_000);
        assert_eq!(edges[1].label_anchor, None);
        assert_eq!(edges[1].label_rect, None);
        Ok(())
    }

    #[test]
    fn nodes_and_titles_can_force_a_non_midpoint_candidate() -> TestResult {
        let mut edges = [edge(
            Some("HTTPS"),
            &[(100_000, 300_000), (700_000, 300_000)],
        )];
        let nodes = [SceneNode {
            id: "blocker".to_owned(),
            parent_group_id: None,
            rect: Rect {
                x: 350_000,
                y: 230_000,
                width: 100_000,
                height: 62_000,
            },
        }];
        let titles = [Rect {
            x: 0,
            y: 308_000,
            width: 800_000,
            height: 80_000,
        }];
        place_labels(&mut edges, &nodes, &titles)?;
        assert_ne!(
            edges[0].label_anchor.ok_or("missing label attachment")?.x,
            400_000
        );
        Ok(())
    }

    #[test]
    fn labels_avoid_each_other_without_reordering_edges() -> TestResult {
        let mut edges = [
            edge(
                Some("First connection"),
                &[(100_000, 250_000), (700_000, 250_000)],
            ),
            edge(
                Some("Second connection"),
                &[(100_000, 250_000), (700_000, 250_000)],
            ),
            edge(Some("Third"), &[(100_000, 250_000), (700_000, 250_000)]),
        ];
        let original = edges.clone();
        place_labels(&mut edges, &[], &[])?;
        for (index, placed) in edges.iter().enumerate() {
            assert_eq!(placed.label, original[index].label);
            assert_eq!(placed.path, original[index].path);
            assert_eq!(placed.direction, original[index].direction);
        }
        let mut repeated = original;
        place_labels(&mut repeated, &[], &[])?;
        assert_eq!(edges, repeated);
        Ok(())
    }

    #[test]
    fn placement_failure_does_not_send_a_label_to_unrelated_distant_space() -> TestResult {
        let (theme, metrics) = resources()?;
        let mut edges = [edge(
            Some("No room"),
            &[(200_000, 200_000), (300_000, 200_000)],
        )];
        let fixed = [Rect {
            x: 100_000,
            y: 100_000,
            width: 300_000,
            height: 200_000,
        }];
        let original = edges.clone();
        assert!(place(&mut edges, &[], bounds(), &fixed, theme, metrics).is_err());
        assert_eq!(edges, original);
        Ok(())
    }

    #[test]
    fn labels_at_canvas_edges_keep_the_attachment_inside_the_segment() -> TestResult {
        let mut edges = [edge(Some("Read"), &[(0, 10_000), (100_000, 10_000)])];
        place_labels(&mut edges, &[], &[])?;
        let rect = edges[0].label_rect.ok_or("missing label rectangle")?;
        let anchor = edges[0].label_anchor.ok_or("missing label attachment")?;
        assert!(rect.x >= 0);
        assert!(rect.y > 10_000);
        assert!(anchor.x > 0 && anchor.x < 100_000);
        Ok(())
    }

    #[test]
    fn labels_too_wide_for_the_canvas_report_failure() -> TestResult {
        let (theme, metrics) = resources()?;
        let label = "A connection description ".repeat(50);
        let mut edges = [edge(
            Some(&label),
            &[(100_000, 100_000), (100_000, 500_000)],
        )];
        assert!(place(&mut edges, &[], bounds(), &[], theme, metrics).is_err());
        Ok(())
    }

    #[test]
    fn validator_rejects_labels_outside_bounds_or_touching_fixed_text() -> TestResult {
        let (theme, _) = resources()?;
        let width = i64::from(theme.connector.width_milli_px);
        let mut edges = [edge(
            Some("HTTPS"),
            &[(100_000, 200_000), (600_000, 200_000)],
        )];
        place_labels(&mut edges, &[], &[])?;
        let rect = edges[0].label_rect.ok_or("missing label rectangle")?;
        let small_bounds = Rect {
            height: 100_000,
            ..bounds()
        };
        assert!(!geometry_is_valid(&edges, &[], small_bounds, &[], width));
        let title = Rect {
            y: rect.y - rect.height,
            ..rect
        };
        assert!(!geometry_is_valid(&edges, &[], bounds(), &[title], width));
        Ok(())
    }

    #[test]
    fn validator_rejects_a_label_missing_its_rectangle_or_attached_far_away() -> TestResult {
        let (theme, _) = resources()?;
        let width = i64::from(theme.connector.width_milli_px);
        let mut edges = [edge(
            Some("HTTPS"),
            &[(100_000, 200_000), (600_000, 200_000)],
        )];
        place_labels(&mut edges, &[], &[])?;
        let original = edges.clone();
        edges[0].label_rect = None;
        assert!(!geometry_is_valid(&edges, &[], bounds(), &[], width));
        edges = original.clone();
        edges[0].label_anchor = Some(Point {
            x: 350_000,
            y: 400_000,
        });
        assert!(!geometry_is_valid(&edges, &[], bounds(), &[], width));
        edges = original;
        edges[0]
            .label_rect
            .as_mut()
            .ok_or("missing label rectangle")?
            .y -= 100_000;
        assert!(!geometry_is_valid(&edges, &[], bounds(), &[], width));
        Ok(())
    }

    #[test]
    fn validator_rejects_contact_with_the_stroke_envelope() -> TestResult {
        let mut edges = vec![edge(
            Some("HTTPS"),
            &[(100_000, 200_000), (600_000, 200_000)],
        )];
        place_labels(&mut edges, &[], &[])?;
        let rect = edges[0].label_rect.ok_or("missing label rectangle")?;
        edges.push(edge(
            None,
            &[(0, rect.y - 1_000), (800_000, rect.y - 1_000)],
        ));
        assert!(!geometry_is_valid(&edges, &[], bounds(), &[], 2_000));
        Ok(())
    }

    #[test]
    fn validator_rejects_overlapping_labels_and_unlabeled_rectangles() -> TestResult {
        let (theme, _) = resources()?;
        let width = i64::from(theme.connector.width_milli_px);
        let mut edges = vec![edge(
            Some("HTTPS"),
            &[(100_000, 200_000), (600_000, 200_000)],
        )];
        place_labels(&mut edges, &[], &[])?;
        edges.push(edges[0].clone());
        assert!(!geometry_is_valid(&edges, &[], bounds(), &[], width));
        edges.pop();
        edges[0].label = None;
        assert!(!geometry_is_valid(&edges, &[], bounds(), &[], width));
        Ok(())
    }

    #[test]
    fn place_next_preserves_previous_labels_and_routes() -> TestResult {
        let (theme, metrics) = resources()?;
        let mut previous = vec![edge(
            Some("Already placed"),
            &[(100_000, 250_000), (700_000, 250_000)],
        )];
        place_labels(&mut previous, &[], &[])?;
        let original = previous.clone();
        let mut next = edge(
            Some("Next connection"),
            &[(100_000, 250_000), (700_000, 250_000)],
        );
        let original_next = next.clone();
        place_next(&mut next, &previous, &[], bounds(), &[], theme, metrics)
            .map_err(|_| "next label does not fit")?;
        assert_eq!(previous, original);
        assert_eq!(next.path, original_next.path);
        assert_eq!(next.label, original_next.label);
        assert_ne!(next.label_rect, previous[0].label_rect);
        previous.push(next);
        assert!(geometry_is_valid(
            &previous,
            &[],
            bounds(),
            &[],
            i64::from(theme.connector.width_milli_px),
        ));
        Ok(())
    }

    #[test]
    fn place_next_avoids_previous_labels_all_routes_nodes_and_titles() -> TestResult {
        let (theme, metrics) = resources()?;
        let nodes = [SceneNode {
            id: "blocker".to_owned(),
            parent_group_id: None,
            rect: Rect {
                x: 460_000,
                y: 208_000,
                width: 150_000,
                height: 37_000,
            },
        }];
        let titles = [Rect {
            x: 0,
            y: 208_000,
            width: 340_000,
            height: 37_000,
        }];
        let mut previous = vec![
            edge(Some("Old"), &[(100_000, 250_000), (700_000, 250_000)]),
            edge(None, &[(0, 180_000), (800_000, 180_000)]),
        ];
        place_labels(&mut previous, &nodes, &titles)?;
        let mut next = edge(Some("HTTPS"), &[(100_000, 200_000), (700_000, 200_000)]);
        place_next(
            &mut next,
            &previous,
            &nodes,
            bounds(),
            &titles,
            theme,
            metrics,
        )
        .map_err(|_| "next label does not fit beside the remaining segment")?;
        let rect = next.label_rect.ok_or("missing next label rectangle")?;
        assert!(rect.x > 610_000);
        assert!(rect.y > 200_000);
        previous.push(next);
        assert!(geometry_is_valid(
            &previous,
            &nodes,
            bounds(),
            &titles,
            i64::from(theme.connector.width_milli_px),
        ));
        Ok(())
    }

    #[test]
    fn place_next_failure_preserves_the_entire_new_edge() -> TestResult {
        let (theme, metrics) = resources()?;
        let mut next = edge(Some("No room"), &[(200_000, 200_000), (300_000, 200_000)]);
        place_labels(std::slice::from_mut(&mut next), &[], &[])?;
        let original = next.clone();
        let occupied = [Rect {
            x: 100_000,
            y: 100_000,
            width: 300_000,
            height: 200_000,
        }];
        assert!(place_next(&mut next, &[], &[], bounds(), &occupied, theme, metrics).is_err());
        assert_eq!(next, original);
        Ok(())
    }

    #[test]
    fn place_next_leaves_an_unlabeled_edge_without_label_metadata() -> TestResult {
        let (theme, metrics) = resources()?;
        let mut next = edge(Some("Old label"), &[(100_000, 200_000), (700_000, 200_000)]);
        place_labels(std::slice::from_mut(&mut next), &[], &[])?;
        next.label = None;
        let original_path = next.path.clone();
        place_next(&mut next, &[], &[], bounds(), &[], theme, metrics)
            .map_err(|_| "unlabeled edge placement failed")?;
        assert_eq!(next.label_rect, None);
        assert_eq!(next.label_anchor, None);
        assert_eq!(next.path, original_path);
        Ok(())
    }
}
