# Bumblebee

Bumblebee is an independent browser-engine project written in Rust.

It is intentionally designed as an engine, not a wrapper around Chromium, WebKit, or an existing browser widget.

## Engine architecture

- **Networking** — HTTP/HTTPS navigation, TLS through Rustls, compression, redirects and user-agent handling.
- **HTML** — HTML5 parsing and document metadata.
- **DOM foundation** — document representation and traversal.
- **CSS foundation** — style values, display/position, lengths, box edges and basic color parsing.
- **Style system** — computed-style foundation ready for selector matching and cascading.
- **Layout** — viewport and block-flow primitives.
- **Painting** — display-list style paint commands.
- **Rendering pipeline** — style → layout → paint pipeline boundary.
- **JavaScript boundary** — runtime trait so a real JS engine can be integrated without coupling the browser core to one implementation.
- **Storage** — origin-keyed persistent-state abstraction.
- **HTTP cache** — bounded-by-TTL in-memory response cache.
- **Security policy** — navigation scheme and insecure-HTTP policy boundary.

## Roadmap to a production browser

1. Complete DOM mutation/events and Web APIs.
2. CSS tokenizer, selectors, cascade, inheritance and computed values.
3. Full layout: block, inline, flex, grid, tables, positioned elements and scrolling.
4. Text shaping, fonts, images, SVG and media.
5. GPU-backed compositing and a real raster backend.
6. JavaScript runtime integration and the browser event loop.
7. Fetch, cookies, cache validation, service workers and storage quotas.
8. Same-origin policy, CORS, CSP, permissions and process isolation.
9. Accessibility tree and input/event routing.
10. Tabs, navigation history, downloads, bookmarks and browser UI.
11. Web-platform tests and conformance testing.

Bumblebee is deliberately being built in layers so every subsystem can become real rather than being represented by a fake “full browser” API.
