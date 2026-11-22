//! Where the interface's pieces go (specs `docs/specs/ui-floating-panels.md` and
//! `docs/specs/panel-system.md`). The interface is made of pieces: the command tabs, Layers,
//! Properties, the command line and the agent connection. Each lives in a panel, alone or with
//! others as tabs. A piece only says what it holds; the layout decides which panel it is in,
//! where that panel sits, how big it is and whether it shows, so every panel can be docked to
//! any edge or float on a soft grid, merged with others, split and hidden. The arrangement is
//! kept per user, beside the recovery files, not in the model.

use eframe::egui::{Pos2, Rect, Vec2, pos2, vec2};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

mod arrange;
use arrange::across;
mod drop;
pub use drop::*;

/// Space between a panel and the window's edge, and between panels: the grid's step.
pub const GAP: f32 = 24.0;
/// How near a floating panel's edge must be to another edge to line up with it.
pub const ALIGN_REACH: f32 = 12.0;

/// The pieces of the interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Piece {
    /// The command buttons, in tabs: File, Curves, Surfaces, Solids, Operations.
    CommandTabs,
    /// The layers.
    Layers,
    /// The selection's properties.
    Properties,
    /// The command line and the drawing aids.
    Command,
    /// The agent connection.
    Agent,
    /// The active viewport's display mode: Wireframe, Shaded, Ghosted.
    Display,
    /// The settings, in tabs.
    Settings,
    /// The block definitions (`BlockManager`).
    Blocks,
    /// The annotation styles (`AnnotationStyle`).
    AnnotationStyles,
    /// The layouts (`Layouts`).
    Layouts,
    /// Printing (`Print`).
    Print,
    /// The model's notes (`Notes`).
    Notes,
}

impl Piece {
    /// Every piece, in their standard order.
    pub const ALL: [Self; 12] = [
        Self::CommandTabs,
        Self::Layers,
        Self::Properties,
        Self::Command,
        Self::Agent,
        Self::Display,
        Self::Settings,
        Self::Blocks,
        Self::Layouts,
        Self::AnnotationStyles,
        Self::Print,
        Self::Notes,
    ];

    /// Its name's key in the translation file.
    #[must_use]
    pub fn name_key(self) -> &'static str {
        match self {
            Self::CommandTabs => "ui.piece.operations",
            Self::Layers => "ui.tab.layers",
            Self::Properties => "ui.tab.properties",
            Self::Command => "ui.piece.command",
            Self::Agent => "ui.piece.assistant",
            Self::Display => "ui.piece.display",
            Self::Settings => "ui.file.settings",
            Self::Blocks => "ui.piece.blocks",
            Self::Layouts => "ui.piece.layouts",
            Self::AnnotationStyles => "ui.piece.annotationstyles",
            Self::Print => "ui.piece.print",
            Self::Notes => "ui.piece.notes",
        }
    }
}

/// A panel is known by the first piece it holds.
pub type PanelId = Piece;

/// Where a panel is.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Dock {
    /// Along the top edge.
    Top,
    /// Along the bottom edge.
    Bottom,
    /// Down the left edge, between the top and bottom panels.
    Left,
    /// Down the right edge, between the top and bottom panels.
    Right,
    /// Free, with its top-left corner here.
    Floating {
        /// Distance from the window's left edge.
        x: f32,
        /// Distance from the window's top edge.
        y: f32,
    },
}

/// How a docked panel shorter than its edge sits along it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Align {
    /// Stretched the whole length.
    #[default]
    Fill,
    /// At the start: the left of a top or bottom edge, the top of a side.
    Start,
    /// In the middle.
    Centre,
    /// At the end: the right of a top or bottom edge, the bottom of a side.
    End,
}

/// An edge of the window, which a panel hides towards.
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

/// One panel: the pieces it holds and its place.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Place {
    /// The pieces it holds, as tabs in this order; never empty.
    pub pieces: Vec<Piece>,
    /// Which of them shows.
    #[serde(default)]
    pub active: usize,
    /// Where it is.
    pub dock: Dock,
    /// How a docked panel shorter than its edge sits along it.
    #[serde(default)]
    pub align: Align,
    /// Its size. A top or bottom panel's height, and its width unless it fills its edge; a side
    /// panel's width, and its height unless it fills; both for a floating one.
    pub size: [f32; 2],
    /// Whether it is hidden, slid to its edge leaving a tab with its name.
    #[serde(default)]
    pub hidden: bool,
}

impl Place {
    /// Whether the panel holds the command tabs and nothing else, so their groups take the title
    /// row's place rather than a title.
    #[must_use]
    pub fn is_command_tabs_alone(&self) -> bool {
        self.pieces.as_slice() == [Piece::CommandTabs]
    }

    /// The panel's name: its first piece.
    #[must_use]
    pub fn id(&self) -> PanelId {
        self.pieces[0]
    }

    /// The piece showing.
    #[must_use]
    pub fn showing(&self) -> Piece {
        self.pieces[self.active.min(self.pieces.len() - 1)]
    }

    fn new(pieces: Vec<Piece>, dock: Dock, align: Align, size: [f32; 2]) -> Self {
        Self {
            pieces,
            active: 0,
            dock,
            align,
            size,
            hidden: false,
        }
    }
}

