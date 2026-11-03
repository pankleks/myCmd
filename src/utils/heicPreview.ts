interface Pixels {
  width: number;
  height: number;
  data: Uint8ClampedArray;
}

export async function heicPreview(
  dataUrl: string,
  signal: AbortSignal,
): Promise<string> {
  if (signal.aborted) throw new DOMException('Preview closed', 'AbortError');
  const prefix = /^data:image\/hei[cf];base64,/i.exec(dataUrl);
  if (
    !prefix ||
    dataUrl.length > Math.ceil((4 * 1024 * 1024) / 3) * 4 + prefix[0].length
  )
    throw new Error('Invalid or oversized HEIC preview.');
  const buffer = Uint8Array.from(
    atob(dataUrl.slice(prefix[0].length)),
    (byte) => byte.charCodeAt(0),
  );
  const pixels = await new Promise<Pixels>((resolve, reject) => {
    const worker = new Worker(
      new URL('../workers/heic.worker.ts', import.meta.url),
      { type: 'module' },
    );
    function finish(error?: Error, pixels?: Pixels) {
      clearTimeout(timer);
      signal.removeEventListener('abort', abort);
      worker.terminate();
      if (error) reject(error);
      else resolve(pixels!);
    }
    function abort() {
      finish(new DOMException('Preview closed', 'AbortError'));
    }
    const timer = setTimeout(
      () =>
        finish(
          new Error(
            'HEIC decoding timed out; open this image in the default app.',
          ),
        ),
      30_000,
    );
    signal.addEventListener('abort', abort, { once: true });
    worker.onmessage = (event: MessageEvent<Pixels & { error?: string }>) => {
      if (event.data.error) finish(new Error(event.data.error));
      else finish(undefined, event.data);
    };
    worker.onerror = () =>
      finish(new Error('HEIC image could not be decoded.'));
    worker.postMessage(buffer.buffer, [buffer.buffer]);
  });
  if (signal.aborted) throw new DOMException('Preview closed', 'AbortError');
  const canvas = document.createElement('canvas');
  canvas.width = pixels.width;
  canvas.height = pixels.height;
  const context = canvas.getContext('2d');
  if (!context) throw new Error('Image preview is unavailable.');
  const image = context.createImageData(pixels.width, pixels.height);
  image.data.set(pixels.data);
  context.putImageData(image, 0, 0);
  const blob = await new Promise<Blob>((resolve, reject) =>
    canvas.toBlob((blob) => {
      if (blob) resolve(blob);
      else reject(new Error('HEIC preview could not be rendered.'));
    }, 'image/png'),
  );
  if (signal.aborted) throw new DOMException('Preview closed', 'AbortError');
  return URL.createObjectURL(blob);
}
