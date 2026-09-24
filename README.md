# Watchlink M3B-4A isolated CI spike

This temporary repository contains only a non-production vodozemac feasibility
spike and a manual CI workflow. It does not select an E2EE stack or contain the
Watchlink app, relay, signing assets, or production configuration.

The workflow runs a Rust host suite on Ubuntu and an unsigned iOS/Swift XCTest
proof on a macOS runner. It accepts `workflow_dispatch` only. The standalone
allowlist and content scan run in both jobs.

For local verification, run from the repository root:

```sh
python3 spikes/vodozemac-m3b4-feasibility/scripts/check-standalone.py
cd spikes/vodozemac-m3b4-feasibility/rust
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

Watchlink-authored spike code is licensed under [Apache-2.0](LICENSE). The
upstream dependency and generated-binding notices are recorded in
[UPSTREAM_NOTICES.md](spikes/vodozemac-m3b4-feasibility/UPSTREAM_NOTICES.md).
The upstream licenses remain in force for their respective material.

A successful simulator run would be feasibility evidence only. The known MAC,
persistence, replay, and identity-binding limits are documented in the spike.
