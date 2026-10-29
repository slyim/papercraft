//! Bounded native geometry shared by painting and semantic selection. Picking is
//! geometric, not an antialias/glyph/miter test. Released path data stays unchanged.
use crate::document::{Document, Node, NodeKind};
use eframe::egui::{self, Pos2, Rect, pos2, vec2};

pub(crate) fn rect(bounds: [f32; 4]) -> Rect {
    Rect::from_min_max(pos2(bounds[0], bounds[1]), pos2(bounds[2], bounds[3]))
}

/// Same fallback used by the released straight-path painter.
pub(crate) fn path_stroke(document: &Document, node: &Node) -> ([u8; 4], f32) {
    (
        document
            .node_stroke(node)
            .unwrap_or_else(|| document.node_fill(node)),
        if node.stroke_width > 0.0 {
            node.stroke_width
        } else {
            2.0
        },
    )
}

pub(crate) fn stroke_width(document: &Document, node: &Node) -> f32 {
    let (color, width) = if node.kind == NodeKind::Path {
        path_stroke(document, node)
    } else {
        (
            document.node_stroke(node).unwrap_or([0; 4]),
            node.stroke_width,
        )
    };
    if color[3] > 0 && node.kind != NodeKind::Text {
        width
    } else {
        0.0
    }
}

/// World envelope, including centered strokes. Text uses the authored layout box.
/// It is deliberately conservative at polygon joins and rounded corners.
pub(crate) fn bounds(document: &Document, node: &Node) -> [f32; 4] {
    let mut b = node.bounds();
    if node.kind == NodeKind::Path && !node.points.is_empty() {
        b = [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ];
        for &[x, y] in &node.points {
            let (x, y) = (node.x + x * node.width, node.y + y * node.height);
            b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
        }
    }
    if matches!(
        node.kind,
        NodeKind::Ellipse | NodeKind::Triangle | NodeKind::Path
    ) {
        let half = stroke_width(document, node) * 0.5;
        b = [b[0] - half, b[1] - half, b[2] + half, b[3] + half];
    }
    b
}

/// World contours. Ellipses retain the painter's bounded 64-segment approximation;
/// rounded rectangles use egui's bounded world-radius contour, avoiding u8 screen
/// radius truncation/capping at different zooms. Shaders on rectangles are rectangular.
pub(crate) fn contour(node: &Node) -> Vec<Pos2> {
    let r = rect(node.bounds());
    match node.kind {
        NodeKind::Path => {
            let mut points: Vec<_> = node
                .points
                .iter()
                .map(|&[x, y]| pos2(node.x + x * node.width, node.y + y * node.height))
                .collect();
            // Normalize the working contour only; released authored vertices persist.
            points.dedup();
            if node.closed && points.len() > 1 && points.first() == points.last() {
                points.pop();
            }
            points
        }
        NodeKind::Ellipse => (0..64)
            .map(|i| {
                let (s, c) = (i as f32 / 64.0 * std::f32::consts::TAU).sin_cos();
                r.center() + vec2(c * r.width() * 0.5, s * r.height() * 0.5)
            })
            .collect(),
        NodeKind::Triangle => vec![
            pos2(r.center().x, r.top()),
            r.right_bottom(),
            r.left_bottom(),
        ],
        _ => {
            let mut points = Vec::new();
            let radius = if node.shader.is_some() {
                0.0
            } else {
                node.radius.min(node.width.min(node.height) * 0.5)
            };
            egui::epaint::tessellator::path::rounded_rectangle(
                &mut points,
                r,
                egui::epaint::CornerRadiusF32::same(radius),
            );
            points
        }
    }
}

/// Consistent turns AND one total turn: rejects collinear, concave and star/self-
/// intersecting contours in O(n), without adding concave tessellation or rewriting data.
fn convex(points: &[Pos2]) -> bool {
    if points.len() < 3 {
        return false;
    }
    let mut sign = 0.0_f32;
    let mut turn = 0.0_f32;
    for i in 0..points.len() {
        let a = points[(i + 1) % points.len()] - points[i];
        let b = points[(i + 2) % points.len()] - points[(i + 1) % points.len()];
        if a.length_sq() == 0.0 || b.length_sq() == 0.0 {
            continue;
        }
        let cross = a.x * b.y - a.y * b.x;
        if cross != 0.0 {
            if sign != 0.0 && sign.signum() != cross.signum() {
                return false;
            }
            sign = cross;
        }
        turn += cross.atan2(a.dot(b));
    }
    sign != 0.0 && (turn.abs() - std::f32::consts::TAU).abs() < 0.01
}

