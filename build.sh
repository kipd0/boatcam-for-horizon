#!/bin/sh
# Compile the standalone Wasm mod against Nova's matching Horizon API.
set -eu
mod_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
build_dir=$(mktemp -d)
trap 'rm -rf "$build_dir"' EXIT
rev=1934f13b7992f123bbf5e200dbca8836d9cc3779
sdk="$build_dir/sdk"
git init -q "$sdk"
git -C "$sdk" fetch --quiet --depth=1 https://github.com/kipd0/nova-ios.git "$rev"
git -C "$sdk" checkout -q --detach FETCH_HEAD
sh "$mod_dir/build-local.sh" "$sdk"
