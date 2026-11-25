//! A panel's title row: its name, or a tab per piece, which are its grips, and the controls to
//! hide and close it.

use super::frames::PanelEvent;
use crate::*;

/// The command bar's title as a small chip on the middle of its top edge, half above it, with
/// room for its name and, at its right end, the hide and close buttons. Paints the chip and returns
/// its row, which the title's grip, name and buttons then use as any title row.
fn title_chip(
    ui: &egui::Ui,
    place: &layout::Place,
    rect: egui::Rect,
    font: &egui::FontId,
    text: &Translations,
) -> egui::Rect {
    let name = text.render(&Message::new(place.showing().name_key()));
    let width = ui_kit::text_width(ui, &name, font);
    // The name centred, with the buttons' room on both sides so it stays centred.
    ui_kit::title_chip(ui, rect, width, 48.0)
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
    iconic: bool,
) -> Option<PanelEvent> {
    let (row, rect) = frame;
    let mut event = None;
    let tab = tabs[index];
    let active = place.pieces.len() > 1 && index == place.active.min(place.pieces.len() - 1);
    if place.pieces.len() > 1 {
        let response = ui
            .interact(
                tab.expand(4.0),
                ui.scope_id().with(("tab", index)),
                egui::Sense::click_and_drag(),
            )
            .on_hover_cursor(egui::CursorIcon::Grab);
        // A tab shown as its icon says its name on hover.
        let response = if iconic {
            response.on_hover_text(galley.text())
        } else {
            response
        };
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
    }
    if iconic {
        let piece = place.pieces[index];
        ui_kit::title_icon_tab(ui, tab, active, |painter, rect, colour| {
            crate::icons::panels::panel_icon(painter, rect, piece, colour);
        });
    } else {
        ui_kit::title_tab(ui, row, tab, galley, active);
    }
    event
}

/// Room kept at each end of a title row for the hide and close buttons, so the tabs stay centred
/// and never run under them.
const BUTTONS_ROOM: f32 = 48.0;

/// Which tabs show as icons rather than names, given each name's `widths`, the `showing` one,
/// the `icon` width, the `spacing` between tabs and the `room` they have: none when all names
/// fit; else every tab but the one showing; else all of them (owner, 25 November 2026).
pub(crate) fn iconic_tabs(
    widths: &[f32],
    showing: usize,
    icon: f32,
    spacing: f32,
    room: f32,
) -> Vec<bool> {
    let gaps = spacing * widths.len().saturating_sub(1) as f32;
    if widths.len() < 2 || widths.iter().sum::<f32>() + gaps <= room {
        return vec![false; widths.len()];
    }
    let others = icon * (widths.len() - 1) as f32;
    let named = widths.get(showing).copied().unwrap_or(icon);
    let keep_name = named + others + gaps <= room;
    (0..widths.len())
        .map(|k| !(keep_name && k == showing))
        .collect()
}

/// Tabs of `widths` side by side, `spacing` apart, centred along `row`.
fn tab_rects(widths: &[f32], row: egui::Rect, spacing: f32) -> Vec<egui::Rect> {
    let total: f32 = widths.iter().sum::<f32>() + spacing * (widths.len().saturating_sub(1)) as f32;
    widths
        .iter()
        .scan(row.center().x - total / 2.0, |left, width| {
            let tab = egui::Rect::from_min_size(
                egui::pos2(*left, row.min.y),
                egui::vec2(*width, TITLE_ROW),
            );
            *left += width + spacing;
            Some(tab)
        })
        .collect()
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
        let font = ui_kit::title_font();
        let row = if place.is_command_alone() {
            // The command bar's title floats on its top edge as a small chip, so all its height
            // is left for the last lines and the prompt (owner, 22 November 2026).
            title_chip(ui, place, rect, &font, &self.text)
        } else {
            egui::Rect::from_min_size(
                rect.min + egui::vec2(PANEL_MARGIN, 4.0),
                egui::vec2(rect.width() - 2.0 * PANEL_MARGIN, TITLE_ROW),
            )
        };
        // The command tabs alone show their groups here instead of a title (option E), so the
        // grip that moves the panel starts after the groups, leaving them their clicks.
        let tabs_alone = place.is_command_tabs_alone();
        let galleys: Vec<_> = place
            .pieces
            .iter()
            .filter(|_| !tabs_alone)
            .map(|piece| {
                ui_kit::galley(
                    ui,
                    self.text.render(&Message::new(piece.name_key())),
                    font.clone(),
                )
            })
            .collect();
        let grip = if tabs_alone {
            egui::Rect::from_min_max(
                egui::pos2(
                    (row.min.x + self.bar.groups_width + 12.0).min(row.max.x),
                    row.min.y,
                ),
                row.max,
            )
        } else {
            row
        };
        if let Some(grabbed) = self.title_grip(ui, grip, rect) {
            event = grabbed;
        }
        let spacing = 18.0;
        let names: Vec<f32> = galleys.iter().map(|g| g.size().x).collect();
        let showing = place.active.min(place.pieces.len().saturating_sub(1));
        let room = row.width() - 2.0 * BUTTONS_ROOM;
        let iconic = iconic_tabs(&names, showing, TITLE_ROW, spacing, room);
        let widths: Vec<f32> = names
            .iter()
            .zip(&iconic)
            .map(|(name, icon)| if *icon { TITLE_ROW } else { *name })
            .collect();
        let tabs = tab_rects(&widths, row, spacing);
        for (index, galley) in galleys.into_iter().enumerate() {
            let icon = iconic[index];
            if let Some(tabbed) = title_tab(ui, place, (row, rect), &tabs, index, galley, icon) {
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
        let towards = match edge {
            layout::Edge::Top => egui::vec2(0.0, -1.0),
            layout::Edge::Bottom => egui::vec2(0.0, 1.0),
            layout::Edge::Left => egui::vec2(-1.0, 0.0),
            layout::Edge::Right => egui::vec2(1.0, 0.0),
        };
        // Shown only while the pointer is over the panel, so resting panels stay quiet.
        let near = ui
            .ctx()
            .pointer_hover_pos()
            .is_some_and(|at| rect.contains(at));
        let hint = self.text.render(&Message::new("ui.panel.hide"));
        let hidden = ui_kit::mark_button(ui, hide, "hide", hint, near, |painter, at, stroke| {
            ui_kit::chevron(painter, at, towards, stroke);
        });
        if hidden.clicked() {
            event = Some(PanelEvent::Hide);
        }
        let hint = self.text.render(&Message::new("ui.panel.close"));
        if ui_kit::mark_button(ui, close, "close", hint, near, ui_kit::cross).clicked() {
            event = Some(PanelEvent::Close);
        }
        event
    }
}

#[cfg(test)]
mod tests {
    use super::iconic_tabs;

    #[test]
    fn names_show_while_they_fit_then_icons_take_their_place() {
        let names = [60.0, 70.0, 80.0];
        // All three names and the gaps between them fit.
        assert_eq!(iconic_tabs(&names, 1, 20.0, 10.0, 300.0), [false; 3]);
        // They do not: the one showing keeps its name, the others become icons.
        assert_eq!(
            iconic_tabs(&names, 1, 20.0, 10.0, 150.0),
            [true, false, true]
        );
        // Even that does not fit: all icons.
        assert_eq!(iconic_tabs(&names, 2, 20.0, 10.0, 90.0), [true; 3]);
        // A panel of one piece always shows its name.
        assert_eq!(iconic_tabs(&[500.0], 0, 20.0, 10.0, 90.0), [false]);
    }
}
