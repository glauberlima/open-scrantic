#!/usr/bin/env bash
# Usage: set-version.sh vX.Y.Z
# Writes the version of a release tag into Cargo.toml and Cargo.lock, so that binaries built before the
# release commit already carry it. Run from the repository root.
set -euo pipefail

TAG="${1:?usage: set-version.sh vX.Y.Z}"
case "$TAG" in v[0-9]*.[0-9]*.[0-9]*) ;; *) echo "Unexpected tag '$TAG'" >&2; exit 1 ;; esac
export VERSION="${TAG#v}"

# perl rather than sed: the same command works with GNU and BSD userlands and on the Windows runner.
perl -0pi.bak -e 's/^version = "[^"]*"/version = "$ENV{VERSION}"/m' Cargo.toml
perl -0pi.bak -e 's/(name = "open-scrantic"\r?\nversion = ")[^"]*/$1$ENV{VERSION}/' Cargo.lock
rm -f Cargo.toml.bak Cargo.lock.bak

# Fail loudly if a pattern stopped matching.
grep -q "^version = \"$VERSION\"" Cargo.toml || { echo "Cargo.toml was not updated" >&2; exit 1; }
perl -0ne 'exit(!/name = "open-scrantic"\r?\nversion = "\Q$ENV{VERSION}\E"/)' Cargo.lock || {
  echo "Cargo.lock was not updated" >&2
  exit 1
}
