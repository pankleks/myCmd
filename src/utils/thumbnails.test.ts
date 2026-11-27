import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { THUMBNAIL_SIDE, loadThumbnail, resetThumbnails } from './thumbnails';

const mocks = vi.hoisted(() => ({
  thumbnail: vi.fn(),
  image: vi.fn(),
  text: vi.fn(),
  heic: vi.fn(),
}));
vi.mock('../filesystem/api', () => ({
  api: {
    readImageThumbnail: mocks.thumbnail,
    readImagePreview: mocks.image,
    readTextPreview: mocks.text,
  },
}));
vi.mock('./heicPreview', () => ({ heicPreview: mocks.heic }));

function image(name: string, modified = 1) {
  return {
    name,
    path: `/photos/${name}`,
    extension: name.split('.').pop()!,
    size: 10,
    modified,
  };
}

function deferred() {
  let resolve!: (value: string) => void;
  const promise = new Promise<string>((done) => (resolve = done));
  return { promise, resolve };
}

beforeEach(() => {
  vi.resetAllMocks();
  resetThumbnails();
});
afterEach(() => resetThumbnails());

describe('loadThumbnail', () => {
  it('asks the backend for a small image and caches it until the file changes', async () => {
    mocks.thumbnail.mockResolvedValue('data:image/jpeg;base64,AA==');
    const signal = new AbortController().signal;

    await expect(loadThumbnail(image('a.jpg'), signal)).resolves.toBe(
      'data:image/jpeg;base64,AA==',
    );
    await loadThumbnail(image('a.jpg'), signal);
    expect(mocks.thumbnail).toHaveBeenCalledExactlyOnceWith(
      '/photos/a.jpg',
      THUMBNAIL_SIDE,
    );

    await loadThumbnail(image('a.jpg', 2), signal);
    expect(mocks.thumbnail).toHaveBeenCalledTimes(2);
  });

  it('uses the full preview for formats the backend cannot scale', async () => {
    mocks.image.mockResolvedValue('data:image/heic;base64,AQ==');
    mocks.heic.mockResolvedValue('blob:heic');
    mocks.text.mockResolvedValue('<svg/>');
    const signal = new AbortController().signal;

    await expect(loadThumbnail(image('b.HEIC'), signal)).resolves.toBe(
      'blob:heic',
    );
    await expect(loadThumbnail(image('c.svg'), signal)).resolves.toBe(
      `data:image/svg+xml;charset=utf-8,${encodeURIComponent('<svg/>')}`,
    );
    expect(mocks.thumbnail).not.toHaveBeenCalled();
    // Blob URLs are owned by the caller, so they are never cached.
    await loadThumbnail(image('b.HEIC'), signal);
    expect(mocks.heic).toHaveBeenCalledTimes(2);
  });

  it('runs at most three loads at once and drops aborted waiting loads', async () => {
    const pending = Array.from({ length: 5 }, deferred);
    pending.forEach(({ promise }) =>
      mocks.thumbnail.mockReturnValueOnce(promise),
    );
    const signal = new AbortController().signal;
    const waiting = new AbortController();

    const results = [
      loadThumbnail(image('1.png'), signal),
      loadThumbnail(image('2.png'), signal),
      loadThumbnail(image('3.png'), signal),
      loadThumbnail(image('4.png'), waiting.signal),
      loadThumbnail(image('5.png'), signal),
    ];
    await vi.waitFor(() => expect(mocks.thumbnail).toHaveBeenCalledTimes(3));

    waiting.abort(new Error('scrolled away'));
    await expect(results[3]).rejects.toThrow('scrolled away');
    pending[0].resolve('data:1');
    await expect(results[0]).resolves.toBe('data:1');
    await vi.waitFor(() => expect(mocks.thumbnail).toHaveBeenCalledTimes(4));
    expect(mocks.thumbnail).toHaveBeenLastCalledWith(
      '/photos/5.png',
      THUMBNAIL_SIDE,
    );

    pending[1].resolve('data:2');
    pending[2].resolve('data:3');
    pending[3].resolve('data:5');
    await expect(
      Promise.all([results[1], results[2], results[4]]),
    ).resolves.toEqual(['data:2', 'data:3', 'data:5']);
  });

  it('releases its place when the backend fails', async () => {
    mocks.thumbnail.mockRejectedValue({ message: 'broken' });
    const signal = new AbortController().signal;
    for (let i = 0; i < 4; i++)
      await expect(loadThumbnail(image(`${i}.png`), signal)).rejects.toEqual({
        message: 'broken',
      });
    expect(mocks.thumbnail).toHaveBeenCalledTimes(4);
  });
});
