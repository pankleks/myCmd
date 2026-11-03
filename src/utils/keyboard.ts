export interface QuickFindKey {
  key: string;
  altKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  altGraph: boolean;
}

export function isQuickFindTrigger(event: QuickFindKey): boolean {
  if (event.metaKey) return false;
  if (!event.key || event.key.length !== 1 || event.key === ' ') return false;
  if (!event.altKey && !event.altGraph) return false;
  if (!event.altGraph && event.shiftKey) return false;
  return true;
}

/** Held modifiers in a fixed order, e.g. '', 'alt', 'shift', 'ctrl+shift'. */
export function modifiers(
  event: Pick<KeyboardEvent, 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>,
): string {
  return [
    event.ctrlKey && 'ctrl',
    event.metaKey && 'meta',
    event.altKey && 'alt',
    event.shiftKey && 'shift',
  ]
    .filter(Boolean)
    .join('+');
}
