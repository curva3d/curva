//! A panel's title row: its name, or a tab per piece, which are its grips, and the controls to
//! hide and close it.

use super::frames::PanelEvent;
use crate::*;

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

/// The title of piece `index` at `tabs[index]`, in the title row of the panel at `frame`'s
/// rectangle (`frame` is the row, then the panel). On a panel with several pieces it is a tab:
/// clicked, it shows its piece; dragged out of the row, the piece comes out as a panel of its
/// own; let go along the row, the tab moves there.
fn title_tab(
    ui: &egui::Ui,
    place: &layout::Place,
    frame: (egui::Rect, egui::Rect),
    tabs: &[egui::Rect],
    index: usize,
    galley: std::sync::Arc<egui::Galley>,
) -> Option<PanelEvent> {
    let (row, rect) = frame;
    let mut event = None;
    let tab = tabs[index];
    let active = place.pieces.len() > 1 && index == place.active.min(place.pieces.len() - 1);
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
        let in_row = pointer.is_some_and(|at| row.expand2(egui::vec2(0.0, 10.0)).contains(at));
        if response.clicked() {
            event = Some(PanelEvent::TabChosen(index));
        } else if response.dragged()
            && !in_row
            && let Some(at) = pointer
        {
            event = Some(PanelEvent::DragStarted(
                at - rect.min,
                Some(place.pieces[index]),
            ));
        } else if response.drag_stopped()
            && in_row
            && let Some(at) = pointer
        {
            event = Some(PanelEvent::TabMoved(index, tab_slot(tabs, at.x)));
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
    event
}

/// Where along a row of tabs a tab let go at `x` lands, as an index among them.
pub(crate) fn tab_slot(tabs: &[egui::Rect], x: f32) -> usize {
    tabs.iter()
        .position(|tab| x < tab.center().x)
        .unwrap_or(tabs.len().saturating_sub(1))
}

impl Curva {
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
        if let Some(grabbed) = self.title_grip(ui, row, rect) {
            event = grabbed;
        }
        let spacing = 18.0;
        let total: f32 = galleys.iter().map(|g| g.size().x).sum::<f32>()
            + spacing * (galleys.len().saturating_sub(1)) as f32;
        let tabs: Vec<egui::Rect> = galleys
            .iter()
            .scan(row.center().x - total / 2.0, |left, galley| {
                let tab = egui::Rect::from_min_size(
                    egui::pos2(*left, row.min.y),
                    egui::vec2(galley.size().x, TITLE_ROW),
                );
                *left += galley.size().x + spacing;
                Some(tab)
            })
            .collect();
        for (index, galley) in galleys.into_iter().enumerate() {
            if let Some(tabbed) = title_tab(ui, place, (row, rect), &tabs, index, galley) {
                event = tabbed;
            }
        }
        if let Some(button) = self.title_buttons(ui, place, row, rect) {
            event = button;
        }
        event
    }

    /// The title row as a whole, which moves the panel, and its menu with Reset layout.
    fn title_grip(&self, ui: &egui::Ui, row: egui::Rect, rect: egui::Rect) -> Option<PanelEvent> {
        let mut event = None;
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
            event = Some(PanelEvent::DragStarted(at - rect.min, None));
        }
        grip.context_menu(|ui| {
            if ui
                .button(self.text.render(&Message::new("ui.layout.reset")))
                .clicked()
            {
                event = Some(PanelEvent::Reset);
                ui.close();
            }
        });
        event
    }

    /// At the title row's end: the hide V, pointing to the edge the panel hides to, then the
    /// cross that closes it.
    fn title_buttons(
        &self,
        ui: &egui::Ui,
        place: &layout::Place,
        row: egui::Rect,
        rect: egui::Rect,
    ) -> Option<PanelEvent> {
        let mut event = None;
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
            event = Some(PanelEvent::Hide);
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
            event = Some(PanelEvent::Close);
        }
        event
    }
}
