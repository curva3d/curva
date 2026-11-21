//! The floating panels' frames: docking, dragging, resizing, titles and focus.

use super::resize::resize_handles;
use crate::*;

/// The panels' focus, sliding and dragging.
#[derive(Default)]
pub(crate) struct Frames {
    /// Focus mode: the panels slid out of view.
    pub(crate) focus: bool,
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

/// The frame every panel is drawn in: rounded, edged and shadowed, glass or solid. A panel that
/// a dragged one would merge into glows inside its edge as well (`glow::edge_glow`).
pub(crate) fn panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::current().glass)
        .stroke(egui::Stroke::new(1.0, theme::current().panel_edge))
        .corner_radius(PANEL_RADIUS)
        .shadow(egui::Shadow {
            offset: [0, 4],
            blur: 16,
            spread: 0,
            color: egui::Color32::from_black_alpha(90),
        })
        .inner_margin(PANEL_MARGIN)
}

/// A panel frame lit in the accent all round, for a box being typed into (text edited in place).
pub(crate) fn lit_panel_frame() -> egui::Frame {
    panel_frame()
        .stroke(egui::Stroke::new(2.0, theme::ACCENT))
        .shadow(egui::Shadow {
            offset: [0, 4],
            blur: 24,
            spread: 0,
            color: theme::ACCENT.gamma_multiply(0.5),
        })
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
        ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::NULL.with("dock preview"),
        ))
        .rect(
            spot,
            PANEL_RADIUS,
            DOCK_PREVIEW_FILL,
            egui::Stroke::new(1.5, theme::ACCENT.gamma_multiply(0.6)),
            egui::StrokeKind::Inside,
        );
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
                let fill = if response.hovered() {
                    theme::current().panel_edge
                } else {
                    theme::current().panel
                };
                ui.painter().rect_filled(rect, 6.0, fill);
                let font = egui::FontId::proportional(12.0);
                let galley =
                    ui.painter()
                        .layout_no_wrap(names.join(" · "), font, theme::current().quiet);
                let text_size = galley.size();
                let (position, angle) = if upright {
                    // Running upwards along a side.
                    (
                        egui::pos2(
                            rect.center().x - text_size.y / 2.0,
                            rect.center().y + text_size.x / 2.0,
                        ),
                        -std::f32::consts::FRAC_PI_2,
                    )
                } else {
                    (rect.center() - text_size / 2.0, 0.0)
                };
                ui.painter().add(
                    egui::epaint::TextShape::new(position, galley, theme::current().quiet)
                        .with_angle(angle),
                );
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
        let frame = panel_frame().fill(if glass {
            theme::current().glass
        } else {
            theme::current().panel
        });
        egui::Area::new(egui::Id::NULL.with(("panel", panel)))
            .fixed_pos(rect.min)
            .order(egui::Order::Middle)
            .constrain(false)
            .show(ctx, |ui| {
                // Nothing a panel holds may spill outside its place.
                ui.set_clip_rect(rect.expand(SHADOW_ROOM));
                // The blurred view behind, under the tint the frame paints over it.
                if glass {
                    ui.painter().add(egui::Shape::from(
                        eframe::egui_wgpu::Callback::new_paint_callback(
                            rect,
                            curva_viewport::Glass {
                                rect,
                                radius: f32::from(PANEL_RADIUS),
                            },
                        ),
                    ));
                }
                frame.show(ui, |ui| {
                    let inner = size - egui::Vec2::splat(2.0 * PANEL_MARGIN);
                    ui.set_min_size(inner);
                    ui.set_max_size(inner);
                    ui.add_space(TITLE_ROW);
                    self.piece_contents(ui, place.showing());
                });
                // A dragged panel would merge into this one: it glows inside its edge, most
                // where the mouse is.
                if glowing && let Some(pointer) = ui.ctx().pointer_latest_pos() {
                    ui.painter().add(crate::panels::glow::edge_glow(
                        rect,
                        f32::from(PANEL_RADIUS),
                        pointer,
                    ));
                }
                // Added after the contents, so these sit above the panel's own controls.
                event = self.panel_title(ui, place, rect);
                if let Some(resize) = resize_handles(ui, place, rect) {
                    event = resize;
                }
            });
        event
    }

    /// What a piece shows inside its panel. Each is inside a scroll area, so nothing a piece
    /// holds grows the panel past its place.
    fn piece_contents(&mut self, ui: &mut egui::Ui, piece: layout::Piece) {
        match piece {
            layout::Piece::CommandTabs => {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.command_tabs(ui));
            }
            layout::Piece::Layers => {
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.layers_panel(ui));
            }
            layout::Piece::Properties => {
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.properties_panel(ui));
            }
            layout::Piece::Command => {
                let bar = ui.max_rect();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| self.command_line(ui));
                // Long jobs over the bar: a progress line and a chip that can stop them (#480).
                if self.command_jobs(ui, bar) {
                    self.processor.cancel(&mut self.doc);
                }
            }
            layout::Piece::Agent => {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.assistant_panel(ui));
            }
            layout::Piece::Display => self.display_piece(ui),
            layout::Piece::Settings => self.settings_piece(ui),
            layout::Piece::Blocks => {
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.blocks_panel(ui));
            }
            layout::Piece::Layouts => {
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.layouts_panel(ui));
            }
            layout::Piece::AnnotationStyles => {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.annotation_styles_panel(ui));
            }
            layout::Piece::Notes => {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.notes_panel(ui));
            }
            layout::Piece::Print => {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.print_panel(ui));
            }
        }
    }

    /// The software's name and version, small and quiet, at the window's bottom right.
    pub(crate) fn version_label(&self, ctx: &egui::Context, window: egui::Rect) {
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Background,
            egui::Id::NULL.with("version"),
        ));
        painter.text(
            window.max - egui::vec2(8.0, 4.0),
            egui::Align2::RIGHT_BOTTOM,
            format!("Curva {}", env!("CARGO_PKG_VERSION")),
            egui::FontId::proportional(11.0),
            theme::current().quiet,
        );
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
                let fill = if response.hovered() {
                    theme::current().panel_edge
                } else {
                    theme::current().panel
                };
                let radius = egui::CornerRadius {
                    nw: 0,
                    ne: 0,
                    sw: 7,
                    se: 7,
                };
                ui.painter().rect_filled(rect, radius, fill);
                // A V while the panels are hidden, to bring them down; turned over to hide them.
                let (mid, half) = (rect.center(), egui::vec2(6.0, 3.0));
                let tip = if self.frames.focus { half.y } else { -half.y };
                let stroke = egui::Stroke::new(1.5, theme::current().quiet);
                ui.painter().line_segment(
                    [mid + egui::vec2(-half.x, -tip), mid + egui::vec2(0.0, tip)],
                    stroke,
                );
                ui.painter().line_segment(
                    [mid + egui::vec2(0.0, tip), mid + egui::vec2(half.x, -tip)],
                    stroke,
                );
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
        let mut answer = None;
        centred_dialog(ctx, "close question", |ui| {
            ui.label(egui::RichText::new(question).strong());
            ui.label(say("ui.close.where"));
            ui.horizontal(|ui| {
                if ui.button(say("ui.close.close")).clicked() {
                    answer = Some(true);
                }
                if ui.button(say("ui.units.cancel")).clicked() {
                    answer = Some(false);
                }
            });
        });
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
