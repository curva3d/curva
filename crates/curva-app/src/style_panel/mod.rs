//! The Style panel (spec `docs/specs/views-and-styles.md` §3, after the owner's approved
//! mock-up): the active view's style, chosen from the built-in styles, the model's own and "My
//! styles", and edited on one simple page: how surfaces are drawn, then three tabs (Look,
//! Objects, Plans and sections) whose sections show a few settings each and keep the rest under
//! More. Every change shows at once in each view drawing with the style, while a slider is still
//! being dragged. A built-in style is never changed: the first edit copies it into the model
//! under a new name. The model's styles are not part of the model's undo (owner's decision C),
//! so the panel keeps its own.

mod drawings;
mod fields;
mod look;
mod objects;

use crate::*;
use curva_document::style::{BUILT_IN, EXTENSION, LibraryStyle, ViewStyle, built_in_name};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

/// How long changes to one style count as one step of the panel's undo, in seconds, so a
/// slider's drag is undone at once.
const MERGE_SECONDS: f64 = 0.8;

/// The tallest the list of styles grows before it scrolls, so the editing page shows below it.
const LIST_HEIGHT: f32 = 170.0;

/// The most steps the panel's undo keeps.
const UNDO_STEPS: usize = 100;

/// The Style panel's tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum StyleTab {
    /// Background, light, lines and the rest of the view's look.
    #[default]
    Look,
    /// Each kind of object.
    Objects,
    /// Live plans, sections and elevations.
    Drawings,
}

/// The kinds of object the Objects tab sets apart.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ObjectKind {
    /// Surfaces, polysurfaces and solids.
    #[default]
    Nurbs,
    /// Meshes.
    Meshes,
    /// SubD objects.
    Subd,
    /// Curves.
    Curves,
    /// Points and point clouds.
    Points,
    /// Text and dimensions.
    Text,
    /// Lights and clipping planes.
    Other,
}

/// What a file dialog the panel opened is for.
enum FileJob {
    /// Reading a style file into the model.
    Import,
    /// Writing this style to a file.
    Export(Box<ViewStyle>),
}

/// What the panel's undo steps back to: the model's styles, and the style each viewport drew
/// with by name.
type Snapshot = (Vec<ViewStyle>, Vec<Option<String>>);

/// The Style panel's own state.
#[derive(Default)]
pub(crate) struct StylePanel {
    tab: StyleTab,
    kind: ObjectKind,
    /// The sections showing their More settings, by their title's key.
    open: BTreeSet<&'static str>,
    /// How things stood before each change, newest last.
    undo: Vec<Snapshot>,
    /// When the last edit was made and to which style, so a slider's drag is one undo step.
    last_edit: Option<(f64, String)>,
    /// The new name being typed, while renaming.
    renaming: Option<String>,
    /// What the panel last did, or why it could not.
    notice: Option<String>,
    /// "My styles", read when the panel first shows and after saving or deleting one.
    library: Option<Vec<LibraryStyle>>,
    /// A file dialog open for importing or exporting.
    file: Option<(FileJob, Receiver<Option<PathBuf>>)>,
    /// A style of My styles waiting for its deletion to be confirmed.
    deleting: Option<String>,
}

impl StylePanel {
    /// Shows `tab`, as the pictures of the interface do.
    pub(crate) fn show_tab(&mut self, tab: StyleTab) {
        self.tab = tab;
    }
}

/// What the list or the buttons asked for, done once they are drawn.
enum StyleAction {
    /// Draw the active view with the model's or a built-in style.
    Use(String),
    /// Copy a style of My styles into the model and draw with it.
    UseMine(Box<ViewStyle>),
    New,
    Duplicate,
    StartRename,
    Rename(String),
    Delete,
    DeleteMine,
    SaveMine,
    Import,
    Export,
    Undo,
}

/// A style's background as the two colours of its swatch, top and bottom.
fn swatch(style: &ViewStyle) -> ([u8; 3], [u8; 3]) {
    let (top, bottom) = crate::styles::style_background(style);
    (
        [top.r(), top.g(), top.b()],
        [bottom.r(), bottom.g(), bottom.b()],
    )
}

