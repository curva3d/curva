//! Tests of the SubD edits (`subd_edit.rs`), each against what Rhino 8 does.

use super::*;
use crate::mesh::tests::cube;

fn open_box() -> SubD {
    SubD::new(cube()).without_faces(&[1]).unwrap()
}

#[test]
fn remove_crease_smooths_inner_edges_but_not_the_border() {
    let creased = SubD::new(cube()).with_creases(&[[0, 1], [1, 2]]);
    let smooth = creased.without_creases(&[[1, 0]]);
    assert!(!smooth.sharpness(0, 1).is_crease());
    assert!(smooth.sharpness(1, 2).is_crease());
    let open = open_box();
    let border: Vec<[u32; 2]> = open.creases().collect();
    assert_eq!(open.clone().without_creases(&border).creases().count(), 4);
    let cornered = SubD::new(cube()).with_corners(&[3]).without_corners(&[3]);
    assert_eq!(cornered.vertex_tag(3), VertexTag::Smooth);
}

#[test]
fn deleting_a_face_opens_a_creased_border_and_keeps_other_creases() {
    let subd = SubD::new(cube()).with_creases(&[[0, 3]]);
    let open = subd.without_faces(&[1]).unwrap();
    assert_eq!(open.cage().face_count(), 5);
    assert_eq!(open.cage().naked_edges().len(), 4);
    assert_eq!(
        open.creases().count(),
        5,
        "the four border edges and the kept crease"
    );
    let two = subd.without_faces(&[0, 1]).unwrap();
    assert_eq!(two.cage().vertex_count(), 8);
    let lone = SubD::new(cube()).without_faces(&[0, 1, 2, 3, 4]).unwrap();
    assert_eq!(
        (lone.cage().vertex_count(), lone.cage().face_count()),
        (4, 1)
    );
}

#[test]
fn fill_closes_the_hole_in_each_style() {
    let open = open_box();
    let single = open.filled(FillStyle::SingleFace).unwrap();
    assert!(single.cage().is_closed());
    assert_eq!(single.cage().face_count(), 6);
    assert_eq!(
        single.creases().count(),
        0,
        "the old border is smooth once filled"
    );
    assert!((single.cage().volume().unwrap() - 1.0).abs() < 1e-12);
    let fan = open.filled(FillStyle::TriangleFan).unwrap();
    assert_eq!(fan.cage().face_count(), 5 + 4);
    let quads = open.filled(FillStyle::Automatic).unwrap();
    assert_eq!(
        quads.cage().face_count(),
        5 + 2,
        "two quads for a square hole"
    );
    assert!(quads.cage().faces().skip(5).all(|f| f.len() == 4));
    assert!(quads.cage().is_closed());
}

#[test]
fn fill_closes_only_the_hole_whose_edge_is_picked() {
    // Rhino 8's Fill takes the picked border edges' holes. A cube without two opposite faces
    // has two holes; picking an edge of one fills that one.
    let open = SubD::new(cube()).without_faces(&[0, 1]).unwrap();
    let border: Vec<[u32; 2]> = open.creases().collect();
    assert_eq!(border.len(), 8);
    let one = open.filled_at(&border[..1], FillStyle::SingleFace).unwrap();
    assert_eq!(one.cage().face_count(), 5);
    assert_eq!(one.creases().count(), 4, "the other hole stays open");
    let none = open.filled_at(&[], FillStyle::SingleFace).unwrap();
    assert_eq!(none.cage().face_count(), 4);
}

#[test]
fn triangulated_fill_adds_no_point() {
    let open = open_box();
    let filled = open.filled(FillStyle::Triangulated).unwrap();
    assert_eq!(filled.cage().vertex_count(), open.cage().vertex_count());
    assert_eq!(filled.cage().face_count(), 5 + 2);
    assert!(filled.cage().is_closed());
    assert!((filled.cage().volume().unwrap() - 1.0).abs() < 1e-12);
}

