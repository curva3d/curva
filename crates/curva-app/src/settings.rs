//! The Settings panel, the theme and the dialogs it asks.

use crate::*;

/// The Settings panel's tab and the questions it is asking.
#[derive(Default)]
pub(crate) struct SettingsUi {
    /// The Settings panel's tab showing.
    pub(crate) settings_tab: SettingsTab,
    /// A unit chosen in Settings, while asking whether to scale the model to it.
    pub(crate) unit_asked: Option<curva_document::Unit>,
    /// Whether the light theme was last put in use, once one has been.
    pub(crate) applied_light: Option<bool>,
}

/// The Settings panel's tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SettingsTab {
    #[default]
    General,
    Interface,
    Assistant,
    About,
}

/// A small question in the centre of the window, drawn like a panel.
pub(crate) fn centred_dialog(ctx: &egui::Context, id: &str, contents: impl FnOnce(&mut egui::Ui)) {
    egui::Area::new(egui::Id::NULL.with(("dialog", id)))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            panel_frame(false).show(ui, |ui| {
                ui.set_max_width(420.0);
                contents(ui);
            });
        });
}

impl Curva {
    /// Puts the chosen theme in use (or the system's), when it changes: the interface's look,
    /// the viewports' grids and the colours objects are drawn in.
    pub(crate) fn apply_theme(&mut self, ctx: &egui::Context) {
        let light = self.settings.light(ctx.system_theme());
        if self.settings_ui.applied_light == Some(light) {
            return;
        }
        self.settings_ui.applied_light = Some(light);
        theme::use_light(light);
        ctx.set_visuals(theme::visuals(theme::current()));
        // One set of sizes for every panel, the top bar included.
        ctx.all_styles_mut(|style| {
            use egui::{FontFamily, FontId, TextStyle};
            style
                .text_styles
                .insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
            style.text_styles.insert(
                TextStyle::Button,
                FontId::new(14.0, FontFamily::Proportional),
            );
            style.text_styles.insert(
                TextStyle::Small,
                FontId::new(12.0, FontFamily::Proportional),
            );
            style.text_styles.insert(
                TextStyle::Heading,
                FontId::new(18.0, FontFamily::Proportional),
            );
            style.text_styles.insert(
                TextStyle::Monospace,
                FontId::new(13.0, FontFamily::Monospace),
            );
        });
        for pane in &mut self.panes {
            let plane = pane.plane;
            pane.set_plane(plane);
        }
        self.drawn = None;
    }

    /// Brings the Settings panel out at a tab: back from closed, floating in the centre of the
    /// window, or to the front of the panel holding it.
    pub(crate) fn open_settings(&mut self, tab: SettingsTab) {
        self.settings_ui.settings_tab = tab;
        let window = self.window_rect;
        self.layout = self.layout.restored(layout::Piece::Settings, window);
        if let Some(panel) = self.layout.panel_of(layout::Piece::Settings)
            && let Some(index) = self
                .layout
                .place(panel)
                .and_then(|p| p.pieces.iter().position(|p| *p == layout::Piece::Settings))
        {
            self.layout = self
                .layout
                .with_active(panel, index)
                .with_hidden(panel, false);
        }
        self.save_layout();
    }

