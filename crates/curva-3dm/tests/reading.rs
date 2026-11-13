//! Reading Rhino files: openNURBS' example files (spec `docs/specs/3dm-files.md` §4).

use curva_document::Shape;

fn open(name: &str) -> (curva_document::Document, curva_3dm::import::Report) {
    let path = format!("{}/tests/files/{name}", env!("CARGO_MANIFEST_DIR"));
    let data = std::fs::read(path).unwrap();
    let model = curva_3dm::read::read(&data).unwrap();
    curva_3dm::import::open(&model)
}

#[test]
fn every_example_file_opens_with_a_report() {
    let dir = format!("{}/tests/files", env!("CARGO_MANIFEST_DIR"));
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let (doc, report) = open(&name);
        println!(
            "{name}: {} objects; {}",
            doc.state().objects.len(),
            report.summary()
        );
    }
}

#[test]
fn curves_come_in_with_their_names_layers_and_sizes() {
    let (doc, report) = open("v7_my_curves.3dm");
    assert!(report.skipped.is_empty(), "{}", report.summary());
    let names: Vec<&str> = doc
        .state()
        .objects
        .iter()
        .map(|o| o.name.as_str())
        .collect();
    assert!(names.contains(&"radius 3 circle"), "{names:?}");
    // The file is in inches: a 3 inch circle is 76.2 mm across its radius.
    let circle = doc
        .state()
        .objects
        .iter()
        .find(|o| o.name == "radius 3 circle")
        .unwrap();
    let Shape::Curve(c) = &circle.shape else {
        panic!()
    };
    let length = c.length().unwrap();
    assert!(
        (length / (std::f64::consts::TAU * 3.0 * 25.4) - 1.0).abs() < 1e-6,
        "{length}"
    );
    let layers: Vec<&str> = doc.layers().iter().map(|l| l.name.as_str()).collect();
    assert!(layers.contains(&"blue circles"), "{layers:?}");
}

#[test]
fn a_brep_comes_in_as_one_body() {
    let (doc, report) = open("v7_my_brep.3dm");
    assert_eq!(doc.state().objects.len(), 1, "{}", report.summary());
    let Shape::Body(body) = &doc.state().objects[0].shape else {
        panic!()
    };
    assert_eq!(body.face_count().unwrap(), 6);
}

#[test]
fn a_trimmed_surface_comes_in() {
    let (doc, report) = open("v7_my_trimmed_surface.3dm");
    assert_eq!(doc.state().objects.len(), 1, "{}", report.summary());
}

#[test]
fn rhino_5_solids_come_in() {
    let (doc, report) = open("v5_teacup.3dm");
    assert_eq!(doc.state().objects.len(), 6, "{}", report.summary());
}

#[test]
fn extrusions_come_in_as_bodies() {
    let (doc, report) = open("v5_extrusions.3dm");
    let bodies = doc
        .state()
        .objects
        .iter()
        .filter(|o| matches!(o.shape, Shape::Body(_)))
        .count();
    assert_eq!(bodies, 7, "{}", report.summary());
}

#[test]
fn meshes_and_point_clouds_are_listed_as_left_out() {
    let (_, report) = open("v7_my_mesh.3dm");
    assert_eq!(report.skipped, vec![("mesh".to_owned(), 1)]);
    let (doc, report) = open("v7_my_points.3dm");
    assert_eq!(doc.state().objects.len(), 2);
    assert_eq!(report.skipped, vec![("point cloud".to_owned(), 1)]);
}
