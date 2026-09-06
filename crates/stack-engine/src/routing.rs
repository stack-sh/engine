//! Deterministic orthogonal edge routing for the internal scene.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

use stack_compiler::ir::{Edge, EdgeDirection, EdgeKind};

use crate::scene::{Rect, SceneNode};

const ROUTE_MARGIN: i64 = 8_000;
// Keep the marker clear of a preceding orthogonal bend. The largest built-in
// arrow is 7.5px long, so a 16px terminal lead-in leaves about 8px of visible
// connector before the marker footprint.
const ARROW_TERMINAL_STUB_LENGTH: i64 = 16_000;
const BEND_PENALTY: i64 = 32_000;
const CROSSING_PENALTY: i64 = 48_000;
const SHARED_LENGTH_PENALTY: i64 = 3;
const PORT_REUSE_PENALTY: i64 = 96_000;
const OFF_CENTER_PORT_PENALTY: i64 = 2_000;
const ALTERNATIVE_PORT_PAIR_LIMIT: usize = 32;
const FRAME_CLEARANCE: i64 = 16_000;
// Reserve two extra pixels for the core connector and frame stroke radii.
// The independent SVG gate uses the actual painted stroke widths.
const FRAME_MARGIN: i64 = FRAME_CLEARANCE + 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Point {
    pub(crate) x: i64,
    pub(crate) y: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Marker {
    None,
    Arrow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SceneEdge {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) direction: EdgeDirection,
    pub(crate) kind: EdgeKind,
    pub(crate) label: Option<String>,
    pub(crate) path: Vec<Point>,
    pub(crate) start_marker: Marker,
    pub(crate) end_marker: Marker,
    pub(crate) label_anchor: Option<Point>,
    pub(crate) label_rect: Option<Rect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RoutingError;

pub(crate) fn route(
    edges: &[Edge],
    nodes: &[SceneNode],
    bounds: Rect,
    fixed_obstacles: &[Rect],
    frames: &[Rect],
) -> Result<Vec<SceneEdge>, RoutingError> {
    let mut router = GridRouter::new_for_edges(nodes, bounds, fixed_obstacles, frames, edges);
    edges
        .iter()
        .map(|edge| {
            let source = node_rect(nodes, &edge.from).ok_or(RoutingError)?;
            let target = node_rect(nodes, &edge.to).ok_or(RoutingError)?;
            let path = router
                .route(source, target, edge.direction)
                .ok_or(RoutingError)?;
            Ok(scene_edge(edge, path))
        })
        .collect()
}

pub(crate) fn route_next(
    edge: &Edge,
    context_edges: &[Edge],
    reserved_edges: &[SceneEdge],
    nodes: &[SceneNode],
    bounds: Rect,
    fixed_obstacles: &[Rect],
    frames: &[Rect],
) -> Result<SceneEdge, RoutingError> {
    let source = node_rect(nodes, &edge.from).ok_or(RoutingError)?;
    let target = node_rect(nodes, &edge.to).ok_or(RoutingError)?;
    let mut router = GridRouter::new_for_edge(
        nodes,
        bounds,
        fixed_obstacles,
        frames,
        context_edges,
        edge,
        reserved_edges,
    );
    router.reserve_edges(reserved_edges);
    let path = router
        .route(source, target, edge.direction)
        .ok_or(RoutingError)?;
    Ok(scene_edge(edge, path))
}

pub(crate) fn geometry_is_valid(
    edges: &[SceneEdge],
    nodes: &[SceneNode],
    bounds: Rect,
    frames: &[Rect],
) -> bool {
    edges.iter().all(|edge| {
        let Some(source) = node_rect(nodes, &edge.from) else {
            return false;
        };
        let Some(target) = node_rect(nodes, &edge.to) else {
            return false;
        };
        if edge.path.len() < 2
            || !source.has_boundary_point(edge.path[0])
            || !target.has_boundary_point(edge.path[edge.path.len() - 1])
            || edge.path.iter().any(|point| !bounds.contains_point(*point))
            || !departs_normally(edge.path[0], edge.path[1], source)
            || !departs_normally(
                edge.path[edge.path.len() - 1],
                edge.path[edge.path.len() - 2],
                target,
            )
            || edge.path.windows(2).enumerate().any(|(index, segment)| {
                segment[0] == segment[1]
                    || !segment_is_axis_aligned(segment[0], segment[1])
                    || frames
                        .iter()
                        .any(|frame| !frame_segment_is_clear(segment[0], segment[1], *frame))
                    || nodes.iter().any(|node| {
                        segment_hits_rect(segment[0], segment[1], node.rect)
                            && !(index == 0
                                && node.id == edge.from
                                && departs_normally(segment[0], segment[1], node.rect))
                            && !(index + 2 == edge.path.len()
                                && node.id == edge.to
                                && departs_normally(segment[1], segment[0], node.rect))
                    })
            })
            || edge.path.windows(3).any(|points| {
                (points[0].x == points[1].x) != (points[1].x == points[2].x)
                    && frames
                        .iter()
                        .any(|frame| !frame_bend_is_clear(points[1], *frame))
            })
        {
            return false;
        }

        let (start_marker, end_marker) = markers(edge.direction);
        if edge.start_marker != start_marker || edge.end_marker != end_marker {
            return false;
        }
        match (edge.label.as_ref(), edge.label_anchor) {
            (Some(_), Some(anchor)) => edge
                .path
                .windows(2)
                .any(|segment| point_is_on_segment(anchor, segment[0], segment[1])),
            (None, None) => true,
            (Some(_), None) | (None, Some(_)) => false,
        }
    })
}

#[cfg(test)]
pub(crate) fn alternative_routes(
    edge: &Edge,
    nodes: &[SceneNode],
    bounds: Rect,
    fixed_obstacles: &[Rect],
    frames: &[Rect],
) -> Result<Vec<SceneEdge>, RoutingError> {
    alternative_routes_with_context(
        edge,
        std::slice::from_ref(edge),
        &[],
        nodes,
        bounds,
        fixed_obstacles,
        frames,
    )
}

pub(crate) fn alternative_routes_with_context(
    edge: &Edge,
    context_edges: &[Edge],
    reserved_edges: &[SceneEdge],
    nodes: &[SceneNode],
    bounds: Rect,
    fixed_obstacles: &[Rect],
    frames: &[Rect],
) -> Result<Vec<SceneEdge>, RoutingError> {
    let source = node_rect(nodes, &edge.from).ok_or(RoutingError)?;
    let target = node_rect(nodes, &edge.to).ok_or(RoutingError)?;
    let mut router = GridRouter::new_for_edge(
        nodes,
        bounds,
        fixed_obstacles,
        frames,
        context_edges,
        edge,
        reserved_edges,
    );
    router.reserve_edges(reserved_edges);
    let (start_marker, end_marker) = markers(edge.direction);
    let source_stubs = terminal_stubs(source, marker_stub_length(start_marker));
    let target_stubs = terminal_stubs(target, marker_stub_length(end_marker));
    let mut port_pairs = Vec::new();
    for (source_index, (source_port, _, _, source_preference)) in source_stubs.iter().enumerate() {
        if !router.port_is_enabled(source, source_index) {
            continue;
        }
        for (target_index, (target_port, _, _, target_preference)) in
            target_stubs.iter().enumerate()
        {
            if !router.port_is_enabled(target, target_index) {
                continue;
            }
            let reuse = i64::from(
                router.used_ports.get(source_port).copied().unwrap_or(0)
                    + router.used_ports.get(target_port).copied().unwrap_or(0),
            ) * PORT_REUSE_PENALTY;
            port_pairs.push((
                *source_preference
                    + *target_preference
                    + reuse
                    + manhattan(*source_port, *target_port),
                source_index,
                target_index,
            ));
        }
    }
    port_pairs.sort();
    let (midpoint_pairs, offset_pairs): (Vec<_>, Vec<_>) = port_pairs
        .into_iter()
        .partition(|(_, source_port, target_port)| source_port % 3 == 0 && target_port % 3 == 0);
    let mut paths = Vec::new();
    for (_, source_port, target_port) in midpoint_pairs {
        if let Some((cost, path)) = router.route_between_with_cost(
            source,
            target,
            Some((source_port, target_port)),
            edge.direction,
        ) {
            paths.push((cost, path));
        }
    }
    for (_, source_port, target_port) in offset_pairs {
        if paths.len() >= ALTERNATIVE_PORT_PAIR_LIMIT {
            break;
        }
        if let Some((cost, path)) = router.route_between_with_cost(
            source,
            target,
            Some((source_port, target_port)),
            edge.direction,
        ) {
            paths.push((cost, path));
        }
    }
    paths.sort();
    paths.dedup_by(|left, right| left.1 == right.1);
    Ok(paths
        .into_iter()
        .map(|(_, path)| scene_edge(edge, path))
        .collect())
}

fn scene_edge(edge: &Edge, path: Vec<Point>) -> SceneEdge {
    let (start_marker, end_marker) = markers(edge.direction);
    let label_anchor = edge.label.as_ref().map(|_| path_midpoint(&path));
    SceneEdge {
        from: edge.from.clone(),
        to: edge.to.clone(),
        direction: edge.direction,
        kind: edge.kind,
        label: edge.label.clone(),
        path,
        start_marker,
        end_marker,
        label_anchor,
        label_rect: None,
    }
}

fn node_rect(nodes: &[SceneNode], identifier: &str) -> Option<Rect> {
    nodes
        .iter()
        .find(|node| node.id == identifier)
        .map(|node| node.rect)
}

fn markers(direction: EdgeDirection) -> (Marker, Marker) {
    match direction {
        EdgeDirection::Forward => (Marker::None, Marker::Arrow),
        EdgeDirection::Bidirectional => (Marker::Arrow, Marker::Arrow),
        EdgeDirection::Association => (Marker::None, Marker::None),
    }
}

fn marker_stub_length(marker: Marker) -> i64 {
    match marker {
        Marker::None => ROUTE_MARGIN,
        Marker::Arrow => ARROW_TERMINAL_STUB_LENGTH,
    }
}

fn ports(rect: Rect) -> [Point; 12] {
    let horizontal = [
        rect.x + rect.width / 2,
        rect.x + rect.width / 4,
        rect.x + 3 * rect.width / 4,
    ];
    let vertical = [
        rect.y + rect.height / 2,
        rect.y + rect.height / 4,
        rect.y + 3 * rect.height / 4,
    ];
    [
        Point {
            x: rect.x + rect.width,
            y: vertical[0],
        },
        Point {
            x: rect.x + rect.width,
            y: vertical[1],
        },
        Point {
            x: rect.x + rect.width,
            y: vertical[2],
        },
        Point {
            x: horizontal[0],
            y: rect.y + rect.height,
        },
        Point {
            x: horizontal[1],
            y: rect.y + rect.height,
        },
        Point {
            x: horizontal[2],
            y: rect.y + rect.height,
        },
        Point {
            x: rect.x,
            y: vertical[0],
        },
        Point {
            x: rect.x,
            y: vertical[1],
        },
        Point {
            x: rect.x,
            y: vertical[2],
        },
        Point {
            x: horizontal[0],
            y: rect.y,
        },
        Point {
            x: horizontal[1],
            y: rect.y,
        },
        Point {
            x: horizontal[2],
            y: rect.y,
        },
    ]
}

fn terminal_stubs(rect: Rect, stub_length: i64) -> [(Point, Point, usize, i64); 12] {
    let ports = ports(rect);
    [
        (
            ports[0],
            Point {
                x: ports[0].x + stub_length,
                y: ports[0].y,
            },
            1,
            0,
        ),
        (
            ports[1],
            Point {
                x: ports[1].x + stub_length,
                y: ports[1].y,
            },
            1,
            OFF_CENTER_PORT_PENALTY,
        ),
        (
            ports[2],
            Point {
                x: ports[2].x + stub_length,
                y: ports[2].y,
            },
            1,
            OFF_CENTER_PORT_PENALTY,
        ),
        (
            ports[3],
            Point {
                x: ports[3].x,
                y: ports[3].y + stub_length,
            },
            2,
            0,
        ),
        (
            ports[4],
            Point {
                x: ports[4].x,
                y: ports[4].y + stub_length,
            },
            2,
            OFF_CENTER_PORT_PENALTY,
        ),
        (
            ports[5],
            Point {
                x: ports[5].x,
                y: ports[5].y + stub_length,
            },
            2,
            OFF_CENTER_PORT_PENALTY,
        ),
        (
            ports[6],
            Point {
                x: ports[6].x - stub_length,
                y: ports[6].y,
            },
            1,
            0,
        ),
        (
            ports[7],
            Point {
                x: ports[7].x - stub_length,
                y: ports[7].y,
            },
            1,
            OFF_CENTER_PORT_PENALTY,
        ),
        (
            ports[8],
            Point {
                x: ports[8].x - stub_length,
                y: ports[8].y,
            },
            1,
            OFF_CENTER_PORT_PENALTY,
        ),
        (
            ports[9],
            Point {
                x: ports[9].x,
                y: ports[9].y - stub_length,
            },
            2,
            0,
        ),
        (
            ports[10],
            Point {
                x: ports[10].x,
                y: ports[10].y - stub_length,
            },
            2,
            OFF_CENTER_PORT_PENALTY,
        ),
        (
            ports[11],
            Point {
                x: ports[11].x,
                y: ports[11].y - stub_length,
            },
            2,
            OFF_CENTER_PORT_PENALTY,
        ),
    ]
}

fn departs_normally(terminal: Point, other: Point, rect: Rect) -> bool {
    if terminal.y == other.y && between(terminal.y, rect.y, rect.y + rect.height) {
        (terminal.x == rect.x && other.x < terminal.x)
            || (terminal.x == rect.x + rect.width && other.x > terminal.x)
    } else if terminal.x == other.x && between(terminal.x, rect.x, rect.x + rect.width) {
        (terminal.y == rect.y && other.y < terminal.y)
            || (terminal.y == rect.y + rect.height && other.y > terminal.y)
    } else {
        false
    }
}

fn path_midpoint(path: &[Point]) -> Point {
    let total = path
        .windows(2)
        .map(|segment| manhattan(segment[0], segment[1]))
        .sum::<i64>();
    let mut remaining = total / 2;
    for segment in path.windows(2) {
        let length = manhattan(segment[0], segment[1]);
        if remaining <= length {
            return if segment[0].x == segment[1].x {
                Point {
                    x: segment[0].x,
                    y: move_toward(segment[0].y, segment[1].y, remaining),
                }
            } else {
                Point {
                    x: move_toward(segment[0].x, segment[1].x, remaining),
                    y: segment[0].y,
                }
            };
        }
        remaining -= length;
    }
    path[path.len() - 1]
}

fn move_toward(start: i64, end: i64, distance: i64) -> i64 {
    if start <= end {
        start + distance
    } else {
        start - distance
    }
}

fn manhattan(left: Point, right: Point) -> i64 {
    (left.x - right.x).abs() + (left.y - right.y).abs()
}

fn point_is_on_segment(point: Point, start: Point, end: Point) -> bool {
    if start.x == end.x {
        point.x == start.x && between(point.y, start.y, end.y)
    } else if start.y == end.y {
        point.y == start.y && between(point.x, start.x, end.x)
    } else {
        false
    }
}

fn between(value: i64, left: i64, right: i64) -> bool {
    value >= left.min(right) && value <= left.max(right)
}

fn segment_is_axis_aligned(start: Point, end: Point) -> bool {
    start.x == end.x || start.y == end.y
}

fn segment_crosses_rect_interior(start: Point, end: Point, rect: Rect) -> bool {
    if start.y == end.y {
        start.y > rect.y
            && start.y < rect.y + rect.height
            && start.x.min(end.x) < rect.x + rect.width
            && start.x.max(end.x) > rect.x
    } else if start.x == end.x {
        start.x > rect.x
            && start.x < rect.x + rect.width
            && start.y.min(end.y) < rect.y + rect.height
            && start.y.max(end.y) > rect.y
    } else {
        true
    }
}

fn segment_hits_rect(start: Point, end: Point, rect: Rect) -> bool {
    if start.y == end.y {
        between(start.y, rect.y, rect.y + rect.height)
            && start.x.min(end.x) < rect.x + rect.width
            && start.x.max(end.x) > rect.x
    } else if start.x == end.x {
        between(start.x, rect.x, rect.x + rect.width)
            && start.y.min(end.y) < rect.y + rect.height
            && start.y.max(end.y) > rect.y
    } else {
        true
    }
}

fn expanded(rect: Rect) -> Rect {
    Rect {
        x: rect.x - ROUTE_MARGIN,
        y: rect.y - ROUTE_MARGIN,
        width: rect.width + 2 * ROUTE_MARGIN,
        height: rect.height + 2 * ROUTE_MARGIN,
    }
}

fn frame_segment_is_clear(start: Point, end: Point, frame: Rect) -> bool {
    // Only the finite, parallel sides constrain a segment. Extending their
    // projection by the same margin also protects perpendicular corner grazes.
    if start.y == end.y {
        let near_side = (start.y - frame.y).abs() < FRAME_MARGIN
            || (start.y - frame.y - frame.height).abs() < FRAME_MARGIN;
        !near_side
            || start.x.max(end.x) <= frame.x - FRAME_MARGIN
            || start.x.min(end.x) >= frame.x + frame.width + FRAME_MARGIN
    } else if start.x == end.x {
        let near_side = (start.x - frame.x).abs() < FRAME_MARGIN
            || (start.x - frame.x - frame.width).abs() < FRAME_MARGIN;
        !near_side
            || start.y.max(end.y) <= frame.y - FRAME_MARGIN
            || start.y.min(end.y) >= frame.y + frame.height + FRAME_MARGIN
    } else {
        false
    }
}

fn frame_bend_is_clear(point: Point, frame: Rect) -> bool {
    let near_horizontal = ((point.y - frame.y).abs() < FRAME_MARGIN
        || (point.y - frame.y - frame.height).abs() < FRAME_MARGIN)
        && point.x > frame.x - FRAME_MARGIN
        && point.x < frame.x + frame.width + FRAME_MARGIN;
    let near_vertical = ((point.x - frame.x).abs() < FRAME_MARGIN
        || (point.x - frame.x - frame.width).abs() < FRAME_MARGIN)
        && point.y > frame.y - FRAME_MARGIN
        && point.y < frame.y + frame.height + FRAME_MARGIN;
    !near_horizontal && !near_vertical
}

impl Rect {
    fn contains_point(self, point: Point) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }

    fn contains_point_interior(self, point: Point) -> bool {
        point.x > self.x
            && point.x < self.x + self.width
            && point.y > self.y
            && point.y < self.y + self.height
    }

    fn has_boundary_point(self, point: Point) -> bool {
        self.contains_point(point)
            && (point.x == self.x
                || point.x == self.x + self.width
                || point.y == self.y
                || point.y == self.y + self.height)
    }
}

fn preferred_sides(source: Rect, target: Rect) -> (usize, usize) {
    let source_center = Point {
        x: 2 * source.x + source.width,
        y: 2 * source.y + source.height,
    };
    let target_center = Point {
        x: 2 * target.x + target.width,
        y: 2 * target.y + target.height,
    };
    let horizontal = target_center.x - source_center.x;
    let vertical = target_center.y - source_center.y;
    if horizontal.abs() > vertical.abs() {
        if horizontal >= 0 { (0, 2) } else { (2, 0) }
    } else if vertical >= 0 {
        (1, 3)
    } else {
        (3, 1)
    }
}

#[derive(Debug)]
struct GridRouter<'a> {
    nodes: &'a [SceneNode],
    fixed_obstacles: &'a [Rect],
    frames: &'a [Rect],
    bounds: Rect,
    xs: Vec<i64>,
    ys: Vec<i64>,
    valid: Vec<bool>,
    bend_allowed: Vec<bool>,
    links: Vec<[Option<usize>; 4]>,
    shared: Vec<[u32; 2]>,
    occupied: Vec<[u32; 2]>,
    used_ports: BTreeMap<Point, u32>,
    multi_port_sides: Vec<[bool; 4]>,
}

