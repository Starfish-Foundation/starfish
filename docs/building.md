# Building

## Prerequisites

- Rust 1.85+ (`rustup default stable`)
- Node.js 20+ and npm
- Roblox Studio for testing the connector
- Tauri platform prerequisites for the desktop shell
  (<https://tauri.app/start/prerequisites/>)

On Windows, PowerShell may block `npm.ps1`. Use `npm.cmd` explicitly if needed.

## Rust workspace

```bash
cargo check -p bfish-core -p bfish-mcp -p bfish-backend
cargo test -p bfish-core -p bfish-mcp -p bfish-backend
cargo fmt --all
cargo clippy -p bfish-core -p bfish-mcp -p bfish-backend -- -D warnings
```

Build release binaries:

```bash
cargo build --release -p bfish-backend -p bfish-mcp
```

Outputs:

- `target/release/bfish-backend`
- `target/release/bfish-mcp` (`.exe` on Windows)

## Desktop shell

The Tauri config lives in `crates/bfish-app/tauri.conf.json`.

```bash
cargo install tauri-cli --version "^2"
npm --prefix app install
cd crates/bfish-app
cargo tauri dev
```

`beforeDevCommand` starts Vite on port 5173. The Rust shell starts the backend on
port 8765.

For a release build:

```bash
cd crates/bfish-app
cargo tauri build
# or, without the Tauri CLI:
cargo build --release -p bfish-app --features custom-protocol
```

The `custom-protocol` feature embeds `app/dist` and makes Tauri load it instead
of `devUrl`. Without it, a plain `cargo build --release` still points at
`http://localhost:5173`.

Bundling is disabled by default. To produce installers:

1. Create an icon source (1024×1024 PNG).
2. Run `cargo tauri icon ../../assets/icon.png`.
3. Set `bundle.active` to `true` in `tauri.conf.json`.
4. Run `cargo tauri build`.

## Studio connector

No Rojo required:

```bash
node plugin/build.mjs
# plugin/dist/BFishConnector.rbxmx
```

With Rojo:

```bash
rojo build plugin/default.project.json -o plugin/dist/BFishConnector.rbxmx
```

The MCP binary embeds `plugin/src/init.server.lua` at compile time. Rebuild
`bfish-mcp` after editing the plugin source if you use `--install-plugin`.

## Website

```bash
npm --prefix website install
npm --prefix website run dev     # http://localhost:4173
npm --prefix website run build   # website/dist
```

Deploy `website/dist` to any static host. The docs view uses `#docs`, so no server
rewrites are required.

## Full local check

```powershell
scripts\build.ps1
```

or:

```bash
cargo fmt --all --check
cargo check -p bfish-core -p bfish-mcp -p bfish-backend
node plugin/build.mjs
npm --prefix app run build
npm --prefix website run build
```

## Project conventions

- Rust: 2021 edition, `anyhow` for application errors, `serde` camelCase at the API
  boundary, snake_case in MCP tool arguments.
- Svelte: runes (`$state`, `$derived`, `$props`), scoped styles, callbacks instead of
  deprecated `createEventDispatcher`.
- Plugin: single-file Luau, no external packages, all UI built with `Instance.new`.
- Every mutating Studio operation should be undoable.
