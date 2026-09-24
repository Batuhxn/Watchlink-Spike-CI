# Upstream dependency and generated-binding notices

The root Apache-2.0 license applies to Watchlink-authored spike code. It does
not replace the licenses of upstream material.

- **vodozemac 0.11.0**, pinned to commit
  `db1b34820f3102307284e762f335b3f72c735bf0`, is an Apache-2.0
  dependency. Its upstream license text is preserved verbatim at
  `licenses/vodozemac-APACHE-2.0.txt`. No vodozemac source is vendored here.
- **UniFFI 0.32.2** is MPL-2.0. The generated Swift binding and C header are
  retained with their generated comments. The MPL-2.0 text is preserved at
  `licenses/MPL-2.0.txt`. The generated files are not relicensed by the root
  Apache-2.0 license.
- Other Rust crates are fetched by Cargo at their locked versions. Their
  source packages, including their own notices and licenses, are not vendored
  in this repository. `rust/Cargo.lock` records the precise dependency graph.

The exact direct dependencies and security-relevant configuration are listed
in `DEPENDENCY_LOCK.md`.
