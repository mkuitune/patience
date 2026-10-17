# Patience

Klondike (pasianssi) in Rust, with a shared headless engine and an `egui`/`wgpu`
interface for desktop and browser. The browser keeps the complete board in the URL
so a bookmark restores the position without an account or database.

## Play

Choose **New game** to deal. Click the stock to draw up to three cards; click the
empty stock to recycle the waste. Only the top
waste card is playable. Move cards by dragging and releasing, or by selecting
a card and clicking its destination. Double-click a card to send it to its foundation.
Use the back-arrow button or Ctrl+Z (Command+Z on macOS) to undo the last move only.
Escape cancels selection or dragging.

**Help** is on initially: green outlines show legal destinations, and red outlines
and tips explain rejected moves. With Help off, an illegal drop briefly flashes
the destination red after release while the cards return to their pile. **Teach**
is off initially: enable it to show curved arrows for useful legal card moves and a
click/tap marker on the stock for drawing or recycling. The toggles are independent
and affect presentation only; they reset on reload.
Teach omits moving an entire king-led column to an empty column, since that only
relocates the same stack. It also omits taking aces back out of foundations.
Those moves remain legal to play.

Build tableau columns downward in alternating colors. Move complete face-up
sequences together. Only kings fill empty columns. Build each suit's foundation
from ace to king. Exposed face-down cards turn over automatically. Stock recycling
is unlimited. There is no scoring, timer, or automatic finish
in this first version.

## Run on desktop

Install Rust and the platform's native build tools, then run:

```sh
cargo run -p patience-app
```

## Build for the browser

Install `wasm-pack` and the browser target:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack --locked
```

From the repository root:

```sh
wasm-pack build apps/desktop --target web --out-dir ../../dist/engines/v1 --out-name patience_app --release --no-pack
```

Copy `web/index.html` to `dist/index.html`, then serve `dist` over HTTP. For example,
on Windows with Python installed:

```powershell
Copy-Item web/index.html dist/index.html
py -3 -m http.server 8080 --bind 127.0.0.1 --directory dist
```

Open <http://127.0.0.1:8080> with a WebGPU-capable browser. The web build requires
WebGPU and has no WebGL fallback. Phone and other LAN clients need trusted HTTPS;
plain HTTP on a LAN IP address does not enable WebGPU. Opening the HTML file
directly does not load the WebAssembly application.
`scripts/build-web.ps1` combines the build and copy steps where PowerShell permits
local scripts; pass `-Dev` for a debug build.

Deploy the contents of `dist` to static hosting. Preserve released engine bundles
under their versioned `engines/` directories; future incompatible engine versions
need separate directories and a loader entry. Only Draw Three is supported;
Draw One bookmarks are rejected. Keep the origin and base path stable for
supported bookmarks.

## Verify

```sh
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

See [Klondike architecture and URL format](doc/KLONDIKE.md) and the
[rich Rust application guide](doc/RUST_RICH_APP.md).
