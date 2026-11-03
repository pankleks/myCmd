declare module 'libheif-js/libheif-wasm/libheif-bundle.mjs' {
  interface Pixels {
    width: number;
    height: number;
    data: Uint8ClampedArray;
  }
  interface HeifImage {
    get_width(): number;
    get_height(): number;
    is_primary(): boolean;
    display(pixels: Pixels, callback: (pixels: Pixels | null) => void): void;
    free(): void;
  }
  export default function createLibheif(): {
    HeifDecoder: new () => { decode(buffer: Uint8Array): HeifImage[] };
  };
}
