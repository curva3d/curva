//! Where the interface's panels go (spec `docs/specs/ui-floating-panels.md`). A panel only says
//! what it holds; the layout decides where it sits and how big it is, so every panel can be
//! docked to any edge of the window or float, and users can arrange them as they like. The
//! arrangement is kept per user, beside the recovery files, not in the model.

use eframe::egui::{Pos2, Rect, Vec2, pos2, vec2};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Space between a panel and the window's edge, and between panels.
pub const GAP: f32 = 8.0;

/// The panels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PanelId {
    /// The command buttons, in tabs: Curves, Surfaces, Solids, Operations.
    CommandTabs,
    /// Layers and properties.
    Side,
    /// The command line, the drawing aids and the agent connection.
    CommandBar,
}

/// Where a panel is.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Dock {
    /// Stretched along the top edge.
    Top,
    /// Stretched along the bottom edge.
    Bottom,
    /// Stretched down the left edge, between the top and bottom panels.
    Left,
    /// Stretched down the right edge, between the top and bottom panels.
    Right,
    /// Free, with its top-left corner here.
    Floating {
        /// Distance from the window's left edge.
        x: f32,
        /// Distance from the window's top edge.
        y: f32,
    },
}

/// An edge of the window, which a panel hides towards in focus mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// The top.
    Top,
    /// The bottom.
    Bottom,
    /// The left.
    Left,
    /// The right.
    Right,
}

/// One panel's place.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Place {
    /// Which panel.
    pub panel: PanelId,
    /// Where it is.
    pub dock: Dock,
    /// Its size: the height of a top or bottom panel, the width of a side panel, both for a
    /// floating one. The other direction of a docked panel follows the window.
    pub size: [f32; 2],
}

/// The arrangement of every panel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// Each panel's place. Panels docked to the same edge stack inwards from it in this order:
    /// the first sits against the window's edge.
    pub places: Vec<Place>,
}

impl Default for Layout {
    /// The arrangement of the spec: command tabs across the top, layers and properties down
    /// the right, the command bar along the bottom.
    fn default() -> Self {
        Self {
            places: vec![
                Place {
                    panel: PanelId::CommandTabs,
                    dock: Dock::Top,
                    size: [0.0, 64.0],
                },
                Place {
                    panel: PanelId::CommandBar,
                    dock: Dock::Bottom,
                    size: [0.0, 132.0],
                },
                Place {
                    panel: PanelId::Side,
                    dock: Dock::Right,
                    size: [260.0, 0.0],
                },
            ],
        }
    }
}

impl Layout {
    /// Each panel's rectangle in a window of this size, and the area they leave free for the
    /// viewports. Top and bottom panels span the window's width; side panels fill the height
    /// between them; floating panels go where they were put, kept inside the window.
    #[must_use]
    pub fn arrange(&self, window: Rect) -> (Vec<(PanelId, Rect)>, Rect) {
        let inner = window.shrink(GAP);
        let mut free = inner;
        let mut placed = Vec::new();
        // Top and bottom first, so side panels fit between them.
        for place in &self.places {
            let height = place.size[1].min(free.height() / 2.0).max(0.0);
            let rect = match place.dock {
                Dock::Top => {
                    let rect = Rect::from_min_size(free.min, vec2(free.width(), height));
                    free.min.y = (rect.max.y + GAP).min(free.max.y);
                    rect
                }
                Dock::Bottom => {
                    let rect = Rect::from_min_max(pos2(free.min.x, free.max.y - height), free.max);
                    free.max.y = (rect.min.y - GAP).max(free.min.y);
                    rect
                }
                _ => continue,
            };
            placed.push((place.panel, rect));
        }
        for place in &self.places {
            let width = place.size[0].min(free.width() / 2.0).max(0.0);
            let rect = match place.dock {
                Dock::Left => {
                    let rect = Rect::from_min_size(free.min, vec2(width, free.height()));
                    free.min.x = (rect.max.x + GAP).min(free.max.x);
                    rect
                }
                Dock::Right => {
                    let rect = Rect::from_min_max(pos2(free.max.x - width, free.min.y), free.max);
                    free.max.x = (rect.min.x - GAP).max(free.min.x);
                    rect
                }
                Dock::Floating { x, y } => {
                    let size = Vec2::from(place.size).min(inner.size());
                    let corner = (window.min + vec2(x, y))
                        .clamp(inner.min, (inner.max - size).max(inner.min));
                    Rect::from_min_size(corner, size)
                }
                Dock::Top | Dock::Bottom => continue,
            };
            placed.push((place.panel, rect));
        }
        // Back in the order of `places`, so drawing and hit testing agree on what is on top.
        placed.sort_by_key(|(panel, _)| self.places.iter().position(|p| p.panel == *panel));
        (placed, free)
    }

