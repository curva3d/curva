//! The top bar: the command tabs and their buttons, the specialization dial and the File items.

use crate::*;
use curva_commands::commands::tabs;

/// The top bar's chosen command tab and specialization.
#[derive(Default)]
pub(crate) struct TopBar {
    /// The command tab showing, as an index into [`COMMAND_TABS`].
    pub(crate) command_tab: usize,
    /// The specialization selected in the top bar's dial, as an index into [`SPECIALIZATIONS`].
    pub(crate) specialization: usize,
    /// Curva's mark, made from the window icon the first time the bar is drawn.
    pub(crate) mark: Option<egui::TextureHandle>,
    /// When the file's path was last copied by clicking it, for its glow and note.
    pub(crate) copied_at: Option<f64>,
    /// The tab last used in each of [`TAB_GROUPS`], opened again when that group is chosen.
    pub(crate) last_tab: [usize; 4],
    /// When the group last changed, for the tabs' fade.
    pub(crate) group_changed_at: f64,
    /// How wide the groups' row is, so the title row's grip starts after it.
    pub(crate) groups_width: f32,
    /// How tall the command tabs' contents are with the panel's margins: the panel is made this
    /// tall when it holds them alone, so no room is wasted below the buttons.
    pub(crate) tabs_fit: f32,
}

/// How long the glow and the note show after the path is copied, in seconds.
const COPIED_FOR: f64 = 1.6;

/// `path` shortened in the middle until `fits` accepts it: the first part and as many of the
/// last parts as fit, with `…` between (spec `feedback-0.4.2.md` §4).
pub(crate) fn shortened_path(path: &str, fits: impl Fn(&str) -> bool) -> String {
    if fits(path) {
        return path.to_owned();
    }
    let separator = if path.contains('\\') { '\\' } else { '/' };
    let parts: Vec<&str> = path.split(separator).collect();
    let Some((first, rest)) = parts.split_first() else {
        return path.to_owned();
    };
    let gap = format!("{separator}…{separator}");
    // Keep the most of the end that fits, always keeping at least the file's own name.
    for kept in (1..rest.len()).rev() {
        let tail = rest[rest.len() - kept..].join(&separator.to_string());
        let candidate = format!("{first}{gap}{tail}");
        if fits(&candidate) {
            return candidate;
        }
    }
    let name = rest.last().copied().unwrap_or(first);
    let short = format!("…{separator}{name}");
    if fits(&short) { short } else { name.to_owned() }
}

/// The command tabs at the top, each with its buttons, from the command folders (spec
/// `docs/specs/command-groups.md` §2): the NURBS tabs, Mesh, then the tabs every kind of
/// geometry shares.
pub(crate) const COMMAND_TABS: [(&str, &[&str]); 11] = [
    ("ui.tab.curves", tabs::CURVES),
    ("ui.tab.surfaces", tabs::SURFACES),
    ("ui.tab.solids", tabs::SOLIDS),
    ("ui.tab.mesh", tabs::MESH),
    ("ui.tab.subd", tabs::SUBD),
    ("ui.tab.edit", tabs::EDIT),
    ("ui.tab.organise", tabs::ORGANISE),
    ("ui.tab.select", tabs::SELECT),
    ("ui.tab.analyze", tabs::ANALYZE),
    ("ui.tab.annotate", tabs::ANNOTATE),
    ("ui.tab.layouts", tabs::LAYOUTS),
];

/// The groups of command tabs, each a run of [`COMMAND_TABS`] (owner, 22 November 2026): NURBS
/// (Curves, Surfaces, Solids), Polygon (Mesh, SubD), Modify (Edit, Organise, Select, Analyze)
/// and Publisher (Annotate, Layouts).
pub(crate) const TAB_GROUPS: [(&str, std::ops::Range<usize>); 4] = [
    ("ui.tab_group.nurbs", 0..3),
    ("ui.tab_group.polygon", 3..5),
    ("ui.tab_group.modify", 5..9),
    ("ui.tab_group.publisher", 9..11),
];

