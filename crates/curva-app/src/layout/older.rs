//! Layout files from before panels held pieces, read and brought up to date.

use super::{Align, Dock, Piece, Place};
use serde::Deserialize;

#[derive(Deserialize)]
enum PanelId {
    CommandTabs,
    Side,
    CommandBar,
}

#[derive(Deserialize)]
struct OldPlace {
    panel: PanelId,
    dock: Dock,
    size: [f32; 2],
}

#[derive(Deserialize)]
pub struct Layout {
    places: Vec<OldPlace>,
}

impl Layout {
    /// The same arrangement in today's terms. The command bar keeps its place, and the
    /// agent connection, once part of it, takes its standard one.
    pub fn updated(self) -> super::Layout {
        let mut places: Vec<Place> = self
            .places
            .into_iter()
            .map(|old| {
                // A command bar along the bottom becomes today's compact, centred one.
                if matches!(old.panel, PanelId::CommandBar)
                    && old.dock == Dock::Bottom
                    && let Some(standard) = super::Layout::default()
                        .places
                        .into_iter()
                        .find(|p| p.pieces == [Piece::Command])
                {
                    return standard;
                }
                let pieces = match old.panel {
                    PanelId::CommandTabs => vec![Piece::CommandTabs],
                    PanelId::Side => vec![Piece::Layers, Piece::Properties],
                    PanelId::CommandBar => vec![Piece::Command],
                };
                Place::new(pieces, old.dock, Align::Fill, old.size)
            })
            .collect();
        if let Some(agent) = super::Layout::default()
            .places
            .into_iter()
            .find(|p| p.pieces == [Piece::Agent])
        {
            places.push(agent);
        }
        super::Layout {
            places,
            closed: Vec::new(),
            layer_columns: super::STANDARD_LAYER_COLUMNS,
        }
        .completed()
    }
}
