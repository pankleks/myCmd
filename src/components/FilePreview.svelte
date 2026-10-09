<script lang="ts">
  import Icon from '@iconify/svelte';
  import { icons } from '../utils/uiIcons';
  import { onDestroy, onMount, tick, type Snippet } from 'svelte';
  import { api, errorMessage } from '../filesystem/api';
  import { isImageFile, isMarkdownFile } from '../utils/viewer';
  import { formatJsonPreview } from '../utils/jsonPreview';
  import { highlightPreviewMatches } from '../utils/previewSearch';
  import DirectorySummary from './DirectorySummary.svelte';
  import PdfPreview from './PdfPreview.svelte';

  let {
    path,
    name,
    extension,
    isDirectory = false,
    autofocus = false,
    title,
    onopened,
    headerActions,
    footerActions,
  }: {
    path: string;
    name: string;
    extension: string;
    isDirectory?: boolean;
    autofocus?: boolean;
    title?: string;
    onopened?: () => void;
    headerActions?: Snippet;
    footerActions?: Snippet;
  } = $props();

  let editorContainer = $state<HTMLDivElement | undefined>();
  let markdownContainer = $state<HTMLElement | undefined>();
  let loading = $state(true);
  let editorLoading = $state(false);
  let error = $state('');
  let editorError = $state('');
  let openError = $state('');
  let opening = $state(false);
  let formatError = $state('');
  let sourceContent = $state<string>();
  let previewHtml = $state('');
  let imagePreview = $state('');
  let pdfControls = $state<Snippet>();
  let isMarkdown = $derived(isMarkdownFile(extension));
  let isImage = $derived(isImageFile(extension));
  let isPdf = $derived(extension.toLowerCase().replace(/^\./, '') === 'pdf');
  let isJson = $derived(
    ['json', 'jsonc'].includes(extension.toLowerCase().replace(/^\./, '')),
  );
  let mode = $state<'preview' | 'source'>('preview');
  let searchOpen = $state(false);
  let searchQuery = $state('');
  let searchInput = $state<HTMLInputElement>();
  let searchMatches = $state.raw<HTMLElement[]>([]);
  let searchIndex = $state(-1);
  let disposed = false;
  let editor = $state.raw<
    ReturnType<(typeof import('../monaco'))['createViewerEditor']> | undefined
  >();

  onMount(() => {
    if (!isDirectory) void initialize();
  });

  onDestroy(() => {
    disposed = true;
    editor?.dispose();
  });

  $effect(() => {
    const container = markdownContainer;
    const query = searchOpen ? searchQuery : '';
    // Reapply after switching back from source or finishing the initial render.
    previewHtml;
    if (!container || mode !== 'preview') return;
    const matches = highlightPreviewMatches(container, query);
    searchMatches = matches;
    searchIndex = matches.length ? 0 : -1;
  });

  $effect(() => {
    searchMatches.forEach((mark, index) => {
      mark.classList.toggle('search-current', index === searchIndex);
    });
    searchMatches[searchIndex]?.scrollIntoView?.({ block: 'center' });
  });

  async function openSearch() {
    if (mode === 'source') {
      editor?.focus();
      await editor?.getAction('actions.find')?.run();
      return;
    }
    searchOpen = true;
    await tick();
    searchInput?.focus();
    searchInput?.select();
  }

  function closeSearch() {
    searchOpen = false;
    markdownContainer?.focus({ preventScroll: true });
  }

  function moveSearch(direction: number) {
    if (searchMatches.length)
      searchIndex =
        (searchIndex + direction + searchMatches.length) % searchMatches.length;
  }

  function searchKeydown(event: KeyboardEvent) {
    if (
      event.defaultPrevented ||
      event.isComposing ||
      isImage ||
      isPdf ||
      loading ||
      editorLoading ||
      error ||
      editorError
    )
      return;
    if (
      (event.ctrlKey || event.metaKey) &&
      !event.altKey &&
      event.key.toLowerCase() === 'f'
    ) {
      event.preventDefault();
      event.stopPropagation();
      void openSearch();
    } else if (mode === 'preview' && searchOpen && event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      closeSearch();
    } else if (mode === 'preview' && searchOpen && event.key === 'Enter') {
      event.preventDefault();
      event.stopPropagation();
      moveSearch(event.shiftKey ? -1 : 1);
    }
  }

  async function initialize() {
    try {
      if (isPdf) return;
      if (isImage) {
        const dataUrl = await api.readImagePreview(path);
        if (!disposed) imagePreview = dataUrl;
        return;
      }
      sourceContent = await api.readTextPreview(path);
      if (disposed) return;
      if (isMarkdown) {
        const { renderMarkdownPreview } =
          await import('../utils/markdownPreview');
        if (disposed) return;
        const html = await renderMarkdownPreview(
          sourceContent,
          (source) => api.readMarkdownImage(path, source),
          () => disposed,
        );
        if (!disposed) previewHtml = html;
      } else {
        mode = 'source';
        await showSource();
      }
    } catch (cause) {
      if (!disposed) error = errorMessage(cause);
    } finally {
      if (!disposed) loading = false;
      if (autofocus && isMarkdown && mode === 'preview') {
        await tick();
        if (!disposed) markdownContainer?.focus({ preventScroll: true });
      }
    }
  }

  async function showSource() {
    if (!editorContainer || sourceContent === undefined || disposed) return;
    editorLoading = true;
    editorError = '';
    try {
      const { createViewerEditor } = await import('../monaco');
      if (disposed || mode !== 'source' || !editorContainer) return;
      editor = createViewerEditor(editorContainer, sourceContent, extension);
      if (autofocus) editor.focus();
    } catch (cause) {
      if (!disposed) editorError = errorMessage(cause);
    } finally {
      if (!disposed) editorLoading = false;
    }
  }

  async function toggleMarkdownMode() {
    if (!isMarkdown || loading || editorLoading) return;
    openError = '';
    if (mode === 'preview') {
      mode = 'source';
      await tick();
      await showSource();
    } else {
      editor?.dispose();
      editor = undefined;
      editorError = '';
      mode = 'preview';
      await tick();
      if (!disposed) markdownContainer?.focus({ preventScroll: true });
    }
  }

  function autoFormat() {
    if (!editor || loading || editorLoading || sourceContent === undefined)
      return;
    formatError = '';
    try {
      const formatted = formatJsonPreview(
        sourceContent,
        extension.toLowerCase().replace(/^\./, '') === 'jsonc',
      );
      editor.setValue(formatted);
      sourceContent = formatted;
    } catch (cause) {
      formatError = errorMessage(cause);
    }
  }

  async function openInDefaultApp() {
    if (disposed || opening) return;
    opening = true;
    openError = '';
    try {
      await api.open(path);
      if (!disposed) onopened?.();
    } catch (cause) {
      if (!disposed) openError = errorMessage(cause);
    } finally {
      if (!disposed) opening = false;
    }
  }
