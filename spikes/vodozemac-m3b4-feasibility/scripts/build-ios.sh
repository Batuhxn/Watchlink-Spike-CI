#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail
spike_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "iOS build requires macOS/Xcode" >&2
  exit 2
fi
command -v xcodebuild >/dev/null
command -v xcrun >/dev/null
command -v rustup >/dev/null
test "$(xcodebuild -version | sed -n '1s/^Xcode //p')" = 26.3
cd "$spike_root/rust"
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
cargo build --release --locked
generated="$(mktemp -d)"
trap 'rm -rf "$generated"' EXIT
cargo run --release --locked --bin uniffi-bindgen -- generate \
  target/release/libwatchlink_vodozemac_feasibility.dylib \
  --library --language swift --out-dir "$generated" --no-format
python3 - "$generated" <<'PY'
from pathlib import Path
import sys

for path in Path(sys.argv[1]).iterdir():
    if path.suffix in (".swift", ".h", ".modulemap"):
        path.write_text("\n".join(line.rstrip() for line in path.read_text().splitlines()).rstrip("\n") + "\n")
PY
for name in watchlink_vodozemac_feasibility.swift watchlink_vodozemac_feasibilityFFI.h watchlink_vodozemac_feasibilityFFI.modulemap; do
  cmp "$generated/$name" "$spike_root/swift/generated/$name" || {
    echo "generated binding drift: $name" >&2
    exit 1
  }
done
cargo build --release --locked --target aarch64-apple-ios
cargo build --release --locked --target aarch64-apple-ios-sim
ios_lib="target/aarch64-apple-ios/release/libwatchlink_vodozemac_feasibility.a"
sim_lib="target/aarch64-apple-ios-sim/release/libwatchlink_vodozemac_feasibility.a"
headers="$generated/headers"
mkdir -p "$headers"
cp "$spike_root/swift/generated/watchlink_vodozemac_feasibilityFFI.h" "$headers/"
cp "$spike_root/swift/generated/watchlink_vodozemac_feasibilityFFI.modulemap" "$headers/module.modulemap"
artifact="$spike_root/swift/Artifacts/WatchlinkVodozemacFFI.xcframework"
mkdir -p "$(dirname "$artifact")"
rm -rf "$artifact"
xcodebuild -create-xcframework \
  -library "$ios_lib" -headers "$headers" \
  -library "$sim_lib" -headers "$headers" \
  -output "$artifact"
for library in "$ios_lib" "$sim_lib"; do
  file "$library"
  xcrun lipo -info "$library"
  test "$(xcrun lipo -archs "$library")" = arm64
done
python3 - "$artifact" <<'PY'
import pathlib
import plistlib
import sys

artifact = pathlib.Path(sys.argv[1])
with (artifact / "Info.plist").open("rb") as source:
    libraries = plistlib.load(source)["AvailableLibraries"]
platforms = {(entry["SupportedPlatform"], entry.get("SupportedPlatformVariant", "")) for entry in libraries}
assert platforms == {("ios", ""), ("ios", "simulator")}, platforms
for entry in libraries:
    assert entry["SupportedArchitectures"] == ["arm64"], entry
    folder = artifact / entry["LibraryIdentifier"]
    assert (folder / entry["LibraryPath"]).is_file()
    headers = folder / entry["HeadersPath"]
    for name in ("watchlink_vodozemac_feasibilityFFI.h", "module.modulemap"):
        assert (headers / name).is_file(), headers / name
    print(f"XCFramework slice: {entry['LibraryIdentifier']} arm64")
print("FFI module: watchlink_vodozemac_feasibilityFFI; generated bindings: MATCH")
PY
printf 'device static library bytes: '; wc -c < "$ios_lib"
printf 'simulator static library bytes: '; wc -c < "$sim_lib"
du -sh "$artifact"
find "$artifact" -maxdepth 2 -type f -print