/// The arrangement of every panel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// The panels. Panels docked to the same edge stack inwards from it in this order: the
    /// first sits against the window's edge. Panels shorter than their edge share a row when
    /// they fit side by side.
    pub places: Vec<Place>,
    /// Pieces closed with their cross, to be brought back from Settings.
    #[serde(default)]
    pub closed: Vec<Piece>,
    /// The Layers panel's column widths, in points: Name, On, Lock, Colour, Linetype and Print
    /// width (spec `feedback-0.4.2.md` §6).
    #[serde(default = "standard_layer_columns")]
    pub layer_columns: [f32; 6],
}

/// The Layers panel's columns as they start, and after Reset layout.
pub const STANDARD_LAYER_COLUMNS: [f32; 6] = [100.0, 26.0, 30.0, 36.0, 84.0, 72.0];

/// The narrowest each of the Layers panel's columns can be made.
pub const LEAST_LAYER_COLUMNS: [f32; 6] = [60.0, 24.0, 28.0, 32.0, 60.0, 56.0];

/// The command tabs' height at first: the groups, their tabs and one row of buttons (owner,
/// 22 November 2026: the bar had a title row and room to spare).
pub const COMMAND_TABS_HEIGHT: f32 = 92.0;

fn standard_layer_columns() -> [f32; 6] {
    STANDARD_LAYER_COLUMNS
}

impl Default for Layout {
    /// The standard arrangement: command tabs across the top, Layers and Properties down the
    /// right, a compact command bar centred at the bottom and the agent connection in the
    /// bottom-right corner.
    fn default() -> Self {
        Self {
            places: vec![
                Place::new(
                    vec![Piece::CommandTabs],
                    Dock::Top,
                    Align::Fill,
                    [0.0, COMMAND_TABS_HEIGHT],
                ),
                Place::new(
                    vec![Piece::Command],
                    Dock::Bottom,
                    Align::Centre,
                    [COMMAND_WIDTH, 132.0],
                ),
                Place::new(
                    vec![Piece::Agent],
                    Dock::Bottom,
                    Align::End,
                    [SIDE_WIDTH, 96.0],
                ),
                Place::new(
                    vec![Piece::Layers, Piece::Properties],
                    Dock::Right,
                    Align::Fill,
                    [SIDE_WIDTH, 0.0],
                ),
                // The display mode button, below the top-left panel.
                Place::new(
                    vec![Piece::Display],
                    Dock::Floating {
                        x: GAP,
                        y: GAP + COMMAND_TABS_HEIGHT + GAP,
                    },
                    Align::Fill,
                    [DISPLAY_WIDTH, 64.0],
                ),
            ],
            closed: vec![
                Piece::Settings,
                Piece::Blocks,
                Piece::Layouts,
                Piece::AnnotationStyles,
                Piece::Print,
                Piece::Notes,
            ],
            layer_columns: STANDARD_LAYER_COLUMNS,
        }
    }
}

/// The size a panel brought back without a standard place opens at, as Settings does.
pub const FLOATING_STANDARD: Vec2 = vec2(560.0, 460.0);

/// The widest the command bar is in the standard arrangement.
pub const COMMAND_WIDTH: f32 = 720.0;

/// The width of the Layers and Properties column in the standard layout, which the agent panel
/// below it shares so their edges line up (spec `feedback-0.4.2.md` §5).
pub const SIDE_WIDTH: f32 = 260.0;

/// The narrowest the Display panel opens at: its two halves, the display mode and the view,
/// each wide enough for the longest name, "View: Perspective" and its arrow (#470, #471).
pub const DISPLAY_WIDTH: f32 = 290.0;

impl Layout {
    /// The place of a panel.
    #[must_use]
    pub fn place(&self, panel: PanelId) -> Option<&Place> {
        self.places.iter().find(|p| p.id() == panel)
    }

    /// The panel holding a piece.
    #[must_use]
    pub fn panel_of(&self, piece: Piece) -> Option<PanelId> {
        self.places
            .iter()
            .find(|p| p.pieces.contains(&piece))
            .map(Place::id)
    }

    /// The edge a panel hides towards: the one it is docked to, or for a floating panel the
    /// window edge nearest its centre.
    #[must_use]
    pub fn edge_of(&self, panel: PanelId, rect: Rect, window: Rect) -> Edge {
        match self.place(panel).map(|p| p.dock) {
            Some(Dock::Top) => Edge::Top,
            Some(Dock::Bottom) => Edge::Bottom,
            Some(Dock::Left) => Edge::Left,
            Some(Dock::Right) | None => Edge::Right,
            Some(Dock::Floating { .. }) => nearest_edge(rect.center(), window),
        }
    }

