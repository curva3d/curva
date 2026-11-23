//! Laying the panels out in the window: along each edge in rows, then the floating ones.

use super::*;

/// Whether a dock runs along the top or bottom.
pub(super) fn across(dock: Dock) -> bool {
    matches!(dock, Dock::Top | Dock::Bottom)
}

impl Layout {
    /// Each panel's rectangle in a window of this size, and the area the showing panels leave
    /// free for the viewports. A hidden panel keeps the place it would have, to slide from, but
    /// leaves its space to the viewports.
    #[must_use]
    pub fn arrange(&self, window: Rect) -> (Vec<(PanelId, Rect)>, Rect) {
        let (all, _) = self.arrange_these(window, |_| true);
        let (shown, free) = self.arrange_these(window, |place| !place.hidden);
        let placed = self
            .places
            .iter()
            .filter_map(|place| {
                let from = if place.hidden { &all } else { &shown };
                from.iter().find(|(p, _)| *p == place.id()).copied()
            })
            .collect();
        (placed, free)
    }

    fn arrange_these(
        &self,
        window: Rect,
        counts: impl Fn(&Place) -> bool,
    ) -> (Vec<(PanelId, Rect)>, Rect) {
        let inner = window.shrink(GAP);
        let cap = end_cap(inner);
        let mut free = inner;
        let mut placed = Vec::new();
        // Top and bottom first, so side panels fit between them.
        for dock in [Dock::Top, Dock::Bottom, Dock::Left, Dock::Right] {
            let on_edge: Vec<&Place> = self
                .places
                .iter()
                .filter(|p| p.dock == dock && counts(p))
                .collect();
            // A side column makes room only for the top and bottom panels it overlaps, so a
            // panel elsewhere along an edge never changes it (owner, 22 November 2026: raising the
            // centred command bar shortened the Layers panel beside it).
            let edge_free = if across(dock) {
                free
            } else {
                column_room(&on_edge, &placed, free, inner, dock, cap)
            };
            for row in rows(&on_edge, edge_free, dock, cap) {
                let thickness = row
                    .iter()
                    .map(|(_, rect)| {
                        if across(dock) {
                            rect.height()
                        } else {
                            rect.width()
                        }
                    })
                    .fold(0.0, f32::max);
                for (panel, rect) in row {
                    let rect = match dock {
                        Dock::Top => rect.translate(vec2(0.0, free.min.y - rect.min.y)),
                        Dock::Bottom => rect.translate(vec2(0.0, free.max.y - rect.max.y)),
                        Dock::Left => rect.translate(vec2(free.min.x - rect.min.x, 0.0)),
                        _ => rect.translate(vec2(free.max.x - rect.max.x, 0.0)),
                    };
                    placed.push((panel, rect));
                }
                match dock {
                    Dock::Top => free.min.y = (free.min.y + thickness + GAP).min(free.max.y),
                    Dock::Bottom => free.max.y = (free.max.y - thickness - GAP).max(free.min.y),
                    Dock::Left => free.min.x = (free.min.x + thickness + GAP).min(free.max.x),
                    _ => free.max.x = (free.max.x - thickness - GAP).max(free.min.x),
                }
            }
        }
        for place in self.places.iter().filter(|p| counts(p)) {
            if let Dock::Floating { x, y } = place.dock {
                let size = Vec2::from(place.size).min(inner.size());
                let corner =
                    (window.min + vec2(x, y)).clamp(inner.min, (inner.max - size).max(inner.min));
                placed.push((place.id(), Rect::from_min_size(corner, size)));
            }
        }
        // Back in the order of `places`, so drawing and hit testing agree on what is on top.
        placed.sort_by_key(|(panel, _)| self.places.iter().position(|p| p.id() == *panel));
        (placed, free)
    }
}

/// The room for the column of side panels on `dock`: as wide as `free` across, and as tall as
/// the window between the top and bottom panels already `placed` that overlap the column, so a
/// top or bottom panel beside the column (the centred command bar) leaves it alone.
fn column_room(
    on_edge: &[&Place],
    placed: &[(PanelId, Rect)],
    free: Rect,
    inner: Rect,
    dock: Dock,
    cap: f32,
) -> Rect {
    let width = on_edge
        .iter()
        .map(|p| p.size[0].min(cap))
        .fold(0.0, f32::max)
        .min(free.width());
    let (left, right) = if dock == Dock::Left {
        (free.min.x, free.min.x + width)
    } else {
        (free.max.x - width, free.max.x)
    };
    let overlaps = |rect: &Rect| rect.max.x > left && rect.min.x < right;
    let (mut top, mut bottom) = (inner.min.y, inner.max.y);
    for (_, rect) in placed.iter().filter(|(_, rect)| overlaps(rect)) {
        if rect.center().y < inner.center().y {
            top = top.max(rect.max.y + GAP);
        } else {
            bottom = bottom.min(rect.min.y - GAP);
        }
    }
    Rect::from_min_max(pos2(free.min.x, top), pos2(free.max.x, bottom.max(top)))
}

