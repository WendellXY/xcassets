use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::Diagnostic;

pub type JsonMap = Map<String, Value>;

#[derive(Debug, Clone, PartialEq)]
pub struct ParseReport {
    pub catalog: AssetCatalog,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetReferenceIndex {
    pub catalog_name: String,
    pub source_path: PathBuf,
    pub references: Vec<AssetReference>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetReference {
    pub kind: AssetReferenceKind,
    pub lookup_name: String,
    pub relative_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetReferenceKind {
    Image,
    Color,
    AppIcon,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssetCatalog {
    pub name: String,
    pub source_path: PathBuf,
    pub files: Vec<PathBuf>,
    pub raw_contents: Option<RawContents>,
    pub contents: Option<FolderContents>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Group(GroupNode),
    ImageSet(ImageSetNode),
    ColorSet(ColorSetNode),
    AppIconSet(AppIconSetNode),
    Opaque(OpaqueNode),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FolderNode<T> {
    pub name: String,
    pub relative_path: PathBuf,
    pub files: Vec<PathBuf>,
    pub raw_contents: Option<RawContents>,
    pub contents: Option<T>,
    pub children: Vec<Node>,
}

pub type GroupNode = FolderNode<FolderContents>;
pub type ImageSetNode = FolderNode<ImageSetContents>;
pub type ColorSetNode = FolderNode<ColorSetContents>;
pub type AppIconSetNode = FolderNode<AppIconSetContents>;

#[derive(Debug, Clone, PartialEq)]
pub struct OpaqueNode {
    pub name: String,
    pub relative_path: PathBuf,
    pub folder_type: String,
    pub files: Vec<PathBuf>,
    pub raw_contents: Option<RawContents>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RawContents {
    Json(Value),
    InvalidJson(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FolderContents {
    pub info: Option<ContentsInfo>,
    #[serde(default)]
    pub properties: FolderProperties,
    #[serde(flatten, default)]
    pub extras: JsonMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ImageSetContents {
    pub info: Option<ContentsInfo>,
    #[serde(default)]
    pub properties: FolderProperties,
    #[serde(default)]
    pub images: Vec<ImageEntry>,
    #[serde(flatten, default)]
    pub extras: JsonMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ColorSetContents {
    pub info: Option<ContentsInfo>,
    #[serde(default)]
    pub properties: FolderProperties,
    #[serde(default)]
    pub colors: Vec<ColorEntry>,
    #[serde(flatten, default)]
    pub extras: JsonMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AppIconSetContents {
    pub info: Option<ContentsInfo>,
    #[serde(default)]
    pub properties: FolderProperties,
    #[serde(default)]
    pub images: Vec<ImageEntry>,
    #[serde(flatten, default)]
    pub extras: JsonMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ContentsInfo {
    pub author: Option<String>,
    pub version: Option<u64>,
    #[serde(flatten, default)]
    pub extras: JsonMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FolderProperties {
    #[serde(rename = "provides-namespace")]
    pub provides_namespace: Option<bool>,
    pub localizable: Option<bool>,
    #[serde(rename = "template-rendering-intent")]
    pub template_rendering_intent: Option<String>,
    #[serde(flatten, default)]
    pub extras: JsonMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ImageEntry {
    pub filename: Option<String>,
    pub idiom: Option<String>,
    pub scale: Option<String>,
    #[serde(rename = "language-direction")]
    pub language_direction: Option<String>,
    #[serde(rename = "display-gamut")]
    pub display_gamut: Option<String>,
    pub platform: Option<String>,
    pub size: Option<String>,
    pub role: Option<String>,
    pub subtype: Option<String>,
    #[serde(rename = "matching-style")]
    pub matching_style: Option<String>,
    pub memory: Option<String>,
    #[serde(rename = "graphics-feature-set")]
    pub graphics_feature_set: Option<String>,
    #[serde(rename = "screen-width")]
    pub screen_width: Option<String>,
    #[serde(rename = "width-class")]
    pub width_class: Option<String>,
    #[serde(rename = "height-class")]
    pub height_class: Option<String>,
    #[serde(default)]
    pub appearances: Vec<Appearance>,
    #[serde(flatten, default)]
    pub extras: JsonMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ColorEntry {
    pub idiom: Option<String>,
    #[serde(rename = "display-gamut")]
    pub display_gamut: Option<String>,
    pub color: Option<ColorValue>,
    #[serde(default)]
    pub appearances: Vec<Appearance>,
    #[serde(flatten, default)]
    pub extras: JsonMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Appearance {
    pub appearance: Option<String>,
    pub value: Option<String>,
    #[serde(flatten, default)]
    pub extras: JsonMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ColorValue {
    #[serde(rename = "color-space")]
    pub color_space: Option<String>,
    #[serde(default)]
    pub components: JsonMap,
    #[serde(flatten, default)]
    pub extras: JsonMap,
}
