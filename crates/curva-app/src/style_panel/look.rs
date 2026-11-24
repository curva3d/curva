//! The Style panel's Look tab: the background, light and shadows, lines, the finish, how
//! selected and locked objects show, and what the style is used for. Settings an engine does not
//! use are kept but not shown (spec §2.5).

use super::fields::Form;
use crate::ui_kit;
use curva_document::style::{
    Background, Colour, Engine, Environment, Fill, Locked, MaterialFrom, OcclusionLight,
    RenderedLight, ShadowEdge, VectorFaces, ViewStyle,
};
use eframe::egui;

/// The backgrounds offered at once, as in the mock-up: the window theme's, then four of the
/// mock-up's colours.
const BACKGROUNDS: [(&str, Background); 5] = [
    (
        "ui.style.bg.theme",
        Background {
            fill: Fill::Theme,
            top: Colour([43, 47, 54]),
            bottom: Colour([27, 29, 34]),
            grain: 0.0,
        },
    ),
    (
        "ui.style.bg.grey",
        Background {
            fill: Fill::Gradient,
            top: Colour([201, 205, 210]),
            bottom: Colour([156, 162, 169]),
            grain: 0.0,
        },
    ),
    (
        "ui.style.bg.white",
        Background {
            fill: Fill::OneColour,
            top: Colour::WHITE,
            bottom: Colour::WHITE,
            grain: 0.0,
        },
    ),
    (
        "ui.style.bg.sky",
        Background {
            fill: Fill::Gradient,
            top: Colour([170, 204, 232]),
            bottom: Colour([184, 184, 179]),
            grain: 0.0,
        },
    ),
    (
        "ui.style.bg.paper",
        Background {
            fill: Fill::Paper,
            top: Colour([239, 233, 223]),
            bottom: Colour([239, 233, 223]),
            grain: 0.3,
        },
    ),
];

/// How surfaces are drawn: the engine, with a line on what it is for.
pub(super) fn engine(ui: &mut egui::Ui, form: &Form, style: &mut ViewStyle) {
    ui_kit::section(ui, form.say("ui.style.engine"));
    let engines = [
        (Engine::Vector, "ui.style.vector"),
        (Engine::Occlusion, "ui.style.occlusion"),
        (Engine::Rendered, "ui.style.rendered"),
    ];
    let names: Vec<String> = engines.iter().map(|(_, key)| form.say(key)).collect();
    let chosen = engines.iter().position(|(e, _)| *e == style.engine);
    if let Some(index) = ui_kit::segmented(ui, &names, chosen.unwrap_or(0)) {
        style.engine = engines[index].0;
    }
    let note = match style.engine {
        Engine::Vector => "ui.style.vector_note",
        Engine::Occlusion => "ui.style.occlusion_note",
        Engine::Rendered => "ui.style.rendered_note",
    };
    ui.label(ui_kit::caption(form.say(note)));
}

/// The Look tab.
pub(super) fn tab(ui: &mut egui::Ui, form: &mut Form, style: &mut ViewStyle) {
    background(ui, form, style);
    light(ui, form, style);
    lines(ui, form, style);
    finish(ui, form, style);
    behaviour(ui, form, style);
    used_for(ui, form, style);
}

/// Behind the model, and the grid.
fn background(ui: &mut egui::Ui, form: &mut Form, style: &mut ViewStyle) {
    let more = form.section(ui, "ui.style.background", 7);
    let names: Vec<String> = BACKGROUNDS.iter().map(|(key, _)| form.say(key)).collect();
    let chosen = BACKGROUNDS
        .iter()
        .position(|(_, b)| {
            b.fill == style.background.fill
                && (b.fill == Fill::Theme || b.top == style.background.top)
        })
        .unwrap_or(usize::MAX);
    ui_kit::form_row(ui, form.say("ui.style.behind"), |ui| {
        if let Some(index) = ui_kit::segmented(ui, &names, chosen) {
            style.background = BACKGROUNDS[index].1;
        }
    });
    form.switch(ui, "ui.style.grid", &mut style.grid.shown);
    if !more {
        return;
    }
    let background = &mut style.background;
    form.choice(
        ui,
        "ui.style.fill",
        &mut background.fill,
        &[
            (Fill::Theme, "ui.style.bg.theme"),
            (Fill::OneColour, "ui.style.one_colour"),
            (Fill::Gradient, "ui.style.gradient"),
            (Fill::Paper, "ui.style.bg.paper"),
        ],
    );
    if background.fill != Fill::Theme {
        form.colour(ui, "ui.style.top", &mut background.top);
    }
    if background.fill == Fill::Gradient {
        form.colour(ui, "ui.style.bottom", &mut background.bottom);
    }
    if background.fill == Fill::Paper {
        form.percent(ui, "ui.style.grain", &mut background.grain);
    }
    let grid = &mut style.grid;
    form.switch(ui, "ui.style.grid_axes", &mut grid.axes);
    form.switch(ui, "ui.style.world_axes", &mut grid.world_axes);
    form.switch(ui, "ui.style.grid_over", &mut grid.over_objects);
}