/// Whether two style names are the same: names ignore case and spaces around them.
fn same_name(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

impl Curva {
    /// The Style panel's contents.
    pub(crate) fn style_panel(&mut self, ui: &mut egui::Ui) {
        self.serve_style_file();
        if self.settings_ui.styles.library.is_none() {
            self.read_library();
        }
        let mut action = None;
        ui_kit::scroll(ui, ui_kit::Scroll::Down, |ui| {
            action = ui_kit::scroll_list(ui, LIST_HEIGHT, |ui| self.style_list(ui));
            if let Some(chosen) = self.style_buttons(ui) {
                action = Some(chosen);
            }
            if let Some(notice) = &self.settings_ui.styles.notice {
                ui.label(ui_kit::caption(notice.clone()));
            }
            ui.separator();
            self.style_editor(ui);
        });
        if let Some(action) = action {
            self.apply_style_action(action);
        }
        self.confirm_mine_deleted(ui.ctx());
    }

    /// The words for `key`.
    fn style_say(&self, key: &'static str) -> String {
        self.text.render(&Message::new(key))
    }

    /// The styles to choose from: built-in, the model's own, and My styles, the active view's
    /// marked.
    fn style_list(&self, ui: &mut egui::Ui) -> Option<StyleAction> {
        let current = self.panes[self.active].style_name();
        let mut action = None;
        ui_kit::section(ui, self.style_say("ui.style.built_in"));
        for name in BUILT_IN {
            let Some(style) = ViewStyle::built_in(name) else {
                continue;
            };
            if ui_kit::swatch_item(ui, same_name(name, &current), swatch(&style), name).clicked() {
                action = Some(StyleAction::Use(name.to_owned()));
            }
        }
        ui_kit::section(ui, self.style_say("ui.style.model"));
        if self.doc.styles().is_empty() {
            ui.label(ui_kit::caption(self.style_say("ui.style.model_none")));
        }
        for style in self.doc.styles() {
            let chosen = same_name(&style.name, &current);
            if ui_kit::swatch_item(ui, chosen, swatch(style), style.name.clone()).clicked() {
                action = Some(StyleAction::Use(style.name.clone()));
            }
        }
        ui_kit::section(ui, self.style_say("ui.style.mine"));
        let mine = self
            .settings_ui
            .styles
            .library
            .as_deref()
            .unwrap_or_default();
        if mine.is_empty() {
            ui.label(ui_kit::caption(self.style_say("ui.style.mine_none")));
        }
        for kept in mine {
            let style = &kept.style;
            let chosen = same_name(&style.name, &current);
            if ui_kit::swatch_item(ui, chosen, swatch(style), style.name.clone()).clicked() {
                action = Some(StyleAction::UseMine(Box::new(style.clone())));
            }
        }
        action
    }

    /// The buttons under the list, or the field for a new name while renaming.
    fn style_buttons(&mut self, ui: &mut egui::Ui) -> Option<StyleAction> {
        if self.settings_ui.styles.renaming.is_some() {
            return self.rename_field(ui);
        }
        let current = self.panes[self.active].style_name();
        let built_in = built_in_name(&current).is_some();
        let in_model = self.doc.model_style(&current).is_some();
        let in_mine = self.mine_named(&current).is_some();
        let say = |key| self.style_say(key);
        let mut action = None;
        let mut button = |ui: &mut egui::Ui, enabled: bool, key, chosen: StyleAction| {
            if ui
                .add_enabled(enabled, egui::Button::new(say(key)))
                .clicked()
            {
                action = Some(chosen);
            }
        };
        ui.add_space(ui_kit::ROW_GAP);
        ui_kit::chip_row(ui, |ui| {
            button(ui, true, "ui.style.new", StyleAction::New);
            button(ui, true, "ui.style.duplicate", StyleAction::Duplicate);
            button(ui, !built_in, "ui.style.rename", StyleAction::StartRename);
            button(ui, in_model, "ui.style.delete", StyleAction::Delete);
            let undo = !self.settings_ui.styles.undo.is_empty();
            button(ui, undo, "ui.style.undo", StyleAction::Undo);
        });
        ui_kit::chip_row(ui, |ui| {
            button(ui, !built_in, "ui.style.save_mine", StyleAction::SaveMine);
            button(ui, in_mine, "ui.style.delete_mine", StyleAction::DeleteMine);
            button(ui, true, "ui.style.import", StyleAction::Import);
            button(ui, true, "ui.style.export", StyleAction::Export);
        });
        action
    }

    /// The field for the active style's new name: Return or OK renames it, Esc or Cancel
    /// leaves it.
    fn rename_field(&mut self, ui: &mut egui::Ui) -> Option<StyleAction> {
        let ok = self.style_say("ui.style.ok");
        let cancel = self.style_say("ui.style.cancel");
        let panel = &mut self.settings_ui.styles;
        let mut action = None;
        ui.horizontal(|ui| {
            let Some(name) = panel.renaming.as_mut() else {
                return;
            };
            let field = ui_kit::plain_field(ui, name, 160.0);
            field.request_focus();
            let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if entered || ui.button(ok).clicked() {
                action = Some(StyleAction::Rename(name.clone()));
            }
            if ui.button(cancel).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                panel.renaming = None;
            }
        });
        action
    }

    /// The editing page for the active view's style; a change is kept at once.
    fn style_editor(&mut self, ui: &mut egui::Ui) {
        let style = self.pane_view_style(self.active).into_owned();
        let mut edited = style.clone();
        let tabs = [
            (StyleTab::Look, "ui.style.tab.look"),
            (StyleTab::Objects, "ui.style.tab.objects"),
            (StyleTab::Drawings, "ui.style.tab.drawings"),
        ];
        let panel = &mut self.settings_ui.styles;
        let mut form = fields::Form {
            text: &self.text,
            open: &mut panel.open,
        };
        look::engine(ui, &form, &mut edited);
        ui.add_space(ui_kit::ROW_GAP);
        ui.horizontal(|ui| {
            for (tab, key) in tabs {
                if ui_kit::tab(ui, panel.tab == tab, form.say(key)).clicked() {
                    panel.tab = tab;
                }
            }
        });
        match panel.tab {
            StyleTab::Look => look::tab(ui, &mut form, &mut edited),
            StyleTab::Objects => objects::tab(ui, &mut form, &mut panel.kind, &mut edited),
            StyleTab::Drawings => drawings::tab(ui, &mut form, &mut edited.drawings),
        }
        if edited != style {
            let now = ui.input(|i| i.time);
            self.keep_edit(edited, now);
        }
    }

    /// Keeps an edit of the active view's style in the model. A built-in style is copied first,
    /// under a new name the view then draws with.
    fn keep_edit(&mut self, mut edited: ViewStyle, now: f64) {
        if let Some(built) = built_in_name(&edited.name) {
            edited.name = self.free_style_name(
                &self
                    .text
                    .render(&Message::with("ui.style.copy_name", vec![built.to_owned()])),
            );
            self.remember_styles(None);
            self.settings_ui.styles.notice = Some(self.text.render(&Message::with(
                "ui.style.copied",
                vec![built.to_owned(), edited.name.clone()],
            )));
        } else {
            self.remember_styles(Some((now, edited.name.clone())));
        }
        let name = edited.name.clone();
        match self.doc.save_style(edited) {
            Ok(()) => self.panes[self.active].set_style(&name),
            Err(error) => self.settings_ui.styles.notice = Some(error.to_string()),
        }
    }

    /// Keeps how things stand for the panel's undo. An edit to the same style soon after the
    /// last (`edit`, with its time) joins the last step.
    fn remember_styles(&mut self, edit: Option<(f64, String)>) {
        let joins = joins_last(edit.as_ref(), self.settings_ui.styles.last_edit.as_ref());
        let snapshot = (
            self.doc.styles().to_vec(),
            self.panes.iter().map(|pane| pane.style.clone()).collect(),
        );
        let panel = &mut self.settings_ui.styles;
        if !joins {
            panel.undo.push(snapshot);
            if panel.undo.len() > UNDO_STEPS {
                panel.undo.remove(0);
            }
        }
        panel.last_edit = edit;
    }

    /// Steps the panel's undo back once.
    fn undo_styles(&mut self) {
        let Some((styles, panes)) = self.settings_ui.styles.undo.pop() else {
            return;
        };
        self.settings_ui.styles.last_edit = None;
        let names: Vec<String> = self.doc.styles().iter().map(|s| s.name.clone()).collect();
        for name in names {
            self.doc.delete_style(&name);
        }
        for style in styles {
            let _ = self.doc.save_style(style);
        }
        for (pane, style) in self.panes.iter_mut().zip(panes) {
            pane.style = style;
        }
    }

    /// `wanted`, or else it with the first free number after it, as a name no built-in style
    /// and none of the model's has.
    fn free_style_name(&self, wanted: &str) -> String {
        free_name(wanted, |name| self.doc.model_style(name).is_some())
    }

    /// My styles' style called `name`, ignoring case.
    fn mine_named(&self, name: &str) -> Option<&LibraryStyle> {
        self.settings_ui
            .styles
            .library
            .as_deref()?
            .iter()
            .find(|kept| same_name(&kept.style.name, name))
    }

    /// Reads My styles afresh.
    fn read_library(&mut self) {
        let styles = crate::styles::library().map(|library| library.list());
        self.settings_ui.styles.library = Some(styles.unwrap_or_default());
    }

    /// Adds `style` to the model, under a free name, and draws the active view with it.
    fn add_style(&mut self, mut style: ViewStyle) {
        self.remember_styles(None);
        style.name = self.free_style_name(&style.name);
        let name = style.name.clone();
        if let Err(error) = self.doc.save_style(style) {
            self.settings_ui.styles.notice = Some(error.to_string());
            return;
        }
        self.panes[self.active].set_style(&name);
    }

    /// Carries out what the list or a button asked for.
    fn apply_style_action(&mut self, action: StyleAction) {
        let current = self.pane_view_style(self.active).into_owned();
        self.settings_ui.styles.notice = None;
        match action {
            StyleAction::Use(name) => {
                if let Err(error) = self.set_pane_style(self.active, &name) {
                    self.settings_ui.styles.notice = Some(error.to_string());
                }
            }
            StyleAction::UseMine(style) => self.use_mine(*style),
            StyleAction::New => {
                let name = self.free_style_name(&self.style_say("ui.style.new_name"));
                self.add_style(ViewStyle {
                    name,
                    ..ViewStyle::shaded()
                });
            }
            StyleAction::Duplicate => {
                let name = self.text.render(&Message::with(
                    "ui.style.copy_name",
                    vec![current.name.clone()],
                ));
                self.add_style(ViewStyle { name, ..current });
            }
            StyleAction::StartRename => self.settings_ui.styles.renaming = Some(current.name),
            StyleAction::Rename(to) => self.rename_active(&current.name, &to),
            StyleAction::Delete => {
                self.remember_styles(None);
                self.doc.delete_style(&current.name);
            }
            StyleAction::DeleteMine => self.settings_ui.styles.deleting = Some(current.name),
            StyleAction::SaveMine => self.save_mine(&current),
            StyleAction::Import => self.open_style_file(FileJob::Import),
            StyleAction::Export => self.open_style_file(FileJob::Export(Box::new(current))),
            StyleAction::Undo => self.undo_styles(),
        }
    }

    /// Draws the active view with a style of My styles, copied into the model; the model's own
    /// copy of the same name is used if it has one, as the spec says the model's is drawn.
    fn use_mine(&mut self, style: ViewStyle) {
        if self.doc.model_style(&style.name).is_some() {
            self.panes[self.active].set_style(&style.name);
            self.settings_ui.styles.notice = Some(
                self.text
                    .render(&Message::with("ui.style.model_copy", vec![style.name])),
            );
            return;
        }
        self.add_style(style);
    }

    /// Renames the active view's style from `from` to `to`.
    fn rename_active(&mut self, from: &str, to: &str) {
        self.remember_styles(None);
        match self.rename_style(from, to) {
            Ok(()) => {
                self.settings_ui.styles.renaming = None;
            }
            Err(error) => {
                self.settings_ui.styles.undo.pop();
                self.settings_ui.styles.notice = Some(error.to_string());
            }
        }
    }

    /// Puts `style` in My styles.
    fn save_mine(&mut self, style: &ViewStyle) {
        let Some(library) = crate::styles::library() else {
            return;
        };
        self.settings_ui.styles.notice = Some(match library.save(style) {
            Ok(_) => self.text.render(&Message::with(
                "ui.style.saved_mine",
                vec![style.name.clone()],
            )),
            Err(error) => error.to_string(),
        });
        self.read_library();
    }

    /// Asks before deleting a style's file from My styles, which cannot be undone.
    fn confirm_mine_deleted(&mut self, ctx: &egui::Context) {
        let Some(name) = self.settings_ui.styles.deleting.clone() else {
            return;
        };
        let title = self.text.render(&Message::with(
            "ui.style.delete_mine_ask",
            vec![name.clone()],
        ));
        let choices = [
            ui_kit::other_choice(self.style_say("ui.style.delete")),
            ui_kit::main_choice(self.style_say("ui.style.cancel")),
        ];
        let note = self.style_say("ui.style.delete_mine_note");
        let Some(answer) = ui_kit::dialog(ctx, "delete style", &title, &choices, |ui| {
            ui.label(note);
        }) else {
            return;
        };
        self.settings_ui.styles.deleting = None;
        if answer != 0 {
            return;
        }
        if let Some(library) = crate::styles::library()
            && let Err(error) = library.delete(&name)
        {
            self.settings_ui.styles.notice = Some(error.to_string());
        }
        self.read_library();
    }

    /// Opens the system's dialog to pick a style file to import, or where to export one.
    fn open_style_file(&mut self, job: FileJob) {
        if self.settings_ui.styles.file.is_some() {
            return;
        }
        let kind = self.style_say("ui.style.file_kind");
        let mut dialog = rfd::AsyncFileDialog::new().add_filter(kind, &[EXTENSION]);
        if let Some(folder) = crate::styles::library().map(|l| l.folder().to_path_buf()) {
            dialog = dialog.set_directory(folder);
        }
        let save = match &job {
            FileJob::Export(style) => {
                dialog = dialog.set_file_name(format!("{}.{EXTENSION}", style.name));
                true
            }
            FileJob::Import => false,
        };
        let (send, picked) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let chosen = pollster::block_on(async {
                if save {
                    dialog.save_file().await
                } else {
                    dialog.pick_file().await
                }
            });
            let _ = send.send(chosen.map(|file| file.path().to_path_buf()));
        });
        self.settings_ui.styles.file = Some((job, picked));
    }

    /// Imports or exports once the dialog the panel opened closes.
    fn serve_style_file(&mut self) {
        let Some((_, picked)) = &self.settings_ui.styles.file else {
            return;
        };
        let answer = match picked.try_recv() {
            Ok(answer) => answer,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => None,
        };
        let Some((job, _)) = self.settings_ui.styles.file.take() else {
            return;
        };
        let Some(path) = answer else {
            return;
        };
        match job {
            FileJob::Import => self.import_style(&path),
            FileJob::Export(style) => {
                let said = match std::fs::write(&path, style.to_text()) {
                    Ok(()) => Message::with("ui.style.exported", vec![path.display().to_string()]),
                    Err(error) => Message::with("ui.style.file_error", vec![error.to_string()]),
                };
                self.settings_ui.styles.notice = Some(self.text.render(&said));
            }
        }
    }

    /// Reads the style file at `path` into the model and draws the active view with it; lines
    /// it could not use are said, by number.
    fn import_style(&mut self, path: &std::path::Path) {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                self.settings_ui.styles.notice = Some(self.text.render(&Message::with(
                    "ui.style.file_error",
                    vec![error.to_string()],
                )));
                return;
            }
        };
        let read = ViewStyle::from_text(&text);
        let mut style = read.style;
        if style.name.trim().is_empty() || built_in_name(&style.name).is_some() {
            let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned());
            style.name = stem.unwrap_or_else(|| self.style_say("ui.style.new_name"));
        }
        self.add_style(style);
        if !read.problems.is_empty() {
            let lines: Vec<String> = read.problems.iter().map(|p| p.line.to_string()).collect();
            self.settings_ui.styles.notice = Some(self.text.render(&Message::with(
                "ui.style.import_problems",
                vec![lines.join(", ")],
            )));
        }
    }
}

