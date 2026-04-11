mod diagnostics;
mod error;
mod model;
mod parser;

pub use diagnostics::{Diagnostic, DiagnosticCode, Severity};
pub use error::ParseError;
pub use model::{
    AppIconSetContents, AppIconSetNode, Appearance, AssetCatalog, ColorEntry, ColorSetContents,
    ColorSetNode, ColorValue, ContentsInfo, FolderContents, FolderNode, FolderProperties,
    GroupNode, ImageEntry, ImageSetContents, ImageSetNode, JsonMap, Node, OpaqueNode, ParseReport,
    RawContents,
};
pub use parser::parse_catalog;
#[cfg(feature = "parallel")]
pub use parser::parse_catalog_parallel;
