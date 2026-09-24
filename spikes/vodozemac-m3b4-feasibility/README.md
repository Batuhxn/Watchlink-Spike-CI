# M3B-4: raw vodozemac iOS feasibility

**EXPERIMENTAL — NON-PRODUCTION — NO STACK SELECTED — NOT SECURITY-APPROVED.**

This is a separate, test-only Rust/UniFFI/Swift package. It does not import
Vault-App, contact a relay, use production identities, implement attachments,
or become a production dependency. Only upstream Olm account and session
behavior is exercised. No AGPL-labelled M3B source was copied.

## Local verification

On Linux, `cd rust && cargo test --locked` runs the pure Rust suite. It covers
basic and 60-message bidirectional conversation, serialization/restoration,
low-order rejection, historical audit Issue G, skipped keys and >2000 gap,
replay, independent SQLite fault boundaries, and concurrent object calls.
`python3 scripts/check-standalone.py` proves the exact standalone file allowlist
and scans every tracked text file for prohibited public content.

On a macOS host with Xcode and Rust, run `scripts/build-ios.sh` for device and
simulator static libraries plus an XCFramework. The manual workflow runs
`scripts/test-ios.sh compile` under Xcode 26.3 and `scripts/test-ios.sh run`
with an available Xcode 26.2 simulator. These scripts have **not** run on the Linux host.
No manual Xcode project editing is needed.

The generated UniFFI surface is in `swift/generated`: one Swift file, one C
header, one module map. The Rust `Mutex` within each object serializes
mutating calls. The XCFramework is generated under `swift/Artifacts/` and
ignored by Git. `DEPENDENCY_LOCK.md` records the exact source pin and direct
crypto-related manifest dependencies. `SECURITY_NOTES.md` records the MAC,
pickle, replay, and crash-model limits.

## Current results and limits

The Linux Rust suite passed locally. This is bridge/session evidence only;
it does not establish an acceptable E2EE stack, identity binding, safe
production persistence, attachment protocol, or App Store readiness.
The current encrypted-pickle API should not be used for repeated saves with
one key; the prototype deliberately stores plaintext opaque pickle bytes in
disposable SQLite. There is no production migration path in this spike.