</script>

{#if isDirectory}
  <DirectorySummary {path} {name} />
{:else}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions (delegates search shortcuts from focused preview controls) -->
  <div
    class="file-preview-content"
    onkeydown={searchKeydown}
    role="region"
    aria-label={`File preview of ${name}`}
  >
    <div class="viewer-header" class:pdf-header={isPdf}>
      <div class="viewer-heading">
        <h2>
          {title ??
            ((isMarkdown && mode === 'preview') || isImage || isPdf
              ? 'Preview'
              : 'View')}
        </h2>
        <span title={path}>{name}</span>
      </div>
      <div class="viewer-toolbar">
        {@render pdfControls?.()}
        {#if isJson && !error && !editorError}
          <button
            type="button"
            class="viewer-toggle"
            title="Format preview only; the file is not changed"
            disabled={loading || editorLoading || !editor}
            onclick={autoFormat}>Auto-format</button
          >
        {/if}
        {#if isMarkdown && !error}<button
            type="button"
            class="viewer-toggle"
            aria-pressed={mode === 'source'}
            disabled={loading || editorLoading}
            onclick={toggleMarkdownMode}
            >{mode === 'preview' ? 'Source' : 'Preview'}</button
          >{/if}
        {#if !isImage && !isPdf && !error && !editorError}
          <button
            type="button"
            class="viewer-toggle"
            disabled={loading ||
              editorLoading ||
              (mode === 'source' && !editor)}
            aria-expanded={mode === 'preview' ? searchOpen : undefined}
            title="Search file (Ctrl/Cmd+F)"
            onclick={openSearch}>Search</button
          >
        {/if}
        {@render headerActions?.()}
      </div>
    </div>
    {#if searchOpen && isMarkdown && mode === 'preview' && !error}
      <div
        class="preview-search"
        role="search"
        aria-label="Search Markdown preview"
      >
        <input
          type="search"
          aria-label="Search preview"
          placeholder="Search preview…"
          bind:this={searchInput}
          bind:value={searchQuery}
        />
        <span role="status"
          >{searchQuery
            ? `${searchIndex + 1} of ${searchMatches.length}`
            : 'Type to search'}</span
        >
        <button
          type="button"
          aria-label="Previous match"
          title="Previous match (Shift+Enter)"
          disabled={!searchMatches.length}
          onclick={() => moveSearch(-1)}
        >
          <Icon icon={icons['chevron-up']} width="16" height="16" />
        </button>
        <button
          type="button"
          aria-label="Next match"
          title="Next match (Enter)"
          disabled={!searchMatches.length}
          onclick={() => moveSearch(1)}
        >
          <Icon icon={icons['chevron-down']} width="16" height="16" />
        </button>
        <button
          type="button"
          aria-label="Close search"
          title="Close search (Escape)"
          onclick={closeSearch}
        >
          <Icon icon={icons.x} width="16" height="16" />
        </button>
      </div>
    {/if}
    {#if formatError}<div role="alert">{formatError}</div>{/if}
    {#if openError && !error && !editorError}<div role="alert">
        {openError}
      </div>{/if}
    <div class="viewer-editor-shell">
      {#if error || editorError}
        <div class="viewer-message" role="alert">
          <p>{error || editorError}</p>
          {#if openError}<p>{openError}</p>{/if}
        </div>
      {:else if isPdf}
        <PdfPreview
          {path}
          {name}
          {autofocus}
          onerror={(message) => (error = message)}
          oncontrols={(controls) => (pdfControls = controls)}
        />
      {:else if isMarkdown && mode === 'preview'}
        <!-- svelte-ignore a11y_no_noninteractive_tabindex (scrollable preview needs keyboard focus) -->
        <article
          class="markdown-preview"
          bind:this={markdownContainer}
          tabindex="0"
          role="region"
          aria-label={`Markdown preview of ${name}`}
        >
          {@html previewHtml}
        </article>
      {:else if isImage}
        <div class="image-preview">
          {#if imagePreview}<img
              src={imagePreview}
              alt={name}
              draggable="false"
              onerror={() => (error = 'This image could not be displayed.')}
            />{/if}
        </div>
      {:else}
        <div class="viewer-editor" bind:this={editorContainer}></div>
      {/if}
      {#if loading || editorLoading}<div class="viewer-loading" role="status">
          {loading ? 'Loading preview…' : 'Loading Monaco…'}
        </div>{/if}
    </div>
    {#if footerActions || error || editorError}<div
        class="dialog-actions viewer-actions"
      >
        {#if error || editorError}<button
            type="button"
            disabled={opening}
            onclick={openInDefaultApp}>Open in default app</button
          >{/if}
        {@render footerActions?.()}
      </div>{/if}
  </div>
{/if}