    /// The Settings panel, in tabs: General, Interface, AI Assistant and About.
    pub(crate) fn settings_piece(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (tab, key) in [
                (SettingsTab::General, "ui.settings.general"),
                (SettingsTab::Interface, "ui.settings.interface"),
                (SettingsTab::Assistant, "ui.piece.assistant"),
                (SettingsTab::About, "ui.file.about"),
            ] {
                let label = self.text.render(&Message::new(key));
                if ui
                    .selectable_label(self.settings_ui.settings_tab == tab, label)
                    .clicked()
                {
                    self.settings_ui.settings_tab = tab;
                }
            }
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| match self.settings_ui.settings_tab {
                SettingsTab::General => self.general_settings(ui),
                SettingsTab::Interface => self.interface_settings(ui),
                SettingsTab::Assistant => self.assistant_settings(ui),
                SettingsTab::About => self.about_settings(ui),
            });
    }

    /// Settings → General: theme, glass or solid panels, the model's unit, the grid, updates.
    pub(crate) fn general_settings(&mut self, ui: &mut egui::Ui) {
        let before = self.settings.clone();
        let mut unit = self.doc.state().unit;
        let text = &self.text;
        let settings = &mut self.settings;
        let say = |key: &'static str| text.render(&Message::new(key));
        ui.label(egui::RichText::new(say("ui.settings.theme")).strong());
        ui.horizontal(|ui| {
            for (choice, key) in [
                (theme::ThemeChoice::System, "ui.settings.system"),
                (theme::ThemeChoice::Light, "ui.settings.light"),
                (theme::ThemeChoice::Dark, "ui.settings.dark"),
            ] {
                ui.radio_value(&mut settings.theme, choice, say(key));
            }
        });
        ui.label(egui::RichText::new(say("ui.settings.panels")).strong());
        ui.horizontal(|ui| {
            ui.radio_value(&mut settings.glass, true, say("ui.settings.glass"));
            ui.radio_value(&mut settings.glass, false, say("ui.settings.solid"));
        });
        ui.label(egui::RichText::new(say("ui.settings.units")).strong());
        ui.horizontal(|ui| {
            ui.radio_value(
                &mut unit,
                curva_document::Unit::Millimetres,
                say("ui.settings.mm"),
            );
            ui.radio_value(
                &mut unit,
                curva_document::Unit::Metres,
                say("ui.settings.m"),
            );
        });
        ui.checkbox(&mut settings.grid, say("ui.settings.grid"));
        ui.checkbox(&mut settings.check_updates, say("ui.settings.updates"));
        ui.label(egui::RichText::new(say("ui.settings.undo")).strong());
        ui.horizontal(|ui| {
            ui.label(say("ui.settings.undo_steps"));
            ui.add(egui::DragValue::new(&mut settings.undo_min_steps).range(0..=1000));
        });
        ui.horizontal(|ui| {
            ui.label(say("ui.settings.undo_memory"));
            ui.add(egui::DragValue::new(&mut settings.undo_max_megabytes).range(1..=65536));
        });
        let (steps, bytes) = self.doc.undo_held();
        ui.label(text.render(&Message::with(
            "ui.settings.undo_held",
            vec![
                steps.to_string(),
                format!("{:.1}", bytes as f64 / theme::MEGABYTE as f64),
            ],
        )));
        if unit != self.doc.state().unit {
            self.settings_ui.unit_asked = Some(unit);
        }
        if self.settings != before {
            self.settings_ui.applied_light = None;
            if self.benchmark.is_none()
                && let Some(file) = theme::file()
            {
                let _ = self.settings.save(&file);
            }
        }
    }

    /// Settings → Interface: every panel with a switch to show or close it, and a way back to the
    /// standard arrangement.
    pub(crate) fn interface_settings(&mut self, ui: &mut egui::Ui) {
        ui.label(
            self.text
                .render(&Message::new("ui.settings.interface_note")),
        );
        let window = self.window_rect;
        for piece in layout::Piece::ALL {
            let mut shown = self.layout.is_shown(piece);
            let name = self.text.render(&Message::new(piece.name_key()));
            if ui.checkbox(&mut shown, name).changed() {
                self.layout = if shown {
                    self.layout.restored(piece, window)
                } else {
                    // Only this piece goes, even from a panel with others.
                    let alone = self.layout.pulled(
                        piece,
                        layout::Dock::Floating { x: 0.0, y: 0.0 },
                        layout::FLOATING_STANDARD,
                    );
                    alone.closing(piece)
                };
                self.save_layout();
            }
        }
        ui.add_space(8.0);
        if ui
            .button(self.text.render(&Message::new("ui.layout.reset")))
            .clicked()
        {
            self.layout = layout::Layout::default().restored(layout::Piece::Settings, window);
            self.save_layout();
        }
    }

    /// Settings → About: the version, the licence, and the credits.
    pub(crate) fn about_settings(&self, ui: &mut egui::Ui) {
        ui.heading(format!("Curva {}", env!("CARGO_PKG_VERSION")));
        ui.label(self.text.render(&Message::new("ui.about.licence")));
        ui.separator();
        ui.label(include_str!("../../../ATTRIBUTION.md"));
    }

    /// As Rhino does, asks whether changing the unit should scale the model to keep its real
    /// size or keep the numbers as they are.
    pub(crate) fn unit_question(&mut self, ctx: &egui::Context) {
        let Some(unit) = self.settings_ui.unit_asked else {
            return;
        };
        let mut answer = None;
        let say = |key: &'static str| self.text.render(&Message::new(key));
        let question = self.text.render(&Message::with(
            "ui.units.question",
            vec![unit.symbol().to_owned()],
        ));
        centred_dialog(ctx, "unit question", |ui| {
            ui.label(question);
            ui.horizontal(|ui| {
                if ui.button(say("ui.units.scale")).clicked() {
                    answer = Some(Some(true));
                }
                if ui.button(say("ui.units.keep")).clicked() {
                    answer = Some(Some(false));
                }
                if ui.button(say("ui.units.cancel")).clicked() {
                    answer = Some(None);
                }
            });
        });
        if let Some(choice) = answer {
            self.settings_ui.unit_asked = None;
            if let Some(scale) = choice {
                let before = self.doc.state().clone();
                self.doc.set_unit(unit, scale);
                self.doc.record_change(before);
            }
        }
    }

    /// The Display panel: the active viewport's display mode, as a button with a V pointing down
    /// that opens the list of modes.
    pub(crate) fn display_piece(&mut self, ui: &mut egui::Ui) {
        let current = self.panes[self.active].display;
        let label = |mode: DisplayMode| match mode {
            DisplayMode::Wireframe => "ui.display.wireframe",
            DisplayMode::Shaded => "ui.display.shaded",
            DisplayMode::Ghosted => "ui.display.ghosted",
        };
        let title = self.text.render(&Message::new(label(current)));
        let mut chosen = None;
        egui::ComboBox::from_id_salt("display mode")
            .selected_text(title)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for mode in [
                    DisplayMode::Wireframe,
                    DisplayMode::Shaded,
                    DisplayMode::Ghosted,
                ] {
                    let name = self.text.render(&Message::new(label(mode)));
                    if ui.selectable_label(mode == current, name).clicked() {
                        chosen = Some(mode);
                    }
                }
            });
        if let Some(mode) = chosen {
            self.panes[self.active].display = mode;
        }
    }

    /// Settings → AI Assistant.
    pub(crate) fn assistant_settings(&mut self, ui: &mut egui::Ui) {
        self.assistant_connections(ui);
    }

    /// Keeps the settings for the next session.
    pub(crate) fn save_settings(&self) {
        if self.benchmark.is_none()
            && let Some(file) = theme::file()
        {
            let _ = self.settings.save(&file);
        }
    }
}
