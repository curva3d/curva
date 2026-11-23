//! The mesh panel (spec `docs/specs/export-formats.md` §2), shared by `Mesh` and every export that
//! meshes: Rhino's presets, its *Fewer polygons ↔ More polygons* slider, the SubD level when SubDs
//! are meshed, and under the slider what the settings give in plain numbers, which Rhino does not
//! show. Advanced adds the most a facet may turn and stand off the surface. It opens when the
//! running command's step asks for it (`Panel::MeshSettings`, #387); OK keeps the settings and
//! answers the step, Cancel ends the command. The live preview is display's (#407).

use crate::*;
use curva_document::meshing::{MeshNumbers, mesh_objects, remember_settings, remembered_settings};
use curva_mesh::{MOST_SUBD_LEVEL, MeshPreset, MeshSettings};

/// The panel's state while it is open, and the Advanced switch, kept between openings.
#[derive(Clone, Default)]
pub(crate) struct MeshPanel {
    /// The settings being edited; `None` while the panel is closed.
    settings: Option<MeshSettings>,
    /// Whether the advanced fields show.
    advanced: bool,
    /// The numbers for the last settings shown, so meshing runs only when they change.
    numbers: Option<(MeshSettings, MeshNumbers)>,
}

/// Where egui keeps the panel's state between frames.
fn state_id() -> egui::Id {
    egui::Id::NULL.with("mesh panel state")
}

/// What the panel's buttons asked for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Answer {
    Waiting,
    Ok,
    Cancel,
}

