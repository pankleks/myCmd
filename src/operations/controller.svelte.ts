import type { Conflict, FileOperation, Progress, Resolution } from './types';

export interface OperationDependencies {
  start: (operation: FileOperation) => Promise<string>;
  cancel: (id: string) => Promise<void>;
  resolve: (id: string, resolution: Resolution) => Promise<void>;
  completed: (progress: Progress, side: 'left' | 'right') => Promise<void>;
  failed: (error: unknown) => void;
}

export function createOperationController(dependencies: OperationDependencies) {
  const state = $state({
    busy: false,
    id: undefined as string | undefined,
    progress: undefined as Progress | undefined,
    conflict: undefined as Conflict | undefined,
  });
  let side: 'left' | 'right' = 'left';
  let finishing = false;
  let cancelRequested = false;
  let disposed = false;
  // Native events can arrive before invoke(start_operation) resolves. Keep
  // only the latest state for each ID until the actual ID is known.
  const early = new Map<string, { progress?: Progress; conflict?: Conflict }>();

  function buffer(id: string) {
    if (!early.has(id)) {
      if (early.size >= 64) early.delete(early.keys().next().value!);
      early.set(id, {});
    }
    return early.get(id)!;
  }

  async function finish(progress: Progress) {
    finishing = true;
    state.conflict = undefined;
    if (progress.state === 'failed' && progress.error)
      dependencies.failed(progress.error);
    try {
      await dependencies.completed(progress, side);
    } catch (error) {
      if (!disposed) dependencies.failed(error);
    } finally {
      if (!disposed) state.busy = false;
      finishing = false;
    }
  }

  function progress(progress: Progress) {
    if (disposed || !state.busy || finishing) return;
    if (!state.id) {
      buffer(progress.operationId).progress = progress;
      return;
    }
    if (progress.operationId !== state.id) return;
    state.progress = progress;
    if (['completed', 'failed', 'cancelled'].includes(progress.state))
      void finish(progress);
  }

  function conflict(conflict: Conflict) {
    if (disposed || !state.busy || finishing) return;
    if (!state.id) {
      buffer(conflict.operationId).conflict = conflict;
      return;
    }
    if (conflict.operationId === state.id) state.conflict = conflict;
  }

  async function start(operation: FileOperation, sourceSide: 'left' | 'right') {
    if (disposed || state.busy) return;
    state.busy = true;
    state.id = undefined;
    state.progress = undefined;
    state.conflict = undefined;
    side = sourceSide;
    cancelRequested = false;
    early.clear();
    try {
      const id = await dependencies.start(operation);
      if (disposed) return;
      state.id = id;
      const pending = early.get(id);
      early.clear();
      if (pending?.conflict) conflict(pending.conflict);
      if (pending?.progress) progress(pending.progress);
      if (cancelRequested) await cancel();
    } catch (error) {
      early.clear();
      if (!disposed) {
        state.busy = false;
        dependencies.failed(error);
      }
    }
  }

  async function cancel() {
    if (!state.busy || finishing || disposed) return;
    if (!state.id) {
      cancelRequested = true;
      return;
    }
    try {
      await dependencies.cancel(state.id);
    } catch (error) {
      if (!disposed) dependencies.failed(error);
    }
  }

  async function resolve(resolution: Resolution) {
    const pending = state.conflict;
    if (!pending || disposed || finishing) return;
    await dependencies.resolve(pending.operationId, resolution);
    // A subsequent conflict may already have arrived while IPC was pending.
    if (!disposed && state.conflict === pending) state.conflict = undefined;
  }

  return {
    state,
    start,
    cancel,
    resolve,
    progress,
    conflict,
    dispose() {
      disposed = true;
      early.clear();
    },
  };
}
