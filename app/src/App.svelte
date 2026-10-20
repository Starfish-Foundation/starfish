<script lang="ts">
  import { onMount } from 'svelte';
  import ChatView from './components/ChatView.svelte';
  import Settings from './components/Settings.svelte';
  import Sidebar from './components/Sidebar.svelte';
  import * as api from './lib/api';
  import type {
    ChatEvent,
    Message,
    PromptPreset,
    ProviderPreset,
    Session,
    SessionSummary,
    Settings as SettingsType,
    StatusResponse,
  } from './lib/api';
  import type { UiMessage, UiTool } from './lib/ui';

  let settings = $state<SettingsType | null>(null);
  let presets = $state<ProviderPreset[]>([]);
  let prompts = $state<PromptPreset[]>([]);
  let defaultPrompt = $state('');
  let sessions = $state<SessionSummary[]>([]);
  let active = $state<Session | null>(null);
  let messages = $state<UiMessage[]>([]);
  let streaming = $state(false);
  let status = $state<StatusResponse | null>(null);
  let settingsOpen = $state(false);
  let sidebarOpen = $state(false);
  let loading = $state(true);
  let fatal = $state('');
  let toast = $state('');
  let toastTimer: number | undefined;
  let controller: AbortController | null = null;
  let stopping = false;
  let pollTimer: number | undefined;
  let detectedModels = $state<Record<string, string[]>>({});
  let detectingModels = $state(false);

  const currentPreset = $derived(
    presets.find((item) => item.id === (active?.provider ?? settings?.provider)) ?? null,
  );
  const activeProvider = $derived(active?.provider ?? settings?.provider ?? 'openai');
  const studioConnected = $derived(status?.studio?.connected ?? false);
  const placeName = $derived(status?.studio?.sessions?.[0]?.placeName ?? 'No place');
  const modelLabel = $derived(
    `${currentPreset?.label ?? active?.provider ?? 'Model'} · ${active?.model ?? settings?.model ?? ''}`,
  );
  const availableModels = $derived(
    detectedModels[activeProvider] ?? currentPreset?.models ?? [],
  );

  function uid(): string {
    return typeof crypto !== 'undefined' && 'randomUUID' in crypto
      ? crypto.randomUUID()
      : Math.random().toString(36).slice(2);
  }

  function showToast(message: string) {
    toast = message;
    if (toastTimer) window.clearTimeout(toastTimer);
    toastTimer = window.setTimeout(() => (toast = ''), 3200);
  }

  function parseArguments(raw: string | undefined): unknown {
    if (!raw) return undefined;
    try {
      return JSON.parse(raw);
    } catch {
      return raw;
    }
  }

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

  /** Prefer a known-good family, then the first chat-like model the API returned. */
  function pickAvailableModel(models: string[], preset: ProviderPreset | null): string {
    const curated = preset?.models.find((model) => models.includes(model));
    if (curated) return curated;
    const chatModels = models.filter(isChatModelName);
    return chatModels[0] ?? models[0];
  }

  async function fetchModels(provider: string): Promise<string[]> {
    const preset = presets.find((item) => item.id === provider);
    const config = settings?.providers[provider];
    if (!preset || !config) throw new Error('Provider is not configured.');
    if (preset.requiresKey && !config.apiKey.trim()) {
      throw new Error(`Add a ${preset.label} API key to detect models.`);
    }
    const result = await api.getProviderModels(provider);
    return result.models;
  }

  /**
   * Fetch the provider's live model list. If the currently selected model is not
   * actually available, move the session (and saved default) to one that is.
   */
  async function syncModels(
    provider: string,
    sessionId?: string,
    force = false,
    notify = false,
  ): Promise<string[] | null> {
    const preset = presets.find((item) => item.id === provider);
    const config = settings?.providers[provider];
    if (!preset || !config) return null;
    if (!force && detectedModels[provider]?.length) return detectedModels[provider];
    if (preset.requiresKey && !config.apiKey.trim()) {
      if (notify) showToast(`Add a ${preset.label} API key to detect models.`);
      return null;
    }

    detectingModels = true;
    try {
      const models = await fetchModels(provider);
      if (models.length === 0) {
        if (notify) showToast(`${preset.label} returned no models.`);
        return null;
      }
      detectedModels = { ...detectedModels, [provider]: models };

      const currentSession = sessionId && active?.id === sessionId ? active : null;
      const currentModel =
        currentSession?.model ??
        (settings?.provider === provider ? settings?.model : settings?.providers[provider]?.model);

      if (currentModel && !models.includes(currentModel)) {
        const next = pickAvailableModel(models, preset);
        if (currentSession) {
          const result = await api.patchSession(currentSession.id, { model: next });
          active = { ...currentSession, model: result.session.model };
        }
        if (settings) {
          const existing = settings.providers[provider] ?? {
            apiKey: '',
            baseUrl: '',
            model: next,
          };
          settings.providers[provider] = { ...existing, model: next };
          if (settings.provider === provider) settings.model = next;
          await api.updateSettings(settings);
        }
        if (notify) {
          showToast(
            `Detected ${models.length} ${preset.label} models — switched to ${next}.`,
          );
        }
      } else if (notify) {
        showToast(`Detected ${models.length} ${preset.label} models.`);
      }

      return models;
    } catch (error) {
      if (notify) showToast(api.errorMessage(error));
      return null;
    } finally {
      detectingModels = false;
    }
  }

  function toUi(messagesRaw: Message[]): UiMessage[] {
    const out: UiMessage[] = [];
    let current: UiMessage | null = null;

    for (const message of messagesRaw) {
      if (message.role === 'user') {
        out.push({ id: uid(), role: 'user', content: message.content, tools: [] });
        current = null;
        continue;
      }

      if (message.role === 'assistant') {
        current = {
          id: uid(),
          role: 'assistant',
          content: message.content,
          tools: (message.toolCalls ?? []).map<UiTool>((call) => ({
            id: call.id,
            name: call.name,
            args: parseArguments(call.arguments),
            status: 'running',
          })),
        };
        out.push(current);
        continue;
      }

      if (message.role === 'tool' && current) {
        const tool =
          current.tools.find((item) => item.id === message.toolCallId) ??
          current.tools.find((item) => item.name === message.name && item.status === 'running');
        if (tool) {
          let parsed: unknown = message.content;
          try {
            parsed = JSON.parse(message.content);
          } catch {
            parsed = message.content;
          }
          tool.result = parsed;
          tool.ok = !(
            parsed &&
            typeof parsed === 'object' &&
            'ok' in parsed &&
            (parsed as { ok?: boolean }).ok === false
          );
          tool.status = tool.ok ? 'ok' : 'error';
          if (parsed && typeof parsed === 'object' && 'error' in parsed) {
            tool.error = String((parsed as { error?: unknown }).error ?? '');
          }
        }
      }
    }

    return out;
  }

  function summaryOf(session: Session): SessionSummary {
    return {
      id: session.id,
      title: session.title,
      provider: session.provider,
      model: session.model,
      createdAt: session.createdAt,
      updatedAt: session.updatedAt,
      messageCount: session.messages.length,
    };
  }

  async function boot() {
    loading = true;
    fatal = '';
    try {
      const [settingsResult, providersResult, promptsResult, sessionsResult, statusResult] =
        await Promise.all([
          api.getSettings(),
          api.getProviders(),
          api.getPrompts(),
          api.listSessions(),
          api.getStatus(),
        ]);

      settings = settingsResult.settings;
      presets = providersResult.providers;
      prompts = promptsResult.presets;
      defaultPrompt = promptsResult.default;
      sessions = sessionsResult.sessions;
      status = statusResult;

      if (sessions.length > 0) {
        await openSession(sessions[0].id);
      } else {
        await newChat();
      }
    } catch (error) {
      fatal = api.errorMessage(error);
    } finally {
      loading = false;
    }
  }

  async function refreshStatus() {
    try {
      status = await api.getStatus();
    } catch {
      // The UI keeps working with the last known status if the backend is
      // restarting. The fatal screen handles a cold start.
    }
  }

  async function refreshSessions() {
    try {
      sessions = (await api.listSessions()).sessions;
    } catch {
      // Non-critical.
    }
  }

  async function openSession(id: string) {
    if (streaming && active?.id !== id) await stop();
    try {
      const result = await api.getSession(id);
      active = result.session;
      messages = toUi(result.session.messages);
      sidebarOpen = false;
      void syncModels(result.session.provider, result.session.id);
    } catch (error) {
      showToast(api.errorMessage(error));
    }
  }

  async function newChat() {
    if (streaming) await stop();
    try {
      const provider = settings?.provider ?? 'openai';
      const model = settings?.model;
      const result = await api.createSession(provider, model);
      active = result.session;
      messages = [];
      sessions = [summaryOf(result.session), ...sessions];
      sidebarOpen = false;
      void syncModels(provider, result.session.id);
    } catch (error) {
      showToast(api.errorMessage(error));
    }
  }

  async function send(text: string) {
    if (!active || streaming) return;
    const assistantId = uid();
    messages = [
      ...messages,
      { id: uid(), role: 'user', content: text, tools: [] },
      { id: assistantId, role: 'assistant', content: '', tools: [], pending: true },
    ];
    streaming = true;
    stopping = false;
    controller = new AbortController();

    const apply = (event: ChatEvent) => {
      const assistant = messages.find((message) => message.id === assistantId);
      switch (event.type) {
        case 'delta':
          if (assistant) assistant.content += event.text;
          break;
        case 'tool_start':
          assistant?.tools.push({
            id: event.id,
            name: event.name,
            args: event.arguments,
            status: 'running',
          });
          break;
        case 'tool_result': {
          const tool = assistant?.tools.find((item) => item.id === event.id);
          if (tool) {
            tool.result = event.result;
            tool.ok = event.ok;
            tool.status = event.ok ? 'ok' : 'error';
            if (event.error) tool.error = event.error;
          }
          break;
        }
        case 'done':
          if (active) {
            active.title = event.session.title;
            active.updatedAt = event.session.updatedAt;
          }
          break;
        case 'error':
          if (assistant) assistant.error = event.message;
          break;
        case 'cancelled':
          break;
      }
    };

    try {
      await api.streamChat(
        {
          sessionId: active.id,
          content: text,
          provider: active.provider,
          model: active.model,
        },
        apply,
        controller.signal,
      );
    } catch (error) {
      if (!stopping && !(error instanceof DOMException && error.name === 'AbortError')) {
        const assistant = messages.find((message) => message.id === assistantId);
        if (assistant) assistant.error = api.errorMessage(error);
      }
    } finally {
      streaming = false;
      stopping = false;
      controller = null;
      const assistant = messages.find((message) => message.id === assistantId);
      if (assistant) assistant.pending = false;
      await refreshSessions();
      if (active) {
        const summary = sessions.find((item) => item.id === active?.id);
        if (summary) {
          active.title = summary.title;
          active.updatedAt = summary.updatedAt;
        }
      }
    }
  }

  async function stop() {
    if (!active) return;
    stopping = true;
    try {
      await api.stopChat(active.id);
    } catch {
      // Aborting the fetch is enough if the stop endpoint races.
    }
    controller?.abort();
  }

  async function renameSession(session: SessionSummary) {
    const title = window.prompt('Rename chat', session.title);
    if (!title || !title.trim()) return;
    try {
      await api.patchSession(session.id, { title: title.trim() });
      await refreshSessions();
      if (active?.id === session.id) active.title = title.trim();
    } catch (error) {
      showToast(api.errorMessage(error));
    }
  }

  async function deleteSession(session: SessionSummary) {
    if (!window.confirm(`Delete “${session.title}”? This cannot be undone.`)) return;
    if (streaming && active?.id === session.id) await stop();
    try {
      await api.deleteSession(session.id);
      sessions = sessions.filter((item) => item.id !== session.id);
      if (active?.id === session.id) {
        if (sessions.length > 0) await openSession(sessions[0].id);
        else await newChat();
      }
    } catch (error) {
      showToast(api.errorMessage(error));
    }
  }

  async function setProvider(provider: string) {
    const preset = presets.find((item) => item.id === provider);
    if (!preset || !settings) return;

    let models = detectedModels[provider];
    if (!models) {
      detectingModels = true;
      try {
        models = await fetchModels(provider);
        detectedModels = { ...detectedModels, [provider]: models };
      } catch (error) {
        if (settings.providers[provider]?.apiKey) showToast(api.errorMessage(error));
      } finally {
        detectingModels = false;
      }
    }

    const preferred = settings.providers[provider]?.model || preset.defaultModel;
    const model =
      models && models.length > 0
        ? models.includes(preferred)
          ? preferred
          : pickAvailableModel(models, preset)
        : preferred;

    try {
      if (active) {
        const result = await api.patchSession(active.id, { provider, model });
        active.provider = result.session.provider;
        active.model = result.session.model;
      }
      const existing = settings.providers[provider] ?? {
        apiKey: '',
        baseUrl: preset.defaultBaseUrl,
        model,
      };
      settings.providers[provider] = { ...existing, model };
      settings.provider = provider as SettingsType['provider'];
      settings.model = model;
      await api.updateSettings(settings);
      await refreshSessions();
      if (models?.length) {
        showToast(`Detected ${models.length} ${preset.label} models — using ${model}.`);
      }
    } catch (error) {
      showToast(api.errorMessage(error));
    }
  }

  async function setModel(model: string) {
    if (!active || !model.trim()) return;
    const next = model.trim();
    try {
      const result = await api.patchSession(active.id, { model: next });
      active.model = result.session.model;
      if (settings) {
        const provider = result.session.provider;
        const existing = settings.providers[provider] ?? {
          apiKey: '',
          baseUrl: '',
          model: next,
        };
        settings.providers[provider] = { ...existing, model: next };
        settings.model = next;
        await api.updateSettings(settings);
      }
    } catch (error) {
      showToast(api.errorMessage(error));
    }
  }

  async function refreshModels() {
    await syncModels(activeProvider, active?.id, true, true);
  }

  async function saveSettings(next: SettingsType) {
    const result = await api.updateSettings(next);
    settings = result.settings;

    const provider = result.settings.provider;
    detectedModels = {};

    const preset = presets.find((item) => item.id === provider);
    const config = result.settings.providers[provider];
    const canDetect = Boolean(config && (!preset?.requiresKey || config.apiKey.trim()));
    if (canDetect) {
      void syncModels(
        provider,
        active?.provider === provider ? active.id : undefined,
        true,
        true,
      );
    }
    showToast('Settings saved');
  }

  async function installPlugin(): Promise<string> {
    const result = await api.installPlugin();
    return `Installed to ${result.path}`;
  }

  onMount(() => {
    void boot();
    pollTimer = window.setInterval(refreshStatus, 4000);
    return () => {
      if (pollTimer) window.clearInterval(pollTimer);
      controller?.abort();
    };
  });
