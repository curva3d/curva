//! Brings older `.curva` files up to date before they are read, so that the model's types
//! describe only the current format (`docs/reviews/code-health-2026-11-14.md`, smaller points).
//!
//! A file says which version of the format it is in. Each step below takes the file's JSON
//! from one version to the next. An upgrade that is no longer needed is retired by deleting
//! its step and refusing the versions before it in [`upgrade`].
//!
//! Fields that a file leaves out because they hold their usual value (an object's layer 0,
//! not hidden, no name) are not upgrades: they are how every file is written, and the types
//! keep their defaults.

use serde_json::{Map, Value, json};

/// The version files are written in.
pub(crate) const CURRENT: u64 = 1;

/// Brings a file's JSON up to [`CURRENT`]. A file without a version is version 0: every file
/// written before 15 November 2026.
pub(crate) fn upgrade(file: &mut Value) -> Result<(), String> {
    let version = file.get("version").and_then(Value::as_u64).unwrap_or(0);
    if version > CURRENT {
        return Err(format!(
            "this file was saved by a newer Curva (format version {version}); update Curva to open it"
        ));
    }
    if version < 1 {
        to_version_1(file)?;
    }
    file["version"] = json!(CURRENT);
    Ok(())
}

/// Version 0 to 1: everything files before 15 November 2026 could lack.
///
/// - Named views and construction planes (files from before they were saved).
/// - Layers, the current layer and the next layer's id (files from before layers): everything
///   goes on the one standard layer.
/// - The unit (files from before units were kept): millimetres.
/// - Groups' and blocks' counters and the block definitions (files from before them).
/// - Circles' planes (files from before circles had planes): the ground.
/// - Bodies as the engine's portable record (files from 10 and 11 November 2026): written
///   again as the engine's native text.
fn to_version_1(file: &mut Value) -> Result<(), String> {
    let root = object(file, "the file")?;
    for key in ["views", "cplanes"] {
        root.entry(key).or_insert_with(|| json!([]));
    }
    let state = object(
        root.get_mut("state")
            .ok_or_else(|| "the file has no model".to_owned())?,
        "the model",
    )?;
    if !state.contains_key("layers") {
        let standard =
            serde_json::to_value(crate::State::default().layers).map_err(|e| e.to_string())?;
        state.insert("layers".to_owned(), standard);
    }
    let defaults = [
        ("current_layer", json!(0)),
        ("next_layer", json!(1)),
        ("unit", json!("Millimetres")),
        ("next_group", json!(0)),
        ("blocks", json!([])),
        ("next_block", json!(0)),
    ];
    for (key, value) in defaults {
        state.entry(key).or_insert(value);
    }
    walk(file)
}

/// Upgrades every circle and body anywhere in `value`.
fn walk(value: &mut Value) -> Result<(), String> {
    match value {
        Value::Object(map) => {
            if let Some(Value::Object(circle)) = map.get_mut("Circle") {
                circle.entry("x").or_insert_with(|| json!([1.0, 0.0, 0.0]));
                circle.entry("y").or_insert_with(|| json!([0.0, 1.0, 0.0]));
            }
            if let Some(Value::Object(body)) = map.get_mut("body") {
                native_body(body)?;
            }
            map.values_mut().try_for_each(walk)
        }
        Value::Array(list) => list.iter_mut().try_for_each(walk),
        _ => Ok(()),
    }
}

/// A body held as the portable record, written again as the native text.
fn native_body(body: &mut Map<String, Value>) -> Result<(), String> {
    if body.contains_key("native") {
        return Ok(());
    }
    let Some(record) = body.remove("record") else {
        return Err("a body without faces".to_owned());
    };
    let record: curva_kernel::record::Brep =
        serde_json::from_value(record).map_err(|e| format!("reading an old body: {e}"))?;
    let made = curva_kernel::Body::from_record(&record).map_err(|e| e.0)?;
    let Value::Object(written) = serde_json::to_value(&made).map_err(|e| e.to_string())? else {
        return Err("writing an old body again failed".to_owned());
    };
    // The placement the file held stays as it was.
    if let Some(native) = written.get("native") {
        body.insert("native".to_owned(), native.clone());
    }
    Ok(())
}

fn object<'a>(value: &'a mut Value, what: &str) -> Result<&'a mut Map<String, Value>, String> {
    value
        .as_object_mut()
        .ok_or_else(|| format!("{what} is not an object"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use curva_kernel::{Body, Curve};

    /// A file's JSON with this model, as files before versions were written.
    fn old_file(state: Value) -> Value {
        json!({ "format": "curva-first-slice", "state": state })
    }

    fn read(mut file: Value) -> crate::State {
        upgrade(&mut file).unwrap();
        assert_eq!(file["version"], json!(CURRENT));
        serde_json::from_value(file["state"].clone()).unwrap()
    }

    #[test]
    fn a_file_from_before_layers_opens_on_the_default_layer() {
        let state = read(old_file(json!({
            "objects": [{"id": 3, "curve": {"Line": {"start": [0.0, 0.0, 0.0], "end": [1.0, 0.0, 0.0]}}}],
            "next_id": 4
        })));
        assert_eq!(state.layers.len(), 1);
        assert_eq!(state.objects[0].layer, state.current_layer);
        assert_eq!(state.unit, crate::Unit::Millimetres);
    }

    #[test]
    fn a_circle_stored_without_a_plane_lies_flat() {
        let state = read(old_file(json!({
            "objects": [{"id": 0, "curve": {"Circle": {"centre": [1.0, 2.0, 0.0], "radius": 3.0}}}],
            "next_id": 1
        })));
        assert_eq!(
            state.objects[0].curve(),
            Some(&Curve::circle([1.0, 2.0, 0.0], 3.0).unwrap())
        );
    }

    #[test]
    fn a_body_saved_as_a_portable_record_still_opens() {
        let body = Body::cuboid([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]).unwrap();
        let placed = body.translated([10.0, 0.0, 0.0]);
        let stored = serde_json::to_value(&placed).unwrap();
        let mut old = json!({ "record": body.portable_record().unwrap() });
        if let Some(placement) = stored.get("placement") {
            old["placement"] = placement.clone();
        }
        let state = read(old_file(json!({
            "objects": [{"id": 0, "body": old}],
            "next_id": 1
        })));
        let back = state.objects[0].shape.body().unwrap();
        assert!((back.volume().unwrap() - 120.0).abs() < 1e-6);
        assert_eq!(back.bounds().unwrap(), placed.bounds().unwrap());
    }

    #[test]
    fn a_file_from_a_newer_curva_says_so() {
        let mut file = old_file(json!({"objects": [], "next_id": 0}));
        file["version"] = json!(CURRENT + 1);
        assert!(upgrade(&mut file).unwrap_err().contains("newer Curva"));
    }

    #[test]
    fn a_current_file_is_left_as_it_is() {
        let doc = crate::Document::default();
        let mut file = json!({
            "format": "curva-first-slice",
            "version": CURRENT,
            "state": doc.state(),
            "views": [],
            "cplanes": []
        });
        let before = file.clone();
        upgrade(&mut file).unwrap();
        assert_eq!(file, before);
    }
}