    /// The arrangement saved in `file`, or the standard one if there is none or it cannot be
    /// read. A file from before panels held pieces is read and brought up to date.
    #[must_use]
    pub fn load(file: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(file) else {
            return Self::default();
        };
        serde_json::from_str::<Self>(&text)
            .ok()
            .or_else(|| {
                serde_json::from_str::<older::Layout>(&text)
                    .ok()
                    .map(older::Layout::updated)
            })
            .map(Self::completed)
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

    /// The arrangement with `panel` moved to `dock`. Docked, it goes innermost on that edge,
    /// keeping its thickness if it was already docked the same way and taking a usable one
    /// otherwise; floating, it takes `floating_size`.
    #[must_use]
    pub fn moved(&self, panel: PanelId, dock: Dock, floating_size: Vec2) -> Self {
        let Some(old) = self.place(panel).cloned() else {
            return self.clone();
        };
        let down = |d: Dock| matches!(d, Dock::Left | Dock::Right);
        let standard = Self::default()
            .places
            .into_iter()
            .find(|p| p.pieces.contains(&panel));
        let (size, align) = match dock {
            Dock::Top | Dock::Bottom if across(old.dock) => (old.size, old.align),
            Dock::Left | Dock::Right if down(old.dock) => (old.size, old.align),
            // Back to its standard edge, it takes its standard size again.
            _ if standard.as_ref().is_some_and(|s| s.dock == dock) => {
                let standard = standard.unwrap_or_else(|| old.clone());
                (standard.size, standard.align)
            }
            // A side panel laid across the top or bottom keeps a sensible height, and a bar
            // stood up keeps a sensible width.
            Dock::Top | Dock::Bottom => ([0.0, old.size[1].clamp(64.0, 200.0)], Align::Fill),
            Dock::Left | Dock::Right => ([old.size[0].max(220.0), 0.0], Align::Fill),
            Dock::Floating { .. } => ([floating_size.x, floating_size.y], Align::Fill),
        };
        let mut places: Vec<Place> = self
            .places
            .iter()
            .filter(|p| p.id() != panel)
            .cloned()
            .collect();
        places.push(Place {
            dock,
            size,
            align,
            hidden: false,
            ..old
        });
        Self {
            places,
            closed: self.closed.clone(),
            layer_columns: self.layer_columns,
        }
    }

    /// The arrangement with `panel` resized, never smaller than a usable panel. For a docked
    /// panel only the directions it does not stretch in are kept.
    #[must_use]
    pub fn resized(&self, panel: PanelId, size: Vec2) -> Self {
        let mut layout = self.clone();
        if let Some(place) = layout.places.iter_mut().find(|p| p.id() == panel) {
            let size = [size.x.max(MIN_PANEL.x), size.y.max(MIN_PANEL.y)];
            match (place.dock, place.align) {
                (Dock::Top | Dock::Bottom, Align::Fill) => place.size[1] = size[1],
                (Dock::Left | Dock::Right, Align::Fill) => place.size[0] = size[0],
                _ => place.size = size,
            }
        }
        layout
    }

    /// The arrangement with `panel` resized by dragging `sides` of it by `by`: a floating panel
    /// from any edge or corner, a docked one only from its inner edge, and a top or bottom panel
    /// shorter than its edge also from its ends. Never smaller than a usable panel.
    #[must_use]
    pub fn resized_from(&self, panel: PanelId, sides: Sides, by: Vec2, window: Rect) -> Self {
        let Some(place) = self.place(panel) else {
            return self.clone();
        };
        let Some((_, rect)) = self
            .arrange(window)
            .0
            .into_iter()
            .find(|(p, _)| *p == panel)
        else {
            return self.clone();
        };
        let allowed = sides.within(Sides::movable(place));
        let mut moved = rect;
        if allowed.left {
            moved.min.x = (rect.min.x + by.x).min(rect.max.x - MIN_PANEL.x);
        }
        if allowed.right {
            moved.max.x = (rect.max.x + by.x).max(rect.min.x + MIN_PANEL.x);
        }
        if allowed.top {
            moved.min.y = (rect.min.y + by.y).min(rect.max.y - MIN_PANEL.y);
        }
        if allowed.bottom {
            moved.max.y = (rect.max.y + by.y).max(rect.min.y + MIN_PANEL.y);
        }
        let mut layout = self.resized(panel, moved.size());
        if let Some(place) = layout.places.iter_mut().find(|p| p.id() == panel)
            && let Dock::Floating { .. } = place.dock
        {
            place.dock = Dock::Floating {
                x: moved.min.x - window.min.x,
                y: moved.min.y - window.min.y,
            };
        }
        layout
    }

    /// The arrangement with the panel `dragged` merged into `onto`: its pieces become tabs at
    /// the end of `onto`'s, and the one that was showing shows.
    #[must_use]
    pub fn merged(&self, dragged: PanelId, onto: PanelId) -> Self {
        if dragged == onto {
            return self.clone();
        }
        let Some(moving) = self.place(dragged).cloned() else {
            return self.clone();
        };
        let mut layout = self.clone();
        layout.places.retain(|p| p.id() != dragged);
        if let Some(target) = layout.places.iter_mut().find(|p| p.id() == onto) {
            target.active = target.pieces.len() + moving.active;
            target.pieces.extend(moving.pieces);
            target.hidden = false;
            layout
        } else {
            self.clone()
        }
    }

    /// The arrangement with `piece` pulled out of its panel into one of its own at `dock`,
    /// leaving the rest together. A panel holding only that piece is simply moved.
    #[must_use]
    pub fn pulled(&self, piece: Piece, dock: Dock, floating_size: Vec2) -> Self {
        let Some(from) = self.places.iter().find(|p| p.pieces.contains(&piece)) else {
            return self.clone();
        };
        if from.pieces.len() == 1 {
            return self.moved(piece, dock, floating_size);
        }
        let mut layout = self.clone();
        if let Some(from) = layout.places.iter_mut().find(|p| p.pieces.contains(&piece)) {
            let showing = from.showing();
            from.pieces.retain(|p| *p != piece);
            from.active = from.pieces.iter().position(|p| *p == showing).unwrap_or(0);
        }
        layout.places.push(Place::new(
            vec![piece],
            Dock::Floating { x: 0.0, y: 0.0 },
            Align::Fill,
            [floating_size.x, floating_size.y],
        ));
        layout.moved(piece, dock, floating_size)
    }

    /// The arrangement with tab `index` of `panel` showing.
    #[must_use]
    pub fn with_active(&self, panel: PanelId, index: usize) -> Self {
        let mut layout = self.clone();
        if let Some(place) = layout.places.iter_mut().find(|p| p.id() == panel) {
            place.active = index.min(place.pieces.len() - 1);
        }
        layout
    }

    /// The arrangement with `panel` hidden or shown.
    #[must_use]
    pub fn with_hidden(&self, panel: PanelId, hidden: bool) -> Self {
        let mut layout = self.clone();
        if let Some(place) = layout.places.iter_mut().find(|p| p.id() == panel) {
            place.hidden = hidden;
        }
        layout
    }

    /// Whether every piece is in exactly one panel, and no panel is empty, so a file from an
    /// older or newer Curva with a different set of pieces falls back to the standard
    /// arrangement.
    fn is_complete(&self) -> bool {
        let all = || {
            self.places
                .iter()
                .flat_map(|p| p.pieces.iter())
                .chain(&self.closed)
        };
        self.places.iter().all(|p| !p.pieces.is_empty())
            && Piece::ALL
                .iter()
                .all(|piece| all().filter(|p| *p == piece).count() == 1)
            && all().count() == Piece::ALL.len()
    }

    /// The arrangement with any piece it lacks (a piece newer than the file) given its standard
    /// place, or closed if it starts closed, as Settings does.
    #[must_use]
    pub fn completed(mut self) -> Self {
        let standard = Self::default();
        for piece in Piece::ALL {
            let has = self.places.iter().any(|p| p.pieces.contains(&piece))
                || self.closed.contains(&piece);
            if has {
                continue;
            }
            match standard.places.iter().find(|p| p.pieces.contains(&piece)) {
                Some(place) => self.places.push(Place {
                    pieces: vec![piece],
                    active: 0,
                    ..place.clone()
                }),
                None => self.closed.push(piece),
            }
        }
        // A Display panel saved narrower than its names need opens wide enough for them (#470).
        for place in &mut self.places {
            if place.pieces == [Piece::Display] {
                place.size[0] = place.size[0].max(DISPLAY_WIDTH);
            }
        }
        self
    }

    /// The arrangement with `panel` closed: its pieces go to the closed list, all of them.
    #[must_use]
    pub fn closing(&self, panel: PanelId) -> Self {
        let mut layout = self.clone();
        if let Some(index) = layout.places.iter().position(|p| p.id() == panel) {
            let place = layout.places.remove(index);
            layout.closed.extend(place.pieces);
        }
        layout
    }

    /// The arrangement with a closed `piece` brought back: to its standard place if it has one,
    /// otherwise floating in the centre of `window`, as Settings opens.
    #[must_use]
    pub fn restored(&self, piece: Piece, window: Rect) -> Self {
        if !self.closed.contains(&piece) {
            return self.clone();
        }
        let mut layout = self.clone();
        layout.closed.retain(|p| *p != piece);
        let standard = Self::default();
        let place = standard
            .places
            .iter()
            .find(|p| p.pieces.contains(&piece))
            .map_or_else(
                || {
                    let size = FLOATING_STANDARD;
                    let centred = window.center() - size / 2.0 - window.min.to_vec2();
                    let corner = self.beside_floating(centred, size, window);
                    Place::new(
                        vec![piece],
                        Dock::Floating {
                            x: corner.x,
                            y: corner.y,
                        },
                        Align::Fill,
                        [size.x, size.y],
                    )
                },
                |place| Place {
                    pieces: vec![piece],
                    active: 0,
                    ..place.clone()
                },
            );
        layout.places.push(place);
        layout
    }

    /// Where a new floating panel of `size` goes: at `corner` (from the window's top left) if no
    /// floating panel is there, otherwise beside the one it would cover, to its right if that fits
    /// in `window`, else to its left (#482: a panel opened from Settings landed on top of it).
    fn beside_floating(&self, corner: Pos2, size: Vec2, window: Rect) -> Pos2 {
        const GAP: f32 = 12.0;
        let wanted = Rect::from_min_size(corner, size);
        let covered = self.places.iter().find_map(|place| match place.dock {
            Dock::Floating { x, y } => {
                let rect = Rect::from_min_size(pos2(x, y), vec2(place.size[0], place.size[1]));
                rect.intersects(wanted).then_some(rect)
            }
            _ => None,
        });
        let Some(other) = covered else {
            return corner;
        };
        let right = other.max.x + GAP;
        let left = other.min.x - GAP - size.x;
        if right + size.x <= window.width() {
            pos2(right, corner.y)
        } else if left >= 0.0 {
            pos2(left, corner.y)
        } else if window.width() - other.max.x >= other.min.x {
            // No room either side: against the window's edge with more room, overlapping as
            // little as it can.
            pos2((window.width() - size.x).max(0.0), corner.y)
        } else {
            pos2(0.0, corner.y)
        }
    }

    /// The arrangement with tab `from` of `panel` moved to position `to`.
    #[must_use]
    pub fn reordered(&self, panel: PanelId, from: usize, to: usize) -> Self {
        let mut layout = self.clone();
        if let Some(place) = layout.places.iter_mut().find(|p| p.id() == panel)
            && from < place.pieces.len()
        {
            let showing = place.showing();
            let piece = place.pieces.remove(from);
            place.pieces.insert(to.min(place.pieces.len()), piece);
            place.active = place.pieces.iter().position(|p| *p == showing).unwrap_or(0);
        }
        layout
    }

    /// Whether a piece is shown somewhere, rather than closed.
    #[must_use]
    pub fn is_shown(&self, piece: Piece) -> bool {
        !self.closed.contains(&piece)
    }
}

/// Which sides of a panel are being dragged to resize it; two for a corner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // One per side is what this is.
pub struct Sides {
    /// The left edge.
    pub left: bool,
    /// The right edge.
    pub right: bool,
    /// The top edge.
    pub top: bool,
    /// The bottom edge.
    pub bottom: bool,
}

impl Sides {
    /// The sides of `place` that can be dragged: every one for a floating panel; for a docked
    /// one its inner edge, and the ends of a top or bottom panel shorter than its edge.
    #[must_use]
    pub fn movable(place: &Place) -> Self {
        let ends = place.align != Align::Fill;
        match place.dock {
            Dock::Floating { .. } => Self {
                left: true,
                right: true,
                top: true,
                bottom: true,
            },
            Dock::Top => Self {
                bottom: true,
                left: ends,
                right: ends,
                top: false,
            },
            Dock::Bottom => Self {
                top: true,
                left: ends,
                right: ends,
                bottom: false,
            },
            Dock::Left => Self {
                right: true,
                top: ends,
                bottom: ends,
                left: false,
            },
            Dock::Right => Self {
                left: true,
                top: ends,
                bottom: ends,
                right: false,
            },
        }
    }

