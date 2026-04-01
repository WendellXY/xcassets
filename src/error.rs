use std::{io, path::PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("catalog root does not exist: {path}")]
    MissingRoot { path: PathBuf },
    #[error("catalog root is not a directory: {path}")]
    RootNotDirectory { path: PathBuf },
    #[error("catalog root must be an .xcassets directory: {path}")]
    InvalidCatalogRoot { path: PathBuf },
    #[error("failed to read catalog root at {path}: {source}")]
    ReadRoot {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}
