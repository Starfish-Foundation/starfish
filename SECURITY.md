# Security

BFish is a local developer tool that can edit Roblox places. Treat it accordingly.

## Threat model

BFish assumes:

- the local user account is trusted
- the loopback interface is not exposed to other machines
- model providers are trusted with the prompts and tool results you send them

BFish does **not** assume model output is safe. Every tool validates paths and
arguments, and destructive operations are gated by the system prompt and Studio's
undo history.

## Local ports

| Port | Service | Binding |
| --- | --- | --- |
| 8765 | App API + SSE | `127.0.0.1` |
| 34871 | MCP HTTP + Studio bridge | `127.0.0.1` |

Neither port uses authentication. The security boundary is loopback binding and the
CORS allowlist on the app API. The Studio bridge sends no CORS headers at all, so a
web page cannot call its `/mcp` or `/studio` endpoints cross-origin. Do not:

- expose either port through a reverse proxy, ngrok, or container port mapping
- run BFish on a shared multi-user machine without OS-level isolation
- change the bind address to `0.0.0.0`

## API keys

Provider keys are stored unencrypted in `<config>/BFish/settings.json` (mode `0600`
on Unix where possible). They are sent only to the selected provider's base URL.

- Do not commit `settings.json`, session files, or `.env` files.
- Do not paste keys into bug reports.
- Revoke a leaked key at the provider immediately.
- A custom base URL receives your key. Only point it at a gateway you trust.

## Studio data

Place contents are sent to the configured model provider as tool results when the
model asks for them. BFish does not upload place data to any BFish-operated server.
Review your provider's data-retention policy before using BFish on private projects.

## Destructive tools

`delete_instance`, `set_properties`, `set_script_source`, `execute_luau`,
`insert_asset`, `clone_instance`, `create_instance`, `undo`, and `redo` can change the
place. MCP clients receive `destructiveHint: true` for these tools. The default
system prompt requires explicit confirmation before large deletions.

`delete_instance` refuses to destroy `game` or `Workspace`. `insert_asset` strips
scripts from inserted assets.

## Studio bridge trust

The plugin accepts commands from anything that can reach `127.0.0.1:34871`. Any
local process running as your user can therefore drive Studio through BFish. If that
is not acceptable, run BFish in a dedicated OS account or VM.

A future version may add a per-install token exchanged through `plugin:SetSetting`.
It is not present in v0.1.

## Reporting

Open a private security advisory on GitHub or email the maintainers listed in the
repository profile. Do not file a public issue with exploit details or real keys.
