#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Fail closed on repository contents before a public spike CI run."""
import pathlib
import re
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[3]
SPIKE = "spikes/vodozemac-m3b4-feasibility/"
EXPECTED = {
    ".github/workflows/m3b4-vodozemac-feasibility.yml",
    "README.md",
    "LICENSE",
    SPIKE + ".gitignore",
    SPIKE + "CI_PLAN.md",
    SPIKE + "DEPENDENCY_LOCK.md",
    SPIKE + "README.md",
    SPIKE + "SECURITY_NOTES.md",
    SPIKE + "UPSTREAM_NOTICES.md",
    SPIKE + "licenses/MPL-2.0.txt",
    SPIKE + "licenses/vodozemac-APACHE-2.0.txt",
    SPIKE + "rust/Cargo.lock",
    SPIKE + "rust/Cargo.toml",
    SPIKE + "rust/src/bin/uniffi-bindgen.rs",
    SPIKE + "rust/src/lib.rs",
    SPIKE + "rust/src/storage.rs",
    SPIKE + "rust/tests/feasibility.rs",
    SPIKE + "scripts/build-ios.sh",
    SPIKE + "scripts/check-standalone.py",
    SPIKE + "scripts/check-xctest-log.py",
    SPIKE + "scripts/pick-simulator.py",
    SPIKE + "scripts/test-ios.sh",
    SPIKE + "swift/Package.swift",
    SPIKE + "swift/Tests/VodozemacBridgeTests.swift",
    SPIKE + "swift/generated/watchlink_vodozemac_feasibility.swift",
    SPIKE + "swift/generated/watchlink_vodozemac_feasibilityFFI.h",
    SPIKE + "swift/generated/watchlink_vodozemac_feasibilityFFI.modulemap",
}
FORBIDDEN_SUFFIXES = (".p8", ".p12", ".mobileprovision", ".cer", ".key", ".age", ".ipa")
RULES = {
    "private IPv4": re.compile(r"\b(?:10(?:\.\d{1,3}){3}|192\.168(?:\.\d{1,3}){2}|172\.(?:1[6-9]|2\d|3[01])(?:\.\d{1,3}){2})\b"),
    "email address": re.compile(r"\b[\w.+-]+@[\w.-]+\.[A-Za-z]{2,}\b"),
    "UDID-like identifier": re.compile(r"\b(?:[0-9A-Fa-f]{8}-[0-9A-Fa-f]{16}|[0-9A-Fa-f]{8}(?:-[0-9A-Fa-f]{4}){3}-[0-9A-Fa-f]{12})\b"),
    "labelled legacy UDID": re.compile(r"\bUDID\s*[:=]\s*[0-9A-Fa-f]{40}\b", re.I),
    "PEM key or certificate": re.compile("-" * 5 + r"BEGIN [A-Z ]*(?:PRIVATE KEY|CERTIFICATE)"),
    "age secret key": re.compile("AGE" + "-SECRET-KEY-1"),
    "Apple team setting": re.compile("APPLE" + "_TEAM_ID"),
    "Xcode team setting": re.compile(r"\bDEVELOPMENT_TEAM\s*[:=]\s*[A-Z0-9]+"),
    "Apple ID setting": re.compile(r"\b(?:APPLE_ID|FASTLANE_USER)\s*[:=]\s*[^\s,]+"),
    "App Store Connect setting": re.compile("ASC" + r"_[A-Z0-9_]+"),
    "API credential assignment": re.compile(r"\b(?:API[_-]?KEY|SECRET[_-]?KEY|ACCESS[_-]?TOKEN)\s*[:=]\s*['\"][^'\"]+", re.I),
    "home-directory path": re.compile("/" + "home" + r"/[A-Za-z0-9_.-]+"),
    "private repository URL": re.compile(r"(?:https?://|git@)github[.]com[/:][^/\s]+/(?:Vault|Vault-CI)(?:[./\s]|$)", re.I),
}


def git_files(*arguments):
    result = subprocess.check_output(["git", "ls-files", "-z", *arguments], cwd=ROOT)
    return {item.decode() for item in result.split(b"\0") if item}


def main():
    tracked = git_files("--cached")
    extra = git_files("--others", "--exclude-standard")
    if tracked != EXPECTED or extra:
        raise SystemExit(f"standalone allowlist mismatch: missing={sorted(EXPECTED - tracked)}, "
                         f"unexpected={sorted((tracked - EXPECTED) | extra)}")
    for rel in sorted(tracked):
        path = ROOT / rel
        if path.is_symlink() or not path.is_file():
            raise SystemExit(f"not a regular file: {rel}")
        if rel.lower().endswith(FORBIDDEN_SUFFIXES):
            raise SystemExit(f"forbidden file type: {rel}")
        try:
            content = path.read_text(encoding="utf-8")
        except UnicodeError:
            raise SystemExit(f"non-UTF-8 file: {rel}") from None
        for label, pattern in RULES.items():
            if pattern.search(content):
                raise SystemExit(f"{rel}: {label}")
        if path.suffix in {".rs", ".swift", ".h"} and re.search(r"AGPL|libsignal", content, re.I):
            raise SystemExit(f"{rel}: earlier M3B source/license marker")
    print(f"standalone allowlist and public-safety scan: PASS ({len(tracked)} tracked text files)")


if __name__ == "__main__":
    main()
