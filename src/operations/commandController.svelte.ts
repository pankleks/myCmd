import type { CommandResult } from '../filesystem/api';
import { errorMessage } from '../filesystem/api';

export function commandOutput(result: CommandResult): string | undefined {
  if (result.success && !result.outputTruncated && !result.outputIncomplete)
    return undefined;
  return (
    [
      result.cancelled
        ? 'Command cancelled.'
        : result.timedOut
          ? 'Command timed out after 120 seconds.'
          : '',
      result.outputTruncated ? 'Output truncated (1 MiB per stream).' : '',
      result.outputIncomplete
        ? 'Output capture incomplete: a stream remained open or could not be read.'
        : '',
      result.stderr,
      result.stdout,
    ]
      .filter(Boolean)
      .join('\n')
      .trim() || 'The command failed without output.'
  );
}

export function createCommandController(dependencies: {
  run: (command: string, cwd: string, id: string) => Promise<CommandResult>;
  cancel: (id: string) => Promise<void>;
  refresh: () => Promise<unknown>;
  report: (command: string, output: string) => void;
  cancelFailed: (error: unknown) => void;
  createId?: () => string;
}) {
  const state = $state({ busy: false, executing: false, cancelling: false });
  let id: string | undefined;
  async function run(command: string, cwd: string) {
    if (state.busy || !command.trim() || !cwd) return;
    state.busy = true;
    state.executing = true;
    try {
      id = (dependencies.createId ?? (() => crypto.randomUUID()))();
      const result = await dependencies.run(command, cwd, id);
      state.executing = false;
      const output = commandOutput(result);
      if (output) dependencies.report(command, output);
      await dependencies.refresh();
    } catch (error) {
      dependencies.report(command, errorMessage(error));
    } finally {
      id = undefined;
      state.executing = false;
      state.cancelling = false;
      state.busy = false;
    }
  }
  async function cancel() {
    if (!state.executing || state.cancelling || !id) return;
    const cancellingId = id;
    state.cancelling = true;
    try {
      await dependencies.cancel(cancellingId);
    } catch (error) {
      if (id === cancellingId) dependencies.cancelFailed(error);
    } finally {
      if (id === cancellingId) state.cancelling = false;
    }
  }
  return { state, run, cancel };
}
