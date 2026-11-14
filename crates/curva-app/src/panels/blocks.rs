//! The Blocks panel, and reading linked blocks again.

use crate::*;

impl Curva {
    /// The Blocks panel (spec `docs/specs/blocks.md` §3): each definition with its instance
    /// count and, if linked, its file; buttons to insert, select, update, embed and delete.
    pub(crate) fn blocks_panel(&mut self, ui: &mut egui::Ui) {
        let definitions = self.doc.definitions().to_vec();
        if definitions.is_empty() {
            ui.label(self.text.render(&Message::new("ui.blocks.none")));
            return;
        }
        let mut run: Option<(String, String)> = None;
        for definition in &definitions {
            let count = self.doc.instances_of(definition.id).len();
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    let mut name = definition.name.clone();
                    let edit = ui.add(egui::TextEdit::singleline(&mut name).desired_width(120.0));
                    if edit.lost_focus()
                        && name.trim() != definition.name
                        && !name.trim().is_empty()
                    {
                        let id = definition.id;
                        let name = name.trim().to_owned();
                        self.undoable(|doc| doc.rename_definition(id, name));
                    }
                    ui.label(
                        self.text
                            .render(&Message::with("ui.blocks.count", vec![count.to_string()])),
                    );
                });
                if let Some(path) = &definition.linked {
                    ui.label(
                        self.text
                            .render(&Message::with("ui.blocks.linked", vec![path.clone()])),
                    );
                }
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button(self.text.render(&Message::new("ui.blocks.insert")))
                        .clicked()
                    {
                        run = Some(("insert".to_owned(), definition.name.clone()));
                    }
                    if ui
                        .button(self.text.render(&Message::new("ui.blocks.select")))
                        .clicked()
                    {
                        run = Some(("selblockinstancenamed".to_owned(), definition.name.clone()));
                    }
                    if definition.linked.is_some() {
                        if ui
                            .button(self.text.render(&Message::new("ui.blocks.update")))
                            .clicked()
                        {
                            self.reload_linked(true);
                        }
                        if ui
                            .button(self.text.render(&Message::new("ui.blocks.embed")))
                            .clicked()
                        {
                            let id = definition.id;
                            self.undoable(|doc| doc.embed_definition(id));
                        }
                    }
                    let unused = count == 0;
                    let delete = ui.add_enabled(
                        unused,
                        egui::Button::new(self.text.render(&Message::new("ui.blocks.delete"))),
                    );
                    if delete.clicked() {
                        let id = definition.id;
                        self.undoable(|doc| {
                            doc.delete_definition(id);
                        });
                    }
                });
            });
        }
        if let Some((command, name)) = run {
            self.run_from_button(&command);
            self.processor.submit(&mut self.doc, &name);
        }
    }

    /// Reads linked block definitions again from their files: those whose file changed since it
    /// was last read, or all with `force`.
    pub(crate) fn reload_linked(&mut self, force: bool) {
        let linked: Vec<(u64, String)> = self
            .doc
            .definitions()
            .iter()
            .filter_map(|d| d.linked.clone().map(|path| (d.id, path)))
            .collect();
        for (id, path) in linked {
            let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
            let seen = self.linked_seen.get(&path).copied();
            if !force && modified.is_some() && seen == modified {
                continue;
            }
            match Document::open(std::path::Path::new(&path)) {
                Ok(opened) => {
                    self.doc.set_definition_objects(
                        id,
                        opened
                            .state()
                            .objects
                            .iter()
                            .map(|o| curva_document::Object::clone(o))
                            .collect(),
                    );
                    if let Some(when) = modified {
                        self.linked_seen.insert(path.clone(), when);
                    }
                }
                Err(error) => self.processor.note(format!("{path}: {}", error.0)),
            }
        }
    }
}
