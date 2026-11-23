//! The floating panels' frames: docking, dragging, resizing, titles and focus.

use super::resize::resize_handles;
use crate::*;

/// The panels' focus, sliding and dragging.
#[derive(Default)]
pub(crate) struct Frames {
    /// Focus mode: the panels slid out of view.
    pub(crate) focus: bool,
    /// The height inside the command bar, and how much of it the prompt, options and toggles
    /// take when last drawn; the history fills the rest.
    pub(crate) command_room: f32,
    pub(crate) command_below: f32,
    /// The Layers panel's own state: the layer picked in it, the search and the filters.
    pub(crate) layers_ui: super::layer_tools::LayersUi,
    /// In focus mode, the one panel brought back out by clicking its strip.
    pub(crate) revealed: Option<layout::PanelId>,
    /// Where each panel was last drawn, for hovering over a hidden one.
    pub(crate) drawn_panels: Vec<(layout::PanelId, egui::Rect)>,
    /// A panel being moved by its grip.
    pub(crate) panel_drag: Option<PanelDrag>,
    /// While a panel's edge is held, the arrangement as it was when the drag began, so the
    /// edge follows the pointer from there and the arrangement is saved once, when let go.
    pub(crate) resize_from: Option<layout::Layout>,
}

/// What happened to a panel this frame.
pub(crate) enum PanelEvent {
    Nothing,
    /// A hidden panel's strip was clicked, in focus mode.
    StripClicked,
    /// Its title began to be dragged, held this far from its corner: one tab's piece, or the
    /// whole panel.
    DragStarted(egui::Vec2, Option<layout::Piece>),
    /// These sides of it are being dragged to resize it, this far since the drag began.
    Resized(layout::Sides, egui::Vec2),
    /// Reset layout was chosen from its grip's menu.
    Reset,
    /// Its hide control was clicked.
    Hide,
    /// One of its tabs was clicked.
    TabChosen(usize),
    /// A tab was dragged along the row from one place to another.
    TabMoved(usize, usize),
    /// Its cross was clicked: ask, then close it.
    Close,
}

/// A panel being moved by its grip.
#[derive(Clone, Copy)]
pub(crate) struct PanelDrag {
    pub(crate) panel: layout::PanelId,
    /// The piece being pulled out by its tab, if only one is.
    pub(crate) piece: Option<layout::Piece>,
    /// Where the grip was held, from the panel's corner.
    pub(crate) grab: egui::Vec2,
    /// Its size while it moves.
    pub(crate) size: egui::Vec2,
}

/// The size of the tab at the top centre that turns focus mode on and off.
const FOCUS_TAB: egui::Vec2 = egui::vec2(44.0, 14.0);

/// A hidden panel's `tab` moved aside if it would lie under the focus tab at the top centre of
/// `window`, as a wide top panel's does in focus mode (#473), so both can be seen and clicked.
fn clear_of_focus_tab(tab: egui::Rect, window: egui::Rect) -> egui::Rect {
    let focus = egui::Rect::from_min_size(
        egui::pos2(window.center().x - FOCUS_TAB.x / 2.0, window.min.y),
        FOCUS_TAB,
    )
    .expand2(egui::vec2(4.0, 0.0));
    if tab.intersects(focus) {
        tab.translate(egui::vec2(focus.max.x - tab.min.x, 0.0))
    } else {
        tab
    }
}

/// What every panel drawn in one frame shares: the window below the top bar, whether a command
/// is being typed, the pointer, whether the mouse button is up, the panel a dragged one would
/// merge into, and the rectangles of the panels not being dragged.
struct PanelPass<'a> {
    window: egui::Rect,
    /// The window hidden panels slide to the edges of: the whole window in focus mode, where the
    /// top bar has slid away too, else the window below the top bar (#473).
    edges: egui::Rect,
    typing: bool,
    pointer: Option<egui::Pos2>,
    released: bool,
    merge_into: Option<layout::PanelId>,
    floating: &'a [egui::Rect],
}

/// Where the dragged `piece` would dock in `landing`, outlined in the accent.
fn dock_preview(
    ctx: &egui::Context,
    landing: &layout::Layout,
    piece: layout::Piece,
    window: egui::Rect,
) {
    if let Some(id) = landing.panel_of(piece)
        && let Some((_, spot)) = landing
            .arrange(window)
            .0
            .into_iter()
            .find(|(p, _)| *p == id)
    {
        ui_kit::dock_preview(ctx, spot);
    }
}

