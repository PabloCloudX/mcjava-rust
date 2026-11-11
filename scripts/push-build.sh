#!/bin/sh
set -eu
remote=$(git remote get-url origin)
case "$remote" in
  https://github.com/PabloCloudX/mcjava-rust.git|git@github.com:PabloCloudX/mcjava-rust.git) ;;
  *) echo "Unexpected origin remote: $remote" >&2; exit 1 ;;
esac
git status --short
if [ -n "$(git status --porcelain)" ]; then
  echo "Commit or discard changes before pushing." >&2
  exit 1
fi
git push origin main
echo "Pushed to $remote"
echo "Actions: https://github.com/PabloCloudX/mcjava-rust/actions"