/// A count to three figures, with thousands marked: 12,437 → "12,400".
fn about(count: usize) -> String {
    let digits = count.to_string().len();
    let step = 10_usize.pow(digits.saturating_sub(3) as u32);
    let rounded = (count + step / 2) / step * step;
    let text = rounded.to_string();
    let mut out = String::new();
    for (k, c) in text.chars().enumerate() {
        if k > 0 && (text.len() - k).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The line under the slider: *about 12,400 faces · strays at most 0.04 mm · about 0.6 MB*. The
/// size is a binary STL's, 50 bytes a triangle, counting a face as two triangles.
fn numbers_line(numbers: &MeshNumbers, unit: &str) -> (String, String, String) {
    let megabytes = numbers.faces as f64 * 2.0 * 50.0 / 1_000_000.0;
    (
        about(numbers.faces),
        if numbers.strays > 0.0 {
            format!("{:.2} {unit}", numbers.strays)
        } else {
            String::new()
        },
        format!("{megabytes:.1}"),
    )
}

impl Curva {
    /// The objects the panel's numbers are for: those selected, or every object when none is.
    fn mesh_panel_objects(&self) -> Vec<u64> {
        let chosen = self.doc.selection();
        if chosen.is_empty() {
            self.doc.state().objects.iter().map(|o| o.id).collect()
        } else {
            chosen
        }
    }

    /// The line under the slider for `settings`, meshing again only when they changed, and
    /// whether SubDs are among the objects.
    fn mesh_panel_numbers(&self, state: &mut MeshPanel, settings: MeshSettings) -> (String, bool) {
        let ids = self.mesh_panel_objects();
        let numbers = match state.numbers {
            Some((shown, numbers)) if shown == settings => numbers,
            _ => {
                let numbers = mesh_objects(&self.doc, &ids, &settings).1;
                state.numbers = Some((settings, numbers));
                numbers
            }
        };
        let subds = self
            .doc
            .state()
            .objects
            .iter()
            .any(|o| ids.contains(&o.id) && matches!(o.shape, Shape::SubD(_)));
        let (faces, strays, size) = numbers_line(&numbers, self.doc.state().unit.symbol());
        let key = if strays.is_empty() {
            "ui.mesh.numbers_exact"
        } else {
            "ui.mesh.numbers"
        };
        let text = self
            .text
            .render(&Message::with(key, vec![faces, strays, size]));
        (text, subds)
    }

    /// Shows the mesh panel while the running command asks for it.
    pub(crate) fn mesh_panel(&mut self, ctx: &egui::Context) {
        let asked = self
            .processor
            .current_request()
            .is_some_and(|r| r.panel == Some(curva_commands::Panel::MeshSettings));
        if !asked {
            ctx.data_mut(|d| d.get_temp_mut_or_default::<MeshPanel>(state_id()).settings = None);
            return;
        }
        let mut state: MeshPanel = ctx.data_mut(|d| d.get_temp(state_id())).unwrap_or_default();
        let mut settings = *state
            .settings
            .get_or_insert_with(|| remembered_settings(&self.doc));
        let (numbers_text, subds) = self.mesh_panel_numbers(&mut state, settings);
        let unit = self.doc.state().unit.symbol();
        let mut advanced = state.advanced;
        let say = |key: &'static str| self.text.render(&Message::new(key));
        let buttons = [
            ui_kit::main_choice(say("ui.mesh.ok")),
            ui_kit::other_choice(say("ui.mesh.cancel")),
        ];
        let clicked = ui_kit::dialog(ctx, "mesh panel", &say("ui.mesh.title"), &buttons, |ui| {
            presets(ui, &mut settings, &say);
            ui.horizontal(|ui| {
                ui.label(say("ui.mesh.fewer"));
                ui.add(egui::Slider::new(&mut settings.density, 0.0..=1.0).show_value(false));
                ui.label(say("ui.mesh.more"));
            });
            if subds {
                ui.add(
                    egui::Slider::new(&mut settings.subd_level, 1..=MOST_SUBD_LEVEL)
                        .text(say("ui.mesh.subd_level")),
                );
            }
            ui.label(numbers_text);
            ui_kit::toggle(ui, &mut advanced, say("ui.mesh.advanced"));
            if advanced {
                advanced_fields(ui, &mut settings, unit, &say);
            }
        });
        let answer = match clicked {
            Some(0) => Answer::Ok,
            Some(_) => Answer::Cancel,
            None => Answer::Waiting,
        };
        state.advanced = advanced;
        state.settings = Some(settings);
        match answer {
            Answer::Ok => {
                state.settings = None;
                remember_settings(&mut self.doc, &settings);
                self.processor.submit(&mut self.doc, "");
            }
            Answer::Cancel => {
                state.settings = None;
                self.processor.cancel(&mut self.doc);
            }
            Answer::Waiting => {}
        }
        ctx.data_mut(|d| d.insert_temp(state_id(), state));
    }
}

/// Rhino's ready-made settings, as buttons.
fn presets(ui: &mut egui::Ui, settings: &mut MeshSettings, say: &dyn Fn(&'static str) -> String) {
    ui.horizontal(|ui| {
        for (preset, key) in [
            (MeshPreset::Coarse, "ui.mesh.coarse"),
            (MeshPreset::Default, "ui.mesh.default"),
            (MeshPreset::Smooth, "ui.mesh.smooth"),
        ] {
            let chosen = MeshSettings {
                subd_level: settings.subd_level,
                ..MeshSettings::preset(preset)
            };
            if ui_kit::choice(ui, *settings == chosen, say(key)).clicked() {
                *settings = chosen;
            }
        }
    });
}

/// The most a facet may turn and the farthest it may stand off the surface, each off until
/// ticked.
fn advanced_fields(
    ui: &mut egui::Ui,
    settings: &mut MeshSettings,
    unit: &str,
    say: &dyn Fn(&'static str) -> String,
) {
    let limit = |ui: &mut egui::Ui, value: &mut Option<f64>, key, start, suffix: String| {
        ui.horizontal(|ui| {
            let mut on = value.is_some();
            ui_kit::toggle(ui, &mut on, say(key));
            match (on, value.as_mut()) {
                (true, Some(v)) => {
                    ui.add(
                        egui::DragValue::new(v)
                            .range(0.0001..=f64::MAX)
                            .speed(0.01)
                            .suffix(suffix),
                    );
                }
                (true, None) => *value = Some(start),
                (false, _) => *value = None,
            }
        });
    };
    limit(
        ui,
        &mut settings.max_angle,
        "ui.mesh.max_angle",
        20.0,
        "°".to_owned(),
    );
    limit(
        ui,
        &mut settings.max_distance,
        "ui.mesh.max_distance",
        0.1,
        format!(" {unit}"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_are_given_to_three_figures() {
        assert_eq!(about(0), "0");
        assert_eq!(about(987), "987");
        assert_eq!(about(12_437), "12,400");
        assert_eq!(about(1_234_567), "1,230,000");
    }

    #[test]
    fn the_line_under_the_slider_gives_faces_strays_and_size() {
        let numbers = MeshNumbers {
            faces: 12_437,
            strays: 0.04,
            left_out: 0,
        };
        assert_eq!(
            numbers_line(&numbers, "mm"),
            ("12,400".to_owned(), "0.04 mm".to_owned(), "1.2".to_owned())
        );
        let exact = MeshNumbers {
            faces: 10,
            strays: 0.0,
            left_out: 0,
        };
        assert!(numbers_line(&exact, "mm").1.is_empty());
    }
}
