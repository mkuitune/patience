# Klondike architecture and URL format

This implementation uses the [rich Rust application pattern](RUST_RICH_APP.md)
with a small headless rules kernel and an immediate-mode `egui` interface.
Cards use the reusable [composable graphics model](COMPOSABLE_GRAPHICS.md),
rendered through egui on the `wgpu` backend.

The web build enables only the WebGPU backend. It requires a WebGPU-capable
browser, an available GPU adapter, and a secure context (HTTPS or localhost).
WebGL fallback is excluded to reduce the deployed binary. Windows builds enable
Direct3D 12 independently of the web backend selection.

`cards.rs` maps game cards to documents from `playing-card-assets`. These contain
ordered instances of shared backgrounds, suit symbols, rank glyphs, and distinct
court illustrations. `graphics2d-egui` rasterizes SVG components with resvg and
tiny-skia and caches textures at their displayed pixel size. The same renderer
runs on desktop and web. Source components and their license statement are in
[the shared asset crate](../Assets/CardComponents/README.md).

## State and update flow

`Game` owns stock, waste, seven tableau columns, and four foundations. Card
lists run from bottom to top. Each tableau stores its face-down prefix length.
Foundations store their highest rank; lower cards are implied.
Any ace can start any empty foundation slot. The ace determines that slot's suit
until the slot is emptied. State records the suit-to-slot arrangement, so undo
and bookmarks preserve the chosen positions. Empty slots have no suit labels.
Draw Three is the only ruleset. Each stock action draws up to three cards,
preserving their order; only the top waste card is playable. There is no draw-mode
setting in application state or the interface.

The interface fans only the cards remaining from the latest draw. The fan shrinks
as those cards are played; once exhausted, it shows the single top waste card.
Older waste cards remain playable as they are uncovered. Undo restores the fan.
The fan is session presentation state: loading a bookmark shows one waste card
until the next draw, without changing the board or available moves.

The update model follows Elm-style state transitions:

```text
Gesture -> proposed move -> kernel assessment -> presentation feedback
Release -> command -> validated next Game -> engine commit
        -> published read-only Game view -> presentation and URL
```

- `Game::assess_move` is a pure query returning `Result<(), MoveError>`. Rejection
  variants distinguish wrong color, wrong rank, king required, foundation rules,
  invalid sources, and invalid destinations. Presentation supplies feedback using
  these reasons; it does not duplicate the rules.
- `Game::updated` returns a new board without changing its input. The kernel
  validates the command again during this transition.
- `Engine::execute` commits only accepted transitions, records only the prior board
  for undo, and returns the resulting state view. Publication is this direct
  return, without an event bus or observer framework.
  Undo restores that one board and consumes the saved state. A rejected action
  does not replace the saved undo state. The interface offers a back-arrow button
  and the standard undo shortcut.
- The interface owns selection, drag position, destination outlines, and error
  messages. Dragging never edits the authoritative board. Legal drops commit on
  release; rejected drops leave the board unchanged. Releasing outside a target
  cancels the gesture. Rejected or off-board drops animate back to their original
  pile over 180 ms, hiding the cards at their original positions until they land.
  Moving the pointer into that pile ends the animation immediately and restores
  its normal presentation. This animation never changes the board or URL.
  Returning cards to their original pile is a neutral cancellation: no rule tip,
  destination highlight, rejection flash, state transition, or undo replacement.
- The host writes the URL after accepted commands, undo, and new deals. Invalid
  incoming links remain unchanged until the user explicitly starts a new game.

The game needs neither a simulation clock nor background evaluation. egui handles
interaction regions, drag ownership, mouse/touch input, and repaint scheduling.
Application code requests another repaint after a board action so the newly
published state appears. Reloading a bookmark resets selection and undo history.

## Help and Teach

Help and Teach are independent session settings; neither changes rules or the URL.
Help starts enabled and Teach starts disabled.

- Help enables the live green/red destination outlines and rule tips.
- With Help disabled, no live destination outlines or rule tips appear. Rejected
  drops flash the target red for 200 ms after release while the existing return
  animation runs. Drops outside any target return without a target flash.
