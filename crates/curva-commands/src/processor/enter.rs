//! Enter with nothing typed: what was picked beforehand answers the question, as Rhino takes a
//! preselection, or Enter takes the default.

use super::{Active, Answer, Document, Kind, Message, Processor, Request};

/// When a command ends, what was picked as its answers while it ran is let go of, as in Rhino,
/// so Enter to repeat it does not take them again; a selection made before it stays (#680).
pub(super) fn let_go_of_picks(doc: &mut Document, active: &Active) {
    // Left alone when the command set a selection of its own, such as SelColor's (#758).
    if !active.selection_before.is_empty()
        || active.picked.is_empty()
        || doc.selection() != active.selection_at_pick
    {
        return;
    }
    let kept: Vec<u64> = doc
        .selection()
        .into_iter()
        .filter(|id| !active.picked.contains(id))
        .collect();
    doc.select(&kept);
}

/// The parts of meshes picked that answer `request`: only the faces for a step that takes
/// faces (#819), else all of them.
fn parts_for(doc: &Document, request: &Request) -> Vec<curva_document::MeshPick> {
    let mut parts = doc.mesh_part_selection();
    if request.faces_only {
        parts.retain(|(_, part)| matches!(part, curva_document::MeshPart::Face(_)));
    }
    parts
}

impl Processor {
    /// Answers `request` with what is picked: objects, edges, faces or parts of meshes; edges
    /// picked with Ctrl+Shift where objects are asked for, lent as curves, as Rhino lets a
    /// command asking for curves take another object's edge; else Enter's default.
    pub(super) fn answer_enter(&mut self, doc: &mut Document, request: &Request) {
        let selection = doc.selection();
        let edges = doc.edge_selection();
        if request.kind == Kind::Objects && !selection.is_empty() {
            self.advance(doc, Some(Answer::Objects(selection)));
        } else if request.kind == Kind::Objects && !edges.is_empty() {
            let lent = doc.lend_picked_edges();
            self.advance(doc, Some(Answer::Objects(lent)));
        } else if request.kind == Kind::Edges && !edges.is_empty() {
            doc.select_edges(&[]);
            self.advance(doc, Some(Answer::Edges(edges)));
        } else if request.kind == Kind::Faces && !doc.face_selection().is_empty() {
            let faces = doc.face_selection();
            doc.select_faces(&[]);
            self.advance(doc, Some(Answer::Faces(faces)));
        } else if request.kind == Kind::MeshParts && !parts_for(doc, request).is_empty() {
            // Parts picked before the command, as Rhino's preselection; only faces for a step
            // that takes faces (#819).
            let parts = parts_for(doc, request);
            doc.select_mesh_parts(&[]);
            self.advance(doc, Some(Answer::MeshParts(parts)));
        } else if request.allow_empty {
            self.advance(doc, Some(Answer::Empty));
        } else {
            self.say(&Message::new("error.needs_input"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_document::{MeshPart, Shape};
    use curva_mesh::{Frame, Mesh};

    #[test]
    fn faces_picked_beforehand_answer_a_faces_only_step_and_points_and_edges_do_not() {
        let mut doc = Document::default();
        doc.add_shape(Shape::Mesh(Mesh::mesh_box(
            &Frame::world(),
            [1.0; 3],
            [1, 1, 1],
        )));
        let id = doc.state().objects[0].id;
        let picked = [
            (id, MeshPart::Point(0)),
            (id, MeshPart::edge(0, 1)),
            (id, MeshPart::Face(2)),
        ];
        doc.select_mesh_parts(&picked);
        let mut request = Request::new("prompt.extractmeshfaces.faces", Kind::MeshParts);
        assert_eq!(parts_for(&doc, &request).len(), 3);
        request.faces_only = true;
        assert_eq!(parts_for(&doc, &request), vec![(id, MeshPart::Face(2))]);
    }
}