#[test]
fn stitch_joins_points_where_asked() {
    // Two separate squares side by side, a gap between them.
    let points = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.2, 0.0, 0.0],
        [2.2, 0.0, 0.0],
        [2.2, 1.0, 0.0],
        [1.2, 1.0, 0.0],
    ];
    let two = SubD::new(Mesh::new(points, &[[0, 1, 2, 3], [4, 5, 6, 7]]).unwrap());
    let joined = two.stitched(&[[1, 4], [2, 7]], StitchAt::Average).unwrap();
    assert_eq!(joined.cage().vertex_count(), 6);
    assert_eq!(joined.cage().vertices()[1], [1.1, 0.0, 0.0]);
    assert_eq!(joined.creases().count(), 6, "the shared edge is inside now");
    let first = two.stitched(&[[1, 4], [2, 7]], StitchAt::First).unwrap();
    assert_eq!(first.cage().vertices()[1], [1.0, 0.0, 0.0]);
}

#[test]
fn a_ring_cut_splits_the_quads_round_a_cube() {
    let subd = SubD::new(cube());
    // Edge 0–1 runs along x; the ring round x passes the bottom, back, top and front faces.
    let cut = subd.with_ring_cut([0, 1], 0.25).unwrap();
    assert_eq!(cut.cage().face_count(), 10);
    assert_eq!(cut.cage().vertex_count(), 12);
    assert!(cut.cage().is_closed());
    assert!((cut.cage().volume().unwrap() - 1.0).abs() < 1e-12);
    assert!(
        cut.cage().vertices()[8..]
            .iter()
            .all(|p| (p[0] - 0.25).abs() < 1e-12)
    );
}

#[test]
fn a_ring_cut_through_the_middle_of_an_open_strip_cuts_both_ways() {
    // Three squares in a row, cut across the middle one's shared edges.
    let points: Vec<Point> = (0..4)
        .flat_map(|i| [[0.0, f64::from(i), 0.0], [1.0, f64::from(i), 0.0]])
        .collect();
    let faces = [[0, 1, 3, 2], [2, 3, 5, 4], [4, 5, 7, 6]];
    let strip = SubD::new(Mesh::new(points, &faces).unwrap());
    let cut = strip.with_ring_cut([2, 3], 0.5).unwrap();
    assert_eq!(
        cut.cage().face_count(),
        6,
        "every square of the strip is cut"
    );
    assert_eq!(cut.cage().vertex_count(), 12);
}

#[test]
fn bridge_joins_two_borders_with_rows_of_quads() {
    // Two open squares facing each other, a unit apart.
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
    let two = SubD::new(Mesh::new(points, &[[0, 3, 2, 1], [4, 5, 6, 7]]).unwrap());
    let bridged = two.bridged(&[0, 1], &[4, 5], 2, 1.0).unwrap();
    assert_eq!(bridged.cage().face_count(), 4);
    assert_eq!(bridged.cage().vertex_count(), 10);
    // The new faces join the borders, so those edges are no longer creased.
    assert!(!bridged.sharpness(0, 1).is_crease());
    assert!(bridged.cage().check(1e-9).misdirected_faces == Some(0));
}

/// Rhino's test: a 2 × 2 × 2 box from a mesh, and the two edges at x = 1 on its y = 0 side.
fn box_and_cut() -> (SubD, Vec<[u32; 2]>) {
    let mesh = Mesh::mesh_box(&crate::Frame::world(), [2.0; 3], [2; 3]);
    let subd = SubD::new(mesh);
    let p = subd.cage().vertices();
    let on_cut = |v: u32| p[v as usize][0] == 1.0 && p[v as usize][1] == 0.0;
    let edges = subd
        .edge_uses()
        .keys()
        .filter(|&&(a, b)| on_cut(a) && on_cut(b))
        .map(|&(a, b)| [a, b])
        .collect();
    (subd, edges)
}

#[test]
fn unweld_cuts_a_box_open_as_rhino_does() {
    let (subd, edges) = box_and_cut();
    assert_eq!(edges.len(), 2);
    assert_eq!(subd.cage().vertex_count(), 26);
    let cut = subd.unwelded(&edges).unwrap();
    // Rhino 8's UnweldEdge on the same box: 27 points, 50 edges, 4 creased, not closed. Only
    // the middle point is copied; the cut's ends stay whole inside the surface.
    assert_eq!(cut.cage().vertex_count(), 27);
    assert_eq!(cut.edge_uses().len(), 50);
    assert_eq!(cut.creases().count(), 4);
    let tags = (0..27).filter(|&v| cut.vertex_tag(v) == VertexTag::Crease);
    assert_eq!(tags.count(), 4);
    assert_eq!(cut.cage().vertices()[26], [1.0, 0.0, 1.0]);
}

