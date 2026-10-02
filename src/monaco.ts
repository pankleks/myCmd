import * as monaco from 'monaco-editor/editor/editor.api.js';
import EditorWorker from 'monaco-editor/editor/editor.worker.js?worker';
import JsonWorker from 'monaco-editor/languages/features/json/json.worker.js?worker';
import 'monaco-editor/languages/definitions/javascript/register.js';
import 'monaco-editor/languages/definitions/markdown/register.js';
import 'monaco-editor/languages/definitions/csharp/register.js';
import 'monaco-editor/languages/definitions/rust/register.js';
import 'monaco-editor/languages/definitions/cpp/register.js';
import 'monaco-editor/languages/definitions/typescript/register.js';
import 'monaco-editor/languages/definitions/css/register.js';
import 'monaco-editor/languages/definitions/xml/register.js';
import 'monaco-editor/languages/features/json/register.js';
import 'monaco-editor/editor/contrib/folding/browser/folding.js';
import '../node_modules/monaco-editor/esm/vs/base/browser/ui/codicons/codicon/codicon.css';
import '../node_modules/monaco-editor/min/vs/editor/editor.main.css';
import { viewerLanguage } from './utils/viewer';
import { svelteLanguage } from './utils/svelteLanguage';

monaco.languages.register({ id: 'svelte', extensions: ['.svelte'] });
monaco.languages.setMonarchTokensProvider('svelte', svelteLanguage);

self.MonacoEnvironment = {
  getWorker(_workerId, label) {
    if (label === 'json') return new JsonWorker();
    return new EditorWorker();
  },
};

monaco.editor.defineTheme('mycmd-dark', {
  base: 'vs-dark',
  inherit: true,
  rules: [],
  colors: {
    'editor.background': '#171b23',
    'editor.foreground': '#d6dce7',
    'editorGutter.background': '#171b23',
    'editorLineNumber.foreground': '#7c8798',
    'editorLineNumber.activeForeground': '#d6dce7',
    'editor.lineHighlightBackground': '#202834',
    'editor.selectionBackground': '#264b73',
  },
});

export function createViewerEditor(
  element: HTMLElement,
  value: string,
  extension: string,
) {
  return monaco.editor.create(element, {
    value,
    language: viewerLanguage(extension),
    theme: 'mycmd-dark',
    readOnly: true,
    domReadOnly: true,
    ariaLabel: 'Read-only file viewer',
    automaticLayout: true,
    minimap: { enabled: false },
    lineNumbers: 'on',
    folding: true,
    showFoldingControls: 'always',
    scrollBeyondLastLine: false,
    wordWrap: 'off',
    renderLineHighlight: 'line',
    fontSize: 14,
    padding: { top: 10, bottom: 10 },
    largeFileOptimizations: true,
  });
}
