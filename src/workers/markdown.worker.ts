/// <reference lib="webworker" />
import { marked } from 'marked';
declare const self: DedicatedWorkerGlobalScope;

self.onmessage = async (event: MessageEvent<string>) => {
  try {
    self.postMessage({ html: await marked.parse(event.data, { gfm: true }) });
  } catch {
    self.postMessage({ error: true });
  }
};
self.onunhandledrejection = (event) => {
  event.preventDefault();
  self.postMessage({ error: true });
};
