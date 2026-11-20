#!/bin/sh
# Compile the standalone Wasm mod against Nova's matching Horizon API.
set -eu
mod_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
build_dir=$(mktemp -d)
trap 'rm -rf "$build_dir"' EXIT
rev=f3cd51d53a82bb8ae6932793a75e61b39960fad6
sdk="$build_dir/sdk"
git init -q "$sdk"
git -C "$sdk" fetch --quiet --depth=1 https://github.com/kipd0/nova-ios.git "$rev"
git -C "$sdk" checkout -q --detach FETCH_HEAD
mkdir "$sdk/boatcam"
cp -R "$mod_dir/src" "$mod_dir/assets" "$mod_dir/Cargo.toml" "$mod_dir/Cargo.lock" "$mod_dir/horizon-mod.toml" "$sdk/boatcam/"
cd "$sdk/boatcam"
version=$(cat "$sdk/tools/ci/rust-version")
rustup toolchain install "$version" --profile minimal --component rust-src --target wasm32-unknown-unknown
export RUSTUP_TOOLCHAIN="$version"
cargo build --manifest-path "$sdk/Cargo.toml" -p cargo-horizon-mod
"$sdk/target/debug/cargo-horizon-mod" build --wasm
cp target/horizon-mod/boatcam.hmod "$mod_dir/boatcam.hmod"
echo "Built $mod_dir/boatcam.hmod"
