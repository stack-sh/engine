//! SVG-independent deterministic scene geometry.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use stack_compiler::ir::{Diagram, Direction, ElementId, Group, Layout, Node};
use stack_theme::{Catalog, FontMetrics, NodeShape, Theme, Typography};

use crate::resources::node_visual;
use crate::routing::{self, Point, SceneEdge};

const NODE_MIN_WIDTH: i64 = 160_000;
const NODE_MIN_HEIGHT: i64 = 72_000;
const NODE_HORIZONTAL_PADDING: i64 = 20_000;
const NODE_VERTICAL_PADDING: i64 = 16_000;
const NODE_ICON_SIZE: i64 = 24_000;
const NODE_ICON_GAP: i64 = 12_000;
const NODE_DETAIL_GAP: i64 = 4_000;
const ITEM_GAP: i64 = 24_000;
const GROUP_PADDING: i64 = 24_000;
const GROUP_LABEL_GAP: i64 = 12_000;
const DIAGRAM_PADDING: i64 = 32_000;
const DIAGRAM_TITLE_GAP: i64 = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SceneDirection {
    Right,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rect {
    pub(crate) x: i64,
    pub(crate) y: i64,
    pub(crate) width: i64,
    pub(crate) height: i64,
}

impl Rect {
    fn contains(self, other: Self) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.x + other.width <= self.x + self.width
            && other.y + other.height <= self.y + self.height
    }

    fn overlaps(self, other: Self) -> bool {
        self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SceneNode {
    pub(crate) id: String,
    pub(crate) parent_group_id: Option<String>,
    pub(crate) rect: Rect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SceneGroup {
    pub(crate) id: String,
    pub(crate) parent_group_id: Option<String>,
    pub(crate) rect: Rect,
    pub(crate) content_rect: Rect,
    pub(crate) direction: SceneDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Scene {
    pub(crate) bounds: Rect,
    pub(crate) content_rect: Rect,
    pub(crate) direction: SceneDirection,
    pub(crate) nodes: Vec<SceneNode>,
    pub(crate) groups: Vec<SceneGroup>,
    pub(crate) edges: Vec<SceneEdge>,
    pub(crate) unsatisfied_orders: Vec<SceneScope>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SceneScope {
    Diagram,
    Group(String),
}

impl Scene {
    pub(crate) fn geometry_is_valid(&self) -> bool {
        if self.bounds.width <= 0
            || self.bounds.height <= 0
            || self.content_rect.width <= 0
            || self.content_rect.height <= 0
            || !self.bounds.contains(self.content_rect)
        {
            return false;
        }
        let group_ids = self
            .groups
            .iter()
            .map(|group| group.id.as_str())
            .collect::<BTreeSet<_>>();
        let node_ids = self
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<BTreeSet<_>>();
        if group_ids.len() != self.groups.len() || node_ids.len() != self.nodes.len() {
            return false;
        }

        for node in &self.nodes {
            let Some(parent) = self.parent_content_rect(node.parent_group_id.as_deref()) else {
                return false;
            };
            if node.rect.width <= 0 || node.rect.height <= 0 || !parent.contains(node.rect) {
                return false;
            }
        }
        for group in &self.groups {
            let Some(parent) = self.parent_content_rect(group.parent_group_id.as_deref()) else {
                return false;
            };
            if group.rect.width <= 0
                || group.rect.height <= 0
                || group.content_rect.width <= 0
                || group.content_rect.height <= 0
                || !parent.contains(group.rect)
                || !group.rect.contains(group.content_rect)
            {
                return false;
            }
        }

        for left in 0..self.nodes.len() {
            for right in (left + 1)..self.nodes.len() {
                if self.nodes[left].parent_group_id == self.nodes[right].parent_group_id
                    && self.nodes[left].rect.overlaps(self.nodes[right].rect)
                {
                    return false;
                }
            }
        }
        for left in 0..self.groups.len() {
            for right in (left + 1)..self.groups.len() {
                if self.groups[left].parent_group_id == self.groups[right].parent_group_id
                    && self.groups[left].rect.overlaps(self.groups[right].rect)
                {
                    return false;
                }
            }
        }
        for node in &self.nodes {
            for group in &self.groups {
                if node.parent_group_id == group.parent_group_id && node.rect.overlaps(group.rect) {
                    return false;
                }
            }
        }

        if !routing::geometry_is_valid(&self.edges, &self.nodes, self.bounds) {
            return false;
        }

        true
    }

    fn parent_content_rect(&self, parent_group_id: Option<&str>) -> Option<Rect> {
        match parent_group_id {
            Some(parent_group_id) => self
                .groups
                .iter()
                .find(|group| group.id == parent_group_id)
                .map(|group| group.content_rect),
            None => Some(self.content_rect),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SceneError {
    MissingTheme,
    MissingFontMetrics,
    InvalidIntermediateRepresentation,
    EdgeRoutingFailed,
}

impl SceneError {
    pub(crate) const fn reason(self) -> &'static str {
        match self {
            Self::MissingTheme => "no requested or fallback theme is available",
            Self::MissingFontMetrics => "theme references unavailable font metrics",
            Self::InvalidIntermediateRepresentation => {
                "normalized containment references are inconsistent"
            }
            Self::EdgeRoutingFailed => "an edge could not be routed outside node interiors",
        }
    }
}

impl fmt::Display for SceneError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.reason())
    }
}

impl Error for SceneError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Size {
    width: i64,
    height: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlacedItem {
    index: usize,
    rect: Rect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Arrangement {
    size: Size,
    direction: SceneDirection,
    items: Vec<PlacedItem>,
}

pub(crate) fn layout(diagram: &Diagram, catalog: &Catalog) -> Result<Scene, SceneError> {
    let theme = selected_theme(diagram, catalog)?;
    let metrics = catalog
        .font_metrics
        .iter()
        .find(|metrics| metrics.id == theme.typography.font_metrics_id)
        .ok_or(SceneError::MissingFontMetrics)?;

    let mut sizes = BTreeMap::new();
    for node in &diagram.nodes {
        sizes.insert(node.id.clone(), node_size(node, theme, metrics));
    }
    for group in diagram.groups.iter().rev() {
        let children = arrange(&group.children, group.layout.as_ref(), &sizes)?;
        sizes.insert(
            group.id.clone(),
            group_size(group, children.size, &theme.typography, metrics),
        );
    }

    let root = arrange(&diagram.children, diagram.layout.as_ref(), &sizes)?;
    let title_height = line_height(
        theme.typography.group_label_size_milli_px,
        &theme.typography,
    );
    let title_width = text_width(
        &diagram.title,
        theme.typography.group_label_size_milli_px,
        metrics,
    );
    let bounds = Rect {
        x: 0,
        y: 0,
        width: (root.size.width + 2 * DIAGRAM_PADDING).max(title_width + 2 * DIAGRAM_PADDING),
        height: root.size.height + 2 * DIAGRAM_PADDING + title_height + DIAGRAM_TITLE_GAP,
    };
    let root_origin = Point {
        x: DIAGRAM_PADDING,
        y: DIAGRAM_PADDING + title_height + DIAGRAM_TITLE_GAP,
    };
    let content_rect = Rect {
        x: root_origin.x,
        y: root_origin.y,
        width: root.size.width,
        height: root.size.height,
    };
    let mut placer = Placer::new(diagram, &sizes, theme);
    placer.place_scope(&diagram.children, diagram.layout.as_ref(), root_origin)?;

    let nodes = diagram
        .nodes
        .iter()
        .map(|node| {
            placer
                .node_rects
                .get(&node.id)
                .copied()
                .map(|rect| SceneNode {
                    id: node.id.clone(),
                    parent_group_id: node.parent_group_id.clone(),
                    rect,
                })
                .ok_or(SceneError::InvalidIntermediateRepresentation)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let groups = diagram
        .groups
        .iter()
        .map(|group| {
            let rect = placer
                .group_rects
                .get(&group.id)
                .copied()
                .ok_or(SceneError::InvalidIntermediateRepresentation)?;
            let content_rect = placer
                .group_content_rects
                .get(&group.id)
                .copied()
                .ok_or(SceneError::InvalidIntermediateRepresentation)?;
            let direction = placer
                .group_directions
                .get(&group.id)
                .copied()
                .ok_or(SceneError::InvalidIntermediateRepresentation)?;
            Ok(SceneGroup {
                id: group.id.clone(),
                parent_group_id: group.parent_group_id.clone(),
                rect,
                content_rect,
                direction,
            })
        })
        .collect::<Result<Vec<_>, SceneError>>()?;
    let edges = routing::route(&diagram.edges, &nodes, bounds)
        .map_err(|_| SceneError::EdgeRoutingFailed)?;
    let unsatisfied_orders = unsatisfied_orders(diagram, &nodes, &groups)?;

    Ok(Scene {
        bounds,
        content_rect,
        direction: root.direction,
        nodes,
        groups,
        edges,
        unsatisfied_orders,
    })
}

fn unsatisfied_orders(
    diagram: &Diagram,
    nodes: &[SceneNode],
    groups: &[SceneGroup],
) -> Result<Vec<SceneScope>, SceneError> {
    let mut unsatisfied = Vec::new();
    if let Some(layout) = &diagram.layout {
        if let Some(order) = &layout.order {
            let direction = resolve_direction(diagram.children.len(), layout.direction);
            if !order_is_satisfied(order, direction, nodes, groups) {
                unsatisfied.push(SceneScope::Diagram);
            }
        }
    }

    for group in &diagram.groups {
        let Some(layout) = &group.layout else {
            continue;
        };
        let Some(order) = &layout.order else {
            continue;
        };
        let direction = groups
            .iter()
            .find(|scene_group| scene_group.id == group.id)
            .map(|scene_group| scene_group.direction)
            .ok_or(SceneError::InvalidIntermediateRepresentation)?;
        if !order_is_satisfied(order, direction, nodes, groups) {
            unsatisfied.push(SceneScope::Group(group.id.clone()));
        }
    }
    Ok(unsatisfied)
}

fn order_is_satisfied(
    order: &[String],
    direction: SceneDirection,
    nodes: &[SceneNode],
    groups: &[SceneGroup],
) -> bool {
    order.windows(2).all(|pair| {
        match (
            element_rect(&pair[0], nodes, groups),
            element_rect(&pair[1], nodes, groups),
        ) {
            (Some(left), Some(right)) => {
                cross_axis_position(left, direction) < cross_axis_position(right, direction)
            }
            _ => false,
        }
    })
}

fn element_rect(identifier: &str, nodes: &[SceneNode], groups: &[SceneGroup]) -> Option<Rect> {
    nodes
        .iter()
        .find(|node| node.id == identifier)
        .map(|node| node.rect)
        .or_else(|| {
            groups
                .iter()
                .find(|group| group.id == identifier)
                .map(|group| group.rect)
        })
}

fn cross_axis_position(rect: Rect, direction: SceneDirection) -> i64 {
    match direction {
        SceneDirection::Right => 2 * rect.y + rect.height,
        SceneDirection::Down => 2 * rect.x + rect.width,
    }
}

fn selected_theme<'a>(diagram: &Diagram, catalog: &'a Catalog) -> Result<&'a Theme, SceneError> {
    catalog
        .themes
        .iter()
        .find(|theme| theme.id == diagram.theme_id)
        .or_else(|| {
            catalog
                .themes
                .iter()
                .find(|theme| theme.id == catalog.fallbacks.missing_theme_id)
        })
        .ok_or(SceneError::MissingTheme)
}

fn node_size(node: &Node, theme: &Theme, metrics: &FontMetrics) -> Size {
    let label_width = text_width(
        &node.label,
        theme.typography.node_label_size_milli_px,
        metrics,
    );
    let detail_width = node.detail.as_deref().map_or(0, |detail| {
        text_width(detail, theme.typography.node_detail_size_milli_px, metrics)
    });
    let content_width = label_width.max(detail_width) + NODE_ICON_SIZE + NODE_ICON_GAP;
    let label_height = line_height(theme.typography.node_label_size_milli_px, &theme.typography);
    let detail_height = node.detail.as_ref().map_or(0, |_| {
        NODE_DETAIL_GAP
            + line_height(
                theme.typography.node_detail_size_milli_px,
                &theme.typography,
            )
    });

    let size = Size {
        width: NODE_MIN_WIDTH.max(content_width + 2 * NODE_HORIZONTAL_PADDING),
        height: NODE_MIN_HEIGHT.max(label_height + detail_height + 2 * NODE_VERTICAL_PADDING),
    };
    if matches!(node_visual(theme, node.kind).shape, NodeShape::Circle) {
        let diameter = size.width.max(size.height);
        Size {
            width: diameter,
            height: diameter,
        }
    } else {
        size
    }
}

fn group_size(
    group: &Group,
    content: Size,
    typography: &Typography,
    metrics: &FontMetrics,
) -> Size {
    let label_width = text_width(&group.label, typography.group_label_size_milli_px, metrics);
    let label_height = line_height(typography.group_label_size_milli_px, typography);
    Size {
        width: (content.width + 2 * GROUP_PADDING).max(label_width + 2 * GROUP_PADDING),
        height: content.height + 2 * GROUP_PADDING + label_height + GROUP_LABEL_GAP,
    }
}

struct Placer<'a> {
    diagram: &'a Diagram,
    sizes: &'a BTreeMap<String, Size>,
    theme: &'a Theme,
    node_rects: BTreeMap<String, Rect>,
    group_rects: BTreeMap<String, Rect>,
    group_content_rects: BTreeMap<String, Rect>,
    group_directions: BTreeMap<String, SceneDirection>,
}

impl<'a> Placer<'a> {
    fn new(diagram: &'a Diagram, sizes: &'a BTreeMap<String, Size>, theme: &'a Theme) -> Self {
        Self {
            diagram,
            sizes,
            theme,
            node_rects: BTreeMap::new(),
            group_rects: BTreeMap::new(),
            group_content_rects: BTreeMap::new(),
            group_directions: BTreeMap::new(),
        }
    }

    fn place_scope(
        &mut self,
        children: &[ElementId],
        layout: Option<&Layout>,
        origin: Point,
    ) -> Result<(), SceneError> {
        let arrangement = arrange(children, layout, self.sizes)?;
        for placed in arrangement.items {
            let child = children
                .get(placed.index)
                .ok_or(SceneError::InvalidIntermediateRepresentation)?;
            let rect = Rect {
                x: origin.x + placed.rect.x,
                y: origin.y + placed.rect.y,
                width: placed.rect.width,
                height: placed.rect.height,
            };
            match child {
                ElementId::Node(identifier) => {
                    if self.diagram.nodes.iter().all(|node| node.id != *identifier) {
                        return Err(SceneError::InvalidIntermediateRepresentation);
                    }
                    self.node_rects.insert(identifier.clone(), rect);
                }
                ElementId::Group(identifier) => {
                    let group = self
                        .diagram
                        .groups
                        .iter()
                        .find(|group| group.id == *identifier)
                        .ok_or(SceneError::InvalidIntermediateRepresentation)?;
                    let child_arrangement =
                        arrange(&group.children, group.layout.as_ref(), self.sizes)?;
                    let label_height = line_height(
                        self.theme.typography.group_label_size_milli_px,
                        &self.theme.typography,
                    );
                    let content_origin = Point {
                        x: rect.x + GROUP_PADDING,
                        y: rect.y + GROUP_PADDING + label_height + GROUP_LABEL_GAP,
                    };
                    self.group_rects.insert(identifier.clone(), rect);
                    self.group_content_rects.insert(
                        identifier.clone(),
                        Rect {
                            x: content_origin.x,
                            y: content_origin.y,
                            width: child_arrangement.size.width,
                            height: child_arrangement.size.height,
                        },
                    );
                    self.group_directions
                        .insert(identifier.clone(), child_arrangement.direction);
                    self.place_scope(&group.children, group.layout.as_ref(), content_origin)?;
                }
            }
        }
        Ok(())
    }
}

fn arrange(
    children: &[ElementId],
    layout: Option<&Layout>,
    sizes: &BTreeMap<String, Size>,
) -> Result<Arrangement, SceneError> {
    if children.is_empty() {
        return Err(SceneError::InvalidIntermediateRepresentation);
    }
    let direction = resolve_direction(children.len(), layout.and_then(|layout| layout.direction));
    let ranks = ranks(children, layout);
    let mut items = Vec::with_capacity(children.len());
    let mut primary_cursor = 0;
    let mut cross_extent = 0;

    for rank in ranks {
        let mut primary_extent = 0;
        let mut rank_cross_cursor = 0;
        for index in &rank {
            let size = sizes
                .get(children[*index].as_str())
                .copied()
                .ok_or(SceneError::InvalidIntermediateRepresentation)?;
            primary_extent = primary_extent.max(match direction {
                SceneDirection::Right => size.width,
                SceneDirection::Down => size.height,
            });
            rank_cross_cursor += match direction {
                SceneDirection::Right => size.height,
                SceneDirection::Down => size.width,
            };
        }
        rank_cross_cursor += ITEM_GAP * (rank.len().saturating_sub(1) as i64);

        let mut cross_cursor = 0;
        for index in rank {
            let size = sizes
                .get(children[index].as_str())
                .copied()
                .ok_or(SceneError::InvalidIntermediateRepresentation)?;
            let rect = match direction {
                SceneDirection::Right => Rect {
                    x: primary_cursor,
                    y: cross_cursor,
                    width: size.width,
                    height: size.height,
                },
                SceneDirection::Down => Rect {
                    x: cross_cursor,
                    y: primary_cursor,
                    width: size.width,
                    height: size.height,
                },
            };
            cross_cursor += match direction {
                SceneDirection::Right => size.height + ITEM_GAP,
                SceneDirection::Down => size.width + ITEM_GAP,
            };
            items.push(PlacedItem { index, rect });
        }
        cross_extent = cross_extent.max(rank_cross_cursor);
        primary_cursor += primary_extent + ITEM_GAP;
    }

    primary_cursor -= ITEM_GAP;
    let size = match direction {
        SceneDirection::Right => Size {
            width: primary_cursor,
            height: cross_extent,
        },
        SceneDirection::Down => Size {
            width: cross_extent,
            height: primary_cursor,
        },
    };
    Ok(Arrangement {
        size,
        direction,
        items,
    })
}

fn ranks(children: &[ElementId], layout: Option<&Layout>) -> Vec<Vec<usize>> {
    let same_ranks = layout.map_or(&[][..], |layout| layout.same_ranks.as_slice());
    let order = layout.and_then(|layout| layout.order.as_deref());
    let mut assigned = vec![false; children.len()];
    let mut ranks = Vec::new();

    for index in 0..children.len() {
        if assigned[index] {
            continue;
        }
        let identifier = children[index].as_str();
        let mut rank = same_ranks
            .iter()
            .find(|rank| rank.iter().any(|entry| entry == identifier))
            .map_or_else(
                || vec![index],
                |same_rank| {
                    children
                        .iter()
                        .enumerate()
                        .filter_map(|(candidate, child)| {
                            same_rank
                                .iter()
                                .any(|entry| entry == child.as_str())
                                .then_some(candidate)
                        })
                        .collect()
                },
            );
        for member in &rank {
            assigned[*member] = true;
        }
        if let Some(order) = order {
            rank.sort_by_key(|member| {
                order
                    .iter()
                    .position(|entry| entry == children[*member].as_str())
                    .map_or((1, *member), |position| (0, position))
            });
        }
        ranks.push(rank);
    }
    ranks
}

fn resolve_direction(child_count: usize, authored: Option<Direction>) -> SceneDirection {
    match authored {
        Some(Direction::Right) => SceneDirection::Right,
        Some(Direction::Down) => SceneDirection::Down,
        None if child_count <= 3 => SceneDirection::Right,
        None => SceneDirection::Down,
    }
}

pub(crate) fn text_width(text: &str, size_milli_px: u32, metrics: &FontMetrics) -> i64 {
    let advance = text
        .chars()
        .map(|character| glyph_advance(character, metrics) as i64)
        .sum::<i64>();
    let units_per_em = i64::from(metrics.units_per_em);
    (advance * i64::from(size_milli_px) + units_per_em - 1) / units_per_em
}

fn glyph_advance(character: char, metrics: &FontMetrics) -> u32 {
    let scalar = character as u32;
    let key = format!("U+{scalar:04X}");
    if let Some(advance) = metrics.glyph_advances.get(&key) {
        return *advance;
    }
    if metrics.wide_ranges.iter().any(|range| {
        let start = unicode_scalar(&range.start);
        let end = unicode_scalar(&range.end);
        start.is_some_and(|start| scalar >= start) && end.is_some_and(|end| scalar <= end)
    }) {
        metrics.wide_advance
    } else {
        metrics.default_advance
    }
}

fn unicode_scalar(label: &str) -> Option<u32> {
    label
        .strip_prefix("U+")
        .and_then(|digits| u32::from_str_radix(digits, 16).ok())
}

pub(crate) fn line_height(size_milli_px: u32, typography: &Typography) -> i64 {
    (i64::from(size_milli_px) * i64::from(typography.line_height_permille) + 999) / 1000
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use stack_compiler::ir::ElementId;

    use super::{Scene, SceneDirection, SceneError, glyph_advance, layout, selected_theme};

    fn scene_from(source: &[u8]) -> Result<Scene, Box<dyn Error>> {
        let compiled = stack_compiler::compile_bytes(source);
        if !compiled.diagnostics.is_empty() {
            return Err("fixture produced compiler diagnostics".into());
        }
        let diagram = compiled.diagram.ok_or("fixture produced no diagram")?;
        Ok(layout(&diagram, stack_theme::catalog())?)
    }

    fn node<'a>(scene: &'a Scene, identifier: &str) -> Option<&'a super::SceneNode> {
        scene.nodes.iter().find(|node| node.id == identifier)
    }

    #[test]
    fn right_direction_satisfies_same_rank_and_cross_axis_order() -> Result<(), Box<dyn Error>> {
        let scene = scene_from(
            b"stack 1.0 diagram \"Right\" { layout { direction right rank same [a, b] order [b, a] } node a \"A\" node b \"B\" node c \"C\" }",
        )?;
        let a = node(&scene, "a").ok_or("missing a")?;
        let b = node(&scene, "b").ok_or("missing b")?;
        let c = node(&scene, "c").ok_or("missing c")?;

        assert_eq!(scene.direction, SceneDirection::Right);
        assert_eq!(a.rect.x, b.rect.x);
        assert!(b.rect.y < a.rect.y);
        assert!(c.rect.x >= a.rect.x + a.rect.width);
        assert!(scene.geometry_is_valid());
        Ok(())
    }

    #[test]
    fn down_direction_satisfies_same_rank_and_cross_axis_order() -> Result<(), Box<dyn Error>> {
        let scene = scene_from(
            b"stack 1.0 diagram \"Down\" { layout { direction down rank same [a, b] order [b, a] } node a \"A\" node b \"B\" node c \"C\" }",
        )?;
        let a = node(&scene, "a").ok_or("missing a")?;
        let b = node(&scene, "b").ok_or("missing b")?;
        let c = node(&scene, "c").ok_or("missing c")?;

        assert_eq!(scene.direction, SceneDirection::Down);
        assert_eq!(a.rect.y, b.rect.y);
        assert!(b.rect.x < a.rect.x);
        assert!(c.rect.y >= a.rect.y + a.rect.height);
        assert!(scene.geometry_is_valid());
        Ok(())
    }

    #[test]
    fn automatic_layout_is_repeatable_and_does_not_inherit() -> Result<(), Box<dyn Error>> {
        let source = b"stack 1.0 diagram \"Auto\" { group pair \"Pair\" { node a \"A\" node b \"B\" } node c \"C\" node d \"D\" node e \"E\" }";
        let first = scene_from(source)?;
        let second = scene_from(source)?;
        assert_eq!(first, second);
        assert_eq!(first.direction, SceneDirection::Down);
        assert_eq!(first.groups[0].direction, SceneDirection::Right);
        assert!(first.geometry_is_valid());
        Ok(())
    }

    #[test]
    fn nested_groups_and_nodes_stay_inside_non_overlapping_parents() -> Result<(), Box<dyn Error>> {
        let scene = scene_from(
            b"stack 1.0 diagram \"Nested\" { layout { direction right } node user \"User\" group system \"System\" { layout { direction down } node web \"Web\" group data \"Data\" { node db \"Database\" { kind database detail \"Primary\" } node cache \"Cache\" { kind cache } } node vendor \"Vendor\" { kind external } } }",
        )?;
        assert_eq!(scene.nodes.len(), 5);
        assert_eq!(scene.groups.len(), 2);
        for group in &scene.groups {
            assert_eq!(group.content_rect.x - group.rect.x, super::GROUP_PADDING);
            assert!(group.rect.contains(group.content_rect));
        }
        for node in &scene.nodes {
            if let Some(parent_group_id) = node.parent_group_id.as_deref() {
                let parent = scene
                    .groups
                    .iter()
                    .find(|group| group.id == parent_group_id)
                    .ok_or("missing parent group")?;
                assert!(parent.content_rect.contains(node.rect));
            }
        }
        assert!(scene.geometry_is_valid());
        Ok(())
    }

    #[test]
    fn geometry_coordinates_have_target_independent_width() -> Result<(), Box<dyn Error>> {
        let scene = scene_from(b"stack 1.0 diagram \"Width\" { node a \"A\" }")?;
        assert_eq!(std::mem::size_of_val(&scene.bounds.x), 8);
        assert_eq!(std::mem::size_of_val(&scene.nodes[0].rect.width), 8);
        Ok(())
    }

    #[test]
    fn geometry_matches_cross_target_numeric_fixture() -> Result<(), Box<dyn Error>> {
        let scene =
            scene_from(b"stack 1.0 diagram \"Parity\" { node first \"A\" node second \"BB\" }")?;
        assert_eq!(
            scene.bounds,
            super::Rect {
                x: 0,
                y: 0,
                width: 408_000,
                height: 174_200,
            }
        );
        assert_eq!(
            scene.content_rect,
            super::Rect {
                x: 32_000,
                y: 70_200,
                width: 344_000,
                height: 72_000,
            }
        );
        assert_eq!(
            scene.nodes.iter().map(|node| node.rect).collect::<Vec<_>>(),
            vec![
                super::Rect {
                    x: 32_000,
                    y: 70_200,
                    width: 160_000,
                    height: 72_000,
                },
                super::Rect {
                    x: 216_000,
                    y: 70_200,
                    width: 160_000,
                    height: 72_000,
                },
            ]
        );
        Ok(())
    }

    #[test]
    fn geometry_validation_rejects_corrupted_scene_data() -> Result<(), Box<dyn Error>> {
        let scene = scene_from(
            b"stack 1.0 diagram \"Validate\" { node a \"A\" node b \"B\" group pair \"Pair\" { node c \"C\" } }",
        )?;

        let mut invalid = scene.clone();
        invalid.bounds.width = 0;
        assert!(!invalid.geometry_is_valid());

        let mut invalid = scene.clone();
        invalid.content_rect.x = invalid.bounds.width;
        assert!(!invalid.geometry_is_valid());

        let mut invalid = scene.clone();
        invalid.nodes.push(invalid.nodes[0].clone());
        assert!(!invalid.geometry_is_valid());

        let mut invalid = scene.clone();
        invalid.nodes[0].parent_group_id = Some("missing".to_owned());
        assert!(!invalid.geometry_is_valid());

        let mut invalid = scene.clone();
        invalid.nodes[0].rect.width = 0;
        assert!(!invalid.geometry_is_valid());

        let mut invalid = scene.clone();
        invalid.nodes[1].rect = invalid.nodes[0].rect;
        assert!(!invalid.geometry_is_valid());

        let mut invalid = scene.clone();
        invalid.groups[0].parent_group_id = Some("missing".to_owned());
        assert!(!invalid.geometry_is_valid());

        let mut invalid = scene.clone();
        invalid.groups[0].content_rect.width = 0;
        assert!(!invalid.geometry_is_valid());

        let mut invalid = scene;
        invalid.nodes[0].rect = invalid.groups[0].rect;
        assert!(!invalid.geometry_is_valid());
        Ok(())
    }

    #[test]
    fn versioned_metrics_make_wide_scalar_measurement_explicit() -> Result<(), Box<dyn Error>> {
        let compiled =
            stack_compiler::compile_bytes(b"stack 1.0 diagram \"Metrics\" { node a \"A\" }");
        let diagram = compiled.diagram.ok_or("missing diagram")?;
        let catalog = stack_theme::catalog();
        let theme = selected_theme(&diagram, catalog)?;
        let metrics = catalog
            .font_metrics
            .iter()
            .find(|metrics| metrics.id == theme.typography.font_metrics_id)
            .ok_or("missing metrics")?;

        assert_eq!(glyph_advance('A', metrics), 600);
        assert_eq!(glyph_advance('日', metrics), 1000);
        assert_eq!(glyph_advance('🚀', metrics), 1000);
        assert_eq!(glyph_advance('i', metrics), 250);
        Ok(())
    }

    #[test]
    fn malformed_catalog_and_ir_return_scene_errors() -> Result<(), Box<dyn Error>> {
        let compiled =
            stack_compiler::compile_bytes(b"stack 1.0 diagram \"Errors\" { node a \"A\" }");
        let mut diagram = compiled.diagram.ok_or("missing diagram")?;
        let mut catalog = stack_theme::catalog().clone();
        catalog.themes.clear();
        assert_eq!(layout(&diagram, &catalog), Err(SceneError::MissingTheme));

        let mut catalog = stack_theme::catalog().clone();
        catalog.font_metrics.clear();
        assert_eq!(
            layout(&diagram, &catalog),
            Err(SceneError::MissingFontMetrics)
        );

        diagram.children.push(ElementId::Node("missing".to_owned()));
        assert_eq!(
            layout(&diagram, stack_theme::catalog()),
            Err(SceneError::InvalidIntermediateRepresentation)
        );
        assert_eq!(
            SceneError::MissingTheme.to_string(),
            "no requested or fallback theme is available"
        );
        assert_eq!(
            SceneError::MissingFontMetrics.to_string(),
            "theme references unavailable font metrics"
        );
        assert_eq!(
            SceneError::InvalidIntermediateRepresentation.to_string(),
            "normalized containment references are inconsistent"
        );
        assert_eq!(
            SceneError::EdgeRoutingFailed.to_string(),
            "an edge could not be routed outside node interiors"
        );
        Ok(())
    }

    #[cfg(feature = "conformance")]
    #[test]
    fn canonical_complete_semantics_matches_snapshot() -> Result<(), Box<dyn Error>> {
        let specification = std::env::var("STACK_SPECIFICATION_DIR")?;
        let source = std::fs::read(
            std::path::Path::new(&specification)
                .join("conformance/valid/complete-semantics/source.stack"),
        )?;
        let scene = scene_from(&source)?;
        let actual = scene_snapshot(&scene);
        let expected = include_str!("../tests/snapshots/complete-semantics.scene.txt");
        assert_eq!(actual, expected);
        Ok(())
    }

    #[cfg(feature = "conformance")]
    #[test]
    fn canonical_examples_produce_valid_routed_scenes() -> Result<(), Box<dyn Error>> {
        let specification = std::env::var("STACK_SPECIFICATION_DIR")?;
        let examples_root = std::path::Path::new(&specification).join("examples");
        let mut examples = std::fs::read_dir(&examples_root)?.collect::<Result<Vec<_>, _>>()?;
        examples.sort_by_key(|entry| entry.file_name());
        if examples.is_empty() {
            return Err(format!("no examples found in {}", examples_root.display()).into());
        }

        for entry in examples {
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("stack") {
                continue;
            }
            let source = std::fs::read(&path)?;
            let scene = scene_from(&source)?;
            if !scene.geometry_is_valid() {
                return Err(format!("{} produced invalid scene geometry", path.display()).into());
            }
        }
        Ok(())
    }

    #[cfg(feature = "conformance")]
    fn scene_snapshot(scene: &Scene) -> String {
        let mut output = format!(
            "scene|bounds={},{},{},{}|content={},{},{},{}|direction={:?}\n",
            scene.bounds.x,
            scene.bounds.y,
            scene.bounds.width,
            scene.bounds.height,
            scene.content_rect.x,
            scene.content_rect.y,
            scene.content_rect.width,
            scene.content_rect.height,
            scene.direction
        );
        for group in &scene.groups {
            output.push_str(&format!(
                "group|{}|parent={}|rect={},{},{},{}|content={},{},{},{}|direction={:?}\n",
                group.id,
                group.parent_group_id.as_deref().unwrap_or("-"),
                group.rect.x,
                group.rect.y,
                group.rect.width,
                group.rect.height,
                group.content_rect.x,
                group.content_rect.y,
                group.content_rect.width,
                group.content_rect.height,
                group.direction
            ));
        }
        for node in &scene.nodes {
            output.push_str(&format!(
                "node|{}|parent={}|rect={},{},{},{}\n",
                node.id,
                node.parent_group_id.as_deref().unwrap_or("-"),
                node.rect.x,
                node.rect.y,
                node.rect.width,
                node.rect.height
            ));
        }
        for edge in &scene.edges {
            let path = edge
                .path
                .iter()
                .map(|point| format!("{},{}", point.x, point.y))
                .collect::<Vec<_>>()
                .join(";");
            output.push_str(&format!(
                "edge|{}|{}|direction={:?}|kind={:?}|label={}|markers={:?},{:?}|anchor={}|path={}\n",
                edge.from,
                edge.to,
                edge.direction,
                edge.kind,
                edge.label.as_deref().unwrap_or("-"),
                edge.start_marker,
                edge.end_marker,
                edge.label_anchor.map_or_else(
                    || "-".to_owned(),
                    |point| format!("{},{}", point.x, point.y)
                ),
                path
            ));
        }
        output
    }
}
