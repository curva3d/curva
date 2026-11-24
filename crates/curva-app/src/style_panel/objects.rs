//! The Style panel's Objects tab: each kind of object's look. Meshes and SubD start as Same as
//! NURBS, one switch; turned off, their own faces, edges and outlines show (spec §2.5).

use super::ObjectKind;
use super::fields::Form;
use crate::ui_kit;
use curva_document::style::{
    BackFaces, Colour, Edges, Faces, Hatch, LineColour, LineEnds, LineLook, PointShape, ViewStyle,
    WireWidth,
};
use eframe::egui;

/// The kinds, in the order they are offered, with their names' keys.
const KINDS: [(ObjectKind, &str); 7] = [
    (ObjectKind::Nurbs, "ui.style.kind.nurbs"),
    (ObjectKind::Meshes, "ui.style.kind.meshes"),
    (ObjectKind::Subd, "ui.style.kind.subd"),
    (ObjectKind::Curves, "ui.style.kind.curves"),
    (ObjectKind::Points, "ui.style.kind.points"),
    (ObjectKind::Text, "ui.style.kind.text"),
    (ObjectKind::Other, "ui.style.kind.other"),
];

/// The Objects tab: a choice of kind, then its settings.
pub(super) fn tab(
    ui: &mut egui::Ui,
    form: &mut Form,
    kind: &mut ObjectKind,
    style: &mut ViewStyle,
) {
    let names: Vec<String> = KINDS.iter().map(|(_, key)| form.say(key)).collect();
    let chosen = KINDS.iter().position(|(k, _)| k == kind).unwrap_or(0);
    ui_kit::chip_row(ui, |ui| {
        if let Some(index) = ui_kit::segmented(ui, &names, chosen) {
            *kind = KINDS[index].0;
        }
    });
    match kind {
        ObjectKind::Nurbs => nurbs(ui, form, style),
        ObjectKind::Meshes => meshes(ui, form, style),
        ObjectKind::Subd => subd(ui, form, style),
        ObjectKind::Curves => curves(ui, form, style),
        ObjectKind::Points => points(ui, form, style),
        ObjectKind::Text => text(ui, form, style),
        ObjectKind::Other => other(ui, form, style),
    }
}

/// A kind's faces: their colour and opacity, then whether they show, back faces and flat
/// shading under More.
fn faces(ui: &mut egui::Ui, form: &mut Form, id: &'static str, faces: &mut Faces) {
    let more = form.section(ui, id, 3);
    form.face_colour(ui, &mut faces.colour);
    form.percent(ui, "ui.style.opacity", &mut faces.opacity);
    if !more {
        return;
    }
    form.switch(ui, "ui.style.show_faces", &mut faces.shown);
    let mut back = match faces.back {
        BackFaces::AsFront => 0,
        BackFaces::One(_) => 1,
        BackFaces::Hidden => 2,
    };
    form.choice(
        ui,
        "ui.style.back_faces",
        &mut back,
        &[
            (0, "ui.style.as_front"),
            (1, "ui.style.other_colour"),
            (2, "ui.style.hide"),
        ],
    );
    faces.back = match (back, faces.back) {
        (1, BackFaces::One(colour)) => BackFaces::One(colour),
        (1, _) => BackFaces::One(Colour([200, 120, 120])),
        (2, _) => BackFaces::Hidden,
        _ => BackFaces::AsFront,
    };
    if let BackFaces::One(colour) = &mut faces.back {
        form.colour(ui, "ui.style.back_colour", colour);
    }
    form.switch(ui, "ui.style.flat_shading", &mut faces.flat);
}

/// A kind's edges: whether they show and how thick, then colour and pattern under More.
fn edges(ui: &mut egui::Ui, form: &mut Form, id: &'static str, edges: &mut Edges) -> bool {
    let more = form.section(ui, id, 2);
    form.switch(ui, "ui.style.show_edges", &mut edges.shown);
    if edges.shown {
        form.width(ui, "ui.style.width", &mut edges.width);
    }
    if more {
        form.line_colour(ui, "ui.style.colour", &mut edges.colour);
        form.pattern(ui, &mut edges.pattern);
    }
    more
}

