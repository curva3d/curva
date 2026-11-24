//! Commands for the views: the standard views, zooming, construction planes and display modes.

pub(crate) mod cplane;
pub(crate) mod four_view;
pub(crate) mod front;
pub(crate) mod named_cplane;
pub(crate) mod named_view;
pub(crate) mod perspective;
pub(crate) mod right;
pub(crate) mod set_display_mode;
mod styles;
mod support;
pub(crate) mod top;
pub(crate) mod view_history;
pub(crate) mod zoom;
pub(crate) mod zoom_extents;
pub(crate) mod zoom_selected;

/// This folder's commands, as the command list holds them; `commands/order.rs` keeps their
/// place in it.
pub(crate) static COMMANDS: &[crate::commands::CommandDef] = &[
    listed!(
        "ZoomSelected",
        ["zs"],
        false,
        false,
        zoom_selected::ZoomSelected
    ),
    listed!(
        "ZoomExtents",
        ["ze", "zea"],
        false,
        false,
        zoom_extents::ZoomExtents
    ),
    listed!("4View", [], false, false, four_view::FourView),
    listed!("UndoView", [], false, false, view_history::UndoView),
    listed!("RedoView", [], false, false, view_history::RedoView),
    listed!("Top", [], false, false, top::Top),
    listed!("Front", [], false, false, front::Front),
    listed!("Right", [], false, false, right::Right),
    listed!("Perspective", [], false, false, perspective::Perspective),
    listed!("CPlane", [], false, false, cplane::SetCPlane),
    listed!("Zoom", [], false, false, zoom::Zoom),
    listed!("NamedView", ["nv"], false, false, named_view::NamedView),
    listed!(
        "NamedCPlane",
        ["ncp"],
        false,
        false,
        named_cplane::NamedCPlane
    ),
    listed!(
        "SetDisplayMode",
        [],
        false,
        false,
        set_display_mode::SetDisplayMode
    ),
    listed!("Styles", [], false, false, styles::Styles),
];
