//! Left-button clicks and drags, read from the raw mouse events.

use crate::egui;

/// Finds left-button clicks and drags from the raw mouse events.
///
/// egui's own click signal drops a press if the mouse travels 6 points before release, and
/// reports at most one per frame, which loses points clicked quickly while moving to the next.
#[derive(Default)]
pub(crate) struct LeftGestures {
    pub(crate) pressed_at: Option<egui::Pos2>,
    /// The keys held when the last press began, which decide what the click picks: they may be
    /// let go before the frame ends, as in a quick Ctrl+Shift click (#686).
    pub(crate) pressed_with: egui::Modifiers,
}

impl LeftGestures {
    /// Where each press finished in these events began and ended. A press only counts if it
    /// was `inside` the viewport.
    pub(crate) fn take(
        &mut self,
        events: &[egui::Event],
        inside: impl Fn(egui::Pos2) -> bool,
    ) -> Vec<(egui::Pos2, egui::Pos2)> {
        let mut gestures = Vec::new();
        for event in events {
            if let egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers,
                ..
            } = event
            {
                if *pressed {
                    self.pressed_at = inside(*pos).then_some(*pos);
                    self.pressed_with = *modifiers;
                } else {
                    gestures.extend(self.pressed_at.take().map(|from| (from, *pos)));
                }
            }
        }
        gestures
    }
}
