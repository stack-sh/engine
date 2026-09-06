//! Independent integer geometry for the test-only layout quality gates.
//!
//! Coordinates and stroke radii use thousandths of a CSS pixel. Computations use
//! wider intermediates so that adding dimensions or expanding an obstacle never
//! wraps at an `i64` coordinate boundary.

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Rect {
    pub(super) x: i64,
    pub(super) y: i64,
    pub(super) width: i64,
    pub(super) height: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Point {
    pub(super) x: i64,
    pub(super) y: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum GeometryError {
    InvalidRect,
    NegativeRadius,
    DiagonalSegment,
}

impl fmt::Display for GeometryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRect => "segment obstacles must have positive width and height",
            Self::NegativeRadius => "stroke radius must not be negative",
            Self::DiagonalSegment => "only axis-aligned segments are supported",
        })
    }
}

impl std::error::Error for GeometryError {}

#[derive(Clone, Copy)]
struct Bounds {
    left: i128,
    top: i128,
    right: i128,
    bottom: i128,
}

impl Bounds {
    fn new(rect: Rect) -> Self {
        Self {
            left: i128::from(rect.x),
            top: i128::from(rect.y),
            right: i128::from(rect.x) + i128::from(rect.width),
            bottom: i128::from(rect.y) + i128::from(rect.height),
        }
    }

    fn expand(self, radius: i64) -> Self {
        let radius = i128::from(radius);
        Self {
            left: self.left - radius,
            top: self.top - radius,
            right: self.right + radius,
            bottom: self.bottom + radius,
        }
    }
}

/// Returns whether two nonempty rectangles share positive area.
/// Empty or inverted rectangles cannot overlap; exact edge contact is excluded.
pub(super) fn overlaps(left: Rect, right: Rect) -> bool {
    if left.width <= 0 || left.height <= 0 || right.width <= 0 || right.height <= 0 {
        return false;
    }
    let left = Bounds::new(left);
    let right = Bounds::new(right);
    left.left.max(right.left) < left.right.min(right.right)
        && left.top.max(right.top) < left.bottom.min(right.bottom)
}

/// Returns inclusive containment, including zero-area inner rectangles.
/// Negative dimensions are invalid and return false.
pub(super) fn contains(outer: Rect, inner: Rect) -> bool {
    if outer.width < 0 || outer.height < 0 || inner.width < 0 || inner.height < 0 {
        return false;
    }
    let outer = Bounds::new(outer);
    let inner = Bounds::new(inner);
    outer.left <= inner.left
        && inner.right <= outer.right
        && outer.top <= inner.top
        && inner.bottom <= outer.bottom
}

/// Tests a centerline against a rectangle expanded by the stroke radius.
/// Positive-length contact on the expanded boundary counts; a single touching
/// point does not. Zero-length segments therefore return false. This models an
/// axis-aligned stroke envelope, not arrowheads or arbitrary stroke cap shapes.
pub(super) fn segment_hits_rect(
    start: Point,
    end: Point,
    rect: Rect,
    radius: i64,
) -> Result<bool, GeometryError> {
    if rect.width <= 0 || rect.height <= 0 {
        return Err(GeometryError::InvalidRect);
    }
    if radius < 0 {
        return Err(GeometryError::NegativeRadius);
    }
    let bounds = Bounds::new(rect).expand(radius);
    if start == end {
        return Ok(false);
    }
    if start.x == end.x {
        let x = i128::from(start.x);
        return Ok(bounds.left <= x
            && x <= bounds.right
            && i128::from(start.y.min(end.y)).max(bounds.top)
                < i128::from(start.y.max(end.y)).min(bounds.bottom));
    }
    if start.y == end.y {
        let y = i128::from(start.y);
        return Ok(bounds.top <= y
            && y <= bounds.bottom
            && i128::from(start.x.min(end.x)).max(bounds.left)
                < i128::from(start.x.max(end.x)).min(bounds.right));
    }
    Err(GeometryError::DiagonalSegment)
}

