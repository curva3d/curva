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
    // The file is in inches, which Curva keeps (spec `feedback-0.4.2.md` §3): the circle's
    // radius is 3.
    assert_eq!(doc.state().unit, curva_document::Unit::Inches);
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
        (length / (std::f64::consts::TAU * 3.0) - 1.0).abs() < 1e-6,
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

/// Damaged files are refused or read in part, never a crash (spec §4): each example file cut
/// short at many points, and with bytes changed through it.
#[test]
fn damaged_files_never_crash_the_reader() {
    let dir = format!("{}/tests/files", env!("CARGO_MANIFEST_DIR"));
    for entry in std::fs::read_dir(dir).unwrap() {
        let data = std::fs::read(entry.unwrap().path()).unwrap();
        // Fewer cases on the large files, which take long to read in a debug build.
        let large = data.len() >= 200_000;
        let step = (data.len() / if large { 23 } else { 97 }).max(1);
        for cut in (0..data.len()).step_by(step) {
            let _ = curva_3dm::read::read(&data[..cut]);
        }
        // A simple generator, the same every run, so a failure can be repeated.
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        // Bringing a model into Curva goes through the kernel, which is slow on large damaged
        // models: the small files, every fortieth case.
        let import = !large;
        for case in 0..if large { 30 } else { 200 } {
            let mut damaged = data.clone();
            for _ in 0..4 {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let at = 32 + (seed as usize) % (damaged.len() - 32);
                damaged[at] = (seed >> 32) as u8;
            }
            if let Ok(model) = curva_3dm::read::read(&damaged)
                && import
                && case % 40 == 0
            {
                // What does read must also come into Curva without a crash.
                let _ = curva_3dm::import::open(&model);
            }
        }
    }
}
