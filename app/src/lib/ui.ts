export interface UiTool {
  id: string;
  name: string;
  args?: unknown;
  result?: unknown;
  ok?: boolean;
  status: 'running' | 'ok' | 'error';
  error?: string;
}

export interface UiMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  tools: UiTool[];
  pending?: boolean;
  error?: string;
}
