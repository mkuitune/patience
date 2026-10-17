
# High level

A collection of patience and klondike games.

* persistent as-small-as-possible state
* implemented in Rust (see Rust usage below)
* url is always the base url plus the serialization of the game state
* published engine versions are never destroyed. Url specifies engine version (v1, v2..) so old urls always work and new engines can be introduced

# Architecture notes
## Storage limits

| Final URL size | Practical judgment |
|---|---|
| **Under 2 KB** | Conservative choice for broadly shared links |
| **2–8 KB** | Comfortable target for a URL-driven app |
| **8–32 KB** | Reasonable for richer state; test intended browsers and sharing channels |
| **32–100 KB** | Specialized use; links become cumbersome |
| **Above 100 KB** | Technically possible, but I’d reconsider the storage model |

## Rust usage

We have two documents for Rust patterns to use. [[RUST_RICH_APP.md]] for complex desktop-parity applications and [[RUST_MINIFIED_WEB_APP.md]] for as-small-as possible in-application browser development patterns.