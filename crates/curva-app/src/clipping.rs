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

/// The faces filling where `cuts` cut the model's closed solids, each in the colour of the
/// solid's section style: its own, else its layer's, else the plane's (spec `layouts.md` §3).
/// A style with no fill leaves the cut open.
/// `cut` as the graphics card cuts with it, `[nx, ny, nz, d]`, cutting away where `n·p + d > 0`,
/// for positions measured from `origin` (#225); worked out in double precision, so a plane far
/// from the origin cuts where it is ([`PlaneCut::for_drawing`] is this from the origin).
fn plane_from(cut: &PlaneCut, origin: Point) -> [f32; 4] {
    let d = (0..3)
        .map(|i| cut.keep[i] * (cut.origin[i] - origin[i]))
        .sum::<f64>();
    #[allow(clippy::cast_possible_truncation)]
    [
        -cut.keep[0] as f32,
        -cut.keep[1] as f32,
        -cut.keep[2] as f32,
        d as f32,
    ]
}

fn fills_for(state: &State, cuts: &[PlaneCut], origin: Point) -> Vec<curva_viewport::MeshVertex> {
    let mut out = Vec::new();
    for cut in cuts {
        for object in state
            .objects
            .iter()
            .filter(|o| o.space.is_none() && !o.hidden)
        {
            let Some(body) = object.shape.body().filter(|b| b.is_solid()) else {
                continue;
            };
            let fill = state
                .section_style(object.id, cut.frame)
                .map_or(cut.section, |style| style.fill);
            let SectionFill::Solid(colour) = fill else {
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
                        position: curva_viewport::narrow_from(*position, origin),
                        normal: cut.keep.map(|v| v as f32),
                        colour,
                        object: curva_viewport::NO_OBJECT,
                        kind: curva_viewport::FaceKind::Given,
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
        origin: Point,
    ) -> (Vec<[f32; 4]>, Option<Arc<Vec<curva_viewport::MeshVertex>>>) {
        let cuts = self.doc.planes_cutting(view);
        if cuts.is_empty() {
            return (Vec::new(), None);
        }
        let planes: Vec<[f32; 4]> = cuts.iter().map(|cut| plane_from(cut, origin)).collect();
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
        let made = Arc::new(fills_for(self.doc.state(), &cuts, origin));
        fills.by_planes.push((planes.clone(), Arc::clone(&made)));
        (planes, Some(made))
    }

    /// The view being worked in: the detail entered on a sheet, else the active viewport.
    pub(crate) fn clip_view(&self) -> ClipView {
        match self.sheets.entered {
            Some(frame) if self.sheets.open.is_some() => ClipView::Detail(frame),
            _ => ClipView::Viewport(self.active),
        }
    }

    /// Picked clipping planes cut, or stop cutting, the active viewport, or the detail being
    /// worked in on a sheet.
    pub(crate) fn clip_in(&mut self, frames: &[u64], on: bool) {
        let view = self.clip_view();
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
        origin: Point,
    ) -> (Vec<[f32; 4]>, Arc<Vec<curva_viewport::MeshVertex>>) {
        let (planes, fills) = self.clipping_for(view, origin);
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

#[cfg(test)]
mod tests {
    use super::fills_for;
    use curva_document::Document;
    use curva_document::clipping::{ClipView, ClippingPlane, SectionFill};
    use curva_document::section::SectionStyle;
    use curva_kernel::{Body, Curve};

    const PLANE: [f32; 3] = [0.1, 0.2, 0.3];

    /// A 10-unit cube cut level through its middle by a plane filling in `PLANE`; the cube.
    fn cut_cube(doc: &mut Document) -> u64 {
        let cube = doc.add_body(Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap());
        let frame = doc.add(
            Curve::polyline(
                vec![
                    [-5.0, -5.0, 5.0],
                    [15.0, -5.0, 5.0],
                    [15.0, 15.0, 5.0],
                    [-5.0, 15.0, 5.0],
                ],
                true,
            )
            .unwrap(),
        );
        doc.add_clipping_plane(ClippingPlane {
            frame,
            flipped: false,
            views: vec![ClipView::Viewport(0)],
            section: SectionFill::Solid(PLANE),
            hatch: None,
        });
        cube
    }

    /// The colours the fills are drawn in.
    fn colours(doc: &Document) -> Vec<[f32; 3]> {
        let cuts = doc.planes_cutting(ClipView::Viewport(0));
        let mut found: Vec<[f32; 3]> = Vec::new();
        for vertex in fills_for(doc.state(), &cuts, [0.0; 3]) {
            if !found.contains(&vertex.colour) {
                found.push(vertex.colour);
            }
        }
        found
    }

    fn solid(colour: [f32; 3]) -> Option<SectionStyle> {
        Some(SectionStyle {
            fill: SectionFill::Solid(colour),
            hatch: None,
        })
    }

    #[test]
    fn a_cut_takes_the_planes_colour_with_no_style_of_its_own() {
        let mut doc = Document::default();
        cut_cube(&mut doc);
        assert_eq!(colours(&doc), vec![PLANE]);
    }

    #[test]
    fn a_solids_own_style_and_then_its_layers_come_first() {
        let mut doc = Document::default();
        let cube = cut_cube(&mut doc);
        let layer = doc.layer_of(cube).unwrap();
        doc.set_layer_section_style(layer, solid([0.0, 1.0, 0.0]));
        assert_eq!(colours(&doc), vec![[0.0, 1.0, 0.0]]);
        doc.set_section_style(&[cube], solid([1.0, 0.0, 0.0]));
        assert_eq!(colours(&doc), vec![[1.0, 0.0, 0.0]]);
    }

    #[test]
    fn a_style_with_no_fill_leaves_the_cut_open() {
        let mut doc = Document::default();
        let cube = cut_cube(&mut doc);
        doc.set_section_style(
            &[cube],
            Some(SectionStyle {
                fill: SectionFill::None,
                hatch: None,
            }),
        );
        assert!(colours(&doc).is_empty());
    }
}