/// Whether a panel made of `pieces` closes at once, without asking: Settings, and the panels a
/// command opens (Print, Layouts, Blocks, annotation styles, notes), come back by running it
/// again, so there is nothing to warn about (#483). The others ask, saying where to bring them back.
fn closes_without_asking(pieces: &[layout::Piece]) -> bool {
    use layout::Piece;
    !pieces.is_empty()
        && pieces.iter().all(|piece| {
            matches!(
                piece,
                Piece::Settings
                    | Piece::Print
                    | Piece::Layouts
                    | Piece::Blocks
                    | Piece::AnnotationStyles
                    | Piece::Notes
            )
        })
}

impl Curva {
    /// Makes the command tabs, docked at the top or bottom on their own, exactly as tall as what
    /// they hold, as measured when last drawn (owner, 22 November 2026: no room to spare below
    /// the buttons). Not while a panel is being dragged or resized.
    fn fit_command_tabs(&mut self) {
        if self.frames.panel_drag.is_some() {
            return;
        }
        // The command bar is not fitted: its height is the user's, and the history fills it.
        let fit = self.bar.tabs_fit;
        if fit <= 0.0 {
            return;
        }
        if let Some(place) = self.layout.places.iter_mut().find(|p| {
            p.is_command_tabs_alone()
                && !p.hidden
                && matches!(p.dock, layout::Dock::Top | layout::Dock::Bottom)
        }) && (place.size[1] - fit).abs() > 0.5
        {
            place.size[1] = fit;
        }
    }

    /// Lays out and draws the panels, sliding hidden ones to their edges (all of them in focus
    /// mode). Returns the area left for the viewports, and how far they may reach to be seen
    /// through glass panels.
    pub(crate) fn panels(
        &mut self,
        ctx: &egui::Context,
        window: egui::Rect,
    ) -> (egui::Rect, egui::Rect) {
        if ctx.input(|input| input.key_pressed(egui::Key::F12)) {
            self.toggle_focus();
        }
        self.history_keys(ctx);
        self.window_rect = window;
        let full_window = window;
        let window = self.top_bar_slot(ctx, window);
        self.fit_command_tabs();
        let (placed, free) = self.layout.arrange(window);
        // While a panel is dragged over another, that one lights up to show they would merge.
        let merge_into = self.frames.panel_drag.and_then(|drag| {
            let at = ctx.pointer_latest_pos()?;
            self.frames
                .drawn_panels
                .iter()
                .find(|(p, rect)| *p != drag.panel && rect.contains(at))
                .map(|(p, _)| *p)
        });
        let floating: Vec<egui::Rect> = placed
            .iter()
            .filter(|(panel, _)| {
                self.frames
                    .panel_drag
                    .is_none_or(|drag| drag.panel != *panel)
            })
            .map(|(_, rect)| *rect)
            .collect();
        let pass = PanelPass {
            window,
            edges: if self.frames.focus {
                full_window
            } else {
                window
            },
            // Typing brings the command line's panel out until the command ends.
            typing: !self.typed.is_empty() || self.processor.is_running(),
            pointer: ctx.pointer_hover_pos(),
            released: !ctx.input(|input| input.pointer.primary_down()),
            merge_into,
            floating: &floating,
        };
        for (panel, rect) in placed {
            self.one_panel(ctx, panel, rect, &pass);
        }
        // A resize is kept once its edge is let go.
        if pass.released && self.frames.resize_from.take().is_some() {
            self.save_layout();
        }
        self.focus_tab(ctx, full_window);
        self.version_label(ctx, full_window);
        // In focus mode the viewports take the window, short of the tabs left at its edges.
        if self.frames.focus {
            let shrunk = full_window.shrink(layout::STRIP);
            return (shrunk, shrunk);
        }
        // The viewports reach under the glass panels, so there is something to see through
        // them; their titles and frames stay in the free area.
        (free, full_window)
    }

