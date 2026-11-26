//! Commands for layouts: paper sheets beside the model (spec `layouts.md`).

pub(crate) mod clipping_plane;
pub(crate) mod copy_layout;
pub(crate) mod detail;
pub(crate) mod layout;
pub(crate) mod layouts_panel;
pub(crate) mod make2d;
pub(crate) mod print;
pub(crate) mod revision;
pub(crate) mod scale_bar;
pub(crate) mod section_style;
pub(crate) mod sheet_index;
pub(crate) mod title_block;

/// This folder's commands, as the command list holds them; `commands/order.rs` keeps their
/// place in it.
pub(crate) static COMMANDS: &[crate::commands::CommandDef] = &[
    listed!("Layout", [], true, true, layout::Layout),
    listed!("CopyLayout", [], false, true, copy_layout::CopyLayout),
    listed!("Layouts", [], false, false, layouts_panel::Layouts),
    listed!("Detail", [], false, true, detail::Detail),
    listed!("Print", ["printsheets", "plot"], false, false, print::Print),
    listed!("ExportPDF", [], false, false, print::ExportPdf),
    listed!("Make2D", [], false, true, make2d::Make2D),
    listed!("TitleBlock", [], false, true, title_block::TitleBlock),
    listed!("ScaleBar", [], true, true, scale_bar::ScaleBar),
    listed!("SheetIndex", [], false, true, sheet_index::SheetIndex),
    listed!("Revision", [], false, true, revision::Revision),
    listed!(
        "ClippingPlane",
        [],
        true,
        true,
        clipping_plane::ClippingPlane
    ),
    listed!(
        "EnableClippingPlane",
        [],
        false,
        true,
        clipping_plane::EnableClippingPlane
    ),
    listed!(
        "DisableClippingPlane",
        [],
        false,
        true,
        clipping_plane::DisableClippingPlane
    ),
    recipe!(
        "SectionStyle",
        ["sectionhatch"],
        true,
        section_style::SectionStyle
    ),
];

/// The buttons of the Layouts tab, in order (spec `docs/specs/command-groups.md` §2).
pub(crate) const TAB: &[&str] = &[
    "Layout",
    "Detail",
    "TitleBlock",
    "ScaleBar",
    "SheetIndex",
    "Revision",
    "CopyLayout",
    "Make2D",
    "Print",
    "ExportPDF",
];
