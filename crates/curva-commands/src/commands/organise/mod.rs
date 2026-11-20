//! Commands that organise the model: layers, properties, groups, locking, draw order and blocks.

pub(crate) mod block;
pub(crate) mod block_edit;
pub(crate) mod block_manager;
pub(crate) mod change_layer;
pub(crate) mod draw_order;
pub(crate) mod explode_block;
pub(crate) mod get_document_user_text;
pub(crate) mod get_user_text;
pub(crate) mod group;
pub(crate) mod insert;
pub(crate) mod linetype_command;
pub(crate) mod lock;
pub(crate) mod lock_swap;
pub(crate) mod match_properties;
pub(crate) mod notes;
pub(crate) mod object_colour;
pub(crate) mod print_width_command;
pub(crate) mod purge;
pub(crate) mod rename;
pub(crate) mod replace_block;
pub(crate) mod set_document_user_text;
pub(crate) mod set_user_text;
mod support;
pub(crate) mod ungroup;
pub(crate) mod unlock;
pub(crate) mod unlock_selected;

/// This folder's commands, as the command list holds them; `commands/order.rs` keeps their
/// place in it.
pub(crate) static COMMANDS: &[crate::commands::CommandDef] = &[
    listed!("Rename", [], true, true, rename::Rename),
    recipe!("Lock", [], true, lock::Lock),
    recipe!("Unlock", [], true, unlock::Unlock),
    recipe!("Group", [], true, group::Group),
    recipe!("Ungroup", [], true, ungroup::Ungroup),
    listed!("Block", [], true, true, block::Block),
    listed!("Insert", [], true, true, insert::Insert),
    listed!(
        "BlockManager",
        [],
        false,
        false,
        block_manager::BlockManager
    ),
    listed!("BlockEdit", [], true, true, block_edit::BlockEdit),
    listed!("ExplodeBlock", [], true, true, explode_block::ExplodeBlock),
    listed!("ReplaceBlock", [], true, true, replace_block::ReplaceBlock),
    listed!("ChangeLayer", [], true, true, change_layer::ChangeLayer),
    listed!(
        "MatchProperties",
        ["matchprop"],
        true,
        true,
        match_properties::MatchProperties
    ),
    listed!(
        "Linetype",
        [],
        true,
        true,
        linetype_command::LinetypeCommand
    ),
    listed!(
        "PrintWidth",
        [],
        true,
        true,
        print_width_command::PrintWidthCommand
    ),
    listed!(
        "ObjectColour",
        ["objectcolor"],
        true,
        true,
        object_colour::ObjectColour
    ),
    listed!("SetUserText", [], true, true, set_user_text::SetUserText),
    listed!("GetUserText", [], false, false, get_user_text::GetUserText),
    listed!(
        "SetDocumentUserText",
        [],
        true,
        true,
        set_document_user_text::SetDocumentUserText<false>
    ),
    listed!(
        "SetLayoutUserText",
        [],
        true,
        true,
        set_document_user_text::SetDocumentUserText<true>
    ),
    listed!(
        "GetDocumentUserText",
        [],
        false,
        false,
        get_document_user_text::GetDocumentUserText
    ),
    listed!("Notes", [], false, true, notes::Notes),
    listed!("Purge", [], true, true, purge::Purge),
    listed!("LockSwap", [], true, true, lock_swap::LockSwap),
    listed!(
        "UnlockSelected",
        [],
        true,
        true,
        unlock_selected::UnlockSelected
    ),
    listed!("BringToFront", [], true, true, draw_order::DrawOrder<true, true>),
    listed!("SendToBack", [], true, true, draw_order::DrawOrder<false, true>),
    listed!("BringForward", [], true, true, draw_order::DrawOrder<true, false>),
    listed!("SendBackward", [], true, true, draw_order::DrawOrder<false, false>),
];