- Teach displays `Game::suggested_moves()`: legal card moves,
  including movable sequences and moves back from foundations, plus stock drawing
  or recycling when possible. Card moves use arrows; stock actions use a pointer
  with click rays on the stock pile. These marks never capture input.
  Suggestions omit relocating an entire king-led tableau column to an empty
  column. King moves that reveal cards or come from waste or foundations remain
  suggested. `Game::legal_moves()` and move validation still allow the omitted
  relocations.
  Suggestions also omit moving a foundation ace back to the tableau. Other
  foundation moves remain eligible; manual ace moves remain legal.
  An ace's suggested destination is the leftmost empty foundation slot. All empty
  slots remain legal manual destinations; double-click uses the leftmost one.
- URL and loading failures remain visible regardless of Help.

Legal-move enumeration belongs to the kernel. Presentation maps those commands
to current card and pile rectangles, so layout and game rules stay separate.

## Language settings

The gear button at the right of the toolbar opens a modal language selector.
Language names appear in their own language. Switching language updates toolbar
labels, settings, rule tips, and game-link errors without changing the board or
undo history. English is the default and the fallback for unknown language codes.

`language.rs` keeps typed message keys and the bundled translation catalogs in
the presentation crate. The kernel returns `MoveError` and `StateError` values;
the interface maps these to localized messages. Claude Haiku supplied the ten
non-English catalogs. Card artwork retains its English-pattern corner indexes.

Web links retain the selected language in the `lang` query parameter, for example
`?lang=fi#v1:<payload>`. Board updates preserve that parameter; the board encoding
does not change. The native build retains the choice for the current session.
Help and Teach remain independent session settings.

## Version 1 encoding

Links use `#v1:<payload>`. The payload is unpadded URL-safe Base64 of the bytes
below. The first byte is the fixed rules marker `3`. Other rules markers and
other format versions are rejected.

Version 1 uses at most 77 bytes, 103 Base64 characters, and 106 fragment characters
excluding `#`.

| Bytes | Meaning |
|---|---|
| 0 | Fixed rules marker: 3 |
| 1–4 | Foundation ranks, clubs/diamonds/hearts/spades; 0 means empty |
| 5 | Stock length |
| 6 | Waste length |
| 7–20 | Seven pairs of tableau length and face-down count |
| 21 onward, before final 4 bytes | Card IDs in stock, waste, then tableau column order |
| Final 4 bytes | Suit indexes assigned to foundation slots, left to right; a permutation of 0, 1, 2, 3 |

A card ID is `suit_index * 13 + rank - 1`, with ranks 1–13 and suit indexes 0–3
in the order above. Cards within each pile are encoded bottom to top. Empty piles
contribute no card bytes. Foundation cards are omitted because their identities
are determined by their suit and highest rank.

No compression, seed,
move log, external storage, or server lookup is needed to restore a position.

Decoding checks the version, input size, Base64 encoding, exact payload length,
card ranges, unique complete deck inventory, foundation ranks, face-down
boundaries, and exposed tableau sequences. These checks establish structural
validity; they do not prove that a board is reachable from an initial deal.
The foundation slot permutation is required and validated. Earlier development
bookmarks without it are unsupported; no compatibility decoder is provided.

`history.replaceState` updates the address without creating a browser history entry
for every move. Undo is an explicit game operation. Hash navigation to another
valid board replaces the session; malformed state shows an error. No local storage
is used as an alternative source of truth.

## Version lifetime

The version identifies both rules and encoding. Once an engine is released, keep
its deployed engine assets available. A future incompatible rules or encoding
change uses a new version and bundle directory. The web loader must dispatch old
links to their retained engine; it must not silently reinterpret them.

The application is still developing its first engine, v1. Only the current v1
format is supported; earlier development URLs, including earlier v1 payloads,
have no compatibility guarantee. Version retention applies after release.
The build directory is staging output; retaining immutable published bundles is
a deployment responsibility for future releases.
