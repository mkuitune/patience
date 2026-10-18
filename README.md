# Patience

Solitaire in Rust, with a shared headless engine and an `egui`/`wgpu`
interface for desktop and browser. The browser keeps the complete board in the URL
so a bookmark restores the position without an account or database.

## Play

Choose **New game** to deal. Click the stock to draw up to three cards; click the
empty stock to recycle the waste. Only the top
waste card is playable. Move cards by dragging and releasing, or by selecting
a card and clicking its destination. Double-click a card to send it to its foundation.
Use the back-arrow button or Ctrl+Z (Command+Z on macOS) to undo the last move only.
Escape cancels selection or dragging.

**Help** opens a guide showing all 52 cards, the game's goal, and the rules.
**Teach** is off initially. Enable it for curved move arrows, a click/tap marker
on the stock, green outlines for legal destinations, and red outlines with rule
explanations for illegal moves. With Teach off, an illegal drop briefly flashes
the destination red after release while the cards return to their pile.
Teach affects presentation only and resets on reload.
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

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-web.ps1
```

The script builds the engine and writes the loader to `dist/index.html`.
Serve `dist` over HTTP. For example, on Windows with Python installed:

```powershell
py -3 -m http.server 8080 --bind 127.0.0.1 --directory dist
```

Open <http://127.0.0.1:8080> with a WebGPU-capable browser. The web build requires
WebGPU and has no WebGL fallback. Phone and other LAN clients need trusted HTTPS;
plain HTTP on a LAN IP address does not enable WebGPU. Opening the HTML file
directly does not load the WebAssembly application.
Pass `-Dev` to the build script for a debug build.

For home-network HTTPS testing, `scripts/serve-web.py --bind <PC-LAN-IP>` serves
the game on port 8443 and the public development CA certificate on port 8081 at
`/patience-dev-ca.cer`. Run it with Python 3. It expects `server.pem`,
`server-key.pem`, and `patience-dev-ca.cer` in the ignored `.tmp/local-https/`
directory. Generate the server certificate with mkcert for the PC's LAN IP and
export its public CA certificate in DER format as `patience-dev-ca.cer`.
Keep both private keys outside `dist`; the certificate endpoint serves only the
public CA certificate. Allow these ports only from the private local subnet.

On iPhone, download the certificate in Safari, install it through Settings >
Profile Downloaded, then enable its trust in Settings > General > About >
Certificate Trust Settings. Open `https://<PC-LAN-IP>:8443/` in Safari.
Use iOS 26 or later for WebGPU. The certificate's IP address must match the URL;
regenerate the server certificate if the PC's LAN IP changes.

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
cargo run -p playing-card-assets --bin check-card-assets
```

See [Solitaire architecture and URL format](doc/SOLITAIRE.md) and the
[rich Rust application guide](doc/RUST_RICH_APP.md).
Card layouts and styles are schema-validated JSON; see the
[composable graphics guide](doc/COMPOSABLE_GRAPHICS.md) for editing and reuse.
Protobuf types are generated during builds using a bundled compiler; set `PROTOC`
only when selecting a different compiler explicitly.
