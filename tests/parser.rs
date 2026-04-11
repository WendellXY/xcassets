use std::{
    fs,
    path::{Path, PathBuf},
};

use tempfile::tempdir;
#[cfg(feature = "parallel")]
use xcassets::parse_catalog_parallel;
use xcassets::{DiagnosticCode, Node, ParseError, RawContents, Severity, parse_catalog};

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
fn parses_minimal_catalog_without_root_contents_json() {
    let temp = tempdir().unwrap();
    let catalog = create_catalog(temp.path(), "Assets");
    let image_dir = catalog.join("icon.imageset");

    write_file(
        &image_dir.join("Contents.json"),
        r#"{
  "images" : [
    {
      "filename" : "icon.png",
      "idiom" : "universal",
      "scale" : "3x"
    }
  ],
  "info" : {
    "author" : "xcode",
    "version" : 1
  }
}"#,
    );
    write_file(&image_dir.join("icon.png"), "png");

    let report = parse_catalog(&catalog).unwrap();
    assert!(report.diagnostics.is_empty());
    assert_eq!(report.catalog.name, "Assets");
    assert_eq!(report.catalog.children.len(), 1);

    match &report.catalog.children[0] {
        Node::ImageSet(node) => {
            let contents = node.contents.as_ref().unwrap();
            assert_eq!(contents.images.len(), 1);
            assert_eq!(contents.images[0].filename.as_deref(), Some("icon.png"));
        }
        other => panic!("expected image set, got {other:?}"),
    }
}

#[test]
fn parses_namespaced_groups() {
    let temp = tempdir().unwrap();
    let catalog = create_catalog(temp.path(), "Assets");
    let group = catalog.join("Navigator");
    let image_dir = group.join("icon_back.imageset");

    write_file(
        &group.join("Contents.json"),
        r#"{
  "info" : {
    "author" : "xcode",
    "version" : 1
  },
  "properties" : {
    "provides-namespace" : true
  }
}"#,
    );
    write_file(
        &image_dir.join("Contents.json"),
        r#"{
  "images" : [
    {
      "filename" : "icon_back.png",
      "idiom" : "universal"
    }
  ],
  "info" : {
    "author" : "xcode",
    "version" : 1
  }
}"#,
    );
    write_file(&image_dir.join("icon_back.png"), "png");

    let report = parse_catalog(&catalog).unwrap();
    assert!(report.diagnostics.is_empty());

    match &report.catalog.children[0] {
        Node::Group(node) => {
            assert_eq!(
                node.contents
                    .as_ref()
                    .and_then(|contents| contents.properties.provides_namespace),
                Some(true)
            );
            assert_eq!(node.children.len(), 1);
        }
        other => panic!("expected group, got {other:?}"),
    }
}

#[test]
fn parses_image_sets_with_language_direction_and_placeholder_slots() {
    let temp = tempdir().unwrap();
    let catalog = create_catalog(temp.path(), "Assets");
    let image_dir = catalog.join("double_reward_banner.imageset");

    write_file(
        &image_dir.join("Contents.json"),
        r#"{
  "images" : [
    {
      "idiom" : "universal",
      "language-direction" : "left-to-right",
      "scale" : "1x"
    },
    {
      "idiom" : "universal",
      "language-direction" : "right-to-left",
      "scale" : "1x"
    },
    {
      "filename" : "task_banner_1.png",
      "idiom" : "universal",
      "language-direction" : "left-to-right",
      "scale" : "3x"
    },
    {
      "filename" : "task_banner_3.png",
      "idiom" : "universal",
      "language-direction" : "right-to-left",
      "scale" : "3x"
    }
  ],
  "info" : {
    "author" : "xcode",
    "version" : 1
  },
  "properties" : {
    "localizable" : true
  }
}"#,
    );
    write_file(&image_dir.join("task_banner_1.png"), "png");
    write_file(&image_dir.join("task_banner_3.png"), "png");

    let report = parse_catalog(&catalog).unwrap();
    assert!(report.diagnostics.is_empty());

    match &report.catalog.children[0] {
        Node::ImageSet(node) => {
            let contents = node.contents.as_ref().unwrap();
            assert_eq!(contents.properties.localizable, Some(true));
            assert_eq!(contents.images.len(), 4);
            assert_eq!(contents.images[0].filename, None);
            assert_eq!(
                contents.images[1].language_direction.as_deref(),
                Some("right-to-left")
            );
        }
        other => panic!("expected image set, got {other:?}"),
    }
}

