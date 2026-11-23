//! Style files (`.curvastyle`, §2.4): plain text, one setting a line under a heading for its
//! group, for example `opacity = 0.5` under `[nurbs.faces]`. Settings this version does not know
//! are kept and written back; a bad value keeps its default and is reported with its line; a bad
//! file never stops Curva.

use super::value::Value;
use super::*;

/// One setting: its full key in files, and how to write and read it.
pub(super) struct Field {
    /// Its group and name, `nurbs.faces.opacity`.
    pub(super) key: &'static str,
    get: fn(&ViewStyle) -> String,
    set: fn(&mut ViewStyle, &str) -> bool,
}

impl Field {
    /// The text `style`'s value of this setting is written as.
    pub(super) fn write(&self, style: &ViewStyle) -> String {
        (self.get)(style)
    }

    /// Sets this setting of `style` from `text`; false, changing nothing, if `text` is not a
    /// value it can take.
    pub(super) fn read(&self, style: &mut ViewStyle, text: &str) -> bool {
        (self.set)(style, text)
    }
}

/// A setting at `$($path).+`, written under `$key`; numbers may be held to a range.
macro_rules! field {
    ($key:literal, $($path:ident).+) => {
        Field {
            key: $key,
            get: |s| s.$($path).+.write(),
            set: |s, t| match Value::read(t) {
                Some(v) => { s.$($path).+ = v; true }
                None => false,
            },
        }
    };
    ($key:literal, $($path:ident).+, $low:literal ..= $high:literal) => {
        Field {
            key: $key,
            get: |s| s.$($path).+.write(),
            set: |s, t| match f32::read(t) {
                Some(v) if ($low..=$high).contains(&v) => { s.$($path).+ = v; true }
                _ => false,
            },
        }
    };
}

