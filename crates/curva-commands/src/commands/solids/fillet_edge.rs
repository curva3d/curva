//! `FilletEdge`: rounds picked edges of solids by a radius, as in Rhino; Enter uses the last one.
//! `ChamferEdge` is the same command bevelling by a distance, so both live here. Rhino's radius
//! handles, `ChainEdges`, `FaceEdges`, `RailType` and the rest are still to come
//! (`docs/commands/rhino-reference.md`).

use crate::Message;
use crate::recipe::{Context, Made, Recipe, StepDef, Value};
use curva_document::Shape;

/// The recipe: rounds, or with `CHAMFER`, bevels.
#[derive(Default)]
pub struct BlendEdges<const CHAMFER: bool>;

impl<const CHAMFER: bool> Recipe for BlendEdges<CHAMFER> {
    const NAME: &'static str = if CHAMFER { "chamferedge" } else { "filletedge" };

    fn steps(&self, cx: &Context<'_>) -> Vec<StepDef> {
        let (edges, size) = if CHAMFER {
            ("prompt.chamferedge.edges", "prompt.chamferedge.size")
        } else {
            ("prompt.filletedge.edges", "prompt.filletedge.size")
        };
        let mut steps = vec![StepDef::edges(edges)];
        if !cx.values.is_empty() {
            steps.push(StepDef::number(size).remembered("Size", 1.0));
        }
        steps
    }

    fn check(&self, cx: &Context<'_>) -> Result<(), Message> {
        if let Some(size) = cx.values.get(1).and_then(Value::number)
            && size <= 0.0
        {
            return Err(Message::new("error.bad_distance"));
        }
        Ok(())
    }

    fn make(&self, cx: &Context<'_>) -> Result<Made, Message> {
        let (Some(Value::Edges(edges)), Some(size)) =
            (cx.values.first(), cx.values.get(1).and_then(Value::number))
        else {
            return Ok(Made::default());
        };
        if size <= 0.0 {
            return Ok(Made::default());
        }
        let mut owners: Vec<u64> = edges.iter().map(|(id, _)| *id).collect();
        owners.sort_unstable();
        owners.dedup();
        let mut made = Made::default();
        for id in owners {
            let Some(body) = cx
                .doc
                .state()
                .objects
                .iter()
                .find(|o| o.id == id)
                .and_then(|o| o.shape.body())
            else {
                continue;
            };
            let mine: Vec<usize> = edges
                .iter()
                .filter(|(owner, _)| *owner == id)
                .map(|(_, edge)| *edge)
                .collect();
            let blended = body
                .blended(&mine, size, CHAMFER)
                .map_err(|e| Message::with("error.geometry", vec![e.to_string()]))?;
            made.replace.push((id, Shape::Body(blended)));
        }
        let key = if CHAMFER {
            "done.chamferedge"
        } else {
            "done.filletedge"
        };
        made.message = Some(Message::with(key, vec![edges.len().to_string()]));
        Ok(made)
    }
}

/// `FilletEdge`.
pub type FilletEdge = BlendEdges<false>;

#[cfg(test)]
mod tests {
    use crate::Processor;
    use curva_document::Document;
    use curva_kernel::Body;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-6 * b.abs().max(1.0)
    }

    fn start(doc: &mut Document, command: &str) -> (Processor, u64) {
        let cube = Body::cuboid([0.0; 3], [10.0, 10.0, 10.0]).unwrap();
        let edge = cube.mesh().unwrap().edge_ids[0];
        let id = doc.add_body(cube);
        let mut processor = Processor::default();
        processor.submit(doc, command);
        doc.select_edges(&[(id, edge)]);
        processor.submit(doc, "");
        (processor, id)
    }

    #[test]
    fn a_picked_edge_is_rounded_and_the_solid_replaced() {
        let mut doc = Document::default();
        let (mut processor, id) = start(&mut doc, "filletedge");
        assert!(!processor.preview(&doc, None, "2").is_empty());
        processor.submit(&mut doc, "2");
        assert_eq!(doc.state().objects.len(), 1);
        assert_eq!(doc.state().objects[0].id, id);
        let volume = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap();
        assert!(close(
            volume,
            1000.0 - (1.0 - std::f64::consts::PI / 4.0) * 40.0
        ));
        processor.submit(&mut doc, "undo");
        let volume = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap();
        assert!(close(volume, 1000.0));
    }

    #[test]
    fn chamfer_bevels_the_edge() {
        let mut doc = Document::default();
        let (mut processor, _) = start(&mut doc, "chamferedge");
        processor.submit(&mut doc, "2");
        let volume = doc.state().objects[0]
            .shape
            .body()
            .unwrap()
            .volume()
            .unwrap();
        assert!(close(volume, 980.0));
    }

    #[test]
    fn without_picked_edges_it_keeps_asking() {
        let mut doc = Document::default();
        doc.add_body(Body::cuboid([0.0; 3], [1.0, 1.0, 1.0]).unwrap());
        let mut processor = Processor::default();
        processor.submit(&mut doc, "filletedge");
        processor.submit(&mut doc, "");
        assert!(processor.is_running());
    }
}