    /// The top bar, fixed along the top and sliding up out of view in focus mode; returns the
    /// window below it, where the other panels are laid out.
    fn top_bar_slot(&mut self, ctx: &egui::Context, window: egui::Rect) -> egui::Rect {
        let bar = egui::Rect::from_min_size(
            window.min + egui::Vec2::splat(layout::GAP),
            egui::vec2(window.width() - 2.0 * layout::GAP, TOP_BAR_HEIGHT),
        );
        let lift = ctx.animate_value_with_time(
            egui::Id::NULL.with("top bar slide"),
            if self.frames.focus {
                bar.max.y - window.min.y
            } else {
                0.0
            },
            SLIDE_TIME,
        );
        self.top_bar(ctx, bar.translate(egui::vec2(0.0, -lift)));
        egui::Rect::from_min_max(egui::pos2(window.min.x, bar.max.y), window.max)
    }

    /// One panel, laid out at `rect`: shown, or slid to its edge leaving a tab there (peeking
    /// out while the pointer is over it), or following the pointer while dragged.
    fn one_panel(
        &mut self,
        ctx: &egui::Context,
        panel: layout::PanelId,
        rect: egui::Rect,
        pass: &PanelPass<'_>,
    ) {
        let window = pass.window;
        let Some(place) = self.layout.place(panel).cloned() else {
            return;
        };
        let shown = (!self.frames.focus && !place.hidden)
            || self.frames.revealed == Some(panel)
            || (place.pieces.contains(&layout::Piece::Command) && pass.typing && !place.hidden);
        let edge = self.layout.edge_of(panel, rect, window);
        let (tab, upright) = layout::edge_tab(rect, edge, pass.edges);
        let tab = clear_of_focus_tab(tab, pass.edges);
        let hovered = pass.pointer.is_some_and(|at| {
            tab.contains(at)
                || self
                    .frames
                    .drawn_panels
                    .iter()
                    .any(|(p, drawn)| *p == panel && drawn.contains(at))
        });
        let target = if shown {
            rect
        } else {
            let visible = if hovered { layout::PEEK } else { 0.0 };
            layout::slid(rect, edge, pass.edges, visible)
        };
        let id = egui::Id::NULL.with(("panel slide", panel));
        let offset = egui::vec2(
            ctx.animate_value_with_time(id.with(0), target.min.x - rect.min.x, SLIDE_TIME),
            ctx.animate_value_with_time(id.with(1), target.min.y - rect.min.y, SLIDE_TIME),
        );
        let (drawn, size) = self
            .dragged_panel(ctx, panel, pass)
            .unwrap_or((rect.translate(offset), rect.size()));
        self.frames.drawn_panels.retain(|(p, _)| *p != panel);
        if shown || hovered || self.frames.panel_drag.is_some_and(|d| d.panel == panel) {
            self.frames.drawn_panels.push((panel, drawn));
        }
        let mut events = Vec::new();
        if !shown {
            events.push(self.edge_tab(ctx, &place, tab, upright));
        }
        if shown || hovered || offset != target.min - rect.min || drawn != target {
            events.push(self.panel_area(ctx, &place, size, drawn, pass.merge_into == Some(panel)));
        }
        for event in events {
            self.panel_event(event, panel, &place, drawn, window);
        }
    }

    /// If `panel` is being dragged: where it is drawn, following the pointer, and its size. It
    /// shows where it would land, and lands there when let go.
    fn dragged_panel(
        &mut self,
        ctx: &egui::Context,
        panel: layout::PanelId,
        pass: &PanelPass<'_>,
    ) -> Option<(egui::Rect, egui::Vec2)> {
        let (Some(drag), Some(at)) = (self.frames.panel_drag, ctx.pointer_latest_pos()) else {
            return None;
        };
        if drag.panel != panel {
            return None;
        }
        let drawn = egui::Rect::from_min_size(at - drag.grab, drag.size);
        let landing = self.landing(drag, at, drawn, pass.window, pass.merge_into, pass.floating);
        if pass.merge_into.is_none() {
            dock_preview(ctx, &landing, drag.piece.unwrap_or(panel), pass.window);
        }
        if pass.released {
            self.layout = landing;
            self.frames.panel_drag = None;
            self.save_layout();
        }
        Some((drawn, drag.size))
    }

