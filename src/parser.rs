use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::{
    diagnostics::{Diagnostic, DiagnosticCode, Severity},
    error::ParseError,
    model::{
        AppIconSetContents, AssetCatalog, ColorSetContents, FolderContents, FolderNode, ImageEntry,
        ImageSetContents, Node, OpaqueNode, ParseReport, RawContents,
    },
};

pub fn parse_catalog(path: impl AsRef<Path>) -> Result<ParseReport, ParseError> {
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

    let mut parser = Parser::new(source_path.clone());
    let catalog = parser.parse_root()?;

    Ok(ParseReport {
        catalog,
        diagnostics: parser.diagnostics,
    })
}

#[derive(Debug)]
struct Parser {
    root_path: PathBuf,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Debug)]
struct DirectorySnapshot {
    files: Vec<PathBuf>,
    child_directories: Vec<ChildDirectory>,
    contents_path: Option<PathBuf>,
}

#[derive(Debug)]
struct ChildDirectory {
    absolute_path: PathBuf,
    relative_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FolderKind {
    Group,
    ImageSet,
    ColorSet,
    AppIconSet,
    Opaque,
}

#[derive(Debug)]
enum LoadedContents {
    Json(Value),
    InvalidJson(String),
}

impl Parser {
    fn new(root_path: PathBuf) -> Self {
        Self {
            root_path,
            diagnostics: Vec::new(),
        }
    }

    fn parse_root(&mut self) -> Result<AssetCatalog, ParseError> {
        let snapshot = self.read_root_directory()?;
        let (contents, raw_contents) = self.parse_optional_contents::<FolderContents>(
            snapshot.contents_path.as_deref(),
            Path::new(""),
        );
        let children = self.parse_children(snapshot.child_directories);

        Ok(AssetCatalog {
            name: self
                .root_path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("catalog")
                .to_string(),
            source_path: self.root_path.clone(),
            files: snapshot.files,
            raw_contents,
            contents,
            children,
        })
    }

    fn parse_children(&mut self, children: Vec<ChildDirectory>) -> Vec<Node> {
        let mut nodes = Vec::with_capacity(children.len());
        for child in children {
            nodes.push(self.parse_node(child.absolute_path, child.relative_path));
        }
        nodes
    }

    fn parse_node(&mut self, absolute_path: PathBuf, relative_path: PathBuf) -> Node {
        let (name, kind, folder_type) = classify_folder(&relative_path);
        let snapshot = match self.read_directory(&absolute_path, &relative_path) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.push_diagnostic(
                    DiagnosticCode::UnreadableDirectory,
                    Severity::Error,
                    relative_path.clone(),
                    format!("failed to read directory: {error}"),
                );
                return self.empty_node(name, relative_path, kind, folder_type);
            }
        };

        let children = self.parse_children(snapshot.child_directories);

