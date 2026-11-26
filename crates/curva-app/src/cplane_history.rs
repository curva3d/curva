//! Each viewport's construction planes before and after the one it has, for CPlane's `Undo` and
//! `Redo`, as Rhino keeps them per viewport (#776).

use crate::Curva;
use crate::viewports::Pane;
use curva_commands::ViewRequest;
use curva_viewport::CPlane;

impl Curva {
    /// A request about the last-used viewport's construction plane: set it, put it back on the
    /// viewport's own, or step back or forward through the planes it has had.
    pub(crate) fn apply_plane_request(&mut self, request: ViewRequest) {
        let pane = &mut self.panes[self.active];
        match request {
            ViewRequest::SetCPlane { origin, x, y } => {
                let normal = curva_kernel::cross(x, y);
                pane.change_plane(CPlane {
                    origin,
                    x,
                    y,
                    normal,
                });
            }
            ViewRequest::ResetCPlane => pane.change_plane(pane.view.plane()),
            ViewRequest::UndoCPlane => {
                pane.undo_plane();
            }
            ViewRequest::RedoCPlane => {
                pane.redo_plane();
            }
            _ => {}
        }
    }
}

impl Pane {
    /// Puts the viewport on `plane`, remembering the one it had for `Undo`.
    pub(crate) fn change_plane(&mut self, plane: CPlane) {
        if plane != self.plane {
            self.planes_back.push(self.plane);
            self.planes_ahead.clear();
        }
        self.set_plane(plane);
    }

    /// Back to the plane before; `false` when there is none.
    pub(crate) fn undo_plane(&mut self) -> bool {
        let Some(before) = self.planes_back.pop() else {
            return false;
        };
        self.planes_ahead.push(self.plane);
        self.set_plane(before);
        true
    }

    /// Forward to a plane stepped back from; `false` when there is none.
    pub(crate) fn redo_plane(&mut self) -> bool {
        let Some(after) = self.planes_ahead.pop() else {
            return false;
        };
        self.planes_back.push(self.plane);
        self.set_plane(after);
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::viewports::Pane;
    use curva_viewport::{CPlane, StandardView};

    #[test]
    fn a_viewport_steps_back_and_forward_through_its_planes() {
        let mut pane = Pane::new(StandardView::Top);
        let raised = CPlane {
            origin: [0.0, 0.0, 3.0],
            ..CPlane::WORLD_XY
        };
        pane.change_plane(raised);
        assert!(pane.undo_plane());
        assert_eq!(pane.plane, CPlane::WORLD_XY);
        assert!(!pane.undo_plane(), "nothing further back");
        assert!(pane.redo_plane());
        assert_eq!(pane.plane, raised);
        assert!(!pane.redo_plane(), "nothing further ahead");
    }
}
