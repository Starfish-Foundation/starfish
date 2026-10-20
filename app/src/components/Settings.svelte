<script lang="ts">
  import { onMount } from 'svelte';
  import * as api from '../lib/api';
  import type { PromptPreset, ProviderPreset, Settings, StatusResponse } from '../lib/api';

  let {
    settings,
    presets,
    prompts,
    defaultPrompt,
    status,
    onclose,
    onsave,
    oninstall,
  }: {
    settings: Settings;
    presets: ProviderPreset[];
    prompts: PromptPreset[];
    defaultPrompt: string;
    status: StatusResponse | null;
    onclose: () => void;
    onsave: (settings: Settings) => Promise<void>;
    oninstall: () => Promise<string>;
  } = $props();

  // svelte-ignore state_referenced_locally
  let draft = $state<Settings>($state.snapshot(settings) as Settings);
  let tab = $state<'providers' | 'general' | 'studio' | 'about'>('providers');
  let selectedProvider = $state<string>(draft.provider);
  let showKey = $state(false);
  let testing = $state(false);
  let detectRequest = 0;
  let testMessage = $state('');
  let testFailed = $state(false);
  // svelte-ignore state_referenced_locally
  let models = $state<string[]>(
    presets.find((item) => item.id === draft.provider)?.models ?? [],
  );
  let modelsDetected = $state(false);
  let installing = $state(false);
  let installPath = $state('');
  let installFailed = $state(false);
  let saving = $state(false);
  let copied = $state('');

  const preset = $derived(presets.find((item) => item.id === selectedProvider) ?? presets[0]);
  const studio = $derived(status?.studio ?? null);
  const placeName = $derived(studio?.sessions?.[0]?.placeName ?? 'No place connected');

  const mcpSnippet = $derived(
    JSON.stringify(
      {
        mcpServers: {
          bfish: {
            command: 'bfish-mcp',
            args: ['--stdio'],
          },
        },
      },
      null,
      2,
    ),
  );

  const httpSnippet = $derived(`http://127.0.0.1:${draft.mcpPort}/mcp`);
  const modelChoices = $derived(
    modelsDetected ? models : [...new Set([...models, ...(preset?.models ?? [])])],
  );

  function isChatModelName(model: string): boolean {
    const name = model.toLowerCase();
    return ![
      'embedding',
      'embed-',
      'tts',
      'whisper',
      'dall-e',
      'moderation',
      'audio',
      'transcribe',
      'rerank',
      'realtime',
      'image',
    ].some((part) => name.includes(part));
  }

  function chooseModel(list: string[], provider: ProviderPreset): string {
    const curated = provider.models.find((model) => list.includes(model));
    if (curated) return curated;
    return list.find(isChatModelName) ?? list[0];
  }

  function selectProvider(id: string) {
    selectedProvider = id;
    showKey = false;
    testMessage = '';
    testFailed = false;
    modelsDetected = false;
    models = presets.find((item) => item.id === id)?.models ?? [];
    void detectModels(id, false);
  }

  /**
   * Ask the provider for its actual model list. This is the source of truth;
   * preset names are only a fallback when no key is available.
   */
  async function detectModels(providerId: string, manual: boolean) {
    const requestId = ++detectRequest;
    const providerPreset = presets.find((item) => item.id === providerId);
    const config = draft.providers[providerId];
    if (!providerPreset || !config) return;

    if (providerPreset.requiresKey && !config.apiKey.trim()) {
      if (manual) {
        testFailed = false;
        testMessage = `Add a ${providerPreset.label} API key to detect models.`;
      }
      models = providerPreset.models;
      modelsDetected = false;
      return;
    }

    testing = true;
    testMessage = manual ? 'Detecting models…' : '';
    testFailed = false;
    try {
      const result = await api.testProvider({
        provider: providerId,
        apiKey: config.apiKey,
        baseUrl: config.baseUrl,
        model: config.model,
      });

      if (requestId !== detectRequest || providerId !== selectedProvider) return;

      if (result.models.length === 0) {
        testFailed = true;
        testMessage = `${providerPreset.label} returned no models.`;
        modelsDetected = false;
        models = providerPreset.models;
        return;
      }

      models = result.models;
      modelsDetected = true;
      if (!result.models.includes(config.model)) {
        config.model = chooseModel(result.models, providerPreset);
      }
      testMessage = `Detected ${result.models.length} models from ${providerPreset.label}.`;
    } catch (error) {
      if (requestId !== detectRequest || providerId !== selectedProvider) return;
      testFailed = true;
      testMessage = api.errorMessage(error);
      modelsDetected = false;
    } finally {
      if (requestId === detectRequest) testing = false;
    }
  }

  async function testConnection() {
    await detectModels(selectedProvider, true);
  }

  onMount(() => {
    void detectModels(selectedProvider, false);
  });

  async function install() {
    installing = true;
    installFailed = false;
    try {
      installPath = await oninstall();
    } catch (error) {
      installFailed = true;
      installPath = api.errorMessage(error);
    } finally {
      installing = false;
    }
  }

  async function save() {
    saving = true;
    try {
      await onsave(draft);
      onclose();
    } finally {
      saving = false;
    }
  }

  function applyPromptPreset(id: string) {
    const chosen = prompts.find((item) => item.id === id);
    if (chosen) draft.systemPrompt = chosen.prompt;
  }

  function copy(text: string, key: string) {
    navigator.clipboard.writeText(text).then(() => {
      copied = key;
      window.setTimeout(() => {
        if (copied === key) copied = '';
      }, 1400);
    });
  }
