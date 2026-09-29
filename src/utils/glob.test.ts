import { describe, expect, it } from 'vitest';
import { matchesGlob } from './glob';

describe('glob matching', () => {
  it('matches stars and question marks case-insensitively', () => {
    expect(matchesGlob('tool.EXE', '*.exe')).toBe(true);
    expect(matchesGlob('tool.exe.bak', '*.exe')).toBe(false);
    expect(matchesGlob('a.c', '?.?')).toBe(true);
    expect(matchesGlob('ab.c', '?.?')).toBe(false);
  });

  it('matches character classes and ranges', () => {
    expect(matchesGlob('file7.txt', 'file[0-9].txt')).toBe(true);
    expect(matchesGlob('filex.txt', 'file[0-9].txt')).toBe(false);
    expect(matchesGlob('filex.txt', 'file[!0-9].txt')).toBe(true);
  });

  it('treats the default *.* mask as all names, including extensionless ones', () => {
    expect(matchesGlob('README', '*.*')).toBe(true);
    expect(matchesGlob('archive.tar', '*.*')).toBe(true);
  });

  it('treats malformed character classes literally', () => {
    expect(matchesGlob('file[abc', 'file[abc')).toBe(true);
  });
});
