# Rich Rust application architecture

Use this pattern for desktop-class Rust applications with 3D scenes, custom 2D
content, panels, inspectors, and overlays. The default is an `egui`/`eframe` shell,
`wgpu` rendering, and a headless command-driven application engine.

This is the rich-application path. Small browser payloads have a separate guide
in [RUST_MINIFIED_WEB_APP.md](RUST_MINIFIED_WEB_APP.md).

## Architectural taste

Choose the simplest structure that preserves correct ownership, responsive
interaction, and testable behavior. Add a boundary when it protects an actual
responsibility; add an abstraction when concrete uses justify it.

- Prefer concrete types, direct calls, and explicit data flow.
- Keep application behavior independent of the GUI and GPU.
- Use one command path for edits, regardless of whether input comes from a panel,
  viewport tool, shortcut, or automation.
- Separate authoritative state from derived scene data and GPU resources.
- Start with modules. Split crates where dependency enforcement, reuse, or build
  isolation makes the boundary useful.
- Add infrastructure when the application needs it. A plugin framework, generic
  event bus, service container, or custom UI framework needs a concrete reason.

Production quality comes from correct behavior, deliberate failure handling,
measured performance, and verified platform support.

### Cargo-based builds

Keep project builds self-contained within Cargo. Beyond Rust and the target's
standard platform toolchain, a fresh checkout should build with ordinary Cargo
commands. Use Cargo build dependencies and `build.rs` for code generation and
required tools. Avoid manual installations, global environment changes, and
pre-build commands outside Cargo whenever an established Cargo-integrated option
exists. Document and justify any unavoidable exception.

### Ecosystem resilience

Choose dependencies partly by the likelihood that their ecosystem will detect
and address security incidents and maintain compatibility with our language and
target platforms. Use a roughly logarithmic adoption scale: a specialized
component can be a sound choice without matching the largest projects in usage.

| Class | Meaning | Selection rule |
|---|---|---|
| A | Broad adoption with substantial institutional support and many independent users | Prefer when suitable |
| B | Specialized but established, with enough active users and maintainers to support continued security attention and compatibility | A normal, acceptable choice |
| C | Limited adoption or insufficient evidence of continued ecosystem care | Investigate deeply before adopting; do not automatically reject |

Rate the concrete implementation and integration we depend on. An A-class format
does not make every library implementing it A. For example, assess Protobuf and
a Rust ProtoJSON adapter separately. Ratings express evidence-based expectations,
not security guarantees.

Check substantial downstream users, maintenance history, maintainer continuity,
security response, compatibility tests, and support for our actual targets.
Downloads and stars are supporting signals, not counts of independent users.
Direct dependency use by a high-profile A-class project establishes a B rating
unless evidence shows the component is end-of-life or being deprecated. Check
that the dependency is current; historical use alone does not establish continued
ecosystem care. This rating does not replace verification on our target platforms.
For C components, understand these points from the start, including unresolved
issues and the cost of replacing the component or maintaining it ourselves.
Record the evidence, uncertainties, and reason for adoption with the decision;
reassess when maintenance or platform support changes materially.

## Default library stack

| Responsibility | Choice | Use |
|---|---|---|
| Application shell and interface | `eframe`, `egui` | Window, lifecycle, panels, controls, overlays |
| Rendering | `wgpu`, `egui-wgpu` | Custom scene rendering integrated with the GUI |
| SVG vector assets | `egui_extras` SVG loader, `resvg`, `tiny-skia` | Runtime SVG rendering and size-aware texture caching |
| Math | `glam` | Vectors, transforms, cameras, quaternions |
| Serialization | `serde`, `serde_json` | Settings, persisted state, command payloads |
| Domain errors | `thiserror` | Errors callers can inspect and handle |
| Host error context | `anyhow` | Startup, loading, and application orchestration |
| CPU parallel work, when needed | `rayon` | Expensive parallel computation |
| Worker messages, when needed | `crossbeam-channel` | Requests and results between workers and UI |
| File dialogs, when needed | `rfd` | Platform file selection |
| Local database, when needed | `rusqlite` | Structured local persistence |
| Spatial queries, when needed | `rstar`, a mesh BVH | Spatial indexing and ray queries |
| Verification | `cargo test`, `egui_kittest` | Headless behavior and UI interaction tests |

A BVH is a bounding volume hierarchy used to accelerate spatial queries. Introduce
one when scene size warrants it; direct hit tests are sufficient for small scenes.