#[test]
fn parses_template_rendering_intent() {
    let temp = tempdir().unwrap();
    let catalog = create_catalog(temp.path(), "Assets");
    let image_dir = catalog.join("icon_kick_out.imageset");

    write_file(
        &image_dir.join("Contents.json"),
        r#"{
  "images" : [
    {
      "filename" : "icon_kick_out.pdf",
      "idiom" : "universal"
    }
  ],
  "info" : {
    "author" : "xcode",
    "version" : 1
  },
  "properties" : {
    "template-rendering-intent" : "template"
  }
}"#,
    );
    write_file(&image_dir.join("icon_kick_out.pdf"), "pdf");

    let report = parse_catalog(&catalog).unwrap();
    assert!(report.diagnostics.is_empty());

    match &report.catalog.children[0] {
        Node::ImageSet(node) => {
            let contents = node.contents.as_ref().unwrap();
            assert_eq!(
                contents.properties.template_rendering_intent.as_deref(),
                Some("template")
            );
        }
        other => panic!("expected image set, got {other:?}"),
    }
}

#[test]
fn parses_color_sets_with_appearances() {
    let temp = tempdir().unwrap();
    let catalog = create_catalog(temp.path(), "Assets");
    let color_dir = catalog.join("medal_text_color.colorset");

    write_file(
        &color_dir.join("Contents.json"),
        r#"{
  "colors" : [
    {
      "color" : {
        "color-space" : "srgb",
        "components" : {
          "alpha" : "1.000",
          "blue" : "0xA2",
          "green" : "0xD6",
          "red" : "0xFF"
        }
      },
      "idiom" : "universal"
    },
    {
      "appearances" : [
        {
          "appearance" : "luminosity",
          "value" : "dark"
        }
      ],
      "color" : {
        "color-space" : "srgb",
        "components" : {
          "alpha" : "1.000",
          "blue" : "1.000",
          "green" : "1.000",
          "red" : "1.000"
        }
      },
      "idiom" : "universal"
    }
  ],
  "info" : {
    "author" : "xcode",
    "version" : 1
  }
}"#,
    );

    let report = parse_catalog(&catalog).unwrap();
    assert!(report.diagnostics.is_empty());

    match &report.catalog.children[0] {
        Node::ColorSet(node) => {
            let contents = node.contents.as_ref().unwrap();
            assert_eq!(contents.colors.len(), 2);
            assert_eq!(
                contents.colors[1].appearances[0].appearance.as_deref(),
                Some("luminosity")
            );
        }
        other => panic!("expected color set, got {other:?}"),
    }
}

#[test]
fn parses_app_icon_sets() {
    let temp = tempdir().unwrap();
    let catalog = create_catalog(temp.path(), "Assets");
    let icon_dir = catalog.join("AppIcon.appiconset");

    write_file(
        &icon_dir.join("Contents.json"),
        r#"{
  "images" : [
    {
      "filename" : "AppIcon.png",
      "idiom" : "universal",
      "platform" : "ios",
      "size" : "1024x1024"
    }
  ],
  "info" : {
    "author" : "xcode",
    "version" : 1
  }
}"#,
    );
    write_file(&icon_dir.join("AppIcon.png"), "png");

    let invalid_root = parse_catalog(&icon_dir).err();
    assert!(matches!(
        invalid_root,
        Some(ParseError::InvalidCatalogRoot { .. })
    ));

    let report = parse_catalog(&catalog).unwrap();
    assert!(report.diagnostics.is_empty());

    match &report.catalog.children[0] {
        Node::AppIconSet(node) => {
            let contents = node.contents.as_ref().unwrap();
            assert_eq!(contents.images.len(), 1);
            assert_eq!(contents.images[0].platform.as_deref(), Some("ios"));
            assert_eq!(contents.images[0].size.as_deref(), Some("1024x1024"));
        }
        other => panic!("expected app icon set, got {other:?}"),
    }
}

#[test]
fn emits_diagnostic_for_missing_contents_json() {
    let temp = tempdir().unwrap();
    let catalog = create_catalog(temp.path(), "Assets");
    fs::create_dir_all(catalog.join("missing.imageset")).unwrap();

    let report = parse_catalog(&catalog).unwrap();
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(
        report.diagnostics[0].code,
        DiagnosticCode::MissingContentsJson
    );
    assert_eq!(report.diagnostics[0].severity, Severity::Error);

    match &report.catalog.children[0] {
        Node::ImageSet(node) => assert!(node.contents.is_none()),
        other => panic!("expected image set, got {other:?}"),
    }
}

