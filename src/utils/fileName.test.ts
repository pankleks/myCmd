import { expect, it } from 'vitest';
import { fileNameWithoutExtension } from './fileName';

it('removes only the displayed final extension', () => {
  expect(fileNameWithoutExtension('report.txt', 'txt')).toBe('report');
  expect(fileNameWithoutExtension('archive.tar.gz', 'gz')).toBe('archive.tar');
  expect(fileNameWithoutExtension('APP.EXE', 'exe')).toBe('APP');
  expect(fileNameWithoutExtension('README', '')).toBe('README');
  expect(fileNameWithoutExtension('.gitignore', '')).toBe('.gitignore');
  expect(fileNameWithoutExtension('.env', 'env')).toBe('.env');
  expect(fileNameWithoutExtension('report.txt', 'zip')).toBe('report.txt');
});
