//! The Select tab's commands: picking by kind, name or layer, and hiding and showing.

pub(crate) mod hide;
pub(crate) mod hide_swap;
pub(crate) mod invert;
pub(crate) mod isolate;
pub(crate) mod sel_all;
pub(crate) mod sel_arc;
pub(crate) mod sel_block_instance_named;
pub(crate) mod sel_body;
pub(crate) mod sel_circle;
pub(crate) mod sel_closed_crv;
pub(crate) mod sel_color;
pub(crate) mod sel_crv;
pub(crate) mod sel_dup;
pub(crate) mod sel_filter;
pub(crate) mod sel_last;
pub(crate) mod sel_layer;
pub(crate) mod sel_line;
pub(crate) mod sel_name;
pub(crate) mod sel_none;
pub(crate) mod sel_open_crv;
pub(crate) mod sel_polyline;
pub(crate) mod sel_prev;
pub(crate) mod sel_pt;
pub(crate) mod show;
mod support;

/// This folder's commands, as the command list holds them; `commands/order.rs` keeps their
/// place in it.
pub(crate) static COMMANDS: &[crate::commands::CommandDef] = &[
    listed!("Hide", ["h"], true, true, hide::Hide),
    listed!("Show", ["unisolate"], true, true, show::Show),
    listed!("SelAll", ["sa"], false, false, sel_all::SelAll),
    listed!("SelNone", ["sn"], false, false, sel_none::SelNone),
    listed!("SelCrv", [], false, false, sel_crv::SelCrv),
    listed!("SelSrf", [], false, false, sel_body::SelSrf),
    listed!("SelPolysrf", [], false, false, sel_body::SelPolysrf),
    listed!("SelSolid", [], false, false, sel_body::SelSolid),
    listed!("SelClosedSrf", [], false, false, sel_body::SelClosedSrf),
    listed!("SelOpenSrf", [], false, false, sel_body::SelOpenSrf),
    listed!("SelPt", [], false, false, sel_pt::SelPt),
    listed!("SelName", [], false, false, sel_name::SelName),
    listed!("SelLayer", [], false, false, sel_layer::SelLayer),
    listed!("SelDup", [], false, false, sel_dup::SelDup),
    listed!("SelLast", [], false, false, sel_last::SelLast),
    listed!("SelPrev", [], false, false, sel_prev::SelPrev),
    listed!("SelFilter", [], false, false, sel_filter::SelFilter),
    listed!(
        "SelFilterToggle",
        [],
        false,
        false,
        sel_filter::SelFilterToggle
    ),
    listed!("Isolate", [], true, true, isolate::Isolate),
    listed!("Invert", [], false, false, invert::Invert),
    listed!("SelLine", [], false, false, sel_line::SelLine),
    listed!("SelPolyline", [], false, false, sel_polyline::SelPolyline),
    listed!("SelCircle", [], false, false, sel_circle::SelCircle),
    listed!("SelArc", [], false, false, sel_arc::SelArc),
    listed!(
        "SelClosedCrv",
        [],
        false,
        false,
        sel_closed_crv::SelClosedCrv
    ),
    listed!("SelOpenCrv", [], false, false, sel_open_crv::SelOpenCrv),
    listed!("HideSwap", [], true, true, hide_swap::HideSwap),
    recipe!("SelColor", [], false, sel_color::SelColor),
    listed!(
        "SelBlockInstanceNamed",
        [],
        false,
        false,
        sel_block_instance_named::SelBlockInstanceNamed
    ),
];

/// The buttons of the Select tab, in order (spec `docs/specs/command-groups.md` §2).
pub(crate) const TAB: &[&str] = &[
    "SelAll",
    "SelNone",
    "Invert",
    "SelCrv",
    "SelLine",
    "SelPolyline",
    "SelCircle",
    "SelArc",
    "SelClosedCrv",
    "SelOpenCrv",
    "SelSrf",
    "SelSolid",
    "SelPt",
    "SelColor",
    "Hide",
    "Show",
    "HideSwap",
];
