/** Debounce filesystem bursts without starving refreshes during continuous writes. */
export function createDirectoryRefresh<Key extends string>(
  dependencies: {
    path: (key: Key) => string;
    enabled: () => boolean;
    refresh: (key: Key) => void | Promise<unknown>;
    failed?: (error: unknown) => void;
  },
  delay = 180,
  maxDelay = 1000,
) {
  const pending = new Map<
    Key,
    { path: string; deadline: number; timer: ReturnType<typeof setTimeout> }
  >();
  const running = new Map<Key, { dirtyPath?: string }>();
  let disposed = false;
  function schedule(key: Key, path: string) {
    if (disposed || !dependencies.enabled() || dependencies.path(key) !== path)
      return;
    const active = running.get(key);
    if (active) {
      active.dirtyPath = path;
      return;
    }
    const previous = pending.get(key);
    if (previous) clearTimeout(previous.timer);
    const deadline =
      previous?.path === path ? previous.deadline : Date.now() + maxDelay;
    const timer = setTimeout(
      () => {
        pending.delete(key);
        if (
          !disposed &&
          dependencies.enabled() &&
          dependencies.path(key) === path
        )
          void refresh(key);
      },
      Math.max(0, Math.min(delay, deadline - Date.now())),
    );
    pending.set(key, { path, deadline, timer });
  }
  async function refresh(key: Key) {
    const active: { dirtyPath?: string } = {};
    running.set(key, active);
    try {
      await dependencies.refresh(key);
    } catch (error) {
      if (!disposed) dependencies.failed?.(error);
    } finally {
      running.delete(key);
      if (active.dirtyPath) schedule(key, active.dirtyPath);
    }
  }
  function dispose() {
    disposed = true;
    for (const item of pending.values()) clearTimeout(item.timer);
    pending.clear();
  }
  return { schedule, dispose };
}