impl<'a> GridRouter<'a> {
    #[cfg(test)]
    fn new(
        nodes: &'a [SceneNode],
        bounds: Rect,
        fixed_obstacles: &'a [Rect],
        frames: &'a [Rect],
    ) -> Self {
        Self::new_for_edges(nodes, bounds, fixed_obstacles, frames, &[])
    }

    fn new_for_edges(
        nodes: &'a [SceneNode],
        bounds: Rect,
        fixed_obstacles: &'a [Rect],
        frames: &'a [Rect],
        edges: &[Edge],
    ) -> Self {
        Self::new_with_context(nodes, bounds, fixed_obstacles, frames, edges, None, &[])
    }

    fn new_for_edge(
        nodes: &'a [SceneNode],
        bounds: Rect,
        fixed_obstacles: &'a [Rect],
        frames: &'a [Rect],
        edges: &[Edge],
        active_edge: &Edge,
        reserved_edges: &[SceneEdge],
    ) -> Self {
        Self::new_with_context(
            nodes,
            bounds,
            fixed_obstacles,
            frames,
            edges,
            Some(active_edge),
            reserved_edges,
        )
    }

    fn new_with_context(
        nodes: &'a [SceneNode],
        bounds: Rect,
        fixed_obstacles: &'a [Rect],
        frames: &'a [Rect],
        edges: &[Edge],
        active_edge: Option<&Edge>,
        reserved_edges: &[SceneEdge],
    ) -> Self {
        let mut side_demand = vec![[0_u32; 4]; nodes.len()];
        for edge in edges {
            let Some(source_index) = nodes.iter().position(|node| node.id == edge.from) else {
                continue;
            };
            let Some(target_index) = nodes.iter().position(|node| node.id == edge.to) else {
                continue;
            };
            let (source_side, target_side) =
                preferred_sides(nodes[source_index].rect, nodes[target_index].rect);
            side_demand[source_index][source_side] += 1;
            side_demand[target_index][target_side] += 1;
        }
        let multi_port_sides = side_demand
            .into_iter()
            .zip(nodes)
            .map(|(demand, node)| {
                let requested = if demand.iter().sum::<u32>() >= 3 {
                    [true; 4]
                } else {
                    demand.map(|count| count > 1)
                };
                std::array::from_fn(|side| requested[side] && node.offset_port_sides[side])
            })
            .collect::<Vec<_>>();
        let mut xs = vec![
            bounds.x + ROUTE_MARGIN,
            bounds.x + bounds.width - ROUTE_MARGIN,
        ];
        let mut ys = vec![
            bounds.y + ROUTE_MARGIN,
            bounds.y + bounds.height - ROUTE_MARGIN,
        ];
        for (node_index, node) in nodes.iter().enumerate() {
            let rect = node.rect;
            xs.extend([
                rect.x - ARROW_TERMINAL_STUB_LENGTH,
                rect.x - ROUTE_MARGIN,
                rect.x + rect.width / 2,
                rect.x + rect.width + ROUTE_MARGIN,
                rect.x + rect.width + ARROW_TERMINAL_STUB_LENGTH,
            ]);
            ys.extend([
                rect.y - ARROW_TERMINAL_STUB_LENGTH,
                rect.y - ROUTE_MARGIN,
                rect.y + rect.height / 2,
                rect.y + rect.height + ROUTE_MARGIN,
                rect.y + rect.height + ARROW_TERMINAL_STUB_LENGTH,
            ]);
            let active = active_edge.is_none_or(|edge| edge.from == node.id || edge.to == node.id);
            if active && (multi_port_sides[node_index][1] || multi_port_sides[node_index][3]) {
                xs.extend([rect.x + rect.width / 4, rect.x + 3 * rect.width / 4]);
            }
            if active && (multi_port_sides[node_index][0] || multi_port_sides[node_index][2]) {
                ys.extend([rect.y + rect.height / 4, rect.y + 3 * rect.height / 4]);
            }
        }
        for rect in fixed_obstacles {
            xs.extend([rect.x - ROUTE_MARGIN, rect.x + rect.width + ROUTE_MARGIN]);
            ys.extend([rect.y - ROUTE_MARGIN, rect.y + rect.height + ROUTE_MARGIN]);
        }
        for frame in frames {
            for side in [frame.x, frame.x + frame.width] {
                xs.extend([side - FRAME_MARGIN, side + FRAME_MARGIN]);
            }
            for side in [frame.y, frame.y + frame.height] {
                ys.extend([side - FRAME_MARGIN, side + FRAME_MARGIN]);
            }
        }
        for edge in reserved_edges {
            for point in &edge.path {
                xs.push(point.x);
                ys.push(point.y);
            }
        }
        xs.retain(|x| *x >= bounds.x && *x <= bounds.x + bounds.width);
        ys.retain(|y| *y >= bounds.y && *y <= bounds.y + bounds.height);
        xs.sort_unstable();
        ys.sort_unstable();
        xs.dedup();
        ys.dedup();
        let obstacles = nodes
            .iter()
            .map(|node| expanded(node.rect))
            .chain(fixed_obstacles.iter().copied().map(expanded))
            .collect::<Vec<_>>();
        let valid: Vec<bool> = ys
            .iter()
            .flat_map(|y| {
                let obstacles = &obstacles;
                xs.iter().map(move |x| {
                    let point = Point { x: *x, y: *y };
                    obstacles
                        .iter()
                        .all(|rect| !rect.contains_point_interior(point))
                })
            })
            .collect();
        let bend_allowed = (0..valid.len())
            .map(|vertex| {
                let point = Point {
                    x: xs[vertex % xs.len()],
                    y: ys[vertex / xs.len()],
                };
                frames
                    .iter()
                    .all(|frame| frame_bend_is_clear(point, *frame))
            })
            .collect();
        let mut router = Self {
            nodes,
            fixed_obstacles,
            frames,
            bounds,
            xs,
            ys,
            links: vec![[None; 4]; valid.len()],
            shared: vec![[0; 2]; valid.len()],
            occupied: vec![[0; 2]; valid.len()],
            used_ports: BTreeMap::new(),
            multi_port_sides,
            valid,
            bend_allowed,
        };
        // The clearance grid is fixed for the entire scene. Search only reads
        // these visibility links, rather than rechecking every obstacle.
        for vertex in 0..router.valid.len() {
            if !router.valid[vertex] {
                continue;
            }
            let x = vertex % router.xs.len();
            let y = vertex / router.xs.len();
            let candidates = [
                x.checked_sub(1).map(|x| y * router.xs.len() + x),
                (x + 1 < router.xs.len()).then_some(vertex + 1),
                y.checked_sub(1).map(|y| y * router.xs.len() + x),
                (y + 1 < router.ys.len()).then_some(vertex + router.xs.len()),
            ];
            for (slot, candidate) in candidates.into_iter().enumerate() {
                if let Some(next) = candidate {
                    if router.valid[next]
                        && obstacles.iter().all(|rect| {
                            !segment_crosses_rect_interior(
                                router.point(vertex),
                                router.point(next),
                                *rect,
                            )
                        })
                        && frames.iter().all(|frame| {
                            frame_segment_is_clear(router.point(vertex), router.point(next), *frame)
                        })
                    {
                        router.links[vertex][slot] = Some(next);
                    }
                }
            }
        }
        router
    }