    /// Only those of these sides that `allowed` has.
    #[must_use]
    pub fn within(self, allowed: Self) -> Self {
        Self {
            left: self.left && allowed.left,
            right: self.right && allowed.right,
            top: self.top && allowed.top,
            bottom: self.bottom && allowed.bottom,
        }
    }
}

/// Where the arrangement is kept: in Curva's own folder of the user's app data.
#[must_use]
pub fn file() -> Option<PathBuf> {
    Some(crate::recovery::app_folder()?.join("layout.json"))
}

/// Layout files from before panels held pieces, read and brought up to date.
mod older;

#[cfg(test)]
mod tests {
    use super::*;

    fn window(width: f32, height: f32) -> Rect {
        Rect::from_min_size(Pos2::ZERO, vec2(width, height))
    }

    fn rect_of(placed: &[(PanelId, Rect)], panel: PanelId) -> Rect {
        placed.iter().find(|(p, _)| *p == panel).unwrap().1
    }

    fn index_of(layout: &Layout, panel: PanelId) -> usize {
        layout.places.iter().position(|p| p.id() == panel).unwrap()
    }

    #[test]
    fn the_standard_arrangement_has_tabs_on_top_the_side_panel_right_and_the_bar_below() {
        for (width, height) in [(1280.0, 800.0), (1920.0, 1080.0), (800.0, 600.0)] {
            let screen = window(width, height);
            let (placed, free) = Layout::default().arrange(screen);
            let tabs = rect_of(&placed, Piece::CommandTabs);
            let side = rect_of(&placed, Piece::Layers);
            let bar = rect_of(&placed, Piece::Command);
            let agent = rect_of(&placed, Piece::Agent);
            assert_eq!(tabs.min, pos2(GAP, GAP));
            assert_eq!(tabs.width(), width - 2.0 * GAP);
            // The command bar is centred at the bottom, at most 720 points wide; the agent
            // connection sits in the bottom-right corner.
            assert_eq!(bar.max.y, height - GAP);
            assert!(bar.width() <= COMMAND_WIDTH);
            assert!((bar.center().x - width / 2.0).abs() < 0.5);
            assert_eq!(agent.max, pos2(width - GAP, height - GAP));
            // The agent panel lines up with the column above it.
            assert_eq!(agent.min.x, side.min.x);
            assert_eq!(agent.max.x, side.max.x);
            // The side panel fits between them, against the right edge, 24 points from each.
            assert_eq!(side.max.x, width - GAP);
            assert_eq!(side.min.y, tabs.max.y + GAP);
            assert_eq!(side.max.y, bar.min.y.min(agent.min.y) - GAP);
            // The viewports get the rest, and no panel overlaps them or another panel.
            // The Display panel floats over the viewports, by design; the docked ones do not.
            for (_, rect) in placed.iter().filter(|(p, _)| *p != Piece::Display) {
                assert!(!rect.intersects(free.shrink(0.5)), "{rect:?} over {free:?}");
                assert!(screen.contains_rect(*rect));
            }
            for (i, (_, a)) in placed.iter().enumerate() {
                for (_, b) in &placed[i + 1..] {
                    assert!(!a.intersects(*b), "{a:?} and {b:?}");
                }
            }
            assert!(free.width() > 0.0 && free.height() > 0.0);
        }
    }

