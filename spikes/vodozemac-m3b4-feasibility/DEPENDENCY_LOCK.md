# M3B-4 dependency lock

This is an isolated experiment. `rust/Cargo.lock` pins every resolved crate;
all scripted Cargo invocations use `--locked`.

| Component | Pin |
|---|---|
| vodozemac | semantic version **0.11.0**, release commit **`db1b34820f3102307284e762f335b3f72c735bf0`**; Cargo Git `rev` and exact `version` both set |
| vodozemac upstream MSRV | Rust 1.96 |
| local Rust verification | `rustc 1.98.1 (48a229cea 2026-09-01)`, `cargo 1.98.1` |
| UniFFI | **0.32.2** (`uniffi`, CLI feature) |
| wrapper serialization | `serde_json = 1.0.151` |
| wrapper errors | `thiserror = 2.0.20` |
| disposable store | `rusqlite = 0.37.0`, bundled SQLite |

Direct upstream cryptography-related dependencies declared by vodozemac
0.11.0 are `aes 0.9.3`, `cipher 0.5.2`, `cbc 0.2.1`,
`chacha20poly1305 0.11.0`, `curve25519-dalek 5.0.0`,
`ed25519-dalek 3.0.0`, `getrandom 0.4.3`, `hkdf 0.13.0`,
`hmac 0.13.0`, `hpke 0.14.1`, `rand 0.10.2`, `sha2 0.11.0`,
`subtle 2.6.1`, `x25519-dalek 3.0.0`, and `zeroize 1.9.0`.
Those are manifest constraints, not all exact resolved versions; the
`rust/Cargo.lock` records resolution (for example `rand` resolves to 0.10.3).
The spike does not enable `experimental-session-config` or `low-level-api`.
It disables vodozemac default features to omit legacy libolm compatibility
and uses only the normal Olm v1 session configuration. Precomputed tables are
also thereby disabled; this affects build size/performance, not the protocol.

Upstream source: [vodozemac 0.11.0](https://github.com/matrix-org/vodozemac/tree/db1b34820f3102307284e762f335b3f72c735bf0).
