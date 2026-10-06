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
await writeFile(join(source, 'project.code-workspace'), '{}');
// Two pages, standard fonts and an OpenAction script that must never execute.
const pdfObjects = [
  '<< /Type /Catalog /Pages 2 0 R /OpenAction 8 0 R >>',
  '<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>',
  '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 900] /Resources << /Font << /F1 7 0 R >> >> /Contents 5 0 R >>',
  '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 900] /Resources << /Font << /F1 7 0 R >> >> /Contents 6 0 R >>',
  ...['PDF preview works', 'Second page'].map((text) => {
    const content = `0 0 1 rg 20 20 50 50 re f BT /F1 18 Tf 20 300 Td (${text}) Tj ET`;
    return `<< /Length ${content.length} >>\nstream\n${content}\nendstream`;
  }),
  '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
  '<< /S /JavaScript /JS (globalThis.pdfScriptRan = true;) >>',
];
let pdfFixture = '%PDF-1.7\n';
const pdfOffsets = [0];
for (const [index, object] of pdfObjects.entries()) {
  pdfOffsets.push(pdfFixture.length);
  pdfFixture += `${index + 1} 0 obj\n${object}\nendobj\n`;
}
const pdfXref = pdfFixture.length;
pdfFixture += `xref\n0 ${pdfOffsets.length}\n0000000000 65535 f \n`;
pdfFixture += pdfOffsets
  .slice(1)
  .map((offset) => `${String(offset).padStart(10, '0')} 00000 n \n`)
  .join('');
pdfFixture += `trailer\n<< /Size ${pdfOffsets.length} /Root 1 0 R >>\nstartxref\n${pdfXref}\n%%EOF\n`;
await writeFile(join(source, 'preview.pdf'), pdfFixture);
await writeFile(
  join(source, 'preview.md'),
  '# Native preview\n\n**Markdown works**\n\n![Local image](preview.png)',
);
await writeFile(
  join(source, 'preview.png'),
  Buffer.from(
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==',
    'base64',
  ),
);
for (const [name, fixtureName] of [
  ['preview.gif', 'animated-gif.base64'],
  ['rejected-animation.png', 'animated-png.base64'],
]) {
  const encoded = await readFile(
    resolve('src/test/fixtures', fixtureName),
    'utf8',
  );
  await writeFile(join(source, name), Buffer.from(encoded.trim(), 'base64'));
}
const server = createServer();
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const port = process.env.MYCMD_SMOKE_DEBUG_PORT
  ? Number(process.env.MYCMD_SMOKE_DEBUG_PORT)
  : server.address().port;
assert.ok(
  Number.isInteger(port) && port > 0 && port <= 65535,
  'Invalid debugger port',
);
await new Promise((r) => server.close(r));
// CI also embeds these flags through tauri.smoke.conf.json because its
// WebView2 runtime does not forward the environment-provided arguments.
const browserArguments = [
  process.env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS ?? '',
  `--remote-debugging-port=${port}`,
  '--remote-allow-origins=*',
]
  .filter(Boolean)
  .join(' ');
