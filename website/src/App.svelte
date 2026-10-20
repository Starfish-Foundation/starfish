<script lang="ts">
import { onMount } from 'svelte';
import Docs from './components/Docs.svelte';

type View = 'home' | 'docs';

let view = $state<View>('home');

function applyHash() {
  const next: View = window.location.hash.startsWith('#docs') ? 'docs' : 'home';
  const changed = next !== view;
  view = next;
  if (changed) window.scrollTo({ top: 0 });
}

function navigate(next: View) {
  window.location.hash = next === 'docs' ? '#docs' : '';
  view = next;
  window.scrollTo({ top: 0 });
}

function copyCode(event: MouseEvent) {
  const target = event.target as HTMLElement | null;
  const button = target?.closest('.copy-code') as HTMLElement | null;
  if (!button) return;
  const code = button.closest('.code')?.querySelector('code')?.textContent ?? '';
  navigator.clipboard.writeText(code).then(() => {
    const previous = button.textContent;
    button.textContent = 'Copied';
    window.setTimeout(() => (button.textContent = previous ?? 'Copy'), 1300);
  });
}

const providers = [
  { name: 'OpenAI', model: 'GPT-4.1 · o3' },
  { name: 'Claude', model: 'Sonnet · Opus' },
  { name: 'Gemini', model: '2.5 Pro · Flash' },
  { name: 'DeepSeek', model: 'V3 · Reasoner' },
  { name: 'xAI', model: 'Grok 4' },
  { name: 'OpenRouter', model: '300+ models' },
  { name: 'Ollama', model: 'Local models' },
];

const features = [
  {
    title: 'Bring your own key',
    body: 'Seven providers in one clean picker. Model lists come live from each provider, and keys stay on your machine.',
    icon: 'key',
  },
  {
    title: 'Real Studio tools',
    body: 'Read the explorer, write Luau, create instances, run playtests, and read console output through MCP.',
    icon: 'wrench',
  },
  {
    title: 'Chat with sessions',
    body: 'A familiar ChatGPT-style workspace. Every conversation is saved locally and can be renamed or deleted.',
    icon: 'chat',
  },
  {
    title: 'Connector-only plugin',
    body: 'The Studio plugin shows connection info and relays tool calls. No second chat window, no keys in Studio.',
    icon: 'plug',
  },
  {
    title: 'Vendor-neutral MCP',
    body: 'bfish-mcp is a Rust fork of robloxstudio-mcp with explicit schemas for Claude, GPT, Gemini, and local models.',
    icon: 'terminal',
  },
  {
    title: 'Local-first',
    body: 'The backend, MCP bridge, sessions, and keys live on 127.0.0.1. Studio changes still respect undo.',
    icon: 'shield',
  },
];

const steps = [
  {
    title: 'Connect Studio',
    body: 'Run the app, install the connector from Settings → Studio, then open BFish Connector from the Plugins tab.',
  },
  {
    title: 'Pick a model',
    body: 'Add an API key for OpenAI, Claude, Gemini, DeepSeek, xAI, or OpenRouter. BFish detects which models your key can use. Or point it at a local Ollama model.',
  },
  {
    title: 'Ask for changes',
    body: 'BFish inspects the place, calls Studio tools, writes code, and verifies the result. Every edit can be undone.',
  },
];

onMount(() => {
  applyHash();
  window.addEventListener('hashchange', applyHash);
  return () => window.removeEventListener('hashchange', applyHash);
});
</script>

<svelte:window onclick={copyCode} />

