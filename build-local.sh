#!/bin/sh
# Build BoatCam using an existing checkout of the matching Nova repository.
# Example: sh build-local.sh "$HOME/minecraft/nova-ios"
set -eu
if [ "$#" -ne 1 ]; then
  echo 'Usage: sh build-local.sh PATH_TO_NOVA_IOS_REPO' >&2
  exit 2
fi
mod_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
sdk=$(CDPATH='' cd -- "$1" && pwd)
if [ ! -f "$sdk/tools/ci/rust-version" ] || [ ! -d "$sdk/crates/horizon-mod-api" ]; then
  echo "Not a compatible Nova/Horizon checkout: $sdk" >&2
  exit 2
fi
build_dir=$(mktemp -d "$sdk/.boatcam-build.XXXXXX")
trap 'rm -rf "$build_dir"' EXIT
cp -R "$mod_dir/src" "$mod_dir/assets" "$mod_dir/Cargo.toml" "$mod_dir/Cargo.lock" "$mod_dir/horizon-mod.toml" "$build_dir/"
version=$(cat "$sdk/tools/ci/rust-version")
rustup toolchain install "$version" --profile minimal --component rust-src --target wasm32-unknown-unknown
export RUSTUP_TOOLCHAIN="$version"
# Run dependency-free camera math regressions before compiling the Wasm mod.
rustc --edition=2024 --test "$build_dir/src/motion.rs" -o "$build_dir/motion-tests"
"$build_dir/motion-tests"
cargo build --manifest-path "$sdk/Cargo.toml" -p cargo-horizon-mod
cd "$build_dir"
"$sdk/target/debug/cargo-horizon-mod" build --wasm
cp "$build_dir/target/horizon-mod/boatcam.hmod" "$mod_dir/boatcam.hmod"
echo "Built $mod_dir/boatcam.hmod"
