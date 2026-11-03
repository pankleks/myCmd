/// <reference lib="webworker" />
import { decodeHeic } from '../utils/heicDecode';
declare const self: DedicatedWorkerGlobalScope;

self.onmessage = async (event: MessageEvent<ArrayBuffer>) => {
  try {
    const pixels = await decodeHeic(new Uint8Array(event.data));
    self.postMessage(pixels, [pixels.data.buffer as ArrayBuffer]);
  } catch (cause) {
    self.postMessage({
      error:
        cause instanceof Error
          ? cause.message
          : 'HEIC image could not be decoded.',
    });
  }
};
self.onunhandledrejection = (event) => {
  event.preventDefault();
  self.postMessage({ error: 'HEIC image could not be decoded.' });
};