        match kind {
            FolderKind::Group => {
                let (contents, raw_contents) = self.parse_optional_contents::<FolderContents>(
                    snapshot.contents_path.as_deref(),
                    &relative_path,
                );
                Node::Group(FolderNode {
                    name,
                    relative_path,
                    files: snapshot.files,
                    raw_contents,
                    contents,
                    children,
                })
            }
            FolderKind::ImageSet => {
                let (contents, raw_contents) = self.parse_required_contents::<ImageSetContents>(
                    snapshot.contents_path.as_deref(),
                    &relative_path,
                );
                if let Some(contents) = contents.as_ref() {
                    self.validate_image_files(&relative_path, &snapshot.files, &contents.images);
                }
                Node::ImageSet(FolderNode {
                    name,
                    relative_path,
                    files: snapshot.files,
                    raw_contents,
                    contents,
                    children,
                })
            }
            FolderKind::ColorSet => {
                let (contents, raw_contents) = self.parse_required_contents::<ColorSetContents>(
                    snapshot.contents_path.as_deref(),
                    &relative_path,
                );
                Node::ColorSet(FolderNode {
                    name,
                    relative_path,
                    files: snapshot.files,
                    raw_contents,
                    contents,
                    children,
                })
            }
            FolderKind::AppIconSet => {
                let (contents, raw_contents) = self.parse_required_contents::<AppIconSetContents>(
                    snapshot.contents_path.as_deref(),
                    &relative_path,
                );
                if let Some(contents) = contents.as_ref() {
                    self.validate_image_files(&relative_path, &snapshot.files, &contents.images);
                }
                Node::AppIconSet(FolderNode {
                    name,
                    relative_path,
                    files: snapshot.files,
                    raw_contents,
                    contents,
                    children,
                })
            }
            FolderKind::Opaque => {
                self.push_diagnostic(
                    DiagnosticCode::UnsupportedFolderType,
                    Severity::Warning,
                    relative_path.clone(),
                    format!(
                        "unsupported folder type: {}",
                        folder_type.as_deref().unwrap_or("unknown")
                    ),
                );
                let raw_contents =
                    self.load_raw_contents(snapshot.contents_path.as_deref(), &relative_path);
                Node::Opaque(OpaqueNode {
                    name,
                    relative_path,
                    folder_type: folder_type.unwrap_or_else(|| "opaque".to_string()),
                    files: snapshot.files,
                    raw_contents,
                    children,
                })
            }
        }
    }

    fn empty_node(
        &self,
        name: String,
        relative_path: PathBuf,
        kind: FolderKind,
        folder_type: Option<String>,
    ) -> Node {
        match kind {
            FolderKind::Group => Node::Group(FolderNode {
                name,
                relative_path,
                files: Vec::new(),
                raw_contents: None,
                contents: None,
                children: Vec::new(),
            }),
            FolderKind::ImageSet => Node::ImageSet(FolderNode {
                name,
                relative_path,
                files: Vec::new(),
                raw_contents: None,
                contents: None,
                children: Vec::new(),
            }),
            FolderKind::ColorSet => Node::ColorSet(FolderNode {
                name,
                relative_path,
                files: Vec::new(),
                raw_contents: None,
                contents: None,
                children: Vec::new(),
            }),
            FolderKind::AppIconSet => Node::AppIconSet(FolderNode {
                name,
                relative_path,
                files: Vec::new(),
                raw_contents: None,
                contents: None,
                children: Vec::new(),
            }),
            FolderKind::Opaque => Node::Opaque(OpaqueNode {
                name,
                relative_path,
                folder_type: folder_type.unwrap_or_else(|| "opaque".to_string()),
                files: Vec::new(),
                raw_contents: None,
                children: Vec::new(),
            }),
        }
    }

    fn read_root_directory(&self) -> Result<DirectorySnapshot, ParseError> {
        self.read_directory(&self.root_path, Path::new(""))
            .map_err(|source| ParseError::ReadRoot {
                path: self.root_path.clone(),
                source,
            })
    }

    fn read_directory(
        &self,
        absolute_path: &Path,
        relative_path: &Path,
    ) -> Result<DirectorySnapshot, std::io::Error> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(absolute_path)? {
            entries.push(entry?);
        }
        entries.sort_by_key(|entry| entry.file_name());

        let mut files = Vec::new();
        let mut child_directories = Vec::new();
        let mut contents_path = None;

        for entry in entries {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy().to_string();
            let file_type = entry.file_type()?;
            let absolute_entry = entry.path();

            if file_type.is_dir() {
                child_directories.push(ChildDirectory {
                    absolute_path: absolute_entry,
                    relative_path: join_relative(relative_path, &file_name),
                });
            } else if file_type.is_file() {
                if file_name == "Contents.json" {
                    contents_path = Some(absolute_entry);
                } else {
                    files.push(join_relative(relative_path, &file_name));
                }
            }
        }

        Ok(DirectorySnapshot {
            files,
            child_directories,
            contents_path,
        })
    }

    fn parse_optional_contents<T>(
        &mut self,
        contents_path: Option<&Path>,
        relative_path: &Path,
    ) -> (Option<T>, Option<RawContents>)
    where
        T: DeserializeOwned,
    {
        match contents_path.and_then(|path| self.load_contents(path, relative_path)) {
            Some(loaded) => self.parse_loaded_contents(loaded, relative_path),
            None => (None, None),
        }
    }

    fn parse_required_contents<T>(
        &mut self,
        contents_path: Option<&Path>,
        relative_path: &Path,
    ) -> (Option<T>, Option<RawContents>)
    where
        T: DeserializeOwned,
    {
        match contents_path.and_then(|path| self.load_contents(path, relative_path)) {
            Some(loaded) => self.parse_loaded_contents(loaded, relative_path),
            None => {
                self.push_diagnostic(
                    DiagnosticCode::MissingContentsJson,
                    Severity::Error,
                    relative_path.to_path_buf(),
                    "missing required Contents.json",
                );
                (None, None)
            }
        }
    }

    fn parse_loaded_contents<T>(
        &mut self,
        loaded: LoadedContents,
        relative_path: &Path,
    ) -> (Option<T>, Option<RawContents>)
    where
        T: DeserializeOwned,
    {
        match loaded {
            LoadedContents::Json(value) => match serde_json::from_value::<T>(value.clone()) {
                Ok(contents) => (Some(contents), None),
                Err(error) => {
                    self.push_diagnostic(
                        DiagnosticCode::InvalidContentsSchema,
                        Severity::Error,
                        relative_path.to_path_buf(),
                        format!("unsupported or malformed Contents.json schema: {error}"),
                    );
                    (None, Some(RawContents::Json(value)))
                }
            },
            LoadedContents::InvalidJson(text) => (None, Some(RawContents::InvalidJson(text))),
        }
    }

    fn load_raw_contents(
        &mut self,
        contents_path: Option<&Path>,
        relative_path: &Path,
    ) -> Option<RawContents> {
        let loaded = contents_path.and_then(|path| self.load_contents(path, relative_path))?;
        Some(match loaded {
            LoadedContents::Json(value) => RawContents::Json(value),
            LoadedContents::InvalidJson(text) => RawContents::InvalidJson(text),
        })
    }

    fn load_contents(
        &mut self,
        contents_path: &Path,
        relative_path: &Path,
    ) -> Option<LoadedContents> {
        let raw_text = match fs::read_to_string(contents_path) {
            Ok(raw_text) => raw_text,
            Err(error) => {
                self.push_diagnostic(
                    DiagnosticCode::UnreadableFile,
                    Severity::Error,
                    relative_path.to_path_buf(),
                    format!("failed to read Contents.json: {error}"),
                );
                return None;
            }
        };

        match serde_json::from_str::<Value>(&raw_text) {
            Ok(value) => Some(LoadedContents::Json(value)),
            Err(error) => {
                self.push_diagnostic(
                    DiagnosticCode::InvalidContentsJson,
                    Severity::Error,
                    relative_path.to_path_buf(),
                    format!("invalid Contents.json: {error}"),
                );
                Some(LoadedContents::InvalidJson(raw_text))
            }
        }
    }

    fn validate_image_files(
        &mut self,
        relative_path: &Path,
        files: &[PathBuf],
        images: &[ImageEntry],
    ) {
        for image in images {
            let Some(filename) = image.filename.as_deref() else {
                continue;
            };

            let file_path = join_relative(relative_path, filename);
            if !files.iter().any(|candidate| candidate == &file_path) {
                self.push_diagnostic(
                    DiagnosticCode::MissingReferencedFile,
                    Severity::Error,
                    file_path,
                    format!("referenced file does not exist: {filename}"),
                );
            }
        }
    }

    fn push_diagnostic(
        &mut self,
        code: DiagnosticCode,
        severity: Severity,
        path: PathBuf,
        message: impl Into<String>,
    ) {
        self.diagnostics
            .push(Diagnostic::new(code, severity, path, message));
    }
}

fn classify_folder(relative_path: &Path) -> (String, FolderKind, Option<String>) {
    let folder_name = relative_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();

    let extension = Path::new(folder_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_string);

    let stem = Path::new(folder_name)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or(folder_name)
        .to_string();

    match extension.as_deref() {
        None => (folder_name.to_string(), FolderKind::Group, None),
        Some("imageset") => (stem, FolderKind::ImageSet, extension),
        Some("colorset") => (stem, FolderKind::ColorSet, extension),
        Some("appiconset") => (stem, FolderKind::AppIconSet, extension),
        Some(_) => (stem, FolderKind::Opaque, extension),
    }
}

fn join_relative(base: &Path, child: &str) -> PathBuf {
    if base.as_os_str().is_empty() {
        PathBuf::from(child)
    } else {
        base.join(child)
    }
}
