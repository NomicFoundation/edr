---
"@nomicfoundation/edr": patch
---

Routed `log` records into the `tracing` subscriber. Several EDR crates report unexpected failures that way, including an interval mine that produced no block, and those records previously had no destination at all. `RUST_LOG` controls what is shown, as before.