const appEnvironment = { ...process.env };
// Windows environment keys are case-insensitive; Node otherwise forwards
// only the first differently-cased duplicate, potentially dropping overrides.
for (const key of Object.keys(appEnvironment)) {
  if (
    [
      'WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS',
      'WEBVIEW2_USER_DATA_FOLDER',
    ].includes(key.toUpperCase())
  ) {
    delete appEnvironment[key];
  }
}
if (!process.env.MYCMD_SMOKE_DEBUG_PORT) {
  appEnvironment.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = browserArguments;
}
const app = spawn(executable, [], {
  env: {
    ...appEnvironment,
    MYCMD_CONFIG_DIR: join(fixture, 'config'),
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
        `document.querySelectorAll('.panel').length === 2 && !!document.querySelector('#system-command') && !document.querySelector('.operation-status') && !document.querySelector('dialog[open]') && !document.querySelector('#system-command').disabled`,
      ),
    'real filesystem loaded',
  );
  assert.equal(await evaluate(`document.querySelectorAll('.panel').length`), 2);
  async function submitCommand(command) {
    await evaluate(`document.querySelector('#system-command').focus()`);
    await send('Input.insertText', { text: command });
    await evaluate(
      `document.querySelector('#system-command').form.requestSubmit()`,
    );
    appLog +=
      '\nCommand submission: ' +
      JSON.stringify(
        await evaluate(
          `({value:document.querySelector('#system-command').value,disabled:document.querySelector('#system-command').disabled,text:document.body.innerText.slice(-1200)})`,
        ),
      );
  }
  const shellStarted = join(fixture, 'shell-started.txt');
  await submitCommand(
    `powershell.exe -NoProfile -Command Set-Content -LiteralPath '${shellStarted.replaceAll("'", "''")}' -Value started; Start-Sleep -Seconds 30`,
  );
  await until(async () => {
    try {
      await access(shellStarted);
      return true;
    } catch {
      return false;
    }
  }, 'shell command started');
  await until(
    () => evaluate(`!!document.querySelector('[data-command-cancel]')`),
    'shell Cancel button',
  );
  await evaluate(`document.querySelector('[data-command-cancel]').click()`);
  await until(
    () =>
      evaluate(
        `!!document.querySelector('dialog[open]')?.textContent.includes('Command cancelled.') && !document.querySelector('#system-command').disabled`,
      ),
    'shell cancellation completed',
  );
  await evaluate(`document.querySelector('dialog[open] button').click()`);
  await until(
    () => evaluate(`!document.querySelector('dialog[open]')`),
    'shell error dialog closed',
  );
  const shellRecovered = join(fixture, 'shell-recovered.txt');
  await submitCommand(
    `powershell.exe -NoProfile -Command Set-Content -LiteralPath '${shellRecovered.replaceAll("'", "''")}' -Value recovered`,
  );
  await until(async () => {
    try {
      await access(shellRecovered);
      return true;
    } catch {
      return false;
    }
  }, 'shell command after cancellation');
  await until(
    () =>
      evaluate(
        `!document.querySelector('#system-command').disabled && !document.querySelector('[data-command-cancel]')`,
      ),
    'shell ready after recovery',
  );
  assert.equal(
    await evaluate(`!!document.querySelector('dialog[open]')`),
    false,
  );
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
  // Start the conflict through the UI. Unrelated IPC operations must not
  // hijack the app's operation controller.
  await evaluate(`(() => {
    const input = document.querySelector('#panel-left .pathbar input');
    input.value = ${JSON.stringify(source)};
    input.dispatchEvent(new Event('input', {bubbles:true}));
    input.form.requestSubmit();
  })()`);
  await until(
    () =>
      evaluate(
        `document.activeElement.id === 'list-left' && document.querySelector('#list-left')?.textContent.includes('nested')`,
      ),
    'source navigation',
  );
  async function selectPreviewFile(name) {
    // Runner TEMP uses an 8.3 path (RUNNER~1), while Rust canonicalizes it to
    // the long path. Match the real basename, not either path spelling or the
    // displayed name (which intentionally omits the extension).
    await evaluate(`(() => {
      const row = Array.from(document.querySelectorAll('#list-left .file-row')).find(row => row.title.split(String.fromCharCode(92)).pop().toLowerCase() === ${JSON.stringify(name.toLowerCase())});
      if (!row) throw new Error('Preview fixture not visible');
      row.click();
    })()`);
  }
  async function previewKey(shiftKey = false) {
    await evaluate(
      `document.activeElement.dispatchEvent(new KeyboardEvent('keydown', {key:'F3', shiftKey:${shiftKey}, bubbles:true, cancelable:true}))`,
    );
  }
  await selectPreviewFile('preview.pdf');
  await previewKey();
  await until(
    () =>
      evaluate(`(() => {
    const canvas = document.querySelector('.file-viewer .pdf-page canvas');
    if (!canvas || !canvas.width || !canvas.height) return false;
    const data = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data;
    for (let i = 0; i < data.length; i += 4) if (data[i + 2] > 200 && data[i] < 50) return true;
    return false;
  })()`),
    'PDF first page rendered',
  );
  assert.equal(
    await evaluate(
      `!!document.querySelector('.file-viewer .viewer-header .pdf-toolbar') && !document.querySelector('.file-viewer .viewer-editor-shell .pdf-toolbar') && !document.querySelector('.file-viewer .viewer-header').textContent.includes('Open in default app')`,
    ),
    true,
    'PDF controls belong in the header',
  );
  await send('Input.dispatchKeyEvent', {
    type: 'keyDown',
    key: 'PageDown',
    code: 'PageDown',
    windowsVirtualKeyCode: 34,
  });
  await send('Input.dispatchKeyEvent', {
    type: 'keyUp',
    key: 'PageDown',
    code: 'PageDown',
    windowsVirtualKeyCode: 34,
  });
  await until(
    () =>
      evaluate(
        `document.querySelector('.file-viewer .pdf-pages')?.scrollTop > 0`,
      ),
    'PDF keyboard scrolling',
  );
  assert.equal(
    await evaluate('!!globalThis.pdfScriptRan'),
    false,
    'PDF scripts must not run',
  );
  await evaluate(
    `document.querySelector('.file-viewer [aria-label="Next page"]').click()`,
  );
  await until(
    () =>
      evaluate(
        `document.querySelector('.file-viewer [aria-label="Page number"]')?.value === '2' && document.querySelector('.file-viewer canvas[data-page="2"]')?.width > 0`,
      ),
    'PDF page navigation',
  );
  const pdfWidth = await evaluate(
    `document.querySelector('.file-viewer canvas[data-page="2"]').width`,
  );
  await evaluate(
    `document.querySelector('.file-viewer [aria-label="Zoom in"]').click()`,
  );
  await until(
    () =>
      evaluate(
        `document.querySelector('.file-viewer canvas[data-page="2"]')?.width > ${pdfWidth}`,
      ),
    'PDF zoom',
  );
  await evaluate(
    `Array.from(document.querySelectorAll('.file-viewer .pdf-toolbar button')).find(button => button.textContent === 'Fit width').click()`,
  );
  await until(
    () =>
      evaluate(
        `document.querySelector('.file-viewer .pdf-toolbar [aria-pressed="true"]')?.textContent === 'Fit width' && document.querySelector('.file-viewer canvas[data-page="2"]')?.width > ${pdfWidth}`,
      ),
    'PDF fit width',
  );
  await evaluate(
    `document.querySelector('.file-viewer .pdf-pages').dispatchEvent(new KeyboardEvent('keydown', {key:'Escape',bubbles:true,cancelable:true}))`,
  );
  await until(
    () => evaluate(`!document.querySelector('.file-viewer')`),
    'PDF viewer closed',
  );
  await selectPreviewFile('preview.md');
  await until(
    () =>
      evaluate(`(() => {
      const row = Array.from(document.querySelectorAll('#list-left .file-row')).find(row => row.title.endsWith('project.code-workspace'));
      const extension = row?.children[1];
      if (!extension) return false;
      const style = getComputedStyle(extension);
      const canvas = document.createElement('canvas');
      const context = canvas.getContext('2d');
      context.font = style.font;
      return extension.clientWidth >= context.measureText(extension.textContent).width + parseFloat(style.paddingLeft) + parseFloat(style.paddingRight);
    })()`),
    'long extension fits its column',
  );
  await evaluate(
    `document.activeElement.dispatchEvent(new KeyboardEvent('keydown', {key:'Enter',ctrlKey:true,bubbles:true,cancelable:true}))`,
  );
  await until(
    () =>
      evaluate(
        `document.activeElement.id === 'system-command' && document.activeElement.value === 'preview.md'`,
      ),
    'Ctrl+Enter filename insertion',
  );
  await evaluate(
    `(() => { const input = document.querySelector('#system-command'); input.value = ''; input.dispatchEvent(new Event('input', {bubbles:true})); document.querySelector('#list-left').focus(); })()`,
  );
  await previewKey();
  await until(
    () =>
      evaluate(
        `document.querySelector('dialog.file-viewer[open] .markdown-preview h1')?.textContent === 'Native preview'`,
      ),
    'native F3 Markdown preview',
  );
  assert.equal(
    await evaluate(`(() => {
    const rect = document.querySelector('dialog.file-viewer[open]').getBoundingClientRect();
    return Math.abs(rect.left) < 1 && Math.abs(rect.top) < 1 && Math.abs(rect.width - innerWidth) < 1 && Math.abs(rect.height - innerHeight) < 1;
  })()`),
    true,
    'F3 viewer fills the application viewport',
  );
  await until(
    () =>
      evaluate(
        `(() => { const image = document.querySelector('dialog.file-viewer[open] .markdown-preview img'); return !!image?.complete && image.naturalWidth === 1; })()`,
      ),
    'native embedded Markdown image decode',
  );
  assert.equal(
    await evaluate(
      `document.querySelector('dialog.file-viewer .markdown-preview strong')?.textContent`,
    ),
    'Markdown works',
  );
  await evaluate(
    `document.querySelector('dialog.file-viewer .viewer-close').click()`,
  );
  await until(
    () =>
      evaluate(
        `!document.querySelector('dialog[open]') && document.activeElement.id === 'list-left'`,
      ),
    'Markdown preview closed',
  );
  await selectPreviewFile('preview.png');
  await previewKey();
  await until(
    () =>
      evaluate(
        `(() => { const image = document.querySelector('dialog.file-viewer[open] .image-preview img'); return !!image?.complete && image.naturalWidth === 1; })()`,
      ),
    'native F3 image decode',
  );
  await evaluate(
    `document.querySelector('dialog.file-viewer .viewer-close').click()`,
  );
  await until(
    () =>
      evaluate(
        `!document.querySelector('dialog[open]') && document.activeElement.id === 'list-left'`,
      ),
    'image preview closed',
  );
  await selectPreviewFile('preview.gif');
  await previewKey();
  await until(
    () =>
      evaluate(
        `(() => { const image = document.querySelector('dialog.file-viewer[open] .image-preview img'); return !!image?.complete && image.naturalWidth === 1; })()`,
      ),
    'native animated GIF decode',
  );
  await evaluate(
    `document.querySelector('dialog.file-viewer .viewer-close').click()`,
  );
  await until(
    () =>
      evaluate(
        `!document.querySelector('dialog[open]') && document.activeElement.id === 'list-left'`,
      ),
    'GIF preview closed',
  );
  await selectPreviewFile('rejected-animation.png');
  await previewKey();
  await until(
    () =>
      evaluate(
        `document.querySelector('dialog.file-viewer[open] .viewer-message')?.textContent.includes('Only GIF animation is supported')`,
      ),
    'animated PNG rejected',
  );
  assert.equal(
    await evaluate(
      `Array.from(document.querySelectorAll('dialog.file-viewer button')).some(button => button.textContent.trim() === 'Open in default app')`,
    ),
    true,
  );
  await evaluate(
    `document.querySelector('dialog.file-viewer .viewer-close').click()`,
  );
  await until(
    () =>
      evaluate(
        `!document.querySelector('dialog[open]') && document.activeElement.id === 'list-left'`,
      ),
    'unsupported animation preview closed',
  );
  await selectPreviewFile('preview.md');
  await previewKey(true);
  await until(
    () =>
      evaluate(
        `document.querySelector('.preview-panel .markdown-preview h1')?.textContent === 'Native preview'`,
      ),
    'Shift+F3 Markdown panel',
  );
  await selectPreviewFile('preview.png');
  await until(
    () =>
      evaluate(
        `(() => { const image = document.querySelector('.preview-panel .image-preview img'); return !!image?.complete && image.naturalWidth === 1; })()`,
      ),
    'preview panel follows cursor',
  );
  await selectPreviewFile('preview.pdf');
  await until(
    () =>
      evaluate(
        `document.querySelector('.preview-panel canvas[data-page="1"]')?.width > 0 && !document.querySelector('.preview-panel [role="alert"]')`,
      ),
    'PDF preview panel',
  );
  await previewKey(true);
  await until(
    () => evaluate(`!document.querySelector('.preview-panel')`),
    'preview panel disabled',
  );
  await evaluate(`(() => {
    Array.from(document.querySelectorAll('#list-left .file-row')).find(row => row.textContent.includes('[nested]')).click();
    document.activeElement.dispatchEvent(new KeyboardEvent('keydown', {key:'F5',bubbles:true,cancelable:true}));
  })()`);
  await until(
    () => evaluate(`!!document.querySelector('dialog[open] input')`),
    'copy dialog',
  );
  await evaluate(`(() => {
    const input = document.querySelector('dialog[open] input');
    input.value = ${JSON.stringify(destination)};
    input.dispatchEvent(new Event('input', {bubbles:true}));
    input.form.requestSubmit();
  })()`);
  await until(
    () =>
      evaluate(
        `!!window.smokeConflict?.operationId && !!document.querySelector('dialog[open] [data-conflict-action="skip"]')`,
      ),
    'conflict dialog',
  );
  const id = await evaluate('window.smokeConflict.operationId');
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
    'PASS: native startup, shell cancellation and recovery, F3 Markdown/image/PDF previews, PDF navigation/zoom/keyboard scrolling and disabled scripts, Shift+F3 cursor-following preview, directory listing, copy, conflict dialog, rename, mkdir, move, delete, progress events, path input and F7 keyboard flow.',
  );
} catch (error) {
  await writeFile(join(diagnostics, 'error.txt'), error.stack ?? String(error));
  const processes = spawnSync(
    'powershell.exe',
    [
      '-NoProfile',
      '-Command',
      "Get-CimInstance Win32_Process | Where-Object { $_.Name -in @('mycmd.exe', 'msedgewebview2.exe') } | Select-Object ProcessId,ParentProcessId,ExecutablePath,CommandLine | ConvertTo-Json -Depth 3",
    ],
    { encoding: 'utf8', timeout: 10000 },
  );
  await writeFile(
    join(diagnostics, 'startup.json'),
    JSON.stringify(
      {
        executable,
        pid: app.pid,
        exitCode: app.exitCode,
        port,
        browserArguments,
        lastDebuggerState,
        processes: processes.stdout,
        processError: processes.stderr,
      },
      null,
      2,
    ),
  );
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
