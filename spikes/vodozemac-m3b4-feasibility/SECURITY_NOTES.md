# Security and interpretation notes

**EXPERIMENTAL — NON-PRODUCTION — NO STACK SELECTED — NOT SECURITY-APPROVED.**

Only generated Alice/Bob accounts and synthetic `message-NNN` assertions are
used. No relay or production identity is involved. Rust `Mutex<Account>` and
`Mutex<Session>` serialize all mutable vodozemac calls behind UniFFI object
handles; Swift never obtains a Rust mutable reference. The bridge exposes
Olm account/session operations, not generic cryptographic primitives.

## Observed upstream behavior

- The 0.11.0 pin includes the contributory-X25519 check added in 0.10.0.
  An all-zero public input produces vodozemac's `SessionCreationError::NonContributoryKey`,
  mapped to the bridge's `BridgeError::NonContributoryKey`. There is no
  Watchlink-authored cryptographic check.
- Invalid pre-key ciphertext fails authentication before vodozemac removes
  the one-time key. The test then uses the same key successfully.
- Default `SessionConfig::version_1()` uses an eight-byte (64-bit) message
  HMAC. Upstream v2 uses a full MAC but is behind
  `experimental-session-config`; this spike does not enable it. Least
  Authority's 2022 Issue J is therefore still relevant.
- Skipped keys: 40 per receiver chain, maximum gap 2000, five receiver
  chains. An old unavailable key surfaces `MissingMessageKey`; a gap over
  2000 surfaces `TooBigMessageGap`. Successful forward decrypt does not
  separately report which old keys ceased to be available.
- The six-turn receiving-chain-eviction test returns bridge `InvalidMac` on
  the old ciphertext: once the old chain is gone, the library attempts a
  new-chain path and fails authentication. This is distinct from a missing
  skipped key on a retained chain, which returns `MissingMessageKey`.

## Persistence format

`Account::pickle()` and `Session::pickle()` return Serde-serializable structs
containing private account/ratchet material. The bridge's `serialize` and
`restore` use JSON **without encryption**; these bytes must be treated as
secrets. The disposable SQLite prototype stores synthetic session state only
and is not suitable for a real device or production messages.

Upstream also offers `AccountPickle::encrypt` and `SessionPickle::encrypt`,
both taking a 32-byte caller-supplied key and returning encoded ciphertext.
Source inspection shows they call `utilities::pickle`, whose
`Cipher::new_pickle` derives the AES key **and IV deterministically** from
that key; `encrypt_pickle` uses an eight-byte truncated MAC. Repeated saves
under one pickle key therefore reuse the IV. The upstream explicit warning
appears on the legacy libolm export method, but the current encrypted-pickle
path uses the same pickle cipher. This spike does **not** exercise repeated
encrypted saves or present that API as a safe persistence scheme. A reviewed,
upstream-supported at-rest format or platform storage design remains open.
[Current utilities](https://github.com/matrix-org/vodozemac/blob/db1b34820f3102307284e762f335b3f72c735bf0/src/utilities/mod.rs),
[pickle cipher](https://github.com/matrix-org/vodozemac/blob/db1b34820f3102307284e762f335b3f72c735bf0/src/cipher/mod.rs).

## Crash model limits

The independent SQLite prototype uses WAL and a transaction containing
`session_state`, `processed_inbox`, and `generation`. Its external generation
argument simulates a protected rollback anchor; it is **not** an implemented
Keychain/secure-enclave anchor. Failure after database commit and before an
external anchor update is intentionally detected as a mismatch and requires
recovery design. Fault tests model process restart by closing/reopening SQLite;
they do not establish power-loss durability, device file protection, or
production safety. A duplicate inbox ID is checked before decrypt.

The generated Swift bridge uses UniFFI's `@unchecked Sendable` object classes,
but actual mutable access is serialized in Rust. The generated code and
XCFramework must still be validated by the macOS CI build. No plaintext or
secret key values are logged by the bridge or tests.

The generated Swift `rustCallWithError` path maps declared bridge failures to
typed `BridgeError` values. Its unexpected Rust-call status (including a Rust
panic caught by UniFFI) becomes `UniffiInternalError.rustPanic`, with a fallback
for an empty error buffer. The spike exports one deliberately panicking test
probe. Swift XCTest requires the probe to throw an error containing its fixed
diagnostic; a passing run is the dynamic FFI panic-conversion check. The
diagnostic contains no secret. Swift XCTest also checks typed malformed-input
errors and concurrent calls through
shared account/session handles. A passing simulator test will show those calls
did not race in that run, while the Rust `Mutex` is the actual serialization
mechanism. Public identities and opaque serialized state are exposed; no
individual private-key getter or generic crypto primitive is exported. Opaque
serialized state still contains secrets and must not be logged or used in
production storage.
