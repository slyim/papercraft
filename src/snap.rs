//! Canvas gesture geometry in artboard units: resize handles, mapping layers from one
//! frame to another, and snapping to the edges and centers of layers and the artboard.

/// Axis-aligned bounds `[left, top, right, bottom]`.
pub type Frame = [f32; 4];

/// Screen distance within which a moving edge or center snaps to a target.
pub const SNAP_DISTANCE: f32 = 5.0;

/// Which edge of one axis a handle moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Start,
    Fixed,
    End,
}

/// A resize handle: the edges it moves on each axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handle {
    pub x: Side,
    pub y: Side,
}

impl Handle {
    /// Corners first, so they win where they overlap edge handles.
    pub const ALL: [Handle; 8] = [
        Handle::new(Side::End, Side::End),
        Handle::new(Side::Start, Side::Start),
        Handle::new(Side::End, Side::Start),
        Handle::new(Side::Start, Side::End),
        Handle::new(Side::Fixed, Side::Start),
        Handle::new(Side::Fixed, Side::End),
        Handle::new(Side::Start, Side::Fixed),
        Handle::new(Side::End, Side::Fixed),
    ];

    pub const fn new(x: Side, y: Side) -> Self {
        Self { x, y }
    }

    pub fn is_corner(self) -> bool {
        self.x != Side::Fixed && self.y != Side::Fixed
    }

    /// Position of the handle on `frame`.
    pub fn point(self, frame: Frame) -> [f32; 2] {
        let along = |side, start: f32, end: f32| match side {
            Side::Start => start,
            Side::Fixed => (start + end) / 2.0,
            Side::End => end,
        };
        [
            along(self.x, frame[0], frame[2]),
            along(self.y, frame[1], frame[3]),
        ]
    }
}

/// A line that moving geometry can snap to, with its extent along the other axis.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Line {
    value: f32,
    low: f32,
    high: f32,
}

/// A guide to draw: a vertical line at `x = value` or a horizontal one at `y = value`,
/// spanning `low..high` on the other axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guide {
    pub vertical: bool,
    pub value: f32,
    pub low: f32,
    pub high: f32,
}

/// Snap targets for one gesture, sorted so each lookup is logarithmic.
#[derive(Clone, Debug, Default)]
pub struct Snapper {
    x: Vec<Line>,
    y: Vec<Line>,
}

fn lines(frame: Frame, vertical: bool) -> [Line; 3] {
    let [left, top, right, bottom] = frame;
    let (start, end, low, high) = if vertical {
        (left, right, top, bottom)
    } else {
        (top, bottom, left, right)
    };
    [start, (start + end) / 2.0, end].map(|value| Line { value, low, high })
}

fn sorted(mut lines: Vec<Line>) -> Vec<Line> {
    lines.sort_by(|a, b| a.value.total_cmp(&b.value));
    lines
}

/// Nearest target value to `value` within `threshold`.
fn nearest(lines: &[Line], value: f32, threshold: f32) -> Option<f32> {
    let index = lines.partition_point(|l| l.value < value);
    [index.checked_sub(1), Some(index)]
        .into_iter()
        .flatten()
        .filter_map(|i| lines.get(i))
        .map(|l| l.value)
        .filter(|target| (target - value).abs() <= threshold)
        .min_by(|a, b| (a - value).abs().total_cmp(&(b - value).abs()))
}

/// Delta that puts one of `values` exactly on a target, choosing the target closest
/// to where `delta` would move it.
fn best(lines: &[Line], values: [f32; 3], delta: f32, threshold: f32) -> Option<f32> {
    values
        .into_iter()
        .filter_map(|v| {
            let target = nearest(lines, v + delta, threshold)?;
            Some((target - (v + delta), target - v))
        })
        .min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()))
        .map(|(_, snapped)| snapped)
}

impl Snapper {
    /// Targets are the edges and centers of `targets` and of the artboard.
    pub fn new(artboard: Frame, targets: impl IntoIterator<Item = Frame>) -> Self {
        let mut x = Vec::new();
        let mut y = Vec::new();
        for frame in std::iter::once(artboard).chain(targets) {
            x.extend(lines(frame, true));
            y.extend(lines(frame, false));
        }
        Self {
            x: sorted(x),
            y: sorted(y),
        }
    }