    /// Where a dragged panel would go if let go with the pointer at `at`: merged into the panel
    /// under the pointer, docked to a near edge, or floating, settled on the grid.
    pub(crate) fn landing(
        &self,
        drag: PanelDrag,
        at: egui::Pos2,
        drawn: egui::Rect,
        window: egui::Rect,
        merge_into: Option<layout::PanelId>,
        others: &[egui::Rect],
    ) -> layout::Layout {
        if let Some(onto) = merge_into {
            return match drag.piece {
                Some(piece) => {
                    let alone = self.layout.pulled(
                        piece,
                        layout::Dock::Floating { x: 0.0, y: 0.0 },
                        drag.size,
                    );
                    alone.merged(piece, onto)
                }
                None => self.layout.merged(drag.panel, onto),
            };
        }
        let mut size = drag.size;
        let mut dock = layout::drop_dock(at, drawn.min, window);
        if matches!(dock, layout::Dock::Floating { .. }) {
            let settled = layout::settle(drawn, window, others);
            size = settled.size();
            dock = layout::Dock::Floating {
                x: settled.min.x - window.min.x,
                y: settled.min.y - window.min.y,
            };
        }
        match drag.piece {
            Some(piece) => self.layout.pulled(piece, dock, size),
            None => self.layout.moved(drag.panel, dock, size),
        }
    }

    /// Carries out what happened on one panel.
    pub(crate) fn panel_event(
        &mut self,
        event: PanelEvent,
        panel: layout::PanelId,
        place: &layout::Place,
        drawn: egui::Rect,
        window: egui::Rect,
    ) {
        match event {
            PanelEvent::Nothing => {}
            PanelEvent::StripClicked => {
                if place.hidden {
                    self.layout = self.layout.with_hidden(panel, false);
                    self.save_layout();
                } else {
                    self.frames.revealed = Some(panel);
                }
            }
            PanelEvent::DragStarted(grab, piece) => {
                self.frames.panel_drag = Some(PanelDrag {
                    panel,
                    piece: piece.filter(|_| place.pieces.len() > 1),
                    grab,
                    size: drawn.size().min(window.size() * 0.6).max(layout::MIN_PANEL),
                });
            }
            PanelEvent::Resized(sides, by) => {
                let start = self
                    .frames
                    .resize_from
                    .get_or_insert_with(|| self.layout.clone());
                self.layout = start.resized_from(panel, sides, by, window);
            }
            PanelEvent::Reset => {
                self.layout = layout::Layout::default();
                self.save_layout();
            }
            PanelEvent::Hide => {
                if self.frames.revealed == Some(panel) {
                    self.frames.revealed = None;
                } else {
                    self.layout = self.layout.with_hidden(panel, true);
                    self.save_layout();
                }
            }
            PanelEvent::TabMoved(from, to) => {
                self.layout = self.layout.reordered(panel, from, to);
                self.save_layout();
            }
            PanelEvent::Close => {
                let pieces = self.layout.place(panel).map(|p| p.pieces.clone());
                if pieces.is_some_and(|pieces| closes_without_asking(&pieces)) {
                    self.layout = self.layout.closing(panel);
                    self.save_layout();
                } else {
                    self.closing = Some(panel);
                }
            }
            PanelEvent::TabChosen(index) => {
                self.layout = self.layout.with_active(panel, index);
                self.save_layout();
            }
        }
    }

