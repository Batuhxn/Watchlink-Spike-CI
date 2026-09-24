# M3B-4A standalone manual CI proof

The only workflow is `.github/workflows/m3b4-vodozemac-feasibility.yml`. It
runs on `workflow_dispatch`, grants `contents: read`, uses no secrets or signing,
and builds no production target. The repository and the workflow contain only
the isolated feasibility spike; no E2EE stack is selected.

The Ubuntu job checks the standalone file allowlist and public-safety scan,
installs Rust 1.98.1, verifies the vodozemac and UniFFI lock pins, then runs
format, nine host tests, and Clippy. It repeats the allowlist scan afterward.

The dependent `macos-26` job checks the same allowlist, selects Xcode 26.3,
builds arm64 iOS device and simulator libraries, compares whitespace-normalized
regenerated UniFFI bindings against checked-in files, builds an XCFramework,
and checks slices, headers, module map, sizes, and architectures. It compiles
the isolated Swift XCTest package under Xcode 26.3.

For execution, the job selects a preinstalled Xcode 26.2 iPhone simulator,
creating a disposable device only if needed. It requires at least eight
executed XCTest cases with zero failures, deletes any disposable device,
restores Xcode 26.3, and repeats the standalone scan even if a step fails. It
does not download a simulator runtime.

The XCFramework remains on the runner and is not uploaded. CI logs report
architectures, binary sizes, binding match, and test summaries. A small log
guard rejects obvious key material and missing or failed XCTest summaries;
it does not prove arbitrary future tool output is secret-free.

No iOS build, Swift compile, simulator run, or FFI panic runtime result can be
claimed from Linux preparation. The macOS run is required for those results.
