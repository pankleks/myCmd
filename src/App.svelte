<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { isTauri } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import FilePanel from './components/FilePanel.svelte';
  import ErrorDialog from './components/ErrorDialog.svelte';
  import OperationDialog, {
    type Action,
  } from './components/OperationDialog.svelte';
  import ConflictDialog from './components/ConflictDialog.svelte';
  import GlobSelectionDialog from './components/GlobSelectionDialog.svelte';
  import SettingsDialog from './components/SettingsDialog.svelte';
  import FileViewerDialog from './components/FileViewerDialog.svelte';
  import {
    commander,
    load,
    rows,
    sources,
    toggle,
    invertSelection,
    selectByGlob,
    open,
    quickFindAppend,
    quickFindBackspace,
    quickFindClose,
    measureDirectory,
    type Side,
    type SelectionMode,
  } from './state/commander.svelte';
  import {
    preferences,
    loadPreferences,
    scheduleSave,
    setShowHidden,
  } from './state/preferences.svelte';
  import {
    DEFAULT_FILE_FONT_SIZE,
    MAX_FILE_FONT_SIZE,
    MIN_FILE_FONT_SIZE,
  } from './utils/config';
  import { api, errorMessage } from './filesystem/api';
  import type { FileEntry } from './filesystem/types';
  import type {
    Conflict,
    FileOperation,
    Progress,
    Resolution,
  } from './operations/types';
  import { bytes } from './utils/format';
  import { displayPath } from './utils/paths';
  import { isQuickFindTrigger } from './utils/keyboard';
  let ready = $state(false);
  let previewMode = $state(false);
  let error = $state('');
  let errorTitle = $state('Error');
  let busy = $state(false);
  let commandRunning = $state(false);
  let commandInput = $state('');
  let commandError = $state<{ command: string; output: string }>();
  let selectionDialog = $state<{ mode: SelectionMode }>();
  let settingsOpen = $state(false);
  let viewer = $state<{
    path: string;
    name: string;
    extension: string;
  }>();
  let commandInputElement: HTMLInputElement;
  let progress = $state<Progress>();
  let conflict = $state<Conflict>();
  let dialog = $state<{
    action: Action;
    entries: FileEntry[];
    parent: string;
    destination: string;
    side: Side;
    permanent: boolean;
  }>();
  let operationSide: Side = 'left';
  let operationId: string | undefined;
  const refreshTimers: Partial<Record<Side, ReturnType<typeof setTimeout>>> =
    {};
  let active = $derived(commander[commander.activePanel]);
  let previewSide = $derived(
    commander.activePanel === 'left' ? 'right' : 'left',
  );
  let previewTarget = $derived.by(() => {
    if (!previewMode || active.loading) return undefined;
    const entry = rows(active)[active.cursor];
    if (
      !entry ||
      entry.parentEntry ||
      entry.type === 'directory' ||
      entry.directoryTarget
    )
      return undefined;
    return {
      path: entry.path,
      name: entry.name,
      extension: entry.extension,
      key: `${commander.activePanel}:${active.revision}:${entry.path}:${entry.size}:${entry.modified ?? ''}`,
    };
  });
  let percent = $derived(
    progress
      ? Math.min(
          100,
          Math.round(
            progress.totalBytes
              ? (progress.processedBytes / progress.totalBytes) * 100
              : progress.totalItems
                ? (progress.processedItems / progress.totalItems) * 100
                : 0,
          ),
        )
      : 0,
  );
  $effect(() => {
    if (!ready) return;
    const paths = [commander.left.path, commander.right.path].filter(Boolean);
    void api.watch(paths).catch((e) => {
      showError(errorMessage(e));
    });
  });
  $effect(() => {
    if (!ready) return;
    commander.left.path;
    commander.right.path;
    preferences.fileFontSize;
    preferences.columnWidths;
    preferences.showHidden;
    preferences.showFunctionBar;
    scheduleSave();
  });
  const actions: [string, Action, string][] = [
    ['F2', 'rename', 'Rename'],
    ['F5', 'copy', 'Copy'],
    ['F6', 'move', 'Move'],
    ['F7', 'createDirectory', 'New folder'],
    ['F8', 'delete', 'Delete'],
  ];
  function focusPanel() {
    document.getElementById(`list-${commander.activePanel}`)?.focus();
  }
  function openDrivePicker(side: Side) {
    commander.activePanel = side;
    const picker = document.getElementById(
      `drive-${side}`,
    ) as HTMLSelectElement | null;
    if (!picker) return;
    picker.focus();
    try {
      picker.showPicker();
    } catch {
      picker.click();
    }
  }
  function closeDialog() {
    dialog = undefined;
    void tick().then(focusPanel);
  }
  function closeSelectionDialog() {
    selectionDialog = undefined;
    void tick().then(focusPanel);
  }
  function applyGlobSelection(pattern: string) {
    if (!selectionDialog) return;
    selectByGlob(active, pattern, selectionDialog.mode);
    closeSelectionDialog();
  }
  function closeSettings() {
    settingsOpen = false;
    void tick().then(focusPanel);
  }
  function closeViewer() {
    viewer = undefined;
    void tick().then(focusPanel);
  }
  function viewCurrentFile() {
    if (!ready || busy || commandRunning || active.loading) return;
    const entry = rows(active)[active.cursor];
    if (!entry || entry.parentEntry) return;
    if (entry.type === 'directory' || entry.directoryTarget) {
      showError('Select a file to view.');
      return;
    }
    quickFindClose();
    viewer = {
      path: entry.path,
      name: entry.name,
      extension: entry.extension,
    };
  }
  function togglePreviewPane() {
    if (!ready || busy || commandRunning) return;
    quickFindClose();
    previewMode = !previewMode;
    void tick().then(focusPanel);
  }
  function applySettings(settings: {
    fileFontSize: number;
    showHidden: boolean;
    showFunctionBar: boolean;
  }) {
    preferences.fileFontSize = settings.fileFontSize;
    preferences.showFunctionBar = settings.showFunctionBar;
    setShowHidden(settings.showHidden);
    closeSettings();
  }
  function showError(message: string, title = 'Error') {
    errorTitle = title;
    error = message;
  }
  function request(action: Action, permanent = false) {
    if (
      !ready ||
      busy ||
      commandRunning ||
      dialog ||
      selectionDialog ||
      settingsOpen ||
      viewer ||
      conflict ||
      commandError ||
      error ||
      active.loading ||
      !active.path
    )
      return;
    const entries = sources(active);
    if (action !== 'createDirectory' && !entries.length) return;
    if (action === 'rename' && entries.length !== 1) {
      showError('Select exactly one item to rename.');
      return;
    }
    error = '';
    dialog = {
      action,
      entries,
      parent: active.path,
      destination:
        commander[commander.activePanel === 'left' ? 'right' : 'left'].path,
      side: commander.activePanel,
      permanent: action === 'delete' && permanent,
    };
  }
  async function submit(operation: FileOperation) {
    operationSide = dialog!.side;
    busy = true;
    progress = undefined;
    operationId = undefined;
    closeDialog();
    try {
      const id = await api.start(operation);
      if (busy) operationId = id;
    } catch (e) {
      showError(errorMessage(e), 'Operation failed');
      busy = false;
    }
  }
  async function resolve(resolution: Resolution) {
    if (!conflict) return;
    try {
      await api.resolve(conflict.operationId, resolution);
      conflict = undefined;
      await tick();
      focusPanel();
    } catch (e) {
      throw new Error(errorMessage(e));
    }
  }
  async function cancel() {
    const id = operationId ?? progress?.operationId;
    if (id)
      try {
        await api.cancel(id);
      } catch (e) {
        showError(errorMessage(e), 'Operation failed');
      }
  }
  async function runCommand(event?: SubmitEvent) {
    event?.preventDefault();
    const command = commandInput.trim();
    if (!command || !ready || busy || commandRunning || !active.path) return;
    commandInput = '';
    commandRunning = true;
    error = '';
    try {
      const result = await api.runCommand(command, active.path);
      if (!result.success) {
        commandError = {
          command,
          output:
            [result.stderr, result.stdout].filter(Boolean).join('\n').trim() ||
            'The command failed without output.',
        };
      }
      await Promise.all([load(commander.left), load(commander.right)]);
    } catch (e) {
      commandError = { command, output: errorMessage(e) };
    } finally {
      commandRunning = false;
    }
  }
  onMount(() => {
    let disposed = false;
    const unlisten: UnlistenFn[] = [];
    async function init() {
      if (!isTauri()) {
        showError(
          'File access requires the desktop app. Run: npm run tauri dev',
        );
        return;
      }
      try {
        const p = await listen<Progress>(
          'operation-progress',
          ({ payload }) => {
            progress = payload;
            operationId = payload.operationId;
            if (['completed', 'failed', 'cancelled'].includes(payload.state)) {
              busy = false;
              conflict = undefined;
              if (payload.error && payload.state === 'failed')
                showError(errorMessage(payload.error), 'Operation failed');
              void Promise.all([
                load(
                  commander.left,
                  undefined,
                  operationSide === 'left' ? payload.resultPath : undefined,
                ),
                load(
                  commander.right,
                  undefined,
                  operationSide === 'right' ? payload.resultPath : undefined,
                ),
              ]).then(() => {
                commander[operationSide].selected = new Set();
              });
            }
          },
        );
        unlisten.push(p);
        const c = await listen<Conflict>(
          'operation-conflict',
          ({ payload }) => (conflict = payload),
        );
        unlisten.push(c);
        const filesystemChanges = await listen<string>(
          'filesystem-changed',
          ({ payload }) => {
            if (!ready || busy || commandRunning) return;
            for (const side of ['left', 'right'] as const) {
              const panel = commander[side];
              if (panel.path !== payload) continue;
              clearTimeout(refreshTimers[side]);
              refreshTimers[side] = setTimeout(() => {
                refreshTimers[side] = undefined;
                if (!disposed && !busy && !commandRunning) void load(panel);
              }, 180);
            }
          },
        );
        unlisten.push(filesystemChanges);
        if (disposed) {
          unlisten.forEach((fn) => fn());
          return;
        }
        commander.roots = await api.roots();
        const home =
          commander.roots.find((r) => r.type === 'home')?.path ?? '~';
        const saved = await loadPreferences();
        await Promise.all([
          load(commander.left, saved.leftPath ?? home),
          load(commander.right, saved.rightPath ?? home),
        ]);
        if (commander.left.error) await load(commander.left, home);
        if (commander.right.error) await load(commander.right, home);
        if (!disposed) {
          ready = true;
          focusPanel();
        }
      } catch (e) {
        showError(errorMessage(e));
      }
    }
    void init();
    return () => {
      disposed = true;
      for (const timer of Object.values(refreshTimers)) clearTimeout(timer);
      unlisten.forEach((fn) => fn());
    };
  });
  function keydown(event: KeyboardEvent) {
    if (
      dialog ||
      selectionDialog ||
      settingsOpen ||
      viewer ||
      conflict ||
      commandError ||
      error ||
      event.defaultPrevented ||
      event.isComposing
    )
      return;
    const target = event.target as HTMLElement;
    if (
      event.altKey &&
      !event.ctrlKey &&
      !event.metaKey &&
      !event.shiftKey &&
      (event.key === 'F1' || event.key === 'F2')
    ) {
      event.preventDefault();
      openDrivePicker(event.key === 'F1' ? 'left' : 'right');
      return;
    }
    if (target.closest('input, select, textarea, dialog')) {
      if (commander.quickFind && event.key === 'Escape') {
        event.preventDefault();
        quickFindClose();
        focusPanel();
        return;
      }
      return;
    }
    if (
      ready &&
      !busy &&
      !commandRunning &&
      !active.loading &&
      !event.ctrlKey &&
      !event.metaKey &&
      !event.altKey &&
      !target.closest('button')
    ) {
      if (event.key === '*') {
        event.preventDefault();
        quickFindClose();
        invertSelection(active);
        return;
      }
      if (event.key === '+' || event.key === '-') {
        event.preventDefault();
        quickFindClose();
        selectionDialog = {
          mode: event.key === '+' ? 'extend' : 'shrink',
        };
        return;
      }
    }
    const ctrl = (event.ctrlKey || event.metaKey) && !event.altKey;
    if (
      event.key === 'F3' &&
      event.shiftKey &&
      !event.ctrlKey &&
      !event.metaKey &&
      !event.altKey
    ) {
      event.preventDefault();
      togglePreviewPane();
      return;
    }
    if (event.key === 'Tab' && !ctrl && !event.altKey) {
      event.preventDefault();
      quickFindClose();
      commander.activePanel =
        commander.activePanel === 'left' ? 'right' : 'left';
      if (previewMode) previewMode = false;
      void tick().then(focusPanel);
      return;
    }
    if (event.key === 'F9' && !ctrl && !event.altKey) {
      event.preventDefault();
      quickFindClose();
      settingsOpen = true;
      return;
    }
    if (
      event.key === 'F3' &&
      !event.ctrlKey &&
      !event.metaKey &&
      !event.altKey &&
      !event.shiftKey
    ) {
      event.preventDefault();
      viewCurrentFile();
      return;
    }
    const action =
      actions.find(([key]) => key === event.key)?.[1] ??
      (event.key === 'Delete' ? 'delete' : undefined);
    if (action) {
      event.preventDefault();
      quickFindClose();
      request(
        action,
        action === 'delete' &&
          event.shiftKey &&
          (event.key === 'Delete' || event.key === 'F8'),
      );
      return;
    }
    if (ctrl) {
      switch (event.key.toLowerCase()) {
        case 'a':
          event.preventDefault();
          active.selected = new Set(
            rows(active)
              .filter((e) => !e.parentEntry)
              .map((e) => e.path),
          );
          break;
        case 'h':
          event.preventDefault();
          setShowHidden(!preferences.showHidden);
          break;
        case 'r':
          event.preventDefault();
          void load(active);
          break;
        case 'i':
          event.preventDefault();
          if (active.path) {
            const other =
              commander[commander.activePanel === 'left' ? 'right' : 'left'];
            void load(other, active.path, rows(active)[active.cursor]?.path);
          }
          break;
        case 'l':
          event.preventDefault();
          (
            document.querySelector(
              '.panel.active .pathbar input',
            ) as HTMLInputElement
          )?.select();
          break;
        case '+':
        case '=':
          event.preventDefault();
          preferences.fileFontSize = Math.min(
            MAX_FILE_FONT_SIZE,
            preferences.fileFontSize + 1,
          );
          break;
        case '-':
        case '_':
          event.preventDefault();
          preferences.fileFontSize = Math.max(
            MIN_FILE_FONT_SIZE,
            preferences.fileFontSize - 1,
          );
          break;
        case '0':
          event.preventDefault();
          preferences.fileFontSize = DEFAULT_FILE_FONT_SIZE;
          break;
      }
      return;
    }
    const altGraph =
      typeof event.getModifierState === 'function' &&
      event.getModifierState('AltGraph');
    if (
      isQuickFindTrigger({
        key: event.key,
        altKey: event.altKey,
        metaKey: event.metaKey,
        shiftKey: event.shiftKey,
        altGraph,
      })
    ) {
      // Dowolny Alt (lewy lub prawy/AltGr) spoza pól tekstowych dokarmia
      // szybkie wyszukiwanie. W polach tekstowych Alt wpisuje polskie znaki.
      event.preventDefault();
      quickFindAppend(commander.activePanel, event.key);
      focusPanel();
      return;
    }
    if (commander.quickFind && !ctrl && !event.metaKey) {
      if (event.altKey && !event.ctrlKey) return;
      if (['Alt', 'AltGraph', 'Control', 'Shift', 'Meta'].includes(event.key))
        return;
      if (event.key === 'Escape' || event.key === 'Enter') {
        event.preventDefault();
        quickFindClose();
        focusPanel();
        return;
      }
      if (event.key === 'Backspace') {
        event.preventDefault();
        quickFindBackspace();
        return;
      }
      if (event.key.length === 1 && !event.altKey) {
        event.preventDefault();
        quickFindAppend(commander.activePanel, event.key);
        return;
      }
      quickFindClose();
    }
    if (
      event.key === 'Enter' &&
      commandInput.trim() &&
      ready &&
      !busy &&
      !commandRunning
    ) {
      event.preventDefault();
      commandInputElement?.focus();
      void runCommand();
      return;
    }
    if (target.closest('button') && ['Enter', ' '].includes(event.key)) return;
    if (
      ready &&
      !busy &&
      !commandRunning &&
      !event.altKey &&
      event.key.length === 1 &&
      (event.key !== ' ' || commandInput.length > 0)
    ) {
      event.preventDefault();
      commandInput += event.key;
      commandInputElement?.focus();
      return;
    }
    if (event.key === 'Backspace' && commandInput && ready && !busy) {
      event.preventDefault();
      commandInput = Array.from(commandInput).slice(0, -1).join('');
      commandInputElement?.focus();
      return;
    }
    const list = rows(active);
    const page = Math.max(
      1,
      Math.floor(
        (document.getElementById(`list-${commander.activePanel}`)
          ?.clientHeight ?? 320) / 28,
      ),
    );
    const movement: Record<string, number> = {
      ArrowUp: -1,
      ArrowDown: 1,
      ArrowLeft: -list.length,
      ArrowRight: list.length,
      PageUp: -page,
      PageDown: page,
      Home: -list.length,
      End: list.length,
    };
    if (event.key in movement) {
      event.preventDefault();
      active.cursor = Math.max(
        0,
        Math.min(list.length - 1, active.cursor + movement[event.key]),
      );
      focusPanel();
    } else if (event.key === 'Enter') {
      event.preventDefault();
      void open(active, list[active.cursor]);
    } else if (event.key === 'Backspace') {
      event.preventDefault();
      if (active.parent) void load(active, active.parent, active.path);
    } else if (event.key === ' ' || event.key === 'Insert') {
      event.preventDefault();
      const row = list[active.cursor];
      toggle(active, row);
      if (
        event.key === ' ' &&
        row &&
        !row.parentEntry &&
        row.type === 'directory'
      )
        void measureDirectory(active, row);
      if (event.key === 'Insert')
        active.cursor = Math.min(
          Math.max(0, list.length - 1),
          active.cursor + 1,
        );
    } else if (event.key === 'Escape') {
      if (commandInput) {
        commandInput = '';
        commandInputElement?.focus();
      } else {
        active.selected = new Set();
        error = '';
      }
    }
  }
