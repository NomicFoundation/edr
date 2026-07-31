#!/usr/bin/env bash
#
# Serve a directory of flamegraphs over HTTP so they can be opened from the host
# browser.
#
# Why this is needed: in this devcontainer /workspaces is a Docker *named volume*
# inside the Linux VM (/dev/vda1[/docker/volumes/...], ext4), not a bind mount
# from the host, and /tmp is container-only. So there is no host path to point a
# `file://` URL at. Serving over HTTP and letting VS Code forward the port is the
# reliable route.
#
# VS Code auto-detects the listening port and forwards it; open the
# http://localhost:<port> it offers, or find it under the Ports panel.
#
# Usage:
#   ./serve.sh [dir] [port]
#
# Default dir is the harness directory itself, where render.sh writes.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIR="${1:-$HERE}"
PORT="${2:-8080}"

if [[ ! -d "$DIR" ]]; then
  echo "error: $DIR does not exist" >&2
  exit 1
fi

DIR="$(cd "$DIR" && pwd)"

echo "serving $DIR on http://localhost:$PORT"
echo
echo "contents:"
find "$DIR" -maxdepth 1 -type f -name '*.svg' -printf '  %-40f %8s bytes\n' 2>/dev/null \
  | sort || ls -la "$DIR"
echo
echo "VS Code should offer to forward port $PORT. If it does not, open the Ports"
echo "panel and add it manually. Ctrl-C to stop."
echo

exec python3 -m http.server "$PORT" --directory "$DIR" --bind 127.0.0.1
