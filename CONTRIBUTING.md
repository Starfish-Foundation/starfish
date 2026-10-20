# Contributing

Thanks for helping BFish. This project is intentionally small; keep changes focused.

## Before you start

- Open an issue for new tools, protocol changes, or large UI work.
- Do not add a provider SDK when a small adapter will do.
- Keep the connector single-file and dependency-free.
- Every Studio mutation must be undoable.

## Development

```bash
cargo check -p bfish-core -p bfish-mcp -p bfish-backend
cargo test -p bfish-core -p bfish-mcp -p bfish-backend
cargo fmt --all
node plugin/build.mjs
npm --prefix app run build
npm --prefix website run build
```

## Style

### Rust

- Edition 2021, `rustfmt` defaults.
- `anyhow` for fallible paths at the edges; structured errors where callers need to
  branch.
- API structs use `serde(rename_all = "camelCase")`; MCP tool arguments stay
  snake_case for broad model compatibility.
- Never log API keys or full session contents.

### Svelte

- Svelte 5 runes only.
- Keep components accessible: buttons for actions, labels for inputs, keyboard
  handlers for interactive non-buttons.
- Colors come from the CSS variables in `app/src/app.css`.

### Luau

- One file: `plugin/src/init.server.lua`.
- Use `pcall` around every Studio API that can fail.
- Record mutations before changing instances.
- Never read BFish settings or keys.

## Adding a tool

1. Add the spec in `crates/bfish-mcp/src/tools.rs` with a clear description, path
   example, return shape, and annotations.
2. Add the binding in `crates/bfish-mcp/src/engine.rs` with argument validation.
3. Add the endpoint to the plugin and document it in `docs/plugin.md` and
   `docs/mcp.md`.
4. Test with a real Studio session and a non-Claude model when possible.

## Commits

- Small, descriptive commits.
- Mention the affected crate/plugin/UI in the subject when useful.
- Do not commit generated `dist/`, `target/`, or `node_modules/`.

## License

By contributing, you agree that your changes are licensed under the MIT License.