#[test]
fn emits_diagnostic_for_missing_referenced_files() {
    let temp = tempdir().unwrap();
    let catalog = create_catalog(temp.path(), "Assets");
    let image_dir = catalog.join("missing_file.imageset");

    write_file(
        &image_dir.join("Contents.json"),
        r#"{
  "images" : [
    {
      "filename" : "missing.png",
      "idiom" : "universal",
      "scale" : "3x"
    }
  ],
  "info" : {
    "author" : "xcode",
    "version" : 1
  }
}"#,
    );

    let report = parse_catalog(&catalog).unwrap();
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(
        report.diagnostics[0].code,
        DiagnosticCode::MissingReferencedFile
    );
}

#[test]
fn preserves_unsupported_folder_types_as_opaque_nodes() {
    let temp = tempdir().unwrap();
    let catalog = create_catalog(temp.path(), "Assets");
    let sprite_dir = catalog.join("Atlas.spriteatlas");
    let image_dir = sprite_dir.join("coin.imageset");

    write_file(
        &sprite_dir.join("Contents.json"),
        r#"{
  "info" : {
    "author" : "xcode",
    "version" : 1
  }
}"#,
    );
    write_file(
        &image_dir.join("Contents.json"),
        r#"{
  "images" : [
    {
      "filename" : "coin.png",
      "idiom" : "universal"
    }
  ],
  "info" : {
    "author" : "xcode",
    "version" : 1
  }
}"#,
    );
    write_file(&image_dir.join("coin.png"), "png");

    let report = parse_catalog(&catalog).unwrap();
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(
        report.diagnostics[0].code,
        DiagnosticCode::UnsupportedFolderType
    );
    assert_eq!(report.diagnostics[0].severity, Severity::Warning);

    match &report.catalog.children[0] {
        Node::Opaque(node) => {
            assert_eq!(node.folder_type, "spriteatlas");
            assert!(matches!(node.raw_contents, Some(RawContents::Json(_))));
            assert_eq!(node.children.len(), 1);
        }
        other => panic!("expected opaque node, got {other:?}"),
    }
}

#[test]
fn accepts_non_ascii_and_space_containing_filenames() {
    let temp = tempdir().unwrap();
    let catalog = create_catalog(temp.path(), "Assets");
    let image_dir = catalog.join("medal.imageset");

    write_file(
        &image_dir.join("Contents.json"),
        r#"{
  "images" : [
    {
      "filename" : "多个 勋章.png",
      "idiom" : "universal"
    }
  ],
  "info" : {
    "author" : "xcode",
    "version" : 1
  }
}"#,
    );
    write_file(&image_dir.join("多个 勋章.png"), "png");

    let report = parse_catalog(&catalog).unwrap();
    assert!(report.diagnostics.is_empty());
}

#[test]
fn emits_diagnostic_for_invalid_json() {
    let temp = tempdir().unwrap();
    let catalog = create_catalog(temp.path(), "Assets");
    let image_dir = catalog.join("bad.imageset");

    write_file(&image_dir.join("Contents.json"), "{ nope");

    let report = parse_catalog(&catalog).unwrap();
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(
        report.diagnostics[0].code,
        DiagnosticCode::InvalidContentsJson
    );

    match &report.catalog.children[0] {
        Node::ImageSet(node) => {
            assert!(matches!(
                node.raw_contents,
                Some(RawContents::InvalidJson(_))
            ));
        }
        other => panic!("expected image set, got {other:?}"),
    }
}

#[test]
#[ignore]
fn parses_real_project_catalogs_when_requested() {
    let project_root = match std::env::var("XCASSETS_REAL_PROJECT") {
        Ok(path) => PathBuf::from(path),
        Err(_) => return,
    };
    let limit = std::env::var("XCASSETS_REAL_PROJECT_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(10);

    let mut catalogs = Vec::new();
    collect_catalogs(&project_root, &mut catalogs);
    catalogs.sort();
    assert!(!catalogs.is_empty(), "no .xcassets catalogs found");

    let mut fatal_failures = Vec::new();
    for catalog in catalogs.into_iter().take(limit) {
        if let Err(error) = parse_catalog(&catalog) {
            fatal_failures.push((catalog, error));
        }
    }

    assert!(
        fatal_failures.is_empty(),
        "fatal catalog parse failures: {fatal_failures:#?}"
    );
}

fn collect_catalogs(dir: &Path, catalogs: &mut Vec<PathBuf>) {
    let read_dir = match fs::read_dir(dir) {
        Ok(read_dir) => read_dir,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) == Some("xcassets") {
            catalogs.push(path);
            continue;
        }
        collect_catalogs(&path, catalogs);
    }
}

#[cfg(feature = "parallel")]
#[test]
fn parallel_parser_matches_sequential_report() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/production_like/Assets.xcassets");

    let sequential = parse_catalog(&fixture).unwrap();
    let parallel = parse_catalog_parallel(&fixture).unwrap();

    assert_eq!(parallel, sequential);
}
