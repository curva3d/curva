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
        } else if request.kind == Kind::MeshParts && !doc.mesh_part_selection().is_empty() {
            // Parts picked before the command, as Rhino's preselection.
            let parts = doc.mesh_part_selection();
            doc.select_mesh_parts(&[]);
            self.advance(doc, Some(Answer::MeshParts(parts)));
        } else if request.allow_empty {
            self.advance(doc, Some(Answer::Empty));
        } else {
            self.say(&Message::new("error.needs_input"));
        }
    }
}
