#!/usr/bin/env sh
set -eu

case "$(uname -s):$(uname -m)" in
  Darwin:arm64) target="macos-arm64" ;;
  Linux:x86_64) target="linux-x86_64" ;;
  *)
    printf '%s\n' 'Epistesys has no packaged binary for this platform' >&2
    exit 2
    ;;
esac

binary="$PLUGIN_ROOT/scripts/lc631/bin/$target/lc631"
if [ ! -x "$binary" ]; then
  printf '%s\n' "Epistesys packaged binary is unavailable for $target" >&2
  exit 2
fi
exec "$binary" lc631-host-stop-hook
