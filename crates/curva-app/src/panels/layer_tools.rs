//! The Layers panel's toolbar, search and filters, as Rhino 8 lays them out (owner, 22 November
//! 2026): a row of small buttons along the top (New layer, New sublayer, Delete, Change object
//! layer, Copy objects to layer, Move up, Move down, Move up one parent, Filters, Tools), then a
//! search box that lists only the layers whose names match, `*` and `?` as wildcards. The icons
//! are drawn in Curva's own line style.

use crate::*;

/// What the Layers panel remembers between frames.
#[derive(Default)]
pub(crate) struct LayersUi {
    /// The layer picked in the list, which the toolbar acts on (the current layer when none).
    pub(crate) selected: Option<u64>,
    /// What the search box holds.
    pub(crate) search: String,
    /// Which layers the Filters menu hides.
    pub(crate) filter: LayerFilter,
}

/// Rhino's layer filters, each kind of layer shown or hidden. All shown at first.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct LayerFilter {
    pub(crate) on: bool,
    pub(crate) off: bool,
    pub(crate) locked: bool,
    pub(crate) unlocked: bool,
    pub(crate) with_objects: bool,
    pub(crate) empty: bool,
}

impl Default for LayerFilter {
    fn default() -> Self {
        Self {
            on: true,
            off: true,
            locked: true,
            unlocked: true,
            with_objects: true,
            empty: true,
        }
    }
}

impl LayerFilter {
    /// Whether a layer passes: shown if its kind is shown on each count.
    pub(crate) fn passes(self, visible: bool, locked: bool, has_objects: bool) -> bool {
        (if visible { self.on } else { self.off })
            && (if locked { self.locked } else { self.unlocked })
            && (if has_objects {
                self.with_objects
            } else {
                self.empty
            })
    }
}

/// Whether `name` matches the search `pattern`, ignoring case: a plain word anywhere in the
/// name, or with `*` (any run of characters) and `?` (any one) the whole name, as Rhino's search.
pub(crate) fn name_matches(name: &str, pattern: &str) -> bool {
    let pattern = pattern.trim().to_lowercase();
    if pattern.is_empty() {
        return true;
    }
    let name = name.to_lowercase();
    if !pattern.contains(['*', '?']) {
        return name.contains(&pattern);
    }
    let (p, n): (Vec<char>, Vec<char>) = (pattern.chars().collect(), name.chars().collect());
    // Matching with a table: whether the first i pattern characters match the first j name ones.
    let mut row = vec![false; n.len() + 1];
    row[0] = true;
    for &c in &p {
        let mut next = vec![false; n.len() + 1];
        next[0] = c == '*' && row[0];
        for j in 1..=n.len() {
            next[j] = match c {
                '*' => next[j - 1] || row[j],
                '?' => row[j - 1],
                _ => row[j - 1] && c == n[j - 1],
            };
        }
        row = next;
    }
    row[n.len()]
}

/// The toolbar's buttons.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    NewLayer,
    NewSublayer,
    Delete,
    ChangeObjectLayer,
    CopyToLayer,
    MoveUp,
    MoveDown,
    UpOneParent,
}

/// The buttons in Rhino's order, with the words shown on hover.
const TOOLS: [(Tool, &str); 8] = [
    (Tool::NewLayer, "ui.layer.new"),
    (Tool::NewSublayer, "ui.layer.new_sublayer"),
    (Tool::Delete, "ui.layer.delete"),
    (Tool::ChangeObjectLayer, "ui.layer.change_object_layer"),
    (Tool::CopyToLayer, "ui.layer.copy_to_layer"),
    (Tool::MoveUp, "ui.layer.move_up"),
    (Tool::MoveDown, "ui.layer.move_down"),
    (Tool::UpOneParent, "ui.layer.up_one_parent"),
];

impl Tool {
    /// The icon the button shows.
    fn icon(self) -> &'static str {
        match self {
            Tool::NewLayer => "layer.new",
            Tool::NewSublayer => "layer.new_sublayer",
            Tool::Delete => "layer.delete",
            Tool::ChangeObjectLayer => "layer.change_object_layer",
            Tool::CopyToLayer => "layer.copy_to_layer",
            Tool::MoveUp => "layer.move_up",
            Tool::MoveDown => "layer.move_down",
            Tool::UpOneParent => "layer.up_one_parent",
        }
    }
}

