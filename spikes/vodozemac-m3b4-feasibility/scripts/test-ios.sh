#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
set -euo pipefail
spike_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mode="${1:?expected compile or run}"
derived_data="${M3B4_DERIVED_DATA:-$spike_root/build/DerivedData}"
cd "$spike_root/swift"
case "$mode" in
  compile)
    test "$(xcodebuild -version | sed -n '1s/^Xcode //p')" = 26.3
    xcodebuild build-for-testing \
      -scheme VodozemacM3B4Feasibility-Package \
      -destination 'generic/platform=iOS Simulator' \
      -derivedDataPath "$derived_data" CODE_SIGNING_ALLOWED=NO
    ;;
  run)
    test "$(xcodebuild -version | sed -n '1s/^Xcode //p')" = 26.2
    test -n "${M3B4_SIMULATOR_ID:-}"
    log="${M3B4_TEST_LOG:-$spike_root/build/xctest.log}"
    mkdir -p "$(dirname "$log")"
    set +e
    xcodebuild test \
      -scheme VodozemacM3B4Feasibility-Package \
      -destination "platform=iOS Simulator,id=$M3B4_SIMULATOR_ID" \
      -derivedDataPath "$derived_data" CODE_SIGNING_ALLOWED=NO 2>&1 | tee "$log"
    test_status="${PIPESTATUS[0]}"
    set -e
    python3 "$spike_root/scripts/check-xctest-log.py" "$log"
    test "$test_status" -eq 0
    ;;
  *) echo "expected compile or run" >&2; exit 2 ;;
esac
