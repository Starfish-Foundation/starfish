# Architecture

BFish is a Rust workspace with a Svelte front end. The desktop app embeds the MCP
engine directly, so there is one process to manage and one Studio bridge.

## Crates

```
bfish-core ──────────────┐
  providers, prompts,    │
  settings, sessions     │
                         ▼
bfish-mcp ───────────▶ bfish-backend ───────────▶ bfish-app (Tauri)
  MCP transports         HTTP API, agent loop       desktop shell
  Studio bridge          SSE streaming
  tools + schemas
```

- **bfish-core** has no dependency on the app or MCP. It owns the common message
  model and provider adapters.
- **bfish-mcp** depends on core for prompts/versioning. It exposes an `Engine` that
  can be embedded or run as a binary.
- **bfish-backend** depends on both. It is a plain Axum server, so it can run
  headless or inside Tauri.
- **bfish-app** is a thin Tauri shell that starts the backend on a background thread.

## Chat loop

`bfish-backend::agent::run_chat`:

1. Load the session and append the user message.
2. Save immediately (so a crash cannot lose the prompt).
3. Build the system prompt and append live Studio context.
4. Snapshot available tools from `Engine::tools()`.
5. Call the provider and stream events:
   - `Text` → SSE `delta` to the UI
   - `ToolCall` → collect
   - `Done(usage)` → accumulate
6. If there are no tool calls, save the assistant message and emit `done`.
7. Otherwise:
   - save the assistant message with tool calls
   - execute each tool through the MCP engine
   - emit `tool_start` / `tool_result`
   - append tool results to the conversation
   - loop (max 12 rounds)
8. Cancellation is a `CancellationToken` plus the SSE sender closing.

Tool results are clipped at 80k characters before being sent back to the model, with
a visible truncation note. The UI receives a 24k preview.

## Studio bridge

`bfish_mcp::bridge` is a small axum server on `127.0.0.1:34871`.

State:

- up to four recent Studio sessions (place, versions, last seen)
- one command queue
- pending `oneshot` channels keyed by command id
- a ring buffer of plugin logs

A tool call:

1. Picks the most recently active session (error if none/stale).
2. Pushes a command and waits up to the tool timeout.
3. The plugin's long-poll receives it, executes it, and POSTs the result.
4. The bridge resolves the pending channel.

The poll handler checks the queue every 150 ms and returns `204` after the wait
window. This avoids notification races and keeps the plugin implementation simple.

## MCP transports

`bfish_mcp::mcp::Engine::handle_json` implements JSON-RPC:

- `initialize` with protocol version negotiation (2024-11-05 → 2025-06-18)
- `tools/list`, `tools/call`
- `prompts/list`, `prompts/get`
- `resources/list`, `resources/read` (`bfish://status`, `bfish://tools`)
- `ping`, `logging/setLevel`

Stdio reads newline-delimited JSON from stdin and writes only JSON to stdout; logs go
to stderr. HTTP uses `POST /mcp` (single or batch). Notifications return `202`.

## Data model

The common message format is provider-neutral:

```rust
ChatMessage { role, content, tool_calls, tool_call_id, name }
```

Adapters translate this to OpenAI `tool_calls`, Anthropic `tool_use`/`tool_result`
blocks, or Gemini `functionCall`/`functionResponse` parts.

## Storage

Plain JSON under the BFish config directory:

- `settings.json` — providers, keys, prompt, temperature, max tokens, bridge port
- `sessions/<id>.json` — title, provider, model, timestamps, message list

Writes go through a temp file + rename. On Unix, permissions are set to `0600`.

## Security boundaries

1. **Provider APIs** — HTTPS, keys sent only to the selected vendor.
2. **Backend** — binds `127.0.0.1:8765`; CORS restricted to localhost/Tauri origins.
3. **Studio bridge** — binds `127.0.0.1:34871`; no network exposure.
4. **Plugin** — outgoing requests only, localhost URLs only.
5. **Model output** — treated as untrusted input; tools validate paths and arguments.

There is no authentication token on either loopback port. This is a local developer
tool; do not expose these ports through a proxy, tunnel, or container port mapping.

## Fork process

The repository started as a clone of `Chrrxs/robloxstudio-mcp`:

```bash
git remote rename origin upstream
git switch -c bfish-rust
# remove TypeScript packages, add the Rust workspace
git commit -m "BFish: Rust rewrite"
```

To publish your own fork:

```bash
git remote add origin https://github.com/<your-account>/bfish.git
git push -u origin bfish-rust
```

Upstream history remains available for diffs and attribution. The MCP API surface is
intentionally compatible in spirit, not byte-for-byte: tool names are snake_case and
results are uniform.

## Known trade-offs

- The plugin has no screenshots or input injection; adding them would require a much
  larger capture pipeline.
- Playtest APIs vary by Studio version; start/stop is best-effort.
- One command in flight per Studio session keeps mutations ordered but limits
  throughput. That is deliberate.
- Keys are stored unencrypted. OS keychain integration is a future task.