/// Every setting, in the order files list them.
pub(super) static FIELDS: &[Field] = &[
    field!("name", name),
    field!("engine", engine),
    field!("used for.views", used_for.views),
    field!("used for.plans", used_for.plans),
    field!("used for.sections", used_for.sections),
    field!("used for.sheets", used_for.sheets),
    field!("background.fill", background.fill),
    field!("background.top", background.top),
    field!("background.bottom", background.bottom),
    field!("background.grain", background.grain, 0.0..=1.0),
    field!("grid.shown", grid.shown),
    field!("grid.colour", grid.colour),
    field!("grid.over objects", grid.over_objects),
    field!("grid.axes", grid.axes),
    field!("grid.world axes", grid.world_axes),
    field!("grid.z axis", grid.z_axis),
    field!("lighting.vector", lighting.vector),
    field!("lighting.occlusion", lighting.occlusion),
    field!("lighting.rendered", lighting.rendered),
    field!("lighting.shine", lighting.shine, 0.0..=1.0),
    field!("lighting.ambient", lighting.ambient),
    field!("sun.compass", sun.compass, 0.0..=360.0),
    field!("sun.height", sun.height, 0.0..=90.0),
    field!("shadows.shown", shadows.shown),
    field!("shadows.edge", shadows.edge),
    field!("shadows.colour", shadows.colour),
    field!("shadows.strength", shadows.strength, 0.0..=1.0),
    field!("shadows.softness", shadows.softness, 0.0..=100.0),
    field!("shadows.ground only", shadows.ground_only),
    field!("ambient occlusion.shown", occlusion.shown),
    field!("ambient occlusion.strength", occlusion.strength, 0.0..=1.0),
    field!(
        "ambient occlusion.spread",
        occlusion.spread,
        0.0..=100_000.0
    ),
    field!("ambient occlusion.colour", occlusion.colour),
    field!("rendered.material from", rendered.material_from),
    field!("rendered.textures", rendered.textures),
    field!("rendered.environment", rendered.environment),
    field!("rendered.reflections", rendered.reflections, 0.0..=1.0),
    field!("rendered.exposure", rendered.exposure, -10.0..=10.0),
    field!("rendered.glow", rendered.glow),
    field!("finish.grain", finish.grain, 0.0..=1.0),
    field!("finish.vignette", finish.vignette, 0.0..=1.0),
    field!("finish.wash", finish.wash, 0.0..=1.0),
    field!("nurbs.faces.shown", nurbs.faces.shown),
    field!("nurbs.faces.colour", nurbs.faces.colour),
    field!("nurbs.faces.opacity", nurbs.faces.opacity, 0.0..=1.0),
    field!("nurbs.faces.back", nurbs.faces.back),
    field!("nurbs.faces.flat", nurbs.faces.flat),
    field!("nurbs.edges.shown", nurbs.edges.shown),
    field!("nurbs.edges.width", nurbs.edges.width),
    field!("nurbs.edges.colour", nurbs.edges.colour),
    field!("nurbs.edges.pattern", nurbs.edges.pattern),
    field!("nurbs.edges.tangent seams", nurbs.tangent_seams),
    field!("nurbs.edges.naked edges", nurbs.naked_edges),
    field!("nurbs.isocurves.shown", nurbs.isocurves.shown),
    field!("nurbs.isocurves.width", nurbs.isocurves.width),
    field!("nurbs.isocurves.colour", nurbs.isocurves.colour),
    field!("nurbs.isocurves.on flat faces", nurbs.isocurves_on_flat),
    field!("nurbs.outline", nurbs.outline),
    field!("meshes.same as nurbs", meshes.same_as_nurbs),
    field!("meshes.faces.shown", meshes.faces.shown),
    field!("meshes.faces.colour", meshes.faces.colour),
    field!("meshes.faces.opacity", meshes.faces.opacity, 0.0..=1.0),
    field!("meshes.faces.back", meshes.faces.back),
    field!("meshes.faces.flat", meshes.faces.flat),
    field!("meshes.edges.shown", meshes.edges.shown),
    field!("meshes.edges.width", meshes.edges.width),
    field!("meshes.edges.colour", meshes.edges.colour),
    field!("meshes.edges.pattern", meshes.edges.pattern),
    field!("meshes.outline", meshes.outline),
    field!("meshes.wires.shown", meshes.wires.shown),
    field!("meshes.wires.width", meshes.wires.width),
    field!("meshes.wires.colour", meshes.wires.colour),
    field!("meshes.vertices", meshes.vertices),
    field!("subd.same as nurbs", subd.same_as_nurbs),
    field!("subd.faces.shown", subd.faces.shown),
    field!("subd.faces.colour", subd.faces.colour),
    field!("subd.faces.opacity", subd.faces.opacity, 0.0..=1.0),
    field!("subd.faces.back", subd.faces.back),
    field!("subd.faces.flat", subd.faces.flat),
    field!("subd.outline", subd.outline),
    field!("subd.smooth edges.shown", subd.smooth.shown),
    field!("subd.smooth edges.width", subd.smooth.width),
    field!("subd.smooth edges.colour", subd.smooth.colour),
    field!("subd.creases.shown", subd.creases.shown),
    field!("subd.creases.width", subd.creases.width),
    field!("subd.creases.colour", subd.creases.colour),
    field!("subd.boundary.shown", subd.boundary.shown),
    field!("subd.boundary.width", subd.boundary.width),
    field!("subd.boundary.colour", subd.boundary.colour),
    field!("subd.reflected half", subd.reflected_half),
    field!("subd.symmetry axis", subd.symmetry_axis),
    field!("curves.shown", curves.shown),
    field!("curves.width", curves.width),
    field!("curves.colour", curves.colour),
    field!("curves.pattern", curves.pattern),
    field!("curves.ends", curves.ends),
    field!("curves.curvature graph", curves.curvature),
    field!("points.shown", points.shown),
    field!("points.size", points.size, 0.0..=50.0),
    field!("points.shape", points.shape),
    field!("points.clouds", points.clouds),
    field!("points.cloud size", points.cloud_size, 0.0..=50.0),
    field!("annotation.text", annotation.text),
    field!("annotation.dimensions", annotation.dimensions),
    field!("annotation.colour", annotation.colour),
    field!("lights.shown", lights.shown),
    field!("lights.light with them", lights.light_with_them),
    field!("clipping.planes shown", clipping.planes_shown),
    field!("clipping.fill", clipping.fill),
    field!("clipping.fill colour", clipping.fill_colour),
    field!("clipping.hatch", clipping.hatch),
    field!("clipping.cut edges", clipping.cut_edges),
    field!(
        "clipping.cut edge width",
        clipping.cut_edge_width,
        0.0..=20.0
    ),
    field!("outlines.shown", outlines.shown),
    field!("outlines.width", outlines.width, 0.0..=20.0),
    field!("outlines.colour", outlines.colour),
    field!("outlines.folds", outlines.folds),
    field!("outlines.where objects meet", outlines.where_objects_meet),
    field!("hidden lines.shown", hidden_lines.shown),
    field!("hidden lines.pattern", hidden_lines.pattern),
    field!("hidden lines.width", hidden_lines.width),
    field!("hidden lines.colour", hidden_lines.colour),
    field!("hidden lines.opacity", hidden_lines.opacity, 0.0..=1.0),
    field!("line rules.weight", line_rules.weight, 0.05..=10.0),
    field!("line rules.by distance", line_rules.by_distance),
    field!("line rules.near", line_rules.near, 0.0..=100_000.0),
    field!("line rules.far", line_rules.far, 0.0..=100_000.0),
    field!("line rules.far width", line_rules.far_width, 0.0..=1.0),
    field!("line rules.far fade", line_rules.far_fade, 0.0..=1.0),
    field!("cut.width", drawings.cut.width, 0.0..=20.0),
    field!("cut.colour", drawings.cut.colour),
    field!("cut.fill", drawings.cut.fill),
    field!("cut.fill colour", drawings.cut.fill_colour),
    field!("cut.hatch", drawings.cut.hatch),
    field!("cut.hatch scale", drawings.cut.hatch_scale, 0.01..=100.0),
    field!("cut.plan height", drawings.cut.plan_height, 0.0..=1000.0),
    field!("beyond.shown", drawings.beyond.shown),
    field!("beyond.width", drawings.beyond.width, 0.0..=20.0),
    field!("beyond.colour", drawings.beyond.colour),
    field!("beyond.fade", drawings.beyond.fade),
    field!("beyond.depth", drawings.beyond.depth, 0.0..=100_000.0),
    field!("beyond.faces", drawings.beyond.faces),
    field!("above.show", drawings.above),
    field!("above.colour", drawings.above_colour),
    field!("below.show", drawings.below),
    field!("behaviour.selection colour", behaviour.selection_colour),
    field!(
        "behaviour.selected show through",
        behaviour.selected_show_through
    ),
    field!("behaviour.locked", behaviour.locked),
    field!(
        "behaviour.locked opacity",
        behaviour.locked_opacity,
        0.0..=1.0
    ),
    field!("behaviour.boxes while moving", behaviour.boxes_while_moving),
    field!("behaviour.transparency", behaviour.transparency),
    field!("behaviour.highlights", behaviour.highlights),
];