/// A line of its own, such as isocurves or SubD creases: whether it shows, then its width and
/// colour (its own, or the edges').
fn own_line(ui: &mut egui::Ui, form: &Form, key: &'static str, line: &mut LineLook, more: bool) {
    form.switch(ui, key, &mut line.shown);
    if more && line.shown {
        form.width(ui, "ui.style.width", &mut line.width);
        let mut own = line.colour.is_some();
        form.switch(ui, "ui.style.own_line_colour", &mut own);
        match (own, &mut line.colour) {
            (true, None) => line.colour = Some(LineColour::Object),
            (false, Some(_)) => line.colour = None,
            (true, Some(colour)) => form.line_colour(ui, "ui.style.colour", colour),
            (false, None) => {}
        }
    }
}

/// Surfaces, polysurfaces and solids.
fn nurbs(ui: &mut egui::Ui, form: &mut Form, style: &mut ViewStyle) {
    let look = &mut style.nurbs;
    faces(ui, form, "ui.style.faces", &mut look.faces);
    if edges(ui, form, "ui.style.edges", &mut look.edges) {
        form.switch(ui, "ui.style.tangent_seams", &mut look.tangent_seams);
        form.switch(ui, "ui.style.naked_edges", &mut look.naked_edges);
        form.outline(ui, &mut look.outline);
    }
    let more = form.section(ui, "ui.style.isocurves", 3);
    own_line(
        ui,
        form,
        "ui.style.show_isocurves",
        &mut look.isocurves,
        more,
    );
    if more {
        form.switch(ui, "ui.style.isocurves_flat", &mut look.isocurves_on_flat);
    }
}

/// Meshes: Same as NURBS, or their own faces and edges; their wires always their own.
fn meshes(ui: &mut egui::Ui, form: &mut Form, style: &mut ViewStyle) {
    let look = &mut style.meshes;
    form.section(ui, "ui.style.faces_and_edges", 0);
    form.switch(ui, "ui.style.same_as_nurbs", &mut look.same_as_nurbs);
    if !look.same_as_nurbs {
        faces(ui, form, "ui.style.mesh_faces", &mut look.faces);
        if edges(ui, form, "ui.style.mesh_edges", &mut look.edges) {
            form.outline(ui, &mut look.outline);
        }
    }
    let more = form.section(ui, "ui.style.wires", 3);
    let wires = &mut look.wires;
    form.switch(ui, "ui.style.show_wires", &mut wires.shown);
    if more {
        form.choice(
            ui,
            "ui.style.width",
            &mut wires.width,
            &[
                (WireWidth::OnePixel, "ui.style.one_pixel"),
                (WireWidth::AsEdges, "ui.style.as_edges"),
            ],
        );
        let mut own = wires.colour.is_some();
        form.switch(ui, "ui.style.own_line_colour", &mut own);
        match (own, &mut wires.colour) {
            (true, None) => wires.colour = Some(LineColour::Object),
            (false, Some(_)) => wires.colour = None,
            (true, Some(colour)) => form.line_colour(ui, "ui.style.colour", colour),
            (false, None) => {}
        }
        form.switch(ui, "ui.style.vertices", &mut look.vertices);
    }
}

/// SubD objects: Same as NURBS, or their own faces; their smooth edges, creases and
/// boundaries always their own.
fn subd(ui: &mut egui::Ui, form: &mut Form, style: &mut ViewStyle) {
    let look = &mut style.subd;
    form.section(ui, "ui.style.faces_and_edges", 0);
    form.switch(ui, "ui.style.same_as_nurbs", &mut look.same_as_nurbs);
    if !look.same_as_nurbs {
        faces(ui, form, "ui.style.subd_faces", &mut look.faces);
        form.outline(ui, &mut look.outline);
    }
    let more = form.section(ui, "ui.style.subd_lines", 3);
    own_line(ui, form, "ui.style.creases", &mut look.creases, more);
    own_line(ui, form, "ui.style.smooth_edges", &mut look.smooth, more);
    if more {
        own_line(ui, form, "ui.style.boundaries", &mut look.boundary, more);
        form.switch(ui, "ui.style.reflected_half", &mut look.reflected_half);
        form.switch(ui, "ui.style.symmetry_axis", &mut look.symmetry_axis);
    }
}

