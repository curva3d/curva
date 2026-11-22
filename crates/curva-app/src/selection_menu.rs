//! Rhino's selection menu (#530): a click where several objects lie under the pointer lists them,
//! nearest first, to choose the one meant. The one highlighted shows selected while the menu is
//! open; Tab moves to the next, Enter or a click takes it, Escape or `None` leaves the selection
//! as it was.

use crate::*;
use curva_document::annotation::Annotation;

/// The menu while it is open.
pub(crate) struct Choosing {
    /// Where the click was, on screen.
    pub(crate) at: egui::Pos2,
    /// The objects under the pointer, nearest first.
    pub(crate) ids: Vec<u64>,
    /// Which one is highlighted.
    pub(crate) highlighted: usize,
    /// The selection before the click, and how the chosen one joins it.
    pub(crate) before: Vec<u64>,
    pub(crate) shift: bool,
    pub(crate) remove: bool,
}

/// The next entry after `index` of `count`, as Tab moves through the menu, going round.
pub(crate) fn next(index: usize, count: usize) -> usize {
    if count == 0 { 0 } else { (index + 1) % count }
}

/// A menu entry's words for an object, as Rhino's menu names kinds.
fn entry_key(shape: &Shape) -> &'static str {
    match shape {
        Shape::Curve(curva_kernel::Curve::Point { .. }) => "ui.menu.point",
        Shape::Curve(_) => "ui.menu.curve",
        Shape::Body(body) if body.is_solid() => "ui.menu.closed_polysurface",
        Shape::Body(body) if body.counts().is_ok_and(|[faces, ..]| faces > 1) => {
            "ui.menu.polysurface"
        }
        Shape::Body(_) => "ui.menu.surface",
        Shape::Mesh(_) => "ui.menu.mesh",
        Shape::SubD(_) => "ui.menu.subd",
        Shape::Annotation(Annotation::Text(_)) => "ui.menu.text",
        Shape::Annotation(_) => "ui.menu.annotation",
        Shape::Instance(_) => "ui.menu.block",
    }
}

impl Curva {
    /// The selection the menu shows while `highlighted` is the one under consideration.
    fn menu_selection(&self, choosing: &Choosing, id: Option<u64>) -> Vec<u64> {
        let hits: Vec<u64> = id.map(|id| self.doc.with_groups(&[id])).unwrap_or_default();
        crate::mouse::combined_selection(
            choosing.before.clone(),
            hits,
            choosing.shift,
            choosing.remove,
        )
    }

    /// Opens the menu for a click at `at` over `ids`, showing the nearest selected.
    pub(crate) fn open_selection_menu(
        &mut self,
        at: egui::Pos2,
        ids: Vec<u64>,
        (shift, remove): (bool, bool),
    ) {
        let choosing = Choosing {
            at,
            ids,
            highlighted: 0,
            before: self.doc.selection(),
            shift,
            remove,
        };
        let shown = self.menu_selection(&choosing, choosing.ids.first().copied());
        self.doc.select(&shown);
        self.mouse.choosing = Some(choosing);
    }

    /// The menu, while open: its entries at the click, Tab, Enter and Escape.
    pub(crate) fn selection_menu(&mut self, ctx: &egui::Context) {
        let Some(mut choosing) = self.mouse.choosing.take() else {
            return;
        };
        let (tab, enter, escape) = ctx.input_mut(|input| {
            (
                input.consume_key(egui::Modifiers::NONE, egui::Key::Tab),
                input.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
                input.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
            )
        });
        if tab {
            choosing.highlighted = next(choosing.highlighted, choosing.ids.len());
        }
        let names: Vec<String> = choosing
            .ids
            .iter()
            .map(|id| {
                self.doc
                    .state()
                    .objects
                    .iter()
                    .find(|o| o.id == *id)
                    .map_or_else(String::new, |o| {
                        self.text.render(&Message::new(entry_key(&o.shape)))
                    })
            })
            .collect();
        let none = self.text.render(&Message::new("ui.menu.none"));
        // The entry clicked: one of the objects, or `None` (past the last).
        let mut chosen: Option<Option<usize>> = None;
        egui::Area::new(egui::Id::NULL.with("selection menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(choosing.at + egui::vec2(12.0, 12.0))
            .show(ctx, |ui| {
                ui_kit::popup_frame(ui).show(ui, |ui| {
                    for (index, name) in names.iter().enumerate() {
                        let entry = ui_kit::choice(ui, index == choosing.highlighted, name);
                        if entry.hovered() {
                            choosing.highlighted = index;
                        }
                        if entry.clicked() {
                            chosen = Some(Some(index));
                        }
                    }
                    ui.separator();
                    if ui_kit::choice(ui, false, none).clicked() {
                        chosen = Some(None);
                    }
                });
            });
        if enter {
            chosen = Some(Some(choosing.highlighted));
        }
        if escape {
            chosen = Some(None);
        }
        match chosen {
            // Taken: the highlighted object joins the selection as the click would have.
            Some(Some(index)) => {
                let picked = self.menu_selection(&choosing, choosing.ids.get(index).copied());
                self.doc.select(&picked);
            }
            // Escape or None: the selection as it was.
            Some(None) => self.doc.select(&choosing.before),
            None => {
                let shown =
                    self.menu_selection(&choosing, choosing.ids.get(choosing.highlighted).copied());
                self.doc.select(&shown);
                self.mouse.choosing = Some(choosing);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn tab_goes_round_the_entries() {
        assert_eq!(super::next(0, 3), 1);
        assert_eq!(super::next(2, 3), 0);
        assert_eq!(super::next(0, 0), 0);
    }
}
