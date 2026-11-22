//! Rhino's selection filter (owner, 22 November 2026): which kinds of object a click or a window
//! may pick, Sub-objects to pick faces, edges and points without Ctrl+Shift, and Disable to let
//! everything through. Shown from the Filter item on the command bar's row of drawing aids, as
//! Rhino shows it from its status bar.

use crate::*;

/// The kinds of object the filter tells apart, in Rhino's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Point,
    Curve,
    Surface,
    Polysurface,
    SubD,
    Mesh,
    Annotation,
    Block,
    Hatch,
    Other,
}

/// What may be picked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct SelectionFilter {
    pub(crate) points: bool,
    pub(crate) curves: bool,
    pub(crate) surfaces: bool,
    pub(crate) polysurfaces: bool,
    pub(crate) subds: bool,
    pub(crate) meshes: bool,
    pub(crate) annotations: bool,
    pub(crate) blocks: bool,
    pub(crate) control_points: bool,
    pub(crate) hatches: bool,
    pub(crate) others: bool,
    /// A plain click or window picks parts of objects, as Ctrl+Shift does.
    pub(crate) sub_objects: bool,
    /// The filter is set aside: everything can be picked.
    pub(crate) disable: bool,
}

impl Default for SelectionFilter {
    fn default() -> Self {
        Self {
            points: true,
            curves: true,
            surfaces: true,
            polysurfaces: true,
            subds: true,
            meshes: true,
            annotations: true,
            blocks: true,
            control_points: true,
            hatches: true,
            others: true,
            sub_objects: false,
            disable: false,
        }
    }
}

impl SelectionFilter {
    /// Whether an object of `kind` may be picked.
    pub(crate) fn allows(&self, kind: Kind) -> bool {
        self.disable
            || match kind {
                Kind::Point => self.points,
                Kind::Curve => self.curves,
                Kind::Surface => self.surfaces,
                Kind::Polysurface => self.polysurfaces,
                Kind::SubD => self.subds,
                Kind::Mesh => self.meshes,
                Kind::Annotation => self.annotations,
                Kind::Block => self.blocks,
                Kind::Hatch => self.hatches,
                Kind::Other => self.others,
            }
    }

    /// Whether control points may be picked.
    pub(crate) fn allows_control_points(&self) -> bool {
        self.disable || self.control_points
    }

    /// Whether it leaves anything out or picks parts, so the command bar can show it is in force.
    pub(crate) fn in_force(&self) -> bool {
        !self.disable && (*self != Self::default())
    }

    /// The boxes the panel shows, with their words, in Rhino's order.
    fn boxes(&mut self) -> [(&'static str, &mut bool); 13] {
        [
            ("ui.filter.points", &mut self.points),
            ("ui.filter.curves", &mut self.curves),
            ("ui.filter.surfaces", &mut self.surfaces),
            ("ui.filter.polysurfaces", &mut self.polysurfaces),
            ("ui.filter.subds", &mut self.subds),
            ("ui.filter.meshes", &mut self.meshes),
            ("ui.filter.annotations", &mut self.annotations),
            ("ui.filter.blocks", &mut self.blocks),
            ("ui.filter.control_points", &mut self.control_points),
            ("ui.filter.hatches", &mut self.hatches),
            ("ui.filter.others", &mut self.others),
            ("ui.filter.sub_objects", &mut self.sub_objects),
            ("ui.filter.disable", &mut self.disable),
        ]
    }
}

/// The kind an object is, for the filter: a point or another curve, a surface of one face or a
/// polysurface of more, and so on.
pub(crate) fn kind_of(shape: &curva_document::Shape) -> Kind {
    use curva_document::Shape;
    use curva_document::annotation::Annotation;
    match shape {
        Shape::Curve(curva_kernel::Curve::Point { .. }) => Kind::Point,
        Shape::Curve(_) => Kind::Curve,
        Shape::Body(body) => {
            if body.face_count().unwrap_or(2) == 1 {
                Kind::Surface
            } else {
                Kind::Polysurface
            }
        }
        Shape::SubD(_) => Kind::SubD,
        Shape::Mesh(_) => Kind::Mesh,
        Shape::Annotation(Annotation::Hatch(_)) => Kind::Hatch,
        Shape::Annotation(_) => Kind::Annotation,
        Shape::Instance(_) => Kind::Block,
        #[allow(unreachable_patterns)]
        _ => Kind::Other,
    }
}

impl Curva {
    /// `ids` with the objects the filter leaves out taken away.
    pub(crate) fn filtered_picks(&self, ids: Vec<u64>) -> Vec<u64> {
        let filter = self.pick_filter;
        if filter.disable {
            return ids;
        }
        let objects = &self.doc.state().objects;
        ids.into_iter()
            .filter(|id| {
                objects
                    .iter()
                    .find(|o| o.id == *id)
                    .is_none_or(|o| filter.allows(kind_of(&o.shape)))
            })
            .collect()
    }

    /// The panel of boxes above the Filter item on the command bar's drawing aids (`item`),
    /// shown or hidden by a click on it.
    pub(crate) fn selection_filter_panel(&mut self, ui: &egui::Ui, item: &egui::Response) {
        if item.clicked() {
            self.filter_row = !self.filter_row;
        }
        if !self.filter_row {
            return;
        }
        let words: Vec<String> = SelectionFilter::default()
            .boxes()
            .map(|(key, _)| self.text.render(&Message::new(key)))
            .into_iter()
            .collect();
        let filter = &mut self.pick_filter;
        egui::Area::new(egui::Id::NULL.with("selection filter"))
            .order(egui::Order::Foreground)
            .fixed_pos(item.rect.left_top() - egui::vec2(0.0, 4.0))
            .pivot(egui::Align2::LEFT_BOTTOM)
            .show(ui.ctx(), |ui| {
                panel_frame().show(ui, |ui| {
                    for (index, ((_, value), word)) in
                        filter.boxes().into_iter().zip(words).enumerate()
                    {
                        // Sub-objects and Disable apart from the kinds, as in Rhino.
                        if index == 11 {
                            ui.separator();
                        }
                        ui_kit::toggle(ui, value, word);
                    }
                });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_filter_lets_through_what_is_ticked_and_everything_when_disabled() {
        let mut filter = SelectionFilter::default();
        assert!(!filter.in_force());
        filter.curves = false;
        assert!(filter.in_force());
        assert!(!filter.allows(Kind::Curve));
        assert!(filter.allows(Kind::Polysurface));
        filter.disable = true;
        assert!(
            filter.allows(Kind::Curve),
            "Disable lets everything through"
        );
        assert!(!filter.in_force());
    }

    #[test]
    fn objects_are_sorted_as_rhino_sorts_them() {
        use curva_document::Shape;
        let point = Shape::Curve(curva_kernel::Curve::Point { at: [0.0; 3] });
        assert_eq!(kind_of(&point), Kind::Point);
        let line = Shape::Curve(curva_kernel::Curve::line([0.0; 3], [1.0, 0.0, 0.0]).unwrap());
        assert_eq!(kind_of(&line), Kind::Curve);
        let cube = curva_kernel::Body::cuboid([0.0; 3], [1.0; 3]).unwrap();
        assert_eq!(kind_of(&Shape::Body(cube)), Kind::Polysurface);
        let square = curva_kernel::Curve::polyline(
            vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            true,
        )
        .unwrap();
        let sheet = curva_kernel::Body::planar(&[square]).unwrap();
        assert_eq!(kind_of(&Shape::Body(sheet)), Kind::Surface);
    }
}
