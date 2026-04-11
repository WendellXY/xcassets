use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(feature = "parallel")]
use rayon::prelude::*;
use serde::de::DeserializeOwned;
use serde_json::Value;
use serde_json::error::Category;

use crate::{
    diagnostics::{Diagnostic, DiagnosticCode, Severity},
    error::ParseError,
    model::{
        AppIconSetContents, AssetCatalog, ColorSetContents, FolderContents, FolderNode, ImageEntry,
        ImageSetContents, Node, OpaqueNode, ParseReport, RawContents,
    },
};

pub fn parse_catalog(path: impl AsRef<Path>) -> Result<ParseReport, ParseError> {
    parse_catalog_impl(path, false)
}

#[cfg(feature = "parallel")]
pub fn parse_catalog_parallel(path: impl AsRef<Path>) -> Result<ParseReport, ParseError> {
    parse_catalog_impl(path, true)
}

fn parse_catalog_impl(path: impl AsRef<Path>, parallel: bool) -> Result<ParseReport, ParseError> {
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

    Parser::new(source_path, parallel).parse_root()
}

#[derive(Debug)]
struct Parser {
    root_path: PathBuf,
    #[cfg(feature = "parallel")]
    parallel: bool,
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

#[derive(Debug)]
struct ParseNodeResult {
    node: Node,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Debug)]
struct ParseChildrenResult {
    nodes: Vec<Node>,
    diagnostics: Vec<Diagnostic>,
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
enum TypedContents<T> {
    Parsed(T),
    InvalidSchema(Value),
    InvalidJson(String),
}

impl Parser {
    fn new(root_path: PathBuf, parallel: bool) -> Self {
        #[cfg(not(feature = "parallel"))]
        let _ = parallel;
        Self {
            root_path,
            #[cfg(feature = "parallel")]
            parallel,
        }
    }

    fn parse_root(&self) -> Result<ParseReport, ParseError> {
        let snapshot = self.read_root_directory()?;
        let mut diagnostics = Vec::new();
        let (contents, raw_contents) = self.parse_optional_contents::<FolderContents>(
            snapshot.contents_path.as_deref(),
            Path::new(""),
            &mut diagnostics,
        );
        let children = self.parse_children(snapshot.child_directories);
        diagnostics.extend(children.diagnostics);

        Ok(ParseReport {
            catalog: AssetCatalog {
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
                children: children.nodes,
            },
            diagnostics,
        })
    }

    fn parse_children(&self, children: Vec<ChildDirectory>) -> ParseChildrenResult {
        #[cfg(feature = "parallel")]
        if self.parallel && children.len() > 1 {
            let results: Vec<_> = children
                .into_par_iter()
                .map(|child| self.parse_node(child.absolute_path, child.relative_path))
                .collect();
            return Self::combine_node_results(results);
        }

        let mut results = Vec::with_capacity(children.len());
        for child in children {
            results.push(self.parse_node(child.absolute_path, child.relative_path));
        }
        Self::combine_node_results(results)
    }

    fn combine_node_results(results: Vec<ParseNodeResult>) -> ParseChildrenResult {
        let mut nodes = Vec::with_capacity(results.len());
        let mut diagnostics = Vec::new();

        for result in results {
            nodes.push(result.node);
            diagnostics.extend(result.diagnostics);
        }

        ParseChildrenResult { nodes, diagnostics }
    }

    fn parse_node(&self, absolute_path: PathBuf, relative_path: PathBuf) -> ParseNodeResult {
        let (name, kind, folder_type) = classify_folder(&relative_path);
        let snapshot = match self.read_directory(&absolute_path, &relative_path) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return ParseNodeResult {
                    node: self.empty_node(name, relative_path.clone(), kind, folder_type),
                    diagnostics: vec![Diagnostic::new(
                        DiagnosticCode::UnreadableDirectory,
                        Severity::Error,
                        relative_path,
                        format!("failed to read directory: {error}"),
                    )],
                };
            }
        };

        let children = self.parse_children(snapshot.child_directories);
        let ParseChildrenResult {
            nodes: child_nodes,
            mut diagnostics,
        } = children;

