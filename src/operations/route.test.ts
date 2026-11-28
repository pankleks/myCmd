import { describe, expect, it } from 'vitest';
import { locationKey } from '../filesystem/providers';
import { routeOperation } from './route';

const archive = locationKey({
  kind: 'archive',
  archivePath: '/data/a.zip',
  directory: 'docs/',
});
const entries = [{ path: `${archive}x.txt`, name: 'x.txt' }];

describe('operation routing', () => {
  it('rewrites the archive for a delete, even a non-permanent one', () => {
    expect(
      routeOperation(
        { type: 'delete', sources: [entries[0].path], permanent: false },
        archive,
        '/target',
        entries,
      ),
    ).toEqual({
      type: 'deleteArchive',
      archivePath: '/data/a.zip',
      directory: 'docs/',
      members: ['x.txt'],
    });
  });

  it('extracts archive members on copy', () => {
    expect(
      routeOperation(
        {
          type: 'copy',
          sources: [entries[0].path],
          destination: '/target',
        },
        archive,
        '/target',
        entries,
      ),
    ).toEqual({
      type: 'extract',
      archivePath: '/data/a.zip',
      directory: 'docs/',
      members: ['x.txt'],
      destination: '/target',
    });
  });

  it('keeps local operations, including the permanent flag', () => {
    const operation = {
      type: 'delete' as const,
      sources: ['/data/x.txt'],
      permanent: true,
    };
    expect(
      routeOperation(operation, '/data', '/target', [
        { path: '/data/x.txt', name: 'x.txt' },
      ]),
    ).toEqual(operation);
  });

  it('turns a delete on an SFTP folder into a remote delete', () => {
    const parent = locationKey({
      kind: 'sftp',
      connectionId: 'c1',
      path: '/home/u',
    });
    const path = locationKey({
      kind: 'sftp',
      connectionId: 'c1',
      path: '/home/u/x.txt',
    });
    expect(
      routeOperation(
        { type: 'delete', sources: [path], permanent: true },
        parent,
        '/target',
        [{ path, name: 'x.txt' }],
      ),
    ).toEqual({
      type: 'sftp',
      connectionId: 'c1',
      operation: { type: 'delete', sources: ['/home/u/x.txt'] },
    });
  });
});
