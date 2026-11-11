#!/bin/sh
set -eu
repo=PabloCloudX/mcjava-rust
release=v0.2.0-login-reject
asset=minecraft-rust-26.3-linux-x86_64
out=${1:-minecraft-rust}
arch=$(uname -m)
case "$arch" in
  x86_64|amd64) ;;
  *) echo "Unsupported published artifact architecture: $arch" >&2; exit 1 ;;
esac
url="https://github.com/$repo/releases/download/$release/$asset"
curl -fL --retry 2 --output "$out" "$url"
chmod +x "$out"
file "$out"