    fn route(
        &mut self,
        source: Rect,
        target: Rect,
        direction: EdgeDirection,
    ) -> Option<Vec<Point>> {
        self.route_between_with_cost(source, target, None, direction)
            .map(|(_, path)| path)
    }

    fn route_between_with_cost(
        &mut self,
        source: Rect,
        target: Rect,
        port_pair: Option<(usize, usize)>,
        direction: EdgeDirection,
    ) -> Option<(i64, Vec<Point>)> {
        let (start_marker, end_marker) = markers(direction);
        let source_stub_length = marker_stub_length(start_marker);
        let target_stub_length = marker_stub_length(end_marker);
        let state_count = self.valid.len() * 3;
        let mut distances = vec![i64::MAX; state_count];
        let mut parents = vec![None; state_count];
        let mut pending = BinaryHeap::new();
        let mut starts = Vec::new();
        for (index, (port, stub, axis, preference)) in terminal_stubs(source, source_stub_length)
            .into_iter()
            .enumerate()
        {
            if port_pair.is_some_and(|(source_port, _)| source_port != index) {
                continue;
            }
            if !self.port_is_enabled(source, index) {
                continue;
            }
            let Some(vertex) = self.vertex(stub) else {
                continue;
            };
            if !self.stub_is_clear(port, stub, source) {
                continue;
            }
            let state = vertex * 3 + axis;
            let cost = source_stub_length
                + preference
                + i64::from(self.used_ports.get(&port).copied().unwrap_or(0)) * PORT_REUSE_PENALTY;
            let cost = cost + self.terminal_congestion_cost(port, stub, axis);
            distances[state] = cost;
            pending.push(Reverse((cost, state)));
            starts.push((state, port));
            if source == target {
                break;
            }
        }
        let targets = terminal_stubs(target, target_stub_length)
            .into_iter()
            .enumerate()
            .filter_map(|(index, (port, stub, axis, preference))| {
                if port_pair.is_some_and(|(_, target_port)| target_port != index) {
                    return None;
                }
                if !self.port_is_enabled(target, index) {
                    return None;
                }
                let vertex = self.vertex(stub)?;
                (self.stub_is_clear(port, stub, target)
                    && !(source == target && starts.iter().any(|(_, start)| *start == port)))
                .then_some((vertex, port, axis, preference))
            })
            .collect::<Vec<_>>();
        if starts.is_empty() || targets.is_empty() {
            return None;
        }
        let mut best: Option<(i64, usize, Point)> = None;

        while let Some(Reverse((cost, state))) = pending.pop() {
            if distances[state] != cost {
                continue;
            }
            if best.is_some_and(|(best_cost, _, _)| cost > best_cost) {
                break;
            }
            let vertex = state / 3;
            let incoming_axis = state % 3;
            for &(target_vertex, port, axis, preference) in &targets {
                if target_vertex == vertex && (incoming_axis == axis || self.bend_allowed[vertex]) {
                    let candidate = (
                        cost + target_stub_length
                            + preference
                            + i64::from(self.used_ports.get(&port).copied().unwrap_or(0))
                                * PORT_REUSE_PENALTY
                            + self.terminal_congestion_cost(port, self.point(vertex), axis)
                            + if incoming_axis == axis {
                                0
                            } else {
                                BEND_PENALTY
                            },
                        state,
                        port,
                    );
                    if best.is_none_or(|best| candidate < best) {
                        best = Some(candidate);
                    }
                }
            }
            for (slot, next_vertex) in self.links[vertex].iter().enumerate() {
                let Some(next_vertex) = *next_vertex else {
                    continue;
                };
                let next_axis = if slot < 2 { 1 } else { 2 };
                if incoming_axis != next_axis && !self.bend_allowed[vertex] {
                    continue;
                }
                let length = manhattan(self.point(vertex), self.point(next_vertex));
                let bend = if incoming_axis != next_axis {
                    BEND_PENALTY
                } else {
                    0
                };
                let shared = i64::from(self.shared[vertex.min(next_vertex)][next_axis - 1]);
                let crossing = i64::from(self.occupied[next_vertex][2 - next_axis]);
                let next_state = next_vertex * 3 + next_axis;
                let next_cost = cost
                    + length
                    + bend
                    + shared * length * SHARED_LENGTH_PENALTY
                    + crossing * CROSSING_PENALTY;
                if next_cost < distances[next_state] {
                    distances[next_state] = next_cost;
                    parents[next_state] = Some(state);
                    pending.push(Reverse((next_cost, next_state)));
                }
            }
        }
        let (cost, state, target_port) = best?;
        let (middle, root) = self.reconstruct(state, &parents);
        let source_port = starts.iter().find(|(state, _)| *state == root)?.1;
        let mut path = Vec::with_capacity(middle.len() + 2);
        for point in std::iter::once(source_port)
            .chain(middle)
            .chain(std::iter::once(target_port))
        {
            push_point(&mut path, point);
        }
        if port_pair.is_none() {
            self.reserve_path(&path);
            for port in [source_port, target_port] {
                *self.used_ports.entry(port).or_default() += 1;
            }
        }
        Some((cost, path))
    }