<div class="site">
  <header class="nav">
    <button class="brand" onclick={() => navigate('home')}>
      <svg viewBox="0 0 64 64" aria-hidden="true">
        <rect width="64" height="64" rx="16" fill="#0f766e" />
        <path d="M14 32c8-11 20-14 30-8l6-6v14l-6 6c-10 6-22 3-30-6z" fill="#ecfdf5" />
        <circle cx="37" cy="29" r="3" fill="#0f766e" />
        <path d="M14 38c6 4 13 5 20 2" stroke="#ecfdf5" stroke-width="3" fill="none" stroke-linecap="round" />
      </svg>
      <span>BFish</span>
    </button>

    <nav class="nav-links">
      <a href="#features">Features</a>
      <a href="#how">How it works</a>
      <a href="#mcp">MCP</a>
      <button onclick={() => navigate('docs')}>Docs</button>
      <a href="https://github.com/bfish-tools/bfish" target="_blank" rel="noreferrer">GitHub</a>
    </nav>

    <div class="nav-actions">
      <button class="docs-link" onclick={() => navigate('docs')}>Read the docs</button>
      <a class="button primary small" href="https://github.com/bfish-tools/bfish/releases">Download</a>
    </div>
  </header>

  {#if view === 'docs'}
    <Docs onback={() => navigate('home')} />
  {:else}
    <main>
      <section class="hero">
        <div class="hero-copy">
          <div class="eyebrow"><span></span> Open source · Bring your own key</div>
          <h1>AI in<br /><em>Roblox Studio.</em></h1>
          <p class="lede">
            BFish is a desktop AI engineer for your place. It reads the explorer, writes Luau,
            builds instances, and checks its work in a live Studio session.
          </p>
          <div class="hero-actions">
            <a class="button primary" href="https://github.com/bfish-tools/bfish/releases">Download BFish</a>
            <button class="button ghost" onclick={() => navigate('docs')}>Read the docs</button>
          </div>
          <div class="hero-meta">
            <span>Windows · macOS · Linux</span>
            <span>Tauri + Rust</span>
            <span>MIT licensed</span>
          </div>
        </div>

        <div class="mockup-wrap" aria-hidden="true">
          <div class="mockup">
            <div class="mockup-sidebar">
              <div class="mockup-brand"><span>B</span> BFish</div>
              <div class="mockup-new">+ New chat</div>
              <div class="mockup-session active">Sword arena lobby</div>
              <div class="mockup-session">Fix DataStore saving</div>
              <div class="mockup-session">Shop UI pass</div>
              <div class="mockup-status"><i></i> Studio connected</div>
            </div>
            <div class="mockup-main">
              <div class="mockup-top">
                <span>Claude · claude-sonnet-4-5</span>
                <span class="mockup-pill">Baseplate</span>
              </div>
              <div class="mockup-chat">
                <div class="mockup-user">Build a round-based sword arena with a lobby and scoreboard.</div>
                <div class="mockup-assistant">
                  <div class="mockup-avatar">B</div>
                  <div>
                    <p>I’ll inspect the place first, then create the arena folder and scripts.</p>
                    <div class="mockup-tool"><span class="check">✓</span> get_file_tree <i>Done</i></div>
                    <div class="mockup-tool"><span class="check">✓</span> create_instance <i>Done</i></div>
                    <div class="mockup-tool running"><span class="dot"></span> set_script_source <i>Running</i></div>
                  </div>
                </div>
              </div>
              <div class="mockup-composer">Ask BFish to build, debug, or explain…</div>
            </div>
          </div>
          <div class="mockup-glow"></div>
        </div>
      </section>

      <section class="provider-strip">
        <span class="strip-label">Works with your favorite models</span>
        <div class="provider-row">
          {#each providers as provider}
            <div class="provider-name">
              <strong>{provider.name}</strong>
              <small>{provider.model}</small>
            </div>
          {/each}
        </div>
      </section>

      <section id="features" class="section">
        <div class="section-head">
          <span class="kicker">Features</span>
          <h2>A small app that does the<br />boring parts of Studio work.</h2>
        </div>
        <div class="feature-grid">
          {#each features as feature}
            <article class="feature">
              <div class="feature-icon">
                {#if feature.icon === 'key'}
                  <svg viewBox="0 0 24 24"><circle cx="8" cy="12" r="4" /><path d="M12 12h9M18 12v3M15 12v2" /></svg>
                {:else if feature.icon === 'wrench'}
                  <svg viewBox="0 0 24 24"><path d="M14 6a4 4 0 0 0-5 5l-5 5 3 3 5-5a4 4 0 0 0 5-5l-2 2-3-3 2-2z" /></svg>
                {:else if feature.icon === 'chat'}
                  <svg viewBox="0 0 24 24"><path d="M4 5h16v11H8l-4 4V5z" /><path d="M8 9h8M8 12h5" /></svg>
                {:else if feature.icon === 'plug'}
                  <svg viewBox="0 0 24 24"><path d="M9 3v5M15 3v5M6 8h12v3a6 6 0 0 1-12 0V8zM12 17v4" /></svg>
                {:else if feature.icon === 'terminal'}
                  <svg viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="16" rx="2" /><path d="M7 9l3 3-3 3M12 15h5" /></svg>
                {:else}
                  <svg viewBox="0 0 24 24"><path d="M12 3l8 4v5c0 5-3.5 8-8 9-4.5-1-8-4-8-9V7l8-4z" /><path d="M9 12l2 2 4-4" /></svg>
                {/if}
              </div>
              <h3>{feature.title}</h3>
              <p>{feature.body}</p>
            </article>
          {/each}
        </div>
      </section>

      <section id="how" class="section how">
        <div class="section-head">
          <span class="kicker">How it works</span>
          <h2>Three steps to a live<br />Studio session.</h2>
        </div>
        <div class="steps">
          {#each steps as step, index}
            <article class="step">
              <span class="step-number">0{index + 1}</span>
              <h3>{step.title}</h3>
              <p>{step.body}</p>
            </article>
          {/each}
        </div>
      </section>

      <section id="mcp" class="mcp-section">
        <div class="mcp-copy">
          <span class="kicker light">bfish-mcp</span>
          <h2>Your tools.<br />Any model.</h2>
          <p>
            BFish ships a Rust MCP server forked from robloxstudio-mcp. Its tool schemas are explicit,
            its errors are actionable, and it works over stdio or streamable HTTP with Claude, GPT,
            Gemini, DeepSeek, Cursor, Codex, and local models.
          </p>
          <div class="mcp-actions">
            <button class="button light" onclick={() => navigate('docs')}>MCP documentation</button>
            <a class="button outline-light" href="https://github.com/bfish-tools/bfish">View source</a>
          </div>
          <div class="tool-chips">
            {#each ['execute_luau', 'get_file_tree', 'set_script_source', 'create_instance', 'search_instances', 'start_playtest', 'get_console_output', '+ 17 more'] as tool}
              <span>{tool}</span>
            {/each}
          </div>
        </div>
        <div class="mcp-code">
          <div class="code-head"><span>claude_desktop_config.json</span><button class="copy-code" type="button">Copy</button></div>
          <pre class="code"><code>{`{
  "mcpServers": {
    "bfish": {
      "command": "bfish-mcp",
      "args": ["--stdio"]
    }
  }
}`}</code></pre>
          <div class="mcp-note">
            <span class="mcp-dot"></span>
            Studio bridge on <code>127.0.0.1:34871</code>
          </div>
        </div>
      </section>

      <section class="section faq" id="faq">
        <div class="section-head">
          <span class="kicker">FAQ</span>
          <h2>Good questions.</h2>
        </div>
        <div class="faq-list">
          <details>
            <summary>Is BFish free?</summary>
            <p>Yes. BFish is MIT licensed and open source. You pay your model provider directly for usage.</p>
          </details>
          <details>
            <summary>Does it work with free local models?</summary>
            <p>Yes. Point the Ollama provider at <code>http://127.0.0.1:11434/v1</code> and use a tool-calling model such as Qwen 3. Local models are less reliable at long tool chains, but they work.</p>
          </details>
          <details>
            <summary>Can BFish delete things?</summary>
            <p>It can, but deletion is a destructive tool and the system prompt requires confirmation. Every mutation runs inside a Studio undo recording, so Ctrl+Z works.</p>
          </details>
          <details>
            <summary>Where are my API keys stored?</summary>
            <p>In your local BFish config folder, for example <code>%APPDATA%\\BFish\\settings.json</code> on Windows. They are never sent anywhere except the provider you selected.</p>
          </details>
          <details>
            <summary>Is this affiliated with Roblox?</summary>
            <p>No. BFish is an independent project. Roblox and Roblox Studio are trademarks of Roblox Corporation.</p>
          </details>
        </div>
      </section>

      <section class="final-cta">
        <h2>Make something in your place.</h2>
        <p>Download BFish, bring a key, and open the connector.</p>
        <div class="hero-actions center">
          <a class="button primary" href="https://github.com/bfish-tools/bfish/releases">Download BFish</a>
          <button class="button ghost" onclick={() => navigate('docs')}>Read the docs</button>
        </div>
      </section>
    </main>
  {/if}

  <footer class="footer">
    <div class="footer-brand">
      <span class="footer-logo">B</span>
      <div>
        <strong>BFish</strong>
        <small>AI in Roblox Studio.</small>
      </div>
    </div>
    <div class="footer-links">
      <button onclick={() => navigate('docs')}>Docs</button>
      <a href="https://github.com/bfish-tools/bfish" target="_blank" rel="noreferrer">GitHub</a>
      <a href="https://github.com/Chrrxs/robloxstudio-mcp" target="_blank" rel="noreferrer">Upstream</a>
    </div>
    <p class="footer-note">
      Independent project. Not affiliated with Roblox Corporation. © {new Date().getFullYear()} BFish contributors.
    </p>
  </footer>
</div>
