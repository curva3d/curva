//! Where a dragged panel docks or settles, and where a hidden one slides to and leaves its
//! tab.

use super::*;

/// Where a floating panel dropped at `rect` settles, as on a desktop: its corner on the
/// 24-point grid and its size rounded to it, lined up with the window's edges and other
/// panels' edges and centres within 12 points, and moved to the nearest free spot if it would
/// overlap another panel.
#[must_use]
pub fn settle(rect: Rect, window: Rect, others: &[Rect]) -> Rect {
    let inner = window.shrink(GAP);
    let round = |v: f32| (v / GAP).round() * GAP;
    let size = vec2(
        round(rect.width()).max(MIN_PANEL.x),
        round(rect.height()).max(MIN_PANEL.y),
    )
    .min(inner.size());
    let corner = pos2(
        inner.min.x + round(rect.min.x - inner.min.x),
        inner.min.y + round(rect.min.y - inner.min.y),
    );
    let mut settled = Rect::from_min_size(corner, size);
    // Line up with edges and centres nearby, keeping the gap between neighbours.
    let mut lines_x = vec![inner.min.x, inner.max.x, inner.center().x];
    let mut lines_y = vec![inner.min.y, inner.max.y, inner.center().y];
    for other in others {
        lines_x.extend([
            other.min.x,
            other.max.x,
            other.min.x - GAP,
            other.max.x + GAP,
        ]);
        lines_y.extend([
            other.min.y,
            other.max.y,
            other.min.y - GAP,
            other.max.y + GAP,
        ]);
        lines_x.push(other.center().x);
        lines_y.push(other.center().y);
    }
    let shift = |edges: [f32; 3], lines: &[f32]| {
        let mut best: Option<f32> = None;
        for edge in edges {
            for line in lines {
                let by = line - edge;
                if by.abs() <= ALIGN_REACH && best.is_none_or(|b| by.abs() < b.abs()) {
                    best = Some(by);
                }
            }
        }
        best.unwrap_or(0.0)
    };
    settled = settled.translate(vec2(
        shift([settled.min.x, settled.max.x, settled.center().x], &lines_x),
        shift([settled.min.y, settled.max.y, settled.center().y], &lines_y),
    ));
    settled = keep_inside(settled, inner);
    let clear = |r: Rect| {
        others
            .iter()
            .all(|o| !r.intersects(o.expand(GAP / 2.0 - 0.5)))
    };
    if clear(settled) {
        return settled;
    }
    // The nearest free spot on the grid, searching outwards ring by ring.
    for ring in 1..=60 {
        let ring = ring as f32;
        let mut best: Option<(f32, Rect)> = None;
        for i in -60..=60 {
            for j in -60..=60 {
                let (i, j) = (i as f32, j as f32);
                if i.abs().max(j.abs()) != ring {
                    continue;
                }
                let candidate = keep_inside(settled.translate(vec2(i * GAP, j * GAP)), inner);
                let distance = (candidate.min - settled.min).length();
                if clear(candidate) && best.is_none_or(|(d, _)| distance < d) {
                    best = Some((distance, candidate));
                }
            }
        }
        if let Some((_, spot)) = best {
            return spot;
        }
    }
    settled
}

fn keep_inside(rect: Rect, inner: Rect) -> Rect {
    let corner = rect
        .min
        .clamp(inner.min, (inner.max - rect.size()).max(inner.min));
    Rect::from_min_size(corner, rect.size())
}

/// How near a window edge a dragged panel's pointer must be to dock there.
pub const DOCK_REACH: f32 = 56.0;
/// The smallest a panel can be made.
pub const MIN_PANEL: Vec2 = vec2(160.0, 60.0);

/// The edge a panel dragged to `pointer` would dock to, if the pointer is near enough to one.
#[must_use]
pub fn drop_edge(pointer: Pos2, window: Rect) -> Option<Edge> {
    let edge = nearest_edge(pointer, window);
    let distance = match edge {
        Edge::Top => pointer.y - window.min.y,
        Edge::Bottom => window.max.y - pointer.y,
        Edge::Left => pointer.x - window.min.x,
        Edge::Right => window.max.x - pointer.x,
    };
    (distance <= DOCK_REACH).then_some(edge)
}

/// Where a panel goes when dropped with its corner at `corner` and the pointer at `pointer`:
/// docked to a near edge, or floating where it was let go.
#[must_use]
pub fn drop_dock(pointer: Pos2, corner: Pos2, window: Rect) -> Dock {
    match drop_edge(pointer, window) {
        Some(Edge::Top) => Dock::Top,
        Some(Edge::Bottom) => Dock::Bottom,
        Some(Edge::Left) => Dock::Left,
        Some(Edge::Right) => Dock::Right,
        None => Dock::Floating {
            x: corner.x - window.min.x,
            y: corner.y - window.min.y,
        },
    }
}

/// The window edge nearest a point.
#[must_use]
pub fn nearest_edge(point: Pos2, window: Rect) -> Edge {
    let distances = [
        (point.y - window.min.y, Edge::Top),
        (window.max.y - point.y, Edge::Bottom),
        (point.x - window.min.x, Edge::Left),
        (window.max.x - point.x, Edge::Right),
    ];
    distances
        .into_iter()
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map_or(Edge::Right, |(_, edge)| edge)
}

/// How much of a hidden panel stays in view, so it can be found again: its edge tab.
pub const STRIP: f32 = 22.0;
/// How much of a hidden panel shows while the pointer is over its tab.
pub const PEEK: f32 = 48.0;
/// The length of a hidden panel's edge tab along its edge, at most.
pub const TAB_LENGTH: f32 = 140.0;

/// Where a panel is when slid towards `edge` until only `visible` points of it remain inside
/// the window. A panel already further out than that stays where it is.
#[must_use]
pub fn slid(rect: Rect, edge: Edge, window: Rect, visible: f32) -> Rect {
    let shift = match edge {
        Edge::Top => vec2(0.0, -(rect.max.y - window.min.y - visible).max(0.0)),
        Edge::Bottom => vec2(0.0, (window.max.y - rect.min.y - visible).max(0.0)),
        Edge::Left => vec2(-(rect.max.x - window.min.x - visible).max(0.0), 0.0),
        Edge::Right => vec2((window.max.x - rect.min.x - visible).max(0.0), 0.0),
    };
    rect.translate(shift)
}

/// The tab a hidden panel leaves at `edge`: `STRIP` deep, centred on the panel along the edge,
/// and whether its name runs up the tab (along a side) rather than across.
#[must_use]
pub fn edge_tab(rect: Rect, edge: Edge, window: Rect) -> (Rect, bool) {
    let upright = matches!(edge, Edge::Left | Edge::Right);
    let length = if upright { rect.height() } else { rect.width() }.min(TAB_LENGTH);
    let tab = match edge {
        Edge::Top => Rect::from_center_size(
            pos2(rect.center().x, window.min.y + STRIP / 2.0),
            vec2(length, STRIP),
        ),
        Edge::Bottom => Rect::from_center_size(
            pos2(rect.center().x, window.max.y - STRIP / 2.0),
            vec2(length, STRIP),
        ),
        Edge::Left => Rect::from_center_size(
            pos2(window.min.x + STRIP / 2.0, rect.center().y),
            vec2(STRIP, length),
        ),
        Edge::Right => Rect::from_center_size(
            pos2(window.max.x - STRIP / 2.0, rect.center().y),
            vec2(STRIP, length),
        ),
    };
    (tab, upright)
}
