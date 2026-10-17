# Test strategy

- Keep focused unit tests beside the Rust module they test.
- Put cross-module and conformance suites in this directory.
- Track ECMAScript conformance gaps explicitly; passing local tests is not a claim of full ECMAScript compatibility.
- Measure memory and runtime cost for representative scripts before optimizing.
- Every new feature should include regression tests; remove obsolete tests when the behavior or implementation is deliberately replaced.
