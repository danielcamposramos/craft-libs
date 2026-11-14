#!/usr/bin/env bash
# Run transferred backend tests with the complete local patch set. Test copies
# keep Cargo's standalone lock resolution from modifying the vendored source.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
test_root=$(mktemp -d "${TMPDIR:-/tmp}/craft-pdf-backends.XXXXXX")
trap 'rm -rf "$test_root"' EXIT
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target}"
patches=()
for backend in hayro hayro-interpret hayro-syntax hayro-jbig2 hayro-cmap hayro-postscript vello_cpu vello_common; do
  mkdir -p "$test_root/vendor"
  cp -R "$root/vendor/$backend" "$test_root/vendor/$backend"
  patches+=(--config "patch.crates-io.$backend.path=\"$test_root/vendor/$backend\"")
done
mkdir -p "$test_root/examples/assets"
cp -R "$root/examples/assets/roboto" "$test_root/examples/assets/roboto"
for backend in hayro-syntax hayro-interpret hayro hayro-jbig2 hayro-cmap hayro-postscript vello_common vello_cpu; do
  cargo test --manifest-path "$test_root/vendor/$backend/Cargo.toml" --lib "${patches[@]}"
done
for backend in vello_common vello_cpu; do
  cargo test --manifest-path "$test_root/vendor/$backend/Cargo.toml" --lib --all-features "${patches[@]}"
done
cargo test --manifest-path "$test_root/vendor/hayro-syntax/Cargo.toml" --lib --features unsafe "${patches[@]}"
cargo test --manifest-path "$test_root/vendor/hayro-syntax/Cargo.toml" --doc "${patches[@]}"
cargo test --manifest-path "$test_root/vendor/hayro-syntax/Cargo.toml" --lib --no-default-features --features std "${patches[@]}"
cp -R "$root/vendor/linesweeper" "$test_root/linesweeper"
cargo test --manifest-path "$test_root/linesweeper/Cargo.toml" --locked --lib --test regression
cargo test --manifest-path "$test_root/linesweeper/Cargo.toml" --locked --doc
