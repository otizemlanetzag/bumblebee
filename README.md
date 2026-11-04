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

The JavaScript subsystem now embeds the QuickJS engine through the Rust `rquickjs` bindings. Bumblebee delegates ECMAScript parsing, execution, built-ins, objects, functions, promises, and garbage collection to QuickJS instead of maintaining a separate partial JavaScript implementation. The wrapper keeps a persistent global context and applies a default 256 MiB JavaScript heap limit.

QuickJS compiles scripts to its own internal bytecode during loading/evaluation. This is not native-code AOT compilation and it is not a JIT engine. The old experimental in-repository parser, interpreter, and bytecode compiler are no longer part of the active build. Browser APIs such as DOM, Fetch, and timers still need to be implemented by Bumblebee and exposed to JavaScript.

## Build and test

Run from the repository root:

```sh
cargo test --manifest-path JS/Cargo.toml
cargo run --manifest-path JS/Cargo.toml -- path/to/script.js\n# Run the currently supported AOT bytecode subset\ncargo run --manifest-path JS/Cargo.toml -- --aot path/to/script.js
```

The embedded engine provides the JavaScript language runtime. Bumblebee must still implement browser-facing APIs and integration layers (DOM, HTML elements, Fetch, timers, rendering hooks, and navigation).
