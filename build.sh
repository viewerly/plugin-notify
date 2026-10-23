#!/usr/bin/env bash
# Builds the automation for wasm32-wasip2, puts it beside the manifest as notify.wasm, and packs
# dist/<id>-<version>.vplugin: a zip with viewerly-plugin.toml at its root.
set -euo pipefail
root="$(cd "$(dirname "$0")" && pwd)"
cd "$root"

rustup target add wasm32-wasip2 >/dev/null 2>&1 || true
cargo build --release --target wasm32-wasip2
target="${CARGO_TARGET_DIR:-$root/target}"
cp "$target/wasm32-wasip2/release/viewerly_notify.wasm" "$root/notify.wasm"

# Viewerly installs only components, not core modules: the header's layer field is 1.
header="$(head -c 8 "notify.wasm" | od -An -tx1 | tr -d ' \n')"
if [ "${header:0:8}" != "0061736d" ] || [ "${header:12:4}" != "0100" ]; then
  echo "notify.wasm is not a WASM component" >&2
  exit 1
fi

field() { sed -n "s/^$1 = \"\(.*\)\"$/\1/p" viewerly-plugin.toml | head -n 1; }
id="$(field id)"
version="$(field version)"
mkdir -p dist
file="dist/$id-$version.vplugin"
rm -f "$file"
zip -qXD "$file" viewerly-plugin.toml ui.toml notify.wasm README.md LICENSE
echo "$root/$file ($(wc -c < "$file" | tr -d ' ') bytes)"
