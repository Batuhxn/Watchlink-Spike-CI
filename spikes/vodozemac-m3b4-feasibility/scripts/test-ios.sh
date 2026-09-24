#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail
spike_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mode="${1:?expected compile or run}"
derived_data="${M3B4_DERIVED_DATA:-$spike_root/build/DerivedData}"
cd "$spike_root/swift"

resolve_scheme() {
  mkdir -p "$derived_data"
  local schemes_file="$derived_data/xcode-schemes.json"
  if ! xcodebuild -list -json > "$schemes_file"; then
    echo "XCTest scheme resolution failed: xcodebuild -list -json" >&2
    return 1
  fi
  python3 - "$schemes_file" <<'PY'
import json
import pathlib
import sys

data = json.loads(pathlib.Path(sys.argv[1]).read_text())
schemes = []
for key in ("workspace", "project"):
    item = data.get(key)
    if isinstance(item, dict):
        schemes.extend(item.get("schemes", []))
if not all(isinstance(name, str) for name in schemes):
    sys.exit("XCTest scheme resolution failed: malformed Xcode scheme list")
print("Exposed Xcode schemes: " + ", ".join(sorted(schemes)), file=sys.stderr)
accepted = [name for name in schemes if name in {
    "VodozemacM3B4Feasibility", "VodozemacBridge"
}]
if len(accepted) != 1:
    sys.exit(f"XCTest scheme resolution failed: expected exactly one package/product scheme; found {accepted}")
print(accepted[0])
PY
}

case "$mode" in
  compile)
    test "$(xcodebuild -version | sed -n '1s/^Xcode //p')" = 26.3
    scheme="$(resolve_scheme)"
    echo "Resolved XCTest scheme: $scheme"
    printf '%s\n' "$scheme" > "$derived_data/resolved-scheme.txt"
    xcodebuild build-for-testing \
      -scheme "$scheme" \
      -destination 'generic/platform=iOS Simulator' \
      -derivedDataPath "$derived_data" CODE_SIGNING_ALLOWED=NO
    ;;
  run)
    test "$(xcodebuild -version | sed -n '1s/^Xcode //p')" = 26.2
    test -n "${M3B4_SIMULATOR_ID:-}"
    scheme="$(resolve_scheme)"
    echo "Resolved XCTest scheme: $scheme"
    read -r compiled_scheme < "$derived_data/resolved-scheme.txt"
    if [[ "$scheme" != "$compiled_scheme" ]]; then
      echo "XCTest scheme resolution failed: compile/run scheme mismatch" >&2
      exit 1
    fi
    log="${M3B4_TEST_LOG:-$spike_root/build/xctest.log}"
    mkdir -p "$(dirname "$log")"
    set +e
    xcodebuild test \
      -scheme "$scheme" \
      -destination "platform=iOS Simulator,id=$M3B4_SIMULATOR_ID" \
      -derivedDataPath "$derived_data" CODE_SIGNING_ALLOWED=NO 2>&1 | tee "$log"
    test_status="${PIPESTATUS[0]}"
    set -e
    python3 "$spike_root/scripts/check-xctest-log.py" "$log"
    test "$test_status" -eq 0
    ;;
  *) echo "expected compile or run" >&2; exit 2 ;;
esac
