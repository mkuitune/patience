# Composable 2D graphics

The `graphics2d` crate describes graphics as ordered instances in a document.
It depends on math and serialization libraries, with no GUI or GPU dependencies.
`graphics2d-egui` renders that data. `playing-card-assets` is one reusable asset
library and a set of card compositions; it has no dependency on Klondike rules.

## Data model

- A `SvgAsset` contains a name, embedded SVG, and local-space bounds.
- An `AssetId` indexes the asset library supplied to the renderer. IDs belong to
  that library; persist documents together with the library's identity/version.
- An `Instance` references an asset, a placement, and a paint choice. `Original`
  preserves source colors. `Solid` colors the source's alpha coverage, allowing
  the same glyph or suit to appear in different colors without copied geometry.
- A `Document` contains the instance list. The first instance paints first;
  later instances paint over it. There is no separate layer or depth field.
- A `View` references a document and a bounding box in document coordinates.
  Bounds define framing. The `clip` flag independently requests clipping.

Documents, instances, and placements support Serde serialization. Asset geometry
is embedded in its Cargo library rather than repeated in stored instances.

## Placement and coordinates

`Placement::Simple` contains five numbers and a handedness sign:

```text
position:     (x, y)
x_direction:  (dx, dy), normalized
scale:        positive uniform scale
handedness:   +1 or -1
```

Constructors and deserialization reject invalid coordinates, zero directions,
nonpositive scales, and invalid handedness. Constructors normalize direction.
Position locates the asset's local origin in document space.

```text
X = (dx, dy)
Y = handedness × (-dy, dx)
document_point = position + scale × (local_x × X + local_y × Y)
```

Documents use X to the right and Y down. With X = (1, 0), positive handedness
maps local Y downward. Negative handedness reflects across the local X axis.
Using X = (-1, 0) with positive handedness rotates by 180 degrees.
`matrix()` resolves placement to a 3×3 affine matrix using column vectors.

The egui renderer fits the view bounds inside the target rectangle with uniform
scale and centers the result. Instance placement and view mapping compose as:

```text
asset local coordinates -> document coordinates -> target coordinates
                         instance placement        view mapping
```

Interaction geometry belongs to the application. A card's hit region remains
its full card rectangle, independently of the graphics inside it.

## Card library

`Assets/CardComponents` is the `playing-card-assets` Cargo crate. Its package
contains its Rust compositions, source SVGs, and license statement. It embeds
assets with `include_str!`; consumers need no downloads or working-directory
assumptions. Use a workspace path dependency now; the same crate boundary supports
distribution with the required companion crates. No registry publication is
required or performed by this project.

`face(rank, suit)` creates a card document; `back()` creates a blue card back from
the same shared background and spade. All card documents use 750×1050 units.
`SAMPLE_LAYOUT.png` is the source reference for pip arrangements and court layout.
Suit assets are available independently through `Suit::asset()`, and rank glyphs
through `rank_asset()`.

A two of spades has seven instances: one background, four instances of the
spade, and two instances of the digit 2. Court cards retain twelve distinct
illustrations, adding shared suit symbols, rank indexes, and a frame.

```rust
use graphics2d_egui::Renderer;
use playing_card_assets::{self as cards, Suit};

let mut renderer = Renderer::new(cards::ASSETS);
let document = cards::face(2, Suit::Spades).unwrap();
// In the frame: renderer.begin_frame();
// In an egui UI: renderer.paint(ui, cards::view(&document), target)?;
```

Keep one renderer per egui context. It parses each SVG once, rasterizes shared
components at the requested pixel size, and draws transformed texture quads in
instance order. Recoloring shares an alpha-mask texture; it does not duplicate
the SVG or require another texture for each color. Rotation and reflection are
applied to geometry, independently of cached textures. Raster sizes include
display pixel density. Textures unused for two frames are released to bound
cache growth during resizing. Individual textures are limited to the lesser of
4096 pixels per side and the egui context's texture limit; exceeding this returns
a rendering error. SVG text and external resources are not part of this asset
library's contract; bundled components are paths.

For a windowless review of all 52 compositions and the back, run
`cargo run -p graphics2d-egui --example card_sheet -- target/card-sheet.png`.
This example uses the same document data with CPU vector rendering and writes a
PNG contact sheet; it does not create replacement raster assets for the app.

## Extension points

Add an affine placement variant when nonuniform scale or skew has a concrete use.
Both variants can resolve to the renderer's existing matrix path. A view can
later become an instance target; that extension must define clipping, transform
composition, and cycle rejection. Neither capability is implemented yet.