    fn terminal_congestion_cost(&self, port: Point, stub: Point, axis: usize) -> i64 {
        let horizontal = axis == 1;
        let (coordinates, fixed, start, end) = if horizontal {
            (&self.xs, self.ys.binary_search(&port.y), port.x, stub.x)
        } else {
            (&self.ys, self.xs.binary_search(&port.x), port.y, stub.y)
        };
        let Ok(fixed) = fixed else {
            return 0;
        };
        let first = coordinates.partition_point(|coordinate| *coordinate < start.min(end));
        let last = coordinates.partition_point(|coordinate| *coordinate <= start.max(end));
        let mut cost = 0;
        for index in first..last {
            let vertex = if horizontal {
                fixed * self.xs.len() + index
            } else {
                index * self.xs.len() + fixed
            };
            cost += i64::from(self.occupied[vertex][2 - axis]) * CROSSING_PENALTY;
            if index + 1 < last {
                let length = coordinates[index + 1] - coordinates[index];
                cost += i64::from(self.shared[vertex][axis - 1]) * length * SHARED_LENGTH_PENALTY;
            }
        }
        cost
    }

    fn stub_is_clear(&self, port: Point, stub: Point, terminal: Rect) -> bool {
        self.bounds.contains_point(port)
            && self.bounds.contains_point(stub)
            && self.nodes.iter().all(|node| {
                node.rect == terminal
                    || !segment_crosses_rect_interior(port, stub, expanded(node.rect))
            })
            && self
                .fixed_obstacles
                .iter()
                .all(|rect| !segment_crosses_rect_interior(port, stub, expanded(*rect)))
            && self
                .frames
                .iter()
                .all(|frame| frame_segment_is_clear(port, stub, *frame))
    }