#[test]
fn weld_joins_the_cut_back_as_rhino_does() {
    let (subd, edges) = box_and_cut();
    let cut = subd.unwelded(&edges).unwrap();
    let border: Vec<[u32; 2]> = cut.creases().collect();
    let welded = cut.welded(&border).unwrap();
    // The same cage, smooth again.
    assert_eq!(welded.cage().vertices(), subd.cage().vertices());
    assert!(welded.cage().faces().eq(subd.cage().faces()));
    assert_eq!(welded.creases().count(), 0);
    // One side of the cut is enough: the edges that do not use the copy, point 26.
    let one_side: Vec<[u32; 2]> = border
        .iter()
        .copied()
        .filter(|e| !e.contains(&26))
        .collect();
    assert_eq!(one_side.len(), 2);
    assert_eq!(cut.welded(&one_side).unwrap().creases().count(), 0);
}

#[test]
fn weld_and_unweld_refuse_edges_they_cannot_use() {
    let (subd, edges) = box_and_cut();
    assert_eq!(
        subd.welded(&edges),
        Err(MeshError::UnusableEdge { edge: edges[0] }),
        "an inner edge is not a border"
    );
    let open = open_box();
    let border: Vec<[u32; 2]> = open.creases().collect();
    assert!(open.welded(&border).is_err(), "no other edge at its place");
    assert!(open.unwelded(&border).is_err(), "a border is open already");
    // Rhino welds nothing that is merely close.
    let cut = subd.unwelded(&edges).unwrap();
    let mut points = cut.cage().vertices().to_vec();
    points[26][0] += 0.0006;
    let moved = cut.clone().with_cage_points(points);
    let border: Vec<[u32; 2]> = moved.creases().collect();
    assert!(moved.welded(&border).is_err());
}

#[test]
fn a_cut_right_across_a_grid_splits_it_and_keeps_its_creases() {
    // A 2 × 2 grid of squares, cut along its middle line from border to border.
    let points: Vec<Point> = (0..9)
        .map(|i| [f64::from(i % 3), f64::from(i / 3), 0.0])
        .collect();
    let faces = [[0, 1, 4, 3], [1, 2, 5, 4], [3, 4, 7, 6], [4, 5, 8, 7]];
    let grid = SubD::new(Mesh::new(points, &faces).unwrap()).with_creases(&[[3, 4]]);
    let cut = grid.unwelded(&[[1, 4], [4, 7]]).unwrap();
    assert_eq!(cut.cage().vertex_count(), 12, "the whole line is copied");
    assert_eq!(
        cut.creases().count(),
        8 + 4 + 1,
        "border, both sides of the cut, 3-4"
    );
    // The crease from 3 to 4 stays on the left piece, which keeps point 4.
    assert!(cut.sharpness(3, 4).is_crease());
    let p = cut.cage().vertices();
    let seam: Vec<[u32; 2]> = cut
        .creases()
        .filter(|e| e.iter().all(|&v| p[v as usize][0] == 1.0))
        .collect();
    assert_eq!(seam.len(), 4);
    let welded = cut.welded(&seam).unwrap();
    assert_eq!(welded.cage().vertex_count(), 9);
    assert_eq!(welded.creases().count(), 8 + 1);
}

#[test]
fn joining_keeps_each_subds_creases_and_corners() {
    let a = SubD::new(cube()).with_creases(&[[0, 1]]).with_corners(&[2]);
    let b = SubD::new(cube()).with_creases(&[[4, 5]]);
    let joined = a.joined(&b).unwrap();
    assert_eq!(joined.cage().vertex_count(), 16);
    assert_eq!(joined.cage().face_count(), 12);
    assert!(joined.sharpness(0, 1).is_crease());
    assert!(
        joined.sharpness(12, 13).is_crease(),
        "b's crease, numbered after a's points"
    );
    assert_eq!(joined.vertex_tag(2), VertexTag::Corner);
    assert_eq!(joined.creases().count(), 2);
}
