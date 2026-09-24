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
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
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
cargo build --release --locked --target x86_64-apple-ios
ios_lib="target/aarch64-apple-ios/release/libwatchlink_vodozemac_feasibility.a"
arm64_sim_lib="target/aarch64-apple-ios-sim/release/libwatchlink_vodozemac_feasibility.a"
x86_64_sim_lib="target/x86_64-apple-ios/release/libwatchlink_vodozemac_feasibility.a"
sim_lib="$generated/libwatchlink_vodozemac_feasibility.a"
check_archs() {
  local library="$1" expected="$2" actual
  file "$library"
  xcrun lipo -info "$library"
  actual="$(xcrun lipo -archs "$library" | tr ' ' '\n' | LC_ALL=C sort | paste -sd ' ' -)"
  if [[ "$actual" != "$expected" ]]; then
    echo "unexpected architectures in $library: $actual (expected $expected)" >&2
    exit 1
  fi
}
check_archs "$ios_lib" arm64
check_archs "$arm64_sim_lib" arm64
check_archs "$x86_64_sim_lib" x86_64
xcrun lipo -create "$arm64_sim_lib" "$x86_64_sim_lib" -output "$sim_lib"
check_archs "$sim_lib" 'arm64 x86_64'
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
python3 - "$artifact" <<'PY'
import pathlib
import plistlib
import subprocess
import sys

artifact = pathlib.Path(sys.argv[1])
with (artifact / "Info.plist").open("rb") as source:
    libraries = plistlib.load(source)["AvailableLibraries"]
platforms = {(entry["SupportedPlatform"], entry.get("SupportedPlatformVariant", "")) for entry in libraries}
assert platforms == {("ios", ""), ("ios", "simulator")}, platforms
for entry in libraries:
    platform = (entry["SupportedPlatform"], entry.get("SupportedPlatformVariant", ""))
    expected = {"arm64"} if platform == ("ios", "") else {"arm64", "x86_64"}
    assert set(entry["SupportedArchitectures"]) == expected, entry
    assert len(entry["SupportedArchitectures"]) == len(expected), entry
    folder = artifact / entry["LibraryIdentifier"]
    library = folder / entry["LibraryPath"]
    assert library.is_file(), library
    subprocess.run(["file", str(library)], check=True)
    subprocess.run(["xcrun", "lipo", "-info", str(library)], check=True)
    actual = set(subprocess.check_output(["xcrun", "lipo", "-archs", str(library)], text=True).split())
    assert actual == expected, (library, actual, expected)
    headers = folder / entry["HeadersPath"]
    for name in ("watchlink_vodozemac_feasibilityFFI.h", "module.modulemap"):
        assert (headers / name).is_file(), headers / name
    print(f"XCFramework slice: {entry['LibraryIdentifier']} {', '.join(sorted(actual))}")
print("FFI module: watchlink_vodozemac_feasibilityFFI; generated bindings: MATCH")
PY
printf 'device static library bytes: '; wc -c < "$ios_lib"
printf 'arm64 simulator thin library bytes: '; wc -c < "$arm64_sim_lib"
printf 'x86_64 simulator thin library bytes: '; wc -c < "$x86_64_sim_lib"
printf 'universal simulator library bytes: '; wc -c < "$sim_lib"
du -sh "$artifact"
find "$artifact" -maxdepth 2 -type f -print
