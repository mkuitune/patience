# Composable 2D graphics

Graphics are ordered asset instances in document space. A view frames a document
with independent bounds. Card layouts and styles are data; Rust resolves and
renders them without generating their layout.

## Schema, storage, and runtime

[`graphics.proto`](../crates/graphics2d-format/proto/graphics.proto) defines the
portable contract. It contains no Rust-specific serialization conventions.
Prost generates Rust transport types; pbjson generates their standard ProtoJSON
readers and writers through Serde. Other languages use the same Protobuf schema
with their own compatible generators and ProtoJSON implementations.

```text
.proto -> generated transport types and JSON mappings
                         |
JSON definitions -> parse and validate -> resolve UUIDs -> runtime documents
                         ^                                    |
                         +------ export persistent UUIDs -----+
```

- `graphics2d`: backend-independent runtime geometry, typed integer handles,
  instances, styles, and views. Runtime documents do not implement serialization.
- `graphics2d-format`: generated transport types, semantic validation, UUID
  resolution, and document export. It needs neither a GUI nor filesystem access.
- `graphics2d-egui`: SVG parsing, rasterization, texture caching, and composition.
- `playing-card-assets`: shared SVG sources, JSON layouts and styles, deck
  bindings, and build-time validation. It has no dependency on Klondike rules.

Protobuf defines structure and field types. The loader additionally checks UUIDs,
reference kinds, finite geometry, positive scale, handedness, color channels, and
style units. Unknown JSON fields are rejected. Parsing a Protobuf message alone
does not establish that it is a valid graphics document.

## Resource identity

Every library, SVG source, document, and style has a persistent UUID. The library
manifest is the single UUID-to-path index. Paths locate content; they are not its
identity. Keep a UUID when moving or editing a resource. Give an independent copy
a new UUID.

`Context::load` accepts manifest text and a caller-supplied content resolver. It
checks duplicate definitions and resolves references once, independently of file
order. Assets, styles, and documents receive separate typed integer handles.
Rendering uses these handles and dense arrays, without UUID lookups.

Handles belong to one immutable application context. Do not persist handles or
carry them into another context. The context retains index-to-UUID metadata;
`export_document` restores persistent references into generated transport types.
The renderer and its documents must use the same context's resources.

## Instances and placement

A document contains an ordered instance list: first paints first. Each instance
references an SVG asset, a placement, optional color, and optional vector style.
An absent color uses the style's fill color when supplied, otherwise source colors.
A supplied color uses source alpha
coverage as a mask, so suit and rank artwork can be reused in other colors.

`Placement::Simple` contains five numbers and a handedness sign:

```text
position:     (x, y)
x_direction:  (dx, dy), normalized during loading
scale:        positive uniform scale
handedness:   +1 or -1

X = (dx, dy)
Y = handedness * (-dy, dx)
document_point = position + scale * (local_x * X + local_y * Y)
```

Coordinates use X right and Y down. Positive handedness with X = (1, 0) maps
local Y downward. Negative handedness reflects across local X. Positive
handedness with X = (-1, 0) rotates by 180 degrees. Placement resolves to a 3x3
affine matrix using column vectors. The Protobuf placement is a `oneof`, leaving
room for a future affine variant when skew or nonuniform scale is needed.

A view's bounds control framing; its clip flag independently requests clipping.
The renderer fits the bounds into the target with uniform scale and centers the
result. Application hit regions remain separate from artwork and its outline.

## Card definitions and editing

The files in [`Assets/CardComponents`](../Assets/CardComponents) are:

| File | Purpose |
|---|---|
| `library.json` | Library UUID, format version, and UUID-to-path resource index |
| `deck.json` | Rank/suit-to-document bindings, back, and reusable suit/rank references |
| `layouts/*.json` | Ordered instances and view bounds for each face and the back |
| `styles/card.json` | Shared cream fill, softened outline, shadow, and units |
| `styles/royal-frame.json` | Light gray inward court frame, measured in source units |
| `*.svg` | Shared backgrounds, suits, ranks, frame, and twelve court illustrations |

Edit an instance's `simple.position`, `xDirection`, or `scale` to adjust layout.
Change instance list order to change paint order. References use UUIDs, never
filenames or runtime indexes. A two of spades has seven instances: background,
four spades, and two digit-2 glyphs. The supplied layouts use 750 by 1050 units;
the document view carries those bounds rather than requiring renderer constants.

After edits, run from the repository root:

```sh
cargo run -p playing-card-assets --bin check-card-assets
cargo run -p graphics2d-egui --example card_sheet -- target/card-sheet.png
```

The checker optionally accepts another library directory as its final argument.
The card crate also validates bundled JSON and references during every relevant
build. SVG parsing and supported rendering behavior are checked by renderer
tests. The sheet writes a PNG for visual review; the application still uses SVGs.

