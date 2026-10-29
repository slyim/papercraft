//! Canonical cubic geometry shared by Pen, persistence, picking, raster and SVG.
use crate::document::{MAX_COORDINATE, Node, NodeKind};
use eframe::egui::{self, Color32, Pos2, pos2};
use serde::{Deserialize, Serialize};

pub const MAX_ANCHORS: usize = 1024;
pub const MAX_FLAT_POINTS: usize = 32768;
pub const FLATNESS: f32 = 0.05;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointMode {
    #[default]
    Corner,
    Smooth,
    Symmetric,
}
impl PointMode {
    pub fn is_corner(&self) -> bool {
        *self == Self::Corner
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Anchor {
    pub position: [f32; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incoming: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outgoing: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "PointMode::is_corner")]
    pub mode: PointMode,
}
impl Anchor {
    pub fn corner(p: Pos2) -> Self {
        Self {
            position: [p.x, p.y],
            incoming: None,
            outgoing: None,
            mode: PointMode::Corner,
        }
    }
    pub fn pos(&self) -> Pos2 {
        pos2(self.position[0], self.position[1])
    }
    pub fn map(&self, f: impl Fn([f32; 2]) -> [f32; 2]) -> Self {
        Self {
            position: f(self.position),
            incoming: self.incoming.map(&f),
            outgoing: self.outgoing.map(f),
            mode: self.mode,
        }
    }
    pub fn translate(&mut self, delta: egui::Vec2) {
        *self = self.map(|p| [p[0] + delta.x, p[1] + delta.y]);
    }
    /// Handles are manipulated in world space; mode controls the opposite handle.
    pub fn set_handle(&mut self, incoming: bool, p: Pos2, independent: bool) {
        let center = self.pos();
        if independent {
            self.mode = PointMode::Corner;
        }
        let (own, other) = if incoming {
            (&mut self.incoming, &mut self.outgoing)
        } else {
            (&mut self.outgoing, &mut self.incoming)
        };
        let old_length = other.map_or(0.0, |v| center.distance(pos2(v[0], v[1])));
        *own = Some([p.x, p.y]);
        if self.mode != PointMode::Corner {
            let delta = p - center;
            let opposite = if self.mode == PointMode::Symmetric {
                center - delta
            } else {
                center - delta.normalized() * old_length
            };
            *other = Some([opposite.x, opposite.y]);
        }
    }
}

pub fn world_anchors(node: &Node) -> Vec<Anchor> {
    if node.anchors.is_empty() {
        node.points
            .iter()
            .map(|p| {
                Anchor::corner(pos2(
                    node.x + p[0] * node.width,
                    node.y + p[1] * node.height,
                ))
            })
            .collect()
    } else {
        node.anchors
            .iter()
            .map(|a| a.map(|p| [node.x + p[0] * node.width, node.y + p[1] * node.height]))
            .collect()
    }
}
fn controls(a: &Anchor, b: &Anchor) -> [Pos2; 4] {
    [
        a.pos(),
        a.outgoing.map_or(a.pos(), |p| pos2(p[0], p[1])),
        b.incoming.map_or(b.pos(), |p| pos2(p[0], p[1])),
        b.pos(),
    ]
}
pub fn cubic(p: [Pos2; 4], t: f32) -> Pos2 {
    let a = p[0].lerp(p[1], t);
    let b = p[1].lerp(p[2], t);
    let c = p[2].lerp(p[3], t);
    a.lerp(b, t).lerp(b.lerp(c, t), t)
}
fn split_controls(p: [Pos2; 4], t: f32) -> ([Pos2; 4], [Pos2; 4]) {
    let a = p[0].lerp(p[1], t);
    let b = p[1].lerp(p[2], t);
    let c = p[2].lerp(p[3], t);
    let d = a.lerp(b, t);
    let e = b.lerp(c, t);
    let m = d.lerp(e, t);
    ([p[0], a, d, m], [m, e, c, p[3]])
}
/// A de Casteljau split preserves all cubic geometry, including closure.
pub fn split_segment(
    anchors: &mut Vec<Anchor>,
    index: usize,
    t: f32,
    closed: bool,
) -> Result<usize, String> {
    if anchors.len() >= MAX_ANCHORS
        || index >= anchors.len()
        || (!closed && index + 1 == anchors.len())
        || !(t > 0.0 && t < 1.0)
    {
        return Err("Cannot split this path segment.".into());
    }
    let next = (index + 1) % anchors.len();
    let (l, r) = split_controls(controls(&anchors[index], &anchors[next]), t);
    let curved = anchors[index].outgoing.is_some() || anchors[next].incoming.is_some();
    let mut new = Anchor::corner(l[3]);
    if curved {
        anchors[index].outgoing = Some([l[1].x, l[1].y]);
        if anchors[index].mode == PointMode::Symmetric {
            anchors[index].mode = PointMode::Smooth;
        }
        anchors[next].incoming = Some([r[2].x, r[2].y]);
        if anchors[next].mode == PointMode::Symmetric {
            anchors[next].mode = PointMode::Smooth;
        }
        new.incoming = Some([l[2].x, l[2].y]);
        new.outgoing = Some([r[1].x, r[1].y]);
        new.mode = PointMode::Smooth;
    }
    anchors.insert(index + 1, new);
    Ok(index + 1)
}

/// Local nearest-parameter search for segment insertion; splitting itself is exact.
pub fn nearest_segment(anchors: &[Anchor], closed: bool, p: Pos2) -> Option<(usize, f32, f32)> {
    let mut best: Option<(usize, f32, f32)> = None;
    for i in 0..anchors.len().saturating_sub(usize::from(!closed)) {
        let controls = controls(&anchors[i], &anchors[(i + 1) % anchors.len()]);
        let mut t = 0.0;
        let mut distance = f32::INFINITY;
        for step in 0..=64 {
            let x = step as f32 / 64.0;
            let d = cubic(controls, x).distance_sq(p);
            if d < distance {
                distance = d;
                t = x;
            }
        }
        let (mut lo, mut hi) = ((t - 1.0 / 64.0).max(0.0), (t + 1.0 / 64.0).min(1.0));
        for _ in 0..16 {
            let a = lo + (hi - lo) / 3.0;
            let b = hi - (hi - lo) / 3.0;
            if cubic(controls, a).distance_sq(p) < cubic(controls, b).distance_sq(p) {
                hi = b;
            } else {
                lo = a;
            }
        }
        t = (lo + hi) * 0.5;
        let distance = cubic(controls, t).distance(p);
        if best.is_none_or(|b| distance < b.2) {
            best = Some((i, t, distance));
        }
    }
    best
}

pub fn world_bounds(anchors: &[Anchor], closed: bool) -> [f32; 4] {
    let mut b = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    let mut include = |p: Pos2| {
        b = [b[0].min(p.x), b[1].min(p.y), b[2].max(p.x), b[3].max(p.y)];
    };
    for a in anchors {
        include(a.pos());
    }
    let count = anchors.len().saturating_sub(usize::from(!closed));
    for i in 0..count {
        let p = controls(&anchors[i], &anchors[(i + 1) % anchors.len()]);
        for axis in 0..2 {
            // Derivative / 3 = a*t²+b*t+c. f64 avoids cancellation in wide curves.
            let v = p.map(|p| f64::from(p[axis]));
            let a = -v[0] + 3.0 * v[1] - 3.0 * v[2] + v[3];
            let b = 2.0 * (v[0] - 2.0 * v[1] + v[2]);
            let c = v[1] - v[0];
            let roots = if a.abs() < 1e-12 {
                [-c / b, f64::NAN]
            } else {
                let d = b * b - 4.0 * a * c;
                [(-b + d.sqrt()) / (2.0 * a), (-b - d.sqrt()) / (2.0 * a)]
            };
            for t in roots {
                if t > 0.0 && t < 1.0 {
                    include(cubic(p, t as f32));
                }
            }
        }
    }
    b
}
/// Bounds rebasing never changes the world positions of anchors/controls.
pub fn set_world_anchors(node: &mut Node, anchors: &[Anchor]) -> Result<(), String> {
    if !(2..=MAX_ANCHORS).contains(&anchors.len()) {
        return Err("Paths require 2–1,024 anchors.".into());
    }
    for a in anchors {
        for p in std::iter::once(a.position)
            .chain(a.incoming)
            .chain(a.outgoing)
        {
            if p.iter().any(|v| !v.is_finite() || v.abs() > MAX_COORDINATE) {
                return Err("Path controls must be finite and within world limits.".into());
            }
        }
    }
    let b = world_bounds(anchors, node.closed);
    let w = (b[2] - b[0]).max(1.0);
    let h = (b[3] - b[1]).max(1.0);
    if w > MAX_COORDINATE || h > MAX_COORDINATE {
        return Err("Path dimensions exceed world limits.".into());
    }
    node.x = b[0];
    node.y = b[1];
    node.width = w;
    node.height = h;
    let normalized: Vec<_> = anchors
        .iter()
        .map(|a| a.map(|p| [(p[0] - b[0]) / w, (p[1] - b[1]) / h]))
        .collect();
    if anchors
        .iter()
        .all(|a| a.incoming.is_none() && a.outgoing.is_none() && a.mode == PointMode::Corner)
    {
        node.points = normalized.iter().map(|a| a.position).collect();
        node.anchors.clear();
    } else {
        node.points.clear();
        node.anchors = normalized;
    }
    Ok(())
}
fn distance_line(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let d = b - a;
    if d.length_sq() == 0.0 {
        return p.distance(a);
    }
    // Distance to segment rather than infinite line catches collinear overshoot loops.
    p.distance(a + d * ((p - a).dot(d) / d.length_sq()).clamp(0.0, 1.0))
}
fn flatten_segment(p: [Pos2; 4], out: &mut Vec<Pos2>, depth: u8) -> Result<(), String> {
    if out.len() >= MAX_FLAT_POINTS {
        return Err("Path exceeds the flattened vertex budget.".into());
    }
    if distance_line(p[1], p[0], p[3]).max(distance_line(p[2], p[0], p[3])) <= FLATNESS {
        out.push(p[3]);
        return Ok(());
    }
    if depth >= 24 {
        return Err("Curve subdivision exceeded its precision budget.".into());
    }
    let (l, r) = split_controls(p, 0.5);
    flatten_segment(l, out, depth + 1)?;
    flatten_segment(r, out, depth + 1)
}
pub fn flatten(node: &Node) -> Result<Vec<Pos2>, String> {
    let a = world_anchors(node);
    if a.len() < 2 {
        return Ok(a.iter().map(Anchor::pos).collect());
    }
    let mut out = vec![a[0].pos()];
    for i in 0..a.len() - usize::from(!node.closed) {
        if out.len() >= MAX_FLAT_POINTS {
            return Err("Path exceeds the flattened vertex budget.".into());
        }
        let next = (i + 1) % a.len();
        if a[i].outgoing.is_none() && a[next].incoming.is_none() {
            out.push(a[next].pos());
        } else {
            flatten_segment(controls(&a[i], &a[next]), &mut out, 0)?;
        }
    }
    out.dedup();
    if node.closed && out.first() == out.last() {
        out.pop();
    }
    Ok(out)
}
pub fn validate(node: &Node) -> Result<(), String> {
    if node.anchors.is_empty() {
        return Ok(());
    }
    if node.kind != NodeKind::Path
        || !node.points.is_empty()
        || !(2..=MAX_ANCHORS).contains(&node.anchors.len())
    {
        return Err(
            "Only paths may contain 2–1,024 anchors, mutually exclusive with points.".into(),
        );
    }
    for a in world_anchors(node) {
        for p in std::iter::once(a.position)
            .chain(a.incoming)
            .chain(a.outgoing)
        {
            if p.iter().any(|v| !v.is_finite() || v.abs() > MAX_COORDINATE) {
                return Err("Path controls must be finite and within world limits.".into());
            }
        }
    }
    flatten(node)?;
    Ok(())
}

pub fn svg_d(node: &Node, relative: bool) -> String {
    let mut a = world_anchors(node);
    if relative {
        for a in &mut a {
            a.translate(-egui::vec2(node.x, node.y));
        }
    }
    let Some(first) = a.first() else {
        return String::new();
    };
    let mut d = format!("M {} {}", first.position[0], first.position[1]);
    for i in 0..a.len() - usize::from(!node.closed) {
        let b = &a[(i + 1) % a.len()];
        let p = controls(&a[i], b);
        if a[i].outgoing.is_some() || b.incoming.is_some() {
            d.push_str(&format!(
                " C {} {} {} {} {} {}",
                p[1].x, p[1].y, p[2].x, p[2].y, p[3].x, p[3].y
            ));
        } else {
            d.push_str(&format!(" L {} {}", p[3].x, p[3].y));
        }
    }
    if node.closed {
        d.push_str(" Z");
    }
    d
}

/// Bounded even-odd fill geometry. Lyon is MIT/Apache-2.0; no proprietary geometry.
pub fn fill_mesh(points: &[Pos2], color: Color32) -> Result<egui::Mesh, String> {
    use lyon_tessellation::geometry_builder::{
        FillGeometryBuilder, GeometryBuilder, GeometryBuilderError,
    };
    use lyon_tessellation::{FillOptions, FillRule, FillTessellator, FillVertex, VertexId};
    struct Output {
        mesh: egui::Mesh,
        overflow: bool,
        color: Color32,
    }
    impl GeometryBuilder for Output {
        fn add_triangle(&mut self, a: VertexId, b: VertexId, c: VertexId) {
            if self.mesh.indices.len() >= 196608 {
                self.overflow = true;
                return;
            }
            self.mesh.indices.extend([a.0, b.0, c.0]);
        }
        fn abort_geometry(&mut self) {
            self.mesh.clear();
        }
    }
    impl FillGeometryBuilder for Output {
        fn add_fill_vertex(&mut self, v: FillVertex) -> Result<VertexId, GeometryBuilderError> {
            if self.overflow || self.mesh.vertices.len() >= 65536 {
                return Err(GeometryBuilderError::TooManyVertices);
            }
            let id = VertexId(self.mesh.vertices.len() as u32);
            let p = v.position();
            self.mesh.colored_vertex(pos2(p.x, p.y), self.color);
            Ok(id)
        }
    }
    if points.len() > MAX_FLAT_POINTS {
        return Err("Fill exceeds path vertex budget.".into());
    }
    let mut builder = lyon_tessellation::path::Path::builder();
    if let Some(p) = points.first() {
        builder.begin(lyon_tessellation::math::point(p.x, p.y));
        for p in &points[1..] {
            builder.line_to(lyon_tessellation::math::point(p.x, p.y));
        }
        builder.close();
    }
    let path = builder.build();
    let mut output = Output {
        mesh: egui::Mesh::default(),
        overflow: false,
        color,
    };
    FillTessellator::new()
        .tessellate_path(
            &path,
            &FillOptions::default().with_fill_rule(FillRule::EvenOdd),
            &mut output,
        )
        .map_err(|e| format!("Path fill failed or exceeded its budget: {e:?}"))?;
    if output.overflow {
        return Err("Path fill exceeds triangle budget.".into());
    }
    Ok(output.mesh)
}
