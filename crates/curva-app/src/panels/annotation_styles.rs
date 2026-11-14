//! The Annotation Styles panel (spec `annotation.md` §1; Rhino's Document Properties >
//! Annotation Styles, `docs/commands/rhino-annotation-styles.md`): every style with which one is
//! current, its name, font and text height, and buttons to copy it or delete it if unused.

use crate::*;

/// What a row asks for, done after the list is drawn.
enum StyleAction {
    Current(u64),
    Copy(u64),
    Delete(u64),
    Change(curva_document::annotation::AnnotationStyle),
}

impl Curva {
    /// The Annotation Styles panel's contents.
    pub(crate) fn annotation_styles_panel(&mut self, ui: &mut egui::Ui) {
        let styles = self.doc.annotation_styles();
        let current = self.doc.current_annotation_style();
        let in_use: std::collections::BTreeSet<u64> = self
            .doc
            .state()
            .objects
            .iter()
            .filter_map(|o| match &o.shape {
                Shape::Annotation(curva_document::annotation::Annotation::Text(t)) => Some(t.style),
                _ => None,
            })
            .collect();
        let say = |key: &'static str| self.text.render(&Message::new(key));
        let mut action = None;
        for style in &styles {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    if ui
                        .radio(style.id == current, "")
                        .on_hover_text(say("ui.annotationstyles.current"))
                        .clicked()
                    {
                        action = Some(StyleAction::Current(style.id));
                    }
                    let mut name = style.name.clone();
                    if ui
                        .add(egui::TextEdit::singleline(&mut name).desired_width(140.0))
                        .changed()
                        && !name.trim().is_empty()
                    {
                        action = Some(StyleAction::Change(
                            curva_document::annotation::AnnotationStyle {
                                name,
                                ..style.clone()
                            },
                        ));
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(say("ui.annotationstyles.height"));
                    let mut height = style.text_height;
                    let unit = if style.imperial { " in" } else { " mm" };
                    if ui
                        .add(
                            egui::DragValue::new(&mut height)
                                .range(1e-4..=1e4)
                                .speed(0.05)
                                .suffix(unit),
                        )
                        .changed()
                    {
                        action = Some(StyleAction::Change(
                            curva_document::annotation::AnnotationStyle {
                                text_height: height,
                                ..style.clone()
                            },
                        ));
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(say("ui.annotationstyles.font"));
                    let mut font = style.font.clone().unwrap_or_default();
                    if ui
                        .add(egui::TextEdit::singleline(&mut font).desired_width(110.0))
                        .lost_focus()
                    {
                        let font = font.trim();
                        let font = (!font.is_empty()).then(|| font.to_owned());
                        if font != style.font {
                            action = Some(StyleAction::Change(
                                curva_document::annotation::AnnotationStyle {
                                    font,
                                    ..style.clone()
                                },
                            ));
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button(say("ui.annotationstyles.copy")).clicked() {
                        action = Some(StyleAction::Copy(style.id));
                    }
                    let used = in_use.contains(&style.id);
                    let delete = ui.add_enabled(
                        !used && styles.len() > 1,
                        egui::Button::new(say("ui.annotationstyles.delete")),
                    );
                    if used {
                        delete.on_disabled_hover_text(say("ui.annotationstyles.in_use"));
                    } else if delete.clicked() {
                        action = Some(StyleAction::Delete(style.id));
                    }
                });
            });
        }
        match action {
            Some(StyleAction::Current(id)) => {
                self.undoable(|doc| doc.set_current_annotation_style(id));
            }
            Some(StyleAction::Copy(id)) => {
                let base = self.doc.annotation_style(id).name;
                let mut n = 2;
                while styles.iter().any(|s| s.name == format!("{base} {n}")) {
                    n += 1;
                }
                let name = format!("{base} {n}");
                self.undoable(|doc| {
                    doc.copy_annotation_style(id, name);
                });
            }
            Some(StyleAction::Delete(id)) => {
                self.undoable(|doc| {
                    doc.delete_annotation_style(id);
                });
            }
            Some(StyleAction::Change(changed)) => {
                let id = changed.id;
                self.undoable(|doc| doc.change_annotation_style(id, |s| *s = changed));
            }
            None => {}
        }
    }
}
