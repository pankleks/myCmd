import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createDirectoryRefresh } from './directoryRefresh';

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(0);
});
afterEach(() => vi.useRealTimers());
function setup() {
  const paths = { left: '/a', right: '/b' };
  const dependencies = {
    path: (key: 'left' | 'right') => paths[key],
    enabled: vi.fn(() => true),
    refresh: vi.fn(),
  };
  return {
    paths,
    dependencies,
    scheduler: createDirectoryRefresh(dependencies),
  };
}
describe('directory refresh scheduler', () => {
  it('queues one follow-up instead of overlapping slow reads', async () => {
    const { scheduler, dependencies } = setup();
    let finish!: () => void;
    dependencies.refresh.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    );
    scheduler.schedule('left', '/a');
    await vi.advanceTimersByTimeAsync(180);
    for (let index = 0; index < 20; index++) {
      scheduler.schedule('left', '/a');
      await vi.advanceTimersByTimeAsync(100);
    }
    expect(dependencies.refresh).toHaveBeenCalledTimes(1);
    finish();
    await vi.advanceTimersByTimeAsync(180);
    expect(dependencies.refresh).toHaveBeenCalledTimes(2);
  });

  it('keeps the other panel independent of an in-flight read', async () => {
    const { scheduler, dependencies } = setup();
    let finish!: () => void;
    dependencies.refresh.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    );
    scheduler.schedule('left', '/a');
    await vi.advanceTimersByTimeAsync(180);
    scheduler.schedule('right', '/b');
    await vi.advanceTimersByTimeAsync(180);
    expect(dependencies.refresh.mock.calls).toEqual([['left'], ['right']]);
    finish();
    await Promise.resolve();
  });

  it('drops queued changes after navigation', async () => {
    const { scheduler, dependencies, paths } = setup();
    let finish!: () => void;
    dependencies.refresh.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    );
    scheduler.schedule('left', '/a');
    await vi.advanceTimersByTimeAsync(180);
    scheduler.schedule('left', '/a');
    paths.left = '/new';
    finish();
    await vi.advanceTimersByTimeAsync(1000);
    expect(dependencies.refresh).toHaveBeenCalledTimes(1);
    scheduler.dispose();
  });

  it('reports refresh rejection and allows the next refresh', async () => {
    const { dependencies } = setup();
    const failed = vi.fn();
    const scheduler = createDirectoryRefresh({ ...dependencies, failed });
    const error = new Error('read failed');
    dependencies.refresh.mockRejectedValueOnce(error);
    scheduler.schedule('left', '/a');
    await vi.advanceTimersByTimeAsync(180);
    expect(failed).toHaveBeenCalledWith(error);
    scheduler.schedule('left', '/a');
    await vi.advanceTimersByTimeAsync(180);
    expect(dependencies.refresh).toHaveBeenCalledTimes(2);
  });

  it('does not reschedule an in-flight refresh after disposal', async () => {
    const { scheduler, dependencies } = setup();
    let finish!: () => void;
    dependencies.refresh.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    );
    scheduler.schedule('left', '/a');
    await vi.advanceTimersByTimeAsync(180);
    scheduler.schedule('left', '/a');
    scheduler.dispose();
    finish();
    await vi.advanceTimersByTimeAsync(1000);
    expect(dependencies.refresh).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);
  });
  it('coalesces bursts independently for each panel', () => {
    const { scheduler, dependencies } = setup();
    scheduler.schedule('left', '/a');
    vi.advanceTimersByTime(100);
    scheduler.schedule('left', '/a');
    scheduler.schedule('right', '/b');
    vi.advanceTimersByTime(179);
    expect(dependencies.refresh).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(dependencies.refresh.mock.calls).toEqual([['left'], ['right']]);
  });
  it('refreshes within one second even under continuous changes', () => {
    const { scheduler, dependencies } = setup();
    scheduler.schedule('left', '/a');
    for (let index = 0; index < 9; index++) {
      vi.advanceTimersByTime(100);
      scheduler.schedule('left', '/a');
    }
    vi.advanceTimersByTime(99);
    expect(dependencies.refresh).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(dependencies.refresh).toHaveBeenCalledExactlyOnceWith('left');
  });
  it('does not refresh a panel that navigated away', () => {
    const { scheduler, dependencies, paths } = setup();
    scheduler.schedule('left', '/a');
    paths.left = '/new';
    vi.advanceTimersByTime(180);
    expect(dependencies.refresh).not.toHaveBeenCalled();
  });
  it('starts a fresh debounce deadline after navigation', () => {
    const { scheduler, dependencies, paths } = setup();
    scheduler.schedule('left', '/a');
    vi.advanceTimersByTime(100);
    paths.left = '/new';
    scheduler.schedule('left', '/new');
    vi.advanceTimersByTime(179);
    expect(dependencies.refresh).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(dependencies.refresh).toHaveBeenCalledExactlyOnceWith('left');
  });
  it('ignores unrelated paths and suppresses work while disabled', () => {
    const { scheduler, dependencies } = setup();
    scheduler.schedule('left', '/unrelated');
    dependencies.enabled.mockReturnValue(false);
    scheduler.schedule('left', '/a');
    vi.advanceTimersByTime(1000);
    expect(dependencies.refresh).not.toHaveBeenCalled();
    dependencies.enabled.mockReturnValue(true);
    scheduler.schedule('left', '/a');
    dependencies.enabled.mockReturnValue(false);
    vi.advanceTimersByTime(180);
    expect(dependencies.refresh).not.toHaveBeenCalled();
  });
  it('cancels pending work on disposal and ignores later events', () => {
    const { scheduler, dependencies } = setup();
    scheduler.schedule('left', '/a');
    scheduler.dispose();
    scheduler.schedule('right', '/b');
    vi.advanceTimersByTime(1000);
    expect(dependencies.refresh).not.toHaveBeenCalled();
    expect(vi.getTimerCount()).toBe(0);
  });
});
