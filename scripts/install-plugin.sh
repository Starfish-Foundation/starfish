#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if command -v bfish-mcp >/dev/null 2>&1; then
  bfish-mcp --install-plugin
  exit 0
fi

node "$ROOT/plugin/build.mjs"
if [[ "$(uname -s)" == "Darwin" ]]; then
  TARGET="$HOME/Documents/Roblox/Plugins"
else
  TARGET="${BFISH_PLUGINS_DIR:-$HOME/.local/share/roblox/Plugins}"
fi
mkdir -p "$TARGET"
cp "$ROOT/plugin/dist/BFishConnector.rbxmx" "$TARGET/"
echo "Installed to $TARGET"
echo "Restart Roblox Studio, then open BFish Connector from the Plugins tab."
