import { describe, expect, it } from 'vitest';
import { locationKey } from '../filesystem/providers';
import {
  SFTP_UNSUPPORTED,
  sftpActionError,
  toSftpOperation,
} from './sftpRules';

const remote = (path: string) =>
  locationKey({ kind: 'sftp', connectionId: 'c1', path });

describe('sftpActionError', () => {
  it('allows upload, download and remote mutations', () => {
    expect(sftpActionError('copy', 'sftp', 'local')).toBeUndefined();
    expect(sftpActionError('copy', 'local', 'sftp')).toBeUndefined();
    expect(sftpActionError('copy', 'search', 'sftp')).toBeUndefined();
    for (const action of ['rename', 'delete', 'createDirectory'])
      expect(sftpActionError(action, 'sftp', 'local')).toBeUndefined();
  });
  it('rejects move, ZIP, remote-to-remote and archive sources', () => {
    expect(sftpActionError('move', 'sftp', 'local')).toBe(SFTP_UNSUPPORTED);
    expect(sftpActionError('move', 'local', 'sftp')).toBe(SFTP_UNSUPPORTED);
    expect(sftpActionError('compress', 'sftp', 'local')).toBe(SFTP_UNSUPPORTED);
    expect(sftpActionError('copy', 'sftp', 'sftp')).toBe(SFTP_UNSUPPORTED);
    expect(sftpActionError('copy', 'archive', 'sftp')).toBe(SFTP_UNSUPPORTED);
  });
  it('ignores actions that do not involve SFTP', () => {
    expect(sftpActionError('move', 'local', 'local')).toBeUndefined();
    expect(sftpActionError('delete', 'local', 'sftp')).toBeUndefined();
  });
});

describe('toSftpOperation', () => {
  it('maps uploads and downloads to the remote connection', () => {
    expect(
      toSftpOperation(
        { type: 'copy', sources: ['/home/a.txt'], destination: '/data' },
        '/home',
        remote('/data'),
        ['/home/a.txt'],
      ),
    ).toEqual({
      type: 'sftp',
      connectionId: 'c1',
      operation: {
        type: 'copy',
        sources: ['/home/a.txt'],
        destination: '/data',
        upload: true,
      },
    });
    expect(
      toSftpOperation(
        {
          type: 'copy',
          sources: [remote('/data/a.txt')],
          destination: '/home',
        },
        remote('/data'),
        '/home',
        [remote('/data/a.txt')],
      ),
    ).toMatchObject({
      connectionId: 'c1',
      operation: { sources: ['/data/a.txt'], upload: false },
    });
  });
  it('maps remote rename, new folder and delete to remote paths', () => {
    const parent = remote('/data');
    const entries = [remote('/data/a b.txt')];
    expect(
      toSftpOperation(
        { type: 'rename', path: entries[0], name: 'c.txt' },
        parent,
        '/home',
        entries,
      ),
    ).toMatchObject({
      operation: { type: 'rename', path: '/data/a b.txt', name: 'c.txt' },
    });
    expect(
      toSftpOperation(
        { type: 'createDirectory', parent, name: 'new' },
        parent,
        '/home',
        [],
      ),
    ).toMatchObject({
      operation: { type: 'createDirectory', parent: '/data', name: 'new' },
    });
    expect(
      toSftpOperation(
        { type: 'delete', sources: entries, permanent: true },
        parent,
        '/home',
        entries,
      ),
    ).toMatchObject({
      operation: { type: 'delete', sources: ['/data/a b.txt'] },
    });
  });
  it('leaves local operations unchanged', () => {
    const operation = {
      type: 'move' as const,
      sources: ['/a/x'],
      destination: '/b',
    };
    expect(toSftpOperation(operation, '/a', '/b', ['/a/x'])).toBe(operation);
  });
});