        let node = match kind {
            FolderKind::Group => {
                let (contents, raw_contents) = self.parse_optional_contents::<FolderContents>(
                    snapshot.contents_path.as_deref(),
                    &relative_path,
                    &mut diagnostics,
                );
                Node::Group(FolderNode {
                    name,
                    relative_path,
                    files: snapshot.files,
                    raw_contents,
                    contents,
                    children: child_nodes,
                })
            }
            FolderKind::ImageSet => {
                let (contents, raw_contents) = self.parse_required_contents::<ImageSetContents>(
                    snapshot.contents_path.as_deref(),
                    &relative_path,
                    &mut diagnostics,
                );
                if let Some(contents) = contents.as_ref() {
                    self.validate_image_files(
                        &relative_path,
                        &snapshot.files,
                        &contents.images,
                        &mut diagnostics,
                    );
                }
                Node::ImageSet(FolderNode {
                    name,
                    relative_path,
                    files: snapshot.files,
                    raw_contents,
                    contents,
                    children: child_nodes,
                })
            }
            FolderKind::ColorSet => {
                let (contents, raw_contents) = self.parse_required_contents::<ColorSetContents>(
                    snapshot.contents_path.as_deref(),
                    &relative_path,
                    &mut diagnostics,
                );
                Node::ColorSet(FolderNode {
                    name,
                    relative_path,
                    files: snapshot.files,
                    raw_contents,
                    contents,
                    children: child_nodes,
                })
            }
            FolderKind::AppIconSet => {
                let (contents, raw_contents) = self.parse_required_contents::<AppIconSetContents>(
                    snapshot.contents_path.as_deref(),
                    &relative_path,
                    &mut diagnostics,
                );
                if let Some(contents) = contents.as_ref() {
                    self.validate_image_files(
                        &relative_path,
                        &snapshot.files,
                        &contents.images,
                        &mut diagnostics,
                    );
                }
                Node::AppIconSet(FolderNode {
                    name,
                    relative_path,
                    files: snapshot.files,
                    raw_contents,
                    contents,
                    children: child_nodes,
                })
            }
            FolderKind::Opaque => {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::UnsupportedFolderType,
                    Severity::Warning,
                    relative_path.clone(),
                    format!(
                        "unsupported folder type: {}",
                        folder_type.as_deref().unwrap_or("unknown")
                    ),
                ));
                let raw_contents = self.load_raw_contents(
                    snapshot.contents_path.as_deref(),
                    &relative_path,
                    &mut diagnostics,
                );
                Node::Opaque(OpaqueNode {
                    name,
                    relative_path,
                    folder_type: folder_type.unwrap_or_else(|| "opaque".to_string()),
                    files: snapshot.files,
                    raw_contents,
                    children: child_nodes,
                })
            }
        };

        ParseNodeResult { node, diagnostics }
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
        &self,
        contents_path: Option<&Path>,
        relative_path: &Path,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> (Option<T>, Option<RawContents>)
    where
        T: DeserializeOwned,
    {
        match contents_path
            .and_then(|path| self.load_typed_contents(path, relative_path, diagnostics))
        {
            Some(loaded) => Self::unpack_typed_contents(loaded),
            None => (None, None),
        }
    }

    fn parse_required_contents<T>(
        &self,
        contents_path: Option<&Path>,
        relative_path: &Path,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> (Option<T>, Option<RawContents>)
    where
        T: DeserializeOwned,
    {
        match contents_path
            .and_then(|path| self.load_typed_contents(path, relative_path, diagnostics))
        {
            Some(loaded) => Self::unpack_typed_contents(loaded),
            None => {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::MissingContentsJson,
                    Severity::Error,
                    relative_path.to_path_buf(),
                    "missing required Contents.json",
                ));
                (None, None)
            }
        }
    }

    fn unpack_typed_contents<T>(loaded: TypedContents<T>) -> (Option<T>, Option<RawContents>)
    where
        T: DeserializeOwned,
    {
        match loaded {
            TypedContents::Parsed(contents) => (Some(contents), None),
            TypedContents::InvalidSchema(value) => (None, Some(RawContents::Json(value))),
            TypedContents::InvalidJson(text) => (None, Some(RawContents::InvalidJson(text))),
        }
    }

    fn load_raw_contents(
        &self,
        contents_path: Option<&Path>,
        relative_path: &Path,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<RawContents> {
        let raw_text = self.read_contents_text(contents_path?, relative_path, diagnostics)?;
        match serde_json::from_str::<Value>(&raw_text) {
            Ok(value) => Some(RawContents::Json(value)),
            Err(error) => {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::InvalidContentsJson,
                    Severity::Error,
                    relative_path.to_path_buf(),
                    format!("invalid Contents.json: {error}"),
                ));
                Some(RawContents::InvalidJson(raw_text))
            }
        }
    }

    fn load_typed_contents<T>(
        &self,
        contents_path: &Path,
        relative_path: &Path,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<TypedContents<T>>
    where
        T: DeserializeOwned,
    {
        let raw_text = self.read_contents_text(contents_path, relative_path, diagnostics)?;

        match serde_json::from_str::<T>(&raw_text) {
            Ok(contents) => Some(TypedContents::Parsed(contents)),
            Err(error) => match error.classify() {
                Category::Data => match serde_json::from_str::<Value>(&raw_text) {
                    Ok(value) => {
                        diagnostics.push(Diagnostic::new(
                            DiagnosticCode::InvalidContentsSchema,
                            Severity::Error,
                            relative_path.to_path_buf(),
                            format!("unsupported or malformed Contents.json schema: {error}"),
                        ));
                        Some(TypedContents::InvalidSchema(value))
                    }
                    Err(value_error) => {
                        diagnostics.push(Diagnostic::new(
                            DiagnosticCode::InvalidContentsJson,
                            Severity::Error,
                            relative_path.to_path_buf(),
                            format!("invalid Contents.json: {value_error}"),
                        ));
                        Some(TypedContents::InvalidJson(raw_text))
                    }
                },
                Category::Syntax | Category::Eof | Category::Io => {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::InvalidContentsJson,
                        Severity::Error,
                        relative_path.to_path_buf(),
                        format!("invalid Contents.json: {error}"),
                    ));
                    Some(TypedContents::InvalidJson(raw_text))
                }
            },
        }
    }

    fn read_contents_text(
        &self,
        contents_path: &Path,
        relative_path: &Path,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<String> {
        match fs::read_to_string(contents_path) {
            Ok(raw_text) => Some(raw_text),
            Err(error) => {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::UnreadableFile,
                    Severity::Error,
                    relative_path.to_path_buf(),
                    format!("failed to read Contents.json: {error}"),
                ));
                None
            }
        }
    }

    fn validate_image_files(
        &self,
        relative_path: &Path,
        files: &[PathBuf],
        images: &[ImageEntry],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for image in images {
            let Some(filename) = image.filename.as_deref() else {
                continue;
            };

            let file_path = join_relative(relative_path, filename);
            if files.binary_search(&file_path).is_err() {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::MissingReferencedFile,
                    Severity::Error,
                    file_path,
                    format!("referenced file does not exist: {filename}"),
                ));
            }
        }
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
