use graphics2d_format::{Context, DeckBindings, LoadError, proto};
use playing_card_assets::{BUNDLED, DECK, Deck, MANIFEST};
use prost::Message;
use serde_json::{Value, json};
use std::collections::HashMap;

fn sources() -> HashMap<String, String> {
    BUNDLED
        .iter()
        .map(|(path, text)| (path.to_string(), text.to_string()))
        .collect()
}
fn load(manifest: &str, files: &HashMap<String, String>) -> Result<Context, LoadError> {
    Context::load(manifest, |path| {
        files
            .get(path)
            .cloned()
            .ok_or_else(|| LoadError(format!("missing {path}")))
    })
}
fn edit(files: &mut HashMap<String, String>, path: &str, update: impl FnOnce(&mut Value)) {
    let mut value: Value = serde_json::from_str(&files[path]).unwrap();
    update(&mut value);
    files.insert(path.into(), serde_json::to_string(&value).unwrap());
}

#[test]
fn resource_reordering_changes_handles_but_preserves_exported_identity_and_layout() {
    let original = Deck::bundled().unwrap();
    let mut manifest: proto::Library = serde_json::from_str(MANIFEST).unwrap();
    manifest.resources.reverse();
    let reordered = load(&serde_json::to_string(&manifest).unwrap(), &sources()).unwrap();
    let bindings = DeckBindings::load(DECK, &reordered).unwrap();
    assert_ne!(original.bindings.suits[0], bindings.suits[0]);
    for (first, second) in original.bindings.faces.into_iter().zip(bindings.faces) {
        assert_eq!(
            original.context.export_document(first),
            reordered.export_document(second)
        );
    }
}

#[test]
fn every_document_round_trips_through_generated_json_binary_and_runtime_types() {
    let deck = Deck::bundled().unwrap();
    let mut files = sources();
    for resource in serde_json::from_str::<proto::Library>(MANIFEST)
        .unwrap()
        .resources
    {
        if !matches!(resource.source, Some(proto::resource::Source::Document(_))) {
            continue;
        }
        let id = deck.context.document_id(&resource.uuid).unwrap();
        let stored: proto::Document = serde_json::from_str(&files[&resource.path]).unwrap();
        let exported = deck.context.export_document(id);
        assert_eq!(stored, exported);
        let binary = exported.encode_to_vec();
        assert_eq!(
            proto::Document::decode(binary.as_slice()).unwrap(),
            exported
        );
        files.insert(resource.path, serde_json::to_string(&exported).unwrap());
    }
    let reloaded = load(MANIFEST, &files).unwrap();
    for id in deck.bindings.faces {
        assert_eq!(deck.context.document(id), reloaded.document(id));
    }
}

