<script lang="ts">
  import { renderMarkdown } from '../lib/markdown';
  import type { UiMessage, UiTool } from '../lib/ui';

  let {
    messages,
    streaming,
    modelLabel,
    onsend,
    onstop,
  }: {
    messages: UiMessage[];
    streaming: boolean;
    modelLabel: string;
    onsend: (text: string) => void;
    onstop: () => void;
  } = $props();

  let input = $state('');
  let textarea = $state<HTMLTextAreaElement | null>(null);
  let scroller = $state<HTMLDivElement | null>(null);
  let sticky = true;

  const suggestions = [
    { title: 'Build a sword arena', text: 'Build a round-based sword arena with a lobby, spawn pads, and a scoreboard.' },
    { title: 'Fix a DataStore script', text: 'Inspect my DataStore script and fix any data-loss bugs you find.' },
    { title: 'Create a shop UI', text: 'Create a clean, mobile-friendly shop UI with three item cards and a purchase button.' },
    { title: 'Find lag spikes', text: 'Profile this place and find the most likely causes of lag. Do not change anything yet.' },
  ];

  function submit() {
    const text = input.trim();
    if (!text || streaming) return;
    onsend(text);
    input = '';
    if (textarea) {
      textarea.style.height = 'auto';
      textarea.focus();
    }
  }

  function onKeyDown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      submit();
    }
  }

  function onScroll() {
    if (!scroller) return;
    sticky = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 80;
  }

  function copyCode(node: HTMLElement) {
    const handler = (event: MouseEvent) => {
      const target = event.target as HTMLElement | null;
      const button = target?.closest('.copy-code') as HTMLElement | null;
      if (!button) return;
      const code = button.closest('.code-block')?.querySelector('code')?.textContent ?? '';
      navigator.clipboard.writeText(code).then(() => {
        button.textContent = 'Copied';
        window.setTimeout(() => (button.textContent = 'Copy'), 1200);
      });
    };
    node.addEventListener('click', handler);
    return {
      destroy() {
        node.removeEventListener('click', handler);
      },
    };
  }

  function formatValue(value: unknown): string {
    if (value === undefined) return '';
    if (typeof value === 'string') {
      return value.length > 8000 ? `${value.slice(0, 8000)}\n…` : value;
    }
    try {
      const text = JSON.stringify(value, null, 2);
      return text.length > 8000 ? `${text.slice(0, 8000)}\n…` : text;
    } catch {
      return String(value);
    }
  }

  function toolLabel(tool: UiTool): string {
    if (tool.status === 'running') return 'Running';
    if (tool.status === 'error') return 'Failed';
    return 'Done';
  }

  $effect(() => {
    const element = textarea;
    if (!element) return;
    element.style.height = 'auto';
    element.style.height = `${Math.min(element.scrollHeight, 200)}px`;
  });

  $effect(() => {
    // Read every message's mutable fields so streaming updates keep scrolling.
    const signal = messages
      .map((message) => `${message.content.length}:${message.tools.length}`)
      .join('|');
    if (sticky && scroller) {
      void signal;
      scroller.scrollTop = scroller.scrollHeight;
    }
  });
</script>

