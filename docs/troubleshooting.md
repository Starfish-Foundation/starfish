# Troubleshooting

## The app says it cannot reach the backend

The UI talks to `http://127.0.0.1:8765`.

- Start it manually: `cargo run -p bfish-backend`
- Check the port: `Get-NetTCPConnection -LocalPort 8765` (Windows) or
  `lsof -i :8765`.
- If `BFISH_PORT` is set, the UI must use the same port. Set `VITE_BFISH_API` when
  building the app, e.g. `VITE_BFISH_API=http://127.0.0.1:9000 npm --prefix app run build`.
- In Tauri, the shell starts the backend automatically. If startup fails, the app
  still loads and points at an existing backend.

## “Port 34871 is already in use”

Only one process can own the Studio bridge. The desktop app and a standalone
`bfish-mcp` cannot both use the default port.

- Close the other process, or
- run the standalone server on a different port:
  ```bash
  bfish-mcp --stdio --port 34872
  ```
  and set the connector URL to `http://127.0.0.1:34872`.

## Studio says “Not connected”

1. Is the BFish app (or standalone MCP server) running?
2. Does the connector URL match the server port?
3. Press **Reconnect** in the connector.
4. Check the bridge:
   ```bash
   curl http://127.0.0.1:34871/studio/status
   ```
5. Fully restart Studio after installing or updating the plugin.

## “HTTP requests are not enabled”

Studio blocks plugin HTTP until you allow it:

**Game Settings → Security → Allow HTTP Requests**, then reconnect.

## The plugin does not appear

- Windows path: `%LOCALAPPDATA%\Roblox\Plugins\BFishConnector.rbxmx`
- macOS: `~/Documents/Roblox/Plugins/BFishConnector.rbxmx`
- Linux: `~/.local/share/roblox/Plugins/BFishConnector.rbxmx` or set
  `BFISH_PLUGINS_DIR`.
- Confirm the file exists and is not zero bytes.
- Restart Studio completely (not just the place).

## “Unknown session”

The backend restarted while Studio stayed open. The connector should re-hello within
a couple of seconds. If it does not, press **Reconnect**. No tool call is silently
replayed; retry the request.

## A tool times out

- `execute_luau` may still be running in Studio even after the bridge timeout.
- Check the Output window and the connector status before retrying.
- Infinite loops cannot be killed by the HTTP timeout. Undo and fix the code.
- Increase the timeout in the tool call (`timeout_seconds`, max 300) for heavy work.

## The model ignores tools

- Use a tool-calling model. Some OpenRouter routes and small Ollama models do not
  support tools reliably.
- Ask explicitly: “Call studio_status first, then get_file_tree.”
- Check the tool card in the chat: if no card appears, the provider returned plain
  text instead of a tool call.
- For Ollama, try a larger model such as `qwen3:14b` or a tools-tagged model.

## Provider errors

Use **Settings → Providers → Detect**.

| Error | Likely cause |
| --- | --- |
| 401 / 403 | Wrong or expired key |
| 404 | Wrong base URL or model name |
| 429 | Rate limit / no credit |
| `model does not support tools` | Pick a different model |
| Connection refused (Ollama) | Ollama is not running |

## Gemini returns empty text

Some Gemini responses contain only function calls or “thought” parts. BFish hides
thoughts; if the model only calls tools, you will see tool cards instead of prose.
Ask it to summarize the result after the tool calls.

## Claude stops after a tool call

Anthropic requires every `tool_use` block to have a matching `tool_result`. BFish
builds that automatically. If you see a validation error, cancel the chat and retry;
the session may contain a partially saved turn from an aborted request.

## Chat text is lost after Stop

BFish saves partial text when cancellation arrives before the provider finishes.
If the process is killed, only previously saved turns remain.

## API keys

Keys are in `settings.json`, unencrypted. Do not commit the BFish config folder, and
do not paste keys into issues. If a key leaks, revoke it at the provider.

## Windows: npm script execution is disabled

PowerShell may refuse `npm.ps1`. Use `npm.cmd`:

```powershell
npm.cmd --prefix app run build
```

## Tauri build fails

- Ensure the front end was built at least once (`app/dist` exists).
- On Linux install WebKitGTK and related Tauri dependencies.
- `bundle.active = false` avoids installer tooling; `cargo build -p bfish-app`
  should still work after `npm --prefix app run build`.
- Tauri expects `frontendDist` relative to `crates/bfish-app/`; it points at
  `../../app/dist`.

## Logs

- Backend: `RUST_LOG=debug cargo run -p bfish-backend`
- MCP: logs go to stderr, so stdio clients keep working.
- Plugin: the connector shows the last event and buffers up to 300 Output messages,
  available through `get_console_output`.