    /// Snapped replacement for `delta` when moving `frame`, per axis: the delta that
    /// puts an edge or the center exactly on a target within `threshold` of where
    /// `delta` would place it, or `None` when no target is that close.
    pub fn snap_move(&self, frame: Frame, delta: [f32; 2], threshold: f32) -> [Option<f32>; 2] {
        let [left, top, right, bottom] = frame;
        [
            best(
                &self.x,
                [left, (left + right) / 2.0, right],
                delta[0],
                threshold,
            ),
            best(
                &self.y,
                [top, (top + bottom) / 2.0, bottom],
                delta[1],
                threshold,
            ),
        ]
    }

    /// Target for one moving edge at `value`, on the x axis when `vertical`.
    fn snap_edge(&self, value: f32, vertical: bool, threshold: f32) -> Option<f32> {
        nearest(if vertical { &self.x } else { &self.y }, value, threshold)
    }

    /// Guides for every edge or center of `frame` that lies on a target.
    pub fn guides(&self, frame: Frame) -> Vec<Guide> {
        let mut guides = Vec::new();
        for vertical in [true, false] {
            let targets = if vertical { &self.x } else { &self.y };
            for line in lines(frame, vertical) {
                let start = targets.partition_point(|l| l.value < line.value - 0.01);
                let matched = targets[start..]
                    .iter()
                    .take_while(|l| l.value <= line.value + 0.01);
                let mut guide = None::<Guide>;
                for target in matched {
                    let g = guide.get_or_insert(Guide {
                        vertical,
                        value: line.value,
                        low: line.low,
                        high: line.high,
                    });
                    g.low = g.low.min(target.low);
                    g.high = g.high.max(target.high);
                }
                guides.extend(guide.filter(|g| !guides.contains(g)));
            }
        }
        guides
    }
}

/// Resizes `start` by dragging `handle` by `delta`. Moving edges advance by whole
/// units unless they snap to a target. Each dimension stays at least 1. With
/// `keep_aspect`, a corner handle keeps the starting proportions. Callers apply the
/// document's coordinate limits.
pub fn resize_frame(
    start: Frame,
    handle: Handle,
    delta: [f32; 2],
    keep_aspect: bool,
    snap: Option<(&Snapper, f32)>,
) -> Frame {
    let mut frame = start;
    for (axis, side) in [(0, handle.x), (1, handle.y)] {
        let (edge, opposite) = match side {
            Side::Fixed => continue,
            Side::Start => (axis, axis + 2),
            Side::End => (axis + 2, axis),
        };
        let raw = start[edge] + delta[axis];
        let mut value = start[edge] + delta[axis].round();
        if let Some(target) = snap.and_then(|(s, t)| s.snap_edge(raw, axis == 0, t)) {
            value = target;
        }
        frame[edge] = if side == Side::Start {
            value.min(start[opposite] - 1.0)
        } else {
            value.max(start[opposite] + 1.0)
        };
    }
    if keep_aspect && handle.is_corner() {
        let [w0, h0] = [start[2] - start[0], start[3] - start[1]];
        let [w, h] = [frame[2] - frame[0], frame[3] - frame[1]];
        let scale = if (w / w0 - 1.0).abs() >= (h / h0 - 1.0).abs() {
            w / w0
        } else {
            h / h0
        };
        let [w, h] = [(w0 * scale).round().max(1.0), (h0 * scale).round().max(1.0)];
        if handle.x == Side::Start {
            frame[0] = frame[2] - w;
        } else {
            frame[2] = frame[0] + w;
        }
        if handle.y == Side::Start {
            frame[1] = frame[3] - h;
        } else {
            frame[3] = frame[1] + h;
        }
    }
    frame
}

/// Maps `rect` from bounds `from` to bounds `to`, keeping its relative position and
/// size. A rect equal to `from` becomes exactly `to`; others round to whole units,
/// at least 1 wide and high.
pub fn map_rect(rect: Frame, from: Frame, to: Frame) -> Frame {
    if rect == from {
        return to;
    }
    let map = |value: f32, axis: usize| {
        let span = from[axis + 2] - from[axis];
        to[axis] + (value - from[axis]) * (to[axis + 2] - to[axis]) / span
    };
    let left = map(rect[0], 0).round();
    let top = map(rect[1], 1).round();
    let right = map(rect[2], 0).round().max(left + 1.0);
    let bottom = map(rect[3], 1).round().max(top + 1.0);
    [left, top, right, bottom]
}

/// Whether two frames overlap or touch.
pub fn intersects(a: Frame, b: Frame) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3]
}
