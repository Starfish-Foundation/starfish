# Desktop app

BFish is a Tauri v2 shell around a local Svelte UI and a Rust backend. Everything
except model API calls stays on your machine.

## Layout

- `app/` — Svelte 5 + Vite UI
- `crates/bfish-app/` — Tauri shell; starts the backend on launch
- `crates/bfish-backend/` — Axum API + agent loop at `127.0.0.1:8765`

## First run

1. Start the backend (the Tauri app does this automatically):
   ```bash
   cargo run -p bfish-backend
   ```
2. Start the UI:
   ```bash
   npm --prefix app run dev
   ```
   Or run the desktop shell:
   ```bash
   cd crates/bfish-app
   cargo tauri dev
   ```
3. Open **Settings → Providers**, pick a vendor, paste a key, and press **Test**.
4. Open **Settings → Studio** and install the connector.
5. Restart Roblox Studio and open **BFish Connector** from the Plugins tab.

## UI tour

### Sessions

Every chat is a JSON file in your local BFish config folder. Sessions are listed by
last update, grouped into Today / Yesterday / Previous 7 days / Older. Hover a
session for rename and delete actions.

### Chat

- Streaming text from the selected provider.
- Tool cards show the tool name, arguments, result, and success/failure.
- The top bar switches provider and model for the current chat.
- Enter sends, Shift+Enter inserts a newline.
- The stop button cancels generation and keeps partial text when possible.

### Providers

Six vendors are built in:

| Vendor | Default base URL | Default model |
| --- | --- | --- |
| OpenAI | `https://api.openai.com/v1` | `gpt-4.1` |
| Claude | `https://api.anthropic.com/v1` | `claude-sonnet-4-5` |
| Gemini | `https://generativelanguage.googleapis.com/v1beta` | `gemini-2.5-pro` |
| xAI | `https://api.x.ai/v1` | `grok-4` |
| OpenRouter | `https://openrouter.ai/api/v1` | `anthropic/claude-sonnet-4.5` |
| Ollama | `http://127.0.0.1:11434/v1` | `qwen3:8b` |

Base URLs and model names are editable. **Test** calls the provider's model-list
endpoint; **Refresh models** in the top bar is not needed because the picker uses
the model lists returned by the test.

Keys are saved in `settings.json` in the BFish config folder. They are not encrypted.
See [SECURITY.md](../SECURITY.md).

### System prompt

**Settings → General** edits the global system prompt. Presets (Default, Builder,
Debugger, Reviewer) are provided. Each chat request appends live Studio context:

- connected place name and PlaceId, or
- an explicit “Studio is NOT connected” warning.

This prevents the model from claiming it edited a place when no connector is open.

### Studio

**Settings → Studio** shows:

- connection status and place name
- plugin install/update button
- MCP endpoint (`http://127.0.0.1:34871/mcp`)
- a ready-to-copy MCP client config

The connector must be restarted after an install or update.

## Backend API

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/api/health` | Liveness + Studio state |
| GET | `/api/status` | Version, provider, bridge status |
| GET/PUT | `/api/settings` | Read/save settings |
| GET | `/api/providers` | Provider presets and saved config |
| POST | `/api/providers/test` | Validate key / list models |
| GET | `/api/providers/models` | Models for a saved provider |
| GET | `/api/prompts` | Default prompt + presets |
| GET/POST | `/api/sessions` | List/create sessions |
| GET/PATCH/DELETE | `/api/sessions/{id}` | Read/update/delete a session |
| POST | `/api/chat` | SSE chat stream |
| POST | `/api/chat/stop` | Cancel a running chat |
| GET | `/api/mcp/status` | Bridge status + logs |
| POST | `/api/mcp/install-plugin` | Install the Studio connector |

CORS allows only localhost/tauri origins. The API binds to `127.0.0.1`.

## Storage

```
<config>/BFish/
├── settings.json
└── sessions/
    └── <id>.json
```

`settings.json` holds provider keys, base URLs, models, temperature, max tokens,
system prompt, and the Studio bridge port.

## Building a desktop installer

Tauri bundling is disabled in `tauri.conf.json` (`bundle.active = false`) so CI can
build the binary without platform-specific packaging. To build installers:

1. Generate icons: `cd crates/bfish-app && cargo tauri icon ../../assets/icon.png`
2. Set `"bundle": { "active": true, ... }`.
3. Run `cd crates/bfish-app && cargo tauri build`.

Platform prerequisites are listed in the Tauri documentation.
