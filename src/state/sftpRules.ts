import type { PanelLocation } from '../filesystem/providers';
import { parseLocation } from '../filesystem/providers';
import type { FileOperation } from '../operations/types';

export const SFTP_UNSUPPORTED =
  'SFTP supports upload/download with F5, rename, new folder, and permanent delete. Remote Move, ZIP, and remote-to-remote transfers are not supported.';

/** Returns an error message when an action touching SFTP is not supported, otherwise undefined. */
export function sftpActionError(
  action: string,
  source: PanelLocation['kind'],
  target: PanelLocation['kind'],
): string | undefined {
  const involved =
    source === 'sftp' ||
    ((action === 'copy' || action === 'move') && target === 'sftp');
  if (!involved) return undefined;
  const copyAllowed =
    action === 'copy' &&
    ((source === 'sftp' && target === 'local') ||
      ((source === 'local' || source === 'search') && target === 'sftp'));
  const mutationAllowed =
    source === 'sftp' &&
    ['rename', 'delete', 'createDirectory'].includes(action);
  return copyAllowed || mutationAllowed ? undefined : SFTP_UNSUPPORTED;
}

function remotePath(path: string) {
  const resource = parseLocation(path);
  return resource.kind === 'sftp' ? resource.path : path;
}

/** Maps a dialog result to an SFTP operation when either side is remote; otherwise returns it unchanged. */
export function toSftpOperation(
  operation: FileOperation,
  parent: string,
  destination: string,
  entryPaths: string[],
): FileOperation {
  const location = parseLocation(parent);
  const target = parseLocation(destination);
  if (
    operation.type === 'copy' &&
    (location.kind === 'sftp' || target.kind === 'sftp')
  ) {
    const upload = target.kind === 'sftp';
    return {
      type: 'sftp',
      connectionId: upload
        ? target.connectionId
        : location.kind === 'sftp'
          ? location.connectionId
          : '',
      operation: {
        type: 'copy',
        sources: operation.sources.map(remotePath),
        destination: operation.destination,
        upload,
      },
    };
  }
  if (location.kind !== 'sftp') return operation;
  const connectionId = location.connectionId;
  const paths = entryPaths.map(remotePath);
  if (operation.type === 'rename')
    return {
      type: 'sftp',
      connectionId,
      operation: { type: 'rename', path: paths[0], name: operation.name },
    };
  if (operation.type === 'createDirectory')
    return {
      type: 'sftp',
      connectionId,
      operation: {
        type: 'createDirectory',
        parent: location.path,
        name: operation.name,
      },
    };
  if (operation.type === 'delete')
    return {
      type: 'sftp',
      connectionId,
      operation: { type: 'delete', sources: paths },
    };
  return operation;
}