/// The buttons of one tab in a row that scrolls sideways when the bar is narrower than they are:
/// a thin bar under them in the panel's own quiet colour (not over them), and the row fading out
/// at an edge where more buttons wait (owner, 22 November 2026). Returns the command clicked.
fn command_buttons_row(ui: &mut egui::Ui, names: &[&'static str]) -> Option<&'static str> {
    let mut chosen = None;
    ui_kit::strip(ui, |ui| {
        for name in names {
            if command_button(ui, name).clicked() {
                chosen = Some(*name);
            }
        }
    });
    chosen
}

/// How long the tabs and buttons take to fade in when the group changes, in seconds.
const GROUP_FADE: f64 = 0.18;

/// The group a command tab belongs to.
pub(crate) fn group_of(tab: usize) -> usize {
    TAB_GROUPS
        .iter()
        .position(|(_, tabs)| tabs.contains(&tab))
        .unwrap_or(0)
}

/// The specializations in the top bar's dial; only Freeform has commands yet.
pub(crate) const SPECIALIZATIONS: [&str; 4] = [
    "ui.specialization.freeform",
    "ui.specialization.architecture",
    "ui.specialization.publisher",
    "ui.specialization.analysis",
];

/// The width of the specializations dial in the top bar, in points.
pub(crate) const DIAL_WIDTH: f32 = 440.0;

/// The height of the top bar, in points.
pub(crate) const TOP_BAR_HEIGHT: f32 = 36.0;

impl Curva {
    /// The command tabs, as the owner chose (option E, 22 November 2026): the groups (NURBS,
    /// Polygon, Modify, Publisher) in a row across the top, in the title row's place, with an
    /// mark that slides to the one chosen; its tabs below, fading in when the group changes;
    /// then the chosen tab's buttons. Each group keeps the tab last used in it.
    pub(crate) fn command_tabs(&mut self, ui: &mut egui::Ui) {
        // Only Freeform has commands yet; the other specializations say when they come.
        if self.bar.specialization != 0 {
            ui.label(self.text.render(&Message::new("ui.specialization.later")));
            return;
        }
        let tab = self.bar.command_tab.min(COMMAND_TABS.len() - 1);
        let group = group_of(tab);
        self.tab_groups_row(ui, group);
        // The tabs and buttons fade in when the group changes, while the groups stay put.
        let now = ui.input(|input| input.time);
        let shown = ((now - self.bar.group_changed_at) / GROUP_FADE).clamp(0.0, 1.0) as f32;
        if shown < 1.0 {
            ui.ctx().request_repaint();
        }
        ui.scope(|ui| {
            ui.multiply_opacity(shown);
            ui.horizontal(|ui| {
                for index in TAB_GROUPS[group].1.clone() {
                    let label = self.text.render(&Message::new(COMMAND_TABS[index].0));
                    if ui_kit::tab(ui, tab == index, label).clicked() {
                        self.bar.command_tab = index;
                    }
                }
            });
            let chosen = command_buttons_row(ui, COMMAND_TABS[tab].1);
            if let Some(name) = chosen {
                self.run_from_button(name);
            }
        });
        // The panel is made exactly this tall, so nothing is left empty below the buttons.
        self.bar.tabs_fit = ui.min_rect().height() + 2.0 * PANEL_MARGIN;
    }

    /// The groups' row, with the chosen one marked, the mark sliding; a click on another
    /// opens the tab last used in it. Notes how wide the row is, for the title row's grip.
    fn tab_groups_row(&mut self, ui: &mut egui::Ui, group: usize) {
        let names: Vec<String> = TAB_GROUPS
            .iter()
            .map(|(key, _)| self.text.render(&Message::new(key)))
            .collect();
        let (clicked, row) = ui_kit::group_tabs(ui, &names, group);
        if let Some(index) = clicked {
            self.bar.last_tab[group] = self.bar.command_tab;
            let last = self.bar.last_tab[index];
            self.bar.command_tab = if TAB_GROUPS[index].1.contains(&last) {
                last
            } else {
                TAB_GROUPS[index].1.start
            };
            self.bar.group_changed_at = ui.input(|input| input.time);
        }
        self.bar.groups_width = row.width();
    }

    /// The top bar, named Curva: always there and fixed (only focus mode hides it). The file's
    /// name and Curva centred on top; under them File and Settings at the left, and the
    /// specializations as a dial in the middle, the selected one centred.
    pub(crate) fn top_bar(&mut self, ctx: &egui::Context, rect: egui::Rect) {
        let glass = self.settings.glass;
        let frame = ui_kit::panel_frame_for(glass);
        let mut chosen = None;
        egui::Area::new(egui::Id::NULL.with("top bar"))
            .fixed_pos(rect.min)
            .order(egui::Order::Middle)
            .constrain(false)
            .show(ctx, |ui| {
                ui.set_clip_rect(rect.expand(SHADOW_ROOM));
                if glass {
                    ui_kit::glass_backdrop(ui, rect);
                }
                frame.show(ui, |ui| {
                    let inner = rect.shrink(PANEL_MARGIN);
                    ui.set_min_size(inner.size());
                    ui.set_max_size(inner.size());
                    // Left: Curva's mark and name, then File and Settings.
                    let left = ui
                        .horizontal_centered(|ui| {
                            let mark = self.top_mark(ui.ctx());
                            if let Some(mark) = mark {
                                ui.add(
                                    egui::Image::new(&mark)
                                        .fit_to_exact_size(egui::Vec2::splat(18.0)),
                                );
                            }
                            ui.label(ui_kit::brand("Curva"));
                            ui.add_space(ui_kit::ITEM_GAP);
                            ui.menu_button(self.text.render(&Message::new("ui.tab.file")), |ui| {
                                chosen = self.file_items(ui);
                            });
                            if ui
                                .button(self.text.render(&Message::new("ui.file.settings")))
                                .clicked()
                            {
                                self.open_settings(SettingsTab::General);
                            }
                        })
                        .response
                        .rect;
                    // Centre: the specialization dial, centred on the window.
                    let dial = egui::Rect::from_center_size(
                        egui::pos2(rect.center().x, inner.center().y),
                        egui::vec2(DIAL_WIDTH, 22.0),
                    );
                    self.specialization_dial(ui, dial);
                    // Right: the file's name over its path, right-aligned.
                    let room = egui::Rect::from_min_max(
                        egui::pos2(dial.max.x.max(left.max.x) + 12.0, rect.min.y),
                        egui::pos2(inner.max.x, rect.max.y),
                    );
                    self.file_label(ui, room);
                });
                if glass {
                    ui_kit::glass_edge_light(ui, rect);
                }
            });
        if let Some(name) = chosen {
            self.run_from_button(name);
        }
    }

    /// Curva's mark as a texture, made once from the window icon.
    fn top_mark(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        if self.bar.mark.is_none() {
            let icon = window_icon()?;
            let size = [icon.width as usize, icon.height as usize];
            let image = egui::ColorImage::from_rgba_unmultiplied(size, &icon.rgba);
            self.bar.mark =
                Some(ctx.load_texture("curva mark", image, egui::TextureOptions::LINEAR));
        }
        self.bar.mark.clone()
    }

    /// The model's folder and name on one line, right-aligned in `room`: the folder quieter, a
    /// long one shortened in the middle and shown in full on hover. Clicking the line, with
    /// either button, copies the full path (the name of an untitled model); the line then glows
    /// and a note says it was copied.
    fn file_label(&mut self, ui: &egui::Ui, room: egui::Rect) {
        if room.width() < 24.0 {
            return;
        }
        let untitled = self.text.render(&Message::new("ui.untitled"));
        let (name, folder, full) = path_parts(self.doc.path(), untitled);
        let name_width = ui_kit::text_width(ui, &name, &ui_kit::name_font());
        let room_for_folder = (room.width() - name_width - 8.0).max(0.0);
        let folder_font = ui_kit::caption_font();
        let shown = folder.as_ref().map(|full| {
            shortened_path(full, |text| {
                ui_kit::text_width(ui, text, &folder_font) <= room_for_folder
            })
        });
        // The glow after copying, strongest at first and fading out.
        let now = ui.input(|i| i.time);
        let glow = glow_left(self.bar.copied_at, now);
        #[allow(clippy::cast_possible_truncation)]
        let strength = glow.map(|left| (left * left) as f32);
        if glow.is_some() {
            ui.ctx().request_repaint();
        }
        let line = ui_kit::name_and_folder(ui, room, &name, shown.as_deref(), strength);
        let response = ui.interact(
            line,
            egui::Id::NULL.with("top bar path"),
            egui::Sense::click(),
        );
        if response.clicked() || response.secondary_clicked() {
            ui.ctx().copy_text(full.clone());
            self.bar.copied_at = Some(now);
        }
        if glow.is_some() {
            let note = self.text.render(&Message::new("ui.path_copied"));
            let at = egui::pos2(line.max.x - 4.0, line.max.y + 6.0);
            ui_kit::note(ui.ctx(), "path copied", at, egui::Align2::RIGHT_TOP, note);
        } else {
            response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(format!(
                    "{full}\n{}",
                    self.text.render(&Message::new("ui.path_copy_hint"))
                ));
        }
    }

    /// The specializations side by side in one pill, the selected one lit (the owner's choice E,
    /// 21 November 2026, #481). Clicking one, or scrolling over the pill, chooses it.
    pub(crate) fn specialization_dial(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let count = SPECIALIZATIONS.len();
        let response = ui.interact(rect, ui.scope_id().with("dial"), egui::Sense::click());
        if response.hovered() {
            let scroll =
                ui.input(|input| input.smooth_scroll_delta.x + input.smooth_scroll_delta.y);
            if scroll.abs() > 20.0 {
                self.bar.specialization =
                    (self.bar.specialization + if scroll < 0.0 { 1 } else { count - 1 }) % count;
            }
        }
        let names: Vec<String> = SPECIALIZATIONS
            .iter()
            .map(|key| self.text.render(&Message::new(key)))
            .collect();
        if let Some(index) = ui_kit::pill(ui, rect, &names, self.bar.specialization) {
            self.bar.specialization = index;
        }
    }

    /// Runs a command as typing it would, ending any command already running, as a Rhino
    /// toolbar button does.
    pub(crate) fn run_from_button(&mut self, name: &str) {
        if self.processor.is_running() {
            self.processor.cancel(&mut self.doc);
        }
        self.typed.clear();
        self.highlight = 0;
        self.processor.submit(&mut self.doc, name);
    }

    /// The File menu: New, Open, Save, Save As, Import, Export and Export Selected run their
    /// commands; About opens Settings at its About tab. Returns a command to run.
    pub(crate) fn file_items(&mut self, ui: &mut egui::Ui) -> Option<&'static str> {
        let mut chosen = None;
        for (name, key) in [
            ("New", "ui.file.new"),
            ("Open", "ui.file.open"),
            ("Save", "ui.file.save"),
            ("SaveAs", "ui.file.save_as"),
        ] {
            if ui.button(self.text.render(&Message::new(key))).clicked() {
                chosen = Some(name);
                ui.close();
            }
        }
        ui.separator();
        // Rhino files: Import adds one to the model, Export and ExportSelected write one.
        for (name, key) in [
            ("Import", "ui.file.import"),
            ("Export", "ui.file.export"),
            ("ExportSelected", "ui.file.export_selected"),
        ] {
            if ui.button(self.text.render(&Message::new(key))).clicked() {
                chosen = Some(name);
                ui.close();
            }
        }
        ui.separator();
        if ui
            .button(self.text.render(&Message::new("ui.file.about")))
            .clicked()
        {
            self.open_settings(SettingsTab::About);
            ui.close();
        }
        chosen
    }

    /// Brings a closed or hidden panel piece out, as Settings is.
    pub(crate) fn open_piece(&mut self, piece: layout::Piece) {
        let window = self.window_rect;
        self.layout = self.layout.restored(piece, window);
        if let Some(panel) = self.layout.panel_of(piece)
            && let Some(index) = self
                .layout
                .place(panel)
                .and_then(|p| p.pieces.iter().position(|p| *p == piece))
        {
            self.layout = self
                .layout
                .with_active(panel, index)
                .with_hidden(panel, false);
        }
        self.save_layout();
    }
}

/// What the file label shows for a model saved at `path`, or an untitled one: its name, its
/// folder with a trailing separator (none when untitled), and the full path that a click copies.
fn path_parts(
    path: Option<&std::path::Path>,
    untitled: String,
) -> (String, Option<String>, String) {
    let name = path
        .and_then(|p| p.file_name())
        .map_or(untitled, |file| file.to_string_lossy().into_owned());
    let folder = path
        .and_then(|p| p.parent())
        .map(|f| format!("{}{}", f.to_string_lossy(), std::path::MAIN_SEPARATOR));
    let full = path.map_or_else(|| name.clone(), |p| p.to_string_lossy().into_owned());
    (name, folder, full)
}

/// How much of the glow after copying is left at `now`: 1 just after, fading to nothing over
/// [`COPIED_FOR`] seconds; `None` once it has gone or before anything was copied.
fn glow_left(copied_at: Option<f64>, now: f64) -> Option<f64> {
    copied_at
        .map(|at| 1.0 - ((now - at) / COPIED_FOR).clamp(0.0, 1.0))
        .filter(|left| *left > 0.0)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_file_label_shows_name_folder_and_copies_the_full_path() {
        let path = std::path::Path::new("models").join("tower.curva");
        let (name, folder, full) = super::path_parts(Some(&path), "Untitled".to_owned());
        assert_eq!(name, "tower.curva");
        assert_eq!(folder, Some(format!("models{}", std::path::MAIN_SEPARATOR)));
        assert_eq!(full, path.to_string_lossy());
        let untitled = super::path_parts(None, "Untitled".to_owned());
        assert_eq!(
            untitled,
            ("Untitled".to_owned(), None, "Untitled".to_owned())
        );
    }

    #[test]
    fn the_copy_glow_fades_out_and_then_is_gone() {
        assert_eq!(super::glow_left(None, 5.0), None);
        assert_eq!(super::glow_left(Some(5.0), 5.0), Some(1.0));
        let halfway = super::glow_left(Some(5.0), 5.0 + super::COPIED_FOR / 2.0).unwrap();
        assert!((halfway - 0.5).abs() < 1e-9);
        assert_eq!(
            super::glow_left(Some(5.0), 5.0 + super::COPIED_FOR + 0.01),
            None
        );
    }

    #[test]
    fn a_long_path_is_shortened_in_the_middle_keeping_the_file_name() {
        let fits = |limit: usize| move |text: &str| text.chars().count() <= limit;
        let path = r"V:\projects\client\2026\tower.curva";
        assert_eq!(super::shortened_path(path, fits(100)), path);
        assert_eq!(
            super::shortened_path(path, fits(26)),
            r"V:\…\2026\tower.curva"
        );
        assert_eq!(super::shortened_path(path, fits(20)), r"V:\…\tower.curva");
        assert_eq!(super::shortened_path(path, fits(12)), "tower.curva");
        assert_eq!(
            super::shortened_path("/home/me/models/tower.curva", fits(22)),
            "/…/models/tower.curva"
        );
    }

    use super::*;

    #[test]
    fn the_specialization_pill_sits_its_names_side_by_side_centred() {
        let segments = ui_kit::pill_segments(&[60.0, 90.0, 70.0, 56.0], 500.0);
        assert_eq!(segments.len(), 4);
        for pair in segments.windows(2) {
            assert!((pair[0].1 - pair[1].0).abs() < 1e-4, "touching, in order");
        }
        let (left, right) = (segments[0].0, segments[3].1);
        assert!(((left + right) / 2.0 - 500.0).abs() < 1e-4, "centred");
        assert!((segments[1].1 - segments[1].0 - (90.0 + 2.0 * ui_kit::PILL_PAD.x)).abs() < 1e-4);
        assert!(
            right - left + 2.0 * ui_kit::PILL_INSET < DIAL_WIDTH,
            "fits the space the dial had"
        );
    }

    #[test]
    fn the_groups_hold_every_tab_once_in_order() {
        // NURBS, Polygon, Modify and Publisher, one after the other, with nothing left out.
        let mut next = 0;
        for (key, tabs) in TAB_GROUPS {
            assert_eq!(tabs.start, next, "{key} starts where the last ended");
            assert!(!tabs.is_empty(), "{key} has tabs");
            next = tabs.end;
        }
        assert_eq!(next, COMMAND_TABS.len());
        assert_eq!(COMMAND_TABS[4].0, "ui.tab.subd");
        assert_eq!(group_of(4), 1, "SubD is a Polygon tab");
        assert_eq!(group_of(10), 3, "Layouts is a Publisher tab");
    }

    #[test]
    fn every_command_button_names_a_command_and_each_tab_has_some() {
        for (tab, names) in COMMAND_TABS {
            assert!(!names.is_empty(), "{tab} is empty");
            for name in names {
                let found = curva_commands::commands::resolve(name).map(|def| def.name);
                assert_eq!(found, Some(*name), "{name} in {tab}");
            }
        }
    }
}
