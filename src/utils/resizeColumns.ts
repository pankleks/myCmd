export function resizeAdjacentColumns(
  widths: readonly number[],
  minimums: readonly number[],
  index: number,
  delta: number,
): number[] {
  if (
    widths.length !== minimums.length ||
    index < 0 ||
    index >= widths.length - 1
  ) {
    throw new RangeError('A resizable column divider must have two columns.');
  }

  const minDelta = minimums[index] - widths[index];
  const maxDelta = widths[index + 1] - minimums[index + 1];
  const adjustedDelta = Math.max(minDelta, Math.min(maxDelta, delta));
  const resized = [...widths];
  resized[index] += adjustedDelta;
  resized[index + 1] -= adjustedDelta;
  return resized;
}

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