/// Whether an edit (its time and style) joins the panel's last undo step: the same style,
/// changed again within [`MERGE_SECONDS`], as a slider being dragged is.
fn joins_last(edit: Option<&(f64, String)>, last: Option<&(f64, String)>) -> bool {
    match (edit, last) {
        (Some((now, name)), Some((then, last))) => {
            now - then < MERGE_SECONDS && same_name(name, last)
        }
        _ => false,
    }
}

/// `wanted`, or else it with the first free number after it: a name no built-in style has and
/// `taken` says the model does not have.
fn free_name(wanted: &str, taken: impl Fn(&str) -> bool) -> String {
    let wanted = wanted.trim();
    let free = |name: &str| built_in_name(name).is_none() && !taken(name);
    if free(wanted) {
        return wanted.to_owned();
    }
    (2..)
        .map(|n| format!("{wanted} {n}"))
        .find(|name| free(name))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_name_is_never_a_built_in_or_taken_one() {
        assert_eq!(free_name(" Diagram ", |_| false), "Diagram");
        assert_eq!(free_name("shaded", |_| false), "shaded 2");
        let taken = ["Shaded copy", "shaded copy 2"];
        let is_taken = |name: &str| taken.iter().any(|t| same_name(t, name));
        assert_eq!(free_name("Shaded copy", is_taken), "Shaded copy 3");
    }

    #[test]
    fn dragging_a_slider_is_one_undo_step() {
        let first = (10.0, "Diagram".to_owned());
        let soon = (10.5, "diagram".to_owned());
        let later = (12.0, "Diagram".to_owned());
        let other = (10.5, "Study".to_owned());
        assert!(joins_last(Some(&soon), Some(&first)));
        assert!(!joins_last(Some(&later), Some(&first)));
        assert!(!joins_last(Some(&other), Some(&first)));
        assert!(!joins_last(None, Some(&first)));
        assert!(!joins_last(Some(&soon), None));
    }

    #[test]
    fn each_style_shows_its_own_background() {
        let massing = swatch(&ViewStyle::massing());
        assert_eq!(massing, ([170, 204, 232], [184, 184, 179]));
        assert_eq!(swatch(&ViewStyle::hidden()), ([255; 3], [255; 3]));
    }
}