    /// The tab a hidden panel leaves at its edge, with its pieces' names written along it:
    /// hovering it peeks the panel out, clicking brings it back, dragging pulls it out.
    pub(crate) fn edge_tab(
        &self,
        ctx: &egui::Context,
        place: &layout::Place,
        tab: egui::Rect,
        upright: bool,
    ) -> PanelEvent {
        let mut event = PanelEvent::Nothing;
        let names: Vec<String> = place
            .pieces
            .iter()
            .map(|piece| self.text.render(&Message::new(piece.name_key())))
            .collect();
        egui::Area::new(egui::Id::NULL.with(("edge tab", place.id())))
            .fixed_pos(tab.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(tab.size(), egui::Sense::click_and_drag());
                ui_kit::edge_strip(ui, rect, response.hovered(), names.join(" · "), upright);
                if response.clicked() {
                    event = PanelEvent::StripClicked;
                } else if response.drag_started() {
                    event = PanelEvent::DragStarted(egui::vec2(24.0, 12.0), None);
                }
            });
        event
    }

    /// Draws one panel at `drawn`, with rounded corners and a soft shadow: its name centred
    /// along its top, or a tab per piece, which are its grips; a control to hide it; and on a
    /// floating one a corner to resize it by. `glowing` lights it in the accent while another
    /// panel dragged over it would merge into it.
    pub(crate) fn panel_area(
        &mut self,
        ctx: &egui::Context,
        place: &layout::Place,
        size: egui::Vec2,
        drawn: egui::Rect,
        glowing: bool,
    ) -> PanelEvent {
        let rect = drawn;
        let panel = place.id();
        let mut event = PanelEvent::Nothing;
        let glass = self.settings.glass;
        let frame = ui_kit::panel_frame_for(glass);
        egui::Area::new(egui::Id::NULL.with(("panel", panel)))
            .fixed_pos(rect.min)
            .order(egui::Order::Middle)
            .constrain(false)
            .show(ctx, |ui| {
                // Nothing a panel holds may spill outside its place.
                ui.set_clip_rect(rect.expand(SHADOW_ROOM));
                // The blurred view behind, under the tint the frame paints over it.
                if glass {
                    ui_kit::glass_backdrop(ui, rect);
                }
                // Where the panel is drawn: its place, or wider when what it holds will not fit
                // (the Display panel's view field), so the glow and the resize edges follow the
                // edge that shows rather than cutting through the panel (owner, 22 November 2026).
                let shown = frame
                    .show(ui, |ui| {
                        let inner = size - egui::Vec2::splat(2.0 * PANEL_MARGIN);
                        ui.set_min_size(inner);
                        ui.set_max_size(inner);
                        // The command tabs' groups sit in the title row's place (option E).
                        if !place.is_command_tabs_alone() && !place.is_command_alone() {
                            ui.add_space(TITLE_ROW);
                        }
                        self.piece_contents(ui, place.showing());
                    })
                    .response
                    .rect
                    .union(rect);
                // A dragged panel would merge into this one: it glows inside its edge, most
                // where the mouse is.
                if glass {
                    ui_kit::glass_edge_light(ui, shown);
                }
                if glowing {
                    ui_kit::glow::merge_glow(ui, shown);
                }
                // Added after the contents, so these sit above the panel's own controls.
                event = self.panel_title(ui, place, rect);
                if let Some(resize) = resize_handles(ui, place, shown) {
                    event = resize;
                }
            });
        event
    }

    /// What a piece shows inside its panel. Each is inside a scroll area, so nothing a piece
    /// holds grows the panel past its place.
    fn piece_contents(&mut self, ui: &mut egui::Ui, piece: layout::Piece) {
        use layout::Piece;
        use ui_kit::Scroll;
        match piece {
            Piece::CommandTabs => ui_kit::scroll(ui, Scroll::Down, |ui| self.command_tabs(ui)),
            Piece::Layers => ui_kit::scroll(ui, Scroll::Both, |ui| self.layers_panel(ui)),
            Piece::Properties => ui_kit::scroll(ui, Scroll::Both, |ui| self.properties_panel(ui)),
            Piece::Command => {
                let bar = ui.max_rect();
                self.frames.command_room = ui.available_height();
                ui_kit::scroll(ui, Scroll::DownFromBottom, |ui| self.command_line(ui));
                // Long jobs over the bar: a progress line and a chip that can stop them (#480).
                if self.command_jobs(ui, bar) {
                    self.processor.stop_background(&mut self.doc);
                }
            }
            Piece::Agent => ui_kit::scroll(ui, Scroll::Down, |ui| self.assistant_panel(ui)),
            Piece::Display => self.display_piece(ui),
            Piece::Settings => self.settings_piece(ui),
            Piece::Blocks => ui_kit::scroll(ui, Scroll::Both, |ui| self.blocks_panel(ui)),
            Piece::Layouts => ui_kit::scroll(ui, Scroll::Both, |ui| self.layouts_panel(ui)),
            Piece::AnnotationStyles => {
                ui_kit::scroll(ui, Scroll::Down, |ui| self.annotation_styles_panel(ui));
            }
            Piece::Notes => ui_kit::scroll(ui, Scroll::Down, |ui| self.notes_panel(ui)),
            Piece::Print => ui_kit::scroll(ui, Scroll::Down, |ui| self.print_panel(ui)),
        }
    }

    /// The software's name and version, small and quiet, at the window's bottom right.
    pub(crate) fn version_label(&self, ctx: &egui::Context, window: egui::Rect) {
        let text = format!("Curva {}", env!("CARGO_PKG_VERSION"));
        ui_kit::corner_caption(ctx, window, text);
    }

    /// Hides every panel, or brings them all back.
    pub(crate) fn toggle_focus(&mut self) {
        self.frames.focus = !self.frames.focus;
        self.frames.revealed = None;
    }

    /// The small tab with an arrow at the top centre that turns focus mode on and off. With one
    /// panel brought out of focus mode, it hides that panel again.
    pub(crate) fn focus_tab(&mut self, ctx: &egui::Context, window: egui::Rect) {
        let size = FOCUS_TAB;
        let corner = egui::pos2(window.center().x - size.x / 2.0, window.min.y);
        egui::Area::new(egui::Id::NULL.with("focus tab"))
            .fixed_pos(corner)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
                // A V while the panels are hidden, to bring them down; turned over to hide them.
                ui_kit::hanging_tab(ui, rect, response.hovered(), self.frames.focus);
                let tip_text = if self.frames.focus {
                    "ui.focus.off"
                } else {
                    "ui.focus.on"
                };
                if response
                    .on_hover_text(self.text.render(&Message::new(tip_text)))
                    .clicked()
                {
                    if self.frames.revealed.is_some() {
                        self.frames.revealed = None;
                    } else {
                        self.toggle_focus();
                    }
                }
            });
    }

    /// Keeps the panel arrangement for the next session. Losing it only loses the arrangement.
    pub(crate) fn save_layout(&self) {
        if self.benchmark.is_none()
            && !self.startup.pictures_run
            && let Some(file) = layout::file()
        {
            let _ = self.layout.save(&file);
        }
    }

    /// Asks whether to close a panel, saying where to bring it back, before closing it.
    pub(crate) fn close_question(&mut self, ctx: &egui::Context) {
        let Some(panel) = self.closing else {
            return;
        };
        let names: Vec<String> = self
            .layout
            .place(panel)
            .map(|p| {
                p.pieces
                    .iter()
                    .map(|piece| self.text.render(&Message::new(piece.name_key())))
                    .collect()
            })
            .unwrap_or_default();
        let say = |key: &'static str| self.text.render(&Message::new(key));
        let question = self
            .text
            .render(&Message::with("ui.close.question", vec![names.join(", ")]));
        let buttons = [
            ui_kit::main_choice(say("ui.close.close")),
            ui_kit::other_choice(say("ui.units.cancel")),
        ];
        let answer = ui_kit::dialog(ctx, "close question", &question, &buttons, |ui| {
            ui.label(say("ui.close.where"));
        })
        .map(|index| index == 0);
        if let Some(close) = answer {
            self.closing = None;
            if close {
                self.layout = self.layout.closing(panel);
                self.save_layout();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FOCUS_TAB, clear_of_focus_tab, closes_without_asking};
    use crate::layout::Piece;
    use crate::{egui, layout};

    #[test]
    fn settings_and_command_panels_close_without_asking_the_others_ask() {
        assert!(closes_without_asking(&[Piece::Settings]));
        assert!(closes_without_asking(&[Piece::Print, Piece::Layouts]));
        assert!(!closes_without_asking(&[Piece::Layers]));
        assert!(!closes_without_asking(&[
            Piece::Settings,
            Piece::Properties
        ]));
        assert!(!closes_without_asking(&[]));
    }

    /// In focus mode a panel at the top, as the Operations bar is, slides to the top of the whole
    /// window, as the others slide to theirs, and its tab there moves aside from the focus tab at
    /// the top centre instead of lying under it (#473).
    #[test]
    fn a_top_panel_in_focus_mode_goes_to_the_window_edge_beside_the_focus_tab() {
        let full = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1600.0, 900.0));
        let bar = egui::Rect::from_min_max(egui::pos2(24.0, 90.0), egui::pos2(1576.0, 170.0));
        let hidden = layout::slid(bar, layout::Edge::Top, full, 0.0);
        assert!(hidden.max.y <= full.min.y + 0.5, "{hidden:?}");
        let (tab, _) = layout::edge_tab(bar, layout::Edge::Top, full);
        assert_eq!(tab.min.y, full.min.y);
        let moved = clear_of_focus_tab(tab, full);
        let focus = egui::Rect::from_center_size(
            egui::pos2(full.center().x, full.min.y + FOCUS_TAB.y / 2.0),
            FOCUS_TAB,
        );
        assert!(
            !moved.intersects(focus) && moved.min.y == tab.min.y,
            "{moved:?}"
        );
        // A tab away from the centre stays where it is.
        let side = egui::Rect::from_min_size(egui::pos2(100.0, 0.0), egui::vec2(140.0, 22.0));
        assert_eq!(clear_of_focus_tab(side, full), side);
    }
}
