//! A body read quickly taking its healed faces (#476).

use super::{Body, IN_PLACE, PlacedRecord, followed_by};

impl Body {
    /// This body with the faces of `made`, a body read in place from the same record, kept where
    /// this one is placed: how a body read quickly takes its healed form once it is ready (#476).
    #[must_use]
    pub fn with_faces_of(&self, made: &Body) -> Body {
        Body {
            shared: std::sync::Arc::clone(&made.shared),
            placement: if made.placement == IN_PLACE {
                self.placement
            } else {
                followed_by(&made.placement, &self.placement)
            },
            placed: PlacedRecord::default(),
        }
    }
}
