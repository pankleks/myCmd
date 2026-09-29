import { describe, expect, it, vi } from 'vitest';
import { createOperationController } from './controller.svelte';
import type { Conflict, Progress } from './types';

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => (resolve = done));
  return { promise, resolve };
}

function setup() {
  const deps = {
    start: vi.fn().mockResolvedValue('own'),
    cancel: vi.fn().mockResolvedValue(undefined),
    resolve: vi.fn().mockResolvedValue(undefined),
    completed: vi.fn().mockResolvedValue(undefined),
    failed: vi.fn(),
  };
  return { deps, controller: createOperationController(deps) };
}
const operation = {
  type: 'delete' as const,
  sources: ['/file'],
  permanent: false,
};
function progress(
  operationId = 'own',
  state: Progress['state'] = 'running',
): Progress {
  return {
    operationId,
    state,
    processedBytes: 0,
    totalBytes: 0,
    processedItems: 0,
    totalItems: 0,
  };
}
function conflict(operationId = 'own'): Conflict {
  const entry = {
    name: 'file',
    path: '/file',
    type: 'file' as const,
    extension: '',
    size: 0,
    hidden: false,
    readonly: false,
    directoryTarget: false,
  };
  return { operationId, source: entry, destination: entry };
}

describe('operation controller', () => {
  it('reports native failure and refresh failure while releasing busy state', async () => {
    const { deps, controller } = setup();
    const nativeError = { code: 'permission_denied', message: 'Cannot copy' };
    const refreshError = new Error('Cannot refresh');
    deps.completed.mockRejectedValue(refreshError);
    await controller.start(operation, 'left');
    controller.progress({ ...progress('own', 'failed'), error: nativeError });
    await Promise.resolve();
    await Promise.resolve();
    expect(deps.failed).toHaveBeenCalledWith(nativeError);
    expect(deps.failed).toHaveBeenCalledWith(refreshError);
    expect(controller.state.busy).toBe(false);
  });

  it('reports cancellation transport failures without clearing the active operation', async () => {
    const { deps, controller } = setup();
    const error = new Error('Cancel IPC failed');
    deps.cancel.mockRejectedValue(error);
    await controller.start(operation, 'left');
    await controller.cancel();
    expect(deps.failed).toHaveBeenCalledWith(error);
    expect(controller.state.busy).toBe(true);
  });
  it('ignores unrelated progress and conflicts', async () => {
    const { deps, controller } = setup();
    await controller.start(operation, 'left');
    controller.progress(progress('other', 'completed'));
    controller.conflict(conflict('other'));
    expect(controller.state.busy).toBe(true);
    expect(controller.state.conflict).toBeUndefined();
    expect(deps.completed).not.toHaveBeenCalled();
  });

  it('handles completion before the start response and waits for refresh', async () => {
    const { deps, controller } = setup();
    const start = deferred<string>();
    const refresh = deferred<void>();
    deps.start.mockReturnValue(start.promise);
    deps.completed.mockReturnValue(refresh.promise);
    const pending = controller.start(operation, 'right');
    controller.progress(progress('other', 'completed'));
    controller.progress(progress('own', 'completed'));
    start.resolve('own');
    await pending;
    expect(deps.completed).toHaveBeenCalledOnce();
    expect(deps.completed).toHaveBeenCalledWith(
      progress('own', 'completed'),
      'right',
    );
    expect(controller.state.busy).toBe(true);
    await controller.start(operation, 'left');
    expect(deps.start).toHaveBeenCalledOnce();
    refresh.resolve();
    await refresh.promise;
    expect(controller.state.busy).toBe(false);
  });

  it('buffers an early conflict for only the matching operation', async () => {
    const { deps, controller } = setup();
    const start = deferred<string>();
    deps.start.mockReturnValue(start.promise);
    const pending = controller.start(operation, 'left');
    controller.conflict(conflict('own'));
    start.resolve('own');
    await pending;
    expect(controller.state.conflict?.operationId).toBe('own');
  });

  it('does not erase a newer conflict after resolving the previous one', async () => {
    const { deps, controller } = setup();
    await controller.start(operation, 'left');
    controller.conflict(conflict());
    const response = deferred<void>();
    deps.resolve.mockReturnValue(response.promise);
    const resolving = controller.resolve({ action: 'skip', applyToAll: false });
    controller.conflict({
      ...conflict(),
      destination: { ...conflict().destination, path: '/next' },
    });
    response.resolve();
    await resolving;
    expect(controller.state.conflict?.destination.path).toBe('/next');
  });

  it('cancels only the acknowledged operation', async () => {
    const { deps, controller } = setup();
    await controller.start(operation, 'left');
    await controller.cancel();
    expect(deps.cancel).toHaveBeenCalledWith('own');
  });

  it('queues cancellation until the start response supplies the ID', async () => {
    const { deps, controller } = setup();
    const response = deferred<string>();
    deps.start.mockReturnValue(response.promise);
    const pending = controller.start(operation, 'left');
    await controller.cancel();
    expect(deps.cancel).not.toHaveBeenCalled();
    response.resolve('own');
    await pending;
    expect(deps.cancel).toHaveBeenCalledWith('own');
  });

  it('handles start failures and ignores events after disposal', async () => {
    const { deps, controller } = setup();
    deps.start.mockRejectedValue(new Error('failed'));
    await controller.start(operation, 'left');
    expect(controller.state.busy).toBe(false);
    expect(deps.failed).toHaveBeenCalledOnce();
    controller.dispose();
    controller.progress(progress());
    await controller.start(operation, 'left');
    expect(deps.start).toHaveBeenCalledOnce();
  });
});
