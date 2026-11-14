//! `ChamferEdge`: bevels picked edges of solids by a distance, as in Rhino. It works as
//! `FilletEdge` does, so it is that command set to bevel.

/// `ChamferEdge`.
pub type ChamferEdge = crate::commands::fillet_edge::BlendEdges<true>;
