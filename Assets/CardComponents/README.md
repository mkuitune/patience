# Composable playing-card assets

This directory is the `playing-card-assets` Cargo crate. The original component
SVGs remain beside this file. [readme.txt](readme.txt) records their source and
public-domain usage statement. `SAMPLE_LAYOUT.png` provides the layout reference.
The filename `QUEEEN_DIAMOND.svg` is retained as supplied.

The library exports shared suit and rank assets, twelve separate court
illustrations, and JSON compositions for all 52 faces. The back is composed
from the shared card background and spade, without another downloaded design.
No asset depends on the Solitaire application or its rules.

See [the composition model](../../doc/COMPOSABLE_GRAPHICS.md) for placement,
mirroring, serialization, rendering, and reuse. The Cargo package includes SVGs
and license text, JSON layouts, and styles; `include_str!` embeds them when
compiling a consuming app. `library.json` is the single UUID-to-path index;
`deck.json` binds faces and reusable symbols. Edit `layouts/*.json` for placement
and `styles/card.json` for outlines. Definitions follow
[`graphics.proto`](../../crates/graphics2d-format/proto/graphics.proto) and are
validated during builds.

From the workspace root, run `cargo run -p playing-card-assets --bin check-card-assets`
after edits. The build supplies `protoc`; no separate compiler installation is needed.
