import { describe, expect, it, vi } from 'vitest';
import {
  archiveProvider,
  locationKey,
  parseLocation,
  providerFor,
} from './providers';
import { invoke } from '@tauri-apps/api/core';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

describe('panel providers', () => {
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
