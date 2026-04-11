use std::{
    fs,
    path::{Path, PathBuf},
};

use tempfile::tempdir;
use xcassets::{AssetReferenceKind, DiagnosticCode, index_asset_references};

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn create_catalog(temp_root: &Path, name: &str) -> PathBuf {
    let catalog = temp_root.join(format!("{name}.xcassets"));
    fs::create_dir_all(&catalog).unwrap();
    catalog
}

#[test]
fn indexes_asset_lookup_names_without_leaf_contents_json() {
    let temp = tempdir().unwrap();
    let catalog = create_catalog(temp.path(), "Assets");

    fs::create_dir_all(catalog.join("icon.imageset")).unwrap();
    fs::create_dir_all(catalog.join("theme.colorset")).unwrap();
    fs::create_dir_all(catalog.join("AppIcon.appiconset")).unwrap();

    let index = index_asset_references(&catalog).unwrap();

    assert!(index.diagnostics.is_empty());
    assert_eq!(index.catalog_name, "Assets");
    let mut references = index
        .references
        .iter()
        .map(|reference| (reference.lookup_name.clone(), reference.kind))
        .collect::<Vec<_>>();
    references.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(
        references,
        vec![
            ("AppIcon".to_string(), AssetReferenceKind::AppIcon),
            ("icon".to_string(), AssetReferenceKind::Image),
            ("theme".to_string(), AssetReferenceKind::Color),
        ]
    );
}

#[test]
fn indexes_namespaced_group_lookup_names() {
    let temp = tempdir().unwrap();
    let catalog = create_catalog(temp.path(), "Assets");
    let group = catalog.join("Navigator");

    write_file(
        &group.join("Contents.json"),
        r#"{
  "properties" : {
    "provides-namespace" : true
  }
}"#,
    );
    fs::create_dir_all(group.join("icon_back.imageset")).unwrap();

    let index = index_asset_references(&catalog).unwrap();

    assert!(index.diagnostics.is_empty());
    assert_eq!(index.references.len(), 1);
    assert_eq!(index.references[0].lookup_name, "Navigator/icon_back");
}

#[test]
fn indexes_namespaced_sprite_atlas_lookup_names() {
    let temp = tempdir().unwrap();
    let catalog = create_catalog(temp.path(), "Assets");
    let atlas = catalog.join("Game.spriteatlas");

    write_file(
        &atlas.join("Contents.json"),
        r#"{
  "properties" : {
    "provides-namespace" : true
  }
}"#,
    );
    fs::create_dir_all(atlas.join("coin.imageset")).unwrap();

    let index = index_asset_references(&catalog).unwrap();

    assert!(index.diagnostics.is_empty());
    assert_eq!(index.references.len(), 1);
    assert_eq!(index.references[0].lookup_name, "Game/coin");
}

#[test]
fn ignores_invalid_folder_types_without_namespace() {
    let temp = tempdir().unwrap();
    let catalog = create_catalog(temp.path(), "Assets");
    fs::create_dir_all(catalog.join("Future.assetpack").join("nested.imageset")).unwrap();

    let index = index_asset_references(&catalog).unwrap();

    assert!(index.diagnostics.is_empty());
    assert_eq!(index.references.len(), 1);
    assert_eq!(index.references[0].lookup_name, "nested");
}

#[test]
fn reports_invalid_namespace_contents_and_falls_back_to_plain_names() {
    let temp = tempdir().unwrap();
    let catalog = create_catalog(temp.path(), "Assets");
    let group = catalog.join("BrokenGroup");

    write_file(&group.join("Contents.json"), "{ nope");
    fs::create_dir_all(group.join("badge.imageset")).unwrap();

    let index = index_asset_references(&catalog).unwrap();

    assert_eq!(index.references.len(), 1);
    assert_eq!(index.references[0].lookup_name, "badge");
    assert_eq!(index.diagnostics.len(), 1);
    assert_eq!(
        index.diagnostics[0].code,
        DiagnosticCode::InvalidContentsJson
    );
}
