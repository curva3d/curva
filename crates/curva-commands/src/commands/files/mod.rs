//! The File menu's commands: new, open, save, import and export.

pub(crate) mod import_export;
pub(crate) mod new;
pub(crate) mod open;
pub(crate) mod save;
pub(crate) mod save_as;
mod support;

/// This folder's commands, as the command list holds them; `commands/order.rs` keeps their
/// place in it.
pub(crate) static COMMANDS: &[crate::commands::CommandDef] = &[
    listed!("Save", [], false, false, save::Save),
    listed!("SaveAs", [], false, false, save_as::SaveAs),
    listed!("Open", [], false, false, open::Open),
    listed!("Import", [], false, true, import_export::Import),
    listed!("Export", [], false, false, import_export::Export),
    listed!(
        "ExportSelected",
        [],
        false,
        false,
        import_export::ExportSelected
    ),
    listed!("New", [], false, false, new::New),
];