/// A panel in a row: which, where it starts along the edge, its length and its thickness.
type RowItem = (PanelId, f32, f32, f32);

/// A panel's length along its edge and its thickness, as it asks for them but held within half
/// the room across and, at the ends of an edge or in a side column, within `cap`.
fn capped_size(place: &Place, free: Rect, dock: Dock, cap: f32) -> (f32, f32) {
    let (along_size, thickness, half) = if across(dock) {
        (place.size[0], place.size[1], free.height() / 2.0)
    } else {
        (place.size[1], place.size[0], free.width() / 2.0)
    };
    let thickness = thickness.min(half).max(0.0);
    if across(dock) {
        let at_end = matches!(place.align, Align::Start | Align::End);
        (
            if at_end {
                along_size.min(cap)
            } else {
                along_size
            },
            thickness,
        )
    } else {
        (along_size, thickness.min(cap))
    }
}

/// The panels on one edge, laid out in rows running along it (columns, for a side), each
/// row's rectangles placed along the edge but not yet against it. A panel that fills its edge
/// has a row to itself; shorter ones share a row while they fit side by side.
fn rows(on_edge: &[&Place], free: Rect, dock: Dock, cap: f32) -> Vec<Vec<(PanelId, Rect)>> {
    let length = if across(dock) {
        free.width()
    } else {
        free.height()
    };
    let start = if across(dock) { free.min.x } else { free.min.y };
    // A centred panel narrows to leave room for those at the ends of its row, as the command
    // bar does beside the agent panel in a small window.
    let ends = on_edge
        .iter()
        .filter(|p| matches!(p.align, Align::Start | Align::End))
        .map(|p| {
            if across(dock) {
                p.size[0].min(cap)
            } else {
                p.size[1]
            }
        })
        .fold(0.0, f32::max);
    // Each row: whether a filling panel has it, and its panels as their start, length along
    // the edge and thickness.
    let mut rows: Vec<(bool, Vec<RowItem>)> = Vec::new();
    for place in on_edge {
        let (along_size, thickness) = capped_size(place, free, dock, cap);
        let fills = place.align == Align::Fill || along_size <= 0.0;
        let along_size = if fills {
            length
        } else {
            along_size.min(length)
        };
        let along_size = if place.align == Align::Centre && ends > 0.0 {
            along_size.min((length - 2.0 * (ends + GAP)).max(MIN_PANEL.x))
        } else {
            along_size
        };
        let from = match place.align {
            Align::Fill | Align::Start => start,
            Align::Centre => start + (length - along_size) / 2.0,
            Align::End => start + length - along_size,
        };
        let item = (place.id(), from, along_size, thickness);
        let fits = |row: &[RowItem]| {
            row.iter()
                .all(|(_, f, s, _)| from + along_size + GAP <= *f || *f + *s + GAP <= from)
        };
        match rows.last_mut() {
            Some((false, row)) if !fills && fits(row) => row.push(item),
            _ => rows.push((fills, vec![item])),
        }
    }
    rows.into_iter()
        .map(|(_, row)| {
            row.into_iter()
                .map(|(panel, from, along_size, thickness)| {
                    let rect = if across(dock) {
                        Rect::from_min_size(pos2(from, free.min.y), vec2(along_size, thickness))
                    } else {
                        Rect::from_min_size(pos2(free.min.x, from), vec2(thickness, along_size))
                    };
                    (panel, rect)
                })
                .collect()
        })
        .collect()
}

/// The most a panel at either end of an edge may take, across or along: enough that a centred
/// panel of the smallest size still fits between two of them, so in a small window the side
/// columns and the corner panels narrow together instead of pushing the command bar into a row of
/// its own.
fn end_cap(inner: Rect) -> f32 {
    ((inner.width() - MIN_PANEL.x) / 2.0 - GAP).max(MIN_PANEL.x)
}
