import { describe, expect, it, vi } from 'vitest';
import {
  archiveProvider,
  locationKey,
  parseLocation,
  providerFor,
  searchProvider,
  searchSessions,
  locationLabel,
  sftpProvider,
  isLocalPath,
} from './providers';
import { invoke } from '@tauri-apps/api/core';
import { api } from './api';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => vi.fn()),
}));

describe('panel providers', () => {
  it('routes all preview formats through the selected SFTP connection', async () => {
    const path = locationKey({
      kind: 'sftp',
      connectionId: 'connection',
      path: '/a # 日本語.svg',
    });
    for (const [read, command] of [
      [api.readTextPreview, 'read_text_preview'],
      [api.readImagePreview, 'read_image_preview'],
      [api.readPdfPreview, 'read_pdf_preview'],
    ] as const) {
      await read(path);
      expect(invoke).toHaveBeenLastCalledWith(command, {
        path: '/a # 日本語.svg',
        connectionId: 'connection',
      });
    }
    await api.readMarkdownImage(path, 'image.png');
    expect(invoke).toHaveBeenLastCalledWith('read_markdown_image', {
      markdownPath: '/a # 日本語.svg',
      connectionId: 'connection',
      source: 'image.png',
    });
    await api.readTextPreview('/local.txt');
    expect(invoke).toHaveBeenLastCalledWith('read_text_preview', {
      path: '/local.txt',
    });
  });
  it('round-trips remote paths without interpreting them as local files', () => {
    const location = {
      kind: 'sftp' as const,
      connectionId: 'test',
      path: '/a !/# % 日本語',
    };
    expect(parseLocation(locationKey(location))).toEqual(location);
    expect(isLocalPath(locationKey(location))).toBe(false);
    expect(providerFor(location)).toBe(sftpProvider);
    expect(locationLabel(locationKey(location))).toContain(location.path);
    expect(() => parseLocation('sftp:broken')).toThrow();
  });

  it('maps SFTP listing children and parent into connection-specific resources', async () => {
    const location = {
      kind: 'sftp' as const,
      connectionId: 'test',
      path: '/home/user/docs',
    };
    vi.mocked(invoke).mockResolvedValue({
      path: '/home/user/docs',
      parent: '/home/user',
      entries: [
        { name: 'file.txt', path: '/home/user/docs/file.txt', type: 'file' },
      ],
    });
    const listing = await sftpProvider.list(location);
    expect(invoke).toHaveBeenLastCalledWith('list_sftp', {
      id: 'test',
      path: '/home/user/docs',
    });
    expect(parseLocation(listing.path)).toEqual(location);
    expect(parseLocation(listing.parent!)).toEqual({
      ...location,
      path: '/home/user',
    });
    expect(parseLocation(listing.entries[0].path)).toEqual({
      ...location,
      path: '/home/user/docs/file.txt',
    });
  });
  it('passes the content filter to the backend on every search refresh', async () => {
    searchSessions.set('content-test', {
      root: '/files',
      pattern: '*.json',
      text: 'some text',
      returnPath: '/previous',
    });
    vi.mocked(invoke).mockResolvedValue({
      root: '/files',
      entries: [],
      skipped: 0,
      limited: false,
      cancelled: false,
    });
    try {
      await searchProvider.list({ kind: 'search', sessionId: 'content-test' });
      await searchProvider.list({ kind: 'search', sessionId: 'content-test' });
      expect(invoke).toHaveBeenLastCalledWith('search_files', {
        root: '/files',
        pattern: '*.json',
        text: 'some text',
        requestId: expect.any(String),
      });
      expect(locationLabel('search:content-test')).toContain(
        'containing "some text"',
      );
    } finally {
      searchSessions.delete('content-test');
    }
  });
  it('roundtrips archive paths without delimiter ambiguity', () => {
    const location = {
      kind: 'archive' as const,
      archivePath: 'C:\\data\\test! #.zip',
      directory: 'nested/a b/',
    };
    expect(parseLocation(locationKey(location))).toEqual(location);
  });
  it('reserves remote and search providers without implementing them', () => {
    expect(() =>
      providerFor({ kind: 'smb', connectionId: 'share', path: '/' }),
    ).toThrow('not implemented');
    expect(() =>
      providerFor({ kind: 'nfs', connectionId: 'share', path: '/' }),
    ).toThrow('not implemented');
  });
  it('maps archive children and parent navigation into resources', async () => {
    vi.mocked(invoke).mockResolvedValue({
      entries: [{ name: 'docs', type: 'directory' }],
      parent: 'C:\\data',
    });
    const location = {
      kind: 'archive' as const,
      archivePath: 'C:\\data\\test.zip',
      directory: 'nested/',
    };
    const result = await archiveProvider.list(location);
    expect(parseLocation(result.entries[0].path)).toEqual({
      ...location,
      directory: 'nested/docs/',
    });
    expect(parseLocation(result.parent!)).toEqual({
      ...location,
      directory: '',
    });
  });
});