</script>

<svelte:window onkeydown={keydown} />
{#snippet previewPane()}
  {#if previewTarget}
    {@const key = previewTarget.key}
    {#key key}
      <FileViewerDialog
        path={previewTarget.path}
        name={previewTarget.name}
        extension={previewTarget.extension}
        inline
        onclose={() => {}}
      />
    {/key}
  {:else}
    <section class="panel preview-panel" aria-label="File preview">
      <div class="viewer-header">
        <div class="viewer-heading"><h2>Preview</h2></div>
      </div>
      <div class="preview-empty">
        {active.loading ? 'Loading files…' : 'Select a file to preview.'}
      </div>
    </section>
  {/if}
{/snippet}
<main style:--file-font-size={`${preferences.fileFontSize}px`}>
  <div class="panels">
    <FilePanel
      panel={commander.left}
      side="left"
      roots={commander.roots}
      hidden={previewMode && previewSide === 'left'}
    />
    {#if previewMode && previewSide === 'left'}{@render previewPane()}{/if}
    <FilePanel
      panel={commander.right}
      side="right"
      roots={commander.roots}
      hidden={previewMode && previewSide === 'right'}
    />
    {#if previewMode && previewSide === 'right'}{@render previewPane()}{/if}
  </div>
  {#if error}<ErrorDialog
      title={errorTitle}
      output={error}
      onclose={() => {
        error = '';
        errorTitle = 'Error';
        void tick().then(() => commandInputElement?.focus());
      }}
    />
  {:else if commandError}<ErrorDialog
      {...commandError}
      onclose={() => {
        commandError = undefined;
        void tick().then(() => commandInputElement?.focus());
      }}
    />{/if}
  {#if busy || !ready}
    <div class="operation-status" role="status" aria-live="polite">
      {#if progress}<span
          >{{
            queued: 'Queued',
            running: 'Operation in progress',
            completed: 'Completed',
            failed: 'Operation failed',
            cancelled: 'Cancelled',
          }[progress.state]} · {progress.processedItems}/{progress.totalItems} · {bytes(
            progress.processedBytes,
          )}</span
        >{#if busy}<progress max="100" value={percent}></progress><span
            >{percent}%</span
          ><button onclick={cancel}>Cancel</button>{/if}<span
          class="current-item"
          title={progress.currentItem}>{progress.currentItem ?? ''}</span
        >
      {:else if busy}
        <span>Preparing operation…</span>
      {:else}
        <span>Waiting for Tauri backend</span>
      {/if}
    </div>
  {/if}
  <form class="command-line" onsubmit={runCommand}>
    <input
      id="system-command"
      bind:this={commandInputElement}
      aria-label="System command (runs in the active folder)"
      bind:value={commandInput}
      autocomplete="off"
      spellcheck="false"
      placeholder="Enter a command to run in the active folder"
      title={`Working directory: ${displayPath(active.path)}`}
      disabled={!ready || busy || commandRunning || !active.path}
    />
  </form>
  {#if preferences.showFunctionBar}<footer>
      {#each actions as [key, action, label]}<button
          disabled={!ready || busy || commandRunning}
          onclick={() => request(action)}><kbd>{key}</kbd>{label}</button
        >{#if key === 'F2'}<button
            disabled={!ready || busy || commandRunning}
            onclick={viewCurrentFile}><kbd>F3</kbd>View</button
          >{/if}{/each}<button
        class:pressed={previewMode}
        aria-pressed={previewMode}
        disabled={!ready || busy || commandRunning}
        onclick={togglePreviewPane}
        title="Toggle preview pane"><kbd>Shift+F3</kbd>Preview pane</button
      ><button
        disabled={!ready || busy || commandRunning}
        onclick={() => {
          quickFindClose();
          settingsOpen = true;
        }}><kbd>F9</kbd>Settings</button
      >
    </footer>{/if}
</main>
{#if viewer}<FileViewerDialog {...viewer} onclose={closeViewer} />{/if}
{#if dialog}<OperationDialog
    {...dialog}
    onsubmit={submit}
    onclose={closeDialog}
  />{/if}
{#if conflict}<ConflictDialog {conflict} onresolve={resolve} />{/if}
{#if selectionDialog}<GlobSelectionDialog
    mode={selectionDialog.mode}
    onsubmit={applyGlobSelection}
    onclose={closeSelectionDialog}
  />{/if}
{#if settingsOpen}<SettingsDialog
    fileFontSize={preferences.fileFontSize}
    showHidden={preferences.showHidden}
    showFunctionBar={preferences.showFunctionBar}
    onsubmit={applySettings}
    onclose={closeSettings}
  />{/if}
