<script lang="ts">
  import type { SessionSummary } from '../lib/api';

  let {
    sessions,
    activeId,
    open,
    studioConnected,
    placeName,
    onnew,
    onselect,
    onrename,
    ondelete,
    onsettings,
    onclose,
  }: {
    sessions: SessionSummary[];
    activeId: string | null;
    open: boolean;
    studioConnected: boolean;
    placeName: string;
    onnew: () => void;
    onselect: (id: string) => void;
    onrename: (session: SessionSummary) => void;
    ondelete: (session: SessionSummary) => void;
    onsettings: () => void;
    onclose: () => void;
  } = $props();

  function groupFor(iso: string): string {
    const date = new Date(iso);
    const today = new Date();
    const start = (value: Date) =>
      new Date(value.getFullYear(), value.getMonth(), value.getDate()).getTime();
    const days = Math.floor((start(today) - start(date)) / 86_400_000);
    if (days <= 0) return 'Today';
    if (days === 1) return 'Yesterday';
    if (days < 7) return 'Previous 7 days';
    if (days < 30) return 'Previous 30 days';
    return 'Older';
  }

  const grouped = $derived.by(() => {
    const order = ['Today', 'Yesterday', 'Previous 7 days', 'Previous 30 days', 'Older'];
    const map = new Map<string, SessionSummary[]>();
    for (const session of sessions) {
      const key = groupFor(session.updatedAt);
      const list = map.get(key) ?? [];
      list.push(session);
      map.set(key, list);
    }
    return order
      .filter((key) => map.has(key))
      .map((key) => ({ label: key, items: map.get(key) ?? [] }));
  });
</script>

