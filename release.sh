#!/bin/sh
set -eu -o pipefail

version="0.1.1"

git tag "v${version}"
git push

rustup target add aarch64-apple-darwin
rustup target add x86_64-apple-darwin

cargo build --target aarch64-apple-darwin --release
cargo build --target x86_64-apple-darwin --release

# From https://github.com/walles/riff/blob/82f77c82e7306dd69d343640670bdf9d31cc0b0b/release.sh#L132-L136
lipo -create \
  -output target/pgslice-apple-darwin \
  target/aarch64-apple-darwin/release/pgslice \
  target/x86_64-apple-darwin/release/pgslice

gh release create "v${version}" --title $version target/pgslice-apple-darwin