/// Curves.
fn curves(ui: &mut egui::Ui, form: &mut Form, style: &mut ViewStyle) {
    let look = &mut style.curves;
    let more = form.section(ui, "ui.style.kind.curves", 4);
    form.switch(ui, "ui.style.show_curves", &mut look.shown);
    form.width(ui, "ui.style.width", &mut look.width);
    if !more {
        return;
    }
    form.line_colour(ui, "ui.style.colour", &mut look.colour);
    form.pattern(ui, &mut look.pattern);
    form.choice(
        ui,
        "ui.style.ends",
        &mut look.ends,
        &[
            (LineEnds::Round, "ui.style.round"),
            (LineEnds::Flat, "ui.style.flat"),
            (LineEnds::Square, "ui.style.square"),
        ],
    );
    form.switch(ui, "ui.style.curvature", &mut look.curvature);
}

/// Points and point clouds.
fn points(ui: &mut egui::Ui, form: &mut Form, style: &mut ViewStyle) {
    let look = &mut style.points;
    let more = form.section(ui, "ui.style.kind.points", 3);
    form.switch(ui, "ui.style.show_points", &mut look.shown);
    form.number(ui, "ui.style.size", &mut look.size, 1.0..=20.0, 1.0, " px");
    if !more {
        return;
    }
    form.choice(
        ui,
        "ui.style.shape",
        &mut look.shape,
        &[
            (PointShape::Square, "ui.style.square"),
            (PointShape::Round, "ui.style.round"),
            (PointShape::Cross, "ui.style.cross"),
        ],
    );
    form.switch(ui, "ui.style.clouds", &mut look.clouds);
    form.number(
        ui,
        "ui.style.cloud_size",
        &mut look.cloud_size,
        1.0..=20.0,
        1.0,
        " px",
    );
}

/// Text and dimensions.
fn text(ui: &mut egui::Ui, form: &mut Form, style: &mut ViewStyle) {
    let look = &mut style.annotation;
    let more = form.section(ui, "ui.style.kind.text", 1);
    form.switch(ui, "ui.style.show_text", &mut look.text);
    form.switch(ui, "ui.style.show_dimensions", &mut look.dimensions);
    if more {
        form.line_colour(ui, "ui.style.colour", &mut look.colour);
    }
}

/// Lights and clipping planes.
fn other(ui: &mut egui::Ui, form: &mut Form, style: &mut ViewStyle) {
    let more = form.section(ui, "ui.style.lights", 1);
    form.switch(ui, "ui.style.show_lights", &mut style.lights.shown);
    if more {
        form.switch(
            ui,
            "ui.style.light_with_them",
            &mut style.lights.light_with_them,
        );
    }
    let clipping = &mut style.clipping;
    let more = form.section(ui, "ui.style.clipping", 4);
    form.switch(ui, "ui.style.show_planes", &mut clipping.planes_shown);
    form.switch(ui, "ui.style.cut_fill", &mut clipping.fill);
    if !more {
        return;
    }
    if clipping.fill {
        form.line_colour(ui, "ui.style.fill_colour", &mut clipping.fill_colour);
        hatch(ui, form, &mut clipping.hatch);
    }
    form.switch(ui, "ui.style.cut_edges", &mut clipping.cut_edges);
    if clipping.cut_edges {
        form.number(
            ui,
            "ui.style.width",
            &mut clipping.cut_edge_width,
            0.0..=5.0,
            0.05,
            " mm",
        );
    }
}

/// A hatch pattern.
pub(super) fn hatch(ui: &mut egui::Ui, form: &Form, hatch: &mut Hatch) {
    form.choice(
        ui,
        "ui.style.hatch",
        hatch,
        &[
            (Hatch::Plain, "ui.style.plain"),
            (Hatch::Diagonal, "ui.style.diagonal"),
            (Hatch::Cross, "ui.style.cross"),
            (Hatch::Concrete, "ui.style.concrete"),
        ],
    );
}
