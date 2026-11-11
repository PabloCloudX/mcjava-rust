#!/bin/sh
set -eu
binary=${1:-./minecraft-rust}
address=${2:-127.0.0.1:25565}
if [ ! -x "$binary" ]; then
  echo "Executable not found: $binary (run scripts/download-build.sh first)" >&2
  exit 1
fi
exec "$binary" "$address"
