#!/usr/bin/env bash
# Sets the workspace version in Cargo.toml and brings Cargo.lock and
# standalone/Cargo.lock in line with it.
# Used by the Release workflow (every build job applies it to a fresh checkout, and the
# final job commits the result), and handy locally:
#
#   bash packaging/set-version.sh 0.2.0
#
# Requires perl (present on every GitHub runner and in Git Bash) and cargo. Deliberately
# avoids sed/awk range tricks, which differ between GNU (Linux, Git Bash) and BSD (macOS).
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <version>   (for example 0.2.0 or 0.2.0-beta.1)" >&2
  exit 2
fi
version=$1
if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "error: '$version' is not a version like 1.2.3 or 1.2.3-beta.1" >&2
  exit 2
fi

cd "$(dirname "$0")/.."

# Replace the first `version = "..."` inside [workspace.package].
VERSION="$version" perl -0pi -e \
  's/(\[workspace\.package\][^\[]*?\nversion = ")[^"]*(")/$1$ENV{VERSION}$2/' Cargo.toml

actual=$(perl -ne 'if (/^\[workspace\.package\]/) { $f = 1; next } if (/^\[/) { $f = 0 } if ($f && /^version = "([^"]*)"/) { print "$1\n"; exit }' Cargo.toml)
if [ "$actual" != "$version" ]; then
  echo "error: could not set [workspace.package] version in Cargo.toml (found '$actual')" >&2
  exit 1
fi

# Only the workspace's own crates change in Cargo.lock; no dependency is upgraded.
cargo update --workspace --quiet
# The standalone apps are a workspace of their own that depends on workspace crates,
# so their lockfile records the version too (CI checks it with --locked).
cargo update --manifest-path standalone/Cargo.toml --workspace --quiet
echo "workspace version is now $version"