    fn port_is_enabled(&self, rect: Rect, port_index: usize) -> bool {
        port_index % 3 == 0
            || self
                .nodes
                .iter()
                .position(|node| node.rect == rect)
                .is_some_and(|node_index| self.multi_port_sides[node_index][port_index / 3])
    }

    fn reserve_path(&mut self, path: &[Point]) {
        for segment in path.windows(2) {
            let horizontal = segment[0].y == segment[1].y;
            let (coordinates, fixed, start, end, axis) = if horizontal {
                (
                    &self.xs,
                    self.ys.binary_search(&segment[0].y),
                    segment[0].x,
                    segment[1].x,
                    0,
                )
            } else {
                (
                    &self.ys,
                    self.xs.binary_search(&segment[0].x),
                    segment[0].y,
                    segment[1].y,
                    1,
                )
            };
            let Ok(fixed) = fixed else { continue };
            let first = coordinates.partition_point(|coordinate| *coordinate < start.min(end));
            let last = coordinates.partition_point(|coordinate| *coordinate <= start.max(end));
            for index in first..last {
                let vertex = if horizontal {
                    fixed * self.xs.len() + index
                } else {
                    index * self.xs.len() + fixed
                };
                if self.valid[vertex] {
                    self.occupied[vertex][axis] += 1;
                    if index + 1 < last && self.links[vertex][axis * 2 + 1].is_some() {
                        self.shared[vertex][axis] += 1;
                    }
                }
            }
        }
    }

    fn reserve_edges(&mut self, edges: &[SceneEdge]) {
        for edge in edges {
            self.reserve_path(&edge.path);
            if let Some(port) = edge.path.first() {
                *self.used_ports.entry(*port).or_default() += 1;
            }
            if let Some(port) = edge.path.last() {
                *self.used_ports.entry(*port).or_default() += 1;
            }
        }
    }

