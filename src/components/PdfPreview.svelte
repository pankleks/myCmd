<script lang="ts">
  import Icon from '@iconify/svelte';
  import { icons } from '../utils/uiIcons';
  import { onMount, tick, untrack, type Snippet } from 'svelte';
  import type {
    PDFDocumentLoadingTask,
    PDFDocumentProxy,
    RenderTask,
  } from 'pdfjs-dist';
  import { api, errorMessage } from '../filesystem/api';

  let {
    path,
    name,
    autofocus = false,
    onerror,
    oncontrols,
  }: {
    path: string;
    name: string;
    autofocus?: boolean;
    onerror: (message: string) => void;
    oncontrols?: (controls: Snippet | undefined) => void;
  } = $props();
  let scroller: HTMLDivElement;
  let pdf = $state.raw<PDFDocumentProxy>();
  let pageCount = $state(0);
  let currentPage = $state(1);
  let width = $state(0);
  let pageWidth = $state(612);
  let pageHeight = $state(792);
  let zoom = $state<number | null>(1);
  let scale = $derived(zoom ?? Math.max(0.1, (width - 32) / pageWidth));
  let loading = $state(true);
  let disposed = false;
  let loader: PDFDocumentLoadingTask | undefined;
  let renderTask: RenderTask | undefined;
  let observer: IntersectionObserver;
  let generation = 0;
  let pumping = false;
  const visible = new Set<HTMLCanvasElement>();
  const canvases = new Set<HTMLCanvasElement>();
  let rendered = new WeakSet<HTMLCanvasElement>();

  function report(cause: unknown) {
    if (disposed) return;
    onerror(
      cause instanceof Error && cause.name === 'PasswordException'
        ? 'This PDF is password-protected. Open it in your default PDF app.'
        : `Cannot preview PDF: ${errorMessage(cause)}`,
    );
  }

  async function initialize() {
    try {
      const [data, library, worker] = await Promise.all([
        api.readPdfPreview(path),
        import('pdfjs-dist/legacy/build/pdf.mjs'),
        import('pdfjs-dist/legacy/build/pdf.worker.min.mjs?url'),
      ]);
      if (disposed) return;
      library.GlobalWorkerOptions.workerSrc = worker.default;
      // Canvas rendering only: no annotation actions, links, forms or PDF scripting.
      loader = library.getDocument({
        data: new Uint8Array(data),
        cMapUrl: '/pdfjs/cmaps/',
        cMapPacked: true,
        standardFontDataUrl: '/pdfjs/standard_fonts/',
        wasmUrl: '/pdfjs/wasm/',
        enableXfa: false,
        maxImageSize: 16_000_000,
        canvasMaxAreaInBytes: 16_000_000,
      });
      const document = await loader.promise;
      if (disposed) return;
      // ponytail: 2,000 page placeholders; virtualize them if larger documents matter.
      if (document.numPages > 2000)
        throw new Error('PDFs with more than 2,000 pages cannot be previewed.');
      const first = await document.getPage(1);
      if (disposed) return;
      const viewport = first.getViewport({ scale: 1 });
      pageWidth = viewport.width;
      pageHeight = viewport.height;
      first.cleanup();
      pdf = document;
      pageCount = document.numPages;
      loading = false;
      await tick();
      if (!disposed && autofocus) scroller.focus({ preventScroll: true });
    } catch (cause) {
      report(cause);
    }
  }

  function observePage(canvas: HTMLCanvasElement) {
    canvases.add(canvas);
    observer.observe(canvas.parentElement!);
    return {
      destroy() {
        observer.unobserve(canvas.parentElement!);
        canvases.delete(canvas);
        visible.delete(canvas);
        canvas.width = canvas.height = 0;
      },
    };
  }

  async function pump() {
    if (pumping || !pdf || disposed) return;
    pumping = true;
    try {
      while (!disposed) {
        const canvas = [...visible].find((canvas) => !rendered.has(canvas));
        if (!canvas) break;
        rendered.add(canvas);
        const version = generation;
        const renderScale = scale;
        const page = await pdf.getPage(Number(canvas.dataset.page));
        try {
          if (disposed || version !== generation || !visible.has(canvas))
            continue;
          const viewport = page.getViewport({ scale: renderScale });
          // Bound both canvas memory and dimensions, even at high zoom/DPI.
          const ratio = Math.min(
            window.devicePixelRatio || 1,
            2,
            Math.sqrt(4_000_000 / (viewport.width * viewport.height)),
            8192 / viewport.width,
            8192 / viewport.height,
          );
          canvas.parentElement!.style.width = `${viewport.width}px`;
          canvas.parentElement!.style.height = `${viewport.height}px`;
          canvas.width = Math.max(1, Math.floor(viewport.width * ratio));
          canvas.height = Math.max(1, Math.floor(viewport.height * ratio));
          renderTask = page.render({
            canvas,
            viewport,
            transform: [ratio, 0, 0, ratio, 0, 0],
            background: '#ffffff',
          });
          await renderTask.promise;
        } catch (cause) {
          if (!(
            cause instanceof Error &&
            cause.name === 'RenderingCancelledException'
          ))
            throw cause;
        } finally {
          renderTask = undefined;
          page.cleanup();
          if (!visible.has(canvas)) canvas.width = canvas.height = 0;
        }
      }
    } catch (cause) {
      report(cause);
    } finally {
      pumping = false;
    }
  }

  $effect(() => {
    if (!pdf) return;
    scale;
    generation++;
    renderTask?.cancel();
    rendered = new WeakSet();
    for (const canvas of canvases) canvas.width = canvas.height = 0;
    const page = untrack(() => currentPage);
    void tick().then(() => {
      if (disposed) return;
      goTo(page);
      void pump();
    });
  });

  function trackPage() {
    const top = scroller.getBoundingClientRect().top;
    let closest = Infinity;
    for (const canvas of visible) {
      const distance = Math.abs(
        canvas.parentElement!.getBoundingClientRect().top - top,
      );
      if (distance < closest) {
        closest = distance;
        currentPage = Number(canvas.dataset.page);
      }
    }
  }

  function goTo(page: number) {
    if (!Number.isInteger(page) || page < 1 || page > pageCount) return;
    currentPage = page;
    scroller
      .querySelector(`[data-page="${page}"]`)
      ?.parentElement?.scrollIntoView({ block: 'start' });
  }

  onMount(() => {
    oncontrols?.(pdfToolbar);
    observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          const canvas = entry.target.querySelector('canvas')!;
          if (entry.isIntersecting) visible.add(canvas);
          else {
            visible.delete(canvas);
            rendered.delete(canvas);
            canvas.width = canvas.height = 0;
          }
        }
        trackPage();
        void pump();
      },
      { root: scroller, rootMargin: '400px 0px' },
    );
    const resize = new ResizeObserver(() => {
      width = scroller.clientWidth;
    });
    resize.observe(scroller);
    width = scroller.clientWidth;
    void initialize();
    return () => {
      disposed = true;
      oncontrols?.(undefined);
      observer.disconnect();
      resize.disconnect();
      renderTask?.cancel();
      void loader?.destroy().catch(() => {});
      for (const canvas of canvases) canvas.width = canvas.height = 0;
    };
  });
