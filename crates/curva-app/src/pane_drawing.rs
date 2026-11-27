//! What each viewport draws in a frame: the drawing handed to the graphics card, and what egui
//! paints over it.

use crate::*;

impl Curva {
    /// What viewport `pane` hands the graphics card this frame: its grid, the scene clipped for
    /// it, its camera and its display mode.
    pub(crate) fn pane_drawing(
        &mut self,
        ui: &egui::Ui,
        pane: usize,
        rect: egui::Rect,
        frame: &PaneFrame,
    ) -> Drawing {
        let size = pane_size(rect);
        let origin = self.drawing.origin;
        // Clipping planes cut the model's viewports, never a sheet.
        let (clips, pane_faces) = if self.shows_sheet(pane) {
            (Vec::new(), Arc::clone(&frame.faces))
        } else {
            self.clipped_view(
                curva_document::clipping::ClipView::Viewport(pane),
                &frame.faces,
                origin,
            )
        };
        let look = self.grid_look(pane);
        self.panes[pane].restyle_grid(look);
        if self.panes[pane].grid_origin != origin {
            let pane = &mut self.panes[pane];
            pane.grid_origin = origin;
            pane.set_plane(pane.plane);
        }
        // The camera as the scene is measured, from its origin (#225).
        let camera = self.panes[pane].camera.measured_from(origin);
        let style = self.pane_style(pane);
        Drawing {
            clips,
            slot: pane,
            // No grid on a layout's paper, as in Rhino 8, whose page shows none even with its grid
            // on (#830).
            grid: if self.settings.grid && !self.shows_sheet(pane) {
                Arc::clone(&self.panes[pane].grid)
            } else {
                Arc::default()
            },
            scene: self.scene_for(&style),
            faces: pane_faces,
            shapes: Arc::clone(&frame.shapes),
            circles: Arc::clone(&self.drawing.circles),
            turns: Arc::clone(&self.drawing.turns),
            objects: Arc::clone(&self.drawing.objects),
            selection: Arc::clone(&self.drawing.selection),
            overlay: Arc::clone(&frame.overlay),
            preview_faces: Arc::clone(&frame.preview_faces),
            matrix: camera.matrix(size[0] / size[1]),
            light: camera.forward().map(|value| value as f32),
            style,
            area: rect,
            frame: ui.ctx().cumulative_frame_nr(),
            background: self.pane_background(pane).0,
            background_low: self.pane_background(pane).1,
            camera,
            over: Vec::new(),
        }
    }

    /// What egui paints over viewport `pane`: details and the sheet's desk, points, grips, picked
    /// parts, the gumball, guides, the snap marker and a selection box being dragged.
    pub(crate) fn paint_pane_marks(
        &mut self,
        ui: &egui::Ui,
        pane: usize,
        rect: egui::Rect,
        frame: &PaneFrame,
    ) {
        let size = pane_size(rect);
        self.paint_sheet_over_details(ui, pane, rect, frame);
        self.paint_desk(ui, pane, rect, size);
        self.paint_frame(ui, pane, rect);
        self.paint_points(ui, pane, rect, size, &frame.previewed);
        self.paint_grips(ui, pane, rect, size);
        self.paint_picked_edges(ui, pane, rect, size);
        self.paint_picked_faces(ui, pane, rect, size);
        self.paint_picked_parts(ui, pane, rect, size);
        self.paint_gumball(ui, pane, rect, size);
        self.paint_guides(ui, pane, rect, size);
        self.paint_dots(ui, pane, rect, size);
        if let Some((hover_pane, ..)) = frame.hovered
            && hover_pane == pane
            && let Some(hit) = frame.aimed.and_then(|(_, hit)| hit)
        {
            self.paint_snap_marker(ui, pane, rect, size, hit);
        }
        let dragging = self.panes[pane]
            .gestures
            .pressed_at
            .zip(ui.input(|input| input.pointer.latest_pos()))
            .filter(|(from, to)| !self.takes_position() && is_box_drag(*from, *to));
        if let Some((from, to)) = dragging {
            paint_selection_box(ui, rect, from, to);
        }
    }
}

/// The viewport under the pointer, its rectangle, and where the pointer is.
pub(crate) type Hovered = Option<(usize, egui::Rect, egui::Pos2)>;
/// Where the pointer aims in the model, and the snap it holds, if any.
pub(crate) type Aimed = Option<(Point, Option<SnapHit>)>;

/// What every viewport draws alike this frame.
pub(crate) struct PaneFrame {
    /// The previewed curves, as lines.
    pub(crate) overlay: Arc<Vec<curva_viewport::Vertex>>,
    /// The ghosted faces of a preview.
    pub(crate) preview_faces: Arc<Vec<MeshVertex>>,
    /// The previewed curves, for their points.
    pub(crate) previewed: Vec<Curve>,
    /// The faces drawn, less any a preview replaces.
    pub(crate) faces: Arc<Vec<MeshVertex>>,
    /// The shared shapes drawn, less any a preview replaces.
    pub(crate) shapes: Arc<Vec<curva_viewport::PlacedShape>>,
    pub(crate) hovered: Hovered,
    pub(crate) aimed: Aimed,
}