</script>

<svelte:window
  onkeydown={(event) => {
    if (event.key === 'Escape') onclose();
  }}
/>

<div class="overlay" role="presentation" onclick={(event) => event.target === event.currentTarget && onclose()}>
  <div class="modal" role="dialog" aria-modal="true" aria-label="BFish settings">
    <div class="modal-nav">
      <div class="modal-brand">
        <span class="modal-logo">B</span>
        <div>
          <strong>Settings</strong>
          <small>BFish {status?.version ?? ''}</small>
        </div>
      </div>

      <button class="nav-item" class:active={tab === 'providers'} onclick={() => (tab = 'providers')}>
        <svg viewBox="0 0 24 24"><path d="M4 7h16M4 12h16M4 17h10" /></svg>
        Providers
      </button>
      <button class="nav-item" class:active={tab === 'general'} onclick={() => (tab = 'general')}>
        <svg viewBox="0 0 24 24"><path d="M5 6h14M5 12h14M5 18h8" /></svg>
        General
      </button>
      <button class="nav-item" class:active={tab === 'studio'} onclick={() => (tab = 'studio')}>
        <svg viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="16" rx="3" /><path d="M8 9h8M8 13h5" /></svg>
        Studio
      </button>
      <button class="nav-item" class:active={tab === 'about'} onclick={() => (tab = 'about')}>
        <svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="9" /><path d="M12 11v5M12 8h.01" /></svg>
        About
      </button>

      <div class="nav-spacer"></div>
      <button class="nav-item close-mobile" onclick={onclose}>Close</button>
    </div>

    <div class="modal-main">
      <header class="modal-header">
        <div>
          <h2>
            {tab === 'providers' ? 'Providers & API keys' : tab === 'general' ? 'General' : tab === 'studio' ? 'Roblox Studio' : 'About BFish'}
          </h2>
          <p>
            {tab === 'providers'
              ? 'Bring your own key. Keys stay on this computer.'
              : tab === 'general'
                ? 'Tune how BFish behaves and how much freedom it has.'
                : tab === 'studio'
                  ? 'Connect the desktop app to a live Studio session.'
                  : 'A small, open Roblox AI client.'}
          </p>
        </div>
        <button class="close-button" title="Close" onclick={onclose}>
          <svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18" /></svg>
        </button>
      </header>

      <div class="modal-content">
        {#if tab === 'providers'}
          <div class="provider-layout">
            <div class="provider-list">
              {#each presets as item}
                <button
                  class="provider-item"
                  class:active={selectedProvider === item.id}
                  onclick={() => selectProvider(item.id)}
                >
                  <span class="provider-dot" class:on={Boolean(draft.providers[item.id]?.apiKey) || !item.requiresKey}></span>
                  <span>{item.label}</span>
                  <span class="provider-arrow">›</span>
                </button>
              {/each}
            </div>

            <div class="provider-panel">
              {#if preset}
                <div class="provider-title">
                  <h3>{preset.label}</h3>
                  <span class="provider-tag">{selectedProvider}</span>
                </div>
                <p class="provider-note">{preset.note}</p>

                {#if preset.requiresKey}
                  <label class="field">
                    <span>API key</span>
                    <div class="input-row">
                      <input
                        type={showKey ? 'text' : 'password'}
                        bind:value={draft.providers[selectedProvider].apiKey}
                        placeholder="Paste your API key"
                        autocomplete="off"
                        spellcheck="false"
                      />
                      <button class="ghost-button" onclick={() => (showKey = !showKey)}>
                        {showKey ? 'Hide' : 'Show'}
                      </button>
                    </div>
                    <small>
                      Get a key at <a href={preset.keyUrl} target="_blank" rel="noreferrer">{preset.keyUrl}</a>
                    </small>
                  </label>
                {:else}
                  <div class="callout">
                    No API key needed. Install Ollama, run <code>ollama serve</code>, and pull a model such as
                    <code>qwen3:8b</code>.
                  </div>
                {/if}

                <label class="field">
                  <span>Base URL</span>
                  <input bind:value={draft.providers[selectedProvider].baseUrl} spellcheck="false" />
                  <small>Override for proxies, Azure-compatible gateways, or a remote Ollama host.</small>
                </label>

                <label class="field">
                  <span>Default model</span>
                  <div class="input-row">
                    <input bind:value={draft.providers[selectedProvider].model} list="bfish-models" spellcheck="false" />
                    <button class="ghost-button" onclick={testConnection} disabled={testing}>
                      {testing ? 'Detecting…' : 'Detect'}
                    </button>
                  </div>
                  <small>
                    {modelsDetected
                      ? 'Live model list from the provider API.'
                      : 'Press Detect to load the models your key can actually use.'}
                  </small>
                </label>

                <datalist id="bfish-models">
                  {#each modelChoices as model}
                    <option value={model}></option>
                  {/each}
                </datalist>

                <div class="model-suggestions">
                  <span>
                    {modelsDetected
                      ? `Detected models (${modelChoices.length})`
                      : 'Suggested model names'}
                  </span>
                  {#each modelChoices.slice(0, 10) as model}
                    <button
                      class="chip"
                      class:selected={draft.providers[selectedProvider].model === model}
                      onclick={() => (draft.providers[selectedProvider].model = model)}
                    >
                      {model}
                    </button>
                  {/each}
                  {#if modelChoices.length > 10}
                    <span class="more-models">+{modelChoices.length - 10} more</span>
                  {/if}
                </div>

                {#if testMessage}
                  <div class="test-result" class:failed={testFailed}>{testMessage}</div>
                {/if}
              {/if}
            </div>
          </div>
        {:else if tab === 'general'}
          <div class="stack">
            <div class="field">
              <span>System prompt</span>
              <div class="prompt-presets">
                {#each prompts as promptPreset}
                  <button class="chip" onclick={() => applyPromptPreset(promptPreset.id)} title={promptPreset.description}>
                    {promptPreset.name}
                  </button>
                {/each}
                <button class="chip" onclick={() => (draft.systemPrompt = defaultPrompt)}>Reset</button>
              </div>
              <textarea bind:value={draft.systemPrompt} rows="16" spellcheck="false"></textarea>
              <small>This is prepended to every conversation. BFish also appends live Studio connection context.</small>
            </div>

            <div class="split">
              <label class="field">
                <span>Temperature <em>{draft.temperature.toFixed(2)}</em></span>
                <input type="range" min="0" max="1" step="0.05" bind:value={draft.temperature} />
                <small>Lower is more deterministic. 0.3–0.5 works well for code.</small>
              </label>

              <label class="field">
                <span>Max output tokens</span>
                <input type="number" min="256" max="65536" step="256" bind:value={draft.maxTokens} />
                <small>Large script generations need 4k or more.</small>
              </label>
            </div>
          </div>
        {:else if tab === 'studio'}
          <div class="stack">
            <div class="studio-card">
              <span class="studio-dot" class:connected={studio?.connected}></span>
              <div>
                <strong>{studio?.connected ? 'Connected' : 'Not connected'}</strong>
                <p>
                  {studio?.connected
                    ? placeName
                    : 'Open Roblox Studio and enable the BFish Connector from the Plugins tab.'}
                </p>
              </div>
              <span class="studio-port">port {draft.mcpPort}</span>
            </div>

            {#if status?.bridgeError}
              <div class="test-result failed">{status.bridgeError}</div>
            {/if}

            <div class="field">
              <span>Studio connector</span>
              <div class="input-row">
                <button class="primary-button" onclick={install} disabled={installing}>
                  {installing ? 'Installing…' : 'Install / update connector'}
                </button>
                <button
                  class="ghost-button"
                  onclick={() => copy(mcpSnippet, 'mcp')}
                >
                  {copied === 'mcp' ? 'Copied' : 'Copy MCP config'}
                </button>
              </div>
              {#if installPath}
                <div class="test-result" class:failed={installFailed}>{installPath}</div>
              {:else}
                <small>Writes <code>BFishConnector.rbxmx</code> to your Roblox Plugins folder. Restart Studio afterwards.</small>
              {/if}
            </div>

            <div class="field">
              <span>MCP endpoint</span>
              <div class="input-row">
                <input value={httpSnippet} readonly />
                <button class="ghost-button" onclick={() => copy(httpSnippet, 'http')}>
                  {copied === 'http' ? 'Copied' : 'Copy'}
                </button>
              </div>
              <small>
                Other MCP clients can use this streamable-HTTP endpoint, or run <code>bfish-mcp --stdio</code>.
              </small>
            </div>

            <div class="field">
              <span>Client configuration</span>
              <pre class="snippet">{mcpSnippet}</pre>
              <small>Use this in Claude Desktop, Cursor, Cline, VS Code, Codex, or any MCP-compatible client.</small>
            </div>

            <div class="callout">
              The plugin is only a connector. It shows status and relays tools; it never stores keys or chats with a model.
            </div>
          </div>
        {:else}
          <div class="stack about">
            <div class="about-hero">
              <span class="about-logo">B</span>
              <div>
                <h3>BFish</h3>
                <p>AI in Roblox Studio. Bring your own model.</p>
              </div>
            </div>
            <div class="about-grid">
              <div><span>Version</span><strong>{status?.version ?? '0.1.0'}</strong></div>
              <div><span>MCP</span><strong>bfish-mcp</strong></div>
              <div><span>Providers</span><strong>{presets.length}</strong></div>
              <div><span>Tools</span><strong>23</strong></div>
            </div>
            <p class="about-text">
              BFish is an independent open-source project. It is not affiliated with, endorsed by, or
              sponsored by Roblox Corporation. Roblox and Roblox Studio are trademarks of Roblox Corporation.
            </p>
            <p class="about-text">
              API keys are stored locally in your user config folder. BFish only binds to 127.0.0.1 and never
              uploads your place to a BFish server.
            </p>
            <div class="about-links">
              <a href="https://github.com/bfish-tools/bfish" target="_blank" rel="noreferrer">GitHub</a>
              <a href="https://github.com/bfish-tools/bfish/tree/main/docs" target="_blank" rel="noreferrer">Documentation</a>
              <a href="https://github.com/Chrrxs/robloxstudio-mcp" target="_blank" rel="noreferrer">Upstream MCP</a>
            </div>
          </div>
        {/if}
      </div>

      <footer class="modal-footer">
        <span class="footer-note">Keys are stored unencrypted in your local BFish config folder.</span>
        <div class="footer-actions">
          <button class="ghost-button" onclick={onclose}>Cancel</button>
          <button class="primary-button" onclick={save} disabled={saving}>
            {saving ? 'Saving…' : 'Save changes'}
          </button>
        </div>
      </footer>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 80;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgba(7, 8, 10, 0.66);
    backdrop-filter: blur(3px);
  }

  .modal {
    width: min(880px, 100%);
    height: min(660px, calc(100vh - 48px));
    display: flex;
    overflow: hidden;
    border: 1px solid var(--border-strong);
    border-radius: 16px;
    background: #1b1c22;
    box-shadow: 0 30px 90px rgba(0, 0, 0, 0.5);
  }

  .modal-nav {
    width: 188px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 14px 10px;
    background: #15161b;
    border-right: 1px solid var(--border);
  }

  .modal-brand {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 6px 8px 16px;
  }

  .modal-logo {
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    border-radius: 8px;
    background: var(--accent);
    color: #082f2b;
    font-weight: 800;
  }

  .modal-brand strong {
    display: block;
    font-size: 12px;
  }

  .modal-brand small {
    font-size: 10px;
    color: var(--muted);
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 9px 10px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--text-soft);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }

  .nav-item:hover {
    background: var(--hover);
  }

  .nav-item.active {
    background: var(--active);
    color: var(--text);
  }

  .nav-item svg {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .nav-spacer {
    flex: 1;
  }

  .close-mobile {
    display: none;
  }

  .modal-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .modal-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    padding: 22px 26px 16px;
    border-bottom: 1px solid var(--border);
  }

  .modal-header h2 {
    margin: 0 0 4px;
    font-size: 18px;
    font-weight: 650;
    letter-spacing: -0.02em;
  }

  .modal-header p {
    margin: 0;
    font-size: 11px;
    color: var(--muted);
  }

  .close-button {
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
  }

  .close-button:hover {
    background: var(--hover);
    color: var(--text);
  }

  .close-button svg {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
  }

  .modal-content {
    flex: 1;
    overflow-y: auto;
    padding: 20px 26px;
  }

  .modal-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 13px 26px;
    border-top: 1px solid var(--border);
  }

  .footer-note {
    font-size: 10px;
    color: var(--muted);
  }

  .footer-actions {
    display: flex;
    gap: 8px;
  }

  .provider-layout {
    display: grid;
    grid-template-columns: 178px minmax(0, 1fr);
    gap: 24px;
  }

  .provider-list {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .provider-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 10px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--text-soft);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }

  .provider-item:hover {
    background: var(--hover);
  }

  .provider-item.active {
    background: var(--active);
    color: var(--text);
  }

  .provider-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #52525b;
    flex: none;
  }

  .provider-dot.on {
    background: var(--accent);
  }

  .provider-arrow {
    margin-left: auto;
    color: var(--muted);
  }

  .provider-panel {
    min-width: 0;
  }

  .provider-title {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 4px;
  }

  .provider-title h3 {
    margin: 0;
    font-size: 16px;
    letter-spacing: -0.01em;
  }

  .provider-tag {
    padding: 2px 6px;
    border: 1px solid var(--border);
    border-radius: 999px;
    font-family: var(--mono);
    font-size: 9px;
    color: var(--muted);
  }

  .provider-note {
    margin: 0 0 18px;
    font-size: 11px;
    line-height: 1.55;
    color: var(--muted);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 16px;
    font-size: 11px;
    color: var(--text-soft);
  }

  .field > span {
    font-weight: 600;
    color: var(--text);
  }

  .field > span em {
    font-style: normal;
    font-weight: 400;
    color: var(--muted);
  }

  .field small {
    font-size: 10px;
    line-height: 1.5;
    color: var(--muted);
  }

  .field small a {
    color: var(--accent);
  }

  input,
  textarea {
    width: 100%;
    padding: 9px 11px;
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    outline: 0;
    background: #16171c;
    color: var(--text);
    font-family: inherit;
    font-size: 12px;
  }

  input:focus,
  textarea:focus {
    border-color: rgba(45, 212, 191, 0.5);
  }

  textarea {
    resize: vertical;
    min-height: 120px;
    line-height: 1.55;
    font-family: var(--mono);
    font-size: 11px;
  }

  input[readonly] {
    color: var(--muted);
  }

  .input-row {
    display: flex;
    gap: 7px;
  }

  .input-row input {
    flex: 1;
    min-width: 0;
  }

  .primary-button,
  .ghost-button {
    padding: 8px 12px;
    border-radius: 7px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    white-space: nowrap;
  }

  .primary-button {
    border: 1px solid transparent;
    background: var(--accent);
    color: #062a26;
  }

  .primary-button:hover:not(:disabled) {
    background: #5eead4;
  }

  .ghost-button {
    border: 1px solid var(--border-strong);
    background: transparent;
    color: var(--text-soft);
  }

  .ghost-button:hover:not(:disabled) {
    background: var(--hover);
    color: var(--text);
  }

  .primary-button:disabled,
  .ghost-button:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .callout {
    margin-bottom: 16px;
    padding: 11px 13px;
    border: 1px solid rgba(45, 212, 191, 0.18);
    border-radius: 8px;
    background: rgba(45, 212, 191, 0.05);
    font-size: 10.5px;
    line-height: 1.55;
    color: var(--text-soft);
  }

  .callout code,
  .field small code {
    padding: 1px 4px;
    border-radius: 4px;
    background: #23252d;
    font-family: var(--mono);
    font-size: 10px;
  }

  .test-result {
    margin-top: 4px;
    padding: 8px 10px;
    border: 1px solid rgba(45, 212, 191, 0.22);
    border-radius: 7px;
    background: rgba(45, 212, 191, 0.06);
    color: #99f6e4;
    font-size: 10.5px;
    line-height: 1.5;
    overflow-wrap: anywhere;
  }

  .test-result.failed {
    border-color: rgba(248, 113, 113, 0.3);
    background: rgba(248, 113, 113, 0.07);
    color: #fca5a5;
  }

  .stack {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .prompt-presets {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .chip {
    padding: 4px 9px;
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    background: transparent;
    color: var(--text-soft);
    font-size: 10px;
    cursor: pointer;
  }

  .chip:hover {
    background: var(--hover);
    color: var(--text);
  }

  .chip.selected {
    border-color: rgba(45, 212, 191, 0.45);
    background: rgba(45, 212, 191, 0.09);
    color: #99f6e4;
  }

  .model-suggestions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin: -6px 0 16px;
  }

  .model-suggestions > span {
    margin-right: 3px;
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--muted);
  }

  .model-suggestions button {
    font-family: var(--mono);
    font-size: 9px;
  }

  .more-models {
    align-self: center;
    font-size: 9px;
    color: var(--muted);
  }

  .split {
    display: grid;
    grid-template-columns: 1.4fr 1fr;
    gap: 22px;
  }

  input[type='range'] {
    padding: 0;
    accent-color: var(--accent);
    border: 0;
    background: transparent;
  }

  .studio-card {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: #17181d;
    margin-bottom: 14px;
  }

  .studio-dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: #64748b;
    flex: none;
  }

  .studio-dot.connected {
    background: var(--accent);
    box-shadow: 0 0 0 4px rgba(45, 212, 191, 0.12);
  }

  .studio-card strong {
    font-size: 12px;
  }

  .studio-card p {
    margin: 2px 0 0;
    font-size: 10.5px;
    color: var(--muted);
  }

  .studio-port {
    margin-left: auto;
    font-family: var(--mono);
    font-size: 10px;
    color: var(--muted);
  }

  .snippet {
    margin: 0;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: #14151a;
    color: #c7d2dd;
    font-family: var(--mono);
    font-size: 10.5px;
    line-height: 1.55;
    overflow-x: auto;
  }

  .about-hero {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-bottom: 16px;
  }

  .about-logo {
    width: 46px;
    height: 46px;
    display: grid;
    place-items: center;
    border-radius: 12px;
    background: var(--accent);
    color: #082f2b;
    font-size: 22px;
    font-weight: 800;
  }

  .about-hero h3 {
    margin: 0;
    font-size: 20px;
  }

  .about-hero p {
    margin: 3px 0 0;
    font-size: 11px;
    color: var(--muted);
  }

  .about-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
    margin-bottom: 18px;
  }

  .about-grid div {
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: #17181d;
  }

  .about-grid span {
    display: block;
    margin-bottom: 3px;
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--muted);
  }

  .about-grid strong {
    font-size: 11px;
  }

  .about-text {
    margin: 0 0 10px;
    font-size: 11px;
    line-height: 1.65;
    color: var(--muted);
  }

  .about-links {
    display: flex;
    gap: 14px;
    margin-top: 8px;
    font-size: 11px;
  }

  .about-links a {
    color: var(--accent);
  }

  @media (max-width: 760px) {
    .overlay {
      padding: 0;
    }

    .modal {
      width: 100%;
      height: 100%;
      border: 0;
      border-radius: 0;
      flex-direction: column;
    }

    .modal-nav {
      width: 100%;
      flex-direction: row;
      align-items: center;
      gap: 2px;
      overflow-x: auto;
      padding: 8px;
      border-right: 0;
      border-bottom: 1px solid var(--border);
    }

    .modal-brand,
    .nav-spacer,
    .close-mobile {
      display: none;
    }

    .nav-item {
      flex: none;
      white-space: nowrap;
    }

    .provider-layout {
      grid-template-columns: 1fr;
      gap: 12px;
    }

    .provider-list {
      flex-direction: row;
      flex-wrap: wrap;
    }

    .provider-item {
      border: 1px solid var(--border);
    }

    .split {
      grid-template-columns: 1fr;
    }

    .footer-note {
      display: none;
    }
  }
</style>