Select mutually compatible versions of `eframe`, `egui`, `egui-wgpu`, and `wgpu`
together. Use the `wgpu` backend explicitly and commit the application lockfile.
The table assigns responsibilities; it is not a requirement to install every
optional dependency on day one.

### Vector graphics

For reusable programmable graphics, use the [composable 2D graphics
model](COMPOSABLE_GRAPHICS.md): ordered asset instances in document space, with
independent views. `graphics2d` owns backend-independent data;
`graphics2d-egui` draws it; asset crates embed shared artwork and compositions.
The card library in `Assets/CardComponents` is the first consumer.

Treat SVG as a runtime asset format. For SVG assets displayed through egui, enable
`egui_extras`'s `svg` feature and install its image loaders when creating the GUI
context. This uses resvg to interpret SVG and tiny-skia to render on the CPU.
Keep the source vectors and request rendering at the actual pixel size, including
display scaling. Reuse the loader's cached textures between frames. Bundle fixed
application artwork so it does not depend on third-party servers during use.

The SVG loader is one concrete rendering capability. A general-purpose vector
engine also needs decisions about paths, text, effects, compositing, and CPU/GPU
execution. Skia and Vello are candidates for that broader responsibility; no
general-purpose backend has been selected yet. Keep this responsibility in the
renderer, with authoritative geometry and edits independent of its types.

TODO: Choose the general-purpose vector renderer after comparing Skia and Vello
on Windows and WebGPU, including supported features, rendering quality,
integration with the shared GPU context, performance, and deployed size.

## Ownership and dependency direction

```text
Host -> UI and interaction -> engine -> application state
                                |
                                v
                         evaluated scene
                                |
                                v
                      renderer and GPU cache

Persistence <-> application state
Automation  -> engine commands
```

- **Host:** startup, platform integration, shutdown, and resource setup.
- **UI and interaction:** layout, focus, input routing, tools, and transient UI
  state. Converts user intent into engine operations.
- **Engine:** commands, validation, application rules, sessions, and undo history.
  Owns authoritative application state and commits edits.
- **Evaluated scene:** derived drawing data, mesh and instance identifiers,
  transforms, materials, and generations. Contains no GUI or GPU handles.
- **Renderer:** GPU resources, uploads, render targets, and draw scheduling.
  Consumes scene data; does not mutate application state.
- **Persistence:** versioned state encoding and storage at the engine boundary.
  Storage policy belongs to the application: file, database, or URL.

The engine and its state must build and run without `egui`, `eframe`, or `wgpu`.
The renderer depends on scene types; scene types do not depend on the renderer.
UI state may use GUI types, but durable state must not require them.

### Persistent resource identity and runtime handles

Identify persistent resources with UUIDs. Resolve UUID references once when
loading an application context, using a UUID-to-typed-index map. Runtime assets,
documents, and evaluated data reference compact integer handles and dense storage;
do not use UUID strings or UUID lookups as the main reference mechanism during
rendering and evaluation. This keeps large models from becoming collections of
repeated GUID strings and hash lookups.

Handles are specializations for one context's lifetime. Never persist handles or
assume they identify the same resource in another context. Keep reverse
index-to-UUID metadata for export. Reject duplicate definitions, missing references,
and references to the wrong resource kind. Preserve UUIDs across edits and moves;
assign new UUIDs to independent copies. Start with append-only or immutable storage;
if slots are later reused, prevent stale handles from referring to new resources.

Keep transport types separate from runtime types. For portable schema-defined
resources, use Protobuf with generated types and its standard ProtoJSON mapping.
Validate application constraints and resolve UUIDs at that boundary. The
[composable graphics implementation](COMPOSABLE_GRAPHICS.md) uses Prost and pbjson
for Rust; Serde remains the general-purpose choice for other formats.

## Starting layout

Start with two crates in a Cargo workspace. The core crate enforces the headless
boundary; the application crate assembles the shell and renderer.

```text
crates/core/src/
  lib.rs
  state.rs          # Authoritative application data
  command.rs        # Typed edits, validation, undo boundaries
  engine.rs         # Session ownership and command execution
  scene.rs          # Derived scene data and scene updates
  persistence.rs    # Versioned encoding and decoding
apps/desktop/src/
  main.rs           # eframe startup
  app.rs            # Application assembly and update loop
  ui.rs             # Panels, controls, overlays
  interaction.rs    # Viewport input, tools, camera control
  renderer.rs       # wgpu resources and viewport rendering
  jobs.rs           # Worker requests and results, when needed
```

