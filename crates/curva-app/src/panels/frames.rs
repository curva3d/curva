//! The floating panels' frames: docking, dragging, resizing, titles and focus.

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
}

/// What happened to a panel this frame.
pub(crate) enum PanelEvent {
    Nothing,
    /// A hidden panel's strip was clicked, in focus mode.
    StripClicked,
    /// Its title began to be dragged, held this far from its corner: one tab's piece, or the
    /// whole panel.
    DragStarted(egui::Vec2, Option<layout::Piece>),
    /// These sides of it were dragged this far to resize it.
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

/// The frame every panel is drawn in: rounded, edged and shadowed, glass or solid; glowing in the
/// accent while another panel dragged over it would merge into it.
pub(crate) fn panel_frame(glowing: bool) -> egui::Frame {
    egui::Frame::new()
        .fill(theme::current().glass)
        .stroke(egui::Stroke::new(
            if glowing { 2.0 } else { 1.0 },
            if glowing {
                theme::ACCENT
            } else {
                theme::current().panel_edge
            },
        ))
        .corner_radius(PANEL_RADIUS)
        .shadow(egui::Shadow {
            offset: [0, 4],
            blur: if glowing { 24 } else { 16 },
            spread: 0,
            color: if glowing {
                theme::ACCENT.gamma_multiply(0.5)
            } else {
                egui::Color32::from_black_alpha(90)
            },
        })
        .inner_margin(PANEL_MARGIN)
}

/// A V pointing along `towards` (a unit direction on screen), as the hide control draws it.
pub(crate) fn paint_chevron(
    painter: &egui::Painter,
    centre: egui::Pos2,
    towards: egui::Vec2,
    stroke: egui::Stroke,
) {
    let side = egui::vec2(-towards.y, towards.x);
    let tip = centre + towards * 2.5;
    let back = centre - towards * 2.5;
    painter.line_segment([back + side * 4.5, tip], stroke);
    painter.line_segment([tip, back - side * 4.5], stroke);
}

/// Where along a row of tabs a tab let go at `x` lands, as an index among them.
pub(crate) fn tab_slot(tabs: &[egui::Rect], x: f32) -> usize {
    tabs.iter()
        .position(|tab| x < tab.center().x)
        .unwrap_or(tabs.len().saturating_sub(1))
}

/// The edges and corners a panel can be resized from: near one, the pointer becomes a resize
/// arrow and the edge pulses gently in the accent. The handles lie just inside the panel, so a drag
/// from one never reaches the viewport behind. Returns the drag, if one is under way.
pub(crate) fn resize_handles(
    ui: &egui::Ui,
    place: &layout::Place,
    rect: egui::Rect,
) -> Option<PanelEvent> {
    let movable = layout::Sides::movable(place);
    let side = |left, right, top, bottom| layout::Sides {
        left,
        right,
        top,
        bottom,
    };
    let reach = RESIZE_REACH;
    let (l, r, t, b) = (rect.min.x, rect.max.x, rect.min.y, rect.max.y);
    let zone = |x0: f32, y0: f32, x1: f32, y1: f32| {
        egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1))
    };
    let handles = [
        (
            side(true, false, true, false),
            zone(l, t, l + 2.0 * reach, t + 2.0 * reach),
            egui::CursorIcon::ResizeNwSe,
        ),
        (
            side(false, true, false, true),
            zone(r - 2.0 * reach, b - 2.0 * reach, r, b),
            egui::CursorIcon::ResizeNwSe,
        ),
        (
            side(false, true, true, false),
            zone(r - 2.0 * reach, t, r, t + 2.0 * reach),
            egui::CursorIcon::ResizeNeSw,
        ),
        (
            side(true, false, false, true),
            zone(l, b - 2.0 * reach, l + 2.0 * reach, b),
            egui::CursorIcon::ResizeNeSw,
        ),
        (
            side(true, false, false, false),
            zone(l, t + 2.0 * reach, l + reach, b - 2.0 * reach),
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            side(false, true, false, false),
            zone(r - reach, t + 2.0 * reach, r, b - 2.0 * reach),
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            side(false, false, true, false),
            zone(l + 2.0 * reach, t, r - 2.0 * reach, t + reach),
            egui::CursorIcon::ResizeVertical,
        ),
        (
            side(false, false, false, true),
            zone(l + 2.0 * reach, b - reach, r - 2.0 * reach, b),
            egui::CursorIcon::ResizeVertical,
        ),
    ];
    let mut event = None;
    for (sides, area, cursor) in handles {
        // A corner needs both its sides to be movable, an edge its one.
        if sides.within(movable) != sides {
            continue;
        }
        let key = (sides.left, sides.right, sides.top, sides.bottom);
        let response = ui
            .interact(
                area,
                ui.scope_id().with(("resize", key)),
                egui::Sense::drag(),
            )
            .on_hover_cursor(cursor);
        if response.hovered() || response.dragged() {
            // A gentle pulse along the edges being held.
            let time = ui.input(|input| input.time) as f32;
            let strength = 0.55 + 0.45 * (time * 5.0).sin();
            let stroke = egui::Stroke::new(2.0, theme::ACCENT.gamma_multiply(strength));
            let painter = ui.painter();
            if sides.left {
                painter.line_segment([rect.left_top(), rect.left_bottom()], stroke);
            }
            if sides.right {
                painter.line_segment([rect.right_top(), rect.right_bottom()], stroke);
            }
            if sides.top {
                painter.line_segment([rect.left_top(), rect.right_top()], stroke);
            }
            if sides.bottom {
                painter.line_segment([rect.left_bottom(), rect.right_bottom()], stroke);
            }
            ui.ctx().request_repaint();
        }
        if response.dragged() {
            event = Some(PanelEvent::Resized(sides, response.drag_delta()));
        }
    }
    event
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
        // The top bar sits fixed along the top, sliding up out of view in focus mode; the other
        // panels are laid out in the window below it.
        let full_window = window;
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
        let window = egui::Rect::from_min_max(egui::pos2(window.min.x, bar.max.y), window.max);
        let (placed, free) = self.layout.arrange(window);
        // Typing brings the command line's panel out until the command ends.
        let typing = !self.typed.is_empty() || self.processor.is_running();
        let pointer = ctx.pointer_hover_pos();
        let released = !ctx.input(|input| input.pointer.primary_down());
        // While a panel is dragged over another, that one lights up to show they would merge.
        let merge_into = self.frames.panel_drag.and_then(|drag| {
            let at = ctx.pointer_latest_pos()?;
            self.frames
                .drawn_panels
                .iter()
                .find(|(p, rect)| *p != drag.panel && rect.contains(at))
                .map(|(p, _)| *p)
        });
        let floating_rects: Vec<egui::Rect> = placed
            .iter()
            .filter(|(panel, _)| {
                self.frames
                    .panel_drag
                    .is_none_or(|drag| drag.panel != *panel)
            })
            .map(|(_, rect)| *rect)
            .collect();
        for (panel, rect) in placed {
            let Some(place) = self.layout.place(panel).cloned() else {
                continue;
            };
            let shown = (!self.frames.focus && !place.hidden)
                || self.frames.revealed == Some(panel)
                || (place.pieces.contains(&layout::Piece::Command) && typing && !place.hidden);
            let edge = self.layout.edge_of(panel, rect, window);
            let (tab, upright) = layout::edge_tab(rect, edge, window);
            let hovered = pointer.is_some_and(|at| {
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
                layout::slid(rect, edge, window, visible)
            };
            let id = egui::Id::NULL.with(("panel slide", panel));
            let offset = egui::vec2(
                ctx.animate_value_with_time(id.with(0), target.min.x - rect.min.x, SLIDE_TIME),
                ctx.animate_value_with_time(id.with(1), target.min.y - rect.min.y, SLIDE_TIME),
            );
            let mut drawn = rect.translate(offset);
            let mut size = rect.size();
            // A panel being dragged follows the pointer, and shows where it would land.
            if let (Some(drag), Some(at)) = (self.frames.panel_drag, ctx.pointer_latest_pos())
                && drag.panel == panel
            {
                drawn = egui::Rect::from_min_size(at - drag.grab, drag.size);
                size = drag.size;
                let landing = self.landing(drag, at, drawn, window, merge_into, &floating_rects);
                if merge_into.is_none() {
                    let shown_piece = drag.piece.unwrap_or(panel);
                    if let Some(id) = landing.panel_of(shown_piece)
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
                if released {
                    self.layout = landing;
                    self.frames.panel_drag = None;
                    self.save_layout();
                }
            }
            self.frames.drawn_panels.retain(|(p, _)| *p != panel);
            if shown || hovered || self.frames.panel_drag.is_some_and(|d| d.panel == panel) {
                self.frames.drawn_panels.push((panel, drawn));
            }
            let mut events = Vec::new();
            if !shown {
                events.push(self.edge_tab(ctx, &place, tab, upright));
            }
            if shown || hovered || offset != target.min - rect.min || drawn != target {
                events.push(self.panel_area(ctx, &place, size, drawn, merge_into == Some(panel)));
            }
            for event in events {
                self.panel_event(event, panel, &place, drawn, window);
            }
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
                self.layout = self.layout.resized_from(panel, sides, by, window);
                self.save_layout();
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
            PanelEvent::Close => self.closing = Some(panel),
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
        let frame = panel_frame(glowing).fill(if glass {
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
                    match place.showing() {
                        // Inside a scroll area, so nothing a piece holds grows the panel past its place.
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
                            egui::ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .stick_to_bottom(true)
                                .show(ui, |ui| self.command_line(ui));
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
                    }
                });
                // Added after the contents, so these sit above the panel's own controls.
                event = self.panel_title(ui, place, rect);
                if let Some(resize) = resize_handles(ui, place, rect) {
                    event = resize;
                }
            });
        event
    }

    /// A panel's title row: its name centred, or one tab per piece with the showing one in the
    /// accent. The row is the grip: dragging a tab of a panel with several pulls that piece
    /// out, dragging elsewhere moves the panel. A V at its end hides the panel.
    pub(crate) fn panel_title(
        &self,
        ui: &egui::Ui,
        place: &layout::Place,
        rect: egui::Rect,
    ) -> PanelEvent {
        let mut event = PanelEvent::Nothing;
        let row = egui::Rect::from_min_size(
            rect.min + egui::vec2(PANEL_MARGIN, 4.0),
            egui::vec2(rect.width() - 2.0 * PANEL_MARGIN, TITLE_ROW),
        );
        let font = egui::FontId::proportional(TITLE_FONT);
        let galleys: Vec<_> = place
            .pieces
            .iter()
            .map(|piece| {
                ui.painter().layout_no_wrap(
                    self.text.render(&Message::new(piece.name_key())),
                    font.clone(),
                    theme::current().quiet,
                )
            })
            .collect();
        let spacing = 18.0;
        let total: f32 = galleys.iter().map(|g| g.size().x).sum::<f32>()
            + spacing * (galleys.len().saturating_sub(1)) as f32;
        let mut x = row.center().x - total / 2.0;
        // The row as a whole moves the panel.
        let grip = ui
            .interact(
                row,
                ui.scope_id().with("title"),
                egui::Sense::click_and_drag(),
            )
            .on_hover_cursor(egui::CursorIcon::Grab);
        if grip.drag_started()
            && let Some(at) = grip.interact_pointer_pos()
        {
            event = PanelEvent::DragStarted(at - rect.min, None);
        }
        grip.context_menu(|ui| {
            if ui
                .button(self.text.render(&Message::new("ui.layout.reset")))
                .clicked()
            {
                event = PanelEvent::Reset;
                ui.close();
            }
        });
        let tabs: Vec<egui::Rect> = galleys
            .iter()
            .scan(x, |left, galley| {
                let tab = egui::Rect::from_min_size(
                    egui::pos2(*left, row.min.y),
                    egui::vec2(galley.size().x, TITLE_ROW),
                );
                *left += galley.size().x + spacing;
                Some(tab)
            })
            .collect();
        for (index, galley) in galleys.into_iter().enumerate() {
            let width = galley.size().x;
            let tab = tabs[index];
            let active =
                place.pieces.len() > 1 && index == place.active.min(place.pieces.len() - 1);
            let colour = if active {
                theme::ACCENT
            } else {
                theme::current().quiet
            };
            if place.pieces.len() > 1 {
                let response = ui
                    .interact(
                        tab.expand(4.0),
                        ui.scope_id().with(("tab", index)),
                        egui::Sense::click_and_drag(),
                    )
                    .on_hover_cursor(egui::CursorIcon::Grab);
                let pointer = ui.input(|input| input.pointer.latest_pos());
                let in_row =
                    pointer.is_some_and(|at| row.expand2(egui::vec2(0.0, 10.0)).contains(at));
                if response.clicked() {
                    event = PanelEvent::TabChosen(index);
                } else if response.dragged()
                    && !in_row
                    && let Some(at) = pointer
                {
                    // Dragged out of the row, the piece comes out as a panel of its own.
                    event = PanelEvent::DragStarted(at - rect.min, Some(place.pieces[index]));
                } else if response.drag_stopped()
                    && in_row
                    && let Some(at) = pointer
                {
                    // Let go along the row, the tab moves there.
                    event = PanelEvent::TabMoved(index, tab_slot(&tabs, at.x));
                }
                if active {
                    ui.painter().line_segment(
                        [
                            egui::pos2(tab.min.x, tab.max.y),
                            egui::pos2(tab.max.x, tab.max.y),
                        ],
                        egui::Stroke::new(2.0, theme::ACCENT),
                    );
                }
            }
            ui.painter().galley(
                egui::pos2(tab.min.x, row.center().y - galley.size().y / 2.0),
                galley,
                colour,
            );
            x += width + spacing;
        }
        let _ = x;
        // At the row's end: the hide V, pointing to the edge the panel hides to, then the cross.
        let edge = self.layout.edge_of(place.id(), rect, self.window_rect);
        let close = egui::Rect::from_center_size(
            egui::pos2(row.max.x - 8.0, row.center().y),
            egui::vec2(16.0, TITLE_ROW),
        );
        let hide = close.translate(egui::vec2(-20.0, 0.0));
        let quiet = theme::current().quiet;
        let lit = |r: &egui::Response| {
            egui::Stroke::new(1.5, if r.hovered() { theme::ACCENT } else { quiet })
        };
        let hidden = ui
            .interact(hide, ui.scope_id().with("hide"), egui::Sense::click())
            .on_hover_text(self.text.render(&Message::new("ui.panel.hide")));
        let towards = match edge {
            layout::Edge::Top => egui::vec2(0.0, -1.0),
            layout::Edge::Bottom => egui::vec2(0.0, 1.0),
            layout::Edge::Left => egui::vec2(-1.0, 0.0),
            layout::Edge::Right => egui::vec2(1.0, 0.0),
        };
        paint_chevron(ui.painter(), hide.center(), towards, lit(&hidden));
        if hidden.clicked() {
            event = PanelEvent::Hide;
        }
        let closed = ui
            .interact(close, ui.scope_id().with("close"), egui::Sense::click())
            .on_hover_text(self.text.render(&Message::new("ui.panel.close")));
        let (mid, arm) = (close.center(), 3.5);
        for (a, b) in [((-arm, -arm), (arm, arm)), ((arm, -arm), (-arm, arm))] {
            ui.painter().line_segment(
                [mid + egui::vec2(a.0, a.1), mid + egui::vec2(b.0, b.1)],
                lit(&closed),
            );
        }
        if closed.clicked() {
            event = PanelEvent::Close;
        }
        event
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
        let size = egui::vec2(44.0, 14.0);
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
