<script lang="ts">
  let { onback }: { onback: () => void } = $props();

  const toolGroups = [
    {
      label: 'Inspect',
      tools: [
        ['studio_status', 'Connection, place, and plugin versions'],
        ['get_place_info', 'PlaceId, GameId, workspace name'],
        ['get_file_tree', 'Nested explorer tree with paths'],
        ['get_services', 'Services present in the place'],
        ['get_instance_children', 'Children of one instance'],
        ['get_instance_properties', 'Property values, typed for JSON'],
        ['get_instance_attributes', 'Attributes and their types'],
        ['search_instances', 'Name, class, or script-content search'],
        ['get_script_source', 'Full source of a script'],
        ['get_selection', 'Current Studio selection'],
        ['get_console_output', 'Recent Output-window messages'],
      ],
    },
    {
      label: 'Edit',
      tools: [
        ['execute_luau', 'Run Luau in edit mode, capture output'],
        ['set_properties', 'Set properties in one undo recording'],
        ['set_script_source', 'Replace a script source'],
        ['create_instance', 'Create and parent any Instance'],
        ['clone_instance', 'Clone a model or UI template'],
        ['delete_instance', 'Destroy an instance (undoable)'],
        ['insert_asset', 'Insert a Creator Store asset'],
        ['set_selection', 'Select what BFish is working on'],
      ],
    },
    {
      label: 'Test & control',
      tools: [
        ['start_playtest', 'Start play or run mode'],
        ['stop_playtest', 'Return Studio to edit mode'],
        ['undo', 'Undo the latest change'],
        ['redo', 'Redo the latest undone change'],
      ],
    },
  ];

  const clients = [
    {
      name: 'Claude Desktop',
      file: 'Claude Desktop config',
      snippet: `{
  "mcpServers": {
    "bfish": {
      "command": "bfish-mcp",
      "args": ["--stdio"]
    }
  }
}`,
    },
    {
      name: 'Cursor / Cline / Windsurf',
      file: 'MCP settings',
      snippet: `{
  "mcpServers": {
    "bfish": {
      "command": "bfish-mcp",
      "args": ["--stdio"]
    }
  }
}`,
    },
    {
      name: 'Codex CLI',
      file: 'Terminal',
      snippet: `codex mcp add bfish -- bfish-mcp --stdio`,
    },
    {
      name: 'Claude Code',
      file: 'Terminal',
      snippet: `claude mcp add bfish -- bfish-mcp --stdio`,
    },
    {
      name: 'Streamable HTTP',
      file: 'Clients that support remote MCP',
      snippet: `{
  "mcpServers": {
    "bfish": {
      "url": "http://127.0.0.1:34871/mcp"
    }
  }
}`,
    },
  ];
</script>

<div class="docs">
  <aside class="docs-nav">
    <button class="back" onclick={onback}>← Home</button>
    <div class="docs-nav-label">bfish-mcp</div>
    <a href="#docs-quickstart">Quickstart</a>
    <a href="#docs-clients">MCP clients</a>
    <a href="#docs-studio">Studio connector</a>
    <a href="#docs-tools">Tool reference</a>
    <a href="#docs-paths">Paths & safety</a>
    <a href="#docs-cli">CLI & config</a>
    <a href="#docs-troubleshooting">Troubleshooting</a>
    <a href="#docs-architecture">Architecture</a>
  </aside>

  <article class="docs-body">
    <header class="docs-header">
      <span class="docs-kicker">Documentation</span>
      <h1>BFish MCP</h1>
      <p>
        Connect any MCP-capable model to a live Roblox Studio session. This is the Rust fork of
        <a href="https://github.com/Chrrxs/robloxstudio-mcp" target="_blank" rel="noreferrer">robloxstudio-mcp</a>,
        rebuilt with explicit schemas, predictable errors, and a connector-only Studio plugin.
      </p>
      <div class="docs-meta">
        <span>Rust</span><span>stdio + HTTP</span><span>23 tools</span><span>MIT</span>
      </div>
    </header>

    <section id="docs-quickstart">
      <h2>Quickstart</h2>
      <h3>Desktop app</h3>
      <ol>
        <li>Install and open BFish.</li>
        <li>Open <strong>Settings → Studio → Install / update connector</strong>.</li>
        <li>Restart Roblox Studio and open <strong>BFish Connector</strong> from the Plugins tab.</li>
        <li>Add a provider key in <strong>Settings → Providers</strong>, then start a chat.</li>
      </ol>
      <p>
        The app starts the backend on <code>127.0.0.1:8765</code> and the Studio bridge/MCP endpoint on
        <code>127.0.0.1:34871</code>. The connector keeps a long-poll open to the bridge.
      </p>

      <h3>Standalone MCP server</h3>
      <p>Build from a checkout, or install a release binary named <code>bfish-mcp</code>:</p>
      <div class="code"><div class="code-head"><span>Terminal</span><button class="copy-code" type="button">Copy</button></div><pre><code>git clone https://github.com/bfish-tools/bfish
