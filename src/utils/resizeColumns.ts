export function resizeColumn(
  widths: readonly number[],
  minimums: readonly number[],
  index: number,
  delta: number,
): number[] {
  if (
    widths.length !== minimums.length ||
    index < 0 ||
    index >= widths.length
  ) {
    throw new RangeError('A resizable column must exist.');
  }

  const resized = [...widths];
  resized[index] = Math.max(minimums[index], widths[index] + delta);
  return resized;
}