impl Curva {
    /// The toolbar and the search box at the top of the Layers panel.
    pub(crate) fn layer_toolbar(&mut self, ui: &mut egui::Ui) {
        // The words first, as the menus change the model while they are drawn.
        let say = |key: &'static str| self.text.render(&Message::new(key));
        let words: Vec<String> = TOOLS.iter().map(|(_, key)| say(key)).collect();
        let (filters_word, tools_word, search_word) = (
            say("ui.layer.filters"),
            say("ui.layer.tools"),
            say("ui.layer.search"),
        );
        let target = self
            .frames
            .layers_ui
            .selected
            .filter(|id| self.doc.layers().iter().any(|l| l.id == *id))
            .unwrap_or_else(|| self.doc.current_layer());
        let mut clicked = None;
        ui.horizontal(|ui| {
            ui_kit::with_gap(ui, ui_kit::TIGHT_GAP, |ui| {
                let size = ui_kit::TOOL_BUTTON;
                for ((tool, _), word) in TOOLS.into_iter().zip(words) {
                    if ui_kit::icon_button(ui, tool.icon(), size, &word).clicked() {
                        clicked = Some(tool);
                    }
                    // A little room between Rhino's groups of buttons.
                    if matches!(tool, Tool::Delete | Tool::CopyToLayer | Tool::UpOneParent) {
                        ui.add_space(ui_kit::CHIP_GAP);
                    }
                }
                let filters = ui_kit::icon_button(ui, "layer.filters", size, &filters_word);
                egui::Popup::menu(&filters).show(|ui| self.layer_filter_menu(ui));
                let tools = ui_kit::icon_button(ui, "layer.tools", size, &tools_word);
                egui::Popup::menu(&tools).show(|ui| self.layer_tools_menu(ui, target));
            });
        });
        if let Some(tool) = clicked {
            self.run_layer_tool(tool, target);
        }
        ui.add(
            egui::TextEdit::singleline(&mut self.frames.layers_ui.search)
                .hint_text(search_word)
                .desired_width(f32::INFINITY),
        );
    }

    /// What a toolbar button does to the layer picked in the list (else the current one).
    fn run_layer_tool(&mut self, tool: Tool, target: u64) {
        let number = self.doc.layers().iter().map(|l| l.id).max().unwrap_or(0) + 1;
        let name = self.text.render(&Message::with(
            "ui.layer.new_name",
            vec![number.to_string()],
        ));
        let selection = self.doc.selection();
        match tool {
            Tool::NewLayer => {
                let mut made = 0;
                self.undoable(|doc| made = doc.add_layer(name));
                self.frames.layers_ui.selected = Some(made);
            }
            Tool::NewSublayer => {
                let mut made = 0;
                self.undoable(|doc| {
                    made = doc.add_sublayer(target, name);
                    doc.set_layer_collapsed(target, false);
                });
                self.frames.layers_ui.selected = Some(made);
            }
            Tool::Delete => {
                let mut deleted = false;
                self.undoable(|doc| deleted = doc.delete_layer(target));
                if !deleted {
                    let said = self.text.render(&Message::new("ui.layer.cannot_delete"));
                    self.processor.note(said);
                }
            }
            Tool::ChangeObjectLayer => {
                self.undoable(|doc| doc.move_to_layer(&selection, target));
            }
            Tool::CopyToLayer => {
                let mut copies = Vec::new();
                self.undoable(|doc| copies = doc.copy_to_layer(&selection, target));
                self.doc.select(&copies);
            }
            Tool::MoveUp | Tool::MoveDown => {
                let up = tool == Tool::MoveUp;
                self.undoable(|doc| {
                    doc.move_layer(target, up);
                });
            }
            Tool::UpOneParent => {
                let state = self.doc.state();
                if let Some(parent) = state.layer(target).and_then(|l| l.parent) {
                    let above = state.layer(parent).and_then(|p| p.parent);
                    self.undoable(|doc| {
                        doc.set_layer_parent(target, above);
                    });
                }
            }
        }
    }