/// Allows only a real terminal's normal departure or arrival at the original
/// node boundary. Callers must enable these flags only on the first/last path
/// segment for that segment's actual source/target node. Every other segment
/// must be checked without the corresponding exemption, including later
/// segments that return to the same node.
pub(super) fn segment_hits_node(
    start: Point,
    end: Point,
    rect: Rect,
    radius: i64,
    allow_start: bool,
    allow_end: bool,
) -> Result<bool, GeometryError> {
    if !segment_hits_rect(start, end, rect, radius)? {
        return Ok(false);
    }
    let bounds = Bounds::new(rect);
    let allowed_departure = allow_start && departs_normally(start, end, bounds);
    let allowed_arrival = allow_end && departs_normally(end, start, bounds);
    Ok(!allowed_departure && !allowed_arrival)
}

fn departs_normally(terminal: Point, other: Point, rect: Bounds) -> bool {
    let x = i128::from(terminal.x);
    let y = i128::from(terminal.y);
    if terminal.y == other.y && rect.top <= y && y <= rect.bottom {
        return (x == rect.left && other.x < terminal.x)
            || (x == rect.right && terminal.x < other.x);
    }
    if terminal.x == other.x && rect.left <= x && x <= rect.right {
        return (y == rect.top && other.y < terminal.y)
            || (y == rect.bottom && terminal.y < other.y);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const NODE: Rect = rect(10, 20, 30, 40);

    const fn rect(x: i64, y: i64, width: i64, height: i64) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    const fn point(x: i64, y: i64) -> Point {
        Point { x, y }
    }

    fn assert_rect_hit(start: Point, end: Point, radius: i64, expected: bool) {
        assert_eq!(
            segment_hits_rect(start, end, NODE, radius),
            Ok(expected),
            "forward: {start:?} -> {end:?}, radius {radius}"
        );
        assert_eq!(
            segment_hits_rect(end, start, NODE, radius),
            Ok(expected),
            "reverse: {end:?} -> {start:?}, radius {radius}"
        );
    }

    #[test]
    fn rectangle_overlap_requires_positive_area() {
        for (other, expected) in [
            (NODE, true),
            (rect(11, 21, 1, 1), true),
            (rect(9, 19, 2, 2), true),
            (rect(0, 0, 50, 70), true),
            (rect(40, 20, 10, 40), false),
            (rect(10, 60, 30, 10), false),
            (rect(40, 60, 10, 10), false),
            (rect(41, 20, 10, 40), false),
            (rect(11, 21, 0, 1), false),
            (rect(11, 21, 1, 0), false),
            (rect(11, 21, -1, 1), false),
        ] {
            assert_eq!(overlaps(NODE, other), expected, "{other:?}");
            assert_eq!(overlaps(other, NODE), expected, "{other:?}");
        }
    }

    #[test]
    fn containment_includes_the_boundary_and_empty_inner_rectangles() {
        for inner in [NODE, rect(10, 20, 1, 1), rect(40, 60, 0, 0)] {
            assert!(contains(NODE, inner), "{inner:?}");
        }
        for inner in [
            rect(9, 20, 2, 1),
            rect(10, 19, 1, 2),
            rect(39, 20, 2, 1),
            rect(10, 59, 1, 2),
            rect(41, 60, 0, 0),
            rect(11, 21, -1, 1),
        ] {
            assert!(!contains(NODE, inner), "{inner:?}");
        }
        assert!(contains(rect(5, 5, 0, 0), rect(5, 5, 0, 0)));
        assert!(!contains(rect(10, 20, -1, 40), NODE));
    }

    #[test]
    fn crossing_and_travel_along_any_rectangle_edge_are_collisions() {
        for (start, end) in [
            (point(0, 40), point(50, 40)),
            (point(25, 0), point(25, 70)),
            (point(0, 20), point(50, 20)),
            (point(0, 60), point(50, 60)),
            (point(10, 0), point(10, 70)),
            (point(40, 0), point(40, 70)),
        ] {
            assert_rect_hit(start, end, 0, true);
        }
    }

    #[test]
    fn point_only_contact_is_excluded_but_one_unit_of_boundary_travel_is_not() {
        for (start, end) in [
            (point(0, 20), point(10, 20)),
            (point(0, 40), point(10, 40)),
            (point(40, 60), point(50, 60)),
            (point(10, 0), point(10, 20)),
            (point(40, 60), point(40, 70)),
        ] {
            assert_rect_hit(start, end, 0, false);
        }
        assert_rect_hit(point(0, 20), point(11, 20), 0, true);
        assert_rect_hit(point(10, 0), point(10, 21), 0, true);
        assert_rect_hit(point(0, 17), point(7, 17), 3, false);
        assert_rect_hit(point(0, 17), point(8, 17), 3, true);
    }

    #[test]
    fn stroke_expansion_detects_collisions_without_centerline_overlap() {
        assert_rect_hit(point(0, 18), point(50, 18), 0, false);
        assert_rect_hit(point(0, 18), point(50, 18), 3, true);
        assert_rect_hit(point(8, 0), point(8, 70), 0, false);
        assert_rect_hit(point(8, 0), point(8, 70), 3, true);
        assert_rect_hit(point(0, 17), point(50, 17), 3, true);
        assert_rect_hit(point(7, 0), point(7, 70), 3, true);
        assert_rect_hit(point(0, 16), point(50, 16), 3, false);
        assert_rect_hit(point(6, 0), point(6, 70), 3, false);
    }

    #[test]
    fn only_the_correct_terminal_flag_allows_normal_contact_on_all_sides() {
        for (terminal, outside) in [
            (point(10, 40), point(0, 40)),
            (point(40, 40), point(50, 40)),
            (point(25, 20), point(25, 10)),
            (point(25, 60), point(25, 70)),
        ] {
            assert_rect_hit(terminal, outside, 3, true);
            for (allow_start, allow_end, expected) in [
                (false, false, true),
                (true, false, false),
                (false, true, true),
                (true, true, false),
            ] {
                assert_eq!(
                    segment_hits_node(terminal, outside, NODE, 3, allow_start, allow_end),
                    Ok(expected),
                    "departure: {terminal:?} -> {outside:?}"
                );
                assert_eq!(
                    segment_hits_node(outside, terminal, NODE, 3, allow_end, allow_start),
                    Ok(expected),
                    "arrival: {outside:?} -> {terminal:?}"
                );
            }
        }
    }

    #[test]
    fn terminal_flags_never_exempt_travel_along_the_node_boundary() {
        for (start, end) in [
            (point(10, 20), point(40, 20)),
            (point(10, 60), point(40, 60)),
            (point(10, 20), point(10, 60)),
            (point(40, 20), point(40, 60)),
            (point(10, 40), point(10, 10)),
            (point(25, 20), point(50, 20)),
        ] {
            for radius in [0, 3] {
                assert_eq!(
                    segment_hits_node(start, end, NODE, radius, true, true),
                    Ok(true),
                    "{start:?} -> {end:?}, radius {radius}"
                );
                assert_eq!(
                    segment_hits_node(end, start, NODE, radius, true, true),
                    Ok(true)
                );
            }
        }
    }

    #[test]
    fn a_corner_can_depart_outward_but_cannot_run_into_its_adjacent_boundary() {
        for (terminal, outside) in [
            (point(10, 20), point(0, 20)),
            (point(10, 20), point(10, 10)),
            (point(40, 60), point(50, 60)),
            (point(40, 60), point(40, 70)),
        ] {
            assert_eq!(
                segment_hits_node(terminal, outside, NODE, 3, true, false),
                Ok(false)
            );
            assert_eq!(
                segment_hits_node(outside, terminal, NODE, 3, false, true),
                Ok(false)
            );
        }
        assert_eq!(
            segment_hits_node(point(10, 20), point(20, 20), NODE, 3, true, true),
            Ok(true)
        );
        assert_eq!(
            segment_hits_node(point(40, 60), point(40, 50), NODE, 3, true, true),
            Ok(true)
        );
    }

    #[test]
    fn an_allowed_endpoint_cannot_cross_through_its_own_node() {
        for (terminal, opposite_outside) in [
            (point(10, 40), point(50, 40)),
            (point(40, 40), point(0, 40)),
            (point(25, 20), point(25, 70)),
            (point(25, 60), point(25, 10)),
        ] {
            assert_eq!(
                segment_hits_node(terminal, opposite_outside, NODE, 3, true, true),
                Ok(true)
            );
            assert_eq!(
                segment_hits_node(opposite_outside, terminal, NODE, 3, true, true),
                Ok(true)
            );
        }
    }

    #[test]
    fn interior_endpoints_are_collisions_even_when_both_flags_are_enabled() {
        for (inside, other) in [
            (point(20, 40), point(0, 40)),
            (point(25, 30), point(25, 10)),
            (point(20, 40), point(30, 40)),
            (point(25, 30), point(25, 40)),
        ] {
            assert_eq!(
                segment_hits_node(inside, other, NODE, 3, true, true),
                Ok(true)
            );
            assert_eq!(
                segment_hits_node(other, inside, NODE, 3, true, true),
                Ok(true)
            );
        }
    }

    #[test]
    fn a_terminal_must_be_on_the_original_rectangle_not_just_its_stroke_halo() {
        for (near_boundary, outside) in [
            (point(8, 40), point(0, 40)),
            (point(42, 40), point(50, 40)),
            (point(25, 18), point(25, 10)),
            (point(25, 62), point(25, 70)),
        ] {
            assert_eq!(
                segment_hits_node(near_boundary, outside, NODE, 3, true, true),
                Ok(true)
            );
            assert_eq!(
                segment_hits_node(outside, near_boundary, NODE, 3, true, true),
                Ok(true)
            );
        }
    }

    #[test]
    fn a_clean_departure_does_not_exempt_later_reentry() {
        let path = [
            point(10, 40),
            point(0, 40),
            point(0, 10),
            point(25, 10),
            point(25, 40),
        ];
        let expected = [false, false, false, true];
        for (index, segment) in path.windows(2).enumerate() {
            assert_eq!(
                segment_hits_node(segment[0], segment[1], NODE, 3, index == 0, false),
                Ok(expected[index]),
                "segment {index}"
            );
        }
    }

    #[test]
    fn a_short_normal_departure_does_not_exempt_the_next_segment_in_the_halo() {
        assert_eq!(
            segment_hits_node(point(25, 20), point(25, 18), NODE, 3, true, false),
            Ok(false)
        );
        assert_eq!(
            segment_hits_node(point(25, 18), point(35, 18), NODE, 3, false, false),
            Ok(true)
        );
    }

    #[test]
    fn unsupported_geometry_returns_errors_instead_of_appearing_collision_free() {
        assert_eq!(
            segment_hits_rect(point(0, 0), point(50, 70), NODE, 3),
            Err(GeometryError::DiagonalSegment)
        );
        assert_eq!(
            segment_hits_node(point(10, 20), point(0, 0), NODE, 3, true, true),
            Err(GeometryError::DiagonalSegment)
        );
        assert_eq!(
            segment_hits_rect(point(0, 40), point(50, 40), NODE, -1),
            Err(GeometryError::NegativeRadius)
        );
        for obstacle in [
            rect(10, 20, 0, 40),
            rect(10, 20, 30, 0),
            rect(10, 20, -1, 40),
            rect(10, 20, 30, -1),
        ] {
            assert_eq!(
                segment_hits_rect(point(0, 40), point(50, 40), obstacle, 3),
                Err(GeometryError::InvalidRect)
            );
        }
    }

    #[test]
    fn zero_length_segments_have_no_positive_length_contact_but_validate_inputs() {
        for position in [point(25, 40), point(10, 20), point(0, 0)] {
            assert_rect_hit(position, position, 3, false);
            assert_eq!(
                segment_hits_node(position, position, NODE, 3, true, true),
                Ok(false)
            );
        }
        assert_eq!(
            segment_hits_rect(point(25, 40), point(25, 40), NODE, -1),
            Err(GeometryError::NegativeRadius)
        );
        assert_eq!(
            segment_hits_rect(point(25, 40), point(25, 40), rect(0, 0, 0, 0), 0),
            Err(GeometryError::InvalidRect)
        );
    }

    #[test]
    fn dimensions_and_stroke_expansion_do_not_overflow_i64_coordinates() {
        let high = rect(i64::MAX - 5, 0, 10, 10);
        let inner = rect(i64::MAX - 1, 1, 5, 5);
        assert!(contains(high, inner));
        assert!(overlaps(high, inner));
        assert_eq!(
            segment_hits_rect(point(i64::MAX, -10), point(i64::MAX, 20), high, 0),
            Ok(true)
        );
        let low = rect(i64::MIN + 2, -2, 4, 4);
        assert_eq!(
            segment_hits_rect(point(i64::MIN, -10), point(i64::MIN, 10), low, i64::MAX),
            Ok(true)
        );
        assert_eq!(
            segment_hits_node(
                point(i64::MIN + 2, 0),
                point(i64::MIN, 0),
                low,
                i64::MAX,
                true,
                false
            ),
            Ok(false)
        );
        assert_eq!(
            segment_hits_rect(point(i64::MIN, 5), point(i64::MAX, 5), high, i64::MAX),
            Ok(true)
        );
    }
}
