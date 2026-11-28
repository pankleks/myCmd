export function bytes(value: number) {
  if (value < 1024) return `${value} B`;
  const unit = Math.min(Math.floor(Math.log(value) / Math.log(1024)), 4);
  return `${(value / 1024 ** unit).toFixed(1)} ${['B', 'KB', 'MB', 'GB', 'TB'][unit]}`;
}
export function date(value?: number) {
  return value == null
    ? '—'
    : new Date(value * 1000).toLocaleString(undefined, {
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
        hourCycle: 'h23',
      });
}
/** "1 item", "2 items". */
export function plural(count: number, word: string, many = `${word}s`) {
  return `${count} ${count === 1 ? word : many}`;
}
/** What the system calls deleted items that can be restored. */
export const trashName =
  typeof navigator !== 'undefined' && /^Win/.test(navigator.platform)
    ? 'Recycle Bin'
    : 'Trash';
