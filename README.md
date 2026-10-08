# Bumblebee

Bumblebee is a browser engine written in Rust.

## Current engine layers

- HTTP/HTTPS navigation with Rustls
- Redirect handling
- HTML5 parsing
- DOM tree traversal
- Page metadata extraction
- Modular engine architecture

## Roadmap

1. DOM APIs
2. CSS tokenizer and style system
3. Layout tree
4. Text and box layout
5. Painting and compositing
6. JavaScript runtime integration
7. Browser networking policies and storage
8. Tabs, history, cache, cookies and permissions
9. Native desktop UI

The goal is a real browser engine built incrementally in Rust, rather than a wrapper around an existing browser engine.
