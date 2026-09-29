import { expect, it } from 'vitest';
import { bytes, date } from './format';

it.each([
  [0, '0 B'],
  [1023, '1023 B'],
  [1024, '1.0 KB'],
  [1536, '1.5 KB'],
  [1024 ** 2, '1.0 MB'],
  [1024 ** 3, '1.0 GB'],
  [1024 ** 4, '1.0 TB'],
] as const)('formats %i bytes as %s', (value, expected) =>
  expect(bytes(value)).toBe(expected),
);
it('represents missing dates and handles the Unix epoch', () => {
  expect(date()).toBe('—');
  expect(date(0)).not.toBe('—');
  expect(date(0)).not.toContain('Invalid');
});
it('uses 24-hour time regardless of locale defaults', () => {
  expect(date(0)).not.toMatch(/AM|PM/i);
  expect(date(1756492800)).not.toMatch(/AM|PM/i);
});
