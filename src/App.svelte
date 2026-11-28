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
  import PropertiesDialog from './components/PropertiesDialog.svelte';
  import ConnectionsDialog from './components/ConnectionsDialog.svelte';
  import ContextMenu, { type MenuItem } from './components/ContextMenu.svelte';
  import SearchDialog from './components/SearchDialog.svelte';
  import ShortcutsDialog from './components/ShortcutsDialog.svelte';
  import { cdTarget } from './utils/cdCommand';
  import FileViewerDialog from './components/FileViewerDialog.svelte';
  import FilePreviewPanel from './components/FilePreviewPanel.svelte';
  import {
    commander,
    load,
    rows,
    previewEntry,
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
    sortMemory,
    type Side,
    type SelectionMode,
  } from './state/commander.svelte';
  import {
    preferences,
    loadPreferences,
    scheduleSave,
    flushSave,
    setShowHidden,
  } from './state/preferences.svelte';
  import { handleControlShortcut, swapPanels } from './state/controlShortcuts';
  import { createFileClipboard } from './state/fileClipboard';
  import { api, errorMessage } from './filesystem/api';
  import {
    isLocalPath,
    parentFocus,
    parseLocation,
    searchSession,
    locationKey,
    sftpNames,
    cancelSearch,
  } from './filesystem/providers';
  import type { FileEntry, SftpConnectResult } from './filesystem/types';
  import type {
    Conflict,
    FileOperation,
    Progress,
    Resolution,
  } from './operations/types';
  import { bytes } from './utils/format';
  import { displayPath } from './utils/paths';
  import { isQuickFindTrigger, modifiers } from './utils/keyboard';
  import { fitFunctionBar } from './utils/fitFunctionBar';
  import { createOperationController } from './operations/controller.svelte';
  import { createCommandController } from './operations/commandController.svelte';
  import { createDirectoryRefresh } from './utils/directoryRefresh';
  import { sftpActionError } from './state/sftpRules';
  import { routeOperation } from './operations/route';
  import { skippedLinksMessage } from './operations/skippedLinks';
  import { contextMenuItems } from './state/contextMenu';
  import { galleryImages, type GalleryImage } from './utils/imageGallery';
  import { isImageFile } from './utils/viewer';
  let ready = $state(false);
  let rightAltDown = false;
  let previewMode = $state(false);
  let error = $state('');
  let errorTitle = $state('Error');
  let createdFileEditor: { editor: string | null } | undefined;
  // Reloading a search panel runs the whole search again, so only the side
  // whose results an operation changed (`search: true`) is searched again.
  function reloadPanels(focus?: {
    side: Side;
    path?: string;
    search?: boolean;
  }) {
    return Promise.all(
      (['left', 'right'] as const).map((side) => {
        const panel = commander[side];
        const mine = side === focus?.side;
        if (searchSession(panel.path) && !(mine && focus?.search)) return;
        return load(panel, undefined, mine ? focus?.path : undefined);
      }),
    );
  }
  const operations = createOperationController({
    start: api.start,
    cancel: api.cancel,
    resolve: api.resolve,
    failed: (cause) => {
      createdFileEditor = undefined;
      if (retryDialog) {
        dialog = { ...retryDialog, error: errorMessage(cause) };
        retryDialog = undefined;
        return;
      }
      showError(errorMessage(cause), 'Operation failed');
    },
    completed: async (payload, side) => {
      retryDialog = undefined;
      const editor = createdFileEditor;
      createdFileEditor = undefined;
      await reloadPanels({ side, path: payload.resultPath, search: true });
      commander[side].selected = new Set();
      const linksMessage =
        payload.state === 'completed'
          ? skippedLinksMessage(payload.skippedLinks, payload.resultPath)
          : undefined;
      if (linksMessage) showError(linksMessage, 'Compression finished');
      if (editor && payload.state === 'completed' && payload.resultPath)
        await editFile(payload.resultPath, editor.editor);
    },
  });
  let busy = $derived(operations.state.busy);
  const commands = createCommandController({
    run: api.runCommand,
    cancel: api.cancelCommand,
    refresh: () => reloadPanels(),
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
  let propertiesDialog = $state<{ path: string; side: Side }>();
  let connectionsDialog = $state<{
    side: Side;
    initialId?: string;
    target?: string;
  }>();
  let shortcutsOpen = $state(false);
  let contextMenu = $state<{
    side: Side;
    x: number;
    y: number;
    item: boolean;
    paste: boolean;
  }>();
  let contextRequest = 0;
  let systemActionsRunning = $state(false);
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
    side: Side;
    images?: GalleryImage[];
    index?: number;
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
    value?: string;
    error?: string;
  }>();
  // A failed rename or new folder/file reopens its dialog with the typed name.
  let retryDialog: typeof dialog;
  let active = $derived(commander[commander.activePanel]);
  // Either panel's search keeps its progress and Cancel while the other is active.
  let searchingPanel = $derived(
    [commander.left, commander.right].find(
      (panel) => panel.loading && !!searchSession(panel.path),
    ),
  );
  let modalOpen = $derived(
    !!(
      dialog ||
      selectionDialog ||
      settingsOpen ||
      propertiesDialog ||
      connectionsDialog ||
      searchDialog ||
      viewer ||
      conflict ||
      commandError ||
      error ||
      shortcutsOpen
    ),
  );
  const fileClipboard = createFileClipboard({
    available: () =>
      isTauri() && ready && !busy && !commandRunning && !modalOpen,
    start: (operation, side) => operations.start(operation, side),
    failed: (cause) =>
      showError(errorMessage(cause), 'Clipboard operation failed'),
  });
  function closeContextMenu(restoreFocus = true) {
    contextRequest++;
    contextMenu = undefined;
    if (restoreFocus) focusPanel();
  }
  async function showContextMenu(
    side: Side,
    x: number,
    y: number,
    item: boolean,
  ) {
    if (
      !ready ||
      busy ||
      commandRunning ||
      modalOpen ||
      commander[side].loading
    )
      return;
    commander.activePanel = side;
    quickFindClose();
    const menu = { side, x, y, item, paste: false };
    const requestId = ++contextRequest;
    contextMenu = menu;
    if (!isLocalPath(commander[side].path)) return;
    try {
      const clipboard = await api.readFilesFromClipboard();
      if (contextMenu && requestId === contextRequest)
        contextMenu = { ...menu, paste: clipboard.paths.length > 0 };
    } catch {
      // Clipboard inspection must not prevent unrelated context-menu actions.
    }
  }
  async function copyContextPath() {
    const entry = rows(active)[active.cursor];
    if (!entry) return;
    try {
      await api.copyText(displayPath(entry.path));
    } catch (cause) {
      showError(errorMessage(cause), 'Copy path failed');
    }
  }
  async function showSystemActions(path: string) {
    if (systemActionsRunning) return;
    systemActionsRunning = true;
    try {
      await api.systemActions(path);
      await reloadPanels();
    } catch (cause) {
      showError(errorMessage(cause), 'System actions failed');
    } finally {
      systemActionsRunning = false;
    }
  }
  const contextItems = $derived.by((): MenuItem[] => {
    if (!contextMenu) return [];
    const side = contextMenu.side;
    const panel = commander[side];
    const opposite = commander[side === 'left' ? 'right' : 'left'];
    const entry = rows(panel)[panel.cursor];
    return contextMenuItems(
      {
        panelPath: panel.path,
        oppositePath: opposite.path,
        oppositeRoot: searchSession(opposite.path)?.root ?? opposite.path,
        item: contextMenu.item,
        paste: contextMenu.paste,
        entry,
        entries: sources(panel),
        systemActionsRunning,
      },
      {
        request,
        connections: () => requestConnections(side),
        refresh: () => void load(panel),
        open: () => void open(panel, entry),
        openTerminal: () => void openTerminal(panel.path),
        paste: () => void fileClipboard.run('paste'),
        copy: () => void fileClipboard.run('copy'),
        cut: () => void fileClipboard.run('cut'),
        view: viewCurrentFile,
        edit: editCurrentFile,
        copyPath: () => void copyContextPath(),
        properties: requestProperties,
        systemActions: () => void showSystemActions(entry.path),
      },
    );
  });
  let previewSide = $derived(
    commander.activePanel === 'left' ? 'right' : 'left',
  );
  let previewTarget = $derived.by(() => {
    if (!previewMode) return undefined;
    const entry = previewEntry(active);
    if (!entry) return undefined;
    return {
      path: entry.path,
      name: entry.name,
      extension: entry.extension,
      isDirectory: entry.type === 'directory',
      key: `${commander.activePanel}:${entry.path}:${entry.size}:${entry.modified ?? ''}`,
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
    preferences.fileRowSpacing;
    preferences.columnWidths;
    preferences.showHidden;
    preferences.showFunctionBar;
    preferences.pinnedDirectories;
    preferences.editor;
    sortMemory.byPath;
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
  function requestConnections(side: Side, target?: string) {
    if (!ready || busy || commandRunning || modalOpen) return;
    commander.activePanel = side;
    const location = parseLocation(target ?? commander[side].path);
    quickFindClose();
    connectionsDialog = {
      side,
      initialId: location.kind === 'sftp' ? location.connectionId : undefined,
      target,
    };
  }
  function closeConnections() {
    connectionsDialog = undefined;
    void tick().then(focusPanel);
  }
  async function connected(
    connection: Extract<SftpConnectResult, { status: 'connected' }>,
  ) {
    const panel = commander[connectionsDialog!.side];
    const target =
      connectionsDialog!.initialId === connection.id
        ? connectionsDialog!.target
        : undefined;
    sftpNames.set(connection.id, connection.name);
    closeConnections();
    await load(
      panel,
      target ??
        locationKey({
          kind: 'sftp',
          connectionId: connection.id,
          path: connection.path,
        }),
    );
    focusPanel();
  }
  async function disconnected(id: string) {
    const home =
      commander.roots.find((root) => root.type === 'home')?.path ?? '~';
    for (const panel of [commander.left, commander.right]) {
      const location = parseLocation(panel.path);
      if (location.kind === 'sftp' && location.connectionId === id)
        await load(panel, home);
    }
    closeConnections();
  }
  function requestSearch() {
    if (!ready || busy || commandRunning || modalOpen) return;
    const location = parseLocation(active.path);
    if (location.kind === 'sftp') {
      showError('Recursive search is not available on SFTP connections.');
      return;
    }
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
  function submitSearch(folder: string, pattern: string, text?: string) {
    const panel = commander[searchDialog!.side];
    closeSearchDialog();
    void startSearch(panel, folder, pattern, text);
  }
  function closeViewer() {
    viewer = undefined;
    void tick().then(focusPanel);
  }
  function navigateViewer(index: number) {
    const current = viewer;
    const image = current?.images?.[index];
    if (!current || !image) return;
    viewer = {
      ...current,
      path: image.path,
      name: image.name,
      extension: image.extension,
      index,
    };
    // Esc then leaves the cursor on the last image shown.
    const panel = commander[current.side];
    const row = rows(panel).findIndex((entry) => entry.path === image.path);
    if (row >= 0) panel.cursor = row;
  }
  function requestProperties() {
    if (propertiesDialog) return;
    if (!ready || busy || commandRunning || active.loading) return;
    const entry = rows(active)[active.cursor];
    if (!entry || entry.parentEntry) return;
    if (active.selected.size > 1) {
      showError(
        'Properties supports one item at a time. Clear the selection first.',
      );
      return;
    }
    if (!isLocalPath(entry.path)) {
      showError('Properties is available for local files and folders only.');
      return;
    }
    quickFindClose();
    propertiesDialog = { path: entry.path, side: commander.activePanel };
  }
  function closeProperties() {
    propertiesDialog = undefined;
    void tick().then(focusPanel);
  }
  async function refreshProperties() {
    const target = propertiesDialog!;
    await reloadPanels({ side: target.side, path: target.path });
  }
  function viewCurrentFile() {
    if (!ready || busy || commandRunning) return;
    if (
      !isLocalPath(active.path) &&
      !searchSession(active.path) &&
      parseLocation(active.path).kind !== 'sftp'
    ) {
      showError('Previews are not available inside archives.');
      return;
    }
    const entry = previewEntry(active);
    if (!entry) return;
    quickFindClose();
    // Note: every real directory arrives with directoryTarget === true
    // (backend marks navigable targets), so check the type first.
    if (entry.type === 'directory') {
      viewer = {
        path: entry.path,
        name: entry.name,
        extension: entry.extension,
        isDirectory: true,
        side: commander.activePanel,
      };
      return;
    }
    const images = isImageFile(entry.extension)
      ? galleryImages(rows(active))
      : [];
    const index = images.findIndex((image) => image.path === entry.path);
    viewer = {
      path: entry.path,
      name: entry.name,
      extension: entry.extension,
      side: commander.activePanel,
      ...(index >= 0 ? { images, index } : {}),
    };
  }
  function togglePreviewPane() {
    if (!ready || busy || commandRunning) return;
    quickFindClose();
    previewMode = !previewMode;
    void tick().then(focusPanel);
  }
  async function editFile(path: string, editor = preferences.editor) {
    try {
      await api.edit(path, editor);
    } catch (cause) {
      showError(errorMessage(cause), 'Open editor failed');
    }
  }
  function editCurrentFile() {
    if (!ready || busy || commandRunning || active.loading) return;
    if (!isLocalPath(active.path) && !searchSession(active.path)) {
      showError('Editing remote or archived files is not supported.');
      return;
    }
    const entry = rows(active)[active.cursor];
    if (!entry || entry.parentEntry) return;
    if (entry.type === 'directory' || entry.directoryTarget) {
      showError('Only files can be edited.');
      return;
    }
    quickFindClose();
    void editFile(entry.path);
  }
  function applySettings(settings: {
    fileFontSize: number;
    fileRowSpacing: number;
    showHidden: boolean;
    showFunctionBar: boolean;
    editor: string | null;
  }) {
    preferences.fileFontSize = settings.fileFontSize;
    preferences.fileRowSpacing = settings.fileRowSpacing;
    preferences.showFunctionBar = settings.showFunctionBar;
    preferences.editor = settings.editor;
    setShowHidden(settings.showHidden);
    closeSettings();
  }
  function showError(message: string, title = 'Error') {
    errorTitle = title;
    error = message;
  }
  async function openTerminal(path: string) {
    if (!isLocalPath(path)) {
      showError('Terminals can only be opened in local folders.');
      return;
    }
    try {
      await api.openTerminal(path);
    } catch (cause) {
      showError(errorMessage(cause), 'Open terminal failed');
    }
  }
  function request(action: Action, permanent = false) {
    if (
      !ready ||
      busy ||
      commandRunning ||
      modalOpen ||
      active.loading ||
      !active.path
    )
      return;
    const entries =
      action === 'createDirectory'
        ? rows(active)
            .slice(active.cursor, active.cursor + 1)
            .filter((entry) => !entry.parentEntry)
        : sources(active);
    const opposite =
      commander[commander.activePanel === 'left' ? 'right' : 'left'];
    const sourceLocation = parseLocation(active.path);
    const targetLocation = parseLocation(
      searchSession(opposite.path)?.root ?? opposite.path,
    );
    const sftpError = sftpActionError(
      action,
      sourceLocation.kind,
      targetLocation.kind,
    );
    if (sftpError) {
      showError(sftpError);
      return;
    }
    if (
      action === 'compress' &&
      ((!isLocalPath(active.path) && !searchSession(active.path)) ||
        !isLocalPath(searchSession(opposite.path)?.root ?? opposite.path) ||
        entries.some((entry) => !isLocalPath(entry.path)))
    ) {
      showError(
        'ZIP compression requires local source items and a local destination folder.',
      );
      return;
    }
    if (
      (sourceLocation.kind === 'archive' &&
        !searchSession(active.path) &&
        action !== 'copy' &&
        action !== 'delete') ||
      ((action === 'copy' || action === 'move') &&
        targetLocation.kind !== 'local' &&
        targetLocation.kind !== 'sftp')
    ) {
      showError(
        'Archives support F5 extraction to a local folder and F8 permanent deletion. Other changes are not supported yet.',
      );
      return;
    }
    if (
      action !== 'createDirectory' &&
      action !== 'createFile' &&
      !entries.length
    )
      return;
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
    retryDialog =
      operation.type === 'rename' ||
      operation.type === 'createDirectory' ||
      operation.type === 'createFile'
        ? { ...dialog!, value: operation.name, error: undefined }
        : undefined;
    operation = routeOperation(
      operation,
      dialog!.parent,
      dialog!.destination,
      dialog!.entries,
    );
    closeDialog();
    createdFileEditor =
      operation.type === 'createFile'
        ? { editor: preferences.editor }
        : undefined;
    await operations.start(
      operation,
      operation.type === 'compress'
        ? side === 'left'
          ? 'right'
          : 'left'
        : side,
    );
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
  // shortcut: best effort. The save is sent before the window goes away, but
  // the webview may close before the backend finishes writing it.
  onMount(() => {
    window.addEventListener('beforeunload', flushSave);
    return () => window.removeEventListener('beforeunload', flushSave);
  });
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
        const notice = await api.takeConfigNotice().catch(() => null);
        if (notice && !disposed) showError(notice, 'Settings were reset');
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
    if (contextMenu || modalOpen || event.isComposing) return;
    if (event.key === 'Tab' && modifiers(event) === 'shift') {
      // In the path bar and other fields Shift+Tab moves focus back as usual;
      // only the panels and the command line swap panels.
      const target = event.target as HTMLElement;
      if (
        target.closest('input, select, textarea') &&
        target !== commandInputElement
      )
        return;
      event.preventDefault();
      event.stopPropagation();
      if (!event.repeat && ready && !busy && !commandRunning) {
        quickFindClose();
        swapPanels();
        void tick().then(focusPanel);
      }
      return;
    }
    if (event.key === 'F3' && modifiers(event) === 'shift') {
      event.preventDefault();
      event.stopPropagation();
      if (!event.repeat) togglePreviewPane();
    }
  }
  function keydown(event: KeyboardEvent) {
    if (contextMenu) return;
    if (shortcutsOpen) return;
    if (event.key === 'F1' && modifiers(event) === '' && !modalOpen) {
      event.preventDefault();
      showShortcuts();
      return;
    }
    if (modalOpen || event.defaultPrevented || event.isComposing) return;
    const target = event.target as HTMLElement;
    if (fileClipboard.handle(event)) return;
    if (
      event.key === 'Enter' &&
      modifiers(event) === 'ctrl' &&
      (!target.closest('input, select, textarea, dialog') ||
        target === commandInputElement)
    ) {
      event.preventDefault();
      const entry = rows(active)[active.cursor];
      if (
        !ready ||
        busy ||
        commandRunning ||
        active.loading ||
        !isLocalPath(active.path) ||
        !entry ||
        entry.parentEntry
      )
        return;
      quickFindClose();
      commandInput += `${commandInput && !/\s$/.test(commandInput) ? ' ' : ''}${entry.name}`;
      void tick().then(() => {
        commandInputElement.focus();
        commandInputElement.setSelectionRange(
          commandInput.length,
          commandInput.length,
        );
      });
      return;
    }
    if (modifiers(event) === 'alt' && event.key === 'F7') {
      event.preventDefault();
      requestSearch();
      return;
    }
    if (
      modifiers(event) === 'alt' &&
      (event.key === 'F1' || event.key === 'F2')
    ) {
      event.preventDefault();
      void openDrivePicker(event.key === 'F1' ? 'left' : 'right');
      return;
    }
    // Typing moves focus into the command line; panel keys must keep working
    // there (as in Total Commander). Left/Right/Home/End/Backspace stay text keys.
    const inCommandLine = target === commandInputElement;
    if (inCommandLine && event.key === 'Escape' && modifiers(event) === '') {
      event.preventDefault();
      commandInput = '';
      focusPanel();
      return;
    }
    const panelKey =
      /^F\d+$/.test(event.key) ||
      (modifiers(event) === '' &&
        ['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Tab'].includes(
          event.key,
        ));
    if (
      target.closest('input, select, textarea, dialog') &&
      !(inCommandLine && panelKey)
    ) {
      if (commander.quickFind && event.key === 'Escape') {
        event.preventDefault();
        quickFindClose();
        focusPanel();
        return;
      }
      return;
    }
    if (event.key === 'F5' && modifiers(event) === 'alt') {
      event.preventDefault();
      quickFindClose();
      request('compress');
      return;
    }
    if (event.key === 'Enter' && modifiers(event) === 'alt') {
      event.preventDefault();
      requestProperties();
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
    if (event.key === 'F3' && modifiers(event) === '') {
      event.preventDefault();
      viewCurrentFile();
      return;
    }
    if (event.key === 'F4' && !ctrl && !event.altKey) {
      event.preventDefault();
      if (event.shiftKey) request('createFile');
      else editCurrentFile();
      return;
    }
    const action =
      actions.find(([key]) => key === event.key)?.[1] ??
      (event.key === 'Delete' ? 'delete' : undefined);
    // Only the plain key, or Shift for permanent delete: Ctrl+F8 or Shift+F5
    // must not quietly run the plain action.
    const held = modifiers(event);
    if (action && (held === '' || (held === 'shift' && action === 'delete'))) {
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
    // A focused inline preview (Markdown, PDF) scrolls with these keys.
    if (
      target.closest('.preview-panel') &&
      /^(Arrow|Page)|^(Home|End)$/.test(event.key)
    )
      return;
    const list = rows(active);
    const page = Math.max(
      1,
      Math.floor(
        (document.getElementById(`list-${commander.activePanel}`)
          ?.clientHeight ?? 320) /
          (26 + preferences.fileRowSpacing), // same row height as FilePanel
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
      bind:panel={commander.left}
      side="left"
      roots={commander.roots}
      oncontext={showContextMenu}
      onconnections={requestConnections}
      hidden={previewMode && previewSide === 'left'}
    />
    {#if previewMode && previewSide === 'left'}{@render previewPane()}{/if}
    <FilePanel
      bind:panel={commander.right}
      side="right"
      roots={commander.roots}
      oncontext={showContextMenu}
      onconnections={requestConnections}
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
  {#if searchingPanel}<div class="operation-status" role="status">
      <div class="operation-details">
        <div class="operation-heading">
          <span class="operation-indicator" aria-hidden="true"></span>Searching
          files and folders…
        </div>
        <div class="operation-metrics">
          {searchingPanel.entries.length}
          {searchingPanel.entries.length === 1 ? 'match' : 'matches'} found
        </div>
        <progress aria-label="Search progress"></progress>
      </div>
      <button
        onclick={() => {
          void cancelSearch(searchingPanel.path).catch((cause) =>
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
              ? 'Starting…'
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
          <!-- Live region: announce state changes, not every percent or file. -->
          {#if measurableProgress}<span
              class="operation-percent"
              aria-live="off">{percent}%</span
            >{/if}
        </div>
        {#if busy}
          <div class="operation-metrics" aria-live="off">
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
              aria-live="off"
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
  {#if preferences.showFunctionBar}<footer
      class="function-bar"
      use:fitFunctionBar
    >
      <button onclick={showShortcuts} title="F1 — Help"
        ><kbd>F1</kbd><span>Help</span></button
      >
      {#each actions as [key, action, label]}<button
          disabled={!ready || busy || commandRunning}
          onclick={() => request(action)}
          title={`${key} — ${label}`}
          ><kbd>{key}</kbd><span>{label}</span></button
        >{#if key === 'F2'}<button
            disabled={!ready || busy || commandRunning}
            onclick={viewCurrentFile}
            title="F3 — View"><kbd>F3</kbd><span>View</span></button
          ><button
            class:pressed={previewMode}
            aria-pressed={previewMode}
            disabled={!ready || busy || commandRunning}
            onclick={togglePreviewPane}
            title="Shift+F3 — Toggle preview pane"
            ><kbd>Shift+F3</kbd><span>Preview</span></button
          ><button
            disabled={!ready || busy || commandRunning}
            onclick={editCurrentFile}
            title="F4 — Edit"><kbd>F4</kbd><span>Edit</span></button
          >{/if}{#if key === 'F7'}<button
            disabled={!ready || busy || commandRunning}
            onclick={requestSearch}
            title="Alt+F7 — Search"><kbd>Alt+F7</kbd><span>Search</span></button
          >{/if}{/each}<button
        disabled={!ready || busy || commandRunning}
        onclick={() => {
          quickFindClose();
          settingsOpen = true;
        }}
        title="F9 — Settings"><kbd>F9</kbd><span>Settings</span></button
      >
      <button
        disabled={!ready || busy || commandRunning || active.loading}
        onclick={requestProperties}
        title="Alt+Enter — Properties of the focused local file or folder"
        ><kbd>Alt+Enter</kbd><span>Properties</span></button
      >
    </footer>{/if}
</main>
{#if contextMenu}<ContextMenu
    x={contextMenu.x}
    y={contextMenu.y}
    items={contextItems}
    onclose={closeContextMenu}
  />{/if}
{#if viewer}<FileViewerDialog
    path={viewer.path}
    name={viewer.name}
    extension={viewer.extension}
    isDirectory={viewer.isDirectory}
    images={viewer.images}
    index={viewer.index}
    onnavigate={navigateViewer}
    onclose={closeViewer}
  />{/if}
{#if propertiesDialog}<PropertiesDialog
    path={propertiesDialog.path}
    onclose={closeProperties}
    onapplied={refreshProperties}
  />{/if}
{#if connectionsDialog}<ConnectionsDialog
    initialId={connectionsDialog.initialId}
    onconnected={connected}
    ondisconnected={disconnected}
    onclose={closeConnections}
  />{/if}
{#if shortcutsOpen}<ShortcutsDialog onclose={closeShortcuts} />{/if}
{#if searchDialog}<SearchDialog
    folder={searchDialog.folder}
    roots={commander.roots}
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
    fileRowSpacing={preferences.fileRowSpacing}
    showHidden={preferences.showHidden}
    showFunctionBar={preferences.showFunctionBar}
    editor={preferences.editor}
    onsubmit={applySettings}
    onclose={closeSettings}
  />{/if}
