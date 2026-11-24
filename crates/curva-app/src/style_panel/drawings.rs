//! The Style panel's Plans and sections tab: how live plans, sections and elevations draw what
//! is cut, what lies beyond the cut, and what is above and below it (spec §2.5).

use super::fields::Form;
use curva_document::style::{Above, Beyond, Cut, CutFill, Drawings};
use eframe::egui;

/// The Plans and sections tab.
pub(super) fn tab(ui: &mut egui::Ui, form: &mut Form, drawings: &mut Drawings) {
    cut(ui, form, &mut drawings.cut);
    beyond(ui, form, &mut drawings.beyond);
    let more = form.section(ui, "ui.style.above_below", 2);
    form.choice(
        ui,
        "ui.style.above",
        &mut drawings.above,
        &[
            (Above::Hide, "ui.style.hide"),
            (Above::Dashed, "ui.style.dashed"),
            (Above::Solid, "ui.style.solid"),
        ],
    );
    if more {
        form.colour(ui, "ui.style.above_colour", &mut drawings.above_colour);
        form.switch(ui, "ui.style.below", &mut drawings.below);
    }
}

/// Where things are cut: the fill and the cut line.
fn cut(ui: &mut egui::Ui, form: &mut Form, cut: &mut Cut) {
    let more = form.section(ui, "ui.style.cut", 5);
    form.choice(
        ui,
        "ui.style.fill",
        &mut cut.fill,
        &[
            (CutFill::Solid, "ui.style.solid"),
            (CutFill::Hatch, "ui.style.hatch"),
            (CutFill::ObjectColour, "ui.style.own"),
            (CutFill::Empty, "ui.style.none"),
        ],
    );
    let mm = " mm";
    form.number(ui, "ui.style.cut_line", &mut cut.width, 0.0..=5.0, 0.05, mm);
    if !more {
        return;
    }
    if matches!(cut.fill, CutFill::Solid | CutFill::Hatch) {
        form.colour(ui, "ui.style.fill_colour", &mut cut.fill_colour);
    }
    if cut.fill == CutFill::Hatch {
        super::objects::hatch(ui, form, &mut cut.hatch);
        let scale = &mut cut.hatch_scale;
        form.number(ui, "ui.style.hatch_scale", scale, 0.1..=10.0, 0.1, " ×");
    }
    form.colour(ui, "ui.style.line_colour", &mut cut.colour);
    let height = &mut cut.plan_height;
    form.number(ui, "ui.style.plan_height", height, 0.0..=10.0, 0.05, " m");
}

/// What lies beyond the cut.
fn beyond(ui: &mut egui::Ui, form: &mut Form, beyond: &mut Beyond) {
    let more = form.section(ui, "ui.style.beyond", 4);
    form.switch(ui, "ui.style.show_beyond", &mut beyond.shown);
    if beyond.shown {
        form.switch(ui, "ui.style.fade", &mut beyond.fade);
    }
    if more {
        form.number(
            ui,
            "ui.style.width",
            &mut beyond.width,
            0.0..=5.0,
            0.05,
            " mm",
        );
        form.colour(ui, "ui.style.colour", &mut beyond.colour);
        form.number(
            ui,
            "ui.style.depth",
            &mut beyond.depth,
            0.0..=100.0,
            1.0,
            " m",
        );
        form.switch(ui, "ui.style.beyond_faces", &mut beyond.faces);
    }
}
