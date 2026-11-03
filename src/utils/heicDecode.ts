import createLibheif from 'libheif-js/libheif-wasm/libheif-bundle.mjs';

export function validateHeicDimensions(width: number, height: number) {
  if (
    !Number.isSafeInteger(width) ||
    !Number.isSafeInteger(height) ||
    width <= 0 ||
    height <= 0
  )
    throw new Error('Invalid HEIC image dimensions.');
  if (width > 16_384 || height > 16_384 || width * height > 16_000_000)
    throw new Error(
      'HEIC image exceeds preview limits (16 million pixels or 16384 pixels per side).',
    );
}

export async function decodeHeic(buffer: Uint8Array) {
  if (!buffer.length || buffer.length > 4 * 1024 * 1024)
    throw new Error('HEIC images must be no larger than 4 MiB.');
  const libheif = createLibheif();
  const images = new libheif.HeifDecoder().decode(buffer);
  try {
    const image = images.find((image) => image.is_primary()) ?? images[0];
    if (!image) throw new Error('HEIC image could not be decoded.');
    const width = image.get_width();
    const height = image.get_height();
    validateHeicDimensions(width, height);
    const pixels = {
      width,
      height,
      data: new Uint8ClampedArray(width * height * 4),
    };
    await new Promise<void>((resolve, reject) =>
      image.display(pixels, (result) => {
        if (result) resolve();
        else reject(new Error('HEIC image could not be decoded.'));
      }),
    );
    return pixels;
  } finally {
    for (const image of images) image.free();
  }
}
