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