/// A line of a style file that could not be used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// Its line number, from 1.
    pub line: usize,
    /// The line as written.
    pub text: String,
    /// What was wrong, in plain words.
    pub reason: String,
}

/// A style read from a file, and the lines that could not be used.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadStyle {
    /// The style: every setting the file gave a good value, the rest at Shaded's.
    pub style: ViewStyle,
    /// The lines that could not be used; their settings kept Shaded's values.
    pub problems: Vec<Problem>,
}

/// The heading a setting's key goes under and its name there: `nurbs.faces` and `opacity`.
fn split(key: &str) -> (&str, &str) {
    key.rsplit_once('.').unwrap_or(("", key))
}

impl ViewStyle {
    /// The style as the text of a `.curvastyle` file.
    #[must_use]
    pub fn to_text(&self) -> String {
        let known = FIELDS
            .iter()
            .map(|field| (field.key.to_owned(), field.write(self)));
        // Settings this version does not know go last, as they were read.
        let all = known.chain(self.unknown.iter().cloned());
        let mut out = String::from("# Curva view style\n");
        let mut heading = String::new();
        for (key, value) in all {
            let (group, name) = split(&key);
            if group != heading {
                out.push_str(&format!("\n[{group}]\n"));
                heading = group.to_owned();
            }
            // A value is one line.
            let value = value.replace(['\n', '\r'], " ");
            out.push_str(&format!("{name} = {value}\n"));
        }
        out
    }

    /// Reads the text of a `.curvastyle` file. Lines it cannot use are reported and change
    /// nothing; settings it does not know are kept in [`ViewStyle::unknown`].
    #[must_use]
    pub fn from_text(text: &str) -> ReadStyle {
        let mut style = Self::shaded();
        let mut problems = Vec::new();
        let mut heading = String::new();
        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(inner) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                heading = inner.trim().to_ascii_lowercase();
                continue;
            }
            let problem = |reason: &str| Problem {
                line: index + 1,
                text: raw.to_owned(),
                reason: reason.to_owned(),
            };
            let Some((name, value)) = line.split_once('=') else {
                problems.push(problem("not a setting: a setting is written name = value"));
                continue;
            };
            let name = name.trim().to_ascii_lowercase();
            let key = if heading.is_empty() {
                name
            } else {
                format!("{heading}.{name}")
            };
            let value = value.trim();
            match FIELDS.iter().find(|field| field.key == key) {
                Some(field) if !field.read(&mut style, value) => {
                    problems.push(problem(
                        "not a value this setting can take; its default is kept",
                    ));
                }
                Some(_) => {}
                None => style.unknown.push((key, value.to_owned())),
            }
        }
        ReadStyle { style, problems }
    }
}
