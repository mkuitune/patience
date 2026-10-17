# Klondike architecture and URL format

This implementation uses the [rich Rust application pattern](RUST_RICH_APP.md)
with a small headless rules kernel and an immediate-mode `egui` interface. Simple
cards use egui painting on the `wgpu` backend; a separate custom renderer adds no
useful boundary for this game.

## State and update flow

`Game` owns stock, waste, seven tableau columns, and four suit foundations. Card
lists run from bottom to top. Each tableau stores its face-down prefix length.
Foundations store their highest rank; lower cards are implied.

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
- `Engine::execute` commits only accepted transitions, records the prior board
  for undo, and returns the resulting state view. Publication is this direct
  return, without an event bus or observer framework.
- The interface owns selection, drag position, destination outlines, and error
  messages. Dragging never edits the authoritative board. Legal drops commit on
  release; rejected drops leave the board unchanged. Releasing outside a target
  cancels the gesture. Rejected or off-board drops animate back to their original
  pile over 180 ms, hiding the cards at their original positions until they land.
  Moving the pointer into that pile ends the animation immediately and restores
  its normal presentation. This animation never changes the board or URL.
- The host writes the URL after accepted commands, undo, and new deals. Invalid
  incoming links remain unchanged until the user explicitly starts a new game.

The game needs neither a simulation clock nor background evaluation. egui handles
interaction regions, drag ownership, mouse/touch input, and repaint scheduling.
Application code requests another repaint after a board action so the newly
published state appears. Reloading a bookmark resets selection and undo history.

## Version 1 encoding

The fragment has the form `#v1:<payload>`. The payload is unpadded URL-safe Base64
of the following bytes, in order:

| Bytes | Meaning |
|---|---|
| 0–3 | Foundation ranks, clubs/diamonds/hearts/spades; 0 means empty |
| 4 | Stock length |
| 5 | Waste length |
| 6–19 | Seven pairs of tableau length and face-down count |
| 20 onward | Card IDs in stock, waste, then tableau column order |

A card ID is `suit_index * 13 + rank - 1`, with ranks 1–13 and suit indexes 0–3
in the order above. Cards within each pile are encoded bottom to top. Empty piles
contribute no card bytes. Foundation cards are omitted because their identities
are determined by their suit and highest rank.

The binary board uses at most 72 bytes. Its payload uses at most 96 characters;
the fragment text uses at most 99 characters excluding `#`. No compression, seed,
move log, external storage, or server lookup is needed to restore a position.

Decoding checks the version, input size, Base64 encoding, exact payload length,
card ranges, unique complete deck inventory, foundation ranks, face-down
boundaries, and exposed tableau sequences. These checks establish structural
validity; they do not prove that a board is reachable from an initial deal.

`history.replaceState` updates the address without creating a browser history entry
for every move. Undo is an explicit game operation. Hash navigation to another
valid board replaces the session; malformed state shows an error. No local storage
is used as an alternative source of truth.

## Version lifetime

The version identifies both rules and encoding. Once version 1 is released, keep
its deployed engine assets available. A future incompatible rules or encoding
change uses a new version and bundle directory. The web loader must dispatch old
links to their retained engine; it must not silently reinterpret them.

The current loader supports version 1 and rejects unknown versions. The current
build directory is staging output; retaining immutable published bundles is a
deployment responsibility.
