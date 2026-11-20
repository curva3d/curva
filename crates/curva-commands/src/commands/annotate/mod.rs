//! Annotation commands: text now; dimensions, leaders, dots and hatches as they come (spec
//! `annotation.md`).

pub(crate) mod arc_length;
pub(crate) mod dimension;
pub(crate) mod dot;
pub(crate) mod hatch;
pub(crate) mod hatch_base;
pub(crate) mod hatch_patterns;
pub(crate) mod leader;
pub(crate) mod measure;
pub(crate) mod ordinate;
pub(crate) mod radial;
pub(crate) mod styles;
pub(crate) mod text;

/// This folder's commands, as the command list holds them; `commands/order.rs` keeps their
/// place in it.
pub(crate) static COMMANDS: &[crate::commands::CommandDef] = &[
    listed!("Text", [], true, true, text::Text),
    listed!("Dim", [], true, true, dimension::Dim),
    listed!("DimAligned", [], true, true, dimension::DimAligned),
    listed!("DimRotated", [], true, true, dimension::DimRotated),
    listed!("DimRadius", [], true, true, radial::DimRadius),
    listed!("DimDiameter", [], true, true, radial::DimDiameter),
    listed!("DimAngle", [], true, true, radial::DimAngle),
    listed!("DimOrdinate", [], true, true, ordinate::DimOrdinate),
    listed!("DimCurveLength", [], true, true, measure::DimCurveLength),
    listed!("DimArea", [], true, true, measure::DimArea),
    listed!("DimVolume", [], true, true, measure::DimVolume),
    recipe!("DimArcLength", [], true, arc_length::DimArcLength),
    listed!("Leader", [], true, true, leader::Leader),
    listed!("Dot", [], true, true, dot::Dot),
    listed!("Hatch", [], true, true, hatch::Hatch),
    listed!(
        "SetHatchBasePoint",
        [],
        true,
        true,
        hatch_base::SetHatchBasePoint
    ),
    listed!(
        "HatchPatterns",
        [],
        false,
        true,
        hatch_patterns::HatchPatterns
    ),
    listed!(
        "AnnotationStyle",
        ["dimstyle"],
        false,
        false,
        styles::AnnotationStyle
    ),
    listed!(
        "SetCurrentAnnotationStyle",
        [],
        false,
        true,
        styles::SetCurrentAnnotationStyle
    ),
    listed!(
        "DupAnnotationStyle",
        [],
        false,
        true,
        styles::DupAnnotationStyle
    ),
];

/// The buttons of the Annotate tab, in order (spec `docs/specs/command-groups.md` §2).
pub(crate) const TAB: &[&str] = &[
    "Text",
    "Dim",
    "DimAligned",
    "DimRotated",
    "DimRadius",
    "DimDiameter",
    "DimAngle",
    "DimOrdinate",
    "Leader",
    "Dot",
    "Hatch",
    "DimCurveLength",
    "DimArea",
    "DimVolume",
];
