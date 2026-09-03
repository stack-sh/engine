//! Deterministic orthogonal edge routing for the internal scene.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use stack_compiler::ir::{Edge, EdgeDirection, EdgeKind};

use crate::scene::{Rect, SceneNode};

const ROUTE_MARGIN: i64 = 8_000;
const BEND_PENALTY: i64 = 32_000;

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RoutingError;

pub(crate) fn route(
    edges: &[Edge],
    nodes: &[SceneNode],
    bounds: Rect,
) -> Result<Vec<SceneEdge>, RoutingError> {
    let router = GridRouter::new(nodes, bounds);
    edges
        .iter()
        .map(|edge| {
            let source = node_rect(nodes, &edge.from).ok_or(RoutingError)?;
            let target = node_rect(nodes, &edge.to).ok_or(RoutingError)?;
            let path = router.route(source, target).ok_or(RoutingError)?;
            let (start_marker, end_marker) = markers(edge.direction);
            let label_anchor = edge.label.as_ref().map(|_| path_midpoint(&path));
            Ok(SceneEdge {
                from: edge.from.clone(),
                to: edge.to.clone(),
                direction: edge.direction,
                kind: edge.kind,
                label: edge.label.clone(),
                path,
                start_marker,
                end_marker,
                label_anchor,
            })
        })
        .collect()
}

