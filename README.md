# Patience

Klondike solitaire in Rust, for desktop and browser. The complete board fits in
77 bytes: bookmark or share a position through the URL, without an account,
database, or server-side state.

- Headless rules engine with validated state transitions and single-move undo.
- Shared `egui` interface, hosted by `eframe` and rendered with `wgpu`.
- Separate WebAssembly bundles for WebGPU and WebGL2, selected by a small
  JavaScript loader with automatic fallback.
- SVG card components composed through schema-validated JSON layouts; rasterized
  with `resvg` and `tiny-skia`.
- Interface translations for all 24 official EU languages.

## Run

Requires Rust 1.88 or later and native build tools.

```sh
cargo run -p patience-app
```

For the browser, install `wasm-pack`, the WebAssembly target, and PowerShell:

```sh
cargo install wasm-pack --locked
rustup target add wasm32-unknown-unknown
```

```powershell
./scripts/build-web.ps1
py -3 -m http.server 8080 --bind 127.0.0.1 --directory dist
```

Open <http://127.0.0.1:8080>. Add `-Dev` for a debug build. Deploy `dist` to static
hosting; WebGPU requires HTTPS or localhost. Preserve versioned `engines/`
directories and the site's base URL to keep existing bookmarks working.

## Check

```sh
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p playing-card-assets --bin check-card-assets
```

Code: [MIT](LICENSE). Card assets: [CC0](Assets/CardComponents/README.md).