#[test]
fn invalid_identity_references_geometry_styles_and_json_are_rejected() {
    for (field, value) in [
        ("assetUuid", json!("c13daa98-f97a-4cae-9932-540f2d16c041")),
        ("assetUuid", json!("not-a-uuid")),
        (
            "simple",
            json!({"position":{},"xDirection":{},"scale":1,"handedness":1}),
        ),
        (
            "simple",
            json!({"position":{},"xDirection":{"x":1},"scale":0,"handedness":1}),
        ),
        (
            "simple",
            json!({"position":{"x":"NaN"},"xDirection":{"x":1},"scale":1,"handedness":1}),
        ),
        (
            "simple",
            json!({"position":{},"xDirection":{"x":1},"scale":1,"handedness":0}),
        ),
        (
            "simple",
            json!({"position":{},"xDirection":{"x":1},"scale":3.4e38,"handedness":1}),
        ),
        ("color", json!({"r":256,"a":255})),
        ("unsupportedField", json!(true)),
    ] {
        let mut files = sources();
        edit(&mut files, "layouts/2-spades.json", |doc| {
            doc["instances"][0][field] = value
        });
        assert!(load(MANIFEST, &files).is_err(), "accepted invalid {field}");
    }
    for (field, value) in [
        ("outlineWidth", json!(-1)),
        ("outlineBlur", json!(-1)),
        ("outlineBlur", json!("NaN")),
        (
            "shadow",
            json!({"color":{"a":28},"offset":{"y":1},"blur":-1,"unit":"WIDTH_UNIT_LOGICAL_PIXELS"}),
        ),
        (
            "shadow",
            json!({"color":{"a":28},"offset":{"y":"Infinity"},"blur":1,"unit":"WIDTH_UNIT_LOGICAL_PIXELS"}),
        ),
        ("outlineWidth", json!("Infinity")),
        ("outlineUnit", json!("WIDTH_UNIT_UNSPECIFIED")),
        ("outlineUnit", json!(77)),
    ] {
        let mut files = sources();
        edit(&mut files, "styles/card.json", |style| style[field] = value);
        assert!(load(MANIFEST, &files).is_err());
    }
    let mut files = sources();
    let style: Value = serde_json::from_str(&files["styles/card.json"]).unwrap();
    edit(&mut files, "layouts/2-spades.json", |doc| {
        doc["instances"][0]["assetUuid"] = style["uuid"].clone()
    });
    assert!(load(MANIFEST, &files).is_err(), "accepted a style as SVG");
    let mut files = sources();
    let doc: Value = serde_json::from_str(&files["layouts/2-spades.json"]).unwrap();
    let asset = doc["instances"][0]["assetUuid"].clone();
    edit(&mut files, "layouts/2-spades.json", |doc| {
        doc["instances"][0]["styleUuid"] = asset
    });
    assert!(load(MANIFEST, &files).is_err(), "accepted SVG as a style");
    let mut files = sources();
    edit(&mut files, "styles/card.json", |style| {
        style["uuid"] = json!("c13daa98-f97a-4cae-9932-540f2d16c041")
    });
    assert!(load(MANIFEST, &files).is_err());
}

#[test]
fn duplicate_definitions_unsafe_paths_missing_files_and_incomplete_decks_are_rejected() {
    let original: proto::Library = serde_json::from_str(MANIFEST).unwrap();
    let mut manifest = original.clone();
    manifest.resources.push(manifest.resources[0].clone());
    assert!(load(&serde_json::to_string(&manifest).unwrap(), &sources()).is_err());
    for path in [
        "../CARD.svg",
        "/CARD.svg",
        "C:\\CARD.svg",
        "layouts//card.json",
    ] {
        let mut manifest = original.clone();
        manifest.resources[0].path = path.into();
        assert!(load(&serde_json::to_string(&manifest).unwrap(), &sources()).is_err());
    }
    let mut missing = sources();
    missing.remove("CARD.svg");
    assert!(load(MANIFEST, &missing).is_err());
    let context = load(MANIFEST, &sources()).unwrap();
    let mut deck: proto::Deck = serde_json::from_str(DECK).unwrap();
    deck.faces.pop();
    assert!(DeckBindings::load(&serde_json::to_string(&deck).unwrap(), &context).is_err());
    deck.faces.push(deck.faces[0].clone());
    assert!(DeckBindings::load(&serde_json::to_string(&deck).unwrap(), &context).is_err());
}

#[test]
fn mirrored_placement_and_source_unit_style_survive_loading_and_export() {
    let mut files = sources();
    edit(&mut files, "layouts/2-spades.json", |doc| {
        doc["instances"][1]["simple"]["handedness"] = json!(-1)
    });
    edit(&mut files, "styles/card.json", |style| {
        style["outlineUnit"] = json!("WIDTH_UNIT_SOURCE")
    });
    let context = load(MANIFEST, &files).unwrap();
    let source: proto::Document = serde_json::from_str(&files["layouts/2-spades.json"]).unwrap();
    let id = context.document_id(&source.uuid).unwrap();
    assert_eq!(source, context.export_document(id));
    assert_eq!(
        context.resources.styles[0].unit,
        graphics2d::WidthUnit::Source
    );
    assert!(
        context.document(id).instances[1]
            .placement
            .matrix()
            .determinant()
            < 0.0
    );
}