    #[test]
    fn panels_can_dock_to_any_edge_or_float_inside_the_window() {
        let screen = window(1200.0, 800.0);
        let mut layout = Layout::default();
        let side = index_of(&layout, Piece::Layers);
        layout.places[side].dock = Dock::Left;
        let (placed, free) = layout.arrange(screen);
        assert_eq!(rect_of(&placed, Piece::Layers).min.x, GAP);
        assert!(free.min.x > rect_of(&placed, Piece::Layers).max.x);
        assert_eq!(
            layout.edge_of(Piece::Layers, rect_of(&placed, Piece::Layers), screen),
            Edge::Left
        );
        // Floating, it is kept inside the window even if put outside it, and takes no space
        // from the viewports.
        layout.places[side].dock = Dock::Floating {
            x: 5000.0,
            y: 300.0,
        };
        layout.places[side].size = [260.0, 400.0];
        let (placed, free) = layout.arrange(screen);
        let floating = rect_of(&placed, Piece::Layers);
        assert!(screen.contains_rect(floating));
        assert_eq!(floating.size(), vec2(260.0, 400.0));
        assert_eq!(free.max.x, 1200.0 - GAP);
        assert_eq!(layout.edge_of(Piece::Layers, floating, screen), Edge::Right);
        // Docked at the bottom too, the command tabs come first in the list, so they sit
        // against the edge, below the command bar.
        let mut layout = Layout::default();
        layout.places[0].dock = Dock::Bottom;
        let (placed, _) = layout.arrange(screen);
        assert!(
            rect_of(&placed, Piece::CommandTabs).min.y > rect_of(&placed, Piece::Command).max.y
        );
    }