</script>

{#snippet pdfToolbar()}
  <div class="pdf-toolbar" role="toolbar" aria-label="PDF controls">
    <button
      disabled={!pdf || currentPage === 1}
      aria-label="Previous page"
      onclick={() => goTo(currentPage - 1)}
      ><Icon icon={icons['chevron-left']} width="18" height="18" /></button
    >
    <input
      aria-label="Page number"
      type="number"
      min="1"
      max={pageCount || 1}
      value={currentPage}
      disabled={!pdf}
      onchange={(event) => goTo(event.currentTarget.valueAsNumber)}
    />
    <span>of {pageCount || '…'}</span>
    <button
      disabled={!pdf || currentPage === pageCount}
      aria-label="Next page"
      onclick={() => goTo(currentPage + 1)}
      ><Icon icon={icons['chevron-right']} width="18" height="18" /></button
    >
    <button
      disabled={!pdf || scale <= 0.25}
      aria-label="Zoom out"
      onclick={() => (zoom = Math.max(0.25, scale / 1.25))}
      ><Icon icon={icons.minus} width="18" height="18" /></button
    >
    <span>{Math.round(scale * 100)}%</span>
    <button
      disabled={!pdf || scale >= 4}
      aria-label="Zoom in"
      onclick={() => (zoom = Math.min(4, scale * 1.25))}
      ><Icon icon={icons.plus} width="18" height="18" /></button
    >
    <button
      disabled={!pdf}
      aria-pressed={zoom === null}
      onclick={() => (zoom = null)}>Fit width</button
    >
  </div>
{/snippet}

<div class="pdf-preview">
  {#if !oncontrols}{@render pdfToolbar()}{/if}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex (scrollable preview needs keyboard focus) -->
  <div
    class="pdf-pages"
    bind:this={scroller}
    role="region"
    aria-label={`PDF preview of ${name}`}
    tabindex="0"
    onscroll={trackPage}
  >
    {#each Array.from({ length: pageCount }, (_, index) => index + 1) as page (page)}
      <div
        class="pdf-page"
        style:width={`${pageWidth * scale}px`}
        style:height={`${pageHeight * scale}px`}
      >
        <canvas
          use:observePage
          data-page={page}
          aria-label={`Page ${page} of ${pageCount}`}
          width="0"
          height="0">Page {page}</canvas
        >
      </div>
    {/each}
  </div>
  {#if loading}<div class="viewer-loading" role="status">Loading PDF…</div>{/if}
</div>

<style>
  .pdf-preview {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
  }
  .pdf-toolbar {
    display: flex;
    flex-wrap: nowrap;
    align-items: center;
    gap: 8px;
    padding: 0;
    font-size: 13px;
  }
  .pdf-toolbar input {
    width: 64px;
    padding: 4px;
  }
  .pdf-toolbar button {
    font-size: 13px;
    min-width: 0;
    padding: 3px 8px;
  }
  .pdf-toolbar :global(svg) {
    display: block;
  }
  .pdf-pages {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 16px;
    outline: none;
  }
  .pdf-page {
    position: relative;
    margin: 0 auto 16px;
    background: #fff;
  }
  canvas {
    display: block;
    width: 100%;
    height: 100%;
  }
</style>
