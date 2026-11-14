//! Helpers several commands in this tab share.

use curva_document::Document;
use curva_document::Shape;

pub(crate) fn instances_among(doc: &Document, ids: &[u64]) -> Vec<u64> {
    doc.state()
        .objects
        .iter()
        .filter(|o| ids.contains(&o.id) && matches!(o.shape, Shape::Instance(_)))
        .map(|o| o.id)
        .collect()
}
