<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte';
  import { api, errorMessage } from '../filesystem/api';
  import { isImageFile, isMarkdownFile } from '../utils/viewer';

  let {
    path,
    name,
    extension,
    onclose,
    inline = false,
  }: {
    path: string;
    name: string;
    extension: string;
    onclose: () => void;
    inline?: boolean;
  } = $props();

  let dialog = $state<HTMLDialogElement>();
  let editorContainer = $state<HTMLDivElement | undefined>();
  let loading = $state(true);
  let editorLoading = $state(false);
  let error = $state('');
  let editorError = $state('');
  let openError = $state('');
  let sourceContent = $state<string>();
  let previewHtml = $state('');
  let imagePreview = $state('');
  let isMarkdown = $derived(isMarkdownFile(extension));
  let isImage = $derived(isImageFile(extension));
  let mode = $state<'preview' | 'source'>('preview');
  let disposed = false;
  let editor:
    ReturnType<(typeof import('../monaco'))['createViewerEditor']> | undefined;

  onMount(() => {
    if (!inline) dialog?.showModal();
    void initialize();
  });

  onDestroy(() => {
    disposed = true;
    editor?.dispose();
  });

  async function initialize() {
    try {
      if (isImage) {
        const dataUrl = await api.readImagePreview(path);
        if (!disposed) imagePreview = dataUrl;
        return;
      }
      sourceContent = await api.readTextPreview(path);
      if (disposed) return;
      if (isMarkdown) previewHtml = await renderMarkdown(sourceContent);
      else {
        mode = 'source';
        await showSource();
      }
    } catch (cause) {
      if (!disposed) error = errorMessage(cause);
    } finally {
      if (!disposed) loading = false;
    }
  }

  async function renderMarkdown(content: string): Promise<string> {
    const [{ marked }, { default: DOMPurify }] = await Promise.all([
      import('marked'),
      import('dompurify'),
    ]);
    const html = await marked.parse(content, { gfm: true });
    const options = {
      USE_PROFILES: { html: true },
      FORBID_TAGS: [
        'style',
        'iframe',
        'object',
        'embed',
        'a',
        'source',
        'video',
        'audio',
        'form',
      ],
      FORBID_ATTR: ['style', 'target', 'srcset', 'action', 'formaction'],
    };
    const preview = new DOMParser().parseFromString(
      DOMPurify.sanitize(html, options),
      'text/html',
    );
    const images = Array.from(preview.body.querySelectorAll('img'));
    let totalImageBytes = 0;
    for (const [index, image] of images.entries()) {
      if (disposed) return '';
      const source = image.getAttribute('src');
      if (!source || index >= 24) {
        image.removeAttribute('src');
        continue;
      }
      try {
        const dataUrl = await api.readMarkdownImage(path, source);
        const payloadLength = dataUrl.length - dataUrl.indexOf(',') - 1;
        const imageBytes = Math.floor((payloadLength * 3) / 4);
        if (totalImageBytes + imageBytes > 12 * 1024 * 1024) {
          image.removeAttribute('src');
          continue;
        }
        totalImageBytes += imageBytes;
        image.setAttribute('src', dataUrl);
        image.setAttribute('loading', 'lazy');
        image.setAttribute('decoding', 'async');
      } catch {
        image.removeAttribute('src');
        if (!image.getAttribute('alt'))
          image.setAttribute('alt', 'Image unavailable in preview');
      }
    }
    return DOMPurify.sanitize(preview.body.innerHTML, options);
  }

  async function showSource() {
    if (!editorContainer || sourceContent === undefined || disposed) return;
    editorLoading = true;
    editorError = '';
    try {
      const { createViewerEditor } = await import('../monaco');
      if (disposed || mode !== 'source' || !editorContainer) return;
      editor = createViewerEditor(editorContainer, sourceContent, extension);
      if (!inline) editor.focus();
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
    }
  }

  async function openInDefaultApp() {
    openError = '';
    try {
      await api.open(path);
      onclose();
    } catch (cause) {
      openError = errorMessage(cause);
    }
  }
</script>

{#snippet viewerContents()}
  <div class="file-preview-content" class:preview-inline-content={inline}>
    <div class="viewer-header">
      <div class="viewer-heading">
        <h2>
          {inline || (isMarkdown && mode === 'preview') || isImage
            ? 'Preview'
            : 'View'}
        </h2>
        <span>{name}</span>
      </div>
      <div class="viewer-toolbar">
        {#if isMarkdown && !error}<button
            type="button"
            class="viewer-toggle"
            aria-pressed={mode === 'source'}
            disabled={loading || editorLoading}
            onclick={toggleMarkdownMode}
            >{mode === 'preview' ? 'Source' : 'Preview'}</button
          >{/if}
        {#if !inline}<button
            type="button"
            class="viewer-close"
            onclick={onclose}
            aria-label="Close viewer">×</button
          >{/if}
      </div>
    </div>
    <div class="viewer-path" title={path}>{path}</div>
    <div class="viewer-editor-shell">
      {#if error || editorError}
        <div class="viewer-message" role="alert">
          <p>{error || editorError}</p>
          {#if openError}<p>{openError}</p>{/if}
        </div>
      {:else if isMarkdown && mode === 'preview'}
        <article class="markdown-preview">
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
    {#if !inline || error || editorError}<div
        class="dialog-actions viewer-actions"
      >
        {#if error || editorError}<button
            type="button"
            onclick={openInDefaultApp}>Open in default app</button
          >{/if}
        {#if !inline}<button type="button" class="primary" onclick={onclose}
            >Close (Esc)</button
          >{/if}
      </div>{/if}
  </div>
{/snippet}

{#if inline}
  <section class="panel preview-panel" aria-label={`Preview ${name}`}>
    {@render viewerContents()}
  </section>
{:else}
  <dialog
    class="file-viewer"
    bind:this={dialog}
    aria-label={`View ${name}`}
    oncancel={(event) => {
      event.preventDefault();
      onclose();
    }}
    onkeydown={(event) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        onclose();
      }
    }}
  >
    {@render viewerContents()}
  </dialog>
{/if}
