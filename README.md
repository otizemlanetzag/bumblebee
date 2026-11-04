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

The JavaScript crate includes a tokenizer, Pratt parser, runtime, and the first Ahead-of-Time (AOT) compiler stage. AOT compiles supported expressions to portable stack bytecode before execution; it is not JIT and does not generate native machine code yet. The current AOT subset covers literals, arithmetic, bitwise operations, shifts, and comparisons. Unsupported syntax fails explicitly. The legacy interpreter remains temporarily available while more syntax is lowered to bytecode.

## Build and test

Run from the repository root:

```sh
cargo test --manifest-path JS/Cargo.toml
cargo run --manifest-path JS/Cargo.toml -- path/to/script.js\n# Run the currently supported AOT bytecode subset\ncargo run --manifest-path JS/Cargo.toml -- --aot path/to/script.js
```

The current implementation is intentionally small and should be expanded against ECMAScript conformance tests before being described as browser-ready.
