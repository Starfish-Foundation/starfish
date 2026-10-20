# BFish MCP

`bfish-mcp` is a Rust MCP server that gives any MCP-capable model read/write access
to a live Roblox Studio session. It is a fork of
[Chrrxs/robloxstudio-mcp](https://github.com/Chrrxs/robloxstudio-mcp), rewritten in
Rust and redesigned for broad model compatibility.

## Why the rewrite

The original server is excellent, but its schemas and documentation were tuned for
frontier Claude/GPT clients. `bfish-mcp` changes the integration surface:

- **Explicit JSON Schemas** with `required`, limits, enums, and examples.
- **Vendor-neutral descriptions**: no "Claude should..." phrasing.
- **Structured errors** with a concrete fix (`Studio is not connected`, missing key,
  invalid path, timeout, undo hint).
- **Consistent results**: `{ "ok": true, "data": ... }` or an MCP `isError` result.
- **Annotation hints** (`readOnlyHint`, `destructiveHint`) so clients can auto-approve
  reads and gate writes.
- **Two transports**: stdio for desktop clients, streamable HTTP at `/mcp`.
- **A connector-only Studio plugin** that is easy to audit and never stores keys.
- **23 focused tools** instead of a large, client-specific surface.

## Requirements

- Roblox Studio (recent version with plugin support)
- HTTP requests enabled in Studio (**Game Settings → Security → Allow HTTP Requests**)
- Rust 1.85+ only if building from source

## Install

### From the BFish app

The desktop app embeds this server. Use **Settings → Studio** to install the
connector; no separate binary is required.

### From source

```bash
git clone https://github.com/bfish-tools/bfish
cd bfish
cargo install --path crates/bfish-mcp --locked
bfish-mcp --install-plugin
```

The installer writes `BFishConnector.rbxmx` into the Roblox Plugins folder:

| OS | Path |
| --- | --- |
| Windows | `%LOCALAPPDATA%\Roblox\Plugins` |
| macOS | `~/Documents/Roblox/Plugins` |
| Linux | `~/.local/share/roblox/Plugins` (override with `BFISH_PLUGINS_DIR`) |

Restart Studio after installing or updating the connector.

## Connect a client

### Claude Desktop

Edit `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "bfish": {
      "command": "bfish-mcp",
      "args": ["--stdio"]
    }
  }
}
```

### Claude Code

```bash
claude mcp add bfish -- bfish-mcp --stdio
```

### Codex CLI

```bash
codex mcp add bfish -- bfish-mcp --stdio
```

### Cursor / Cline / Windsurf

Add an MCP server with command `bfish-mcp` and argument `--stdio`, or paste the
same `mcpServers` JSON into the client's MCP settings.

### Streamable HTTP

```bash
bfish-mcp --http
```

```json
{
  "mcpServers": {
    "bfish": {
      "url": "http://127.0.0.1:34871/mcp"
    }
  }
}
```

Some clients require `"type": "http"`. The endpoint is local-only.

## Usage

1. Start Studio and open **BFish Connector** from the Plugins tab. Wait for
   **Connected**.
2. Start your MCP client. It will spawn `bfish-mcp` over stdio.
3. Ask the model to inspect the place before editing, for example:

```text
Call studio_status, then get_file_tree on game.Workspace.
Create an anchored neon part named Beacon at (0, 8, 0) and verify it.
```

The MCP server owns the Studio bridge on `127.0.0.1:34871`. Only one process can
own that port: close the desktop app if you want to run the standalone server, or
point the standalone server at another port and update the connector URL.

## Tools

| Tool | Read-only | Purpose |
| --- | --- | --- |
| `studio_status` | yes | Connection, place, plugin/server versions |
| `get_place_info` | yes | PlaceId, GameId, Workspace name |
| `get_file_tree` | yes | Nested explorer tree with real paths |
| `get_services` | yes | Services in the place |
| `get_instance_children` | yes | Children of one instance |
| `get_instance_properties` | yes | Typed property values |
| `get_instance_attributes` | yes | Attributes with types |
| `set_properties` | no | Set one or more properties |
| `execute_luau` | no | Run Luau in edit mode |
| `search_instances` | yes | Search name/class/script content |
| `get_script_source` | yes | Read a script |
| `set_script_source` | no | Replace script source |
| `create_instance` | no | Create and parent an instance |
| `clone_instance` | no | Clone a model/UI template |
| `delete_instance` | no | Destroy an instance |
| `insert_asset` | no | Insert a public Creator Store asset |
| `get_selection` | yes | Current Studio selection |
| `set_selection` | yes | Change Studio selection |
| `get_console_output` | yes | Recent Output messages |
| `start_playtest` | no | Start play or run mode |
| `stop_playtest` | no | Return to edit mode |
| `undo` | no | Studio undo |
| `redo` | no | Studio redo |

Every mutating tool uses a `ChangeHistoryService` recording where Studio supports
it and returns the affected instance path.

## Paths

Paths are dot-separated from `game`:

```text
game.Workspace
game.Workspace.Map.Spawn
game.ReplicatedStorage.Remotes.BuyItem
game.Workspace["My Model"].Part
```

Never guess paths. Use `get_file_tree`, `get_services`, or `search_instances`.
Names containing dots, spaces, or punctuation must use bracket notation.

## Typed property values

Properties accept plain JSON and typed wrappers:

```json
{
  "Size": { "_type": "Vector3", "X": 12, "Y": 1, "Z": 12 },
  "Color": { "_type": "Color3", "R": 1, "G": 0.3, "B": 0.1 },
  "CFrame": {
    "_type": "CFrame",
    "Position": { "X": 0, "Y": 10, "Z": 0 },
    "Rotation": { "X": 0, "Y": 1.57, "Z": 0 }
  },
  "AnchorPoint": {
    "_type": "UDim2",
    "X": { "Scale": 0.5, "Offset": 0 },
    "Y": { "Scale": 0.5, "Offset": 0 }
  },
  "Material": { "_type": "EnumItem", "value": "Enum.Material.Neon" }
}
```

## `execute_luau`

- Runs synchronously in Studio's edit context.
- `print` and `warn` are captured and returned in `output`.
- The return value is serialized; tables become JSON.
- Changes are wrapped in an undo recording.
- The HTTP timeout does **not** kill the Luau thread. Avoid infinite loops.
- Prefer `set_script_source` for writing scripts and `create_instance` for
  individual objects; use Luau for bulk or generated work.

## CLI

```text
bfish-mcp [OPTIONS]

  --stdio              Speak MCP over stdin/stdout (default)
  --http               Serve MCP at http://127.0.0.1:<port>/mcp
  --port <PORT>        Studio bridge / HTTP port (default 34871)
  --install-plugin     Install the Studio connector and exit
  --no-bridge          Do not start the Studio bridge (stdio only)
  -h, --help           Show help
  -V, --version        Print the version
```

Environment:

- `BFISH_PLUGINS_DIR` — custom Roblox Plugins folder.
- `RUST_LOG` — e.g. `bfish_mcp=debug`. Logs go to stderr so stdout stays clean.

## Security

- The bridge binds to `127.0.0.1` only.
- The bridge sends no CORS headers, so a web page cannot call `/mcp` or the Studio endpoints cross-origin.
- The connector refuses non-localhost URLs.
- No authentication token is required because the bridge is loopback-only. This
  is a deliberate trade-off for local usability; do not expose the port with a
  proxy or tunnel.
- Tool results never include the contents of API keys.
- `delete_instance` refuses to destroy `game` or `Workspace`.

## Troubleshooting

See [troubleshooting.md](./troubleshooting.md).

## Attribution

The protocol design and the original tool surface come from
[robloxstudio-mcp](https://github.com/Chrrxs/robloxstudio-mcp) (MIT). This fork
keeps that repository as the `upstream` git remote. BFish is not affiliated with
Roblox Corporation.