    /// Rhino's layer filters: each kind of layer shown or hidden, and All layers to show all.
    fn layer_filter_menu(&mut self, ui: &mut egui::Ui) {
        let text = &self.text;
        let say = |key: &'static str| text.render(&Message::new(key));
        let filter = &mut self.frames.layers_ui.filter;
        if ui.button(say("ui.layer.filter_all")).clicked() {
            *filter = LayerFilter::default();
        }
        ui.separator();
        ui_kit::toggle(ui, &mut filter.on, say("ui.layer.filter_on"));
        ui_kit::toggle(ui, &mut filter.off, say("ui.layer.filter_off"));
        ui_kit::toggle(ui, &mut filter.locked, say("ui.layer.filter_locked"));
        ui_kit::toggle(ui, &mut filter.unlocked, say("ui.layer.filter_unlocked"));
        ui_kit::toggle(
            ui,
            &mut filter.with_objects,
            say("ui.layer.filter_with_objects"),
        );
        ui_kit::toggle(ui, &mut filter.empty, say("ui.layer.filter_empty"));
    }

    /// Rhino's Tools menu, the parts Curva has: picking the selected objects' layer, selecting
    /// objects on the layer, turning every layer on, and opening or closing all sublayers.
    fn layer_tools_menu(&mut self, ui: &mut egui::Ui, target: u64) {
        // The words first, as the items change the model.
        let words = [
            "ui.layer.select_object_layer",
            "ui.layer.select_objects",
            "ui.layer.select_sublayer_objects",
            "ui.layer.all_on",
            "ui.layer.expand_all",
            "ui.layer.collapse_all",
        ]
        .map(|key| (key, self.text.render(&Message::new(key))));
        let say = |key: &str| {
            words
                .iter()
                .find(|(k, _)| *k == key)
                .map_or_else(String::new, |(_, w)| w.clone())
        };
        if ui.button(say("ui.layer.select_object_layer")).clicked() {
            let first = self.doc.selection().first().copied();
            self.frames.layers_ui.selected = first.and_then(|id| self.doc.layer_of(id));
        }
        if ui.button(say("ui.layer.select_objects")).clicked() {
            let picked = self.doc.objects_on_layer(target, false);
            self.doc.select(&picked);
        }
        if ui.button(say("ui.layer.select_sublayer_objects")).clicked() {
            let picked = self.doc.objects_on_layer(target, true);
            self.doc.select(&picked);
        }
        ui.separator();
        if ui.button(say("ui.layer.all_on")).clicked() {
            let ids: Vec<u64> = self.doc.layers().iter().map(|l| l.id).collect();
            self.undoable(|doc| {
                for id in ids {
                    doc.set_layer_visible(id, true);
                }
            });
        }
        for (key, collapsed) in [
            ("ui.layer.expand_all", false),
            ("ui.layer.collapse_all", true),
        ] {
            if ui.button(say(key)).clicked() {
                let ids: Vec<u64> = self.doc.layers().iter().map(|l| l.id).collect();
                for id in ids {
                    self.doc.set_layer_collapsed(id, collapsed);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_finds_words_and_wildcards_as_rhino() {
        assert!(name_matches("Curtain walls", "wall"));
        assert!(name_matches("Curtain walls", "WALL"));
        assert!(!name_matches("Doors", "wall"));
        assert!(name_matches("Slabs-Structure", "slabs*"));
        assert!(
            !name_matches("Roof slabs", "slabs*"),
            "a wildcard pattern is the whole name"
        );
        assert!(name_matches("Door1", "door?"));
        assert!(!name_matches("Door12", "door?"));
        assert!(name_matches("anything", "  "));
    }

    #[test]
    fn filters_hide_each_kind_of_layer() {
        let all = LayerFilter::default();
        assert!(all.passes(false, true, false));
        let only_on = LayerFilter { off: false, ..all };
        assert!(only_on.passes(true, false, true));
        assert!(!only_on.passes(false, false, true));
        let with_objects = LayerFilter {
            empty: false,
            ..all
        };
        assert!(!with_objects.passes(true, false, false));
    }
}