/// The light for the style's engine, and shadows.
fn light(ui: &mut egui::Ui, form: &mut Form, style: &mut ViewStyle) {
    let more = form.section(ui, "ui.style.light", 6);
    let lighting = &mut style.lighting;
    match style.engine {
        Engine::Vector => form.choice(
            ui,
            "ui.style.faces",
            &mut lighting.vector,
            &[
                (VectorFaces::Flat, "ui.style.flat"),
                (VectorFaces::Tones, "ui.style.tones"),
            ],
        ),
        Engine::Occlusion => form.choice(
            ui,
            "ui.style.light_from",
            &mut lighting.occlusion,
            &[
                (OcclusionLight::Studio, "ui.style.studio"),
                (OcclusionLight::Sun, "ui.style.sun"),
                (OcclusionLight::Flat, "ui.style.flat"),
            ],
        ),
        Engine::Rendered => form.choice(
            ui,
            "ui.style.light_from",
            &mut lighting.rendered,
            &[
                (RenderedLight::SunAndSky, "ui.style.sun_and_sky"),
                (RenderedLight::Studio, "ui.style.studio"),
                (RenderedLight::ModelLights, "ui.style.model_lights"),
            ],
        ),
    }
    form.switch(ui, "ui.style.shadows", &mut style.shadows.shown);
    if style.engine == Engine::Occlusion {
        form.switch(ui, "ui.style.occlusion_shadows", &mut style.occlusion.shown);
    }
    if !more {
        return;
    }
    form.number(
        ui,
        "ui.style.sun_compass",
        &mut style.sun.compass,
        0.0..=360.0,
        1.0,
        "°",
    );
    form.number(
        ui,
        "ui.style.sun_height",
        &mut style.sun.height,
        0.0..=90.0,
        1.0,
        "°",
    );
    shadow_details(ui, form, style);
    if style.engine != Engine::Vector {
        form.percent(ui, "ui.style.shine", &mut style.lighting.shine);
        form.colour(ui, "ui.style.ambient", &mut style.lighting.ambient);
    }
    if style.engine == Engine::Occlusion && style.occlusion.shown {
        form.percent(
            ui,
            "ui.style.occlusion_strength",
            &mut style.occlusion.strength,
        );
    }
    if style.engine == Engine::Rendered {
        rendered(ui, form, style);
    }
}

/// The shadows' strength, edge and colour.
fn shadow_details(ui: &mut egui::Ui, form: &Form, style: &mut ViewStyle) {
    let shadows = &mut style.shadows;
    form.percent(ui, "ui.style.shadow_strength", &mut shadows.strength);
    form.choice(
        ui,
        "ui.style.shadow_edge",
        &mut shadows.edge,
        &[
            (ShadowEdge::Crisp, "ui.style.crisp"),
            (ShadowEdge::Soft, "ui.style.soft"),
        ],
    );
    if shadows.edge == ShadowEdge::Soft {
        form.number(
            ui,
            "ui.style.softness",
            &mut shadows.softness,
            0.0..=100.0,
            1.0,
            "",
        );
    }
    form.colour(ui, "ui.style.shadow_colour", &mut shadows.colour);
    form.switch(ui, "ui.style.ground_only", &mut shadows.ground_only);
}

/// The Rendered engine's materials, environment and exposure.
fn rendered(ui: &mut egui::Ui, form: &Form, style: &mut ViewStyle) {
    let rendered = &mut style.rendered;
    form.choice(
        ui,
        "ui.style.material_from",
        &mut rendered.material_from,
        &[
            (MaterialFrom::Material, "ui.style.material"),
            (MaterialFrom::Layer, "ui.style.layer"),
            (MaterialFrom::ObjectColour, "ui.style.own"),
        ],
    );
    form.choice(
        ui,
        "ui.style.environment",
        &mut rendered.environment,
        &[
            (Environment::Studio, "ui.style.studio"),
            (Environment::Sky, "ui.style.sky"),
            (Environment::Plain, "ui.style.plain"),
        ],
    );
    form.percent(ui, "ui.style.reflections", &mut rendered.reflections);
    form.number(
        ui,
        "ui.style.exposure",
        &mut rendered.exposure,
        -5.0..=5.0,
        0.1,
        "",
    );
    form.switch(ui, "ui.style.textures", &mut rendered.textures);
}

