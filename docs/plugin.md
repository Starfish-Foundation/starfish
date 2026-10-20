# Studio connector

`BFishConnector.rbxmx` is a single Roblox Studio plugin script. It is a connector,
not a chat client.

## What it shows

- Connection status (Connected / Connecting / Not connected)
- Server URL (editable, localhost only)
- Place name, PlaceId
- Plugin version, Studio version
- Session id and the last connection event

There is no AI chat, no model picker, and no API-key storage in Studio.

## Install

### App

**Settings → Studio → Install / update connector**

### CLI

```bash
bfish-mcp --install-plugin
```

### Manual

```bash
node plugin/build.mjs
```

Copy `plugin/dist/BFishConnector.rbxmx` to:

- Windows: `%LOCALAPPDATA%\Roblox\Plugins\`
- macOS: `~/Documents/Roblox/Plugins/`
- Linux: `~/.local/share/roblox/Plugins/`

Then restart Studio. The plugin appears under the **Plugins** tab and opens a dock
widget automatically. A toolbar button named **Connector** toggles the dock.

## How it talks to BFish

Roblox plugins cannot host a server, so the plugin uses outgoing HTTP long-polling:

```
plugin ── POST /studio/hello ──────────────▶ bridge
plugin ── GET  /studio/poll?session=… ─────▶ bridge   (25s long poll)
plugin ◀─ { id, endpoint, data } ─────────── bridge
plugin ── executes handler ──
plugin ── POST /studio/result ──────────────▶ bridge
plugin ── POST /studio/log ─────────────────▶ bridge
```

The bridge returns `204 No Content` when no work arrives, and `404` when the
server has restarted; the plugin then re-registers automatically.

There is one command in flight per session. This preserves mutation order (create →
set properties → read back) and avoids overlapping undo recordings.

## Endpoints implemented by the plugin

| Endpoint | Handler |
| --- | --- |
| `/api/ping` | Plugin, place, Studio version |
| `/api/place-info` | PlaceId, GameId, Workspace |
| `/api/services` | Top-level services |
| `/api/file-tree` | Nested explorer tree |
| `/api/children` | Children with optional depth |
| `/api/properties` | Typed properties |
| `/api/attributes` | Attributes |
| `/api/set-properties` | Property mutation |
| `/api/execute-luau` | Luau execution + output capture |
| `/api/search` | Name/class/content search |
| `/api/script-source` | Read a script |
| `/api/set-script-source` | Write a script |
| `/api/create-instance` | Create + parent |
| `/api/clone-instance` | Clone |
| `/api/delete-instance` | Destroy |
| `/api/insert-asset` | InsertService asset, scripts stripped |
| `/api/selection` | Read selection |
| `/api/set-selection` | Change selection |
| `/api/logs` | Recent Output messages |
| `/api/start-playtest` | StudioTestService play/run |
| `/api/stop-playtest` | End test |
| `/api/undo`, `/api/redo` | ChangeHistoryService |

## Undo

Mutations are wrapped in `ChangeHistoryService:TryBeginRecording` /
`FinishRecording(Commit)`. On older Studio versions the plugin falls back to
`SetWaypoint`. Deleting an instance and writing a script are both undoable.

`execute_luau` is also wrapped, but code that creates its own instances outside the
recording may not be perfectly grouped in Studio's history.

## Security model

- Only `http://127.0.0.1`, `http://localhost`, and `[::1]` URLs are accepted.
- No inbound ports are opened in Studio.
- The plugin never reads BFish settings or API keys.
- `insert_asset` strips scripts from the loaded asset before parenting it.
- `delete_instance` refuses `game` and `Workspace`.
- The server binds to loopback; do not proxy or tunnel port `34871`.

There is no shared token in v0.1. Loopback binding and the absence of CORS headers
are the security boundary. Any local process running as your user can still drive
Studio through the bridge; if you need isolation, use a separate OS account or VM.

## Developing the plugin

```bash
node plugin/build.mjs
# or with Rojo:
rojo build plugin/default.project.json -o plugin/dist/BFishConnector.rbxmx
```

Reload the plugin with **Plugins → Manage Plugins**, or restart Studio. The plugin
uses `plugin:GetSetting`/`SetSetting` to remember the server URL.

## Troubleshooting

- **HTTP requests are not enabled**: enable it in Game Settings → Security.
- **Unknown session**: the backend restarted; the plugin reconnects automatically.
- **Port in use**: if another BFish server owns it, the app and stdio server reuse
  it automatically. Use a different port only when the port is owned by a different
  application.
- **Plugin does not appear**: check the exact Plugins folder, then fully restart Studio.
