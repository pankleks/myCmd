import { describe, expect, it } from 'vitest';
import { formatJsonPreview } from './jsonPreview';

describe('JSON preview formatting', () => {
  it('indents without changing large numbers, duplicate keys or escaped strings', () => {
    const source = '{"n":9007199254740993,"n":1,"s":"\\u0061"}';
    expect(formatJsonPreview(source)).toBe(
      '{\n  "n": 9007199254740993,\n  "n": 1,\n  "s": "\\u0061"\n}',
    );
  });
  it('preserves JSONC comments and trailing commas', () => {
    const formatted = formatJsonPreview('{/* note */"a":1,}', true);
    expect(formatted).toContain('/* note */');
    expect(formatted).toContain('"a": 1,');
    expect(formatJsonPreview(formatted, true)).toBe(formatted);
  });
  it('rejects invalid JSON and JSONC syntax in strict JSON', () => {
    for (const source of ['', '{"a":}', '{"a":1,}', '{/* comment */"a":1}'])
      expect(() => formatJsonPreview(source)).toThrow(
        'Cannot format invalid JSON',
      );
    expect(() => formatJsonPreview('{"a":}', true)).toThrow();
  });
});
