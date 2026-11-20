//! The kinds of file each file dialog offers, in order, written down before the formats move onto
//! one list (spec `docs/specs/file-formats.md` §4, step 1). Every later step keeps these the same;
//! since step 5 the lists come from the full list of formats the interface hands every model.

use curva_commands::Processor;
use curva_document::Document;

/// The formats the dialog offers when `typed` is entered with file dialogs on: each one's name
/// key and its extensions.
fn offered(doc: &mut Document, typed: &[&str]) -> Vec<(String, Vec<String>)> {
    let mut app = Processor::default();
    app.use_file_dialogs(true);
    for line in typed {
        app.submit(doc, line);
    }
    let dialog = app
        .current_request()
        .and_then(|r| r.file.clone())
        .unwrap_or_else(|| panic!("{typed:?} shows no file dialog"));
    dialog
        .formats
        .iter()
        .map(|(name, extensions)| {
            (
                (*name).to_owned(),
                extensions.iter().map(|e| (*e).to_owned()).collect(),
            )
        })
        .collect()
}

fn list(formats: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
    formats
        .iter()
        .map(|(name, extensions)| {
            (
                (*name).to_owned(),
                extensions.iter().map(|e| (*e).to_owned()).collect(),
            )
        })
        .collect()
}

#[test]
fn open_offers_these_formats_in_this_order() {
    assert_eq!(
        offered(&mut curva_formats::new_document(), &["Open"]),
        list(&[
            ("files.curva", &["curva"]),
            ("files.rhino", &["3dm"]),
            ("files.dxf", &["dxf"]),
            ("files.dwg", &["dwg"]),
            ("files.skp", &["skp"]),
            ("files.stl", &["stl"]),
            ("files.obj", &["obj"]),
            ("files.ply", &["ply"]),
        ])
    );
}

#[test]
fn import_offers_these_formats_in_this_order() {
    assert_eq!(
        offered(&mut curva_formats::new_document(), &["Import"]),
        list(&[
            ("files.curva", &["curva"]),
            ("files.rhino", &["3dm"]),
            ("files.dxf", &["dxf"]),
            ("files.dwg", &["dwg"]),
            ("files.stl", &["stl"]),
            ("files.obj", &["obj"]),
            ("files.ply", &["ply"]),
        ])
    );
}

#[test]
fn save_and_save_as_offer_these_formats_in_this_order() {
    let saved = list(&[("files.curva", &["curva"]), ("files.rhino", &["3dm"])]);
    assert_eq!(
        offered(&mut curva_formats::new_document(), &["Save"]),
        saved
    );
    assert_eq!(
        offered(&mut curva_formats::new_document(), &["SaveAs"]),
        saved
    );
}

#[test]
fn export_and_export_selected_offer_these_formats_in_this_order() {
    let exported = list(&[
        ("files.rhino", &["3dm"]),
        ("files.dxf", &["dxf"]),
        ("files.dwg", &["dwg"]),
        ("files.step", &["stp", "step"]),
        ("files.stl", &["stl"]),
        ("files.obj", &["obj"]),
        ("files.ply", &["ply"]),
        ("files.svg", &["svg"]),
    ]);
    let mut doc = curva_formats::new_document();
    assert_eq!(offered(&mut doc, &["Export"]), exported);
    let mut processor = Processor::default();
    processor.submit(&mut doc, "Line");
    processor.submit(&mut doc, "0,0");
    processor.submit(&mut doc, "5,0");
    let line = doc.state().objects[0].id;
    doc.select(&[line]);
    assert_eq!(offered(&mut doc, &["ExportSelected"]), exported);
}

#[test]
fn printing_to_a_file_offers_pdf() {
    let mut doc = curva_formats::new_document();
    let paper = curva_document::layouts::paper_size("A3").unwrap();
    doc.add_layout("Plans".to_owned(), paper, true);
    assert_eq!(
        offered(&mut doc, &["ExportPDF", ""]),
        list(&[("files.pdf", &["pdf"])])
    );
}
