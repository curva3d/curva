//! Clipping planes in the window (spec `layouts.md` §3): which planes cut each viewport and
//! detail, and the section fills where they cut closed solids.

use crate::*;
use curva_document::clipping::{ClipView, PlaneCut, SectionFill};

/// Faces as the graphics card draws them, shared between frames.
type Faces = Arc<Vec<curva_viewport::MeshVertex>>;

/// Clipping planes as the graphics card cuts with them.
type Planes = Vec<[f32; 4]>;

/// Section fills already worked out, for the model and the planes they were made for.
#[derive(Default)]
pub(crate) struct SectionFills {
    /// The state they were made from, compared quickly with `State::same_as`.
    made_from: Option<State>,
    /// For each set of planes (as drawn), the fill faces.
    by_planes: Vec<(Planes, Faces)>,
    /// Faces with fills added, for the face list (by address) and planes they were made for, so
    /// a frame draws them without copying.
    combined: Vec<(usize, Planes, Faces)>,
}

/// The faces filling where `cuts` cut the model's closed solids, each in its plane's colour.
fn fills_for(state: &State, cuts: &[PlaneCut]) -> Vec<curva_viewport::MeshVertex> {
    let mut out = Vec::new();
    for cut in cuts {
        let SectionFill::Solid(colour) = cut.section else {
            continue;
        };
        for object in state
            .objects
            .iter()
            .filter(|o| o.space.is_none() && !o.hidden)
        {
            let Some(body) = object.shape.body().filter(|b| b.is_solid()) else {
                continue;
            };
            let Ok(curves) = body.section(cut.origin, cut.keep) else {
                continue;
            };
            if curves.is_empty() {
                continue;
            }
            let Ok(face) = curva_kernel::Body::planar(&curves) else {
                continue;
            };
            let Ok(mesh) = face.mesh() else {
                continue;
            };
            for &corner in mesh.triangles.iter().flatten() {
                let index = corner as usize;
                if let Some(position) = mesh.positions.get(index) {
                    #[allow(clippy::cast_possible_truncation)]
                    out.push(curva_viewport::MeshVertex {
                        position: position.map(|v| v as f32),
                        normal: cut.keep.map(|v| v as f32),
                        colour,
                    });
                }
            }
        }
    }
    out
}

impl Curva {
    /// The planes cutting `view`, as the graphics card cuts with them, and the faces filling
    /// the solids they cut.
    pub(crate) fn clipping_for(
        &mut self,
        view: ClipView,
    ) -> (Vec<[f32; 4]>, Option<Arc<Vec<curva_viewport::MeshVertex>>>) {
        let cuts = self.doc.planes_cutting(view);
        if cuts.is_empty() {
            return (Vec::new(), None);
        }
        let planes: Vec<[f32; 4]> = cuts.iter().map(PlaneCut::for_drawing).collect();
        let fills = &mut self.section_fills;
        if !fills
            .made_from
            .as_ref()
            .is_some_and(|state| state.same_as(self.doc.state()))
        {
            fills.by_planes.clear();
            fills.combined.clear();
            fills.made_from = Some(self.doc.state().clone());
        }
        if let Some((_, made)) = fills.by_planes.iter().find(|(p, _)| *p == planes) {
            return (planes, Some(Arc::clone(made)));
        }
        let made = Arc::new(fills_for(self.doc.state(), &cuts));
        fills.by_planes.push((planes.clone(), Arc::clone(&made)));
        (planes, Some(made))
    }

    /// Picked clipping planes cut, or stop cutting, the active viewport, or the detail being
    /// worked in on a sheet.
    pub(crate) fn clip_in(&mut self, frames: &[u64], on: bool) {
        let view = match self.sheets.entered {
            Some(frame) if self.sheets.open.is_some() => ClipView::Detail(frame),
            _ => ClipView::Viewport(self.active),
        };
        let frames = frames.to_vec();
        self.undoable(|doc| {
            for frame in frames {
                doc.change_clipping_plane(frame, |plane| {
                    plane.views.retain(|v| *v != view);
                    if on {
                        plane.views.push(view);
                    }
                });
            }
        });
    }
}

impl Curva {
    /// What a view draws when clipping planes cut it: the planes, and `faces` with the section
    /// fills added, kept so later frames copy nothing.
    pub(crate) fn clipped_view(
        &mut self,
        view: ClipView,
        faces: &Arc<Vec<curva_viewport::MeshVertex>>,
    ) -> (Vec<[f32; 4]>, Arc<Vec<curva_viewport::MeshVertex>>) {
        let (planes, fills) = self.clipping_for(view);
        let Some(fills) = fills.filter(|f| !f.is_empty()) else {
            return (planes, Arc::clone(faces));
        };
        let key = Arc::as_ptr(faces) as usize;
        let combined = &mut self.section_fills.combined;
        if let Some((_, _, made)) = combined.iter().find(|(k, p, _)| *k == key && *p == planes) {
            return (planes, Arc::clone(made));
        }
        let mut all = Vec::with_capacity(faces.len() + fills.len());
        all.extend_from_slice(faces);
        all.extend_from_slice(&fills);
        let all = Arc::new(all);
        // Only the latest few are worth keeping: a face list replaced is not drawn again.
        if combined.len() > 8 {
            combined.remove(0);
        }
        combined.push((key, planes.clone(), Arc::clone(&all)));
        (planes, all)
    }
}
