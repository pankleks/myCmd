<script lang="ts">
  import { onMount, tick } from 'svelte';
  import UpdateStatus from './UpdateStatus.svelte';
  import {
    MAX_FILE_FONT_SIZE,
    MAX_FILE_ROW_SPACING,
    MIN_FILE_FONT_SIZE,
    MIN_FILE_ROW_SPACING,
  } from '../utils/config';

  let {
    fileFontSize,
    fileRowSpacing,
    showHidden,
    showFunctionBar,
    editor,
    onsubmit,
    onclose,
  }: {
    fileFontSize: number;
    fileRowSpacing: number;
    showHidden: boolean;
    showFunctionBar: boolean;
    editor: string | null;
    onsubmit: (settings: {
      fileFontSize: number;
      fileRowSpacing: number;
      showHidden: boolean;
      showFunctionBar: boolean;
      editor: string | null;
    }) => void;
    onclose: () => void;
  } = $props();

  let fontSize = $state(MIN_FILE_FONT_SIZE);
  let rowSpacing = $state(MIN_FILE_ROW_SPACING);
  let hidden = $state(false);
  let functionBar = $state(true);
  let editorPath = $state('');
  let dialog: HTMLDialogElement;

  onMount(() => {
    fontSize = fileFontSize;
    rowSpacing = fileRowSpacing;
    hidden = showHidden;
    functionBar = showFunctionBar;
    editorPath = editor ?? '';
    dialog.showModal();
    void tick().then(() => {
      const input = dialog.querySelector('input');
      input?.focus();
      input?.select();
    });
  });
</script>

<dialog
  class="settings-dialog sectioned"
  aria-labelledby="settings-title"
  bind:this={dialog}
  oncancel={(event) => {
    event.preventDefault();
    onclose();
  }}
>
  <form
    onsubmit={(event) => {
      event.preventDefault();
      const size = Math.round(Number(fontSize));
      const spacing = Math.round(Number(rowSpacing));
      onsubmit({
        fileFontSize: Number.isFinite(size)
          ? Math.min(MAX_FILE_FONT_SIZE, Math.max(MIN_FILE_FONT_SIZE, size))
          : fileFontSize,
        fileRowSpacing: Number.isFinite(spacing)
          ? Math.min(
              MAX_FILE_ROW_SPACING,
              Math.max(MIN_FILE_ROW_SPACING, spacing),
            )
          : fileRowSpacing,
        showHidden: hidden,
        showFunctionBar: functionBar,
        editor: editorPath.trim() || null,
      });
    }}
  >
    <header><h2 id="settings-title">Settings</h2></header>
    <section aria-labelledby="settings-appearance-title">
      <h3 id="settings-appearance-title">Appearance</h3>
      <div class="settings-row">
        <label for="settings-font-size"
          >File font size <span class="muted">px</span></label
        ><input
          id="settings-font-size"
          type="number"
          bind:value={fontSize}
          required
          min={MIN_FILE_FONT_SIZE}
          max={MAX_FILE_FONT_SIZE}
          autocomplete="off"
          spellcheck="false"
        />
      </div>
      <div class="settings-row">
        <label for="settings-row-spacing"
          >File row spacing <span class="muted">px</span></label
        ><input
          id="settings-row-spacing"
          type="number"
          bind:value={rowSpacing}
          required
          min={MIN_FILE_ROW_SPACING}
          max={MAX_FILE_ROW_SPACING}
          autocomplete="off"
          spellcheck="false"
        />
      </div>
      <div class="settings-row">
        <label for="settings-show-hidden">Show hidden files</label><input
          id="settings-show-hidden"
          type="checkbox"
          bind:checked={hidden}
        />
      </div>
      <div class="settings-row">
        <label for="settings-function-bar">Show function buttons</label><input
          id="settings-function-bar"
          type="checkbox"
          bind:checked={functionBar}
        />
      </div>
    </section>
    <section aria-labelledby="settings-editor-title">
      <h3 id="settings-editor-title">Editor</h3>
      <label for="settings-editor" class="editor-label"
        >Editor executable <span class="muted">Optional</span></label
      >
      <input
        id="settings-editor"
        type="text"
        bind:value={editorPath}
        placeholder="System default application"
        spellcheck="false"
        aria-describedby="settings-editor-help"
      />
      <p id="settings-editor-help" class="hint">
        Leave blank to use the system file association.<br />Otherwise enter an
        executable name or full path, without arguments.
      </p>
    </section>
    <section aria-labelledby="settings-updates-title">
      <h3 id="settings-updates-title">Updates</h3>
      <UpdateStatus />
    </section>
    <footer>
      <div class="dialog-actions">
        <button type="button" onclick={onclose}>Cancel</button><button
          class="primary"
          type="submit">Save</button
        >
      </div>
    </footer>
  </form>
</dialog>

<style>
  .settings-dialog {
    width: 520px;
  }
  .settings-row {
    gap: 24px;
    margin-top: 0;
  }
  .settings-row + .settings-row {
    margin-top: 16px;
  }
  .settings-dialog label {
    display: block;
  }
  .muted {
    color: var(--text-muted);
    font-size: 12px;
    margin-left: 5px;
  }
  .editor-label {
    margin-bottom: 8px;
  }
  input[type='text'] {
    font-family: var(--font-mono);
  }
</style>
