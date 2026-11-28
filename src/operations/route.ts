import { parseLocation } from '../filesystem/providers';
import { toSftpOperation } from '../state/sftpRules';
import type { FileOperation } from './types';

/** The operation the backend runs for a dialog's choice, given where its items live. */
export function routeOperation(
  operation: FileOperation,
  parent: string,
  destination: string,
  entries: { path: string; name: string }[],
): FileOperation {
  const location = parseLocation(parent);
  operation = toSftpOperation(
    operation,
    parent,
    destination,
    entries.map((entry) => entry.path),
  );
  if (location.kind !== 'archive') return operation;
  const archive = {
    archivePath: location.archivePath,
    directory: location.directory,
    members: entries.map((entry) => entry.name),
  };
  // Archives are rewritten (no Recycle Bin) or extracted, never changed in place.
  if (operation.type === 'delete') return { type: 'deleteArchive', ...archive };
  if (operation.type === 'copy')
    return {
      type: 'extract',
      ...archive,
      destination: operation.destination,
    };
  return operation;
}
