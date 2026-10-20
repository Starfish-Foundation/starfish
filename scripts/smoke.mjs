#!/usr/bin/env node
// End-to-end smoke test: starts the backend, simulates the Studio connector,
// calls MCP tools over HTTP, and checks the chat error path.
import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const BACKEND_PORT = 8799;
const BRIDGE_PORT = 34999;
const MOCK_PORT = 8801;
const BACKEND = `http://127.0.0.1:${BACKEND_PORT}`;
const BRIDGE = `http://127.0.0.1:${BRIDGE_PORT}`;
const ROOT = new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1');
const BIN = process.env.BFISH_BACKEND_BIN ||
  join(ROOT, 'target', 'debug', process.platform === 'win32' ? 'bfish-backend.exe' : 'bfish-backend');

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

async function waitFor(url, timeoutMs = 15000) {
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    try {
      const response = await fetch(url);
      if (response.ok) return await response.json();
    } catch {
      // not up yet
    }
    await sleep(200);
  }
  throw new Error(`Timed out waiting for ${url}`);
}

async function post(path, body) {
  const response = await fetch(`${BRIDGE}${path}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  return response.json();
}

const state = {
  alive: true,
  session: null,
  commands: [],
  plugin: null,
};

function handleCommand(command) {
  state.commands.push(command);
  const data = command.data ?? {};
  switch (command.endpoint) {
    case '/api/place-info':
      return {
        placeName: 'BFish Smoke Place',
        placeId: 123456,
        gameId: 654321,
        workspaceName: 'Workspace',
      };
    case '/api/file-tree':
      return {
        tree: { name: 'game', className: 'DataModel', path: 'game', children: [] },
        rootPath: 'game',
      };
    case '/api/set-properties':
      return {
        path: data.path,
        changed: Object.keys(data.properties ?? {}),
        count: Object.keys(data.properties ?? {}).length,
      };
    case '/api/execute-luau':
      return { returnValue: 'smoke-ok', output: ['smoke'], durationMs: 1 };
    case '/api/logs':
      return { logs: [{ at: new Date().toISOString(), level: 'info', message: 'smoke' }], count: 1 };
    default:
      return { ok: true, endpoint: command.endpoint };
  }
}

async function pluginLoop() {
  const hello = await post('/studio/hello', {
    pluginVersion: 'test',
    placeName: 'BFish Smoke Place',
    placeId: 123456,
    studioVersion: 'test',
    role: 'edit',
  });
  state.session = hello.sessionId;
  assert(Boolean(state.session), 'hello did not return a session id');

  while (state.alive) {
    try {
      const response = await fetch(
        `${BRIDGE}/studio/poll?session=${state.session}&wait=5`,
      );
      if (response.status === 204) continue;
      if (response.status === 404) {
        state.alive = false;
        break;
      }
      const command = await response.json();
      const result = handleCommand(command);
      await post('/studio/result', {
        sessionId: state.session,
        id: command.id,
        ok: true,
        result,
      });
    } catch (error) {
      if (!state.alive) break;
      console.error('plugin loop error:', error.message);
      await sleep(250);
    }
  }
}

let rpcId = 1;
async function mcp(method, params = {}) {
  const response = await fetch(`${BRIDGE}/mcp`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: rpcId++, method, params }),
  });
  return response.json();
}

async function chatSmoke(sessionId) {
  const response = await fetch(`${BACKEND}/api/chat`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ sessionId, content: 'Say hello' }),
  });
  assert(response.ok, `chat HTTP ${response.status}`);
  const events = await collectEvents(response);
  const error = events.find((event) => event.type === 'error');
  assert(error, 'chat without an API key should emit an error event');
  assert(
    /API key|key is missing/i.test(error.message),
    `unexpected chat error: ${error.message}`,
  );
  return true;
}

async function collectEvents(response, timeoutMs = 20000) {
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  const events = [];
  let buffer = '';
  const deadline = Date.now() + timeoutMs;

  while (Date.now() < deadline) {
    const { done, value } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    const parts = buffer.split('\n\n');
    buffer = parts.pop() ?? '';
    for (const part of parts) {
      const data = part
        .split('\n')
        .filter((line) => line.startsWith('data:'))
        .map((line) => line.slice(5).trim())
        .join('\n');
      if (!data || data === 'bfish-keepalive') continue;
      events.push(JSON.parse(data));
      if (events.some((event) => ['done', 'error', 'cancelled'].includes(event.type))) {
        await reader.cancel();
        return events;
      }
    }
  }
  await reader.cancel();
  return events;
}

function startMockProvider() {
  const server = createServer(async (request, response) => {
    let body = '';
    for await (const chunk of request) body += chunk;

    if (request.url?.endsWith('/models')) {
      response.writeHead(200, { 'Content-Type': 'application/json' });
      response.end(
        JSON.stringify({
          object: 'list',
          data: [{ id: 'mock-chat' }, { id: 'mock-embedding' }],
        }),
      );
      return;
    }

    if (!request.url?.endsWith('/chat/completions')) {
      response.writeHead(404).end();
      return;
    }

    const parsed = JSON.parse(body || '{}');
    const hasToolResult = (parsed.messages ?? []).some((message) => message.role === 'tool');

    response.writeHead(200, {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache',
    });
    const send = (payload) => response.write(`data: ${JSON.stringify(payload)}\n\n`);

    if (!hasToolResult) {
      send({
        choices: [
          {
            delta: {
              role: 'assistant',
              tool_calls: [
                {
                  index: 0,
                  id: 'call_smoke',
                  type: 'function',
                  function: { name: 'get_place_info', arguments: '' },
                },
              ],
            },
          },
        ],
      });
      send({
        choices: [
          {
            delta: {
              tool_calls: [
                { index: 0, function: { arguments: '{}' } },
              ],
            },
          },
        ],
      });
      send({ choices: [{ delta: {}, finish_reason: 'tool_calls' }] });
    } else {
      send({
        choices: [{ delta: { content: 'The place is BFish Smoke Place.' } }],
      });
      send({ choices: [{ delta: {}, finish_reason: 'stop' }] });
    }
    response.write('data: [DONE]\n\n');
    response.end();
  });

  return new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(MOCK_PORT, '127.0.0.1', () => resolve(server));
  });
}

async function main() {
  if (!existsSync(BIN)) {
    throw new Error(`backend binary not found at ${BIN}; run cargo build -p bfish-backend`);
  }

  const home = await mkdtemp(join(tmpdir(), 'bfish-smoke-'));
  const plugins = join(home, 'plugins');
  await writeFile(
    join(home, 'settings.json'),
    JSON.stringify({ mcpPort: BRIDGE_PORT, provider: 'openai' }),
  );

  const backend = spawn(BIN, [], {
    env: { ...process.env, BFISH_HOME: home, BFISH_PORT: String(BACKEND_PORT) },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  backend.stderr.on('data', () => {});
  backend.stdout.on('data', () => {});
  let mock = null;

  try {
    const health = await waitFor(`${BACKEND}/api/health`);
    assert(health.ok === true, 'health check failed');
    console.log('✓ backend health');

    const settings = await (await fetch(`${BACKEND}/api/settings`)).json();
    assert(settings.settings.mcpPort === BRIDGE_PORT, 'bridge port not applied');

    const sessionResponse = await fetch(`${BACKEND}/api/sessions`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({}),
    });
    const session = (await sessionResponse.json()).session;
    assert(session.id, 'session creation failed');
    console.log('✓ sessions');

    const init = await mcp('initialize', {
      protocolVersion: '2025-06-18',
      clientInfo: { name: 'smoke', version: '1' },
    });
    assert(init.result.serverInfo.name === 'bfish-mcp', 'initialize failed');
    assert(init.result.capabilities.tools, 'missing tools capability');
    console.log('✓ MCP initialize');

    const toolList = await mcp('tools/list');
    assert(toolList.result.tools.length >= 20, 'tool list too small');
    console.log(`✓ MCP tools/list (${toolList.result.tools.length} tools)`);

    state.plugin = pluginLoop();
    await sleep(400);

    const status = await mcp('tools/call', { name: 'studio_status', arguments: {} });
    assert(status.result.isError === false, 'studio_status errored');
    assert(
      status.result.content[0].text.includes('"connected": true'),
      'studio_status did not report connected',
    );
    console.log('✓ fake Studio connected');

    const place = await mcp('tools/call', { name: 'get_place_info', arguments: {} });
    assert(
      place.result.content[0].text.includes('BFish Smoke Place'),
      `place info missing: ${place.result.content[0].text}`,
    );
    console.log('✓ MCP tool round-trip');

    const set = await mcp('tools/call', {
      name: 'set_properties',
      arguments: {
        path: 'game.Workspace.Part',
        properties: { Anchored: true, Transparency: 0.5 },
      },
    });
    assert(set.result.isError === false, 'set_properties errored');
    assert(set.result.content[0].text.includes('changed'), 'set_properties result missing');
    assert(
      state.commands.some((command) => command.endpoint === '/api/set-properties'),
      'plugin did not receive set_properties',
    );
    console.log('✓ mutation round-trip');

    const invalid = await mcp('tools/call', {
      name: 'set_properties',
      arguments: { path: 'game.Workspace.Part' },
    });
    assert(invalid.result.isError === true, 'invalid arguments should be an error');
    console.log('✓ argument validation');

    const missing = await mcp('tools/call', {
      name: 'nope',
      arguments: {},
    });
    assert(missing.result.isError === true, 'unknown tool should be an error');
    console.log('✓ unknown tool');

    const errored = await chatSmoke(session.id);
    assert(errored, 'chat without an API key should emit an error event');
    console.log('✓ chat error path');

    mock = await startMockProvider();
    const currentSettings = (await (await fetch(`${BACKEND}/api/settings`)).json()).settings;
    currentSettings.provider = 'openai';
    currentSettings.model = 'gpt-4.1';
    currentSettings.providers.openai.apiKey = 'test-key';
    currentSettings.providers.openai.baseUrl = `http://127.0.0.1:${MOCK_PORT}/v1`;
    const savedSettings = await fetch(`${BACKEND}/api/settings`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(currentSettings),
    });
    assert(savedSettings.ok, 'could not save mock provider settings');

    const detected = await (
      await fetch(`${BACKEND}/api/providers/models?provider=openai`)
    ).json();
    assert(
      Array.isArray(detected.models) && detected.models.includes('mock-chat'),
      'provider model detection failed',
    );
    assert(detected.count >= 1, 'provider model count missing');
    console.log('✓ live model detection');

    const toolSession = (
      await (
        await fetch(`${BACKEND}/api/sessions`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({}),
        })
      ).json()
    ).session;

    const chatResponse = await fetch(`${BACKEND}/api/chat`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        sessionId: toolSession.id,
        content: 'Where am I?',
        provider: 'openai',
        model: 'gpt-4.1',
      }),
    });
    const events = await collectEvents(chatResponse);
    const text = events
      .filter((event) => event.type === 'delta')
      .map((event) => event.text)
      .join('');
    assert(text.includes('BFish Smoke Place'), `missing streamed text: ${text}`);
    const toolStart = events.find((event) => event.type === 'tool_start');
    assert(toolStart?.name === 'get_place_info', 'tool_start missing');
    const toolResult = events.find((event) => event.type === 'tool_result');
    assert(toolResult?.ok === true, 'tool_result failed');
    assert(events.some((event) => event.type === 'done'), 'missing done event');

    const stored = (await (await fetch(`${BACKEND}/api/sessions/${toolSession.id}`)).json())
      .session;
    assert(
      stored.messages.some((message) => message.role === 'tool'),
      'tool result was not persisted',
    );
    assert(
      stored.messages.some(
        (message) => message.role === 'assistant' && message.content.includes('BFish Smoke Place'),
      ),
      'final assistant message was not persisted',
    );
    console.log('✓ streaming agent tool loop');

    const install = spawn(
      process.platform === 'win32'
        ? join(ROOT, 'target', 'debug', 'bfish-mcp.exe')
        : join(ROOT, 'target', 'debug', 'bfish-mcp'),
      ['--install-plugin'],
      { env: { ...process.env, BFISH_PLUGINS_DIR: plugins } },
    );
    await new Promise((resolve, reject) => {
      install.on('exit', (code) => (code === 0 ? resolve() : reject(new Error(`install exited ${code}`))));
    });
    const installed = await readFile(join(plugins, 'BFishConnector.rbxmx'), 'utf8');
    assert(installed.includes('class="Script"'), 'installed plugin is not a Script');
    assert(installed.includes('BFish Studio Connector'), 'installed plugin source is wrong');
    console.log('✓ connector install');

    console.log('\nAll smoke tests passed.');
  } finally {
    state.alive = false;
    if (mock) await new Promise((resolve) => mock.close(resolve));
    backend.kill();
    await sleep(300);
  }
}

main().catch((error) => {
  state.alive = false;
  console.error(`\nSmoke test failed: ${error.stack ?? error}`);
  process.exit(1);
});