    #[test]
    fn hidden_panels_leave_a_strip_at_their_edge_and_peek_further_out() {
        let screen = window(1200.0, 800.0);
        let layout = Layout::default();
        let (placed, _) = layout.arrange(screen);
        for (panel, rect) in &placed {
            let edge = layout.edge_of(*panel, *rect, screen);
            let hidden = slid(*rect, edge, screen, STRIP);
            let peeking = slid(*rect, edge, screen, PEEK);
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
            // Its tab lies along that edge, with the name along the edge's direction.
            let (tab, upright) = edge_tab(*rect, edge, screen);
            assert!(screen.contains_rect(tab));
            assert_eq!(upright, matches!(edge, Edge::Left | Edge::Right));
            let depth = if upright { tab.width() } else { tab.height() };
            assert!((depth - STRIP).abs() < 1e-3);
        }
        let edges: Vec<Edge> = placed
            .iter()
            .map(|(p, r)| layout.edge_of(*p, *r, screen))
            .collect();
        assert_eq!(
            edges,
            [
                Edge::Top,
                Edge::Bottom,
                Edge::Bottom,
                Edge::Right,
                Edge::Left
            ]
        );
        // A hidden panel leaves its space to the viewports but keeps its place to slide from.
        let hiding = layout.with_hidden(Piece::Layers, true);
        let (placed_hidden, free) = hiding.arrange(screen);
        assert_eq!(free.max.x, 1200.0 - GAP);
        assert_eq!(
            rect_of(&placed_hidden, Piece::Layers),
            rect_of(&placed, Piece::Layers)
        );
    }

    #[test]
    fn a_dragged_panel_docks_near_an_edge_and_floats_elsewhere() {
        let screen = window(1200.0, 800.0);
        assert_eq!(
            drop_dock(pos2(600.0, 20.0), pos2(0.0, 0.0), screen),
            Dock::Top
        );
        assert_eq!(
            drop_dock(pos2(30.0, 400.0), pos2(0.0, 0.0), screen),
            Dock::Left
        );
        assert_eq!(
            drop_dock(pos2(1190.0, 400.0), pos2(0.0, 0.0), screen),
            Dock::Right
        );
        assert_eq!(
            drop_dock(pos2(600.0, 790.0), pos2(0.0, 0.0), screen),
            Dock::Bottom
        );
        assert_eq!(
            drop_dock(pos2(600.0, 400.0), pos2(500.0, 380.0), screen),
            Dock::Floating { x: 500.0, y: 380.0 }
        );
    }

