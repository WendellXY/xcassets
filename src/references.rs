use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::{
    diagnostics::{Diagnostic, DiagnosticCode, Severity},
    error::ParseError,
    model::{AssetReference, AssetReferenceIndex, AssetReferenceKind},
};

pub fn index_asset_references(path: impl AsRef<Path>) -> Result<AssetReferenceIndex, ParseError> {
    let source_path = path.as_ref().to_path_buf();

    if !source_path.exists() {
        return Err(ParseError::MissingRoot { path: source_path });
    }
    if !source_path.is_dir() {
        return Err(ParseError::RootNotDirectory { path: source_path });
    }
    if source_path.extension().and_then(|ext| ext.to_str()) != Some("xcassets") {
        return Err(ParseError::InvalidCatalogRoot { path: source_path });
    }

    let mut indexer = ReferenceIndexer::new();
    indexer.walk_directory(&source_path, Path::new(""), &mut Vec::new());

    Ok(AssetReferenceIndex {
        catalog_name: source_path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("catalog")
            .to_string(),
        source_path,
        references: indexer.references,
        diagnostics: indexer.diagnostics,
    })
}

#[derive(Debug)]
struct ReferenceIndexer {
    references: Vec<AssetReference>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReferenceFolderKind {
    Group,
    ImageSet,
    ColorSet,
    AppIconSet,
    SpriteAtlas,
    Other,
}

#[derive(Debug, Default, Deserialize)]
struct NamespaceFolderContents {
    #[serde(default)]
    properties: NamespaceFolderProperties,
}

#[derive(Debug, Default, Deserialize)]
struct NamespaceFolderProperties {
    #[serde(rename = "provides-namespace")]
    provides_namespace: Option<bool>,
}

impl ReferenceIndexer {
    fn new() -> Self {
        Self {
            references: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn walk_directory(
        &mut self,
        absolute_path: &Path,
        relative_path: &Path,
        namespace: &mut Vec<String>,
    ) {
        let (folder_name, kind) = classify_reference_folder(relative_path);

        match kind {
            ReferenceFolderKind::ImageSet => {
                self.references.push(AssetReference {
                    kind: AssetReferenceKind::Image,
                    lookup_name: join_lookup_name(namespace, &folder_name),
                    relative_path: relative_path.to_path_buf(),
                });
                return;
            }
            ReferenceFolderKind::ColorSet => {
                self.references.push(AssetReference {
                    kind: AssetReferenceKind::Color,
                    lookup_name: join_lookup_name(namespace, &folder_name),
                    relative_path: relative_path.to_path_buf(),
                });
                return;
            }
            ReferenceFolderKind::AppIconSet => {
                self.references.push(AssetReference {
                    kind: AssetReferenceKind::AppIcon,
                    lookup_name: join_lookup_name(namespace, &folder_name),
                    relative_path: relative_path.to_path_buf(),
                });
                return;
            }
            ReferenceFolderKind::Group
            | ReferenceFolderKind::SpriteAtlas
            | ReferenceFolderKind::Other => {}
        }

        let scan = match self.scan_directory(absolute_path, relative_path) {
            Ok(scan) => scan,
            Err(error) => {
                self.diagnostics.push(Diagnostic::new(
                    DiagnosticCode::UnreadableDirectory,
                    Severity::Error,
                    relative_path.to_path_buf(),
                    format!("failed to read directory: {error}"),
                ));
                return;
            }
        };

        let add_namespace = matches!(
            kind,
            ReferenceFolderKind::Group | ReferenceFolderKind::SpriteAtlas
        ) && !relative_path.as_os_str().is_empty()
            && self.directory_provides_namespace(relative_path, scan.contents_path.as_deref());

        if add_namespace {
            namespace.push(folder_name);
        }

        for child in scan.child_directories {
            self.walk_directory(&child.absolute_path, &child.relative_path, namespace);
        }

        if add_namespace {
            namespace.pop();
        }
    }

    fn scan_directory(
        &self,
        absolute_path: &Path,
        relative_path: &Path,
    ) -> Result<DirectoryScan, std::io::Error> {
        let mut child_directories = Vec::new();
        let mut contents_path = None;

        for entry in fs::read_dir(absolute_path)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy().to_string();

            if file_type.is_dir() {
                child_directories.push(ChildDirectory {
                    absolute_path: entry.path(),
                    relative_path: join_relative(relative_path, &file_name),
                });
            } else if file_type.is_file() && file_name == "Contents.json" {
                contents_path = Some(entry.path());
            }
        }

        Ok(DirectoryScan {
            child_directories,
            contents_path,
        })
    }

    fn directory_provides_namespace(
        &mut self,
        relative_path: &Path,
        contents_path: Option<&Path>,
    ) -> bool {
        let Some(contents_path) = contents_path else {
            return false;
        };

        let raw_text = match fs::read_to_string(contents_path) {
            Ok(raw_text) => raw_text,
            Err(error) => {
                self.diagnostics.push(Diagnostic::new(
                    DiagnosticCode::UnreadableFile,
                    Severity::Error,
                    relative_path.to_path_buf(),
                    format!("failed to read Contents.json: {error}"),
                ));
                return false;
            }
        };

        match serde_json::from_str::<NamespaceFolderContents>(&raw_text) {
            Ok(contents) => contents.properties.provides_namespace.unwrap_or(false),
            Err(error) => {
                let code = if error.is_data() {
                    DiagnosticCode::InvalidContentsSchema
                } else {
                    DiagnosticCode::InvalidContentsJson
                };
                let prefix = if error.is_data() {
                    "unsupported or malformed Contents.json schema"
                } else {
                    "invalid Contents.json"
                };
                self.diagnostics.push(Diagnostic::new(
                    code,
                    Severity::Error,
                    relative_path.to_path_buf(),
                    format!("{prefix}: {error}"),
                ));
                false
            }
        }
    }
}

#[derive(Debug)]
struct DirectoryScan {
    child_directories: Vec<ChildDirectory>,
    contents_path: Option<PathBuf>,
}

#[derive(Debug)]
struct ChildDirectory {
    absolute_path: PathBuf,
    relative_path: PathBuf,
}

fn classify_reference_folder(relative_path: &Path) -> (String, ReferenceFolderKind) {
    let folder_name = relative_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();

    let extension = Path::new(folder_name)
        .extension()
        .and_then(|extension| extension.to_str());

    let stem = Path::new(folder_name)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or(folder_name)
        .to_string();

    let kind = match extension {
        None => ReferenceFolderKind::Group,
        Some("imageset") => ReferenceFolderKind::ImageSet,
        Some("colorset") => ReferenceFolderKind::ColorSet,
        Some("appiconset") => ReferenceFolderKind::AppIconSet,
        Some("spriteatlas") => ReferenceFolderKind::SpriteAtlas,
        Some(_) => ReferenceFolderKind::Other,
    };

    (stem, kind)
}

fn join_lookup_name(namespace: &[String], name: &str) -> String {
    if namespace.is_empty() {
        name.to_string()
    } else {
        let mut full_name = namespace.join("/");
        full_name.push('/');
        full_name.push_str(name);
        full_name
    }
}

fn join_relative(base: &Path, child: &str) -> PathBuf {
    if base.as_os_str().is_empty() {
        PathBuf::from(child)
    } else {
        base.join(child)
    }
}