The build embeds definitions and sources, with no working-directory or network
requirements during play. `build.rs` derives the embedded source table from the
manifest; there is no second manually maintained UUID/path table. Cargo packages
include definitions, sources, and license text. No registry publication is required.

```rust
use graphics2d_egui::Renderer;
use playing_card_assets::{Deck, Suit};

let deck = Deck::bundled()?;
let mut renderer = Renderer::new(deck.context.resources.clone());
let view = deck.face_view(2, Suit::Spades).unwrap();
// During an egui frame: renderer.begin_frame(); renderer.paint(ui, view, target)?;
```

## Outlines and units

The shared card style uses a slight cream fill (`#fffdf7`), a faint warm gray
0.65-logical-pixel outline with 0.3 units of blur, and a subtle shadow offset
downward by 2 logical pixels with 3 units of blur at about 24% opacity. Blur values are Gaussian
standard deviations. The renderer strokes the actual background path and clips
the stroke to its interior, preserving the underlying card boundary and hit region.
Blur and shadows can extend beyond that boundary; explicit view clipping still
applies. Outline support currently requires
a single closed SVG path without effects; unsupported sources return an error.
The bundled card background meets that constraint.

Court cards use `ROYAL_FRAME_BORDER_ONLY.svg` with a light gray, 6-source-unit
inward outline. Their frame placements have unit scale, so the line is 6 document
units wide. A transparent instance color suppresses the SVG fill while retaining
the outline. Transparent fills do not allocate fill textures or emit fill quads.

`fillColor` supplies the fill when an instance preserves original paint; explicit
instance colors take precedence. `outlineBlur` uses the outline's width unit.
A shadow has its own color, offset, blur, and unit. Logical-pixel shadow offsets
follow screen axes; source-unit offsets scale with the instance and view.
Blur is applied after clipping the inward outline, softening its outer edge.

- `WIDTH_UNIT_SOURCE`: width is measured in the SVG's source coordinates and
  scales with instance placement and document zoom.
- `WIDTH_UNIT_LOGICAL_PIXELS`: width is measured in host UI units, independently
  of instance scale and document zoom. Egui calls these units points.

Logical pixels are not physical display pixels or subpixels. Physical width is
logical width multiplied by egui's `pixels_per_point`. Windows display scaling
and browser page zoom affect that conversion with the rest of the UI. Do not
apply device pixel ratio or operating-system scaling a second time. These units
do not promise an exact physical size in millimetres.

Keep one renderer per egui context. It parses each SVG once and caches textures
by asset handle, pixel dimensions, and rendering treatment. Recoloring shares a
mask texture; outlines and shadows have separate textures so fill tint does not
alter their colors. Resvg applies Gaussian blur when creating a cached texture,
with padding for the blur falloff. Cards of the same size and display scale share
their shadow and outline textures across faces and frames. Pointer movement only
changes quad placement. Rotation and reflection apply to quad geometry.

During window resizing, `RasterScale` keeps texture resolution and effect scale
fixed while presentation geometry follows the window. The card host waits for
180 ms without a viewport size or display-scale change before rendering at the
new resolution. This avoids rasterization and texture uploads on each resize event.

Textures unused for two frames are released. Each texture is limited to the lesser of 4096 pixels per
side and egui's texture limit. Bundled SVGs are paths; SVG text and external
resources are outside this asset library's contract.

## Dependency decisions

The contract is Protobuf (A); the Rust implementation uses Prost (B), Serde and
serde_json (A), pbjson (B), and UUID (B). These ratings follow the
[ecosystem resilience rule](RUST_RICH_APP.md#ecosystem-resilience).

Pbjson has direct downstream use in [Apache DataFusion](https://github.com/apache/datafusion/blob/main/Cargo.toml)
and [InfluxDB's core workspace](https://github.com/influxdata/influxdb3_core/blob/main/Cargo.toml).
That establishes B under the downstream-use rule. Its small public profile alone
is not evidence of ecosystem isolation. The compiler helper
`protoc-bin-vendored` (B, build only) supplies a compiler without manual setup;
[Apache DataFusion Java](https://github.com/apache/datafusion-java/blob/main/Cargo.toml)
declares it directly. `PROTOC` can override its compiler path. The helper is a
third-party packaging project, distinct from Google's compiler project. Cargo.lock
pins the versions used here. These adoption observations were checked in October
2026; reassess if upstream maintenance or compatibility changes.

## Future extensions

The same loader can consume bundled files, memory content, or editor-produced
definitions. Document export uses the same generated schema. A future editor can
therefore save ProtoJSON and put a compressed, Base64url-encoded document into a
URL without replacing the renderer or resource identity model. That feature still
needs a versioned envelope, input limits, and a way to supply referenced artwork.
The editor and URL codec are not implemented.

A future view-instance extension must define clipping, transform composition,
and cycle rejection. The current flat instance list does not implement a scene
graph.
