//! Moving the gumball itself, as Rhino 8 relocates it: a double-click on a handle, then a drag of
//! any handle moves the gumball, not what it holds. Its scale handles and circles then stretch
//! from where it was put: from a corner, only the side dragged moves.

use crate::*;

/// What the gumball held when it was moved: the objects selected, or those with picked parts and
/// how many points are picked. Picking something else puts it back in the middle.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Holding {
    objects: Vec<u64>,
    points: usize,
}

impl Curva {
    /// What the gumball holds now.
    fn holding(&self) -> Holding {
        if self.doc.has_picked_parts() {
            Holding {
                objects: self.doc.picked_part_owners(),
                points: self.doc.picked_part_points().map_or(0, |p| p.len()),
            }
        } else {
            Holding {
                objects: self.doc.selection(),
                points: 0,
            }
        }
    }

    /// The gumball where it rests: where it was moved to, for what it still holds, else the
    /// middle of what it holds.
    pub(crate) fn gumball_base(&self) -> Option<Gumball> {
        let natural = self.natural_gumball()?;
        match &self.gumball_state.moved_to {
            Some((held, centre)) if *held == self.holding() => Some(Gumball {
                centre: *centre,
                ..natural
            }),
            _ => Some(natural),
        }
    }

    /// The gumball as drawn and clicked: where it rests, or following a drag that moves it.
    pub(crate) fn gumball(&self) -> Option<Gumball> {
        let base = self.gumball_base()?;
        let moving = self
            .gumball_state
            .gumball_drag
            .filter(|drag| drag.relocate)
            .and_then(|drag| self.gumball_change(&drag));
        Some(match moving {
            Some(change) => Gumball {
                centre: change.moved_point(base.centre),
                ..base
            },
            None => base,
        })
    }

    /// Ends a drag that moves the gumball: it stays where it was dragged, for what it holds. A
    /// turn or a stretch leaves it where it was.
    pub(crate) fn finish_relocating(&mut self, drag: &gumball::GumballDrag) {
        self.gumball_state.relocating = false;
        let Some(base) = self.gumball_base() else {
            return;
        };
        let moved = match self.gumball_change(drag) {
            Some(change @ (GumballChange::Move(_) | GumballChange::Extrude { .. })) => {
                change.moved_point(base.centre)
            }
            _ => return,
        };
        self.gumball_state.moved_to = Some((self.holding(), moved));
    }
}
