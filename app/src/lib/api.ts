export type ProviderId = 'openai' | 'anthropic' | 'gemini' | 'xai' | 'openrouter' | 'ollama';

export interface ProviderConfig {
  apiKey: string;
  baseUrl: string;
  model: string;
}

export interface Settings {
  provider: ProviderId;
  model: string;
  temperature: number;
  maxTokens: number;
  systemPrompt: string;
  providers: Record<string, ProviderConfig>;
  mcpPort: number;
  autoInstallPlugin: boolean;
  onboarded: boolean;
}

export interface ProviderPreset {
  id: ProviderId;
  label: string;
  defaultBaseUrl: string;
  defaultModel: string;
  keyUrl: string;
  requiresKey: boolean;
  models: string[];
  note: string;
}

export interface PromptPreset {
  id: string;
  name: string;
  description: string;
  prompt: string;
}

export interface ToolCall {
  id: string;
  name: string;
  arguments: string;
}

export interface Message {
  role: 'user' | 'assistant' | 'tool';
  content: string;
  toolCalls?: ToolCall[];
  toolCallId?: string;
  name?: string;
}

export interface SessionSummary {
  id: string;
  title: string;
  provider: ProviderId;
  model: string;
  createdAt: string;
  updatedAt: string;
  messageCount: number;
}

export interface Session {
  id: string;
  title: string;
  provider: ProviderId;
  model: string;
  createdAt: string;
  updatedAt: string;
  messages: Message[];
}

export interface StudioSessionInfo {
  id: string;
  pluginVersion: string;
  placeName: string;
  placeId: number;
  studioVersion: string;
  role: string;
  connectedAt: string;
  lastSeen: string;
}

export interface StudioLog {
  at: string;
  level: string;
  message: string;
}

export interface BridgeStatus {
  connected: boolean;
  port: number;
  sessions: StudioSessionInfo[];
  calls: number;
  recentLogs: StudioLog[];
}

export interface McpStatus extends BridgeStatus {
  error: string | null;
  mcpUrl: string;
}

export interface StatusResponse {
  version: string;
  provider: string;
  model: string;
  studio: BridgeStatus;
  bridgeError: string | null;
}

export type ChatEvent =
  | { type: 'start'; sessionId: string; title: string; provider: string; model: string }
  | { type: 'delta'; text: string }
  | { type: 'tool_start'; id: string; name: string; arguments: unknown }
  | { type: 'tool_result'; id: string; name: string; ok: boolean; result?: unknown; error?: string }
  | { type: 'done'; session: SessionSummary; usage: { inputTokens: number; outputTokens: number } }
  | { type: 'cancelled' }
  | { type: 'error'; message: string };

export const API_BASE =
  (import.meta.env.VITE_BFISH_API as string | undefined) ?? 'http://127.0.0.1:8765';

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const response = await fetch(`${API_BASE}${path}`, {
    ...init,
    headers: {
      'Content-Type': 'application/json',
      ...(init.headers ?? {}),
    },
  });
  const text = await response.text();
  let data: Record<string, unknown> = {};
  if (text) {
    try {
      data = JSON.parse(text);
    } catch {
      data = { error: text };
    }
  }
  if (!response.ok) {
    throw new Error(
      typeof data.error === 'string' ? data.error : `Request failed (${response.status})`,
    );
  }
  return data as T;
}

export function getSettings() {
  return request<{ settings: Settings }>('/api/settings');
}

export function updateSettings(settings: Settings) {
  return request<{ ok: boolean; settings: Settings }>('/api/settings', {
    method: 'PUT',
    body: JSON.stringify(settings),
  });
}

export function getProviders() {
  return request<{ providers: ProviderPreset[]; saved: Record<string, ProviderConfig> }>(
    '/api/providers',
  );
}

export function getPrompts() {
  return request<{ default: string; presets: PromptPreset[]; current: string }>('/api/prompts');
}

export function testProvider(body: {
  provider: string;
  apiKey: string;
  baseUrl: string;
  model: string;
}) {
  return request<{ ok: boolean; models: string[] }>('/api/providers/test', {
    method: 'POST',
    body: JSON.stringify(body),
  });
}

export function getProviderModels(provider: string) {
  return request<{ ok: boolean; models: string[] }>(
    `/api/providers/models?provider=${encodeURIComponent(provider)}`,
  );
}

export function listSessions() {
  return request<{ sessions: SessionSummary[] }>('/api/sessions');
}

export function createSession(provider?: string, model?: string) {
  return request<{ ok: boolean; session: Session }>('/api/sessions', {
    method: 'POST',
    body: JSON.stringify({ provider, model }),
  });
}

export function getSession(id: string) {
  return request<{ session: Session }>(`/api/sessions/${encodeURIComponent(id)}`);
}

export function patchSession(
  id: string,
  patch: { title?: string; provider?: string; model?: string },
) {
  return request<{ ok: boolean; session: Session }>(
    `/api/sessions/${encodeURIComponent(id)}`,
    {
      method: 'PATCH',
      body: JSON.stringify(patch),
    },
  );
}

export function deleteSession(id: string) {
  return request<{ ok: boolean }>(`/api/sessions/${encodeURIComponent(id)}`, {
    method: 'DELETE',
  });
}

export function getStatus() {
  return request<StatusResponse>('/api/status');
}

export function getMcpStatus() {
  return request<McpStatus>('/api/mcp/status');
}

export function installPlugin() {
  return request<{ ok: boolean; path: string }>('/api/mcp/install-plugin', { method: 'POST' });
}

export function stopChat(sessionId: string) {
  return request<{ ok: boolean; stopped: boolean }>('/api/chat/stop', {
    method: 'POST',
    body: JSON.stringify({ sessionId }),
  });
}

export async function streamChat(
  body: { sessionId: string; content: string; provider?: string; model?: string },
  onEvent: (event: ChatEvent) => void,
  signal: AbortSignal,
): Promise<void> {
  const response = await fetch(`${API_BASE}/api/chat`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
    signal,
  });

  if (!response.ok) {
    const text = await response.text();
    let message = `Chat failed (${response.status})`;
    try {
      const parsed = JSON.parse(text);
      if (typeof parsed.error === 'string') message = parsed.error;
    } catch {
      if (text) message = text;
    }
    throw new Error(message);
  }
  if (!response.body) {
    throw new Error('This webview does not support streaming responses.');
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    const parts = buffer.split(/\r?\n\r?\n/);
    buffer = parts.pop() ?? '';

    for (const part of parts) {
      const data = part
        .split(/\r?\n/)
        .filter((line) => line.startsWith('data:'))
        .map((line) => line.slice(5).trimStart())
        .join('\n');
      if (!data || data === 'bfish-keepalive') continue;
      try {
        onEvent(JSON.parse(data) as ChatEvent);
      } catch {
        // Ignore malformed keepalive fragments.
      }
    }
  }
}

export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}