cd bfish
cargo install --path crates/bfish-mcp --locked

bfish-mcp --install-plugin
bfish-mcp --stdio</code></pre></div>
      <p class="note">
        Only one process can own port <code>34871</code>. If the desktop app is running, use its
        built-in endpoint instead of starting a second background server.
      </p>
    </section>

    <section id="docs-clients">
      <h2>MCP client setup</h2>
      <p>Every client below talks to the same local server. Restart Studio after installing the connector.</p>
      <div class="client-grid">
        {#each clients as client}
          <div class="client-card">
            <div class="client-head"><strong>{client.name}</strong><span>{client.file}</span></div>
            <div class="code compact"><div class="code-head"><span>{client.name}</span><button class="copy-code" type="button">Copy</button></div><pre><code>{client.snippet}</code></pre></div>
          </div>
        {/each}
      </div>
      <p>
        For GUI clients, the config file location varies. Paste the <code>mcpServers</code> block into
        the client's MCP settings, or use its “Add MCP server” flow and point it at the
        <code>bfish-mcp</code> executable.
      </p>
    </section>

    <section id="docs-studio">
      <h2>Studio connector</h2>
      <p>
        The plugin is intentionally small. It shows the server URL, connection status, place name,
        place id, plugin version, Studio version, and session id. It contains no chat UI and no API keys.
      </p>
      <div class="two-col">
        <div>
          <h3>Install</h3>
          <div class="code compact"><div class="code-head"><span>Terminal</span><button class="copy-code" type="button">Copy</button></div><pre><code>bfish-mcp --install-plugin</code></pre></div>
          <p>
            This writes <code>BFishConnector.rbxmx</code> to your Roblox Plugins folder. You can also drag
            <code>plugin/dist/BFishConnector.rbxmx</code> there manually, or use Rojo.
          </p>
        </div>
        <div>
          <h3>Status</h3>
          <ul>
            <li><strong>Connected</strong> — the bridge is reachable and polls are succeeding.</li>
            <li><strong>Not connected</strong> — start BFish, then press <strong>Reconnect</strong>.</li>
            <li><strong>Invalid server</strong> — only <code>localhost</code> and <code>127.0.0.1</code> are allowed.</li>
          </ul>
        </div>
      </div>
      <h3>Custom port</h3>
      <p>
        Change the URL in the connector, e.g. <code>http://127.0.0.1:34872</code>, and start the server
        with the same port:
      </p>
      <div class="code compact"><div class="code-head"><span>Terminal</span><button class="copy-code" type="button">Copy</button></div><pre><code>bfish-mcp --stdio --port 34872</code></pre></div>
    </section>

    <section id="docs-tools">
      <h2>Tool reference</h2>
      <p>
        Tool arguments are JSON objects. Instance paths use dot notation:
        <code>game.Workspace.Model.Part</code>. Names with spaces use brackets:
        <code>game.Workspace["My Model"].Part</code>.
      </p>
      {#each toolGroups as group}
        <h3>{group.label}</h3>
        <div class="tool-table">
          {#each group.tools as tool}
            <div class="tool-row">
              <code>{tool[0]}</code>
              <span>{tool[1]}</span>
            </div>
          {/each}
        </div>
      {/each}
      <p class="note">
        Successful calls return <code>{`{ "ok": true, "data": ... }`}</code>. Failures return an MCP
        <code>isError</code> result with an actionable message, never a fake success.
      </p>
    </section>

    <section id="docs-paths">
      <h2>Paths & safety</h2>
      <ul>
        <li>Always discover paths with <code>get_file_tree</code> or <code>search_instances</code>; do not guess.</li>
        <li>Mutations are wrapped in a <code>ChangeHistoryService</code> recording, so Ctrl+Z works.</li>
        <li><code>execute_luau</code> runs synchronously in edit mode. Avoid infinite loops; timeouts do not kill the Luau thread.</li>
        <li>Playtest start/stop uses <code>StudioTestService</code> and can vary slightly between Studio versions.</li>
        <li>Screenshots and input injection are intentionally not included in v0.1 to keep the connector auditable.</li>
        <li>The bridge binds to <code>127.0.0.1</code> only and is never exposed to the network.</li>
      </ul>
    </section>

    <section id="docs-cli">
      <h2>CLI & configuration</h2>
      <div class="code"><div class="code-head"><span>bfish-mcp --help</span><button class="copy-code" type="button">Copy</button></div><pre><code>bfish-mcp [OPTIONS]

  --stdio              Speak MCP over stdin/stdout (default)
  --http               Serve MCP at http://127.0.0.1:34871/mcp
  --port &lt;PORT&gt;       Studio bridge / HTTP port (default 34871)
  --install-plugin     Install the Studio connector and exit
  --no-bridge          Do not start the Studio bridge (stdio only)
  -h, --help           Show help
  -V, --version        Print the version</code></pre></div>
      <h3>Environment</h3>
      <ul>
        <li><code>BFISH_PLUGINS_DIR</code> — override the Roblox Plugins folder.</li>
        <li><code>BFISH_HOME</code> — override the app config directory (settings and sessions).</li>
        <li><code>RUST_LOG</code> — e.g. <code>bfish_mcp=debug</code>. Logs go to stderr so stdio stays clean.</li>
      </ul>
      <h3>Desktop config</h3>
      <p>
        Settings and sessions are plain JSON under <code>%APPDATA%\BFish</code> on Windows,
        <code>~/Library/Application Support/BFish</code> on macOS, and
        <code>~/.config/BFish</code> on Linux.
      </p>
    </section>

    <section id="docs-troubleshooting">
      <h2>Troubleshooting</h2>
      <h3>“Studio is not connected”</h3>
      <ul>
        <li>Open Studio and check the BFish Connector window. It should say <strong>Connected</strong>.</li>
        <li>Press <strong>Reconnect</strong>. The plugin re-registers with a new session automatically.</li>
        <li>Make sure only one BFish process is using port 34871.</li>
      </ul>
      <h3>“HTTP requests are not enabled”</h3>
      <p>In Studio, enable <strong>Game Settings → Security → Allow HTTP Requests</strong>, then reconnect.</p>
      <h3>Tools work, but the model ignores them</h3>
      <p>
        Use a tool-calling model. Small local models may need an explicit instruction such as
        “Call studio_status first”. BFish's tool descriptions include path examples and return shapes to
        reduce that failure mode.
      </p>
      <h3>Provider errors</h3>
      <p>
        Use <strong>Settings → Providers → Test</strong> to list models. A 401 means the key is wrong;
        a 404 usually means the base URL or model name is wrong.
      </p>
      <h3>Playtest does not stop</h3>
      <p>
        Studio's test service can be slow to transition. Wait for edit mode, then retry. The manual
        stop button in Studio always works.
      </p>
    </section>

    <section id="docs-architecture">
      <h2>Architecture</h2>
      <div class="code"><div class="code-head"><span>Local-only data flow</span><button class="copy-code" type="button">Copy</button></div><pre><code>BFish desktop app (Tauri)
├── Svelte chat UI  ──HTTP/SSE──▶  bfish-backend (127.0.0.1:8765)
│                                  ├── provider adapters
│                                  ├── session + prompt storage
│                                  └── bfish-mcp Engine
│                                        └── Studio bridge (127.0.0.1:34871)
│                                              ▲ long poll / result
│                                              │
└── Roblox Studio  ◀── BFish Connector plugin ─┘

Standalone clients ──stdio or /mcp──▶ bfish-mcp</code></pre></div>
      <p>
        <code>bfish-mcp</code> exposes MCP over stdio and streamable HTTP, and hosts the same Studio
        bridge. The desktop app embeds the engine directly so there is no extra process to manage.
      </p>
      <h3>Fork lineage</h3>
      <p>
        The repository keeps <code>Chrrxs/robloxstudio-mcp</code> as the <code>upstream</code> git remote.
        The <code>bfish-rust</code> branch replaces the TypeScript packages with this Rust workspace while
        preserving upstream history for attribution and diffs.
      </p>
    </section>

    <div class="docs-footer">
      <button class="button ghost" onclick={onback}>← Back to home</button>
      <a class="button primary" href="https://github.com/bfish-tools/bfish">GitHub repository</a>
    </div>
  </article>
</div>

<style>
  .docs {
    display: grid;
    grid-template-columns: 210px minmax(0, 760px);
    gap: 54px;
    justify-content: center;
    max-width: 1180px;
    margin: 0 auto;
    padding: 48px 24px 90px;
  }

  .docs-nav {
    position: sticky;
    top: 92px;
    align-self: start;
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 11.5px;
    color: var(--muted);
  }

  .back {
    margin-bottom: 18px;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--muted);
    font-size: 11px;
    text-align: left;
    cursor: pointer;
  }

  .back:hover {
    color: var(--ink);
  }

  .docs-nav-label {
    padding-bottom: 7px;
    font-family: var(--mono);
    font-size: 9.5px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #aaa69d;
  }

  .docs-nav a {
    padding: 6px 8px;
    border-radius: 5px;
  }

  .docs-nav a:hover {
    background: var(--paper-2);
    color: var(--ink);
  }

  .docs-header {
    padding-bottom: 28px;
    border-bottom: 1px solid var(--line);
    margin-bottom: 8px;
  }

  .docs-kicker {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.11em;
    text-transform: uppercase;
    color: var(--teal);
  }

  .docs-header h1 {
    margin: 12px 0 14px;
    font-size: 44px;
    font-weight: 650;
    letter-spacing: -0.045em;
  }

  .docs-header p {
    margin: 0;
    font-size: 13.5px;
    line-height: 1.7;
    color: var(--muted);
  }

  .docs-header a {
    color: var(--teal);
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .docs-meta {
    display: flex;
    gap: 16px;
    margin-top: 18px;
    font-family: var(--mono);
    font-size: 9.5px;
    color: #8e8b84;
  }

  section {
    padding-top: 30px;
  }

  h2 {
    margin: 0 0 16px;
    font-size: 24px;
    font-weight: 640;
    letter-spacing: -0.03em;
  }

  h3 {
    margin: 24px 0 8px;
    font-size: 14px;
    font-weight: 650;
  }

  p,
  li {
    font-size: 12.5px;
    line-height: 1.72;
    color: var(--muted);
  }

  p {
    margin: 0 0 12px;
  }

  ul,
  ol {
    margin: 0 0 14px;
    padding-left: 20px;
  }

  li {
    margin: 4px 0;
  }

  li strong {
    color: var(--ink);
    font-weight: 600;
  }

  code {
    padding: 1.5px 4px;
    border-radius: 4px;
    background: var(--paper-2);
    font-family: var(--mono);
    font-size: 11px;
    color: #265e56;
  }

  .code {
    margin: 12px 0 16px;
    overflow: hidden;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: #151a19;
  }

  .code-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    font-family: var(--mono);
    font-size: 9.5px;
    color: #7f9b95;
  }

  .code pre {
    margin: 0;
    padding: 13px;
    overflow-x: auto;
    font-family: var(--mono);
    font-size: 10.5px;
    line-height: 1.7;
    color: #d3ebe5;
  }

  .code code {
    padding: 0;
    background: transparent;
    color: inherit;
    font-size: inherit;
  }

  .code .copy-code {
    border: 0;
    background: transparent;
    color: #7f9b95;
    font-family: var(--mono);
    font-size: 9.5px;
    cursor: pointer;
  }

  .code .copy-code:hover {
    color: #d1fae5;
  }

  .note {
    padding: 10px 12px;
    border-left: 2px solid var(--teal);
    background: var(--teal-soft);
    color: #47615c;
    font-size: 11.5px;
    line-height: 1.65;
  }

  .client-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
    margin: 14px 0 18px;
  }

  .client-card {
    padding: 12px;
    border: 1px solid var(--line);
    border-radius: 9px;
    background: #fdfcfb;
  }

  .client-head {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-bottom: 8px;
  }

  .client-head strong {
    font-size: 12px;
  }

  .client-head span {
    font-size: 9.5px;
    color: #99958d;
  }

  .client-card .code {
    margin: 0;
  }

  .client-card .code-head {
    display: none;
  }

  .client-card .code pre {
    max-height: 190px;
  }

  .two-col {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 24px;
  }

  .tool-table {
    overflow: hidden;
    border: 1px solid var(--line);
    border-radius: 8px;
    margin-bottom: 10px;
  }

  .tool-row {
    display: grid;
    grid-template-columns: 190px minmax(0, 1fr);
    gap: 14px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line);
    font-size: 11.5px;
  }

  .tool-row:last-child {
    border-bottom: 0;
  }

  .tool-row:nth-child(odd) {
    background: #f7f5f1;
  }

  .tool-row code {
    background: transparent;
    padding: 0;
    font-size: 10.5px;
    color: var(--teal);
  }

  .tool-row span {
    color: var(--muted);
    line-height: 1.5;
  }

  .docs-footer {
    display: flex;
    gap: 10px;
    margin-top: 50px;
    padding-top: 26px;
    border-top: 1px solid var(--line);
  }

  @media (max-width: 900px) {
    .docs {
      grid-template-columns: 1fr;
      padding-top: 28px;
    }

    .docs-nav {
      position: static;
      flex-direction: row;
      flex-wrap: wrap;
      gap: 4px 10px;
      padding-bottom: 18px;
      border-bottom: 1px solid var(--line);
    }

    .docs-nav-label {
      display: none;
    }

    .back {
      width: 100%;
      margin-bottom: 8px;
    }
  }

  @media (max-width: 640px) {
    .client-grid,
    .two-col {
      grid-template-columns: 1fr;
    }

    .tool-row {
      grid-template-columns: 1fr;
      gap: 2px;
    }

    .docs-header h1 {
      font-size: 36px;
    }
  }
</style>