pub(crate) fn geometry_is_valid(edges: &[SceneEdge], nodes: &[SceneNode], bounds: Rect) -> bool {
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
            || edge.path.windows(2).any(|segment| {
                segment[0] == segment[1]
                    || !segment_is_axis_aligned(segment[0], segment[1])
                    || nodes.iter().any(|node| {
                        segment_crosses_rect_interior(segment[0], segment[1], node.rect)
                    })
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

fn ports(rect: Rect) -> [Point; 4] {
    [
        Point {
            x: rect.x + rect.width,
            y: rect.y + rect.height / 2,
        },
        Point {
            x: rect.x + rect.width / 2,
            y: rect.y + rect.height,
        },
        Point {
            x: rect.x,
            y: rect.y + rect.height / 2,
        },
        Point {
            x: rect.x + rect.width / 2,
            y: rect.y,
        },
    ]
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

#[derive(Debug)]
struct GridRouter<'a> {
    nodes: &'a [SceneNode],
    bounds: Rect,
    xs: Vec<i64>,
    ys: Vec<i64>,
    valid: Vec<bool>,
}

impl<'a> GridRouter<'a> {
    fn new(nodes: &'a [SceneNode], bounds: Rect) -> Self {
        let mut xs = vec![
            bounds.x + ROUTE_MARGIN,
            bounds.x + bounds.width - ROUTE_MARGIN,
        ];
        let mut ys = vec![
            bounds.y + ROUTE_MARGIN,
            bounds.y + bounds.height - ROUTE_MARGIN,
        ];
        for node in nodes {
            let rect = node.rect;
            xs.extend([
                rect.x - ROUTE_MARGIN,
                rect.x,
                rect.x + rect.width / 2,
                rect.x + rect.width,
                rect.x + rect.width + ROUTE_MARGIN,
            ]);
            ys.extend([
                rect.y - ROUTE_MARGIN,
                rect.y,
                rect.y + rect.height / 2,
                rect.y + rect.height,
                rect.y + rect.height + ROUTE_MARGIN,
            ]);
        }
        xs.retain(|x| *x >= bounds.x && *x <= bounds.x + bounds.width);
        ys.retain(|y| *y >= bounds.y && *y <= bounds.y + bounds.height);
        xs.sort_unstable();
        ys.sort_unstable();
        xs.dedup();
        ys.dedup();
        let valid = ys
            .iter()
            .flat_map(|y| {
                xs.iter().map(move |x| {
                    let point = Point { x: *x, y: *y };
                    nodes
                        .iter()
                        .all(|node| !node.rect.contains_point_interior(point))
                })
            })
            .collect();
        Self {
            nodes,
            bounds,
            xs,
            ys,
            valid,
        }
    }

    fn route(&self, source: Rect, target: Rect) -> Option<Vec<Point>> {
        let state_count = self.valid.len() * 3;
        let mut distances = vec![i64::MAX; state_count];
        let mut parents = vec![None; state_count];
        let mut pending = BinaryHeap::new();
        for port in ports(source) {
            let vertex = self.vertex(port)?;
            let state = vertex * 3;
            distances[state] = 0;
            pending.push(Reverse((0_i64, state)));
        }
        let target_vertices = ports(target)
            .into_iter()
            .map(|port| self.vertex(port))
            .collect::<Option<Vec<_>>>()?;

        while let Some(Reverse((cost, state))) = pending.pop() {
            if distances[state] != cost {
                continue;
            }
            let vertex = state / 3;
            let incoming_axis = state % 3;
            if incoming_axis != 0 && target_vertices.contains(&vertex) {
                return Some(self.reconstruct(state, &parents));
            }
            for (next_vertex, next_axis, length) in self.neighbors(vertex) {
                let bend = if incoming_axis != 0 && incoming_axis != next_axis {
                    BEND_PENALTY
                } else {
                    0
                };
                let next_state = next_vertex * 3 + next_axis;
                let next_cost = cost + length + bend;
                if next_cost < distances[next_state] {
                    distances[next_state] = next_cost;
                    parents[next_state] = Some(state);
                    pending.push(Reverse((next_cost, next_state)));
                }
            }
        }
        None
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

    fn neighbors(&self, vertex: usize) -> Vec<(usize, usize, i64)> {
        let x = vertex % self.xs.len();
        let y = vertex / self.xs.len();
        let mut neighbors = Vec::with_capacity(4);
        self.scan_neighbor(x, y, -1, 0, 1, &mut neighbors);
        self.scan_neighbor(x, y, 1, 0, 1, &mut neighbors);
        self.scan_neighbor(x, y, 0, -1, 2, &mut neighbors);
        self.scan_neighbor(x, y, 0, 1, 2, &mut neighbors);
        neighbors
    }

    fn scan_neighbor(
        &self,
        x: usize,
        y: usize,
        x_step: isize,
        y_step: isize,
        axis: usize,
        neighbors: &mut Vec<(usize, usize, i64)>,
    ) {
        let mut candidate_x = x as isize + x_step;
        let mut candidate_y = y as isize + y_step;
        while candidate_x >= 0
            && candidate_y >= 0
            && candidate_x < self.xs.len() as isize
            && candidate_y < self.ys.len() as isize
        {
            let candidate = candidate_y as usize * self.xs.len() + candidate_x as usize;
            if self.valid[candidate] {
                let start = self.point(y * self.xs.len() + x);
                let end = self.point(candidate);
                if self.bounds.contains_point(end)
                    && self
                        .nodes
                        .iter()
                        .all(|node| !segment_crosses_rect_interior(start, end, node.rect))
                {
                    neighbors.push((candidate, axis, manhattan(start, end)));
                }
                break;
            }
            candidate_x += x_step;
            candidate_y += y_step;
        }
    }

    fn reconstruct(&self, state: usize, parents: &[Option<usize>]) -> Vec<Point> {
        let mut states = Vec::new();
        let mut cursor = Some(state);
        while let Some(current) = cursor {
            states.push(current);
            cursor = parents[current];
        }
        states.reverse();

        let mut path = Vec::new();
        for state in states {
            let point = self.point(state / 3);
            if path.last() == Some(&point) {
                continue;
            }
            if path.len() >= 2 {
                let previous: Point = path[path.len() - 2];
                let last: Point = path[path.len() - 1];
                if (previous.x == last.x && last.x == point.x)
                    || (previous.y == last.y && last.y == point.y)
                {
                    path.pop();
                }
            }
            path.push(point);
        }
        path
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use stack_compiler::ir::{EdgeDirection, EdgeKind};

    use super::Marker;

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
        let scene = scene_from(
            b"stack 1.0 diagram \"Obstacle\" { layout { direction right } node left \"Left\" node blocker \"Blocker\" node right \"Right\" edge left -> right }",
        )?;
        assert!(scene.edges[0].path.len() >= 4);
        assert!(scene.geometry_is_valid());
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
}
