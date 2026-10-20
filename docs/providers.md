# Providers

BFish normalizes six vendors into one internal chat format. Tool calls, tool
results, and streaming text work across all of them.

## Quick comparison

| Provider | API | Streaming | Tool calls | Key required |
| --- | --- | --- | --- | --- |
| OpenAI | Chat Completions | SSE | native | yes |
| Claude | Messages | SSE | native | yes |
| Gemini | `streamGenerateContent` | SSE | native | yes |
| xAI | OpenAI-compatible | SSE | native | yes |
| OpenRouter | OpenAI-compatible | SSE | depends on model | yes |
| Ollama | OpenAI-compatible `/v1` | SSE | depends on model | no |

## OpenAI

- Base URL: `https://api.openai.com/v1`
- Default model: `gpt-4.1`
- Key: <https://platform.openai.com/api-keys>
- Reasoning models (`o1`, `o3`, `o4`, `gpt-5`) automatically omit `temperature`
  and use `max_completion_tokens`.
- Tool calling uses the standard `tools` + `tool_calls` format.

## Claude (Anthropic)

- Base URL: `https://api.anthropic.com/v1`
- Default model: `claude-sonnet-4-5`
- Key: <https://console.anthropic.com/settings/keys>
- Uses the Messages API with `x-api-key` and `anthropic-version: 2023-06-01`.
- Tool results are sent as `tool_result` content blocks; consecutive results are
  merged into one user turn.
- `max_tokens` is required by Anthropic and is taken from the app's max-token setting.

## Gemini

- Base URL: `https://generativelanguage.googleapis.com/v1beta`
- Default model: `gemini-2.5-pro`
- Key: <https://aistudio.google.com/app/apikey>
- Uses `streamGenerateContent?alt=sse`.
- Gemini function calls do not carry ids, so BFish synthesizes stable ids and maps
  tool results back by function name.
- Thought parts (`"thought": true`) are not shown in the chat.

## xAI

- Base URL: `https://api.x.ai/v1`
- Default model: `grok-4`
- Key: <https://console.x.ai>
- OpenAI-compatible request/response shape.

## OpenRouter

- Base URL: `https://openrouter.ai/api/v1`
- Default model: `anthropic/claude-sonnet-4.5`
- Key: <https://openrouter.ai/keys>
- Sends `HTTP-Referer` and `X-Title` attribution headers.
- Tool-calling support depends on the routed model. Use a model with the tools tag.

## Ollama

- Base URL: `http://127.0.0.1:11434/v1`
- Default model: `qwen3:8b`
- No key required.
- Start it before chatting:
  ```bash
  ollama serve
  ollama pull qwen3:8b
  ```
- Local models are more likely to ignore tool instructions. BFish's explicit tool
  descriptions and `studio_status` guidance mitigate this, but a 7–8B model is not a
  frontier model. Prefer a tool-capable model and keep requests concrete.

## How a request flows

1. The backend loads the session and prepends the system prompt + live Studio context.
2. `bfish-core` converts messages and tool specs into the provider dialect.
3. The provider streams text and/or tool-call deltas.
4. The backend executes each tool through the MCP engine and appends tool results.
5. The loop repeats (max 12 rounds) until the model answers without tool calls.

## Adding a provider

1. Add a preset in `crates/bfish-core/src/config.rs`.
2. If the provider is OpenAI-compatible, add its id to `is_openai_compatible` in
   `crates/bfish-core/src/provider/mod.rs`.
3. Otherwise implement `stream` and `list_models` in a new adapter and register it
   in `stream_chat` / `list_models`.
4. Add a metadata row to the UI's provider list only if it is not returned by
   `/api/providers` (the UI already reads presets from the backend).

## Error behavior

Provider errors are surfaced verbatim but trimmed:

```
OpenAI API error (401 Unauthorized): Incorrect API key provided...
```

Missing keys produce an actionable message pointing at the key page. Network errors
mention the provider and suggest checking the base URL.
