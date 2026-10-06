<script lang="ts">
  import { onMount, tick } from 'svelte';
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
    <h2>Settings</h2>
    <div class="settings-row">
      <span id="settings-font-size">File font size</span><input
        type="number"
        aria-labelledby="settings-font-size"
        bind:value={fontSize}
        required
        min={MIN_FILE_FONT_SIZE}
        max={MAX_FILE_FONT_SIZE}
        autocomplete="off"
        spellcheck="false"
      />
    </div>
    <div class="settings-row">
      <span id="settings-show-hidden">Show hidden files</span><input
        type="checkbox"
        aria-labelledby="settings-show-hidden"
        bind:checked={hidden}
      />
    </div>
    <div class="settings-row">
      <span id="settings-function-bar">Show function buttons</span><input
        type="checkbox"
        aria-labelledby="settings-function-bar"
        bind:checked={functionBar}
      />
    </div>
    <div class="settings-row">
      <label
        >Editor executable (optional)<input
          bind:value={editorPath}
          placeholder="System default application"
          spellcheck="false"
        /></label
      >
    </div>
    <p>
      Leave blank to use the system file association. Enter an executable name
      or full path, without arguments.
    </p>
    <div class="dialog-actions">
      <button type="button" onclick={onclose}>Cancel</button><button
        class="primary"
        type="submit">Save</button
      >
    </div>
  </form>
</dialog>
