# Composable playing-card assets

This directory is the `playing-card-assets` Cargo crate. The original component
SVGs remain beside this file. [readme.txt](readme.txt) records their source and
public-domain usage statement. `SAMPLE_LAYOUT.png` provides the layout reference.
The filename `QUEEEN_DIAMOND.svg` is retained as supplied.

The library exports shared suit and rank assets, twelve separate court
illustrations, and ordered compositions for all 52 faces. The back is composed
from the shared card background and spade, without another downloaded design.
No asset depends on the Klondike application or its rules.

See [the composition model](../../doc/COMPOSABLE_GRAPHICS.md) for placement,
mirroring, serialization, rendering, and reuse. The Cargo package includes SVGs
and license text; `include_str!` embeds them when compiling a consuming app.
