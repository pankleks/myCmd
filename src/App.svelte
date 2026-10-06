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
  import SearchDialog from './components/SearchDialog.svelte';
  import ShortcutsDialog from './components/ShortcutsDialog.svelte';
  import { cdTarget } from './utils/cdCommand';
  import FileViewerDialog from './components/FileViewerDialog.svelte';
  import FilePreviewPanel from './components/FilePreviewPanel.svelte';
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
    dirSizing,
    cancelDirectorySizing,
    startSearch,
    leaveSearch,
    type Side,
    type SelectionMode,
  } from './state/commander.svelte';
  import {
    preferences,
    loadPreferences,
    scheduleSave,
    setShowHidden,
  } from './state/preferences.svelte';
  import { handleControlShortcut, swapPanels } from './state/controlShortcuts';
  import { api, errorMessage } from './filesystem/api';
  import {
    isLocalPath,
    parentFocus,
    parseLocation,
    searchSession,
    cancelSearch,
  } from './filesystem/providers';
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
  import { createOperationController } from './operations/controller.svelte';
  import { createCommandController } from './operations/commandController.svelte';
  import { createDirectoryRefresh } from './utils/directoryRefresh';
  let ready = $state(false);
  let rightAltDown = false;
  let previewMode = $state(false);
  let error = $state('');
  let errorTitle = $state('Error');
  const operations = createOperationController({
    start: api.start,
    cancel: api.cancel,
    resolve: api.resolve,
    failed: (cause) => showError(errorMessage(cause), 'Operation failed'),
    completed: async (payload, side) => {
      await Promise.all([
        load(
          commander.left,
          undefined,
          side === 'left' ? payload.resultPath : undefined,
        ),
        load(
          commander.right,
          undefined,
          side === 'right' ? payload.resultPath : undefined,
        ),
      ]);
      commander[side].selected = new Set();
    },
  });
  let busy = $derived(operations.state.busy);
  const commands = createCommandController({
    run: api.runCommand,
    cancel: api.cancelCommand,
    refresh: () => Promise.all([load(commander.left), load(commander.right)]),
    report: (command, output) => {
      commandError = { command, output };
    },
    cancelFailed: (cause) => showError(errorMessage(cause)),
  });
  let commandRunning = $derived(commands.state.busy);
  let commandInput = $state('');
  let commandError = $state<{ command: string; output: string }>();
  let selectionDialog = $state<{ mode: SelectionMode }>();
  let settingsOpen = $state(false);
  let shortcutsOpen = $state(false);
  function showShortcuts() {
    quickFindClose();
    shortcutsOpen = true;
  }
  function closeShortcuts() {
    shortcutsOpen = false;
    void tick().then(focusPanel);
  }
  let searchDialog = $state<{ side: Side; folder: string }>();
  let viewer = $state<{
    path: string;
    name: string;
    extension: string;
    isDirectory?: boolean;
  }>();
  let commandInputElement: HTMLInputElement;
  let progress = $derived(operations.state.progress);
  let conflict = $derived(operations.state.conflict);
  let dialog = $state<{
    action: Action;
    entries: FileEntry[];
    parent: string;
    destination: string;
    side: Side;
    permanent: boolean;
  }>();
  let active = $derived(commander[commander.activePanel]);
  let previewSide = $derived(
    commander.activePanel === 'left' ? 'right' : 'left',
  );
  let previewTarget = $derived.by(() => {
    if (
      !previewMode ||
      active.loading ||
      (!isLocalPath(active.path) && !searchSession(active.path))
    )
      return undefined;
    const entry = rows(active)[active.cursor];
    if (!entry || entry.parentEntry) return undefined;
    return {
      path: entry.path,
      name: entry.name,
      extension: entry.extension,
      isDirectory: entry.type === 'directory',
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
  let measurableProgress = $derived(
    !!progress && (progress.totalBytes > 0 || progress.totalItems > 0),
  );
  $effect(() => {
    if (!ready) return;
    const paths = [commander.left.path, commander.right.path].filter(
      (path) => path && isLocalPath(path),
    );
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
    preferences.pinnedDirectories;
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
  async function openDrivePicker(side: Side) {
    if (previewMode && side !== commander.activePanel) previewMode = false;
    commander.activePanel = side;
    await tick();
    const picker = document.getElementById(
      `drive-${side}`,
    ) as HTMLButtonElement | null;
    if (!picker) return;
    picker.focus();
    picker.click();
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
  function requestSearch() {
    if (
      !ready ||
      busy ||
      commandRunning ||
      dialog ||
      selectionDialog ||
      settingsOpen ||
      viewer ||
      error ||
      searchDialog
    )
      return;
    const location = parseLocation(active.path);
    const folder =
      searchSession(active.path)?.root ??
      (location.kind === 'archive'
        ? location.archivePath.replace(/[\\/][^\\/]+$/, '')
        : active.path);
    quickFindClose();
    searchDialog = { side: commander.activePanel, folder };
  }
  function closeSearchDialog() {
    searchDialog = undefined;
    void tick().then(focusPanel);
  }
  function submitSearch(folder: string, pattern: string) {
    const panel = commander[searchDialog!.side];
    closeSearchDialog();
    void startSearch(panel, folder, pattern);
  }
  function closeViewer() {
    viewer = undefined;
    void tick().then(focusPanel);
  }
  function viewCurrentFile() {
    if (!ready || busy || commandRunning || active.loading) return;
    if (!isLocalPath(active.path) && !searchSession(active.path)) {
      showError('Archive previews are not supported yet.');
      return;
    }
    const entry = rows(active)[active.cursor];
    if (!entry || entry.parentEntry) return;
    quickFindClose();
    // Note: every real directory arrives with directoryTarget === true
    // (backend marks navigable targets), so check the type first.
    if (entry.type === 'directory') {
      viewer = {
        path: entry.path,
        name: entry.name,
        extension: entry.extension,
        isDirectory: true,
      };
      return;
    }
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
  async function openTerminal(path: string) {
    if (!isLocalPath(path)) {
      showError('Terminals can only be opened in local directories.');
      return;
    }
    try {
      await api.openTerminal(path);
    } catch (cause) {
      showError(errorMessage(cause), 'Unable to open terminal');
    }
  }
  function request(action: Action, permanent = false) {
    if (
      !ready ||
      busy ||
      commandRunning ||
      dialog ||
      selectionDialog ||
      searchDialog ||
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
    const opposite =
      commander[commander.activePanel === 'left' ? 'right' : 'left'];
    if (
      (!isLocalPath(active.path) &&
        !searchSession(active.path) &&
        action !== 'copy' &&
        action !== 'delete') ||
      ((action === 'copy' || action === 'move') &&
        !isLocalPath(opposite.path) &&
        !searchSession(opposite.path))
    ) {
      showError(
        'Archives support F5 extraction to a local directory and F8 permanent deletion. Other changes are not supported yet.',
      );
      return;
    }
    if (action !== 'createDirectory' && !entries.length) return;
    if (action === 'rename' && entries.length !== 1) {
      showError('Select exactly one item to rename.');
      return;
    }
    error = '';
    dialog = {
      action,
      entries,
      parent: searchSession(active.path)?.root ?? active.path,
      destination: searchSession(opposite.path)?.root ?? opposite.path,
      side: commander.activePanel,
      permanent:
        action === 'delete' &&
        (permanent ||
          (!isLocalPath(active.path) && !searchSession(active.path))),
    };
  }
  async function submit(operation: FileOperation) {
    const side = dialog!.side;
    const location = parseLocation(dialog!.parent);
    if (operation.type === 'delete' && location.kind === 'archive') {
      operation = {
        type: 'deleteArchive',
        archivePath: location.archivePath,
        directory: location.directory,
        members: dialog!.entries.map((entry) => entry.name),
      };
    }
    if (operation.type === 'copy' && location.kind === 'archive') {
      operation = {
        type: 'extract',
        archivePath: location.archivePath,
        directory: location.directory,
        members: dialog!.entries.map((entry) => entry.name),
        destination: operation.destination,
      };
    }
    closeDialog();
    await operations.start(operation, side);
  }
  async function resolve(resolution: Resolution) {
    if (!conflict) return;
    try {
      await operations.resolve(resolution);
      await tick();
      focusPanel();
    } catch (e) {
      throw new Error(errorMessage(e));
    }
  }
  async function cancel() {
    await operations.cancel();
  }
  async function runCommand(event?: SubmitEvent) {
    event?.preventDefault();
    const command = commandInput.trim();
    if (
      !command ||
      !ready ||
      busy ||
      commandRunning ||
      !active.path ||
      !isLocalPath(active.path)
    )
      return;
    commandInput = '';
    error = '';
    const target = cdTarget(command, active.path);
    if (target !== undefined) {
      const panel = active;
      await load(panel, target);
      if (panel.error) showError(panel.error);
      await tick();
      focusPanel();
      return;
    }
    await commands.run(command, active.path);
  }
  onMount(() => {
    let disposed = false;
    const directoryRefresh = createDirectoryRefresh<Side>({
      path: (side) => commander[side].path,
      enabled: () => ready && !busy && !commandRunning && !disposed,
      refresh: (side) => load(commander[side]),
      failed: (cause) => showError(errorMessage(cause)),
    });
    const unlisten: UnlistenFn[] = [];
    let rootsRefreshing = false;
    const rootsTimer = setInterval(async () => {
      if (!ready || disposed || rootsRefreshing) return;
      rootsRefreshing = true;
      try {
        const roots = await api.roots();
        if (
          !disposed &&
          JSON.stringify(roots) !== JSON.stringify(commander.roots)
        )
          commander.roots = roots;
      } catch {
        // Keep the last known drives on transient errors; retry next time.
      } finally {
        rootsRefreshing = false;
      }
    }, 2000);
    async function init() {
      if (!isTauri()) {
        showError(
          'File access requires the desktop app. Run: npm run tauri dev',
        );
        return;
      }
      try {
        const p = await listen<Progress>('operation-progress', ({ payload }) =>
          operations.progress(payload),
        );
        unlisten.push(p);
        const c = await listen<Conflict>('operation-conflict', ({ payload }) =>
          operations.conflict(payload),
        );
        unlisten.push(c);
        const filesystemChanges = await listen<string>(
          'filesystem-changed',
          ({ payload }) => {
            for (const side of ['left', 'right'] as const) {
              directoryRefresh.schedule(side, payload);
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
      clearInterval(rootsTimer);
      operations.dispose();
      directoryRefresh.dispose();
      unlisten.forEach((fn) => fn());
    };
  });
  function previewShortcut(event: KeyboardEvent) {
    if (shortcutsOpen) return;
    if (
      dialog ||
      selectionDialog ||
      settingsOpen ||
      viewer ||
      conflict ||
      commandError ||
      error ||
      event.isComposing
    )
      return;
    if (
      event.key === 'Tab' &&
      event.shiftKey &&
      !event.ctrlKey &&
      !event.metaKey &&
      !event.altKey
    ) {
      event.preventDefault();
      event.stopPropagation();
      if (!event.repeat && ready && !busy && !commandRunning) {
        quickFindClose();
        swapPanels();
        void tick().then(focusPanel);
      }
      return;
    }
    if (
      event.key === 'F3' &&
      event.shiftKey &&
      !event.ctrlKey &&
      !event.metaKey &&
      !event.altKey
    ) {
      event.preventDefault();
      event.stopPropagation();
      if (!event.repeat) togglePreviewPane();
    }
  }
  function keydown(event: KeyboardEvent) {
    if (shortcutsOpen) return;
    if (
      event.key === 'F1' &&
      !event.altKey &&
      !event.ctrlKey &&
      !event.metaKey &&
      !event.shiftKey &&
      !dialog &&
      !searchDialog &&
      !settingsOpen &&
      !viewer &&
      !conflict &&
      !error &&
      !commandError &&
      !selectionDialog
    ) {
      event.preventDefault();
      showShortcuts();
      return;
    }
    if (
      dialog ||
      selectionDialog ||
      settingsOpen ||
      searchDialog ||
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
      event.key === 'F7'
    ) {
      event.preventDefault();
      requestSearch();
      return;
    }
    if (
      event.altKey &&
      !event.ctrlKey &&
      !event.metaKey &&
      !event.shiftKey &&
      (event.key === 'F1' || event.key === 'F2')
    ) {
      event.preventDefault();
      void openDrivePicker(event.key === 'F1' ? 'left' : 'right');
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
      (event.key === 'Escape' || event.key === 'Backspace') &&
      searchSession(active.path) &&
      !event.ctrlKey &&
      !event.altKey &&
      !event.metaKey
    ) {
      event.preventDefault();
      void leaveSearch(active);
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
      handleControlShortcut(
        event,
        () => {
          (
            document.querySelector(
              '.panel.active .pathbar input',
            ) as HTMLInputElement
          )?.select();
        },
        (path) => {
          void openTerminal(path);
        },
      );
      return;
    }
    const altGraph =
      rightAltDown ||
      (typeof event.getModifierState === 'function' &&
        event.getModifierState('AltGraph'));
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
      isLocalPath(active.path) &&
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
          ?.clientHeight ?? 320) / 26,
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
      if (active.parent)
        void load(active, active.parent, parentFocus(active.path));
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

<svelte:window
  onkeydowncapture={(event) => {
    if (event.code === 'AltRight' || event.key === 'AltGraph')
      rightAltDown = true;
    previewShortcut(event);
  }}
  onkeydown={keydown}
  onkeyup={(event) => {
    if (event.code === 'AltRight' || event.key === 'AltGraph')
      rightAltDown = false;
  }}
  onblur={() => {
    rightAltDown = false;
  }}
/>
{#snippet previewPane()}
  <FilePreviewPanel target={previewTarget} loading={active.loading} />
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
        void tick().then(focusPanel);
      }}
    />
  {:else if commandError}<ErrorDialog
      {...commandError}
      onclose={() => {
        commandError = undefined;
        void tick().then(() => commandInputElement?.focus());
      }}
    />{/if}
  {#if dirSizing.paths.length}<button
      data-sizing-cancel
      disabled={dirSizing.cancelling}
      onclick={() => {
        void cancelDirectorySizing().catch((cause) =>
          showError(errorMessage(cause)),
        );
      }}>Cancel sizing</button
    >{/if}
  {#if commands.state.executing}<button
      data-command-cancel
      disabled={commands.state.cancelling}
      onclick={() => {
        void commands.cancel();
      }}>Cancel command</button
    >{/if}
  {#if active.loading && searchSession(active.path)}<div
      class="operation-status"
      role="status"
    >
      <div class="operation-details">
        <div class="operation-heading">
          <span class="operation-indicator" aria-hidden="true"></span>Searching
          files and folders…
        </div>
        <div class="operation-metrics">
          {active.entries.length} matches found
        </div>
        <progress aria-label="Search progress"></progress>
      </div>
      <button
        onclick={() => {
          void cancelSearch(active.path).catch((cause) =>
            showError(errorMessage(cause)),
          );
        }}>Cancel search</button
      >
    </div>{/if}
  {#if busy || !ready}
    <div class="operation-status" role="status" aria-live="polite">
      <div class="operation-details">
        <div class="operation-heading">
          <span class="operation-indicator" aria-hidden="true"></span>
          <span
            >{!ready
              ? 'Waiting for Tauri backend'
              : progress
                ? {
                    queued: 'Queued',
                    running: 'Operation in progress',
                    completed: 'Completed',
                    failed: 'Operation failed',
                    cancelled: 'Cancelled',
                  }[progress.state]
                : 'Preparing operation…'}</span
          >
          {#if measurableProgress}<span class="operation-percent"
              >{percent}%</span
            >{/if}
        </div>
        {#if busy}
          <div class="operation-metrics">
            {#if progress && measurableProgress}
              {progress.processedItems} / {progress.totalItems} items · {bytes(
                progress.processedBytes,
              )} / {bytes(progress.totalBytes)}
            {:else}
              Preparing files… Progress will appear when available.
            {/if}
          </div>
          <progress
            max="100"
            value={measurableProgress ? percent : undefined}
            aria-label="Operation progress"
          ></progress>
          {#if progress?.currentItem}<div
              class="current-item"
              title={progress.currentItem}
            >
              {progress.currentItem}
            </div>{/if}
        {/if}
      </div>
      {#if busy}<button onclick={cancel}>Cancel</button>{/if}
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
      disabled={!ready ||
        busy ||
        commandRunning ||
        !active.path ||
        !isLocalPath(active.path)}
    />
  </form>
  {#if preferences.showFunctionBar}<footer>
      <button onclick={showShortcuts}><kbd>F1</kbd>Help</button>
      {#each actions as [key, action, label]}<button
          disabled={!ready || busy || commandRunning}
          onclick={() => request(action)}><kbd>{key}</kbd>{label}</button
        >{#if key === 'F2'}<button
            disabled={!ready || busy || commandRunning}
            onclick={viewCurrentFile}><kbd>F3</kbd>View</button
          ><button
            class:pressed={previewMode}
            aria-pressed={previewMode}
            disabled={!ready || busy || commandRunning}
            onclick={togglePreviewPane}
            title="Toggle preview pane"><kbd>Shift+F3</kbd>Preview</button
          >{/if}{#if key === 'F7'}<button
            disabled={!ready || busy || commandRunning}
            onclick={requestSearch}><kbd>Alt+F7</kbd>Search</button
          >{/if}{/each}<button
        disabled={!ready || busy || commandRunning}
        onclick={() => {
          quickFindClose();
          settingsOpen = true;
        }}><kbd>F9</kbd>Settings</button
      >
    </footer>{/if}
</main>
{#if viewer}<FileViewerDialog {...viewer} onclose={closeViewer} />{/if}
{#if shortcutsOpen}<ShortcutsDialog onclose={closeShortcuts} />{/if}
{#if searchDialog}<SearchDialog
    folder={searchDialog.folder}
    onsubmit={submitSearch}
    onclose={closeSearchDialog}
  />{/if}
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
