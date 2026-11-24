//! The Style panel's rows: each setting as a labelled control in the UI kit's form, and the
//! sections with their More switch.

use crate::ui_kit;
use curva_commands::{Message, Translations};
use curva_document::style::{Colour, ColourFrom, LineColour, LineWidth, OutlineChoice, Pattern};
use eframe::egui;
use std::collections::BTreeSet;

/// What the rows need: the words, and which sections show their More settings.
pub(super) struct Form<'a> {
    pub(super) text: &'a Translations,
    pub(super) open: &'a mut BTreeSet<&'static str>,
}

impl Form<'_> {
    /// The words for `key`.
    pub(super) fn say(&self, key: &'static str) -> String {
        self.text.render(&Message::new(key))
    }

    /// A section's heading, titled by `key`, with a More switch when it has `more` settings
    /// beyond its first ones. Returns whether its More settings show.
    pub(super) fn section(&mut self, ui: &mut egui::Ui, key: &'static str, more: usize) -> bool {
        let open = self.open.contains(key);
        let words = (more > 0).then(|| {
            if open {
                self.say("ui.style.less")
            } else {
                self.text
                    .render(&Message::with("ui.style.more", vec![more.to_string()]))
            }
        });
        if ui_kit::section_with_more(ui, self.say(key), words) {
            if open {
                self.open.remove(key);
            } else {
                self.open.insert(key);
            }
        }
        open
    }

    /// An on/off setting.
    pub(super) fn switch(&self, ui: &mut egui::Ui, key: &'static str, on: &mut bool) {
        let words = [self.say("ui.style.off"), self.say("ui.style.on")];
        ui_kit::form_row(ui, self.say(key), |ui| ui_kit::either(ui, on, words));
    }

    /// A choice among a few, side by side.
    pub(super) fn choice<T: Copy + PartialEq>(
        &self,
        ui: &mut egui::Ui,
        key: &'static str,
        value: &mut T,
        options: &[(T, &'static str)],
    ) {
        let names: Vec<String> = options.iter().map(|(_, key)| self.say(key)).collect();
        let chosen = options.iter().position(|(v, _)| v == value).unwrap_or(0);
        ui_kit::form_row(ui, self.say(key), |ui| {
            if let Some(index) = ui_kit::segmented(ui, &names, chosen) {
                *value = options[index].0;
            }
        });
    }

    /// A number on a slider.
    pub(super) fn number(
        &self,
        ui: &mut egui::Ui,
        key: &'static str,
        value: &mut f32,
        range: std::ops::RangeInclusive<f32>,
        step: f32,
        suffix: &str,
    ) {
        ui_kit::form_row(ui, self.say(key), |ui| {
            ui_kit::slider(ui, value, range, step, suffix)
        });
    }

    /// A share from 0 to 1, shown in percent.
    pub(super) fn percent(&self, ui: &mut egui::Ui, key: &'static str, value: &mut f32) {
        let mut shown = (*value * 100.0).round();
        ui_kit::form_row(ui, self.say(key), |ui| {
            if ui_kit::slider(ui, &mut shown, 0.0..=100.0, 1.0, " %") {
                *value = shown / 100.0;
            }
        });
    }

    /// A colour.
    pub(super) fn colour(&self, ui: &mut egui::Ui, key: &'static str, colour: &mut Colour) {
        ui_kit::form_row(ui, self.say(key), |ui| {
            ui_kit::colour_well(ui, &mut colour.0)
        });
    }

    /// A line's colour: the object's own, or one for all.
    pub(super) fn line_colour(
        &self,
        ui: &mut egui::Ui,
        key: &'static str,
        colour: &mut LineColour,
    ) {
        let words = [self.say("ui.style.own"), self.say("ui.style.one_colour")];
        ui_kit::form_row(ui, self.say(key), |ui| {
            let mut one = matches!(colour, LineColour::One(_));
            if ui_kit::either(ui, &mut one, words) {
                *colour = if one {
                    LineColour::One(Colour::BLACK)
                } else {
                    LineColour::Object
                };
            }
            if let LineColour::One(chosen) = colour {
                ui_kit::colour_well(ui, &mut chosen.0);
            }
        });
    }

    /// Where faces take their colour from.
    pub(super) fn face_colour(&self, ui: &mut egui::Ui, colour: &mut ColourFrom) {
        let names = [
            self.say("ui.style.own"),
            self.say("ui.style.layer"),
            self.say("ui.style.material"),
            self.say("ui.style.one_colour"),
        ];
        let chosen = match colour {
            ColourFrom::Object => 0,
            ColourFrom::Layer => 1,
            ColourFrom::Material => 2,
            ColourFrom::One(_) => 3,
        };
        ui_kit::form_row(ui, self.say("ui.style.colour"), |ui| {
            if let Some(index) = ui_kit::segmented(ui, &names, chosen) {
                *colour = match index {
                    0 => ColourFrom::Object,
                    1 => ColourFrom::Layer,
                    2 => ColourFrom::Material,
                    _ => ColourFrom::One(Colour::WHITE),
                };
            }
        });
        if let ColourFrom::One(chosen) = colour {
            self.colour(ui, "ui.style.which_colour", chosen);
        }
    }

    /// How thick a line is: the object's own width times a factor, or fixed in millimetres.
    pub(super) fn width(&self, ui: &mut egui::Ui, key: &'static str, width: &mut LineWidth) {
        let words = [self.say("ui.style.own"), self.say("ui.style.fixed")];
        ui_kit::form_row(ui, self.say(key), |ui| {
            let mut fixed = matches!(width, LineWidth::Fixed(_));
            if ui_kit::either(ui, &mut fixed, words) {
                *width = if fixed {
                    LineWidth::Fixed(0.25)
                } else {
                    LineWidth::Object(1.0)
                };
            }
            match width {
                LineWidth::Object(factor) => ui_kit::slider(ui, factor, 0.0..=10.0, 0.05, " ×"),
                LineWidth::Fixed(mm) => ui_kit::slider(ui, mm, 0.0..=5.0, 0.05, " mm"),
            };
        });
    }

    /// A line's pattern.
    pub(super) fn pattern(&self, ui: &mut egui::Ui, pattern: &mut Pattern) {
        self.choice(
            ui,
            "ui.style.pattern",
            pattern,
            &[
                (Pattern::Solid, "ui.style.solid"),
                (Pattern::Dashed, "ui.style.dashed"),
                (Pattern::Dotted, "ui.style.dotted"),
            ],
        );
    }

    /// How a kind of object's outlines differ from the style's.
    pub(super) fn outline(&self, ui: &mut egui::Ui, outline: &mut OutlineChoice) {
        self.choice(
            ui,
            "ui.style.outline",
            outline,
            &[
                (OutlineChoice::AsStyle, "ui.style.as_style"),
                (OutlineChoice::Thicker, "ui.style.thicker"),
                (OutlineChoice::Off, "ui.style.off"),
            ],
        );
    }
}
