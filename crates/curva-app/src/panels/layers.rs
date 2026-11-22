//! The Layers panel, and the linetype and print width choosers it shares with Properties.

use crate::*;

/// The width of the Layers panel's column for the entered detail.
pub(super) const DETAIL_COLUMN: f32 = 48.0;

/// How far each level of sublayers is indented in the Layers panel.
pub(super) const INDENT: f32 = 10.0;

/// The width of a layer's expand triangle.
pub(super) const ARROW: f32 = 12.0;

/// The width of a layer's drag grip.
pub(super) const GRIP: f32 = 7.0;

/// The layers as the panel lists them (spec `sublayers.md` §2): each top-level layer followed
/// by its sublayers, depth first, those under a collapsed layer left out; with each its depth
/// and whether it has sublayers.
pub(super) fn tree_rows(
    layers: &[curva_document::Layer],
) -> Vec<(&curva_document::Layer, usize, bool)> {
    let known: std::collections::HashSet<u64> = layers.iter().map(|l| l.id).collect();
    // Each layer's sublayers, in the list's order, found once rather than for every layer.
    let mut children_of: std::collections::HashMap<u64, Vec<&curva_document::Layer>> =
        std::collections::HashMap::new();
    for layer in layers {
        if let Some(parent) = layer.parent.filter(|p| known.contains(p)) {
            children_of.entry(parent).or_default().push(layer);
        }
    }
    let mut rows = Vec::new();
    // Layers still to list, the next last; a layer whose parent is missing is top-level.
    let mut stack: Vec<(&curva_document::Layer, usize)> = layers
        .iter()
        .filter(|l| l.parent.is_none_or(|p| !known.contains(&p)))
        .rev()
        .map(|l| (l, 0))
        .collect();
    let mut listed = std::collections::HashSet::new();
    while let Some((layer, depth)) = stack.pop() {
        if !listed.insert(layer.id) {
            continue;
        }
        let children = children_of.remove(&layer.id).unwrap_or_default();
        rows.push((layer, depth, !children.is_empty()));
        if !layer.collapsed {
            stack.extend(children.into_iter().rev().map(|l| (l, depth + 1)));
        }
    }
    rows
}

impl Curva {
    /// Runs an edit of the model from a panel as one undo step, if it changed anything.
    pub(crate) fn undoable(&mut self, edit: impl FnOnce(&mut Document)) {
        let before = self.doc.state().clone();
        edit(&mut self.doc);
        if self.doc.state() != &before {
            self.doc.record_change(before);
        }
    }

    /// A drop-down of linetypes, with `ByLayer` when `by_layer`; returns the one chosen.
    pub(crate) fn linetype_choice(
        &self,
        ui: &mut egui::Ui,
        id: egui::Id,
        shown: Option<curva_document::Linetype>,
        by_layer: bool,
        width: f32,
    ) -> Option<curva_document::Linetype> {
        use curva_document::Linetype;
        let state = self.doc.state();
        let mut chosen = None;
        let label = shown.map_or_else(
            || self.text.render(&Message::new("ui.width.varies")),
            |l| state.linetype_name(l),
        );
        egui::ComboBox::from_id_salt(id)
            .selected_text(label)
            .width(width)
            .show_ui(ui, |ui| {
                let choices = by_layer
                    .then_some(Linetype::ByLayer)
                    .into_iter()
                    .chain(state.linetypes_offered());
                for choice in choices {
                    if ui
                        .selectable_label(shown == Some(choice), state.linetype_name(choice))
                        .clicked()
                    {
                        chosen = Some(choice);
                    }
                }
            });
        chosen
    }

