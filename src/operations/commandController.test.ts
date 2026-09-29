import { describe, expect, it, vi } from 'vitest';
import type { CommandResult } from '../filesystem/api';
import {
  commandOutput,
  createCommandController,
} from './commandController.svelte';

const success: CommandResult = {
  success: true,
  exitCode: 0,
  stdout: '',
  stderr: '',
  cancelled: false,
  timedOut: false,
  outputTruncated: false,
  outputIncomplete: false,
};
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (cause: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
function setup() {
  let id = 0;
  const dependencies = {
    run: vi.fn().mockResolvedValue(success),
    cancel: vi.fn().mockResolvedValue(undefined),
    refresh: vi.fn().mockResolvedValue(undefined),
    report: vi.fn(),
    cancelFailed: vi.fn(),
    createId: () => `command-${++id}`,
  };
  return { dependencies, controller: createCommandController(dependencies) };
}
describe('command controller', () => {
  it('creates a command ID with the platform UUID generator by default', async () => {
    const { dependencies } = setup();
    const randomUUID = vi.fn().mockReturnValue('generated-id');
    vi.stubGlobal('crypto', { randomUUID });
    try {
      const controller = createCommandController({
        ...dependencies,
        createId: undefined,
      });
      await controller.run('echo test', '/files');
      expect(dependencies.run).toHaveBeenCalledWith(
        'echo test',
        '/files',
        'generated-id',
      );
      expect(randomUUID).toHaveBeenCalledOnce();
    } finally {
      vi.unstubAllGlobals();
    }
  });
  it('rejects duplicate starts and holds busy state through refresh', async () => {
    const { dependencies, controller } = setup();
    const refresh = deferred<void>();
    dependencies.refresh.mockReturnValue(refresh.promise);
    const running = controller.run('echo test', '/files');
    await Promise.resolve();
    expect(controller.state.busy).toBe(true);
    expect(controller.state.executing).toBe(false);
    await controller.run('duplicate', '/files');
    await controller.cancel();
    expect(dependencies.run).toHaveBeenCalledTimes(1);
    expect(dependencies.cancel).not.toHaveBeenCalled();
    refresh.resolve();
    await running;
    expect(controller.state.busy).toBe(false);
  });
  it('ignores empty commands and missing working directories', async () => {
    const { dependencies, controller } = setup();
    await controller.run('  ', '/files');
    await controller.run('echo test', '');
    expect(dependencies.run).not.toHaveBeenCalled();
  });
  it('isolates cancellation and suppresses duplicate requests', async () => {
    const { dependencies, controller } = setup();
    const result = deferred<CommandResult>();
    const cancellation = deferred<void>();
    dependencies.run.mockReturnValue(result.promise);
    dependencies.cancel.mockReturnValue(cancellation.promise);
    const running = controller.run('sleep', '/files');
    const cancelling = controller.cancel();
    await controller.cancel();
    expect(dependencies.cancel).toHaveBeenCalledExactlyOnceWith('command-1');
    cancellation.resolve();
    await cancelling;
    result.resolve({ ...success, success: false, cancelled: true });
    await running;
    expect(dependencies.report).toHaveBeenCalledWith(
      'sleep',
      'Command cancelled.',
    );
  });
  it('allows retrying cancellation after a failure', async () => {
    const { dependencies, controller } = setup();
    const result = deferred<CommandResult>();
    dependencies.run.mockReturnValue(result.promise);
    dependencies.cancel.mockRejectedValueOnce(new Error('IPC failed'));
    const running = controller.run('sleep', '/files');
    await controller.cancel();
    expect(dependencies.cancelFailed).toHaveBeenCalledOnce();
    expect(controller.state.cancelling).toBe(false);
    await controller.cancel();
    expect(dependencies.cancel).toHaveBeenCalledTimes(2);
    result.resolve(success);
    await running;
  });
  it('ignores a stale cancellation failure after a newer command starts', async () => {
    const { dependencies, controller } = setup();
    const first = deferred<CommandResult>();
    const second = deferred<CommandResult>();
    const cancellation = deferred<void>();
    dependencies.run
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    dependencies.cancel.mockReturnValue(cancellation.promise);
    const running = controller.run('first', '/files');
    const cancelling = controller.cancel();
    first.resolve(success);
    await running;
    const newer = controller.run('second', '/files');
    cancellation.reject(new Error('stale'));
    await cancelling;
    expect(dependencies.cancelFailed).not.toHaveBeenCalled();
    expect(controller.state.executing).toBe(true);
    second.resolve(success);
    await newer;
  });
  it('reports invocation and refresh failures and releases busy state', async () => {
    const { dependencies, controller } = setup();
    dependencies.run.mockRejectedValueOnce(new Error('invoke failed'));
    await controller.run('first', '/files');
    expect(dependencies.report).toHaveBeenCalledWith('first', 'invoke failed');
    expect(controller.state.busy).toBe(false);
    dependencies.refresh.mockRejectedValueOnce(new Error('refresh failed'));
    await controller.run('second', '/files');
    expect(dependencies.report).toHaveBeenCalledWith(
      'second',
      'refresh failed',
    );
    expect(controller.state.busy).toBe(false);
  });
});
describe('command output', () => {
  it('does not display successful complete output', () => {
    expect(
      commandOutput({ ...success, stdout: 'normal output' }),
    ).toBeUndefined();
  });
  it('keeps partial output alongside distinct warnings', () => {
    expect(
      commandOutput({
        ...success,
        timedOut: true,
        success: false,
        outputTruncated: true,
        outputIncomplete: true,
        stderr: 'error',
        stdout: 'partial',
      }),
    ).toContain('error\npartial');
    expect(commandOutput({ ...success, outputIncomplete: true })).toContain(
      'Output capture incomplete',
    );
    expect(commandOutput({ ...success, outputTruncated: true })).toContain(
      'Output truncated',
    );
    expect(
      commandOutput({ ...success, success: false, timedOut: true }),
    ).toContain('120 seconds');
    expect(commandOutput({ ...success, success: false })).toBe(
      'The command failed without output.',
    );
  });
});
