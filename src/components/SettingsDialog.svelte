<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { version } from '../../package.json';
  import { MAX_FILE_FONT_SIZE, MIN_FILE_FONT_SIZE } from '../utils/config';

  let {
    fileFontSize,
    showHidden,
    showFunctionBar,
    editor,
    onsubmit,
    onclose,
  }: {
    fileFontSize: number;
    showHidden: boolean;
    showFunctionBar: boolean;
    editor: string | null;
    onsubmit: (settings: {
      fileFontSize: number;
      showHidden: boolean;
      showFunctionBar: boolean;
      editor: string | null;
    }) => void;
    onclose: () => void;
  } = $props();

  let fontSize = $state(MIN_FILE_FONT_SIZE);
  let hidden = $state(false);
  let functionBar = $state(true);
  let editorPath = $state('');
  let dialog: HTMLDialogElement;

  onMount(() => {
    fontSize = fileFontSize;
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
  class="settings-dialog"
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
      onsubmit({
        fileFontSize: Number.isFinite(size)
          ? Math.min(MAX_FILE_FONT_SIZE, Math.max(MIN_FILE_FONT_SIZE, size))
          : fileFontSize,
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
    <footer>
      <span class="version">myCmd · v{version}</span>
      <div class="actions">
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
    max-height: 90vh;
    padding: 0;
    background: #222b38;
    border: 1px solid #3c485c;
    border-radius: 0;
    box-shadow: 0 24px 80px #0008;
    overflow: auto;
  }
  header {
    padding: 24px 28px 20px;
    border-bottom: 1px solid #364153;
  }
  header h2 {
    margin: 0;
    font-size: 22px;
    font-weight: 650;
  }
  section {
    padding: 22px 28px;
  }
  section + section {
    border-top: 1px solid #364153;
  }
  h3 {
    margin: 0 0 18px;
    font-size: 12px;
    font-weight: 650;
    letter-spacing: 1px;
    text-transform: uppercase;
    color: #aab7ca;
  }
  .settings-row {
    gap: 24px;
    margin-top: 0;
  }
  .settings-row + .settings-row {
    margin-top: 18px;
  }
  .settings-dialog label {
    display: block;
  }
  .muted {
    color: #94a3b8;
    font-size: 12px;
    margin-left: 5px;
  }
  .settings-dialog input:not([type='checkbox']) {
    background: #151d29;
    border: 1px solid #46536a;
    border-radius: 0;
    padding: 9px 11px;
  }
  .editor-label {
    margin-bottom: 9px;
  }
  input[type='text'] {
    font-family: ui-monospace, monospace;
    font-size: 13px;
  }
  input::placeholder {
    color: #93a2b8;
    opacity: 1;
  }
  input:focus-visible,
  button:focus-visible {
    outline: 1px solid #8290a3;
    outline-offset: -1px;
  }
  .hint {
    color: #aab7ca;
    font-size: 12px;
    margin: 10px 0 0;
  }
  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    background: #1d2531;
    border-top: 1px solid #364153;
    padding: 16px 28px;
  }
  .version {
    font-size: 12px;
    color: #94a3b8;
  }
  .actions {
    display: flex;
    gap: 10px;
  }
  button {
    height: auto;
    font-size: 13px;
    font-weight: 550;
    border: 1px solid #46536a;
    border-radius: 0;
    padding: 9px 17px;
    background: transparent;
  }
  button:hover {
    background: #303d51;
  }
  button.primary {
    background: #426cb0;
    border-color: #5a82c4;
  }
  button.primary:hover {
    background: #507dc3;
  }
  @media (max-width: 420px) {
    header,
    section {
      padding: 20px;
    }
    footer {
      padding: 16px 20px;
    }
  }
</style>