    fn vertex(&self, point: Point) -> Option<usize> {
        let x = self.xs.binary_search(&point.x).ok()?;
        let y = self.ys.binary_search(&point.y).ok()?;
        let vertex = y * self.xs.len() + x;
        self.valid[vertex].then_some(vertex)
    }

    fn point(&self, vertex: usize) -> Point {
        Point {
            x: self.xs[vertex % self.xs.len()],
            y: self.ys[vertex / self.xs.len()],
        }
    }

    fn reconstruct(&self, state: usize, parents: &[Option<usize>]) -> (Vec<Point>, usize) {
        let mut states = Vec::new();
        let mut cursor = Some(state);
        while let Some(current) = cursor {
            states.push(current);
            cursor = parents[current];
        }
        states.reverse();

        let mut path = Vec::new();
        let root = states[0];
        for state in states {
            push_point(&mut path, self.point(state / 3));
        }
        (path, root)
    }
}

fn push_point(path: &mut Vec<Point>, point: Point) {
    if path.last() == Some(&point) {
        return;
    }
    if path.len() >= 2 {
        let previous = path[path.len() - 2];
        let last = path[path.len() - 1];
        if (previous.x == last.x && last.x == point.x)
            || (previous.y == last.y && last.y == point.y)
        {
            path.pop();
        }
    }
    path.push(point);
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use stack_compiler::ir::{EdgeDirection, EdgeKind};

    use super::{Marker, Point, SceneEdge};
    use crate::scene::{Rect, SceneNode};

    fn test_node(id: &str, x: i64, y: i64) -> SceneNode {
        SceneNode {
            id: id.to_owned(),
            parent_group_id: None,
            rect: Rect {
                x,
                y,
                width: 100_000,
                height: 100_000,
            },
            offset_port_sides: [true; 4],
        }
    }

    fn test_edge(path: &[(i64, i64)]) -> SceneEdge {
        SceneEdge {
            from: "source".to_owned(),
            to: "target".to_owned(),
            direction: EdgeDirection::Forward,
            kind: EdgeKind::Flow,
            label: None,
            path: path.iter().map(|&(x, y)| Point { x, y }).collect(),
            start_marker: Marker::None,
            end_marker: Marker::Arrow,
            label_anchor: None,
            label_rect: None,
        }
    }

    fn ir_edge(from: &str, to: &str) -> stack_compiler::ir::Edge {
        stack_compiler::ir::Edge {
            from: from.to_owned(),
            to: to.to_owned(),
            direction: EdgeDirection::Forward,
            kind: EdgeKind::Flow,
            label: None,
        }
    }

    fn test_bounds() -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: 500_000,
            height: 400_000,
        }
    }

    fn test_frame() -> Rect {
        Rect {
            x: 100_000,
            y: 100_000,
            width: 200_000,
            height: 200_000,
        }
    }

    #[test]
    fn frame_rejects_parallel_border_travel() {
        assert!(!super::frame_segment_is_clear(
            Point {
                x: 120_000,
                y: 100_000
            },
            Point {
                x: 280_000,
                y: 100_000
            },
            test_frame(),
        ));
    }

    #[test]
    fn frame_rejects_parallel_routes_inside_the_painted_clearance() {
        for y in [83_000, 117_000, 283_000, 317_000] {
            assert!(!super::frame_segment_is_clear(
                Point { x: 120_000, y },
                Point { x: 280_000, y },
                test_frame(),
            ));
        }
        for x in [83_000, 117_000, 283_000, 317_000] {
            assert!(!super::frame_segment_is_clear(
                Point { x, y: 120_000 },
                Point { x, y: 280_000 },
                test_frame(),
            ));
        }
    }

    #[test]
    fn frame_rejects_corner_crossings_and_bends_on_the_border() {
        assert!(!super::frame_segment_is_clear(
            Point {
                x: 110_000,
                y: 60_000
            },
            Point {
                x: 110_000,
                y: 160_000
            },
            test_frame(),
        ));
        assert!(!super::frame_bend_is_clear(
            Point {
                x: 100_000,
                y: 200_000
            },
            test_frame(),
        ));
        assert!(!super::frame_bend_is_clear(
            Point {
                x: 90_000,
                y: 90_000
            },
            test_frame(),
        ));
    }

    #[test]
    fn frame_allows_normal_crossings_and_clear_parallel_lanes() {
        for (start, end) in [
            ((200_000, 60_000), (200_000, 160_000)),
            ((60_000, 200_000), (160_000, 200_000)),
            ((120_000, 82_000), (280_000, 82_000)),
            ((120_000, 118_000), (280_000, 118_000)),
            ((10_000, 100_000), (50_000, 100_000)),
        ] {
            assert!(super::frame_segment_is_clear(
                Point {
                    x: start.0,
                    y: start.1
                },
                Point { x: end.0, y: end.1 },
                test_frame(),
            ));
        }
    }

    #[test]
    fn frame_routes_leave_a_clear_parallel_lane() -> Result<(), Box<dyn Error>> {
        let nodes = [
            test_node("source", 20_000, 100_000),
            test_node("target", 320_000, 100_000),
        ];
        let frames = [Rect {
            x: 150_000,
            y: 160_000,
            width: 200_000,
            height: 160_000,
        }];
        let mut edge = test_edge(&[(120_000, 150_000), (320_000, 150_000)]);
        assert!(super::geometry_is_valid(
            &[edge.clone()],
            &nodes,
            test_bounds(),
            &[],
        ));
        assert!(!super::geometry_is_valid(
            &[edge.clone()],
            &nodes,
            test_bounds(),
            &frames,
        ));
        let mut router = super::GridRouter::new(&nodes, test_bounds(), &[], &frames);
        edge.path = router
            .route(nodes[0].rect, nodes[1].rect, EdgeDirection::Forward)
            .ok_or("no clear route beside the frame")?;
        assert!(super::geometry_is_valid(
            &[edge],
            &nodes,
            test_bounds(),
            &frames,
        ));
        Ok(())
    }

    #[test]
    fn frame_routes_cross_straight_into_the_group() -> Result<(), Box<dyn Error>> {
        let nodes = [
            test_node("source", 20_000, 160_000),
            test_node("target", 220_000, 160_000),
        ];
        let frames = [Rect {
            x: 150_000,
            y: 110_000,
            width: 200_000,
            height: 200_000,
        }];
        let mut router = super::GridRouter::new(&nodes, test_bounds(), &[], &frames);
        let path = router
            .route(nodes[0].rect, nodes[1].rect, EdgeDirection::Forward)
            .ok_or("normal frame crossing is missing")?;
        let edge = test_edge(&[(120_000, 210_000), (220_000, 210_000)]);
        assert_eq!(path, edge.path);
        assert!(super::geometry_is_valid(
            &[edge],
            &nodes,
            test_bounds(),
            &frames,
        ));
        Ok(())
    }

    #[test]
    fn target_bends_leave_room_for_arrow_markers() -> Result<(), Box<dyn Error>> {
        let nodes = [
            test_node("source", 120_000, 20_000),
            test_node("target", 100_000, 260_000),
        ];
        let mut router = super::GridRouter::new(&nodes, test_bounds(), &[], &[]);
        let path = router
            .route(nodes[0].rect, nodes[1].rect, EdgeDirection::Forward)
            .ok_or("no route between vertically offset nodes")?;
        if path.len() < 3 {
            return Err("expected a bend before the target terminal".into());
        }

        let bend = path[path.len() - 2];
        let target = path[path.len() - 1];
        assert!(
            super::manhattan(bend, target) >= 16_000,
            "the final segment must keep the arrow marker clear of its preceding bend: {path:?}"
        );

        Ok(())
    }

    #[test]
    fn terminal_stub_lengths_follow_edge_markers() {
        for (direction, expected) in [
            (EdgeDirection::Forward, (8_000, 16_000)),
            (EdgeDirection::Bidirectional, (16_000, 16_000)),
            (EdgeDirection::Association, (8_000, 8_000)),
        ] {
            let (start, end) = super::markers(direction);
            assert_eq!(
                (
                    super::marker_stub_length(start),
                    super::marker_stub_length(end)
                ),
                expected
            );
        }
    }

    #[test]
    fn rejects_tangential_terminal_departure() {
        let nodes = [
            test_node("source", 20_000, 100_000),
            test_node("target", 320_000, 100_000),
        ];
        let edge = test_edge(&[(70_000, 100_000), (370_000, 100_000)]);
        assert!(!super::geometry_is_valid(
            &[edge],
            &nodes,
            test_bounds(),
            &[]
        ));
    }

    #[test]
    fn rejects_intervening_node_boundary_travel() {
        let nodes = [
            test_node("source", 20_000, 100_000),
            test_node("blocker", 170_000, 50_000),
            test_node("target", 320_000, 100_000),
        ];
        let edge = test_edge(&[(120_000, 150_000), (320_000, 150_000)]);
        assert!(!super::geometry_is_valid(
            &[edge],
            &nodes,
            test_bounds(),
            &[]
        ));
    }

    #[test]
    fn rejects_later_contact_with_the_source_boundary() {
        let nodes = [
            test_node("source", 20_000, 100_000),
            test_node("target", 320_000, 100_000),
        ];
        let edge = test_edge(&[
            (120_000, 150_000),
            (150_000, 150_000),
            (150_000, 100_000),
            (70_000, 100_000),
            (70_000, 50_000),
            (370_000, 50_000),
            (370_000, 100_000),
        ]);
        assert!(!super::geometry_is_valid(
            &[edge],
            &nodes,
            test_bounds(),
            &[]
        ));
    }

    fn scene_from(source: &[u8]) -> Result<crate::scene::Scene, Box<dyn Error>> {
        let compiled = stack_compiler::compile_bytes(source);
        if !compiled.diagnostics.is_empty() {
            return Err("fixture produced compiler diagnostics".into());
        }
        let diagram = compiled.diagram.ok_or("fixture produced no diagram")?;
        Ok(crate::scene::layout(&diagram, stack_theme::catalog())?)
    }

    #[test]
    fn preserves_edge_order_semantics_labels_and_markers() -> Result<(), Box<dyn Error>> {
        let scene = scene_from(
            b"stack 1.0 diagram \"Edges\" { node a \"A\" node b \"B\" node c \"C\" edge a -> b \"Request\" { kind request } edge b <-> c \"Flow\" edge c -- a { kind dependency } }",
        )?;
        assert_eq!(scene.edges.len(), 3);
        assert_eq!(scene.edges[0].from, "a");
        assert_eq!(scene.edges[0].to, "b");
        assert_eq!(scene.edges[0].kind, EdgeKind::Request);
        assert_eq!(scene.edges[0].label.as_deref(), Some("Request"));
        assert_eq!(scene.edges[0].start_marker, Marker::None);
        assert_eq!(scene.edges[0].end_marker, Marker::Arrow);
        assert_eq!(scene.edges[1].direction, EdgeDirection::Bidirectional);
        assert_eq!(scene.edges[1].start_marker, Marker::Arrow);
        assert_eq!(scene.edges[1].end_marker, Marker::Arrow);
        assert_eq!(scene.edges[2].direction, EdgeDirection::Association);
        assert_eq!(scene.edges[2].kind, EdgeKind::Dependency);
        assert_eq!(scene.edges[2].start_marker, Marker::None);
        assert_eq!(scene.edges[2].end_marker, Marker::None);
        assert!(scene.edges[0].label_anchor.is_some());
        assert!(scene.edges[2].label_anchor.is_none());
        assert!(scene.geometry_is_valid());
        Ok(())
    }

    #[test]
    fn routes_around_an_intervening_node() -> Result<(), Box<dyn Error>> {
        let nodes = [
            test_node("source", 20_000, 100_000),
            test_node("blocker", 170_000, 100_000),
            test_node("target", 320_000, 100_000),
        ];
        let mut router = super::GridRouter::new(&nodes, test_bounds(), &[], &[]);
        let path = router
            .route(nodes[0].rect, nodes[2].rect, EdgeDirection::Forward)
            .ok_or("no route around the intervening node")?;
        assert!(path.len() >= 4);
        let mut edge = test_edge(&[]);
        edge.path = path;
        assert!(super::geometry_is_valid(
            &[edge],
            &nodes,
            test_bounds(),
            &[]
        ));
        Ok(())
    }

    #[test]
    fn routes_around_reserved_title_boxes() -> Result<(), Box<dyn Error>> {
        let nodes = [
            test_node("source", 20_000, 100_000),
            test_node("target", 320_000, 100_000),
        ];
        let title = Rect {
            x: 180_000,
            y: 125_000,
            width: 70_000,
            height: 50_000,
        };
        let fixed = [title];
        let mut router = super::GridRouter::new(&nodes, test_bounds(), &fixed, &[]);
        let path = router
            .route(nodes[0].rect, nodes[1].rect, EdgeDirection::Forward)
            .ok_or("no route around the reserved title")?;
        assert!(
            path.windows(2)
                .all(|segment| { !super::segment_hits_rect(segment[0], segment[1], title) })
        );
        let mut edge = test_edge(&[]);
        edge.path = path;
        assert!(super::geometry_is_valid(
            &[edge],
            &nodes,
            test_bounds(),
            &[]
        ));
        Ok(())
    }

    #[test]
    fn separates_repeated_routes_when_another_clear_lane_exists() -> Result<(), Box<dyn Error>> {
        let nodes = [
            test_node("source", 20_000, 100_000),
            test_node("target", 320_000, 100_000),
        ];
        let mut router = super::GridRouter::new(&nodes, test_bounds(), &[], &[]);
        let first = router
            .route(nodes[0].rect, nodes[1].rect, EdgeDirection::Forward)
            .ok_or("first route is missing")?;
        let second = router
            .route(nodes[0].rect, nodes[1].rect, EdgeDirection::Forward)
            .ok_or("second route is missing")?;
        assert_ne!(first, second);
        for path in [first, second] {
            let mut edge = test_edge(&[]);
            edge.path = path;
            assert!(super::geometry_is_valid(
                &[edge],
                &nodes,
                test_bounds(),
                &[]
            ));
        }
        Ok(())
    }

    #[test]
    fn contextual_routing_preserves_reserved_ports_for_label_fallbacks()
    -> Result<(), Box<dyn Error>> {
        let nodes = [
            test_node("source", 20_000, 150_000),
            test_node("upper", 320_000, 50_000),
            test_node("lower", 320_000, 250_000),
        ];
        let edges = [ir_edge("source", "upper"), ir_edge("source", "lower")];
        let first = super::route_next(&edges[0], &edges, &[], &nodes, test_bounds(), &[], &[])
            .map_err(|_| "first route is missing")?;
        let second = super::route_next(
            &edges[1],
            &edges,
            std::slice::from_ref(&first),
            &nodes,
            test_bounds(),
            &[],
            &[],
        )
        .map_err(|_| "second route is missing")?;
        assert_ne!(first.path.first(), second.path.first());

        let alternatives = super::alternative_routes_with_context(
            &edges[1],
            &edges,
            std::slice::from_ref(&first),
            &nodes,
            test_bounds(),
            &[],
            &[],
        )
        .map_err(|_| "alternative routes are missing")?;
        let preferred = alternatives.first().ok_or("missing alternative route")?;
        assert_ne!(first.path.first(), preferred.path.first());
        assert!(super::geometry_is_valid(
            &[first, second, preferred.clone()],
            &nodes,
            test_bounds(),
            &[]
        ));
        Ok(())
    }

    #[test]
    fn bounded_alternatives_keep_midpoint_escape_routes_when_preferred_stubs_are_blocked()
    -> Result<(), Box<dyn Error>> {
        let source = test_node("source", 170_000, 150_000);
        let target = test_node("target", 320_000, 150_000);
        let mut duplicate_target_one = target.clone();
        duplicate_target_one.id = "duplicate-one".to_owned();
        let mut duplicate_target_two = target.clone();
        duplicate_target_two.id = "duplicate-two".to_owned();
        let nodes = [source, target, duplicate_target_one, duplicate_target_two];
        let current = ir_edge("source", "target");
        let context = [
            current.clone(),
            ir_edge("source", "duplicate-one"),
            ir_edge("source", "duplicate-two"),
        ];
        let blocked_stubs = [
            Rect {
                x: 270_000,
                y: 130_000,
                width: 20_000,
                height: 140_000,
            },
            Rect {
                x: 150_000,
                y: 120_000,
                width: 140_000,
                height: 30_000,
            },
            Rect {
                x: 150_000,
                y: 250_000,
                width: 140_000,
                height: 30_000,
            },
        ];

        let alternatives = super::alternative_routes_with_context(
            &current,
            &context,
            &[],
            &nodes,
            test_bounds(),
            &blocked_stubs,
            &[],
        )
        .map_err(|_| "alternative route search failed")?;
        assert!(!alternatives.is_empty());
        assert!(alternatives.iter().all(|edge| {
            edge.path
                .first()
                .is_some_and(|point| point.x == nodes[0].rect.x)
        }));
        Ok(())
    }

    #[test]
    fn self_edges_leave_and_return_through_different_normal_ports() -> Result<(), Box<dyn Error>> {
        let nodes = [test_node("source", 100_000, 100_000)];
        let mut router = super::GridRouter::new(&nodes, test_bounds(), &[], &[]);
        let path = router
            .route(nodes[0].rect, nodes[0].rect, EdgeDirection::Forward)
            .ok_or("self route is missing")?;
        assert_ne!(path.first(), path.last());
        let mut edge = test_edge(&[]);
        edge.to = "source".to_owned();
        edge.path = path;
        assert!(super::geometry_is_valid(
            &[edge],
            &nodes,
            test_bounds(),
            &[]
        ));
        Ok(())
    }

    #[test]
    fn routing_matches_cross_target_numeric_fixture() -> Result<(), Box<dyn Error>> {
        let scene = scene_from(
            b"stack 1.0 diagram \"Route parity\" { node a \"A\" node b \"B\" edge a -> b }",
        )?;
        assert_eq!(
            scene.edges[0].path,
            vec![
                super::Point {
                    x: 192_000,
                    y: 106_200,
                },
                super::Point {
                    x: 216_000,
                    y: 106_200,
                },
            ]
        );
        assert!(scene.geometry_is_valid());
        Ok(())
    }

    #[test]
    fn rejects_corrupted_edge_geometry() -> Result<(), Box<dyn Error>> {
        let scene = scene_from(
            b"stack 1.0 diagram \"Validate edge\" { node a \"A\" node b \"B\" edge a -> b \"Call\" }",
        )?;

        let mut invalid = scene.clone();
        invalid.edges[0].path.truncate(1);
        assert!(!invalid.geometry_is_valid());

        let mut invalid = scene.clone();
        invalid.edges[0].path[0].x += 1;
        assert!(!invalid.geometry_is_valid());

        let mut invalid = scene.clone();
        invalid.edges[0].path[1].y += 1;
        assert!(!invalid.geometry_is_valid());

        let mut invalid = scene.clone();
        invalid.edges[0].end_marker = Marker::None;
        assert!(!invalid.geometry_is_valid());

        let mut invalid = scene.clone();
        invalid.edges[0].label_anchor = None;
        assert!(!invalid.geometry_is_valid());

        let mut invalid = scene;
        invalid.edges[0].from = "missing".to_owned();
        assert!(!invalid.geometry_is_valid());
        Ok(())
    }

    #[test]
    fn missing_endpoints_and_enclosed_terminals_fail_without_partial_routes() {
        let nodes = [
            test_node("source", 20_000, 100_000),
            test_node("target", 320_000, 100_000),
        ];
        let edge = stack_compiler::ir::Edge {
            from: "source".into(),
            to: "target".into(),
            direction: EdgeDirection::Forward,
            kind: EdgeKind::Flow,
            label: None,
        };
        assert!(super::route(std::slice::from_ref(&edge), &nodes, test_bounds(), &[], &[]).is_ok());
        for missing in [0, 1] {
            let mut corrupted = edge.clone();
            if missing == 0 {
                corrupted.from = "absent".into();
            } else {
                corrupted.to = "absent".into();
            }
            assert!(
                super::route(
                    std::slice::from_ref(&corrupted),
                    &nodes,
                    test_bounds(),
                    &[],
                    &[]
                )
                .is_err()
            );
            assert!(
                super::alternative_routes(&corrupted, &nodes, test_bounds(), &[], &[]).is_err()
            );
        }
        let covering_obstacle = [test_bounds()];
        assert!(
            super::route(
                std::slice::from_ref(&edge),
                &nodes,
                test_bounds(),
                &covering_obstacle,
                &[]
            )
            .is_err()
        );
        assert_eq!(
            super::alternative_routes(&edge, &nodes, test_bounds(), &covering_obstacle, &[]),
            Ok(Vec::new())
        );
        let mut invalid = test_edge(&[(120_000, 150_000), (320_000, 150_000)]);
        invalid.to = "absent".into();
        assert!(!super::geometry_is_valid(
            &[invalid],
            &nodes,
            test_bounds(),
            &[]
        ));
    }

    #[test]
    fn unsupported_diagonal_segments_fail_closed_in_routing_predicates() {
        let start = Point { x: 0, y: 0 };
        let end = Point {
            x: 400_000,
            y: 400_000,
        };
        assert!(!super::segment_is_axis_aligned(start, end));
        assert!(!super::point_is_on_segment(start, start, end));
        assert!(super::segment_hits_rect(start, end, test_frame()));
        assert!(super::segment_crosses_rect_interior(
            start,
            end,
            test_frame()
        ));
        assert!(!super::frame_segment_is_clear(start, end, test_frame()));
    }
}