    /// The edge a panel hides towards in focus mode: the one it is docked to, or for a floating
    /// panel the window edge nearest its centre.
    #[must_use]
    pub fn edge_of(&self, panel: PanelId, rect: Rect, window: Rect) -> Edge {
        match self
            .places
            .iter()
            .find(|p| p.panel == panel)
            .map(|p| p.dock)
        {
            Some(Dock::Top) => Edge::Top,
            Some(Dock::Bottom) => Edge::Bottom,
            Some(Dock::Left) => Edge::Left,
            Some(Dock::Right) | None => Edge::Right,
            Some(Dock::Floating { .. }) => nearest_edge(rect.center(), window),
        }
    }

    /// The arrangement saved in `file`, or the standard one if there is none or it cannot be read.
    #[must_use]
    pub fn load(file: &Path) -> Self {
        std::fs::read_to_string(file)
            .ok()
            .and_then(|text| serde_json::from_str::<Self>(&text).ok())
            .filter(Self::is_complete)
            .unwrap_or_default()
    }

    /// Saves the arrangement to `file`. Losing it only loses the arrangement, so a failure is
    /// returned for the caller to ignore or report.
    pub fn save(&self, file: &Path) -> std::io::Result<()> {
        if let Some(folder) = file.parent() {
            std::fs::create_dir_all(folder)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(file, text)
    }

    /// Whether every panel has exactly one place, so a file from an older or newer Curva with
    /// a different set of panels falls back to the standard arrangement.
    fn is_complete(&self) -> bool {
        let panels = [PanelId::CommandTabs, PanelId::Side, PanelId::CommandBar];
        self.places.len() == panels.len()
            && panels
                .iter()
                .all(|panel| self.places.iter().filter(|p| p.panel == *panel).count() == 1)
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

/// How much of a hidden panel stays in view in focus mode, so it can be found again.
pub const STRIP: f32 = 6.0;
/// How much of a hidden panel shows while the pointer is over its strip.
pub const PEEK: f32 = 28.0;

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

/// Where the arrangement is kept: in Curva's own folder of the user's app data.
#[must_use]
pub fn file() -> Option<PathBuf> {
    Some(crate::recovery::app_folder()?.join("layout.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(width: f32, height: f32) -> Rect {
        Rect::from_min_size(Pos2::ZERO, vec2(width, height))
    }

    fn rect_of(placed: &[(PanelId, Rect)], panel: PanelId) -> Rect {
        placed.iter().find(|(p, _)| *p == panel).unwrap().1
    }

    #[test]
    fn the_standard_arrangement_has_tabs_on_top_the_side_panel_right_and_the_bar_below() {
        for (width, height) in [(1280.0, 800.0), (1920.0, 1080.0), (800.0, 600.0)] {
            let screen = window(width, height);
            let (placed, free) = Layout::default().arrange(screen);
            let tabs = rect_of(&placed, PanelId::CommandTabs);
            let side = rect_of(&placed, PanelId::Side);
            let bar = rect_of(&placed, PanelId::CommandBar);
            assert_eq!(tabs.min, pos2(GAP, GAP));
            assert_eq!(tabs.width(), width - 2.0 * GAP);
            assert_eq!(bar.max, pos2(width - GAP, height - GAP));
            assert_eq!(bar.width(), width - 2.0 * GAP);
            // The side panel fits between the two, against the right edge.
            assert_eq!(side.max.x, width - GAP);
            assert_eq!(side.min.y, tabs.max.y + GAP);
            assert_eq!(side.max.y, bar.min.y - GAP);
            // The viewports get the rest, and no panel overlaps them or another panel.
            for (_, rect) in &placed {
                assert!(!rect.intersects(free.shrink(0.5)), "{rect:?} over {free:?}");
                assert!(screen.contains_rect(*rect));
            }
            assert!(!tabs.intersects(side) && !side.intersects(bar) && !tabs.intersects(bar));
            assert!(free.width() > 0.0 && free.height() > 0.0);
        }
    }

    #[test]
    fn panels_can_dock_to_any_edge_or_float_inside_the_window() {
        let screen = window(1200.0, 800.0);
        let mut layout = Layout::default();
        layout.places[2].dock = Dock::Left;
        let (placed, free) = layout.arrange(screen);
        assert_eq!(rect_of(&placed, PanelId::Side).min.x, GAP);
        assert!(free.min.x > rect_of(&placed, PanelId::Side).max.x);
        assert_eq!(
            layout.edge_of(PanelId::Side, rect_of(&placed, PanelId::Side), screen),
            Edge::Left
        );
        // Floating, it is kept inside the window even if put outside it, and takes no space
        // from the viewports.
        layout.places[2].dock = Dock::Floating {
            x: 5000.0,
            y: 300.0,
        };
        layout.places[2].size = [260.0, 400.0];
        let (placed, free) = layout.arrange(screen);
        let side = rect_of(&placed, PanelId::Side);
        assert!(screen.contains_rect(side));
        assert_eq!(side.size(), vec2(260.0, 400.0));
        assert_eq!(free.max.x, 1200.0 - GAP);
        assert_eq!(layout.edge_of(PanelId::Side, side, screen), Edge::Right);
        // Docked at the bottom too, the command tabs come first in the list, so they sit
        // against the edge, below the command bar.
        let mut layout = Layout::default();
        layout.places[0].dock = Dock::Bottom;
        let (placed, _) = layout.arrange(screen);
        assert!(
            rect_of(&placed, PanelId::CommandTabs).min.y
                > rect_of(&placed, PanelId::CommandBar).max.y
        );
    }

    #[test]
    fn hidden_panels_leave_a_strip_at_their_edge_and_peek_further_out() {
        let screen = window(1200.0, 800.0);
        let layout = Layout::default();
        let (placed, _) = layout.arrange(screen);
        for (panel, rect) in placed {
            let edge = layout.edge_of(panel, rect, screen);
            let hidden = slid(rect, edge, screen, STRIP);
            let peeking = slid(rect, edge, screen, PEEK);
            let inside = |r: Rect| r.intersect(screen);
            // Only the strip, then the peek, is left in the window, along the panel's own edge.
            let visible = |r: Rect| match edge {
                Edge::Top | Edge::Bottom => inside(r).height(),
                Edge::Left | Edge::Right => inside(r).width(),
            };
            assert!(
                (visible(hidden) - STRIP).abs() < 1e-3,
                "{panel:?} {hidden:?}"
            );
            assert!(
                (visible(peeking) - PEEK).abs() < 1e-3,
                "{panel:?} {peeking:?}"
            );
            // It slides straight out: the other direction does not move.
            match edge {
                Edge::Top | Edge::Bottom => assert_eq!(hidden.min.x, rect.min.x),
                Edge::Left | Edge::Right => assert_eq!(hidden.min.y, rect.min.y),
            }
        }
        let (placed, _) = layout.arrange(screen);
        let edges: Vec<Edge> = placed
            .iter()
            .map(|(p, r)| layout.edge_of(*p, *r, screen))
            .collect();
        assert_eq!(edges, [Edge::Top, Edge::Bottom, Edge::Right]);
    }

    #[test]
    fn the_nearest_edge_is_found_for_floating_panels() {
        let screen = window(1000.0, 600.0);
        assert_eq!(nearest_edge(pos2(500.0, 20.0), screen), Edge::Top);
        assert_eq!(nearest_edge(pos2(500.0, 590.0), screen), Edge::Bottom);
        assert_eq!(nearest_edge(pos2(10.0, 300.0), screen), Edge::Left);
        assert_eq!(nearest_edge(pos2(990.0, 300.0), screen), Edge::Right);
    }

    #[test]
    fn an_arrangement_saved_and_read_back_is_the_same_and_a_bad_file_gives_the_standard_one() {
        let folder = std::env::temp_dir().join(format!("curva-layout-{}", std::process::id()));
        let file = folder.join("layout.json");
        let mut layout = Layout::default();
        layout.places[2].dock = Dock::Floating { x: 40.0, y: 90.0 };
        layout.save(&file).unwrap();
        assert_eq!(Layout::load(&file), layout);
        std::fs::write(&file, "{ not a layout").unwrap();
        assert_eq!(Layout::load(&file), Layout::default());
        assert_eq!(
            Layout::load(&folder.join("missing.json")),
            Layout::default()
        );
        let _ = std::fs::remove_dir_all(&folder);
    }
}