    /// A drop-down of print widths (spec `line-weights.md` §5): `By layer` when `by_layer`,
    /// `Default`, Rhino's common widths, and a field for any other. `shown` is what it shows;
    /// returns the width chosen, if one was.
    pub(crate) fn width_choice(
        &self,
        ui: &mut egui::Ui,
        id: egui::Id,
        shown: Option<curva_document::PrintWidth>,
        by_layer: bool,
        width: f32,
    ) -> Option<curva_document::PrintWidth> {
        use curva_document::PrintWidth;
        let text = |key: &'static str| self.text.render(&Message::new(key));
        let named = |width: PrintWidth| match width {
            PrintWidth::ByLayer => text("ui.width.by_layer"),
            PrintWidth::Default => text("ui.width.default"),
            PrintWidth::Millimetres(mm) => self
                .text
                .render(&Message::with("ui.width.mm", vec![format!("{mm:.2}")])),
        };
        let mut chosen = None;
        egui::ComboBox::from_id_salt(id)
            .selected_text(shown.map_or_else(|| text("ui.width.varies"), named))
            .width(width)
            .show_ui(ui, |ui| {
                let mut choices = Vec::new();
                if by_layer {
                    choices.push(PrintWidth::ByLayer);
                }
                choices.push(PrintWidth::Default);
                choices.extend(PrintWidth::COMMON.map(PrintWidth::Millimetres));
                for choice in choices {
                    if ui
                        .selectable_label(shown == Some(choice), named(choice))
                        .clicked()
                    {
                        chosen = Some(choice);
                    }
                }
                ui.horizontal(|ui| {
                    ui.label(text("ui.width.custom"));
                    let mut mm = match shown {
                        Some(PrintWidth::Millimetres(mm)) => mm,
                        _ => 0.25,
                    };
                    let field = ui.add(
                        egui::DragValue::new(&mut mm)
                            .range(0.01..=PrintWidth::MOST)
                            .speed(0.01)
                            .fixed_decimals(2)
                            .suffix(" mm"),
                    );
                    if field.changed() {
                        chosen = Some(PrintWidth::Millimetres(mm));
                    }
                });
            });
        chosen
    }

    /// The Layers panel: Rhino's toolbar and search on top, then the table of layers.
    pub(crate) fn layers_panel(&mut self, ui: &mut egui::Ui) {
        let layers = self.doc.layers().to_vec();
        let current = self.doc.current_layer();
        self.layer_toolbar(ui);
        // The panel scrolls both ways (`panel_area`), so wide columns can be reached.
        self.layer_table(ui, &layers, current);
    }
}

#[cfg(test)]
mod tests {
    use super::tree_rows;
    use curva_document::Document;

    #[test]
    fn the_layer_tree_lists_each_layer_under_its_parent_depth_first() {
        let mut doc = Document::default();
        let walls = doc.add_layer("Walls".to_owned());
        let doors = doc.add_sublayer(walls, "Doors".to_owned());
        let frames = doc.add_sublayer(doors, "Frames".to_owned());
        let roof = doc.add_layer("Roof".to_owned());
        let tiles = doc.add_sublayer(roof, "Tiles".to_owned());
        let windows = doc.add_sublayer(walls, "Windows".to_owned());
        doc.set_layer_collapsed(roof, true);
        let mut layers = doc.layers().to_vec();
        // A layer whose parent is missing is listed at the top level.
        let mut stray = layers[layers.len() - 1].clone();
        stray.id = 999;
        stray.name = "Stray".to_owned();
        stray.parent = Some(12_345);
        layers.push(stray);
        let rows: Vec<(u64, usize, bool)> = tree_rows(&layers)
            .into_iter()
            .map(|(l, depth, has)| (l.id, depth, has))
            .collect();
        let first = layers[0].id;
        assert_eq!(
            rows,
            [
                (first, 0, false),
                (walls, 0, true),
                (doors, 1, true),
                (frames, 2, false),
                (windows, 1, false),
                // Collapsed: its sublayer is not listed, but it still shows it has one.
                (roof, 0, true),
                (999, 0, false),
            ]
        );
        assert!(!rows.iter().any(|(id, ..)| *id == tiles));
    }
}