/// How heavy lines are, outlines, and lines hidden behind faces.
fn lines(ui: &mut egui::Ui, form: &mut Form, style: &mut ViewStyle) {
    let more = form.section(ui, "ui.style.lines", 6);
    form.number(
        ui,
        "ui.style.weight",
        &mut style.line_rules.weight,
        0.1..=5.0,
        0.05,
        " ×",
    );
    form.switch(ui, "ui.style.outlines", &mut style.outlines.shown);
    if !more {
        return;
    }
    let outlines = &mut style.outlines;
    if outlines.shown {
        form.number(
            ui,
            "ui.style.outline_width",
            &mut outlines.width,
            0.0..=5.0,
            0.05,
            " mm",
        );
        form.colour(ui, "ui.style.outline_colour", &mut outlines.colour);
        form.switch(ui, "ui.style.folds", &mut outlines.folds);
        form.switch(ui, "ui.style.where_meet", &mut outlines.where_objects_meet);
    }
    let hidden = &mut style.hidden_lines;
    form.switch(ui, "ui.style.hidden_lines", &mut hidden.shown);
    if hidden.shown {
        form.pattern(ui, &mut hidden.pattern);
        form.colour(ui, "ui.style.hidden_colour", &mut hidden.colour);
        form.percent(ui, "ui.style.opacity", &mut hidden.opacity);
    }
    let rules = &mut style.line_rules;
    form.switch(ui, "ui.style.by_distance", &mut rules.by_distance);
    if rules.by_distance {
        form.percent(ui, "ui.style.far_width", &mut rules.far_width);
        form.percent(ui, "ui.style.far_fade", &mut rules.far_fade);
    }
}

/// Grain, vignette and wash over the whole view.
fn finish(ui: &mut egui::Ui, form: &mut Form, style: &mut ViewStyle) {
    let more = form.section(ui, "ui.style.finish", 2);
    form.percent(ui, "ui.style.grain", &mut style.finish.grain);
    if more {
        form.percent(ui, "ui.style.vignette", &mut style.finish.vignette);
        form.percent(ui, "ui.style.wash", &mut style.finish.wash);
    }
}

/// How selected and locked objects show.
fn behaviour(ui: &mut egui::Ui, form: &mut Form, style: &mut ViewStyle) {
    let more = form.section(ui, "ui.style.behaviour", 5);
    let behaviour = &mut style.behaviour;
    form.colour(
        ui,
        "ui.style.selection_colour",
        &mut behaviour.selection_colour,
    );
    form.choice(
        ui,
        "ui.style.locked",
        &mut behaviour.locked,
        &[
            (Locked::Normal, "ui.style.normal"),
            (Locked::Greyed, "ui.style.greyed"),
            (Locked::Ghosted, "ui.style.ghosted"),
        ],
    );
    if !more {
        return;
    }
    form.percent(ui, "ui.style.locked_opacity", &mut behaviour.locked_opacity);
    form.switch(
        ui,
        "ui.style.show_through",
        &mut behaviour.selected_show_through,
    );
    form.switch(ui, "ui.style.transparency", &mut behaviour.transparency);
    form.switch(ui, "ui.style.highlights", &mut behaviour.highlights);
    form.switch(ui, "ui.style.boxes", &mut behaviour.boxes_while_moving);
}

/// What the style is for: 3D views, plans, sections and elevations, sheets.
fn used_for(ui: &mut egui::Ui, form: &mut Form, style: &mut ViewStyle) {
    form.section(ui, "ui.style.used_for", 0);
    let used = &mut style.used_for;
    ui_kit::chip_row(ui, |ui| {
        ui_kit::toggle(ui, &mut used.views, form.say("ui.style.views"));
        ui_kit::toggle(ui, &mut used.plans, form.say("ui.style.plans"));
        ui_kit::toggle(ui, &mut used.sections, form.say("ui.style.sections"));
        ui_kit::toggle(ui, &mut used.sheets, form.say("ui.style.sheets"));
    });
}