    #[test]
    fn moving_a_panel_keeps_every_panel_placed_and_gives_it_a_usable_size() {
        let screen = window(1200.0, 800.0);
        let layout = Layout::default();
        // The command bar stood up on the left gets a width; the side panel laid along the top
        // gets a height.
        let moved = layout.moved(Piece::Command, Dock::Left, Vec2::ZERO).moved(
            Piece::Layers,
            Dock::Top,
            Vec2::ZERO,
        );
        assert!(moved.is_complete());
        let (placed, free) = moved.arrange(screen);
        for (panel, rect) in &placed {
            assert!(
                rect.width() >= 100.0 && rect.height() >= 50.0,
                "{panel:?} {rect:?}"
            );
            assert!(!rect.intersects(free.shrink(0.5)));
        }
        // Docked to an edge that already has a panel, it goes inside it.
        let tabs = rect_of(&placed, Piece::CommandTabs);
        let side = rect_of(&placed, Piece::Layers);
        assert!(side.min.y > tabs.max.y);
        // Floating, it takes the size it had while dragged, and resizing never goes too small.
        let floating = layout.moved(
            Piece::Layers,
            Dock::Floating { x: 100.0, y: 100.0 },
            vec2(300.0, 420.0),
        );
        let (placed, _) = floating.arrange(screen);
        assert_eq!(rect_of(&placed, Piece::Layers).size(), vec2(300.0, 420.0));
        let tiny = floating.resized(Piece::Layers, vec2(10.0, 10.0));
        let (placed, _) = tiny.arrange(screen);
        assert_eq!(rect_of(&placed, Piece::Layers).size(), MIN_PANEL);
        // Moving it back to the right gives back its standard width.
        let back = floating.moved(Piece::Layers, Dock::Right, Vec2::ZERO);
        let rect = |l: &Layout| rect_of(&l.arrange(screen).0, Piece::Layers);
        assert_eq!(rect(&back), rect(&layout));
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
        let mut layout = Layout::default().merged(Piece::Agent, Piece::Layers);
        layout.places[0].dock = Dock::Floating { x: 40.0, y: 90.0 };
        layout.save(&file).unwrap();
        assert_eq!(Layout::load(&file), layout);
        std::fs::write(&file, "{ not a layout").unwrap();
        assert_eq!(Layout::load(&file), Layout::default());
        assert_eq!(
            Layout::load(&folder.join("missing.json")),
            Layout::default()
        );
        // A file from before panels held pieces is read, keeping where its panels were.
        std::fs::write(
            &file,
            r#"{"places":[{"panel":"CommandTabs","dock":"Top","size":[0.0,64.0]},
                {"panel":"CommandBar","dock":"Bottom","size":[0.0,132.0]},
                {"panel":"Side","dock":"Left","size":[300.0,0.0]}]}"#,
        )
        .unwrap();
        let older = Layout::load(&file);
        assert!(older.is_complete());
        assert_eq!(older.place(Piece::Layers).unwrap().dock, Dock::Left);
        assert_eq!(
            older.place(Piece::Layers).unwrap().pieces,
            [Piece::Layers, Piece::Properties]
        );
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn merging_adds_tabs_and_pulling_a_tab_out_makes_a_panel_of_it() {
        let layout = Layout::default();
        let merged = layout.merged(Piece::Command, Piece::Layers);
        let side = merged.place(Piece::Layers).unwrap();
        assert_eq!(
            side.pieces,
            [Piece::Layers, Piece::Properties, Piece::Command]
        );
        assert_eq!(side.showing(), Piece::Command);
        assert!(merged.is_complete());
        // Pulling Properties out leaves Layers and Command together.
        let pulled = merged.pulled(
            Piece::Properties,
            Dock::Floating { x: 200.0, y: 200.0 },
            vec2(240.0, 300.0),
        );
        assert!(pulled.is_complete());
        assert_eq!(
            pulled.place(Piece::Layers).unwrap().pieces,
            [Piece::Layers, Piece::Command]
        );
        assert_eq!(
            pulled.place(Piece::Properties).unwrap().pieces,
            [Piece::Properties]
        );
        assert_eq!(pulled.panel_of(Piece::Command), Some(Piece::Layers));
        // Every piece is always in exactly one panel, or closed.
        for piece in Piece::ALL {
            let holding = pulled
                .places
                .iter()
                .filter(|p| p.pieces.contains(&piece))
                .count()
                + usize::from(pulled.closed.contains(&piece));
            assert_eq!(holding, 1, "{piece:?}");
        }
    }

    #[test]
    fn dropped_panels_settle_on_the_grid_line_up_and_never_overlap() {
        let screen = window(1200.0, 800.0);
        let other = Rect::from_min_size(pos2(GAP, GAP), vec2(240.0, 240.0));
        // Off the grid, it settles on it, size rounded too.
        let settled = settle(
            Rect::from_min_size(pos2(300.0, 150.0), vec2(250.0, 230.0)),
            screen,
            &[other],
        );
        assert_eq!((settled.min.x - GAP) % GAP, 0.0);
        assert_eq!((settled.min.y - GAP) % GAP, 0.0);
        assert_eq!(settled.width() % GAP, 0.0);
        // Within 12 points of a neighbour's edge it lines up, a gap away.
        let near = settle(
            Rect::from_min_size(pos2(GAP + 240.0 + GAP + 10.0, GAP), vec2(240.0, 240.0)),
            screen,
            &[other],
        );
        assert_eq!(near.min.x, other.max.x + GAP);
        // Dropped over another panel, it moves to the nearest free spot.
        let moved = settle(
            Rect::from_min_size(pos2(60.0, 60.0), vec2(240.0, 240.0)),
            screen,
            &[other],
        );
        assert!(!moved.intersects(other));
        assert!(screen.contains_rect(moved));
    }

