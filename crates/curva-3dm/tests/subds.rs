//! Rhino's SubDs read and written (#323; spec `docs/specs/mesh-and-subd.md` §5).
//!
//! `v8_subds.3dm` was saved by Rhino 8 (`File3dm.Write`, 17 November 2026) with three SubDs made by
//! `SubD.CreateFromMesh`: a smooth cube of side 2 with its first face coloured (200, 10, 30); an
//! open 3 × 3 grid of side 3; and a cube of side 2 creased on every edge. Rhino gave their limit
//! surfaces as 2.620377 and 9.197474 (volume, area), area 8.666667, and 8 and 24.

use curva_3dm::model::{Attributes, Geometry, Model, Object};
use curva_mesh::{SubD, VertexTag};

fn read(name: &str) -> Model {
    let path = format!("{}/tests/files/{name}", env!("CARGO_MANIFEST_DIR"));
    curva_3dm::read::read(&std::fs::read(path).unwrap()).unwrap()
}

fn subds(model: &Model) -> Vec<SubD> {
    model
        .objects
        .iter()
        .filter_map(|o| match &o.geometry {
            Geometry::SubD(s) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

fn round_trip(these: &[SubD]) -> Vec<SubD> {
    let model = Model {
        objects: these
            .iter()
            .map(|s| Object {
                geometry: Geometry::SubD(s.clone()),
                attributes: Attributes::default(),
            })
            .collect(),
        ..Model::default()
    };
    let back = curva_3dm::read::read(&curva_3dm::write::write(&model)).unwrap();
    assert!(back.skipped.is_empty(), "{:?}", back.skipped);
    subds(&back)
}

fn near(a: f64, b: f64) -> bool {
    (a / b - 1.0).abs() < 5e-4
}

#[test]
fn rhinos_three_subds_come_in_with_their_tags() {
    let all = subds(&read("v8_subds.3dm"));
    assert_eq!(all.len(), 3);
    let (smooth, grid, creased) = (&all[0], &all[1], &all[2]);
    assert_eq!(
        (smooth.cage().vertex_count(), smooth.cage().face_count()),
        (8, 6)
    );
    assert!((0..8).all(|v| smooth.vertex_tag(v) == VertexTag::Smooth));
    assert_eq!(smooth.creases().count(), 0);
    let colours = smooth.face_colours().expect("one face is coloured");
    assert_eq!(
        colours[0],
        Some([200.0 / 255.0, 10.0 / 255.0, 30.0 / 255.0])
    );
    assert!(colours[1..].iter().all(Option::is_none));

    assert_eq!(
        (grid.cage().vertex_count(), grid.cage().face_count()),
        (16, 9)
    );
    assert_eq!(grid.creases().count(), 12, "its border");
    let tags: Vec<VertexTag> = (0..16).map(|v| grid.vertex_tag(v)).collect();
    assert_eq!(tags.iter().filter(|t| **t == VertexTag::Crease).count(), 12);
    assert_eq!(tags.iter().filter(|t| **t == VertexTag::Smooth).count(), 4);

    assert_eq!(creased.creases().count(), 12);
    assert!((0..8).all(|v| creased.vertex_tag(v) == VertexTag::Corner));
}

#[test]
fn rhinos_subds_smooth_to_rhinos_limit_surfaces() {
    let all = subds(&read("v8_subds.3dm"));
    let smooth = all[0].surface(6).unwrap();
    assert!(
        near(smooth.volume().unwrap(), 2.620_377),
        "{:?}",
        smooth.volume()
    );
    assert!(near(smooth.area(), 9.197_474), "{}", smooth.area());
    let grid = all[1].surface(6).unwrap();
    assert!(near(grid.area(), 8.666_667), "{}", grid.area());
    let creased = all[2].surface(2).unwrap();
    assert!(near(creased.volume().unwrap(), 8.0) && near(creased.area(), 24.0));
}

#[test]
fn rhinos_subds_go_back_out_unchanged() {
    let before = subds(&read("v8_subds.3dm"));
    assert_eq!(round_trip(&before), before);
}

#[test]
fn texture_points_and_partly_sharp_edges_go_out_and_back() {
    let points = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.0, 1.0, 1.0],
    ];
    let faces = [
        [0, 3, 2, 1],
        [4, 5, 6, 7],
        [0, 1, 5, 4],
        [1, 2, 6, 5],
        [2, 3, 7, 6],
        [3, 0, 4, 7],
    ];
    let cage = curva_mesh::Mesh::new(points, &faces).unwrap();
    let texture: Vec<[f64; 2]> = (0..24)
        .map(|i| [f64::from(i % 4) * 0.25, f64::from(i / 4) * 0.125])
        .collect();
    let cage = cage
        .with_texture(curva_mesh::Attribute::PerCorner(texture))
        .unwrap();
    let subd = SubD::new(cage)
        .with_creases(&[[0, 1]])
        .with_sharpness([1, 2], curva_mesh::Sharpness::new(2.5));
    let back = &round_trip(std::slice::from_ref(&subd))[0];
    assert_eq!(back.cage().texture(), subd.cage().texture());
    assert!(back.sharpness(0, 1).is_crease());
    assert_eq!(back.sharpness(1, 2).value(), 2.5);
}

/// `v8_subd_corners.3dm`, saved by Rhino 8 the same day: the open grid with its four outer points
/// tagged as corners (`ConvexCornersAndInteriorCreases`), whose limit surface Rhino gives as the
/// whole square, area 9.
#[test]
fn rhinos_corners_come_in_and_keep_the_square_whole() {
    let all = subds(&read("v8_subd_corners.3dm"));
    let grid = &all[0];
    let tags: Vec<VertexTag> = (0..16).map(|v| grid.vertex_tag(v)).collect();
    assert_eq!(tags.iter().filter(|t| **t == VertexTag::Corner).count(), 4);
    let surface = grid.surface(4).unwrap();
    assert!(near(surface.area(), 9.0), "{}", surface.area());
    assert_eq!(round_trip(&all), all);
}

#[test]
fn an_unwelded_cut_stays_cut_out_and_back() {
    // A 2 x 2 x 2 box cut along the two edges at x = 1 on its y = 0 side, as Rhino's UnweldEdge:
    // its middle point is there twice, and reading must not join the copies.
    let mesh = curva_mesh::Mesh::mesh_box(&curva_mesh::Frame::world(), [2.0; 3], [2; 3]);
    let subd = SubD::new(mesh);
    let p = subd.cage().vertices();
    let on_cut = |v: u32| p[v as usize][0] == 1.0 && p[v as usize][1] == 0.0;
    let mut edges = Vec::new();
    for face in subd.cage().faces() {
        for k in 0..face.len() {
            let (a, b) = (face[k], face[(k + 1) % face.len()]);
            if a < b && on_cut(a) && on_cut(b) {
                edges.push([a, b]);
            }
        }
    }
    let cut = subd.unwelded(&edges).unwrap();
    let back = round_trip(std::slice::from_ref(&cut));
    assert_eq!(back[0].cage().vertex_count(), 27);
    assert_eq!(back[0].creases().count(), 4);
}
