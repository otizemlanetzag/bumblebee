# Bumblebee

Bumblebee is an original browser-engine project written in Rust. Its goal is a modular, memory-conscious implementation with broad web compatibility, developed without copying Chromium source code.

## Repository layout

- `JS/` — JavaScript engine and its Cargo package.
- `CSS/` — CSS parsing, style resolution, layout, and painting.
- `HTML/` — HTML, DOM, networking, browser APIs, and subsystem integration.
- `TESTS/` — cross-module, conformance, and performance test plans.
- `LICENSE` — project license.

All project files belong inside one of the four subsystem directories, except this README and the root LICENSE.

## Current milestone

The JavaScript crate starts with a dependency-free tokenizer, Pratt expression parser, a small value model, and a minimal runtime for literals, arithmetic, comparisons, logical operators, and `let`/`const` bindings. This is a foundation, not a complete ECMAScript implementation.

## Build and test

Run from the repository root:

```sh
cargo test --manifest-path JS/Cargo.toml
cargo run --manifest-path JS/Cargo.toml -- path/to/script.js
```

The current implementation is intentionally small and should be expanded against ECMAScript conformance tests before being described as browser-ready.