Create modules as functionality arrives. Split the renderer or scene into its own
crate when there is a concrete benefit. Add a CLI host when automation needs one;
it calls the same core engine.

## State and commands

Keep three kinds of state distinct:

1. **Authoritative:** user data and committed edits needed to reconstruct the
   application.
2. **Session and interaction:** selection, focus, camera, active tool, drag preview.
   Persist the subset the product needs to restore.
3. **Derived:** scene data, tessellation, spatial indexes, GPU buffers. Rebuild
   these from the relevant authoritative and session state.

Use typed commands for meaningful edits. Validate and commit in the engine; UI
widgets must not bypass that path. Ordinary focus changes and camera navigation
can use direct session operations without becoming undoable commands.

Model a drag as preview, commit, or cancel. Commit one undoable edit; intermediate
pointer movements must not flood history. Use straightforward snapshots for small
state. Introduce shared immutable data or change records when measured state size
and history costs justify them.

Keep persistence versions explicit. Validate decoded state before accepting it.
Persist stable application identifiers and data rather than renderer handles.

### Interaction and state transitions

Use an Elm-style update flow: an application event produces a new authoritative
state or a typed rejection. The engine commits accepted transitions and exposes
the resulting state view to presentation and persistence. A direct return is
sufficient publication; introduce subscriptions only when actual consumers need
them.

For interactions that propose an edit, let presentation query the kernel before
committing. The query returns legal or illegal with an enumerated reason and
does not mutate application state. Presentation uses that result for activation
highlights and rejection feedback. Release commits the proposed edit; the kernel
validates it again. Cancellation leaves authoritative state unchanged.

Keep continuous simulation updates separate from undoable user edits. The same
ownership rule applies: engine logic advances authoritative state, and rendering
consumes published state without advancing application rules. The detailed
presentation and scheduling model remains part of the TODO below.

## Rendering and composition

Use the `wgpu` device and queue provided by the `eframe` integration. Keep one
window and one GPU context for the default application.

- Reserve an `egui` region for the viewport. Integrate custom rendering through
  `egui_wgpu::CallbackTrait`.
- For simple 2D content, draw in the existing GUI render pass when its attachment
  and ordering constraints are sufficient.
- For a 3D viewport, render into owned color and depth targets, then compose the
  color result into the viewport. Encode offscreen work in callback preparation
  and compose during painting.
- Draw controls and interaction overlays above the scene. Define layer order
  explicitly: scene, scene annotations, interaction feedback, application UI.
- Add multisampling, transparency passes, selection targets, and post-processing
  when features require them. Keep their resources and lifetimes in the renderer.
- Own target resizing, logical-to-physical pixel conversion, clipping, texture
  lifetime, and GPU failure recovery explicitly.

Start with callback integration. Take over application render scheduling only
when a demonstrated requirement exceeds that integration.

Express UI-sized strokes and hit tolerances in logical pixels (egui points).
Physical pixels equal logical pixels multiplied by `pixels_per_point`, which
accounts for host display scaling and browser page zoom. Do not apply that scaling
twice. Document/source units scale with content zoom; logical-pixel styles remain
constant in UI units as content zoom changes. Neither unit promises a physical
size in millimetres.

Camera changes update view uniforms. They must not rebuild unchanged geometry or
recompute application rules. Cache scene and GPU data by stable identity and
generation; upload only changed resources. For large coordinate ranges, retain
appropriate CPU precision and use positions relative to a nearby origin on the
GPU.

## Input and selection

Route input through the UI and interaction controller before changing the scene.
Respect GUI focus, pointer ownership, and active drags so a panel interaction does
not also operate the viewport.

Use egui's interaction and focus machinery. Input policy remains application
specific: egui identifies the interaction; the application decides whether it
moves a card, paints pixels, selects an object, or controls a camera.

- For an embedded viewport, register an interaction region with a stable `Id`
  and an appropriate `Sense`. Use its `Response` for hover, clicks, and drag
  start/update/end. Let egui arbitrate overlapping widgets rather than independently
  testing every region against raw pointer input.
- For a game that handles input outside egui with a GUI overlay, respect egui's
  pointer and keyboard input-interest signals before routing input to the game.
  Do not apply that blanket gate to an embedded egui viewport: the viewport itself
  participates in egui interaction and should use its own response.
- Define overlay interaction explicitly. Visual transparency does not imply
  input pass-through. Application-specific gestures, shortcuts, and focus rules
  build on egui conventions; they do not require a separate input framework.

