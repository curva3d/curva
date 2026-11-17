//! Rhino's meshes read and written (#275; spec `docs/specs/mesh-and-subd.md` §5).
//!
//! `v8_mesh_ngons.3dm` was saved by Rhino 8 (`File3dm.Write`, 17 November 2026): a five-sided
//! prism a million units from the origin, its sides quads and its ends n-gons, with a colour, a
//! texture coordinate and a normal at each point.

use curva_3dm::model::{Attributes, Geometry, Model, Object};
use curva_mesh::{Attribute, Mesh, Point};

fn read(name: &str) -> Model {
    let path = format!("{}/tests/files/{name}", env!("CARGO_MANIFEST_DIR"));
    curva_3dm::read::read(&std::fs::read(path).unwrap()).unwrap()
}

fn meshes(model: &Model) -> Vec<Mesh> {
    model
        .objects
        .iter()
        .filter_map(|o| match &o.geometry {
            Geometry::Mesh(m) => Some(m.clone()),
            _ => None,
        })
        .collect()
}

/// The meshes written into a file of their own and read back.
fn round_trip(meshes_in: &[Mesh]) -> Vec<Mesh> {
    let model = Model {
        objects: meshes_in
            .iter()
            .map(|m| Object {
                geometry: Geometry::Mesh(m.clone()),
                attributes: Attributes::default(),
            })
            .collect(),
        ..Model::default()
    };
    let back = curva_3dm::read::read(&curva_3dm::write::write(&model)).unwrap();
    assert!(back.skipped.is_empty(), "{:?}", back.skipped);
    meshes(&back)
}

#[test]
fn rhinos_pyramid_comes_in_with_its_hard_base_edge() {
    let mesh = &meshes(&read("v7_my_mesh.3dm"))[0];
    // Rhino keeps the base's hard edge as a second set of points at the same places.
    assert_eq!((mesh.vertex_count(), mesh.face_count()), (9, 5));
    assert_eq!(mesh.face(4), &[5, 8, 7, 6]);
    assert!(mesh.is_closed());
    assert_eq!(mesh.volume(), Some(10.0));
    let Attribute::PerVertex(n) = mesh.normals() else {
        panic!("Rhino keeps a normal per point")
    };
    assert_eq!(n[5], [0.0, 0.0, -1.0]);
}

#[test]
fn rhinos_n_gons_colours_and_far_points_come_in() {
    let mesh = &meshes(&read("v8_mesh_ngons.3dm"))[0];
    // Five quads and two five-sided ends, each end one face rather than Rhino's three triangles.
    assert_eq!((mesh.vertex_count(), mesh.face_count()), (10, 7));
    assert_eq!(mesh.face(5), &[0, 4, 3, 2, 1]);
    assert_eq!(mesh.face(6), &[5, 6, 7, 8, 9]);
    // Rhino's double-precision points, not its single-precision copies.
    assert_eq!(mesh.vertices()[0], [1_000_001.1, 0.0, 0.0]);
    assert!(mesh.is_closed());
    // Rhino gave the volume as 4.755282581.
    assert!((mesh.volume().unwrap() - 4.755_282_581).abs() < 1e-8);
    let Attribute::PerVertex(c) = mesh.colours() else {
        panic!()
    };
    assert_eq!(c[1], [25.0 / 255.0, 230.0 / 255.0, 100.0 / 255.0]);
    let Attribute::PerVertex(t) = mesh.texture() else {
        panic!()
    };
    assert_eq!(t[3], [0.3, 0.25]);
    assert!(!mesh.normals().is_none());
}

#[test]
fn rhinos_meshes_go_back_out_unchanged() {
    for name in ["v7_my_mesh.3dm", "v8_mesh_ngons.3dm"] {
        let before = meshes(&read(name));
        assert_eq!(round_trip(&before), before, "{name}");
    }
}

/// A unit cube from the origin.
fn cube() -> Mesh {
    let v: Vec<Point> = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.0, 1.0, 1.0],
    ];
    let faces: [[u32; 4]; 6] = [
        [0, 3, 2, 1],
        [4, 5, 6, 7],
        [0, 1, 5, 4],
        [1, 2, 6, 5],
        [2, 3, 7, 6],
        [3, 0, 4, 7],
    ];
    Mesh::new(v, &faces).unwrap()
}

#[test]
fn data_kept_per_corner_goes_out_as_rhino_keeps_it_and_comes_back_on_each_corner() {
    let texture: Vec<[f64; 2]> = (0..24)
        .map(|i| [f64::from(i % 4), f64::from(i / 4)])
        .collect();
    let mesh = cube()
        .with_texture(Attribute::PerCorner(texture))
        .unwrap()
        .translated([1.0e6 + 0.1, 0.0, 0.0]);
    let back = &round_trip(std::slice::from_ref(&mesh))[0];
    assert_eq!((back.vertex_count(), back.face_count()), (24, 6));
    for c in 0..24 {
        let (v, w) = (mesh.corners()[c] as usize, back.corners()[c] as usize);
        assert_eq!(back.vertices()[w], mesh.vertices()[v], "corner {c}");
        assert_eq!(
            back.texture().at(c, w),
            mesh.texture().at(c, v),
            "corner {c}"
        );
    }
    assert!(back.is_closed());
}

#[test]
fn faces_of_many_corners_go_out_as_n_gons_and_come_back_whole() {
    let n = 300u32;
    let mut points: Vec<Point> = (0..n)
        .map(|k| {
            let a = std::f64::consts::TAU * f64::from(k) / f64::from(n);
            [a.cos(), a.sin(), 0.0]
        })
        .collect();
    points.push([5.0, 5.0, 5.0]); // a point no face uses stays too
    let ring: Vec<u32> = (0..n).collect();
    let colours = vec![[0.2, 0.4, 0.6]; points.len()];
    let disc = Mesh::new(points, &[ring])
        .unwrap()
        .with_colours(Attribute::PerVertex(colours.clone()))
        .unwrap();
    let back = &round_trip(std::slice::from_ref(&disc))[0];
    assert_eq!(back.face_count(), 1);
    assert_eq!(back.face(0).len(), 300);
    assert_eq!(back.vertices(), disc.vertices());
    // Colours go through bytes: each comes back as the nearest of Rhino's 256 steps.
    let Attribute::PerVertex(c) = back.colours() else {
        panic!()
    };
    assert!(
        c.iter()
            .all(|c| (c[0] - 0.2).abs() < 0.002 && (c[2] - 0.6).abs() < 0.002)
    );
}

#[test]
fn an_empty_mesh_goes_out_and_back() {
    assert_eq!(round_trip(&[Mesh::default()]), vec![Mesh::default()]);
}