fn segments(points: &[Pos2], closed: bool) -> impl Iterator<Item = (Pos2, Pos2)> + '_ {
    points.windows(2).map(|p| (p[0], p[1])).chain(
        closed
            .then(|| Some((*points.last()?, *points.first()?)))
            .flatten(),
    )
}

fn segment_intersects(a: Pos2, b: Pos2, area: Rect) -> bool {
    let d = b - a;
    let mut low = 0.0_f32;
    let mut high = 1.0_f32;
    for axis in 0..2 {
        if d[axis] == 0.0 {
            if a[axis] < area.min[axis] || a[axis] > area.max[axis] {
                return false;
            }
        } else {
            let x = (area.min[axis] - a[axis]) / d[axis];
            let y = (area.max[axis] - a[axis]) / d[axis];
            low = low.max(x.min(y));
            high = high.min(x.max(y));
            if low > high {
                return false;
            }
        }
    }
    true
}

fn point_segment_distance_sq(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let d = b - a;
    let t = if d.length_sq() > 0.0 {
        ((p - a).dot(d) / d.length_sq()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance_sq(a + d * t)
}
fn segment_near(a: Pos2, b: Pos2, area: Rect, radius: f32) -> bool {
    if segment_intersects(a, b, area) {
        return true;
    }
    let distance = a
        .distance_sq(area.clamp(a))
        .min(b.distance_sq(area.clamp(b)));
    distance <= radius * radius
        || area_corners(area)
            .into_iter()
            .any(|p| point_segment_distance_sq(p, a, b) <= radius * radius)
}
fn area_corners(r: Rect) -> [Pos2; 4] {
    [
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
    ]
}
fn contains_convex(points: &[Pos2], p: Pos2) -> bool {
    let mut positive = false;
    let mut negative = false;
    for (a, b) in segments(points, true) {
        let cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
        positive |= cross > 0.0;
        negative |= cross < 0.0;
        if positive && negative {
            return false;
        }
    }
    true
}
fn polygon_intersects(points: &[Pos2], area: Rect) -> bool {
    points.iter().any(|p| area.contains(*p))
        || area_corners(area)
            .into_iter()
            .any(|p| contains_convex(points, p))
        || segments(points, true).any(|(a, b)| segment_intersects(a, b, area))
}

pub(crate) fn intersects(document: &Document, node: &Node, area: Rect, tolerance: f32) -> bool {
    if !rect(bounds(document, node))
        .expand(tolerance)
        .intersects(area)
    {
        return false;
    }
    if node.kind == NodeKind::Text {
        return rect(node.bounds()).intersects(area);
    }
    let points = contour(node);
    let closed = node.kind != NodeKind::Path || node.closed;
    let convex = closed && (node.kind != NodeKind::Path || convex(&points));
    let fill = document.node_fill(node)[3] > 0
        || (matches!(
            node.kind,
            NodeKind::Rectangle | NodeKind::Frame | NodeKind::Ellipse
        ) && node.shader.is_some_and(|s| s.secondary[3] > 0));
    let inside = convex && polygon_intersects(&points, area);
    if fill && inside {
        return true;
    }
    let width = stroke_width(document, node);
    if width <= 0.0 {
        return false;
    }
    if matches!(node.kind, NodeKind::Rectangle | NodeKind::Frame) {
        // Inside strokes: no outside hit beyond the pointer tolerance.
        segments(&points, true).any(|(a, b)| {
            segment_near(
                a,
                b,
                area,
                if inside { width + tolerance } else { tolerance },
            )
        })
    } else {
        segments(&points, closed).any(|(a, b)| segment_near(a, b, area, width * 0.5 + tolerance))
    }
}