    #[test]
    fn resizing_from_an_edge_moves_that_side_only_and_a_corner_two() {
        let screen = window(1200.0, 800.0);
        let layout = Layout::default().moved(
            Piece::Layers,
            Dock::Floating { x: 300.0, y: 200.0 },
            vec2(240.0, 300.0),
        );
        let rect = |l: &Layout| rect_of(&l.arrange(screen).0, Piece::Layers);
        let before = rect(&layout);
        let side = |left, right, top, bottom| Sides {
            left,
            right,
            top,
            bottom,
        };
        let wider = layout.resized_from(
            Piece::Layers,
            side(false, true, false, false),
            vec2(48.0, 30.0),
            screen,
        );
        assert_eq!(rect(&wider).min, before.min);
        assert_eq!(rect(&wider).size(), before.size() + vec2(48.0, 0.0));
        let left = layout.resized_from(
            Piece::Layers,
            side(true, false, false, false),
            vec2(-48.0, 0.0),
            screen,
        );
        assert_eq!(rect(&left).max, before.max);
        assert_eq!(rect(&left).min.x, before.min.x - 48.0);
        let corner = layout.resized_from(
            Piece::Layers,
            side(false, true, false, true),
            vec2(24.0, 24.0),
            screen,
        );
        assert_eq!(rect(&corner).min, before.min);
        assert_eq!(rect(&corner).max, before.max + vec2(24.0, 24.0));
        // Docked on the right, only its inner edge moves.
        let docked = Layout::default();
        let before = rect(&docked);
        let pulled = docked.resized_from(
            Piece::Layers,
            side(true, false, true, false),
            vec2(-40.0, -40.0),
            screen,
        );
        assert_eq!(rect(&pulled).width(), before.width() + 40.0);
        assert_eq!(rect(&pulled).min.y, before.min.y);
    }

    #[test]
    fn a_panel_opened_while_settings_floats_goes_beside_it() {
        let layout = Layout::default();
        assert!(!layout.is_shown(Piece::Settings) && !layout.is_shown(Piece::Blocks));
        // With room, beside it.
        let wide = window(1800.0, 900.0);
        let both = layout
            .restored(Piece::Settings, wide)
            .restored(Piece::Blocks, wide);
        let (arranged, _) = both.arrange(wide);
        let settings = rect_of(&arranged, Piece::Settings);
        let blocks = rect_of(&arranged, Piece::Blocks);
        assert!(!settings.intersects(blocks), "{settings:?} {blocks:?}");
        // Without, against the window's edge: never straight on top.
        let narrow = window(1200.0, 800.0);
        let both = layout
            .restored(Piece::Settings, narrow)
            .restored(Piece::Blocks, narrow);
        let (arranged, _) = both.arrange(narrow);
        let settings = rect_of(&arranged, Piece::Settings);
        let blocks = rect_of(&arranged, Piece::Blocks);
        assert!(
            (settings.center().x - blocks.center().x).abs() > 200.0,
            "{settings:?} {blocks:?}"
        );
    }

    #[test]
    fn panels_close_and_come_back_and_tabs_reorder() {
        let screen = window(1200.0, 800.0);
        let layout = Layout::default();
        assert!(layout.is_complete());
        assert!(!layout.is_shown(Piece::Settings));
        // Closing a panel with tabs closes them all; each comes back on its own.
        let closed = layout.closing(Piece::Layers);
        assert!(closed.is_complete());
        assert!(!closed.is_shown(Piece::Layers) && !closed.is_shown(Piece::Properties));
        let back = closed.restored(Piece::Properties, screen);
        assert!(back.is_shown(Piece::Properties) && !back.is_shown(Piece::Layers));
        // Settings, with no standard place, opens floating in the centre of the window.
        let settings = layout.restored(Piece::Settings, screen);
        let rect = rect_of(&settings.arrange(screen).0, Piece::Settings);
        assert!((rect.center() - screen.center()).length() < 1.0, "{rect:?}");
        // Tabs reorder, and the one showing stays showing.
        let merged = layout.merged(Piece::Command, Piece::Layers);
        let moved = merged.reordered(Piece::Layers, 2, 0);
        let place = moved.place(Piece::Command).unwrap();
        assert_eq!(
            place.pieces,
            [Piece::Command, Piece::Layers, Piece::Properties]
        );
        assert_eq!(place.showing(), Piece::Command);
        // A file without the newer pieces gains them.
        let mut old = Layout::default();
        old.places.retain(|p| p.pieces != [Piece::Display]);
        old.closed.clear();
        assert!(old.completed().is_complete());
    }

    /// The Display panel opens wide enough for "Perspective" and its arrow, even from a file
    /// saved when it was narrower; a wider one keeps its width (#470).
    #[test]
    fn the_display_panel_is_wide_enough_for_its_names() {
        let width = |layout: &Layout| {
            let place = layout.places.iter().find(|p| p.pieces == [Piece::Display]);
            place.unwrap().size[0]
        };
        assert_eq!(width(&Layout::default()), DISPLAY_WIDTH);
        let mut old = Layout::default();
        for wide in [180.0, 320.0] {
            for place in &mut old.places {
                if place.pieces == [Piece::Display] {
                    place.size[0] = wide;
                }
            }
            assert_eq!(width(&old.clone().completed()), DISPLAY_WIDTH.max(wide));
        }
    }
}
