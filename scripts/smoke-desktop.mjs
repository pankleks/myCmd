// Windows integration smoke test: a real Tauri/WebView2 process, IPC and UI.
// All file mutations are confined to a newly created temporary directory.
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import {
  mkdtemp,
  mkdir,
  writeFile,
  readFile,
  rm,
  access,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { createServer } from 'node:net';
import { setTimeout as delay } from 'node:timers/promises';

if (process.platform !== 'win32')
  throw new Error(
    'This smoke test uses Windows WebView2. Run cargo test on other platforms.',
  );
const executable = resolve(
  process.argv[2] ?? 'src-tauri/target/debug/mycmd.exe',
);
const diagnostics = resolve('test-results/desktop');
await mkdir(diagnostics, { recursive: true });
await access(executable);
const fixture = await mkdtemp(join(tmpdir(), 'mycmd-smoke-'));
const source = join(fixture, 'source');
const destination = join(fixture, 'destination');
await mkdir(join(source, 'nested'), { recursive: true });
await mkdir(destination);
await writeFile(join(source, 'nested', 'sample.txt'), 'smoke payload');
const server = createServer();
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const port = server.address().port;
await new Promise((r) => server.close(r));
const app = spawn(executable, [], {
  env: {
    ...process.env,
    MYCMD_CONFIG_DIR: join(fixture, 'config'),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-allow-origins=*`,
    WEBVIEW2_USER_DATA_FOLDER: join(fixture, 'webview-profile'),
  },
  stdio: ['ignore', 'pipe', 'pipe'],
});
let appError;
let appLog = '';
let lastDebuggerState = 'no debugger attempts yet';
app.on('error', (error) => {
  appError = error;
});
for (const stream of [app.stdout, app.stderr])
  stream.on('data', (chunk) => {
    appLog = (appLog + chunk.toString()).slice(-100000);
  });
let socket;
let capture;
const deadlineTimer = setTimeout(() => {
  appError = new Error('Desktop smoke test exceeded 180 seconds.');
  app.kill();
}, 180000);
async function until(fn, message, timeoutMs = 20000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (appError) throw appError;
    if (app.exitCode !== null)
      throw new Error(
        `Application exited (${app.exitCode}) while waiting for ${message}.\n${appLog}`,
      );
    const value = await fn();
    if (value) return value;
    await delay(50);
  }
  throw new Error(
    `Timed out: ${message}. Last debugger state: ${lastDebuggerState}. App exit: ${app.exitCode}. Log:\n${appLog}`,
  );
}
try {
  const target = await until(
    async () => {
      try {
        const response = await fetch(`http://127.0.0.1:${port}/json/list`, {
          signal: AbortSignal.timeout(2000),
        });
        if (!response.ok) {
          lastDebuggerState = `HTTP ${response.status} from /json/list`;
          return undefined;
        }
        const targets = await response.json();
        lastDebuggerState = `targets: ${JSON.stringify(targets).slice(0, 2000)}`;
        return (
          targets.find((t) => t.type === 'page' && t.webSocketDebuggerUrl) ??
          targets.find((t) => t.webSocketDebuggerUrl)
        );
      } catch (error) {
        lastDebuggerState = `fetch failed: ${error.message}`;
        return undefined;
      }
    },
    'WebView2 startup',
    60000,
  );
  socket = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error('WebView2 debugger connection timed out.')),
      10000,
    );
    socket.addEventListener(
      'open',
      () => {
        clearTimeout(timer);
        resolve();
      },
      { once: true },
    );
    socket.addEventListener(
      'error',
      (error) => {
        clearTimeout(timer);
        reject(error);
      },
      { once: true },
    );
  });
  let sequence = 0;
  const pending = new Map();
  socket.addEventListener('message', ({ data }) => {
    const message = JSON.parse(data);
    if (message.id) {
      const request = pending.get(message.id);
      if (!request) return;
      pending.delete(message.id);
      clearTimeout(request.timer);
      if (message.error) request.reject(message.error);
      else request.resolve(message.result);
    }
  });
  socket.addEventListener('close', () => {
    for (const { reject, timer } of pending.values()) {
      clearTimeout(timer);
      reject(new Error('WebView2 debugger connection closed.'));
    }
    pending.clear();
  });
  function send(method, params) {
    return new Promise((resolve, reject) => {
      const id = ++sequence;
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`Debugger request timed out: ${method}`));
      }, 10000);
      pending.set(id, { resolve, reject, timer });
      socket.send(JSON.stringify({ id, method, params }));
    });
  }
  capture = async () => {
    const screenshot = await send('Page.captureScreenshot', { format: 'png' });
    await writeFile(
      join(diagnostics, 'failure.png'),
      Buffer.from(screenshot.data, 'base64'),
    );
    const page = await evaluate('document.body.innerText');
    await writeFile(join(diagnostics, 'page.txt'), page);
  };
  async function evaluate(expression) {
    const result = await send('Runtime.evaluate', {
      expression,
      returnByValue: true,
      awaitPromise: true,
    });
    if (result.exceptionDetails)
      throw new Error(JSON.stringify(result.exceptionDetails));
    return result.result.value;
  }
  await until(
    () =>
      evaluate(
        `!document.querySelector('.operation-status') && !document.querySelector('dialog[open]') && !document.querySelector('#system-command')?.disabled`,
      ),
    'real filesystem loaded',
  );
  assert.equal(await evaluate(`document.querySelectorAll('.panel').length`), 2);
  await evaluate(`(async () => {
    window.smokeProgress = {}; window.smokeConflict = null;
    const ipc = window.__TAURI_INTERNALS__;
    await ipc.invoke('plugin:event|listen', { event: 'operation-progress', target: {kind:'Any'}, handler: ipc.transformCallback(e => window.smokeProgress[e.payload.operationId] = e.payload) });
    await ipc.invoke('plugin:event|listen', { event: 'operation-conflict', target: {kind:'Any'}, handler: ipc.transformCallback(e => window.smokeConflict = e.payload) });
  })()`);
  const invoke = (command, args) =>
    evaluate(
      `window.__TAURI_INTERNALS__.invoke(${JSON.stringify(command)}, ${JSON.stringify(args)})`,
    );
  async function operation(operation) {
    const id = await invoke('start_operation', { operation });
    const progress = await until(
      () =>
        evaluate(
          `['completed','failed','cancelled'].includes(window.smokeProgress[${JSON.stringify(id)}]?.state) && window.smokeProgress[${JSON.stringify(id)}]`,
        ),
      operation.type,
    );
    assert.equal(progress.state, 'completed', JSON.stringify(progress.error));
    return progress;
  }
  await operation({
    type: 'copy',
    sources: [join(source, 'nested')],
    destination,
  });
  assert.equal(
    await readFile(join(destination, 'nested', 'sample.txt'), 'utf8'),
    'smoke payload',
  );
  const id = await invoke('start_operation', {
    operation: { type: 'copy', sources: [join(source, 'nested')], destination },
  });
  await until(
    () =>
      evaluate(
        `window.smokeConflict?.operationId === ${JSON.stringify(id)} && !!document.querySelector('dialog[open] [data-conflict-action="skip"]')`,
      ),
    'conflict dialog',
  );
  await evaluate(
    `document.querySelector('dialog[open] [data-conflict-action="skip"]').click()`,
  );
  await until(
    () =>
      evaluate(
        `window.smokeProgress[${JSON.stringify(id)}]?.state === 'completed'`,
      ),
    'skip conflict',
  );
  await operation({
    type: 'rename',
    path: join(destination, 'nested', 'sample.txt'),
    name: 'renamed.txt',
  });
  await operation({
    type: 'createDirectory',
    parent: destination,
    name: 'archive',
  });
  await operation({
    type: 'move',
    sources: [join(destination, 'nested', 'renamed.txt')],
    destination: join(destination, 'archive'),
  });
  assert.equal(
    await readFile(join(destination, 'archive', 'renamed.txt'), 'utf8'),
    'smoke payload',
  );
  // Type a path through the actual path bar and verify keyboard operation dialogs.
  await evaluate(
    `(() => { const input=document.querySelector('.panel .pathbar input'); input.value=${JSON.stringify(destination)}; input.dispatchEvent(new Event('input',{bubbles:true})); input.form.requestSubmit(); })()`,
  );
  await until(
    () =>
      evaluate(
        `document.querySelector('#list-left')?.textContent.includes('archive') && document.activeElement.id === 'list-left'`,
      ),
    'path navigation',
  );
  await evaluate(
    `document.activeElement.dispatchEvent(new KeyboardEvent('keydown',{key:'F7',bubbles:true,cancelable:true}))`,
  );
  await until(
    () => evaluate(`!!document.querySelector('dialog[open] input')`),
    'F7 dialog',
  );
  await evaluate(
    `(() => { const input=document.querySelector('dialog input'); input.value='via-keyboard'; input.dispatchEvent(new Event('input',{bubbles:true})); input.form.requestSubmit(); })()`,
  );
  await until(
    () =>
      evaluate(
        `document.querySelector('#list-left .cursor')?.textContent.includes('via-keyboard')`,
      ),
    'new directory cursor',
  );
  await access(join(destination, 'via-keyboard'));
  await operation({
    type: 'delete',
    sources: [
      join(destination, 'nested'),
      join(destination, 'archive'),
      join(destination, 'via-keyboard'),
    ],
  });
  await assert.rejects(access(join(destination, 'archive')));
  console.log(
    'PASS: native startup, directory listing, copy, conflict dialog, rename, mkdir, move, delete, progress events, path input and F7 keyboard flow.',
  );
} catch (error) {
  await writeFile(join(diagnostics, 'error.txt'), error.stack ?? String(error));
  try {
    await capture?.();
  } catch (captureError) {
    appLog += `\nCapture failed: ${captureError.message}`;
  }
  throw error;
} finally {
  clearTimeout(deadlineTimer);
  await writeFile(join(diagnostics, 'app.log'), appLog);
  socket?.close();
  if (app.pid && app.exitCode === null)
    spawnSync('taskkill', ['/PID', String(app.pid), '/T', '/F'], {
      stdio: 'ignore',
    });
  await rm(fixture, {
    recursive: true,
    force: true,
    maxRetries: 10,
    retryDelay: 200,
  });
}
