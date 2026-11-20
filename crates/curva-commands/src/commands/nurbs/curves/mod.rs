//! The Curves tab's commands, and the curve tools that have no button yet.

pub(crate) mod arc;
pub(crate) mod blend_crv;
pub(crate) mod chamfer;
pub(crate) mod circle;
pub(crate) mod close_crv;
pub(crate) mod conic;
pub(crate) mod curve;
pub(crate) mod curve_boolean;
pub(crate) mod divide;
pub(crate) mod ellipse;
pub(crate) mod extend;
pub(crate) mod fair;
pub(crate) mod fillet;
pub(crate) mod fillet_corners;
pub(crate) mod handle_curve;
pub(crate) mod helix;
pub(crate) mod hyperbola;
pub(crate) mod interpcrv;
pub(crate) mod line;
pub(crate) mod lines;
pub(crate) mod make_uniform;
pub(crate) mod match_curve;
pub(crate) mod offset;
pub(crate) mod offset_multiple;
pub(crate) mod parabola;
pub(crate) mod point;
pub(crate) mod points;
pub(crate) mod polygon;
pub(crate) mod polyline;
pub(crate) mod rectangle;
pub(crate) mod reparameterize;
pub(crate) mod simplify_crv;
pub(crate) mod sketch;
pub(crate) mod slot;
pub(crate) mod spiral;
mod support;
pub(crate) mod text_object;
pub(crate) mod tween_curves;
pub(crate) mod weight;

/// This folder's commands, as the command list holds them; `commands/order.rs` keeps their
/// place in it.
pub(crate) static COMMANDS: &[crate::commands::CommandDef] = &[
    recipe!("Line", ["l"], true, line::Line),
    recipe!("Polyline", ["pl", "pline"], true, polyline::Polyline),
    recipe!("InterpCrv", ["interpcurve"], true, interpcrv::InterpCrv),
    recipe!("Circle", ["c", "ci"], true, circle::Circle),
    recipe!("Point", ["pt"], true, point::PointObject),
    listed!("Divide", [], true, true, divide::Divide),
    listed!("CloseCrv", [], true, true, close_crv::CloseCrv),
    recipe!("Offset", [], true, offset::Offset),
    recipe!("Fillet", [], true, fillet::Fillet),
    listed!(
        "FilletCorners",
        [],
        true,
        true,
        fillet_corners::FilletCorners
    ),
    listed!("TweenCurves", [], true, true, tween_curves::TweenCurves),
    recipe!("Chamfer", [], true, chamfer::Chamfer),
    recipe!("Ellipse", ["el"], true, ellipse::Ellipse),
    recipe!("Arc", ["a"], true, arc::Arc),
    recipe!("Rectangle", ["rec"], true, rectangle::Rectangle),
    recipe!("Polygon", [], true, polygon::Polygon),
    recipe!("Helix", [], true, helix::Helix),
    recipe!("Spiral", [], true, spiral::Spiral),
    recipe!("Points", [], true, points::Points),
    recipe!("Lines", [], true, lines::Lines),
    recipe!("Slot", [], true, slot::Slot),
    recipe!("Parabola", [], true, parabola::Parabola),
    recipe!("HandleCurve", [], true, handle_curve::HandleCurve),
    recipe!("Sketch", [], true, sketch::Sketch),
    recipe!("Conic", [], true, conic::Conic),
    recipe!("Hyperbola", [], true, hyperbola::Hyperbola),
    recipe!("BlendCrv", ["blend"], true, blend_crv::BlendCrv),
    recipe!("OffsetMultiple", [], true, offset_multiple::OffsetMultiple),
    recipe!("Extend", ["ex"], true, extend::Extend),
    recipe!("Fair", [], true, fair::Fair),
    recipe!("CurveBoolean", [], true, curve_boolean::CurveBoolean),
    recipe!("Match", ["matchcrv"], true, match_curve::Match),
    recipe!("SimplifyCrv", [], true, simplify_crv::SimplifyCrv),
    recipe!("Reparameterize", [], true, reparameterize::Reparameterize),
    recipe!("MakeUniform", [], true, make_uniform::MakeUniform),
    recipe!("Weight", [], true, weight::Weight),
    listed!("TextObject", [], true, true, text_object::TextObject),
    recipe!("Curve", [], true, curve::CurveCommand),
];

/// The buttons of the Curves tab, in order (spec `docs/specs/command-groups.md` §2).
pub(crate) const TAB: &[&str] = &[
    "Line",
    "Polyline",
    "Curve",
    "InterpCrv",
    "Circle",
    "Arc",
    "Ellipse",
    "Rectangle",
    "Polygon",
    "Slot",
    "Helix",
    "Spiral",
    "Parabola",
    "Conic",
    "TextObject",
    "Hyperbola",
    "HandleCurve",
    "Sketch",
    "Point",
    "Points",
    "Lines",
    "Offset",
    "Fillet",
    "Chamfer",
    "BlendCrv",
    "OffsetMultiple",
    "Extend",
    "Match",
    "Fair",
    "SimplifyCrv",
    "CurveBoolean",
    "MakeUniform",
    "Reparameterize",
    "Weight",
];
