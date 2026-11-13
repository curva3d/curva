//! Prints each extrusion's profile kinds.
fn kind(c: &curva_3dm::model::Curve) -> String {
    use curva_3dm::model::Curve;
    match c {
        Curve::Line { .. } => "line".into(),
        Curve::Polyline { .. } => "polyline".into(),
        Curve::Arc { .. } => "arc".into(),
        Curve::Nurbs(n) => format!(
            "nurbs(order {}, rational {}, n {})",
            n.order,
            n.rational,
            n.points.len()
        ),
        Curve::Poly { segments, .. } => format!(
            "poly[{}]",
            segments.iter().map(kind).collect::<Vec<_>>().join(", ")
        ),
    }
}
fn main() {
    let data = std::fs::read(std::env::args().nth(1).unwrap()).unwrap();
    let m = curva_3dm::read::read(&data).unwrap();
    for o in &m.objects {
        if let curva_3dm::model::Geometry::Extrusion(e) = &o.geometry {
            println!(
                "caps {:?}: {}",
                e.caps,
                e.profiles.iter().map(kind).collect::<Vec<_>>().join(" | ")
            );
        }
    }
}
