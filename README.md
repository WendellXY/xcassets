# xcassets

`xcassets` is a Rust library for parsing Xcode asset catalogs.

It reads a single `.xcassets` directory into a typed tree, keeps unsupported
folder types visible as opaque nodes, and reports non-fatal problems as
diagnostics instead of stopping at the first issue.

Repository: <https://github.com/WendellXY/xcassets>

## What It Supports Today

The current parser has first-class support for:

- catalog roots
- groups
- image sets
- named color sets
- app icon sets

Other typed asset folders are preserved as `Node::Opaque` so callers can still
inspect the catalog tree without losing information.

## Installation

Add the crate to your project:

```toml
[dependencies]
xcassets = { git = "https://github.com/WendellXY/xcassets.git" }
```

## Example

```rust
use xcassets::{parse_catalog, Node, Severity};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = parse_catalog("Assets.xcassets")?;

    for diagnostic in &report.diagnostics {
        if diagnostic.severity == Severity::Error {
            eprintln!(
                "{}: {}",
                diagnostic.path.display(),
                diagnostic.message
            );
        }
    }

    for child in &report.catalog.children {
        match child {
            Node::ImageSet(image_set) => {
                println!("image set: {}", image_set.name);
            }
            Node::ColorSet(color_set) => {
                println!("color set: {}", color_set.name);
            }
            Node::AppIconSet(app_icon_set) => {
                println!("app icon set: {}", app_icon_set.name);
            }
            Node::Group(group) => {
                println!("group: {}", group.name);
            }
            Node::Opaque(node) => {
                println!("unsupported folder type .{}", node.folder_type);
            }
        }
    }

    Ok(())
}
```

## API Overview

The main entry point is:

```rust
pub fn parse_catalog(
    path: impl AsRef<std::path::Path>,
) -> Result<xcassets::ParseReport, xcassets::ParseError>
```

`ParseReport` contains:

- `catalog`: the parsed `AssetCatalog`
- `diagnostics`: non-fatal issues found while parsing

`ParseError` is reserved for failures that prevent producing a report at all,
such as passing a non-directory path or a path that is not a `.xcassets`
catalog root.

## Diagnostics

The parser currently emits diagnostics for cases like:

- missing required `Contents.json`
- invalid `Contents.json`
- missing referenced asset files
- unreadable files or directories
- unsupported typed asset folders

This makes the crate useful for tooling, validation, and migration workflows
where partial results are still valuable.

## Format Notes

The parser is intentionally lenient:

- root catalogs and groups may omit `Contents.json`
- supported nodes preserve unknown JSON fields in `extras`
- image renditions without `filename` are allowed
- filenames with spaces or non-ASCII characters are supported

This matches real-world Xcode projects more closely than a strict schema-only
approach.

## Development

Run the test suite with:

```bash
cargo test
```

You can also run the ignored smoke test against a real iOS project:

```bash
XCASSETS_REAL_PROJECT=/path/to/ios/project \
XCASSETS_REAL_PROJECT_LIMIT=25 \
  cargo test --test parser parses_real_project_catalogs_when_requested -- --ignored --nocapture
```

This test walks the project tree, finds `.xcassets` catalogs, and verifies a
bounded set of them can be parsed without fatal failures. It defaults to 10
catalogs so it stays fast enough for normal development, and you can raise the
limit when you want a broader smoke pass.

## License

MIT