<aside class="sidebar" class:open>
  <div class="sidebar-top">
    <div class="brand">
      <svg viewBox="0 0 64 64" aria-hidden="true">
        <rect width="64" height="64" rx="16" fill="var(--accent)" />
        <path d="M14 32c8-11 20-14 30-8l6-6v14l-6 6c-10 6-22 3-30-6z" fill="#082f2b" />
        <circle cx="37" cy="29" r="3" fill="#2dd4bf" />
        <path d="M14 38c6 4 13 5 20 2" stroke="#082f2b" stroke-width="3" fill="none" stroke-linecap="round" />
      </svg>
      <span>BFish</span>
    </div>
    <button class="icon-button mobile-only" title="Close menu" onclick={onclose}>
      <svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18" /></svg>
    </button>
  </div>

  <button class="new-chat" onclick={onnew}>
    <svg viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" /></svg>
    <span>New chat</span>
  </button>

  <div class="session-scroll">
    {#if sessions.length === 0}
      <p class="sidebar-empty">No chats yet. Start by describing what you want to build.</p>
    {/if}
    {#each grouped as group}
      <div class="session-group">
        <div class="session-group-label">{group.label}</div>
        {#each group.items as session}
          <button
            class="session-item"
            class:active={session.id === activeId}
            onclick={() => onselect(session.id)}
            title={session.title}
          >
            <span class="session-title">{session.title}</span>
            <span class="session-actions">
              <span
                class="mini-action"
                role="button"
                tabindex="0"
                title="Rename"
                onclick={(event) => {
                  event.stopPropagation();
                  onrename(session);
                }}
                onkeydown={(event) => {
                  if (event.key === 'Enter') {
                    event.stopPropagation();
                    onrename(session);
                  }
                }}
              >
                <svg viewBox="0 0 24 24"><path d="M4 20l4-1 10-10-3-3L5 16l-1 4zM14 6l3 3" /></svg>
              </span>
              <span
                class="mini-action"
                role="button"
                tabindex="0"
                title="Delete"
                onclick={(event) => {
                  event.stopPropagation();
                  ondelete(session);
                }}
                onkeydown={(event) => {
                  if (event.key === 'Enter') {
                    event.stopPropagation();
                    ondelete(session);
                  }
                }}
              >
                <svg viewBox="0 0 24 24"><path d="M5 7h14M9 7V5h6v2M8 7l1 12h6l1-12" /></svg>
              </span>
            </span>
          </button>
        {/each}
      </div>
    {/each}
  </div>

  <div class="sidebar-footer">
    <div class="studio-pill" class:connected={studioConnected}>
      <span class="status-dot"></span>
      <span class="studio-text">
        <strong>{studioConnected ? 'Studio connected' : 'Studio offline'}</strong>
        <small>{studioConnected ? placeName : 'Open the connector plugin'}</small>
      </span>
    </div>
    <button class="settings-button" onclick={onsettings}>
      <svg viewBox="0 0 24 24">
        <circle cx="12" cy="12" r="3" />
        <path d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1 1.55V21a2 2 0 1 1-4 0v-.09a1.7 1.7 0 0 0-1.11-1.55 1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-1.55-1H3a2 2 0 1 1 0-4h.09A1.7 1.7 0 0 0 4.6 8.9a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1-1.55V3a2 2 0 1 1 4 0v.09a1.7 1.7 0 0 0 1 1.55 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.4 9c.14.37.5.64.91.69H21a2 2 0 1 1 0 4h-.09a1.7 1.7 0 0 0-1.51 1.31z" />
      </svg>
      <span>Settings & API keys</span>
    </button>
  </div>
</aside>

<style>
  .sidebar {
    width: 268px;
    min-width: 268px;
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--sidebar);
    border-right: 1px solid var(--border);
  }

  .sidebar-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 16px 10px;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    font-weight: 700;
    font-size: 18px;
    letter-spacing: -0.02em;
  }

  .brand svg {
    width: 28px;
    height: 28px;
    border-radius: 8px;
  }

  .new-chat {
    margin: 6px 12px 10px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: transparent;
    color: var(--text);
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: background 120ms ease, border-color 120ms ease;
  }

  .new-chat:hover {
    background: var(--hover);
    border-color: var(--border-strong);
  }

  .new-chat svg {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
  }

  .session-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 4px 8px 12px;
  }

  .session-group {
    margin-top: 10px;
  }

  .session-group-label {
    padding: 6px 10px;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.09em;
    color: var(--muted);
  }

  .session-item {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 9px 10px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--text-soft);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }

  .session-item:hover {
    background: var(--hover);
    color: var(--text);
  }

  .session-item.active {
    background: var(--active);
    color: var(--text);
  }

  .session-title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .session-actions {
    display: flex;
    gap: 2px;
    opacity: 0;
    transition: opacity 120ms ease;
  }

  .session-item:hover .session-actions,
  .session-item.active .session-actions {
    opacity: 1;
  }

  .mini-action {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 6px;
    color: var(--muted);
  }

  .mini-action:hover {
    background: var(--border);
    color: var(--text);
  }

  .mini-action svg {
    width: 13px;
    height: 13px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .sidebar-empty {
    padding: 12px 10px;
    font-size: 12px;
    line-height: 1.5;
    color: var(--muted);
  }

  .sidebar-footer {
    border-top: 1px solid var(--border);
    padding: 10px;
  }

  .studio-pill {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 9px 10px;
    border-radius: 8px;
    background: var(--panel);
    margin-bottom: 6px;
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #64748b;
    box-shadow: 0 0 0 3px rgba(100, 116, 139, 0.12);
    flex: none;
  }

  .studio-pill.connected .status-dot {
    background: var(--accent);
    box-shadow: 0 0 0 3px rgba(45, 212, 191, 0.14);
  }

  .studio-text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .studio-text strong {
    font-size: 11px;
    font-weight: 600;
  }

  .studio-text small {
    font-size: 10px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .settings-button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 10px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--text-soft);
    font-size: 12px;
    cursor: pointer;
  }

  .settings-button:hover {
    background: var(--hover);
    color: var(--text);
  }

  .settings-button svg {
    width: 15px;
    height: 15px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .icon-button {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--text-soft);
    cursor: pointer;
  }

  .icon-button svg {
    width: 18px;
    height: 18px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
  }

  .mobile-only {
    display: none;
  }

  @media (max-width: 860px) {
    .sidebar {
      position: fixed;
      inset: 0 auto 0 0;
      z-index: 40;
      transform: translateX(-102%);
      transition: transform 180ms ease;
      box-shadow: 24px 0 60px rgba(0, 0, 0, 0.35);
    }

    .sidebar.open {
      transform: translateX(0);
    }

    .mobile-only {
      display: grid;
    }
  }
</style>