</script>

{#if loading}
  <div class="boot-screen">
    <div class="boot-logo">B</div>
    <div class="boot-text">Starting BFish…</div>
  </div>
{:else if fatal}
  <div class="boot-screen">
    <div class="boot-logo error">!</div>
    <h1>BFish could not reach its local backend</h1>
    <p>{fatal}</p>
    <p class="boot-hint">Start the app again, or run <code>cargo run -p bfish-backend</code> for development.</p>
    <button class="retry" onclick={boot}>Try again</button>
  </div>
{:else}
  <div class="app-shell">
    <Sidebar
      sessions={sessions}
      activeId={active?.id ?? null}
      open={sidebarOpen}
      studioConnected={studioConnected}
      placeName={placeName}
      onnew={newChat}
      onselect={openSession}
      onrename={renameSession}
      ondelete={deleteSession}
      onsettings={() => (settingsOpen = true)}
      onclose={() => (sidebarOpen = false)}
    />

    {#if sidebarOpen}
      <button class="sidebar-scrim" aria-label="Close menu" onclick={() => (sidebarOpen = false)}></button>
    {/if}

    <main class="main">
      <header class="topbar">
        <button class="mobile-menu" title="Open menu" onclick={() => (sidebarOpen = true)}>
          <svg viewBox="0 0 24 24"><path d="M4 7h16M4 12h16M4 17h16" /></svg>
        </button>

        <div class="model-control">
          <select
            value={active?.provider ?? settings?.provider}
            onchange={(event) => setProvider(event.currentTarget.value)}
            title="Provider"
          >
            {#each presets as item}
              <option value={item.id}>{item.label}</option>
            {/each}
          </select>
          <input
            class="model-input"
            value={active?.model ?? settings?.model}
            list="top-models"
            spellcheck="false"
            title={detectingModels ? 'Detecting models…' : 'Model'}
            onchange={(event) => setModel(event.currentTarget.value)}
          />
          <button
            class="model-refresh"
            title="Detect available models from the provider"
            onclick={refreshModels}
            disabled={detectingModels}
          >
            <svg class:spin={detectingModels} viewBox="0 0 24 24">
              <path d="M20 11a8 8 0 1 0-2.3 5.7M20 5v6h-6" />
            </svg>
          </button>
          <datalist id="top-models">
            {#each availableModels as model}
              <option value={model}></option>
            {/each}
          </datalist>
        </div>

        <button
          class="studio-chip"
          class:connected={studioConnected}
          onclick={() => (settingsOpen = true)}
          title="Studio connection"
        >
          <span class="status-dot"></span>
          <span>{studioConnected ? placeName : 'Studio offline'}</span>
        </button>

        <button class="topbar-button" title="Settings" onclick={() => (settingsOpen = true)}>
          <svg viewBox="0 0 24 24">
            <circle cx="12" cy="12" r="3" />
            <path d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1 1.55V21a2 2 0 1 1-4 0v-.09a1.7 1.7 0 0 0-1.11-1.55 1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-1.55-1H3a2 2 0 1 1 0-4h.09A1.7 1.7 0 0 0 4.6 8.9a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1-1.55V3a2 2 0 1 1 4 0v.09a1.7 1.7 0 0 0 1 1.55 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.4 9c.14.37.5.64.91.69H21a2 2 0 1 1 0 4h-.09a1.7 1.7 0 0 0-1.51 1.31z" />
          </svg>
        </button>
      </header>

      <ChatView
        {messages}
        {streaming}
        {modelLabel}
        onsend={send}
        onstop={stop}
      />
    </main>
  </div>
{/if}

{#if settingsOpen && settings}
  <Settings
    {settings}
    {presets}
    {prompts}
    {defaultPrompt}
    {status}
    onclose={() => (settingsOpen = false)}
    onsave={saveSettings}
    oninstall={installPlugin}
  />
{/if}

{#if toast}
  <div class="toast">{toast}</div>
{/if}
