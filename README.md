# xcassets

`xcassets` is a Rust library for reading Xcode asset catalogs.

It is aimed at tooling authors who need one of these two jobs:

- parse a catalog into a typed tree
- index runtime asset lookup names without paying for full parsing

The crate reads a single `.xcassets` directory, keeps unsupported folder types
visible as opaque nodes, and reports non-fatal problems as diagnostics instead
of stopping at the first issue.

Repository: <https://github.com/oops-rs/xcassets>

## Choose An API

Use `parse_catalog(...)` when you need the actual catalog structure and
`Contents.json` data:

- groups
- image sets
- color sets
- app icon sets
- raw JSON preservation on malformed or unsupported nodes
- diagnostics such as missing files or invalid `Contents.json`

Use `index_asset_references(...)` when you only need runtime lookup names such
as `icon`, `Navigator/icon_back`, or `Game/coin`:

- much lighter-weight than full parsing
- does not parse leaf image-set or color-set rendition configs
- only reads folder `Contents.json` when needed for namespace resolution
- returned order is not guaranteed

## What It Supports

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

```bash
cargo add xcassets
```

Enable the optional parallel parser when you want multi-threaded subtree walks:

```bash
cargo add xcassets --features parallel
```

## Quick Start

### Full Parse

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

### Runtime Reference Index

```rust
use xcassets::{AssetReferenceKind, index_asset_references};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let index = index_asset_references("Assets.xcassets")?;

    for reference in &index.references {
        match reference.kind {
            AssetReferenceKind::Image => {
                println!("image lookup: {}", reference.lookup_name);
            }
            AssetReferenceKind::Color => {
                println!("color lookup: {}", reference.lookup_name);
            }
            AssetReferenceKind::AppIcon => {
                println!("app icon set: {}", reference.lookup_name);
            }
        }
    }

    Ok(())
}
```

## API Reference

### `parse_catalog`

```rust
pub fn parse_catalog(
    path: impl AsRef<std::path::Path>,
) -> Result<xcassets::ParseReport, xcassets::ParseError>
```

Returns a `ParseReport`:

- `catalog`: the parsed `AssetCatalog`
- `diagnostics`: non-fatal issues found while parsing

`ParseError` is reserved for failures that prevent producing a report at all,
such as passing a non-directory path or a path that is not a `.xcassets`
catalog root.

### `parse_catalog_parallel`

With the `parallel` feature enabled, you can opt into sibling-directory parsing
across the Rayon thread pool:

```rust
#[cfg(feature = "parallel")]
pub fn parse_catalog_parallel(
    path: impl AsRef<std::path::Path>,
) -> Result<xcassets::ParseReport, xcassets::ParseError>
```

The parallel entry point preserves the same child ordering and diagnostic
ordering as the sequential parser so callers can compare results directly.

### `index_asset_references`

For callers that only need runtime lookup names, there is also a lightweight
indexer:

```rust
pub fn index_asset_references(
    path: impl AsRef<std::path::Path>,
) -> Result<xcassets::AssetReferenceIndex, xcassets::ParseError>
```

Returns an `AssetReferenceIndex`:

- `catalog_name`: the catalog stem
- `source_path`: the `.xcassets` root
- `references`: discovered runtime asset names
- `diagnostics`: non-fatal issues found while resolving namespace metadata

This API walks the catalog tree and only consults folder `Contents.json` files
when needed to resolve namespace-providing groups or sprite atlases. It does
not parse leaf image-set or color-set rendition configs, which makes it a good
fit for code generation, lookup validation, and resource indexing.

The returned reference order is not guaranteed. Callers that need a stable
order should sort the references themselves.

## Diagnostics

The crate emits diagnostics for cases like:

- missing required `Contents.json`
- invalid `Contents.json`
- missing referenced asset files
- unreadable files or directories
- unsupported typed asset folders

This makes the crate useful for tooling, validation, and migration workflows
where partial results are still valuable.

## Behavior Notes

The parser is intentionally lenient:

- root catalogs and groups may omit `Contents.json`
- supported nodes preserve unknown JSON fields in `extras`
- image renditions without `filename` are allowed
- filenames with spaces or non-ASCII characters are supported

This matches real-world Xcode projects more closely than a strict schema-only
approach.

Namespace-aware asset references follow the asset catalog rules for
`provides-namespace` on groups and sprite atlases.

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
