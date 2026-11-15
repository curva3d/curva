//! Prints what a `.3dm` file holds.
fn main() {
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).expect("readable");
        match curva_3dm::read::read(&data) {
            Ok(m) => {
                println!(
                    "{path}: version {} unit {} tol {}",
                    m.version, m.unit, m.tolerance
                );
                println!(
                    "  layers: {:?}",
                    m.layers
                        .iter()
                        .map(|l| (&l.name, l.color, l.visible, l.locked))
                        .collect::<Vec<_>>()
                );
                println!(
                    "  views {:?} cplanes {:?}",
                    m.views
                        .iter()
                        .map(|v| (&v.name, v.parallel))
                        .collect::<Vec<_>>(),
                    m.cplanes.iter().map(|c| &c.name).collect::<Vec<_>>()
                );
                println!(
                    "  groups {} blocks {:?}",
                    m.groups.len(),
                    m.blocks
                        .iter()
                        .map(|b| (&b.name, b.objects.len()))
                        .collect::<Vec<_>>()
                );
                let mut kinds = std::collections::BTreeMap::new();
                for o in &m.objects {
                    let k = match &o.geometry {
                        curva_3dm::model::Geometry::Point(_) => "point".to_owned(),
                        curva_3dm::model::Geometry::Dot { .. } => "dot".to_owned(),
                        curva_3dm::model::Geometry::Hatch(_) => "hatch".to_owned(),
                        curva_3dm::model::Geometry::Dimension(_) => "linear dimension".to_owned(),
                        curva_3dm::model::Geometry::Radial(_) => "radial dimension".to_owned(),
                        curva_3dm::model::Geometry::Angular(_) => "angle dimension".to_owned(),
                        curva_3dm::model::Geometry::Leader(_) => "leader".to_owned(),
                        curva_3dm::model::Geometry::Text(t) => format!(
                            "text {:?} own {:?} x {:?}",
                            t.rich,
                            t.own
                                .as_ref()
                                .map(|s| (s.text_height, s.h_align, s.v_align)),
                            t.plane.x
                        ),
                        curva_3dm::model::Geometry::Curve(c) => format!(
                            "curve {}",
                            match c {
                                curva_3dm::model::Curve::Line { .. } => "line",
                                curva_3dm::model::Curve::Polyline { .. } => "polyline",
                                curva_3dm::model::Curve::Arc { .. } => "arc",
                                curva_3dm::model::Curve::Nurbs(_) => "nurbs",
                                curva_3dm::model::Curve::Poly { .. } => "poly",
                            }
                        ),
                        curva_3dm::model::Geometry::Surface(s) => format!(
                            "surface {}",
                            match s {
                                curva_3dm::model::Surface::Nurbs(_) => "nurbs",
                                curva_3dm::model::Surface::Plane { .. } => "plane",
                                curva_3dm::model::Surface::Revolution { .. } => "rev",
                                curva_3dm::model::Surface::Sum { .. } => "sum",
                            }
                        ),
                        curva_3dm::model::Geometry::Brep(b) => {
                            format!("brep faces {}", b.faces.len())
                        }
                        curva_3dm::model::Geometry::Extrusion(_) => "extrusion".to_owned(),
                        curva_3dm::model::Geometry::Instance { .. } => "instance".to_owned(),
                    };
                    *kinds.entry(k).or_insert(0) += 1;
                }
                println!("  objects {kinds:?}");
                println!("  styles {:?}", m.dim_styles);
                println!(
                    "  names {:?}",
                    m.objects
                        .iter()
                        .map(|o| &o.attributes.name)
                        .filter(|n| !n.is_empty())
                        .take(8)
                        .collect::<Vec<_>>()
                );
                println!("  skipped {:?}", m.skipped);
            }
            Err(e) => println!("{path}: ERROR {e}"),
        }
    }
}