See the [egui interaction API](https://docs.rs/egui/latest/egui/struct.Ui.html#method.interact),
[`Response`](https://docs.rs/egui/latest/egui/response/struct.Response.html), and
[input-interest signals](https://docs.rs/egui/latest/egui/struct.Ui.html#method.egui_wants_pointer_input).
Check exact API names against the selected egui version.

Keep screen, viewport, and world coordinate conversions explicit. Account for
display scaling in hit tolerances. Use cursor-centered zoom and deliberate
pointer capture for drags.

Keep picking independent of rendering. Start with CPU hit tests or ray queries;
accelerate them with spatial indexes when needed. Add GPU ID picking only when
visible-surface selection requires it, and account for asynchronous readback.
Tools should consume application object identifiers rather than GPU details.

## Updates and background work

Keep input and GUI updates on the UI thread. Begin with synchronous computation
while it stays comfortably within the interaction budget. Move expensive scene
building, decoding, and computation to workers as needed.

```text
Input -> preview or command -> state revision
      -> compute scene update -> accept current result
      -> upload changed resources -> repaint
```

Tag worker requests and results with session identity, state revision, and request
generation as needed. Accept a result only while it still matches the intended
state. Coalesce superseded preview work and bound pending work; an obsolete result
must never overwrite newer state.

Keep the last complete scene visible while an update runs. Preserve the revision
relationship between displayed geometry and selection queries. Workers return
data and errors; the UI thread accepts results and requests repaint. Do not hold
state write locks during expensive computation or GPU uploads.

Repaint on input, completed work, and active animation. Use a continuous update
loop only while the application has work that needs it. If simulation requires a
fixed timestep, keep that clock separate from rendering cadence.

## Production checks and platform scope

- Test commands, validation, undo, persistence round trips, and scene invalidation
  headlessly.
- Test interaction boundaries: preview/commit/cancel, focus, overlays, clipping,
  display scaling, resizing, stale results, and shutdown with work in flight.
- Exercise GPU failure and recovery paths on supported platforms. Use diagnostics
  that identify failed operations and relevant state revisions.
- Measure startup, idle CPU use, interaction latency, scene update time, and GPU
  resource growth with representative workloads.
- Keep dependency direction enforced as the workspace grows.

Keep the engine portable. Browser support requires deliberate host choices for
workers, persistence, dialogs, and GPU capabilities; native libraries alone do not
establish browser support. Verify each promised target with a working build and
interaction tests. Keep platform differences at host boundaries.

## First implementation sequence

1. Build the core state and one typed command; test it without a GUI.
2. Create the `eframe` host with a custom `wgpu` viewport and camera control.
3. Display one scene object and a 2D interface overlay on the shared GPU context.
4. Complete one interaction through preview, commit, cancel, and undo.
5. Add versioned state round trips and CPU selection.
6. Add scene generations, caching, and workers where representative workloads
   demonstrate the need.
7. Verify platform lifecycle, failure handling, packaging, and performance before
   expanding features.

## TODO: Presentation architecture and application archetypes

Describe the high-level presentation architecture once the model has solidified.
The detailed update and composition model remains open; use these cases to refine
it before prescribing a common implementation.

Cover four application archetypes: 2D games, 3D games, desktop applications led by
2D layout, and desktop applications containing 3D views. Use Klondike, vector
drawing, layered pixel drawing, a rich 3D editor, and both static-state and
continuous-action 2D/3D games as concrete examples.

Clarify:

- Application state updates, including edits and simulation, independently of
  presentation and rendering.
- Multiple presentation states, their ownership, and visual animation without
  changes to authoritative application state.
- Rendering presentation parts into shared or separate targets, then composing
  their outputs with explicit layering and transparency.
- State changes versus repaint dependencies: an unchanged transparent GUI may
  need recomposition as the underlying 3D view changes.
- Update and repaint scheduling for idle interfaces, animation playback, and
  continuous simulation, without requiring a general scheduling framework.
- Application-specific viewport gestures, shortcuts, and overlay interaction,
  following the egui conventions in [Input and selection](#input-and-selection).
  Keep visual transparency separate from input behavior.

## Basis

Adapted from *Modern Rust Application Architecture* (15 October 2026), the supplied
study of CADCraft's application boundaries and `egui`/`wgpu` integration. This guide
retains the general application pattern and omits CAD-specific architecture.
The two-crate starting layout and staged adoption of optional infrastructure are
project recommendations.