<div class="chat-view" use:copyCode>
  <div class="chat-scroll" bind:this={scroller} onscroll={onScroll}>
    <div class="chat-inner">
      {#if messages.length === 0}
        <div class="empty-state">
          <div class="empty-logo">
            <svg viewBox="0 0 64 64" aria-hidden="true">
              <rect width="64" height="64" rx="16" fill="var(--accent)" />
              <path d="M14 32c8-11 20-14 30-8l6-6v14l-6 6c-10 6-22 3-30-6z" fill="#082f2b" />
              <circle cx="37" cy="29" r="3" fill="#2dd4bf" />
              <path d="M14 38c6 4 13 5 20 2" stroke="#082f2b" stroke-width="3" fill="none" stroke-linecap="round" />
            </svg>
          </div>
          <h1>What should we build?</h1>
          <p>BFish can inspect your place, write Luau, create instances, and verify changes in Studio.</p>
          <div class="suggestions">
            {#each suggestions as suggestion}
              <button class="suggestion" onclick={() => onsend(suggestion.text)}>
                <strong>{suggestion.title}</strong>
                <span>{suggestion.text}</span>
              </button>
            {/each}
          </div>
        </div>
      {/if}

      {#each messages as message (message.id)}
        <article class="message {message.role}" class:pending={message.pending}>
          <div class="avatar" aria-hidden="true">
            {#if message.role === 'assistant'}
              <span>B</span>
            {:else}
              <span>You</span>
            {/if}
          </div>
          <div class="message-main">
            <div class="message-role">{message.role === 'assistant' ? 'BFish' : 'You'}</div>
            {#if message.content}
              <div class="prose">
                {@html renderMarkdown(message.content)}
              </div>
            {:else if message.pending && message.tools.length === 0}
              <div class="typing"><span></span><span></span><span></span></div>
            {/if}

            {#if message.tools.length > 0}
              <div class="tool-list">
                {#each message.tools as tool}
                  <details class="tool-card" class:failed={tool.status === 'error'}>
                    <summary>
                      <span class="tool-icon {tool.status}">
                        {#if tool.status === 'running'}
                          <span class="spinner"></span>
                        {:else if tool.status === 'error'}
                          <svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18" /></svg>
                        {:else}
                          <svg viewBox="0 0 24 24"><path d="M5 13l4 4L19 7" /></svg>
                        {/if}
                      </span>
                      <span class="tool-name">{tool.name}</span>
                      <span class="tool-status">{toolLabel(tool)}</span>
                      <svg class="chevron" viewBox="0 0 24 24"><path d="M6 9l6 6 6-6" /></svg>
                    </summary>
                    <div class="tool-body">
                      {#if tool.args !== undefined}
                        <div class="tool-section">
                          <span class="tool-section-label">Arguments</span>
                          <pre>{formatValue(tool.args)}</pre>
                        </div>
                      {/if}
                      {#if tool.result !== undefined}
                        <div class="tool-section">
                          <span class="tool-section-label">Result</span>
                          <pre>{formatValue(tool.result)}</pre>
                        </div>
                      {/if}
                      {#if tool.error}
                        <div class="tool-section error">
                          <span class="tool-section-label">Error</span>
                          <pre>{tool.error}</pre>
                        </div>
                      {/if}
                    </div>
                  </details>
                {/each}
              </div>
            {/if}

            {#if message.error}
              <div class="message-error">{message.error}</div>
            {/if}
          </div>
        </article>
      {/each}
    </div>
  </div>

  <div class="composer-wrap">
    <div class="composer">
      <textarea
        bind:this={textarea}
        bind:value={input}
        onkeydown={onKeyDown}
        placeholder="Ask BFish to build, debug, or explain something in your place…"
        rows="1"
        disabled={streaming}
      ></textarea>
      <div class="composer-row">
        <span class="composer-hint">
          {modelLabel}
          <span class="dot-sep">·</span>
          Enter to send
        </span>
        {#if streaming}
          <button class="send-button stop" title="Stop generating" onclick={onstop}>
            <span></span>
          </button>
        {:else}
          <button class="send-button" title="Send" onclick={submit} disabled={!input.trim()}>
            <svg viewBox="0 0 24 24"><path d="M12 19V5M6 11l6-6 6 6" /></svg>
          </button>
        {/if}
      </div>
    </div>
    <p class="composer-disclaimer">
      BFish can make mistakes. Studio edits are undoable with Ctrl+Z.
    </p>
  </div>
</div>

<style>
  .chat-view {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .chat-scroll {
    flex: 1;
    overflow-y: auto;
    scroll-behavior: smooth;
  }

  .chat-inner {
    width: 100%;
    max-width: 800px;
    margin: 0 auto;
    padding: 20px 28px 8px;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    padding: 8vh 0 24px;
  }

  .empty-logo svg {
    width: 54px;
    height: 54px;
    border-radius: 15px;
    box-shadow: 0 12px 40px rgba(45, 212, 191, 0.18);
  }

  .empty-state h1 {
    margin: 22px 0 8px;
    font-size: 28px;
    font-weight: 650;
    letter-spacing: -0.03em;
  }

  .empty-state p {
    margin: 0;
    max-width: 460px;
    color: var(--muted);
    font-size: 14px;
    line-height: 1.6;
  }

  .suggestions {
    margin-top: 30px;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
    width: 100%;
    max-width: 620px;
  }

  .suggestion {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 13px 14px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--panel);
    color: var(--text-soft);
    text-align: left;
    cursor: pointer;
    transition: border-color 120ms ease, background 120ms ease, transform 120ms ease;
  }

  .suggestion:hover {
    background: var(--hover);
    border-color: var(--border-strong);
    transform: translateY(-1px);
  }

  .suggestion strong {
    font-size: 12px;
    color: var(--text);
  }

  .suggestion span {
    font-size: 11px;
    line-height: 1.45;
    color: var(--muted);
  }

  .message {
    display: flex;
    gap: 14px;
    padding: 20px 0;
  }

  .message + .message {
    border-top: 1px solid var(--border-soft);
  }

  .avatar {
    flex: none;
    width: 30px;
    height: 30px;
    border-radius: 9px;
    display: grid;
    place-items: center;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: -0.02em;
    color: #e7fffb;
    background: linear-gradient(145deg, #14b8a6, #0f766e);
  }

  .message.user .avatar {
    background: #3a3d47;
    color: #dfe3ea;
  }

  .message-main {
    flex: 1;
    min-width: 0;
  }

  .message-role {
    margin: 4px 0 7px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-soft);
    letter-spacing: 0.01em;
  }

  .prose {
    font-size: 14px;
    line-height: 1.68;
    color: var(--text);
    overflow-wrap: anywhere;
  }

  .prose :global(p) {
    margin: 0 0 12px;
  }

  .prose :global(p:last-child) {
    margin-bottom: 0;
  }

  .prose :global(h2),
  .prose :global(h3),
  .prose :global(h4) {
    margin: 18px 0 8px;
    font-size: 15px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }

  .prose :global(ul),
  .prose :global(ol) {
    margin: 0 0 12px;
    padding-left: 20px;
  }

  .prose :global(li) {
    margin: 3px 0;
  }

  .prose :global(code) {
    padding: 2px 5px;
    border-radius: 5px;
    background: #2b2d35;
    font-family: var(--mono);
    font-size: 12px;
    color: #d5f5ef;
  }

  .prose :global(a) {
    color: var(--accent);
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .prose :global(blockquote) {
    margin: 0 0 12px;
    padding-left: 12px;
    border-left: 2px solid var(--border-strong);
    color: var(--muted);
  }

  .prose :global(.code-block) {
    margin: 12px 0;
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
    background: #16171c;
  }

  .prose :global(.code-block-head) {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 7px 11px;
    border-bottom: 1px solid var(--border);
    font-family: var(--mono);
    font-size: 10px;
    color: var(--muted);
    background: #1c1d23;
  }

  .prose :global(.copy-code) {
    border: 0;
    background: transparent;
    color: var(--muted);
    font-size: 10px;
    cursor: pointer;
    padding: 2px 4px;
  }

  .prose :global(.copy-code:hover) {
    color: var(--text);
  }

  .prose :global(.code-block pre) {
    margin: 0;
    padding: 12px;
    overflow-x: auto;
  }

  .prose :global(.code-block code) {
    padding: 0;
    background: transparent;
    font-size: 12px;
    line-height: 1.6;
    color: #d8e2ea;
  }

  .typing {
    display: flex;
    gap: 4px;
    padding: 8px 2px;
  }

  .typing span {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--muted);
    animation: bounce 1.1s infinite ease-in-out;
  }

  .typing span:nth-child(2) {
    animation-delay: 0.14s;
  }

  .typing span:nth-child(3) {
    animation-delay: 0.28s;
  }

  @keyframes bounce {
    0%, 60%, 100% { transform: translateY(0); opacity: 0.45; }
    30% { transform: translateY(-3px); opacity: 1; }
  }

  .tool-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 12px 0 4px;
  }

  .tool-card {
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--panel);
    font-size: 12px;
    overflow: hidden;
  }

  .tool-card summary {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 11px;
    cursor: pointer;
    list-style: none;
    user-select: none;
  }

  .tool-card summary::-webkit-details-marker {
    display: none;
  }

  .tool-icon {
    width: 16px;
    height: 16px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    flex: none;
    color: var(--accent);
  }

  .tool-icon svg {
    width: 11px;
    height: 11px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .tool-icon.error {
    color: #f87171;
  }

  .tool-name {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--text-soft);
  }

  .tool-status {
    margin-left: auto;
    color: var(--muted);
    font-size: 10px;
  }

  .chevron {
    width: 13px;
    height: 13px;
    fill: none;
    stroke: var(--muted);
    stroke-width: 2;
    stroke-linecap: round;
    transition: transform 120ms ease;
  }

  .tool-card[open] .chevron {
    transform: rotate(180deg);
  }

  .spinner {
    width: 9px;
    height: 9px;
    border: 1.5px solid rgba(45, 212, 191, 0.3);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .tool-body {
    border-top: 1px solid var(--border);
    padding: 8px 11px 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .tool-section-label {
    display: block;
    margin-bottom: 4px;
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
  }

  .tool-section pre {
    margin: 0;
    max-height: 260px;
    overflow: auto;
    padding: 8px;
    border-radius: 6px;
    background: #17181d;
    font-family: var(--mono);
    font-size: 10.5px;
    line-height: 1.5;
    color: #c7d2dd;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .tool-section.error pre {
    color: #fca5a5;
  }

  .message-error {
    margin-top: 10px;
    padding: 10px 12px;
    border: 1px solid rgba(248, 113, 113, 0.28);
    border-radius: 8px;
    background: rgba(248, 113, 113, 0.07);
    color: #fca5a5;
    font-size: 12px;
    line-height: 1.5;
  }

  .composer-wrap {
    padding: 10px 28px 16px;
    background: linear-gradient(180deg, transparent, var(--bg) 28%);
  }

  .composer {
    max-width: 800px;
    margin: 0 auto;
    padding: 12px 12px 9px;
    border: 1px solid var(--border-strong);
    border-radius: 16px;
    background: var(--panel);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.22);
  }

  .composer:focus-within {
    border-color: rgba(45, 212, 191, 0.45);
  }

  textarea {
    width: 100%;
    min-height: 26px;
    max-height: 200px;
    padding: 2px 4px;
    border: 0;
    outline: 0;
    resize: none;
    background: transparent;
    color: var(--text);
    font-family: inherit;
    font-size: 14px;
    line-height: 1.55;
  }

  textarea::placeholder {
    color: var(--muted);
  }

  .composer-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding-top: 4px;
  }

  .composer-hint {
    font-size: 10px;
    color: var(--muted);
  }

  .dot-sep {
    margin: 0 3px;
    opacity: 0.6;
  }

  .send-button {
    width: 31px;
    height: 31px;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: 50%;
    background: var(--text);
    color: #16171c;
    cursor: pointer;
    transition: opacity 120ms ease, transform 120ms ease;
  }

  .send-button:hover:not(:disabled) {
    transform: scale(1.04);
  }

  .send-button:disabled {
    opacity: 0.25;
    cursor: default;
  }

  .send-button svg {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2.2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .send-button.stop {
    background: #f3f4f6;
  }

  .send-button.stop span {
    width: 9px;
    height: 9px;
    border-radius: 2px;
    background: #16171c;
  }

  .composer-disclaimer {
    margin: 8px 0 0;
    text-align: center;
    font-size: 10px;
    color: #71717a;
  }

  @media (max-width: 720px) {
    .chat-inner {
      padding: 12px 16px 4px;
    }

    .composer-wrap {
      padding: 8px 14px 12px;
    }

    .suggestions {
      grid-template-columns: 1fr;
    }

    .empty-state h1 {
      font-size: 24px;
    }
  }
</style>
