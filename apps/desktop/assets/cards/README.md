# English-pattern court cards

The twelve jack, queen, and king designs are by Dmitry Fomin and are dedicated
to the public domain under [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/).
The individual Wikimedia Commons file pages identify the author and license.
Their URLs and the original download URLs are recorded in [sources.json](sources.json).

The original SVGs in `source/` are embedded in the application with
`egui::include_image!`. The host installs `egui_extras` SVG support, backed by
resvg and tiny-skia. Each card is rendered at its actual display resolution and
cached by egui's image loaders. Changing display scale requests a matching render.
Numbered cards remain painted directly.

The same SVG loader runs on native desktop and WebAssembly. Assets are bundled
locally, so play requires no Wikimedia requests or external conversion tools.
