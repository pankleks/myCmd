import { displayPath } from './paths';

/** Only intercept standalone directory changes, never compound shell commands. */
export function cdTarget(command: string, cwd: string): string | undefined {
  const match = /^\s*(?:cd|chdir)\s+(?:\/d\s+)?(.+?)\s*$/i.exec(command);
  if (!match) return undefined;
  const argument = match[1];
  const quoted = /^"[^"]*"$/.test(argument);
  if (!quoted && /[&|<>;"`]/.test(argument)) return undefined;
  const target = quoted ? argument.slice(1, -1) : argument;
  if (!target || /[%$]/.test(target)) return undefined;
  const base = displayPath(cwd);
  const windows = /^[a-z]:[\\/]/i.test(base) || base.startsWith('\\\\');
  if (/^[a-z]:[\\/]/i.test(target) || target.startsWith('\\\\')) return target;
  // Drive-relative paths need shell drive-state semantics we do not have.
  if (/^[a-z]:/i.test(target)) return undefined;
  if (windows) {
    if (/^[\\/]/.test(target)) {
      const drive = /^[a-z]:/i.exec(base)?.[0];
      return drive ? `${drive}${target}` : undefined;
    }
    return `${base.replace(/[\\/]$/, '')}\\${target}`;
  }
  return target.startsWith('/')
    ? target
    : `${base.replace(/\/$/, '')}/${target}`;
}
